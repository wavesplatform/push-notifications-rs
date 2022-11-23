use waves_rust::model::{Amount, AssetId};

pub enum OrderType {
    Limit,
    Market,
}

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
