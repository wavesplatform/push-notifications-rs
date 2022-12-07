use diesel::{ExpressionMethods,};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    device::Device,
    error::Error,
    schema::messages,
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
    pub async fn enqueue(
        &self,
        message: WithTimestamp<PreparedMessage>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let values = (
            messages::device_uid.eq(message.value.device.device_uid),
            messages::notification_title.eq(message.value.message.notification_title),
            messages::notification_body.eq(message.value.message.notification_body),
            //messages::data.eq(message.value.data), //TODO optional data
            messages::collapse_key.eq(message.value.collapse_key),
        );
        let num_rows = diesel::insert_into(messages::table)
            .values(values)
            .execute(conn)
            .await?;
        debug_assert_eq!(num_rows, 1); //TODO return error?

        Ok(())
    }
}
