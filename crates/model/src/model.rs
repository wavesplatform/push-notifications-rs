//! Waves blockchain data model: address, asset, amount etc.

use std::fmt;

//TODO probably using data model from the `waves_rust` crate is suboptimal:
// that crate stores data as bytes while we often need base58 strings,
// so a lot of unnecessary conversion occurs
pub use waves_rust::model::{Address, Amount, AssetId};

pub type Lang = String;

use waves_rust::model::ByteString;

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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(i64);

pub type DateTimeUtc = chrono::DateTime<chrono::Utc>;
pub type DateTimeTz = chrono::DateTime<chrono::FixedOffset>;

impl Timestamp {
    pub fn from_unix_timestamp_millis(unix_timestamp: i64) -> Self {
        Timestamp(unix_timestamp)
    }

    pub fn unix_timestamp_millis(&self) -> i64 {
        self.0
    }

    pub fn date_time_utc(&self) -> Option<DateTimeUtc> {
        use chrono::{TimeZone, Utc};
        let unix_timestamp = self.unix_timestamp_millis();
        Utc.timestamp_millis_opt(unix_timestamp).earliest()
    }

    pub fn date_time(&self, utc_offset_seconds: i32) -> Option<DateTimeTz> {
        use chrono::{FixedOffset, TimeZone};
        let unix_timestamp = self.unix_timestamp_millis();
        let tz = FixedOffset::east_opt(utc_offset_seconds)?;
        tz.timestamp_millis_opt(unix_timestamp).earliest()
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(dt) = self.date_time_utc() {
            write!(f, "{}", dt.format("%+")) // like "2001-07-08T00:34:60.026490+09:30"
        } else {
            write!(f, "{}", self.unix_timestamp_millis())
        }
    }
}
