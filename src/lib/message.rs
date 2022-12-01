use crate::{
    error::Error,
    stream::{OrderSide, OrderType},
    WithTimestamp,
};

pub enum Message {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        amount_asset_ticker: String,
        price_asset_ticker: String,
    },
    OrderPartiallyExecuted {
        order_type: OrderType,
        side: OrderSide,
        amount_asset_ticker: String,
        price_asset_ticker: String,
        execution_percentage: f64,
    },
    PriceThresholdReached {
        amount_asset_ticker: String,
        price_asset_ticker: String,
        threshold: f64, // decimals already applied
    },
}

pub struct Queue {}

impl Queue {
    pub async fn enqueue(message: WithTimestamp<Message>) -> Result<(), Error> {
        todo!("impl")
    }
}
