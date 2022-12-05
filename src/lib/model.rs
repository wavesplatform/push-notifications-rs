//! Waves blockchain data model: address, asset, amount etc.

//TODO probably using data model from the `waves_rust` crate is suboptimal:
// that crate stores data as bytes while we often need base58 strings,
// so a lot of unnecessary conversion occurs
pub use waves_rust::model::{Address, Amount, AssetId, ByteString};

pub type Lang = String;
