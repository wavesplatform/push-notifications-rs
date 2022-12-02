pub mod postgres;

use crate::error::Error;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub postgres: postgres::Config,
    pub fcm_api_key: String,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        Ok(Self {
            postgres: postgres::Config::load()?,
            fcm_api_key: std::env::var("FCM_API_KEY").unwrap_or_default(),
        })
    }
}
