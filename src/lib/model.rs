//! Waves blockchain data model: address, asset, amount etc.

use std::fmt;

//TODO probably using data model from the `waves_rust` crate is suboptimal:
// that crate stores data as bytes while we often need base58 strings,
// so a lot of unnecessary conversion occurs
pub use waves_rust::model::{Address, Amount, AssetId};

pub type Lang = String;

use waves_rust::model::ByteString;

use crate::Error;

pub trait AsBase58String {
    fn as_base58_string(&self) -> String;
}

impl AsBase58String for AssetId {
    fn as_base58_string(&self) -> String {
        self.encoded()
    }
}

impl AsBase58String for Address {
    fn as_base58_string(&self) -> String {
        self.encoded()
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
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

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id())
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AssetPair {
    pub amount_asset: Asset,
    pub price_asset: Asset,
}

impl AssetPair {
    pub fn assets_as_ref(&self) -> (&Asset, &Asset) {
        (&self.amount_asset, &self.price_asset)
    }
}
