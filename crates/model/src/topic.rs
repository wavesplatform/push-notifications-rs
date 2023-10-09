use std::hash::{Hash, Hasher};

use crate::{asset::Asset, price::Price};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SubscriptionMode {
    Once,
    Repeat,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Topic {
    OrderFulfilled,
    PriceThreshold(PriceThreshold),
}

#[derive(Debug, PartialEq)]
pub struct PriceThreshold {
    pub amount_asset: Asset,
    pub price_asset: Asset,
    pub price_threshold: Price,
}

impl Eq for PriceThreshold {} // Ignore the fact that Topic can contain `f64`

// Has to impl Hash manually because of `f64` inside
impl Hash for PriceThreshold {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.amount_asset.hash(state);
        self.price_asset.hash(state);
        self.price_threshold.to_bits().hash(state);
    }
}
