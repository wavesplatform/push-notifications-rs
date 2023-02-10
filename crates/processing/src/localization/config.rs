//! Lokalise API config

use serde::Deserialize;
use std::fmt;

#[derive(Deserialize, Clone)]
pub struct LokaliseConfig {
    pub token: String,
    pub project_id: String,

    #[serde(default = "default_api_url")]
    pub api_url: String,
}

fn default_api_url() -> String {
    "https://api.lokalise.com/api2".to_string()
}

impl LokaliseConfig {
    pub fn load() -> Result<Self, envy::Error> {
        Ok(envy::prefixed("LOKALISE_").from_env::<LokaliseConfig>()?)
    }
}

impl fmt::Debug for LokaliseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Intentionally avoid printing password for security reasons
        f.debug_struct("LokaliseConfig")
            .field("token", &"****")
            .field("project_id", &self.project_id)
            .field("api_url", &self.api_url)
            .finish()
    }
}
