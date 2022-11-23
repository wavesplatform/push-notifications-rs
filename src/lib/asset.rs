use crate::error::Error;
use waves_rust::model::AssetId;

pub struct RemoteGateway {}

impl RemoteGateway {
    pub async fn ticker(asset_id: &AssetId) -> Result<String, Error> {
        todo!("impl")
    }

    pub async fn decimals(asset_id: &AssetId) -> Result<u8, Error> {
        todo!("impl")
    }
}
