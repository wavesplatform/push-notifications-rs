//! Push notifications Processor config

use chrono::Duration;
use serde::Deserialize;

use crate::{error::Error, model::Address};

#[derive(Clone)]
pub struct Config {
    pub assets_service_url: String,
    pub lokalise_sdk_token: String,
    pub blockchain_updates_url: String,
    pub starting_height: u32,
    pub matcher_address: Address,
    pub data_service_url: String,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        let config = envy::from_env::<RawConfig>()?;
        let config = Config {
            assets_service_url: config.assets_service_url,
            lokalise_sdk_token: config.lokalise_sdk_token,
            blockchain_updates_url: config.blockchain_updates_url,
            starting_height: config.starting_height,
            matcher_address: Address::from_string(&config.matcher_address)
                .map_err(|_| Error::BadConfigValue("matcher_address"))?,
            data_service_url: config.data_service_url,
        };
        Ok(config)
    }
}

#[derive(Deserialize)]
struct RawConfig {
    assets_service_url: String,
    data_service_url: String,
    blockchain_updates_url: String,
    #[serde(default = "default_starting_height")]
    starting_height: u32,
    matcher_address: String,
    lokalise_sdk_token: String,
}

fn default_starting_height() -> u32 {
    1
}
