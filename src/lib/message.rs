use crate::{
    error::Error,
    stream::{OrderExecution, OrderSide, OrderType},
    timestamp::WithTimestamp,
};

pub enum Message {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        amount_asset_ticker: String,
        price_asset_ticker: String,
        execution: OrderExecution,
    },
    PriceThresholdReached {
        amount_asset_ticker: String,
        price_asset_ticker: String,
        threshold: f64, // decimals already applied
    },
}

pub struct Queue {}

impl Queue {
    pub async fn enqueue(&self, message: WithTimestamp<Message>) -> Result<(), Error> {
        todo!("message enqueue impl")
    }
}
