use std::fmt;

use crate::waves::{AsBase58String, AssetId};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Asset {
    Waves,
    IssuedAsset(AssetId),
}

impl Asset {
    pub const WAVES_ASSET_ID: &str = "WAVES";

    pub fn from_id(id: &str) -> Result<Self, ()> {
        if id == Self::WAVES_ASSET_ID {
            Ok(Asset::Waves)
        } else {
            let asset_id = AssetId::from_string(id).map_err(|_| ())?;
            Ok(Asset::IssuedAsset(asset_id))
        }
    }

    pub fn id(&self) -> String {
        match self {
            Asset::Waves => Self::WAVES_ASSET_ID.to_string(),
            Asset::IssuedAsset(asset_id) => asset_id.as_base58_string(),
        }
    }
}

impl fmt::Debug for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id())
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AssetPair {
    pub amount_asset: Asset,
    pub price_asset: Asset,
}

impl AssetPair {
    pub fn assets_as_ref(&self) -> (&Asset, &Asset) {
        (&self.amount_asset, &self.price_asset)
    }
}

impl fmt::Debug for AssetPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.amount_asset, self.price_asset)
    }
}
