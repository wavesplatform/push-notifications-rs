use std::fmt::Display;

use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use futures::FutureExt;
use reqwest::Url;

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
    Once = 0,
    Repeat = 1,
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
        match mode {
            0 => Self::Once,
            1 => Self::Repeat,
            _ => panic!("unknown subscription mode {mode}"),
        }
    }
}

impl Topic {
    fn from_url_string(topic_url_raw: &str) -> Result<(Self, SubscriptionMode), Error> {
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

    fn as_url_string(&self) -> String {
        match self {
            Topic::OrderFulfilled { .. } => "push://orders".to_string(),
            Topic::PriceThreshold {
                amount_asset,
                price_asset,
                price_threshold,
            } => format!("push://price_threshold/{amount_asset}/{price_asset}/{price_threshold}",),
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

        rows.into_iter()
            .map(|(address, created_at, topic_type, topic)| {
                Ok(Subscription {
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
        topic_urls: Vec<String>,
        conn: &mut AsyncPgConnection,
    ) -> Result<(), Error> {
        let topics = topic_urls
            .into_iter()
            .map(|t| Topic::from_url_string(&t))
            .collect::<Result<Vec<_>, _>>()?;

        let rows = topics
            .into_iter()
            .map(|(topic, subscr_mode)| {
                (
                    subscriptions::subscriber_address.eq(address.as_base58_string()),
                    subscriptions::topic.eq(topic.as_url_string()),
                    subscriptions::topic_type.eq(subscr_mode as i32),
                )
            })
            .collect::<Vec<_>>();

        conn.transaction(move |conn| {
            async move {
                let topic_uids_urls: Vec<(i32, String)> = diesel::insert_into(subscriptions::table)
                    .values(rows)
                    .on_conflict_do_nothing()
                    .returning((subscriptions::uid, subscriptions::topic))
                    .get_results(conn)
                    .await?;

                let new_price_threshold_topics = topic_uids_urls
                    .into_iter()
                    .filter_map(|(uid, topic_url)| {
                        let (topic, _) =
                            Topic::from_url_string(&topic_url).expect("topic url corrupted");

                        if let Topic::PriceThreshold {
                            amount_asset,
                            price_asset,
                            price_threshold,
                        } = topic
                        {
                            let row = (
                                topics_price_threshold::subscription_uid.eq(uid),
                                topics_price_threshold::amount_asset_id.eq(amount_asset.id()),
                                topics_price_threshold::price_asset_id.eq(price_asset.id()),
                                topics_price_threshold::price_threshold.eq(price_threshold),
                            );
                            Some(row)
                        } else {
                            None
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
            .boxed()
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
            .boxed()
        })
        .await
    }
}
