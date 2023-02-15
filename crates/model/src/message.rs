use serde::Serialize;

use crate::{
    device::Device,
    order::{OrderExecution, OrderSide, OrderType},
    price::Price,
    time::Timestamp,
};

pub enum Message {
    OrderExecuted {
        order_type: OrderType,
        side: OrderSide,
        amount_asset_ticker: String,
        price_asset_ticker: String,
        execution: OrderExecution,
        timestamp: Timestamp,
    },
    PriceThresholdReached {
        amount_asset_ticker: String,
        price_asset_ticker: String,
        threshold: Price, // decimals already applied
        timestamp: Timestamp,
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
        address: String,
    },
    OrderExecuted {
        amount_asset_id: String,
        price_asset_id: String,
        address: String,
    },
    PriceThresholdReached {
        amount_asset_id: String,
        price_asset_id: String,
        address: String,
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
            address: "1234567890".to_string(),
        };
        let expected_json = json! (
            {
                "type": "order_partially_executed",
                "amount_asset_id": "asset1",
                "price_asset_id": "asset2",
                "address": "1234567890",
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
            address: "1234567890".to_string(),
        };
        let expected_json = json! (
            {
                "type": "order_executed",
                "amount_asset_id": "asset1",
                "price_asset_id": "asset2",
                "address": "1234567890",
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
            address: "1234567890".to_string(),
        };
        let expected_json = json! (
            {
                "type": "price_threshold_reached",
                "amount_asset_id": "asset1",
                "price_asset_id": "asset2",
                "address": "1234567890",
            }
        );
        let value = to_value(data).expect("serialize");
        assert_eq!(value, expected_json);
    }
}
