use crate::error::Error;
use wavesexchange_apis::HttpClient;

pub struct Repo {
    lokalise_client: HttpClient<()>,
    project_id: String,
}

impl Repo {
    pub fn new(lokalise_client: HttpClient<()>, project_id: impl Into<String>) -> Self {
        Repo {
            lokalise_client,
            project_id: project_id.into(),
        }
    }

    pub async fn get(&self, key: impl AsRef<str>, lang: impl AsRef<str>) -> Result<String, Error> {
        let searched_key = key.as_ref();
        let searched_lang = lang.as_ref();
        let keys = self
            .lokalise_client
            .create_req_handler::<dto::KeysResponse>(
                self.lokalise_client
                    .http_get(format!("projects/{}/keys", self.project_id)),
                "lokalise::get",
            )
            .execute()
            .await?;

        keys.keys
            .into_iter()
            .find(|k| k.key_name.web == searched_key)
            .map(|k| {
                let err = Error::TranslationError(format!(
                    "Translation by key '{searched_key}' and lang '{searched_lang}' not found"
                ));
                match k.translations {
                    Some(t) => t
                        .into_iter()
                        .find(|tr| tr.language_iso == searched_lang)
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
        pub modified_by_emali: String,
        pub modified_at: String,
        pub modified_at_timestamp: i64,
        pub is_reviewed: bool,
        pub is_unverified: bool,
        pub reviewed_by: i64,
        pub task_id: i64,
    }
}
