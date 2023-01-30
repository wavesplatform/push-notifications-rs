use reqwest::Url;

use crate::{asset::Asset, price::Price};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SubscriptionMode {
    Once,
    Repeat,
}

#[derive(Debug, PartialEq)]
pub enum Topic {
    OrderFulfilled,
    PriceThreshold {
        amount_asset: Asset,
        price_asset: Asset,
        price_threshold: Price,
    },
}

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

impl SubscriptionMode {
    pub fn from_int(mode: u8) -> Self {
        match mode {
            0 => Self::Once,
            1 => Self::Repeat,
            _ => panic!("unknown subscription mode {mode}"),
        }
    }

    pub fn to_int(&self) -> u8 {
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
            TopicKind::Orders => Topic::OrderFulfilled,
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
        let topic = match self {
            Topic::OrderFulfilled { .. } => "push://orders".to_string(),
            Topic::PriceThreshold {
                amount_asset,
                price_asset,
                price_threshold,
            } => {
                format!("push://price_threshold/{amount_asset}/{price_asset}/{price_threshold}")
            }
        };

        let subscription_mode = if let SubscriptionMode::Once = mode {
            "?oneshot"
        } else {
            ""
        };

        format!("{topic}{subscription_mode}")
    }
}

#[cfg(test)]
mod tests {
    use super::{SubscriptionMode, Topic, TopicError};
    use crate::asset::Asset;

    #[test]
    fn parse_topics() {
        let topic_urls_and_parsed_ok = [
            (
                "push://orders",
                (
                    Topic::OrderFulfilled,
                    SubscriptionMode::Repeat,
                ),
            ),
            (
                "push://orders?oneshot",
                (
                    Topic::OrderFulfilled,
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
                Topic::OrderFulfilled,
                SubscriptionMode::Once,
                "push://orders?oneshot"
            ),
            (
                Topic::OrderFulfilled,
                SubscriptionMode::Repeat,
                "push://orders"
            )
        ];

        for (topic, sub_mode, expected_url) in topics_sub_modes_urls {
            assert_eq!(topic.as_url_string(sub_mode), expected_url);
        }
    }
}
