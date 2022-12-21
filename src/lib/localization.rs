use std::collections::HashMap;

use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
    model::Lang,
    stream::OrderExecution,
};
use wavesexchange_apis::HttpClient;

mod lokalise_keys {
    pub const ORDER_FILLED_TITLE: &str = "orderFilledTitle";
    pub const ORDER_FILLED_MSG: &str = "orderFilledMessage";
    pub const ORDER_PART_FILLED_MSG: &str = "orderPartFilledMessage";
    pub const PRICE_ALERT_TITLE: &str = "priceAlertTitle";
    pub const PRICE_ALERT_MSG: &str = "priceAlertMessage";
}

struct RemoteGateway {
    lokalise_client: HttpClient<()>,
}

impl RemoteGateway {
    pub fn new(lokalise_client: HttpClient<()>) -> Self {
        RemoteGateway { lokalise_client }
    }

    pub async fn keys_for_project(&self, project_id: &str) -> Result<dto::KeysResponse, Error> {
        self.lokalise_client
            .create_req_handler::<dto::KeysResponse>(
                self.lokalise_client
                    .http_get(format!("projects/{project_id}/keys?include_translations=1",)),
                "lokalise::get",
            )
            .execute()
            .await
            .map_err(Error::from)
    }
}

mod dto {
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    pub struct KeysResponse {
        pub project_id: String,
        pub keys: Vec<Key>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Key {
        pub key_id: i64,
        pub created_at: String,
        pub created_at_timestamp: i64,
        pub key_name: PlatformStrings,
        pub filenames: PlatformStrings,
        pub description: String,
        pub platforms: Vec<String>,
        pub tags: Vec<String>,
        pub translations: Option<Vec<Translation>>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct PlatformStrings {
        pub ios: String,
        pub android: String,
        pub web: String,
        pub other: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Translation {
        pub translation_id: i64,
        pub key_id: i64,
        pub language_iso: String,
        pub translation: String,
        pub modified_by: i64,
        pub modified_by_email: String,
        pub modified_at: String,
        pub modified_at_timestamp: i64,
        pub is_reviewed: bool,
        pub is_unverified: bool,
        pub reviewed_by: i64,
        pub task_id: Option<i64>,
    }
}

type Key = String;
type Value = String;
type TranslationMap = HashMap<Key, HashMap<Lang, Value>>;

pub struct Repo {
    translations: TranslationMap,
}

pub struct LokaliseConfig {
    pub token: String,
    pub project_id: String,
}

impl Repo {
    pub async fn new(config: LokaliseConfig) -> Result<Self, Error> {
        let auth_header = HashMap::from([("X-Api-Token".to_string(), config.token)]);

        let lokalise_client = HttpClient::<()>::builder()
            .with_base_url("https://api.lokalise.co/api2")
            .with_reqwest_builder(|rb| rb.default_headers((&auth_header).try_into().unwrap()))
            .build();

        let remote_gateway = RemoteGateway::new(lokalise_client);
        let keys = remote_gateway.keys_for_project(&config.project_id).await?;
        let mut translations: TranslationMap = HashMap::new();

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

        log::trace!("Lokalise translations: {:?}", translations);

        Ok(Self { translations })
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
