use model::asset::Asset;
use wavesexchange_apis::{
    assets::dto::{AssetInfo, OutputFormat},
    AssetsService, HttpClient,
};
use wavesexchange_loaders::{CachedLoader, Loader as _, LoaderError, TimedCache};

type Ticker = String;

#[derive(Debug, Clone)]
struct LocalAssetInfo {
    ticker: Option<Ticker>,
}

#[derive(Clone)]
pub struct RemoteGateway {
    assets_client: HttpClient<AssetsService>,
}

pub type GatewayError = LoaderError<wavesexchange_apis::Error>;

impl RemoteGateway {
    pub fn new(asset_service_url: impl AsRef<str>) -> Self {
        let url = asset_service_url.as_ref();
        let assets_client = HttpClient::<AssetsService>::from_base_url(url);
        RemoteGateway { assets_client }
    }

    pub async fn preload(&self, assets: Vec<Asset>) -> Result<(), GatewayError> {
        let _ = self.load_many(assets).await?;
        Ok(())
    }

    pub async fn ticker(&self, asset: &Asset) -> Result<Option<Ticker>, GatewayError> {
        self.asset_info(asset).await.map(|a| a.ticker)
    }

    async fn asset_info(&self, asset: &Asset) -> Result<LocalAssetInfo, GatewayError> {
        self.load(asset.to_owned()).await
    }
}

#[async_trait]
impl CachedLoader<Asset, LocalAssetInfo> for RemoteGateway {
    type Cache = TimedCache<Asset, LocalAssetInfo>;

    type Error = wavesexchange_apis::Error;

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
                Some(AssetInfo::Full(a)) => LocalAssetInfo { ticker: a.ticker },
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
