//! Push notifications Processor config

use std::fmt;

use serde::Deserialize;
use error::Error;

use model::{AsBase58String, Address};

#[derive(Clone)]
pub struct Config {
    pub metrics_port: u16,
    pub assets_service_url: String,
    pub lokalise_token: String,
    pub lokalise_project_id: String,
    pub blockchain_updates_url: String,
    pub starting_height: Option<u32>,
    pub matcher_address: Address,
    pub data_service_url: String,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Intentionally avoid printing passwords for security reasons
        f.debug_struct("Config")
            .field("assets_service_url", &self.assets_service_url)
            .field("lokalise_token", &"****")
            .field("lokalise_project_id", &self.lokalise_project_id)
            .field("blockchain_updates_url", &self.blockchain_updates_url)
            .field("starting_height", &self.starting_height)
            .field(
                "matcher_address",
                &format_args!("{}", self.matcher_address.as_base58_string()),
            )
            .field("data_service_url", &self.data_service_url)
            .finish()
    }
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        let config = envy::from_env::<RawConfig>()?;
        let config = Config {
            metrics_port: config.metrics_port,
            assets_service_url: config.assets_service_url,
            lokalise_token: config.lokalise_token,
            lokalise_project_id: config.lokalise_project_id,
            blockchain_updates_url: config.blockchain_updates_url,
            starting_height: if config.starting_height != Some(0) {
                config.starting_height
            } else {
                None
            },
            matcher_address: Address::from_string(&config.matcher_address)
                .map_err(|_| Error::BadConfigValue("matcher_address"))?,
            data_service_url: config.data_service_url,
        };
        Ok(config)
    }
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default = "default_metrics_port")]
    metrics_port: u16,
    assets_service_url: String,
    data_service_url: String,
    blockchain_updates_url: String,
    starting_height: Option<u32>,
    matcher_address: String,
    lokalise_token: String,
    lokalise_project_id: String,
}

fn default_metrics_port() -> u16 {
    9090
}
