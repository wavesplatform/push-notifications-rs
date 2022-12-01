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

pub enum Event {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        amount_asset_id: AssetId,
        price_asset_id: AssetId,
        execution_percentage: f64,
    },
    PriceChanged {
        amount_asset_id: AssetId,
        low: Amount,
        high: Amount,
    },
}
