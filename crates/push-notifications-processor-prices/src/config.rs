//! Push notifications Processor config

use std::fmt;

use serde::Deserialize;

use model::waves::{Address, AsBase58String};
use processing::localization::LokaliseConfig;

use self::error::Error;

#[derive(Clone)]
pub struct Config {
    pub assets_service_url: String,
    pub blockchain_updates_url: String,
    pub starting_height: Option<u32>,
    pub matcher_address: Address,
    pub data_service_url: String,
    pub lokalise: LokaliseConfig,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Intentionally avoid printing passwords for security reasons
        f.debug_struct("Config")
            .field("assets_service_url", &self.assets_service_url)
            .field("blockchain_updates_url", &self.blockchain_updates_url)
            .field("starting_height", &self.starting_height)
            .field(
                "matcher_address",
                &format_args!("{}", self.matcher_address.as_base58_string()),
            )
            .field("data_service_url", &self.data_service_url)
            .field("lokalise", &self.lokalise)
            .finish()
    }
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        let config = envy::from_env::<RawConfig>()?;
        let config = Config {
            assets_service_url: config.assets_service_url,
            blockchain_updates_url: config.blockchain_updates_url,
            starting_height: if config.starting_height != Some(0) {
                config.starting_height
            } else {
                None
            },
            matcher_address: Address::from_string(&config.matcher_address)
                .map_err(|_| Error::BadConfigValue("matcher_address"))?,
            data_service_url: config.data_service_url,
            lokalise: LokaliseConfig::load()?,
        };
        Ok(config)
    }
}

#[derive(Deserialize)]
struct RawConfig {
    assets_service_url: String,
    data_service_url: String,
    blockchain_updates_url: String,
    starting_height: Option<u32>,
    matcher_address: String,
}

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error("LoadConfigFailed: {0}")]
        LoadConfigFailed(#[from] envy::Error),

        #[error("BadConfigValue: {0}")]
        BadConfigValue(&'static str),
    }
}
