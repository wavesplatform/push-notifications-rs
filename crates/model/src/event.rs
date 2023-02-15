use crate::{
    asset::AssetPair,
    order::{OrderExecution, OrderSide, OrderType},
    price::PriceRange,
    time::Timestamp,
    waves::Address,
};

#[derive(Debug)]
pub enum Event {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        asset_pair: AssetPair,
        execution: OrderExecution,
        address: Address,
        timestamp: Timestamp,
    },
    PriceChanged {
        asset_pair: AssetPair,
        price_range: PriceRange,
        timestamp: Timestamp,
    },
}
