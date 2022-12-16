use crate::{error::Error, model::Asset};
use wavesexchange_apis::{
    assets::dto::{AssetInfo, OutputFormat},
    AssetsService, HttpClient,
};
use wavesexchange_loaders::{CachedLoader, Loader as _, TimedCache, UnboundCache};

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
    pub fn new(assets_url: impl AsRef<str>) -> Self {
        let assets_client = HttpClient::<AssetsService>::from_base_url(assets_url.as_ref());
        RemoteGateway { assets_client }
    }

    pub async fn ticker(&self, asset: &Asset) -> Result<Option<Ticker>, Error> {
        self.asset_info(asset).await.map(|a| a.ticker)
    }

    pub async fn decimals(&self, asset: &Asset) -> Result<Decimals, Error> {
        self.load(asset.to_owned()).await.map_err(Error::from)
    }

    async fn asset_info(&self, asset: &Asset) -> Result<LocalAssetInfo, Error> {
        self.load(asset.to_owned()).await.map_err(Error::from)
    }
}

#[async_trait]
impl CachedLoader<Asset, Decimals> for RemoteGateway {
    type Cache = UnboundCache<Asset, Decimals>;

    type Error = Error;

    async fn load_fn(&mut self, keys: &[Asset]) -> Result<Vec<Decimals>, Self::Error> {
        let mut result = vec![];
        for asset in keys {
            let asset = self.asset_info(asset).await?;
            result.push(asset.decimals)
        }
        Ok(result)
    }

    fn init_cache() -> Self::Cache {
        UnboundCache::new()
    }
}

#[async_trait]
impl CachedLoader<Asset, LocalAssetInfo> for RemoteGateway {
    type Cache = TimedCache<Asset, LocalAssetInfo>;

    type Error = Error;

    async fn load_fn(&mut self, keys: &[Asset]) -> Result<Vec<LocalAssetInfo>, Self::Error> {
        let mut result = vec![];
        for asset in keys {
            let asset_id = asset.id();
            let mut asset = self
                .assets_client
                .get([asset_id], None, OutputFormat::Full, false)
                .await?;
            if let Some(AssetInfo::Full(a)) = asset.data.remove(0).data {
                result.push(LocalAssetInfo {
                    ticker: a.ticker,
                    decimals: a.precision as u8,
                })
            } else {
                unreachable!("Full info expected")
            }
        }
        Ok(result)
    }

    fn init_cache() -> Self::Cache {
        TimedCache::with_lifespan(60 * 60 * 24)
    }
}
