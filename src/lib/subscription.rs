use std::fmt::Display;

use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use reqwest::Url;

use crate::{
    error::Error,
    model::{Address, AsBase58String, Asset},
    schema::{subscriptions, topics_price_threshold},
    stream::{Event, Price, PriceRange},
};

use crate::scoped_futures::ScopedFutureExt;

#[derive(Debug)]
pub struct Subscription {
    pub uid: i32,
    pub subscriber: Address,
    pub created_at: DateTime<Utc>,
    pub mode: SubscriptionMode,
    pub topic: Topic,
}

pub struct SubscriptionRequest {
    pub topic_url: String,
    pub topic: Topic,
    pub mode: SubscriptionMode,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SubscriptionMode {
    Once,
    Repeat,
}

#[derive(Debug)]
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
        match mode {
            0 => Self::Once,
            1 => Self::Repeat,
            _ => panic!("unknown subscription mode {mode}"),
        }
    }

    fn to_int(&self) -> u8 {
        match self {
            SubscriptionMode::Once => 0,
            SubscriptionMode::Repeat => 1,
        }
    }
}

impl Topic {
    pub fn from_url_string(topic_url_raw: &str) -> Result<(Self, SubscriptionMode), Error> {
        enum TopicKind {
            Orders,
            PriceThreshold,
        }

        impl TopicKind {
            fn parse(s: &str) -> Result<Self, &str> {
                match s {
                    "orders" => Ok(TopicKind::Orders),
                    "price_threshold" => Ok(TopicKind::PriceThreshold),
                    _ => Err(s),
                }
            }
        }

        fn topic_err<S: Display>(msg: S) -> Error {
            Error::BadTopic(msg.to_string())
        }

        let topic_url = {
            let topic = Url::parse(topic_url_raw).map_err(topic_err)?;
            let topic_scheme = topic.scheme();
            if topic_scheme != "push" {
                return Err(topic_err(format!(
                    "topic scheme != 'push', but '{topic_scheme}'"
                )));
            }
            topic
        };

        let topic_kind = {
            let raw_topic_kind = topic_url.domain().ok_or(topic_err("unknown topic kind"))?;

            TopicKind::parse(raw_topic_kind)
                .map_err(|e| topic_err(format!("unknown topic kind: {e}")))?
        };

        match topic_kind {
            TopicKind::Orders => {
                let topic = Topic::OrderFulfilled {
                    //todo: refactor
                    amount_asset: Asset::Waves,
                    price_asset: Asset::Waves,
                };
                Ok((topic, SubscriptionMode::Once))
            }
            TopicKind::PriceThreshold => {
                let threshold_info = topic_url
                    .path_segments()
                    .expect("relative url")
                    .take(3)
                    .collect::<Vec<&str>>();

                let amount_asset = threshold_info
                    .get(0)
                    .ok_or_else(|| topic_err("can't find amount asset"))
                    .and_then(|a| {
                        Asset::from_id(a).map_err(|_| Error::AssetParseError(a.to_string()))
                    })?;

                let price_asset = threshold_info
                    .get(1)
                    .ok_or_else(|| topic_err("can't find price asset"))
                    .and_then(|a| {
                        Asset::from_id(a).map_err(|_| Error::AssetParseError(a.to_string()))
                    })?;

                let price_threshold = threshold_info
                    .get(2)
                    .ok_or_else(|| topic_err("can't find threshold value"))
                    .and_then(|v| {
                        v.parse()
                            .map_err(|_| topic_err(format!("invalid threshold_value '{v}'")))
                    })?;

                let subscription_mode = match topic_url.query_pairs().find(|(k, _)| k == "oneshot")
                {
                    Some(_) => SubscriptionMode::Once,
                    None => SubscriptionMode::Repeat,
                };

                let topic = Topic::PriceThreshold {
                    amount_asset,
                    price_asset,
                    price_threshold,
                };

                Ok((topic, subscription_mode))
            }
        }
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

    //TODO async fn matching_order_subscriptions(...) -> Result<Vec<Subscription>, Error> { ... }

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
        let rows = subscriptions
            .iter()
            .map(|subscr| {
                (
                    subscriptions::subscriber_address.eq(address.as_base58_string()),
                    subscriptions::topic.eq(subscr.topic_url.clone()),
                    subscriptions::topic_type.eq(subscr.mode.to_int() as i32),
                )
            })
            .collect::<Vec<_>>();

        conn.transaction(move |conn| {
            async move {
                let uids = diesel::insert_into(subscriptions::table)
                    .values(rows)
                    .on_conflict_do_nothing()
                    .returning(subscriptions::uid)
                    .get_results::<i32>(conn)
                    .await?;

                let subscr_with_uids = subscriptions.iter().zip(uids.into_iter());

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
            .scope_boxed()
        })
        .await
    }

    pub async fn unsubscribe(
        &self,
        address: &Address,
        topics: Option<Vec<String>>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        conn.transaction(move |conn| {
            let address = address.as_base58_string();
            async move {
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

                diesel::delete(
                    subscriptions::table.filter(subscriptions::uid.eq_any(uids_to_remove)),
                )
                .execute(conn)
                .await?;

                Ok(())
            }
            .scope_boxed()
        })
        .await
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
