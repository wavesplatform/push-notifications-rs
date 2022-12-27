use std::collections::HashMap;

use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
    model::Lang,
    stream::{OrderExecution, OrderSide},
};

use self::{
    localise_gateway::{RemoteGateway, LOCALISE_API_URL},
    translations::TranslationMap,
};

pub use self::localise_gateway::LokaliseConfig;

mod localise_gateway;

mod lokalise_keys {
    pub const ORDER_FILLED_TITLE: &str = "orderFilledTitle";
    pub const ORDER_FILLED_MSG: &str = "orderFilledMessage";
    pub const ORDER_PART_FILLED_MSG: &str = "orderPartFilledMessage";
    pub const PRICE_ALERT_TITLE: &str = "priceAlertTitle";
    pub const PRICE_ALERT_MSG: &str = "priceAlertMessage";
    pub const BUY: &str = "buy";
    pub const SELL: &str = "sell";
}

mod translations {
    use super::localise_gateway::dto::KeysResponse;
    use crate::model::Lang;
    use std::{
        collections::{BTreeSet, HashMap},
        fmt,
    };

    pub(super) type Key = String;
    pub(super) type Value = String;
    pub(super) type ValuesMap = HashMap<Lang, Value>;
    pub(super) struct TranslationMap(HashMap<Key, ValuesMap>);

    impl TranslationMap {
        pub(super) fn build(keys: KeysResponse) -> Self {
            let mut translations = HashMap::<Key, ValuesMap>::new();
            for key in keys.keys {
                let key_name = key.key_name.web;

                if let Some(t) = key.translations {
                    for tr in t {
                        translations
                            .entry(key_name.clone())
                            .or_default()
                            .insert(tr.language_iso, tr.translation);
                    }
                }
            }
            TranslationMap(translations)
        }

        pub(super) fn is_complete(&self) -> bool {
            let TranslationMap(translations) = self;
            let keys = self.keys();
            let langs = self.langs();
            for lang in &langs {
                for key in &keys {
                    let has_value = translations
                        .get(key)
                        .map(|values| values.get(lang))
                        .flatten()
                        .is_some();
                    if !has_value {
                        return false;
                    }
                }
            }
            true
        }

        fn keys(&self) -> BTreeSet<Key> {
            let TranslationMap(translations) = self;
            translations.keys().map(String::to_owned).collect()
        }

        fn langs(&self) -> BTreeSet<Lang> {
            let TranslationMap(translations) = self;
            translations
                .values()
                .map(|values| values.keys())
                .flatten()
                .map(String::to_owned)
                .collect()
        }

        pub(super) fn translate(&self, key: &str, lang: &str) -> Option<&Value> {
            let TranslationMap(translations) = self;
            translations[key].get(lang)
        }
    }

    impl fmt::Debug for TranslationMap {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "Languages: {:?}, Keys: {:?}, Translations: {:?}",
                self.langs(),
                self.keys(),
                self.0,
            )
        }
    }
}

pub struct Repo {
    translations: TranslationMap,
}

impl Repo {
    pub async fn new(config: LokaliseConfig) -> Result<Self, Error> {
        let remote_gateway = RemoteGateway::new(LOCALISE_API_URL, &config.token);
        let keys = remote_gateway.keys_for_project(&config.project_id).await?;
        let translations = TranslationMap::build(keys);
        if translations.is_complete() {
            log::trace!("Lokalise translations: {:?}", translations);
        } else {
            log::warn!("Incomplete lokalise translations: {:?}", translations);
        }
        Ok(Self { translations })
    }

    pub fn localize(&self, message: &Message, lang: &Lang) -> Option<LocalizedMessage> {
        let translate = |key| self.translations.translate(key, lang);

        let title_key = match message {
            Message::OrderExecuted { .. } => lokalise_keys::ORDER_FILLED_TITLE,
            Message::PriceThresholdReached { .. } => lokalise_keys::PRICE_ALERT_TITLE,
        };

        let body_key = match message {
            Message::OrderExecuted { execution, .. } => match execution {
                OrderExecution::Full => lokalise_keys::ORDER_FILLED_MSG,
                OrderExecution::Partial { .. } => lokalise_keys::ORDER_PART_FILLED_MSG,
            },
            Message::PriceThresholdReached { .. } => lokalise_keys::PRICE_ALERT_MSG,
        };

        let side_key = match message {
            Message::OrderExecuted { side, .. } => Some(match side {
                OrderSide::Buy => lokalise_keys::BUY,
                OrderSide::Sell => lokalise_keys::SELL,
            }),
            Message::PriceThresholdReached { .. } => None,
        };

        let side = match side_key {
            Some(key) => translate(key)?,
            None => "",
        };

        let (amount_token, price_token) = match message {
            Message::OrderExecuted {
                amount_asset_ticker,
                price_asset_ticker,
                ..
            }
            | Message::PriceThresholdReached {
                amount_asset_ticker,
                price_asset_ticker,
                ..
            } => (amount_asset_ticker, price_asset_ticker),
        };

        let pair = format!("{} / {}", amount_token, price_token);

        let value = match message {
            Message::OrderExecuted { .. } => "".to_string(),
            Message::PriceThresholdReached { threshold, .. } => format!("{}", threshold),
        };

        let date = "?"; //TODO Need date field in the message
        let time = "?"; //TODO Need time field in the message

        let title = translate(title_key)?;
        let body = translate(body_key)?;

        let subst = HashMap::from([
            ("", ""),
            ("amountToken", amount_token),
            ("priceToken", price_token),
            ("pair", &pair),
            ("side", side),
            ("value", &value),
            ("date", date),
            ("time", time),
        ]);

        Some(LocalizedMessage {
            notification_title: template::interpolate(title, &subst),
            notification_body: template::interpolate(body, &subst),
        })
    }
}

mod template {
    use regex::{Captures, Regex};
    use std::borrow::Cow;
    use std::collections::HashMap;

    pub(super) fn interpolate(s: &str, subst: &HashMap<&str, &str>) -> String {
        let re = Regex::new(r#"\[%s:([a-zA-z]+)]"#).expect("regex");
        re.replace_all(s, |caps: &Captures| {
            let key = caps.get(1).expect("regex capture").as_str();
            subst
                .get(key)
                .map(|s| Cow::Borrowed(*s))
                .unwrap_or_else(|| Cow::Owned(format!("<{}>", key)))
        })
        .to_string()
    }

    #[test]
    fn test_interpolate() {
        let subst = HashMap::from([("foo", "bar"), ("fee", "baz")]);
        assert_eq!(&interpolate("", &subst), "");
        assert_eq!(&interpolate("[%s:foo]", &subst), "bar");
        assert_eq!(&interpolate("[%s:foo] bar", &subst), "bar bar");
        assert_eq!(&interpolate("[%s:foo] [%s:fee]", &subst), "bar baz");
        assert_eq!(&interpolate("[%s:foo] [%s:foo]", &subst), "bar bar");
        assert_eq!(
            &interpolate("[%s:foo] [%s:fee] [%s:foo]", &subst),
            "bar baz bar"
        );
        assert_eq!(&interpolate("[%s:unknown]", &subst), "<unknown>");
    }
}
