use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    error::Error,
    model::{Address, AsBase58String, Asset},
    schema::{subscriptions, topics_price_threshold},
    stream::{Event, Price},
};

pub struct Subscription {
    pub subscriber: Address,
    pub created_at: DateTime<Utc>,
    pub mode: SubscriptionMode,
    pub topic: Topic,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SubscriptionMode {
    Once,
    Repeat,
}

pub enum Topic {
    OrderFulfilled {
        amount_asset: Asset,
        price_asset: Asset,
    },
    PriceThreshold {
        amount_asset: Asset,
        price_asset: Asset,
        price_threshold: Price,
    },
}

impl SubscriptionMode {
    fn from_int(mode: u8) -> Self {
        todo!("impl SubscriptionMode from topic_type conversion")
    }
}

impl Topic {
    fn from_url_string(topic_url: &str) -> Self {
        todo!("impl Topic from url string conversion")
    }

    fn as_url_string(&self) -> String {
        todo!("impl Topic to url string conversion")
    }
}

#[derive(Clone)]
pub struct Repo {}

impl Repo {
    pub async fn matching(
        &self,
        event: &Event,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<Subscription>, Error> {
        match event {
            Event::OrderExecuted { .. } => {
                //TODO matching_order_subscriptions(...).await
                todo!("impl find matching subscriptions for OrderExecuted event")
            }
            Event::PriceChanged {
                asset_pair,
                price_range,
            } => {
                let (price_low, price_high) = price_range.low_high();
                self.matching_price_subscriptions(
                    asset_pair.amount_asset.id(),
                    asset_pair.price_asset.id(),
                    price_low,
                    price_high,
                    conn,
                )
                .await
            }
        }
    }

    //TODO async fn matching_order_subscriptions(...) -> Result<Vec<Subscription>, Error> { ... }

    async fn matching_price_subscriptions(
        &self,
        amount_asset_id: String,
        price_asset_id: String,
        price_low: Price,
        price_high: Price,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<Subscription>, Error> {
        let rows = topics_price_threshold::table
            .inner_join(
                subscriptions::table
                    .on(topics_price_threshold::subscription_uid.eq(subscriptions::uid)),
            )
            .select((
                subscriptions::subscriber_address,
                subscriptions::created_at,
                subscriptions::topic_type,
                subscriptions::topic,
            ))
            .filter(topics_price_threshold::amount_asset_id.eq(amount_asset_id))
            .filter(topics_price_threshold::price_asset_id.eq(price_asset_id))
            .filter(topics_price_threshold::price_threshold.between(price_low, price_high))
            .order(subscriptions::uid)
            .load::<(String, DateTime<Utc>, i32, String)>(conn)
            .await?;

        let subscriptions = rows
            .into_iter()
            .map(|(address, created_at, topic_type, topic)| Subscription {
                subscriber: Address::from_string(&address).expect("address in db"),
                created_at,
                mode: SubscriptionMode::from_int(topic_type as u8),
                topic: Topic::from_url_string(&topic),
            })
            .collect();

        Ok(subscriptions)
    }

    pub async fn complete_oneshot(
        &self,
        subscription: Subscription,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        debug_assert_eq!(subscription.mode, SubscriptionMode::Once);
        let address = subscription.subscriber.as_base58_string();
        let topic = subscription.topic.as_url_string();
        let num_rows = diesel::delete(
            subscriptions::table
                //.filter(subscriptions::uid.eq(subscription.uid)) //TODO delete by uid or by primary key?
                .filter(subscriptions::subscriber_address.eq(address))
                .filter(subscriptions::topic.eq(topic))
                .filter(subscriptions::topic_type.eq(SubscriptionMode::Once as i32)),
        )
        .execute(conn)
        .await?;
        debug_assert_eq!(num_rows, 1); //TODO return warning?
        Ok(())
    }

    pub async fn subscribe(
        &self,
        address: &Address,
        topics: Vec<String>,
        topic_type: SubscriptionMode,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let values = topics
            .into_iter()
            .map(|topic| {
                (
                    subscriptions::subscriber_address.eq(address.as_base58_string()),
                    subscriptions::topic.eq(topic),
                    subscriptions::topic_type.eq(topic_type as i32),
                )
            })
            .collect::<Vec<_>>();

        diesel::insert_into(subscriptions::table)
            .values(values)
            .on_conflict_do_nothing()
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn unsubscribe(
        &self,
        address: &Address,
        topics: Option<Vec<String>>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        if let Some(topics) = topics {
            diesel::delete(
                subscriptions::table
                    .filter(subscriptions::subscriber_address.eq(address.as_base58_string()))
                    .filter(subscriptions::topic.eq_any(topics)),
            )
            .execute(conn)
            .await?;
        } else {
            diesel::delete(
                subscriptions::table
                    .filter(subscriptions::subscriber_address.eq(address.as_base58_string())),
            )
            .execute(conn)
            .await?;
        }

        Ok(())
    }
}
