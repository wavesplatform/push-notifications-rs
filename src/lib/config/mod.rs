mod lokalise;
mod postgres;

pub use lokalise::LokaliseConfig;
pub use postgres::PostgresConfig;

use crate::error::Error;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub fcm_api_key: String,
    pub postgres: PostgresConfig,
    pub lokalise: LokaliseConfig,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        Ok(Self {
            fcm_api_key: std::env::var("FCM_API_KEY").unwrap_or_default(), // todo make required
            postgres: PostgresConfig::load()?,
            lokalise: LokaliseConfig::load()?,
        })
    }
}
