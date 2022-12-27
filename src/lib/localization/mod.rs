use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
    model::Lang,
    stream::OrderExecution,
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

        pub(super) fn translate(&self, key: &str, lang: &str) -> Option<Value> {
            let TranslationMap(translations) = self;
            translations[key].get(lang).cloned()
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

        let title = match message {
            Message::OrderExecuted { .. } => lokalise_keys::ORDER_FILLED_TITLE,
            Message::PriceThresholdReached { .. } => lokalise_keys::PRICE_ALERT_TITLE,
        };

        let body = match message {
            Message::OrderExecuted { execution, .. } => match execution {
                OrderExecution::Full => lokalise_keys::ORDER_FILLED_MSG,
                OrderExecution::Partial { .. } => lokalise_keys::ORDER_PART_FILLED_MSG,
            },
            Message::PriceThresholdReached { .. } => lokalise_keys::PRICE_ALERT_MSG,
        };

        Some(LocalizedMessage {
            notification_title: translate(title)?,
            notification_body: translate(body)?,
        })
    }
}
