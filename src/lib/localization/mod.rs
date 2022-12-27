use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
    model::Lang,
    stream::OrderExecution,
};
use std::collections::HashMap;

use self::localise_gateway::{dto::KeysResponse, RemoteGateway, LOCALISE_API_URL};

pub use self::localise_gateway::LokaliseConfig;

mod localise_gateway;

mod lokalise_keys {
    pub const ORDER_FILLED_TITLE: &str = "orderFilledTitle";
    pub const ORDER_FILLED_MSG: &str = "orderFilledMessage";
    pub const ORDER_PART_FILLED_MSG: &str = "orderPartFilledMessage";
    pub const PRICE_ALERT_TITLE: &str = "priceAlertTitle";
    pub const PRICE_ALERT_MSG: &str = "priceAlertMessage";
}

type Key = String;
type Value = String;
type ValuesMap = HashMap<Lang, Value>;
type TranslationMap = HashMap<Key, ValuesMap>;

pub struct Repo {
    translations: TranslationMap,
}

impl Repo {
    pub async fn new(config: LokaliseConfig) -> Result<Self, Error> {
        let remote_gateway = RemoteGateway::new(LOCALISE_API_URL, &config.token);
        let keys = remote_gateway.keys_for_project(&config.project_id).await?;
        let translations = Self::keys_to_translations(keys);
        log::trace!("Lokalise translations: {:?}", translations);
        Ok(Self { translations })
    }

    fn keys_to_translations(keys: KeysResponse) -> TranslationMap {
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
        translations
    }

    pub fn localize(&self, message: &Message, lang: &Lang) -> Option<LocalizedMessage> {
        let translate = |key| self.translations[key].get(lang).cloned();

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
