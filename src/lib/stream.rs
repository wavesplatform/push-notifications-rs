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
    open: u64,
    close: u64,
    low: u64,
    high: u64,
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
    pub fn has_crossed_threshold(&self, other: &Self, threshold: u64) -> bool {
        let threshold = threshold as i64;
        let mut prices = [
            self.open as i64,
            self.close as i64,
            self.low as i64,
            self.high as i64,
            other.open as i64,
            other.close as i64,
            other.low as i64,
            other.high as i64,
        ];
        prices.sort_unstable_by(i64::cmp);
        prices
            .into_iter()
            .tuple_windows()
            .any(|(a, b)| i64::signum(threshold - a) != i64::signum(threshold - b))
    }
}
