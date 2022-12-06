use chrono::{DateTime, Utc};
use diesel_async::AsyncPgConnection;

use crate::model::{Address, Amount, AssetId};
use crate::stream::Event;

pub struct Subscription {
    pub subscriber: Address,
    pub created_at: DateTime<Utc>,
    pub mode: SubscriptionMode,
    pub topic: Topic,
}

#[derive(Copy, Clone, PartialEq, Eq)]
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
    pub async fn matching(&self, event: &Event, conn: &mut AsyncPgConnection) -> Vec<Subscription> {
        todo!("subscriptions repo impl")
    }

    pub async fn cancel(&self, subscription: Subscription, conn: &mut AsyncPgConnection) {
        todo!("cancel oneshot subscription impl")
    }
}
