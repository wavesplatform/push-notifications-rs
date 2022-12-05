use std::{collections::HashMap, fmt::Display};

use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
    stream::OrderExecution,
};
use wavesexchange_apis::HttpClient;

const PROJECT_ID: &str = "8193754062287cbb2742a0.58698490";

struct RemoteGateway {
    lokalise_client: HttpClient<()>,
}

impl RemoteGateway {
    pub fn new(lokalise_client: HttpClient<()>) -> Self {
        RemoteGateway { lokalise_client }
    }

    pub async fn localize(&self) -> Result<dto::KeysResponse, Error> {
        self.lokalise_client
            .create_req_handler::<dto::KeysResponse>(
                self.lokalise_client
                    .http_get(format!("projects/{PROJECT_ID}/keys?include_translations=1",)),
                "lokalise::get",
            )
            .execute()
            .await
            .map_err(Error::from)
    }
}

pub mod dto {
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Lang {
    Ru,
    En,
    Es,
}

impl TryFrom<&str> for Lang {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(match s {
            "ru" => Lang::Ru,
            "en" => Lang::En,
            "es" => Lang::Es,
            lang => return Err(Error::Generic(format!("unknown language {lang}"))),
        })
    }
}

// {key: {lang: value}}
pub type TranslationMap = HashMap<String, HashMap<Lang, String>>;

pub struct Repo {
    translations: TranslationMap,
}

impl Repo {
    pub async fn new(lokalise_client: HttpClient<()>) -> Result<Self, Error> {
        let remote_gateway = RemoteGateway { lokalise_client };
        let keys = remote_gateway.localize().await?;
        let mut translations: TranslationMap = HashMap::new();

        for key in keys.keys {
            let key_name = key.key_name.web;

            if let Some(t) = key.translations {
                for tr in t {
                    let lang = Lang::try_from(tr.language_iso.as_str())?;
                    translations
                        .entry(key_name.clone())
                        .or_default()
                        .insert(lang, tr.translation);
                }
            }
        }

        Ok(Self { translations })
    }

    pub fn localize(&self, message: &Message, lang: Lang) -> LocalizedMessage {
        let translate = |key| self.translations[key][&lang].clone();

        let (title, body) = match message {
            Message::OrderExecuted { execution, .. } => {
                let message = match execution {
                    OrderExecution::Full => translate("orderFilledMessage"),
                    OrderExecution::Partial { .. } => translate("orderPartFilledMessage"),
                };
                (translate("orderFilledTitle"), message)
            }
            Message::PriceThresholdReached { .. } => {
                (translate("priceAlertTitle"), translate("priceAlertMessage"))
            }
        };

        LocalizedMessage {
            notification_title: title,
            notification_body: body,
        }
    }
}
