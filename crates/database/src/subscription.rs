use std::collections::HashSet;

use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, Queryable};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use model::{
    asset::{Asset, AssetPair},
    event::Event,
    price::PriceRange,
    topic::{SubscriptionMode, Topic},
    waves::{Address, AsBase58String},
};

use crate::{
    error::Error,
    schema::{subscribers, subscriptions, topics_order_execution, topics_price_threshold},
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

#[derive(Clone, Debug)]
pub struct SubscribeConfig {
    pub max_subscriptions_per_address_per_pair: u32,
    pub max_subscriptions_per_address_total: u32,
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
                self.matching_price_subscriptions(asset_pair, price_range, conn)
                    .await
            }
        }
    }

    async fn matching_order_subscriptions(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<Subscription>, Error> {
        let rows = topics_order_execution::table
            .inner_join(
                subscriptions::table
                    .on(topics_order_execution::subscription_uid.eq(subscriptions::uid)),
            )
            .select((
                subscriptions::uid,
                subscriptions::created_at,
                subscriptions::topic_type,
            ))
            .filter(subscriptions::subscriber_address.eq(address.as_base58_string()))
            .order(subscriptions::uid)
            .load::<(i32, DateTime<Utc>, i32)>(conn)
            .await?;

        rows.into_iter()
            .map(|(uid, created_at, topic_type)| {
                Ok(Subscription {
                    uid,
                    subscriber: address.to_owned(),
                    created_at,
                    mode: SubscriptionMode::from_int(topic_type as u8),
                    topic: Topic::OrderFulfilled,
                })
            })
            .collect()
    }

    async fn matching_price_subscriptions(
        &self,
        asset_pair: &AssetPair,
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
                topics_price_threshold::price_threshold,
            ))
            .filter(topics_price_threshold::amount_asset_id.eq(asset_pair.amount_asset.id()))
            .filter(topics_price_threshold::price_asset_id.eq(asset_pair.price_asset.id()))
            .filter(topics_price_threshold::price_threshold.between(price_low, price_high))
            .order(subscriptions::uid)
            .load::<(i32, String, DateTime<Utc>, i32, f64)>(conn)
            .await?;

        rows.into_iter()
            .filter(|&(_, _, _, _, threshold)| {
                // Since we've used simple BETWEEN filter in SQL query,
                // there can be extra rows that we need to filter properly.
                price_range.contains(threshold)
            })
            .map(|(uid, address, created_at, topic_type, price_threshold)| {
                let address =
                    Address::from_string(&address).map_err(|_| Error::BadAddress(address))?;
                Ok(Subscription {
                    uid,
                    subscriber: address,
                    created_at,
                    mode: SubscriptionMode::from_int(topic_type as u8),
                    topic: Topic::PriceThreshold {
                        amount_asset: asset_pair.amount_asset.clone(),
                        price_asset: asset_pair.price_asset.clone(),
                        price_threshold,
                    },
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
        config: &SubscribeConfig,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let existing_topics: HashSet<String> = HashSet::from_iter(
            subscriptions::table
                .select(subscriptions::topic)
                .filter(subscriptions::subscriber_address.eq(address.as_base58_string()))
                .get_results::<String>(conn)
                .await?,
        );

        let new_subs_count = existing_topics.len() + subscriptions.len();
        let max_subs_count = config.max_subscriptions_per_address_total as usize;
        if new_subs_count > max_subs_count {
            return Err(Error::LimitExceeded(
                address.to_owned(),
                config.max_subscriptions_per_address_total,
            ));
        }
        //TODO implement per-pair check

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

    pub async fn subscriptions_by_address(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<(Topic, SubscriptionMode)>, Error> {
        let address = address.as_base58_string();

        let query = subscriptions::table
            .left_outer_join(
                topics_order_execution::table
                    .on(topics_order_execution::subscription_uid.eq(subscriptions::uid)),
            )
            .left_outer_join(
                topics_price_threshold::table
                    .on(topics_price_threshold::subscription_uid.eq(subscriptions::uid)),
            )
            .select((
                subscriptions::topic_type,
                topics_order_execution::subscription_uid.nullable(),
                topics_price_threshold::subscription_uid.nullable(),
                topics_price_threshold::amount_asset_id.nullable(),
                topics_price_threshold::price_asset_id.nullable(),
                topics_price_threshold::price_threshold.nullable(),
            ))
            .filter(subscriptions::subscriber_address.eq(address))
            .order(subscriptions::uid);

        #[derive(Queryable)]
        struct Subscription {
            topic_type: i32,
            order_subscription_uid: Option<i32>,
            price_subscription_uid: Option<i32>,
            amount_asset_id: Option<String>,
            price_asset_id: Option<String>,
            price_threshold: Option<f64>,
        }

        let rows = query.load::<Subscription>(conn).await?;

        let res = rows
            .into_iter()
            .map(|row| {
                let topic = {
                    if row.order_subscription_uid.is_some() {
                        Topic::OrderFulfilled
                    } else if row.price_subscription_uid.is_some() {
                        let parse_asset =
                            |id: String| Asset::from_id(&id).map_err(|()| Error::BadAsset(id));
                        // Unwraps below are safe because of the check `price_subscription_uid.is_some()`
                        // If it fails - database JOIN query is broken
                        Topic::PriceThreshold {
                            amount_asset: parse_asset(row.amount_asset_id.unwrap())?,
                            price_asset: parse_asset(row.price_asset_id.unwrap())?,
                            price_threshold: row.price_threshold.unwrap(),
                        }
                    } else {
                        unreachable!("broken JOIN");
                    }
                };

                let mode = SubscriptionMode::from_int(row.topic_type as u8);

                Ok((topic, mode))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(res)
    }
}
