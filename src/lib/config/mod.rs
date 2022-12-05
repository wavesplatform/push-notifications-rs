pub mod lokalise;
pub mod postgres;

use crate::error::Error;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub fcm_api_key: String,
    pub postgres: postgres::Config,
    pub lokalise: lokalise::Config,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        Ok(Self {
            fcm_api_key: std::env::var("FCM_API_KEY").unwrap_or_default(), // todo make required
            postgres: postgres::Config::load()?,
            lokalise: lokalise::Config::load()?,
        })
    }
}
