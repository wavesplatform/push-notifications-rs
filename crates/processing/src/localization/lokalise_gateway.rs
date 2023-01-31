use wavesexchange_apis::HttpClient;

//TODO Move to config
pub(super) const LOCALISE_API_URL: &str = "https://api.lokalise.com/api2";

pub struct LokaliseConfig {
    pub token: String,
    pub project_id: String,
}

pub(super) struct RemoteGateway {
    lokalise_client: HttpClient<()>,
}

pub type GatewayError = wavesexchange_apis::Error;

impl RemoteGateway {
    pub(super) fn new(api_url: &str, api_token: &str) -> Self {
        let lokalise_client = HttpClient::<()>::builder()
            .with_base_url(api_url)
            .with_reqwest_builder(|req| {
                use reqwest::header::{HeaderMap, HeaderValue};
                let mut headers = HeaderMap::new();
                let value = |s| HeaderValue::from_str(s).expect("api token");
                headers.insert("X-Api-Token", value(api_token));
                req.default_headers(headers)
            })
            .build();
        RemoteGateway { lokalise_client }
    }

    pub(super) async fn keys_for_project(
        &self,
        project_id: &str,
    ) -> Result<dto::KeysResponse, GatewayError> {
        self.lokalise_client
            .create_req_handler::<dto::KeysResponse>(
                self.lokalise_client
                    .http_get(format!("projects/{project_id}/keys?include_translations=1",)),
                "lokalise::get",
            )
            .execute()
            .await
    }
}

pub(super) mod dto {
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
