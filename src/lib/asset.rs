use crate::error::Error;
use wavesexchange_apis::{
    assets::dto::{AssetInfo, OutputFormat},
    AssetsService, HttpClient,
};
use wavesexchange_loaders::{CachedLoader, Loader as _, TimedCache, UnboundCache};

type Ticker = String;
type Decimals = i32;
type AssetId = String;

#[derive(Clone)]
pub struct RemoteGateway {
    assets_client: HttpClient<AssetsService>,
}

impl RemoteGateway {
    pub fn new(assets_client: HttpClient<AssetsService>) -> Self {
        RemoteGateway { assets_client }
    }

    pub async fn ticker(&self, asset_id: &AssetId) -> Result<Ticker, Error> {
        self.load(asset_id.to_owned()).await.map_err(Error::from)
    }

    pub async fn decimals(&self, asset_id: &AssetId) -> Result<Decimals, Error> {
        self.load(asset_id.to_owned()).await.map_err(Error::from)
    }
}

#[async_trait]
impl CachedLoader<AssetId, Ticker> for RemoteGateway {
    type Cache = TimedCache<AssetId, Ticker>;

    type Error = Error;

    async fn load_fn(&mut self, keys: &[AssetId]) -> Result<Vec<Ticker>, Self::Error> {
        let mut result = vec![];
        for asset_id in keys {
            let mut asset = self
                .assets_client
                .get([asset_id], None, OutputFormat::Brief, false)
                .await?;
            if let Some(AssetInfo::Brief(a)) = asset.data.remove(0).data {
                result.push(a.ticker.unwrap());
            } else {
                unreachable!("Brief info expected")
            }
        }
        Ok(result)
    }

    fn init_cache() -> Self::Cache {
        TimedCache::with_lifespan(60 * 60 * 24)
    }
}

#[async_trait]
impl CachedLoader<AssetId, Decimals> for RemoteGateway {
    type Cache = UnboundCache<AssetId, Decimals>;

    type Error = Error;

    async fn load_fn(&mut self, keys: &[AssetId]) -> Result<Vec<Decimals>, Self::Error> {
        let mut result = vec![];
        for asset_id in keys {
            let mut asset = self
                .assets_client
                .get([asset_id], None, OutputFormat::Brief, false)
                .await?;
            if let Some(AssetInfo::Full(a)) = asset.data.remove(0).data {
                result.push(a.precision)
            } else {
                unreachable!("Full info expected")
            }
        }
        Ok(result)
    }

    fn init_cache() -> Self::Cache {
        UnboundCache::new()
    }
}
