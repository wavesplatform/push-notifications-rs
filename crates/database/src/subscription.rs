use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use diesel::{
    dsl::sql_query, ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, Queryable,
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use itertools::{Either, Itertools};

use model::{
    asset::{Asset, AssetPair},
    event::Event,
    price::PriceRange,
    topic::{PriceThreshold, SubscriptionMode, Topic},
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
    /// This field (and the corresponding database column) is not used anymore and can be safely deleted
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
                    mode: topic_type_from_int(topic_type)?,
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
                    mode: topic_type_from_int(topic_type)?,
                    topic: Topic::PriceThreshold(PriceThreshold {
                        amount_asset: asset_pair.amount_asset.clone(),
                        price_asset: asset_pair.price_asset.clone(),
                        price_threshold,
                    }),
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
        let existing_subscriptions = self.subscriptions(address, conn).await?;

        // Check limits
        {
            // Check total limit on subscriptions
            let new_subs_count = existing_subscriptions.len() + subscriptions.len();
            let max_subs_count = config.max_subscriptions_per_address_total as usize;
            if new_subs_count > max_subs_count {
                return Err(Error::LimitExceeded(
                    address.to_owned(),
                    config.max_subscriptions_per_address_total,
                ));
            }

            // Check per-pair limit on price subscriptions
            let subscriptions_per_pair = {
                // Map of: asset pair -> set of prices
                let mut price_subs =
                    HashMap::<AssetPair, HashSet<_>>::with_capacity(new_subs_count);

                // Topics of existing subscriptions
                let existing_topics = existing_subscriptions
                    .iter()
                    .map(|&(_, ref topic, _)| topic);

                // Topics of new subscriptions
                let new_topics = subscriptions.iter().map(|sub| &sub.topic);

                // Topics of all subscriptions, old and new
                let topics = existing_topics.chain(new_topics);

                // Group by asset pair, collect all unique price thresholds
                for topic in topics {
                    if let Topic::PriceThreshold(t) = topic {
                        let pair = AssetPair {
                            amount_asset: t.amount_asset.clone(),
                            price_asset: t.price_asset.clone(),
                        };
                        let prices = price_subs.entry(pair).or_default();
                        prices.insert(t.price_threshold.to_bits());
                    }
                }

                price_subs
            };

            let limit = config.max_subscriptions_per_address_per_pair as usize;

            if subscriptions_per_pair
                .values()
                .any(|prices| prices.len() > limit)
            {
                return Err(Error::LimitExceeded(
                    address.to_owned(),
                    config.max_subscriptions_per_address_per_pair,
                ));
            }
        }

        // Convert existing subscriptions to a map keyed by topic
        let existing = HashMap::<Topic, (SubscriptionMode, i32)>::from_iter(
            existing_subscriptions
                .into_iter()
                .map(|(uid, topic, mode)| (topic, (mode, uid))),
        );

        // We need to split the subscriptions into two categories:
        //  1. Those that exists in database but with different subscription mode (need to update them).
        //  2. Not existing in the database (need to add them).
        // Those existing in database with the same subscription mode can be safely ignored.
        let (to_update, to_add) = subscriptions
            .into_iter()
            .filter(|sub| existing.get(&sub.topic).copied().map(|(mode, _)| mode) != Some(sub.mode))
            .partition_map::<Vec<_>, Vec<_>, _, _, _>(|sub| match existing.get(&sub.topic) {
                Some(&(_, uid)) => Either::Left((uid, sub)),
                None => Either::Right(sub),
            });

        for (uid, sub) in to_update {
            log::debug!("Updating for {:?}: {:?}", address, sub);
            let count = diesel::update(subscriptions::table.filter(subscriptions::uid.eq(uid)))
                .set(subscriptions::topic_type.eq(topic_type_to_int(sub.mode)))
                .execute(conn)
                .await?;
            log::debug!("Updated {} subscriptions for {:?}", count, address);
        }

        if !to_add.is_empty() {
            log::debug!("Creating subs for {:?}: {:?}", address, to_add);

            let address = address.as_base58_string();

            // Create subscriber (if missing)
            diesel::insert_into(subscribers::table)
                .values(subscribers::address.eq(&address))
                .on_conflict_do_nothing()
                .execute(conn)
                .await?;

            // Subscriptions - common data
            let insert_rows = to_add
                .iter()
                .map(|sub| {
                    (
                        subscriptions::subscriber_address.eq(&address),
                        subscriptions::topic.eq(&sub.topic_url),
                        subscriptions::topic_type.eq(topic_type_to_int(sub.mode)),
                    )
                })
                .collect::<Vec<_>>();
            let uids = diesel::insert_into(subscriptions::table)
                .values(insert_rows)
                .returning(subscriptions::uid)
                .get_results::<i32>(conn)
                .await?;
            assert_eq!(uids.len(), to_add.len());

            // Subscriptions - topic data
            let subs = to_add.into_iter().map(|sub| sub.topic).zip(uids);
            let (orders, prices) =
                subs.partition_map::<Vec<_>, Vec<_>, _, _, _>(|(topic, uid)| match topic {
                    Topic::OrderFulfilled => Either::Left((uid,)),
                    Topic::PriceThreshold(t) => Either::Right((uid, t)),
                });
            if !orders.is_empty() {
                let insert_rows = orders
                    .into_iter()
                    .map(|(uid,)| (topics_order_execution::subscription_uid.eq(uid),))
                    .collect::<Vec<_>>();

                diesel::insert_into(topics_order_execution::table)
                    .values(insert_rows)
                    .execute(conn)
                    .await?;
            }
            if !prices.is_empty() {
                let insert_rows = prices
                    .into_iter()
                    .map(|(uid, topic)| {
                        (
                            topics_price_threshold::subscription_uid.eq(uid),
                            topics_price_threshold::amount_asset_id.eq(topic.amount_asset.id()),
                            topics_price_threshold::price_asset_id.eq(topic.price_asset.id()),
                            topics_price_threshold::price_threshold.eq(topic.price_threshold),
                        )
                    })
                    .collect::<Vec<_>>();

                diesel::insert_into(topics_price_threshold::table)
                    .values(insert_rows)
                    .execute(conn)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn unsubscribe(
        &self,
        address: &Address,
        topics: Vec<Topic>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let address = address.as_base58_string();

        // Safe from sql injections, because asset IDs are safe
        let conditions = topics
            .iter()
            .map(|t| match t {
                Topic::OrderFulfilled => format!("(o.subscription_uid IS NOT NULL)"),
                Topic::PriceThreshold(t) => {
                    format!(
                        "(p.amount_asset_id = '{}' AND p.price_asset_id = '{}' AND p.price_threshold = {})",
                        t.amount_asset.id(),
                        t.price_asset.id(),
                        t.price_threshold,
                    )
                }
            })
            .join(" OR ");

        // Safe from sql injections, because addresses are safe
        let query = format!(
            r#"
                DELETE FROM subscriptions WHERE uid IN (
                    SELECT s.uid
                    FROM subscriptions s
                         LEFT OUTER JOIN topics_price_threshold p ON (p.subscription_uid = s.uid)
                         LEFT OUTER JOIN topics_order_execution o ON (o.subscription_uid = s.uid)
                    WHERE (s.subscriber_address = '{}') AND ({})
                )
            "#,
            address, conditions
        );

        let query = sql_query(query);

        let count = query.execute(conn).await?;

        log::debug!(
            "Deleted {} of {} requested subscriptions for {}",
            count,
            topics.len(),
            address
        );

        Ok(())
    }

    pub async fn unsubscribe_all(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let address = address.as_base58_string();

        let count = diesel::delete(
            subscriptions::table.filter(subscriptions::subscriber_address.eq(&address)),
        )
        .execute(conn)
        .await?;

        log::debug!("Deleted {} subscriptions for {}", count, address);

        Ok(())
    }

    pub async fn subscriptions_by_address(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<(Topic, SubscriptionMode)>, Error> {
        let res = self
            .subscriptions(address, conn)
            .await?
            .into_iter()
            .map(|(_, topic, mode)| (topic, mode))
            .collect();
        Ok(res)
    }

    async fn subscriptions(
        &self,
        address: &Address,
        conn: &mut AsyncPgConnection,
    ) -> Result<Vec<(i32, Topic, SubscriptionMode)>, Error> {
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
                subscriptions::uid,
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
            uid: i32,
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
                let uid = row.uid;

                let topic = {
                    if row.order_subscription_uid.is_some() {
                        Topic::OrderFulfilled
                    } else if row.price_subscription_uid.is_some() {
                        let parse_asset =
                            |id: String| Asset::from_id(&id).map_err(|()| Error::BadAsset(id));
                        // Unwraps below are safe because of the check `price_subscription_uid.is_some()`
                        // If it fails - database JOIN query is broken
                        Topic::PriceThreshold(PriceThreshold {
                            amount_asset: parse_asset(row.amount_asset_id.unwrap())?,
                            price_asset: parse_asset(row.price_asset_id.unwrap())?,
                            price_threshold: row.price_threshold.unwrap(),
                        })
                    } else {
                        log::warn!("Bad subscription {} (unknown type) - ignored", row.uid);
                        return Ok(None);
                    }
                };

                let mode = topic_type_from_int(row.topic_type)?;

                Ok(Some((uid, topic, mode)))
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(res)
    }
}

fn topic_type_from_int(mode: i32) -> Result<SubscriptionMode, Error> {
    match mode {
        0 => Ok(SubscriptionMode::Once),
        1 => Ok(SubscriptionMode::Repeat),
        _ => Err(Error::BadTopicType(mode)),
    }
}

pub fn topic_type_to_int(mode: SubscriptionMode) -> i32 {
    match mode {
        SubscriptionMode::Once => 0,
        SubscriptionMode::Repeat => 1,
    }
}

#[test]
fn test_topic_type_to_from_int() {
    let check = |m: i32| {
        let mode = topic_type_from_int(m).expect("bad mode");
        let out = topic_type_to_int(mode);
        assert_eq!(m, out, "failed for mode {:?}", mode);
    };
    check(0);
    check(1);
}
