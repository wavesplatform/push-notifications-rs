use crate::error::Error;
use crate::model::{AsBase58String, AssetId};
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

    pub async fn ticker(&self, asset_id: &AssetId) -> Result<Option<Ticker>, Error> {
        self.asset_info(asset_id).await.map(|a| a.ticker)
    }

    pub async fn decimals(&self, asset_id: &AssetId) -> Result<Decimals, Error> {
        self.load(asset_id.to_owned()).await.map_err(Error::from)
    }

    async fn asset_info(&self, asset_id: &AssetId) -> Result<LocalAssetInfo, Error> {
        self.load(asset_id.to_owned()).await.map_err(Error::from)
    }
}

#[async_trait]
impl CachedLoader<AssetId, Decimals> for RemoteGateway {
    type Cache = UnboundCache<AssetId, Decimals>;

    type Error = Error;

    async fn load_fn(&mut self, keys: &[AssetId]) -> Result<Vec<Decimals>, Self::Error> {
        let mut result = vec![];
        for asset_id in keys {
            let asset = self.asset_info(asset_id).await?;
            result.push(asset.decimals)
        }
        Ok(result)
    }

    fn init_cache() -> Self::Cache {
        UnboundCache::new()
    }
}

#[async_trait]
impl CachedLoader<AssetId, LocalAssetInfo> for RemoteGateway {
    type Cache = TimedCache<AssetId, LocalAssetInfo>;

    type Error = Error;

    async fn load_fn(&mut self, keys: &[AssetId]) -> Result<Vec<LocalAssetInfo>, Self::Error> {
        let mut result = vec![];
        for asset_id in keys {
            let asset_id = asset_id.as_base58_string();
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
