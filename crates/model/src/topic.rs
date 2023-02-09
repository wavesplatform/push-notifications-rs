use std::hash::{Hash, Hasher};

use crate::{asset::Asset, price::Price};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SubscriptionMode {
    Once,
    Repeat,
}

#[derive(Debug, PartialEq)]
pub enum Topic {
    OrderFulfilled,
    PriceThreshold {
        amount_asset: Asset,
        price_asset: Asset,
        price_threshold: Price,
    },
}

impl Eq for Topic {} // Ignore the fact that Topic can contain `f64`

impl Hash for Topic { // Has to impl Hash manually because of `f64` inside
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Topic::OrderFulfilled => {
                state.write_u8(1);
            }
            Topic::PriceThreshold {
                amount_asset,
                price_asset,
                price_threshold,
            } => {
                state.write_u8(2);
                amount_asset.hash(state);
                price_asset.hash(state);
                price_threshold.to_bits().hash(state);
            }
        }
    }
}
