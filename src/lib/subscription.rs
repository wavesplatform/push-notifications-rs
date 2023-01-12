use std::collections::HashSet;

use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use reqwest::Url;

use crate::{
    error::Error,
    model::{Address, AsBase58String, Asset},
    schema::{subscribers, subscriptions, topics_price_threshold},
    stream::{Event, Price, PriceRange},
};

use crate::scoped_futures::ScopedFutureExt;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum TopicError {
    #[error("Unknown scheme, only 'push' is allowed")]
    UnknownScheme,

    #[error("Topic parse error: {0}")]
    ParseError(String),

    #[error("Unknown topic kind, only 'orders' and 'price_threshold' are allowed")]
    UnknownTopicKind(String),

    #[error("Invalid/missing amount asset")]
    InvalidAmountAsset,

    #[error("Invalid/missing price asset")]
    InvalidPriceAsset,

    #[error("Invalid/missing threshold value")]
    InvalidThreshold,
}

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

#[derive(Debug, PartialEq)]
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
    pub fn from_url_string(topic_url_raw: &str) -> Result<(Self, SubscriptionMode), TopicError> {
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

        let topic_url = {
            let topic =
                Url::parse(topic_url_raw).map_err(|e| TopicError::ParseError(e.to_string()))?;
            let topic_scheme = topic.scheme();
            if topic_scheme != "push" {
                return Err(TopicError::UnknownScheme);
            }
            topic
        };

        let topic_kind = {
            let raw_topic_kind = topic_url
                .domain()
                .ok_or(TopicError::UnknownTopicKind(String::new()))?;

            TopicKind::parse(raw_topic_kind)
                .map_err(|e| TopicError::UnknownTopicKind(e.to_string()))?
        };

        let subscription_mode = match topic_url.query_pairs().find(|(k, _)| k == "oneshot") {
            Some(_) => SubscriptionMode::Once,
            None => SubscriptionMode::Repeat,
        };

        let topic = match topic_kind {
            TopicKind::Orders => {
                Topic::OrderFulfilled {
                    //todo: refactor
                    amount_asset: Asset::Waves,
                    price_asset: Asset::Waves,
                }
            }
            TopicKind::PriceThreshold => {
                let threshold_info = topic_url
                    .path_segments()
                    .expect("relative url")
                    .take(3)
                    .collect::<Vec<&str>>();

                let amount_asset = threshold_info
                    .get(0)
                    .ok_or_else(|| TopicError::InvalidAmountAsset)
                    .and_then(|a| Asset::from_id(a).map_err(|_| TopicError::InvalidAmountAsset))?;

                let price_asset = threshold_info
                    .get(1)
                    .ok_or_else(|| TopicError::InvalidPriceAsset)
                    .and_then(|a| Asset::from_id(a).map_err(|_| TopicError::InvalidPriceAsset))?;

                let price_threshold = threshold_info
                    .get(2)
                    .ok_or_else(|| TopicError::InvalidThreshold)
                    .and_then(|v| v.parse().map_err(|_| TopicError::InvalidThreshold))?;

                Topic::PriceThreshold {
                    amount_asset,
                    price_asset,
                    price_threshold,
                }
            }
        };

        Ok((topic, subscription_mode))
    }

    pub fn as_url_string(&self, mode: SubscriptionMode) -> String {
        match self {
            Topic::OrderFulfilled { .. } => "push://orders".to_string(),
            Topic::PriceThreshold {
                amount_asset,
                price_asset,
                price_threshold,
            } => {
                let mut url = format!(
                    "push://price_threshold/{amount_asset}/{price_asset}/{price_threshold}"
                );
                if let SubscriptionMode::Once = mode {
                    url += "?oneshot";
                }
                url
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
        conn.transaction(move |conn| {
            async move {
                let existing_topics: HashSet<String> = HashSet::from_iter(
                    subscriptions::table
                        .select(subscriptions::topic)
                        .get_results::<String>(conn)
                        .await?,
                );

                let filtered_subscriptions = subscriptions
                    .iter()
                    .filter(|subscr| !existing_topics.contains(&subscr.topic_url));

                let rows = filtered_subscriptions
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
                    .values(rows)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_topics() {
        let topic_urls_and_parsed_ok = [
            (
                "push://orders",
                (
                    Topic::OrderFulfilled {
                        amount_asset: Asset::Waves,
                        price_asset: Asset::Waves,
                    },
                    SubscriptionMode::Repeat,
                ),
            ),
            (
                "push://orders?oneshot",
                (
                    Topic::OrderFulfilled {
                        amount_asset: Asset::Waves,
                        price_asset: Asset::Waves,
                    },
                    SubscriptionMode::Once,
                ),
            ),
            (
                "push://price_threshold/8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc/WAVES/500.0",
                (
                    Topic::PriceThreshold {
                        amount_asset: Asset::from_id(
                            "8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc",
                        )
                        .unwrap(),
                        price_asset: Asset::Waves,
                        price_threshold: 500.0,
                    },
                    SubscriptionMode::Repeat,
                ),
            ),
            (
                "push://price_threshold/WAVES/8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc/500.0?oneshot",
                (
                    Topic::PriceThreshold {
                        amount_asset: Asset::Waves,
                        price_asset: Asset::from_id(
                            "8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc",
                        )
                        .unwrap(),
                        price_threshold: 500.0,
                    },
                    SubscriptionMode::Once,
                ),
            ),
            (
                "push://price_threshold/WAVES/WAVES/-10.5?LKJH=nhwqg734xn&qwe=zxc#asdqwlvkj",
                (
                    Topic::PriceThreshold {
                        amount_asset: Asset::Waves,
                        price_asset: Asset::Waves,
                        price_threshold: -10.5,
                    },
                    SubscriptionMode::Repeat,
                ),
            ),
        ];

        for (url, expected_result) in topic_urls_and_parsed_ok {
            let actual_result = Topic::from_url_string(url).unwrap();
            assert_eq!(actual_result, expected_result);
        }

        let topic_urls_and_parsed_err = [
            (
                "push://pop",
                TopicError::UnknownTopicKind("pop".to_string()),
            ),
            ("shush://orders", TopicError::UnknownScheme),
            (
                "push://price_threshold/WAVES/WAVES",
                TopicError::InvalidThreshold,
            ),
            // TODO: current library Asset implementation accepts invalid asset addresses, so this test doesn't fail but should,
            // uncomment after fixing it
            // (
            //     "push://price_threshold/1234567qwe/WAVES/-10.5",
            //     "1234567qwe".to_string(),
            // ),
        ];

        for (url, expected_error) in topic_urls_and_parsed_err {
            let actual_error = Topic::from_url_string(url).unwrap_err();
            assert_eq!(actual_error, expected_error);
        }
    }

    #[test]
    fn topics_as_urls() {
        let topics_sub_modes_urls = [
            (
                Topic::PriceThreshold {
                    amount_asset: Asset::Waves,
                    price_asset: Asset::from_id("8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc")
                        .unwrap(),
                    price_threshold: 1.7,
                },
                SubscriptionMode::Repeat,
                "push://price_threshold/WAVES/8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc/1.7",
            ),
            (
                Topic::PriceThreshold {
                    amount_asset: Asset::from_id("8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc")
                        .unwrap(),
                    price_asset: Asset::Waves,
                    price_threshold: 2.,
                },
                SubscriptionMode::Once,
                "push://price_threshold/8cwrggsqQREpCLkPwZcD2xMwChi1MLaP7rofenGZ5Xuc/WAVES/2?oneshot",
            ),
            (
                Topic::OrderFulfilled {
                    amount_asset: Asset::Waves,
                    price_asset: Asset::Waves
                },
                SubscriptionMode::Once,
                "push://orders"
            )
        ];

        for (topic, sub_mode, expected_url) in topics_sub_modes_urls {
            assert_eq!(topic.as_url_string(sub_mode), expected_url);
        }
    }
}
