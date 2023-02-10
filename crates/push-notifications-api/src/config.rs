//! Push notifications API config

use serde::Deserialize;
use std::time::Duration;

fn default_port() -> u16 {
    8080
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_pool_connection_timeout() -> u32 {
    5
}

fn default_max_subscriptions_per_address_per_pair() -> u32 {
    10
}

fn default_max_subscriptions_per_address_total() -> u32 {
    50
}

#[derive(Deserialize)]
struct ConfigFlat {
    #[serde(default = "default_port")]
    port: u16,

    #[serde(default = "default_metrics_port")]
    metrics_port: u16,

    #[serde(default = "default_pool_connection_timeout")]
    pool_connection_timeout_sec: u32,

    #[serde(default = "default_max_subscriptions_per_address_per_pair")]
    max_subscriptions_per_address_per_pair: u32,

    #[serde(default = "default_max_subscriptions_per_address_total")]
    max_subscriptions_per_address_total: u32,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub metrics_port: u16,
    pub pool_connection_timeout: Duration,
    pub max_subscriptions_per_address_per_pair: u32,
    pub max_subscriptions_per_address_total: u32,
}

impl Config {
    pub fn load() -> Result<Config, envy::Error> {
        let conf = envy::from_env::<ConfigFlat>()?;

        Ok(Config {
            port: conf.port,
            metrics_port: conf.metrics_port,
            pool_connection_timeout: Duration::from_secs(conf.pool_connection_timeout_sec as u64),
            max_subscriptions_per_address_per_pair: conf.max_subscriptions_per_address_per_pair,
            max_subscriptions_per_address_total: conf.max_subscriptions_per_address_total,
        })
    }
}
