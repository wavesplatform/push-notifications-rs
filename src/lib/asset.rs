use crate::{error::Error, model::Asset};
use wavesexchange_apis::{
    assets::dto::{AssetInfo, OutputFormat},
    AssetsService, HttpClient,
};
use wavesexchange_loaders::{CachedLoader, Loader as _, TimedCache};

type Ticker = String;
type Decimals = u8;

#[derive(Debug, Clone)]
struct LocalAssetInfo {
    ticker: Option<Ticker>,
    decimals: Decimals,
}

#[derive(Clone)]
pub struct RemoteGateway {
    assets_client: HttpClient<AssetsService>,
}

impl RemoteGateway {
    pub fn new(asset_service_url: impl AsRef<str>) -> Self {
        let url = asset_service_url.as_ref();
        let assets_client = HttpClient::<AssetsService>::from_base_url(url);
        RemoteGateway { assets_client }
    }

    pub async fn preload(&self, assets: Vec<Asset>) -> Result<(), Error> {
        let _ = self.load_many(assets).await?;
        Ok(())
    }

    pub async fn ticker(&self, asset: &Asset) -> Result<Option<Ticker>, Error> {
        self.asset_info(asset).await.map(|a| a.ticker)
    }

    pub async fn decimals(&self, asset: &Asset) -> Result<Decimals, Error> {
        self.asset_info(asset).await.map(|a| a.decimals)
    }

    async fn asset_info(&self, asset: &Asset) -> Result<LocalAssetInfo, Error> {
        self.load(asset.to_owned()).await.map_err(Error::from)
    }
}

#[async_trait]
impl CachedLoader<Asset, LocalAssetInfo> for RemoteGateway {
    type Cache = TimedCache<Asset, LocalAssetInfo>;

    type Error = Error;

    async fn load_fn(&mut self, keys: &[Asset]) -> Result<Vec<LocalAssetInfo>, Self::Error> {
        let asset_ids = keys.iter().map(|k| k.id()).collect::<Vec<_>>();
        let assets = self
            .assets_client
            .get(asset_ids, None, OutputFormat::Full, false)
            .await?;
        assert_eq!(assets.data.len(), keys.len());

        Ok(assets
            .data
            .into_iter()
            .zip(keys)
            .map(|(asset, asset_id)| match asset.data {
                Some(AssetInfo::Full(a)) => LocalAssetInfo {
                    ticker: a.ticker,
                    decimals: a.precision as u8,
                },
                Some(AssetInfo::Brief(_)) => {
                    unreachable!("Broken API: Full info expected for asset {}", asset_id);
                }
                None => {
                    panic!("No AssetInfo for asset {}", asset_id);
                }
            })
            .collect())
    }

    fn init_cache() -> Self::Cache {
        //TODO Ugly API requiring raw seconds instead of a Duration. Find ways to refactor.
        TimedCache::with_lifespan(60 * 60 * 24)
    }
}
