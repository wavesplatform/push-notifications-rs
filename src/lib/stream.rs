use itertools::Itertools;

use crate::model::{Amount, AssetId};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Copy, Clone, PartialEq)]
pub enum OrderExecution {
    Full,
    Partial { percentage: f64 },
}

pub struct PriceOHLC {
    open: f64,
    close: f64,
    low: f64,
    high: f64,
}

pub enum Event {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        amount_asset_id: AssetId,
        price_asset_id: AssetId,
        execution: OrderExecution,
    },
    PriceChanged {
        amount_asset_id: AssetId,
        price_asset_id: AssetId,
        current_price: PriceOHLC,
        previous_price: PriceOHLC,
    },
}

impl PriceOHLC {
    pub fn has_crossed_threshold(&self, other: &Self, threshold: f64) -> bool {
        let mut prices = [
            self.open,
            self.close,
            self.low,
            self.high,
            other.open,
            other.close,
            other.low,
            other.high,
        ];
        prices.sort_unstable_by(f64::total_cmp);
        prices
            .into_iter()
            .tuple_windows()
            .any(|(a, b)| f64::signum(threshold - a) != f64::signum(threshold - b))
    }
}
