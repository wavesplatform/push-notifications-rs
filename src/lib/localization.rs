use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
    model::Lang,
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

    pub async fn localize(&self, message: &Message, lang: &str) -> Result<String, Error> {
        let searched_key = match message { todo!() };
        let keys = self
            .lokalise_client
            .create_req_handler::<dto::KeysResponse>(
                self.lokalise_client
                    .http_get(format!("projects/{PROJECT_ID}/keys?include_translations=1",)),
                "lokalise::get",
            )
            .execute()
            .await?;

        keys.keys
            .into_iter()
            .find(|k| k.key_name.web == searched_key)
            .map(|k| {
                let err = Error::TranslationError(format!(
                    "Translation by key '{searched_key}' and lang '{lang}' not found"
                ));
                match k.translations {
                    Some(t) => t
                        .into_iter()
                        .find(|tr| tr.language_iso == lang)
                        .map(|tr| tr.translation)
                        .ok_or_else(|| err),
                    None => Err(err),
                }
            })
            .ok_or_else(|| {
                Error::TranslationError(format!("Translation by key '{searched_key}' not found"))
            })?
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

pub struct Repo {}

impl Repo {
    //TODO This is the factory method that can construct Repo during app startup.
    // Does all the network calls and caches everything (using the lower-level gateway above).
    // Can fail (this will lead to app crash on startup).
    pub fn new() -> Result<Self, Error> {
        todo!("localizer repo factory method impl")
    }

    //TODO This is the higher-level localization that is totally infallible and never does any network calls
    pub fn localize(&self, message: &Message, lang: &Lang) -> LocalizedMessage {
        //TODO parse input message, decide which localization keys and arguments we need
        match message {
            Message::OrderExecuted { .. } => { /* TODO */ }
            Message::PriceThresholdReached { .. } => { /* TODO */ }
        }
        todo!("localize message impl")
    }
}
