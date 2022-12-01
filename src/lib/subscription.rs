use chrono::{DateTime, Utc};
use waves_rust::model::{Address, Amount, AssetId};

use crate::stream::Event;

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

pub struct Repo {}

impl Repo {
    // probably will have different interface
    pub async fn matching(event: &Event) -> Vec<Subscription> {
        todo!("impl")
    }
}
