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

pub struct LocalizedMessage {
    pub notification_title: String,
    pub notification_body: String,
}

//TODO Separate MessageData struct with the remaining fields: data, collapse_key etc.

pub struct Queue {}

impl Queue {
    pub async fn enqueue(&self, message: WithTimestamp<LocalizedMessage>) -> Result<(), Error> {
        todo!("message enqueue impl")
    }
}
