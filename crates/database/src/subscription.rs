use std::collections::HashSet;

use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, TextExpressionMethods};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use model::{
    event::Event,
    price::PriceRange,
    topic::{SubscriptionMode, Topic},
    waves::{Address, AsBase58String},
};

use crate::{
    error::Error,
    schema::{subscribers, subscriptions, topics_price_threshold},
};

#[derive(Debug)]
pub struct Subscription {
    pub uid: i32,
    pub subscriber: Address,
    pub created_at: DateTime<Utc>,
    pub mode: SubscriptionMode,
    pub topic: Topic,
}

#[derive(Debug)]
pub struct SubscriptionRequest {
    pub topic_url: String,
    pub topic: Topic,
    pub mode: SubscriptionMode,
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
            Event::OrderExecuted { address, .. } => {
                self.matching_order_subscriptions(address, conn).await
            }
            Event::PriceChanged {
                asset_pair,
                price_range,
                ..
            } => {
                self.matching_price_subscriptions(
                    asset_pair.amount_asset.id(),
                    asset_pair.price_asset.id(),
                    price_range,
                    conn,
                )
                .await
            }
        }
    }

    async fn matching_order_subscriptions(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<Subscription>, Error> {
        let rows = subscriptions::table
            .select((
                subscriptions::uid,
                subscriptions::subscriber_address,
                subscriptions::created_at,
                subscriptions::topic_type,
                subscriptions::topic,
            ))
            .filter(subscriptions::subscriber_address.eq(address.as_base58_string()))
            .filter(subscriptions::topic.like("push://orders%"))
            .order(subscriptions::uid)
            .load::<(i32, String, DateTime<Utc>, i32, String)>(conn)
            .await?;

        rows.into_iter()
            .map(|(uid, address, created_at, topic_type, topic)| {
                Ok(Subscription {
                    uid,
                    subscriber: Address::from_string(&address).expect("address in db"),
                    created_at,
                    mode: SubscriptionMode::from_int(topic_type as u8),
                    topic: Topic::from_url_string(&topic)?.0,
                })
            })
            .collect()
    }

    async fn matching_price_subscriptions(
        &self,
        amount_asset_id: String,
        price_asset_id: String,
        price_range: &PriceRange,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<Subscription>, Error> {
        let (price_low, price_high) = price_range.low_high();
        let rows = topics_price_threshold::table
            .inner_join(
                subscriptions::table
                    .on(topics_price_threshold::subscription_uid.eq(subscriptions::uid)),
            )
            .select((
                subscriptions::uid,
                subscriptions::subscriber_address,
                subscriptions::created_at,
                subscriptions::topic_type,
                subscriptions::topic,
                topics_price_threshold::price_threshold,
            ))
            .filter(topics_price_threshold::amount_asset_id.eq(amount_asset_id))
            .filter(topics_price_threshold::price_asset_id.eq(price_asset_id))
            .filter(topics_price_threshold::price_threshold.between(price_low, price_high))
            .order(subscriptions::uid)
            .load::<(i32, String, DateTime<Utc>, i32, String, f64)>(conn)
            .await?;

        rows.into_iter()
            .filter(|&(_, _, _, _, _, threshold)| {
                // Since we've used simple BETWEEN filter in SQL query,
                // there can be extra rows that we need to filter properly.
                price_range.contains(threshold)
            })
            .map(|(uid, address, created_at, topic_type, topic, _)| {
                Ok(Subscription {
                    uid,
                    subscriber: Address::from_string(&address).expect("address in db"),
                    created_at,
                    mode: SubscriptionMode::from_int(topic_type as u8),
                    topic: Topic::from_url_string(&topic)?.0,
                })
            })
            .collect()
    }

    pub async fn complete_oneshot(
        &self,
        subscription: Subscription,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        debug_assert_eq!(subscription.mode, SubscriptionMode::Once);
        let num_rows =
            diesel::delete(subscriptions::table.filter(subscriptions::uid.eq(subscription.uid)))
                .execute(conn)
                .await?;
        debug_assert_eq!(num_rows, 1); //TODO return warning?
        Ok(())
    }

    pub async fn subscribe(
        &self,
        address: &Address,
        subscriptions: Vec<SubscriptionRequest>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let existing_topics: HashSet<String> = HashSet::from_iter(
            subscriptions::table
                .select(subscriptions::topic)
                .filter(subscriptions::subscriber_address.eq(address.as_base58_string()))
                .get_results::<String>(conn)
                .await?,
        );

        let filtered_subscriptions = subscriptions
            .iter()
            .filter(|subscr| !existing_topics.contains(&subscr.topic_url));

        //TODO The following fragment definitely contains a logical error and must be rewritten:
        // It attempts to zip HashMap (with undefined order of elements) with other collection,
        // which will result in mixed up pairs of topic url & subscription.

        // if db contains "topic_name" topic and the request has "topic_name?oneshot",
        // remove old topic from subscriptions and topics_price_threshold tables and insert the new one
        let subscriptions_to_update_sub_mode = existing_topics
            .iter()
            .map(|topic_url| {
                let (topic, sub_mode) =
                    Topic::from_url_string(&topic_url).expect("broken topic url"); //TODO don't unwrap, convert and propagate this error
                (topic_url, topic, sub_mode)
            })
            .zip(filtered_subscriptions.clone())
            .filter(|((_, topic, sub_mode), sub_req)| {
                *topic == sub_req.topic && *sub_mode != sub_req.mode
            })
            .map(|((topic_url, ..), _)| topic_url.as_ref())
            .collect::<Vec<&str>>();

        if !subscriptions_to_update_sub_mode.is_empty() {
            diesel::delete(
                subscriptions::table
                    .filter(subscriptions::topic.eq_any(subscriptions_to_update_sub_mode)),
            )
            .execute(conn)
            .await?;
        }

        let subscriptions_rows = filtered_subscriptions
            .clone()
            .map(|subscr| {
                (
                    subscriptions::subscriber_address.eq(address.as_base58_string()),
                    subscriptions::topic.eq(subscr.topic_url.clone()),
                    subscriptions::topic_type.eq(subscr.mode.to_int() as i32),
                )
            })
            .collect::<Vec<_>>();

        diesel::insert_into(subscribers::table)
            .values(subscribers::address.eq(address.as_base58_string()))
            .on_conflict_do_nothing()
            .execute(conn)
            .await?;

        let uids = diesel::insert_into(subscriptions::table)
            .values(subscriptions_rows)
            .returning(subscriptions::uid)
            .get_results::<i32>(conn)
            .await?;

        let subscr_with_uids = filtered_subscriptions.zip(uids.into_iter());

        let new_price_threshold_topics = subscr_with_uids
            .filter(|&(subscr, _uid)| matches!(subscr.topic, Topic::PriceThreshold { .. }))
            .map(|(subscr, uid)| {
                if let Topic::PriceThreshold {
                    amount_asset,
                    price_asset,
                    price_threshold,
                } = &subscr.topic
                {
                    (
                        topics_price_threshold::subscription_uid.eq(uid),
                        topics_price_threshold::amount_asset_id.eq(amount_asset.id()),
                        topics_price_threshold::price_asset_id.eq(price_asset.id()),
                        topics_price_threshold::price_threshold.eq(price_threshold),
                    )
                } else {
                    unreachable!("broken filter by topic type")
                }
            })
            .collect::<Vec<_>>();

        if new_price_threshold_topics.len() > 0 {
            diesel::insert_into(topics_price_threshold::table)
                .values(new_price_threshold_topics)
                .execute(conn)
                .await?;
        }
        Ok(())
    }

    pub async fn unsubscribe(
        &self,
        address: &Address,
        topics: Option<Vec<String>>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let address = address.as_base58_string();

        let uids_to_remove: Vec<i32> = match topics {
            Some(topics) => {
                subscriptions::table
                    .select(subscriptions::uid)
                    .filter(subscriptions::subscriber_address.eq(address))
                    .filter(subscriptions::topic.eq_any(topics))
                    .get_results(conn)
                    .await?
            }
            None => {
                subscriptions::table
                    .select(subscriptions::uid)
                    .filter(subscriptions::subscriber_address.eq(address))
                    .get_results(conn)
                    .await?
            }
        };

        diesel::delete(
            topics_price_threshold::table
                .filter(topics_price_threshold::subscription_uid.eq_any(&uids_to_remove)),
        )
        .execute(conn)
        .await?;

        diesel::delete(subscriptions::table.filter(subscriptions::uid.eq_any(uids_to_remove)))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn get_topics_by_address(
        &self,
        addr: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<String>, Error> {
        subscriptions::table
            .select(subscriptions::topic)
            .filter(subscriptions::subscriber_address.eq(addr.as_base58_string()))
            .get_results(conn)
            .await
            .map_err(Error::from)
    }
}
