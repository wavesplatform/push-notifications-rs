use crate::{
    device::Device,
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

pub struct PreparedMessage {
    pub device: Device, // device_uid and address
    pub message: LocalizedMessage,
    pub data: Option<()>, //TODO specify correct type instead of `()`
    pub collapse_key: Option<String>,
}

pub struct Queue {}

impl Queue {
    pub async fn enqueue(&self, message: WithTimestamp<PreparedMessage>) -> Result<(), Error> {
        todo!("message enqueue impl")
    }
}
