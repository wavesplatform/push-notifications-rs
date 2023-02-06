//! Push notifications Processor config

use std::fmt;

use serde::Deserialize;

use crate::model::AsBase58String;
use crate::{error::Error, model::Address};

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
    pub redis_hostname: String,
    pub redis_port: u16,
    pub redis_user: String,
    pub redis_password: String,
    pub redis_stream_name: String,
    pub redis_group_name: String,
    pub redis_consumer_name: String,
    pub redis_batch_size: u32,
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
            .field("redis_hostname", &self.redis_hostname)
            .field("redis_port", &self.redis_port)
            .field("redis_user", &self.redis_user)
            .field("redis_password", &"****")
            .field("redis_stream_name", &self.redis_stream_name)
            .field("redis_group_name", &self.redis_group_name)
            .field("redis_consumer_name", &self.redis_consumer_name)
            .field("redis_batch_size", &self.redis_batch_size)
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
            redis_hostname: config.redis_hostname,
            redis_port: config.redis_port,
            redis_user: config.redis_user,
            redis_password: config.redis_password,
            redis_stream_name: config.redis_stream_name,
            redis_group_name: config.redis_group_name,
            redis_consumer_name: config.redis_consumer_name,
            redis_batch_size: config.redis_batch_size,
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
    redis_hostname: String,
    #[serde(default = "default_redis_port")]
    redis_port: u16,
    #[serde(default = "default_redis_user")]
    redis_user: String,
    redis_password: String,
    redis_stream_name: String,
    redis_group_name: String,
    redis_consumer_name: String,
    #[serde(default = "default_redis_batch_size")]
    redis_batch_size: u32,
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_redis_port() -> u16 {
    6379
}

fn default_redis_user() -> String {
    "default".to_string()
}

fn default_redis_batch_size() -> u32 {
    100
}
