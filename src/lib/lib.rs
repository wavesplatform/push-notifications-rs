#![allow(dead_code, unused_variables)]

mod device;

use chrono::{DateTime, Utc};
use waves_rust::model::{Address, Amount, AssetId};

pub struct Subscription {
    subscriber: Address,
    created_at: DateTime<Utc>,
    mode: SubscriptionMode,
    topic: Topic,
}

pub enum SubscriptionMode {
    Once,
    Repeat,
}

pub enum Topic {
    OrderFulfilled {
        amount_asset: Option<AssetId>,
        price_asset: Option<AssetId>,
    },
    PriceThreshold {
        amount_asset: Option<AssetId>,
        price_threshold: Amount,
    },
}

pub struct Event {
    timestamp: DateTime<Utc>,
    payload: EventPayload,
}

pub enum EventPayload {
    OrderFulfilled {/* TODO */},
    PriceChanged {
        amount_asset_id: AssetId,
        low: Amount,
        high: Amount,
    },
}
