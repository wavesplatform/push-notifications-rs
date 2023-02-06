//! Waves blockchain data model: address, asset, amount etc.

//TODO probably using data model from the `waves_rust` crate is suboptimal:
// that crate stores data as bytes while we often need base58 strings,
// so a lot of unnecessary conversion occurs
pub use waves_rust::model::{Address, Amount, AssetId};

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
