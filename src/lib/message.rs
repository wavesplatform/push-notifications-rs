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

#[derive(Debug)]
pub struct LocalizedMessage {
    pub notification_title: String,
    pub notification_body: String,
}

#[derive(Debug)]
pub struct PreparedMessage {
    pub device: Device, // device_uid and address
    pub message: LocalizedMessage,
    pub data: Option<MessageData>, // JSON-serializable data
    pub collapse_key: Option<String>,
}

#[derive(Clone, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageData {
    OrderPartiallyExecuted {
        amount_asset_id: String,
        price_asset_id: String,
    },
    OrderExecuted {
        amount_asset_id: String,
        price_asset_id: String,
    },
    PriceThresholdReached {
        amount_asset_id: String,
        price_asset_id: String,
    },
}

#[cfg(test)]
mod message_data_serialize_tests {
    use super::MessageData;
    use serde_json::{json, to_value};

    #[test]
    fn test_order_part() {
        let data = MessageData::OrderPartiallyExecuted {
            amount_asset_id: "asset1".to_string(),
            price_asset_id: "asset2".to_string(),
        };
        let expected_json = json! (
            {
                "type": "order_partially_executed",
                "amount_asset_id": "asset1",
                "price_asset_id": "asset2",
            }
        );
        let value = to_value(data).expect("serialize");
        assert_eq!(value, expected_json);
    }

    #[test]
    fn test_order_full() {
        let data = MessageData::OrderExecuted {
            amount_asset_id: "asset1".to_string(),
            price_asset_id: "asset2".to_string(),
        };
        let expected_json = json! (
            {
                "type": "order_executed",
                "amount_asset_id": "asset1",
                "price_asset_id": "asset2",
            }
        );
        let value = to_value(data).expect("serialize");
        assert_eq!(value, expected_json);
    }

    #[test]
    fn test_price() {
        let data = MessageData::PriceThresholdReached {
            amount_asset_id: "asset1".to_string(),
            price_asset_id: "asset2".to_string(),
        };
        let expected_json = json! (
            {
                "type": "price_threshold_reached",
                "amount_asset_id": "asset1",
                "price_asset_id": "asset2",
            }
        );
        let value = to_value(data).expect("serialize");
        assert_eq!(value, expected_json);
    }
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
