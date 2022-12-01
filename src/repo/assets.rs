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
pub struct Repo {
    assets_client: HttpClient<AssetsService>,
}

impl Repo {
    pub fn new(assets_client: HttpClient<AssetsService>) -> Self {
        Repo { assets_client }
    }

    pub async fn get_ticker_by_id(&self, asset_id: AssetId) -> Result<Ticker, Error> {
        self.load(asset_id).await
    }

    pub async fn get_decimals_by_id(&self, asset_id: AssetId) -> Result<Decimals, Error> {
        self.load(asset_id).await
    }
}

#[async_trait]
impl CachedLoader<AssetId, Ticker> for Repo {
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
impl CachedLoader<AssetId, Decimals> for Repo {
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
