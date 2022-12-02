use crate::error::Error;
use crate::model::AssetId;

pub struct RemoteGateway {}

impl RemoteGateway {
    pub async fn ticker(&self, asset_id: &AssetId) -> Result<String, Error> {
        todo!("impl")
    }

    pub async fn decimals(&self, asset_id: &AssetId) -> Result<u8, Error> {
        todo!("impl")
    }
}
