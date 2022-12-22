use diesel::ExpressionMethods;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::Serialize;

use crate::{
    device::Device,
    error::Error,
    schema::messages,
    stream::{OrderExecution, OrderSide, OrderType, Price},
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
        threshold: Price, // decimals already applied
    },
}

pub struct LocalizedMessage {
    pub notification_title: String,
    pub notification_body: String,
}

pub struct PreparedMessage {
    pub device: Device, // device_uid and address
    pub message: LocalizedMessage,
    pub data: Option<MessageData>, // JSON-serializable data
    pub collapse_key: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct MessageData {
    #[serde(rename = "t")]
    pub event_type: u8,
    #[serde(rename = "a")]
    pub amount_asset_id: String,
    #[serde(rename = "p")]
    pub price_asset_id: String,
}

impl MessageData {
    pub const TYPE_ORDER_PART: u8 = 0;
    pub const TYPE_ORDER_FULL: u8 = 1;
    pub const TYPE_PRICE: u8 = 2;
}

#[test]
fn test_message_data_serialize() {
    use serde_json::{json, to_value};
    let data = MessageData {
        event_type: 42,
        amount_asset_id: "asset1".to_string(),
        price_asset_id: "asset2".to_string(),
    };
    let expected_json = json! (
        {
            "t": 42,
            "a": "asset1",
            "p": "asset2",
        }
    );
    let value = to_value(data).expect("serialize");
    assert_eq!(value, expected_json);
}

pub struct Queue {}

impl Queue {
    pub async fn enqueue(
        &self,
        message: PreparedMessage,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let values = (
            messages::device_uid.eq(message.device.device_uid),
            messages::notification_title.eq(message.message.notification_title),
            messages::notification_body.eq(message.message.notification_body),
            messages::data.eq(serde_json::to_value(message.data)?),
            messages::collapse_key.eq(message.collapse_key),
        );
        let num_rows = diesel::insert_into(messages::table)
            .values(values)
            .execute(conn)
            .await?;
        debug_assert_eq!(num_rows, 1); //TODO return error?

        Ok(())
    }
}
