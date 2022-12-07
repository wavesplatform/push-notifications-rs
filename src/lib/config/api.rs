use serde::Deserialize;

use crate::error::Error;

fn default_port() -> u16 {
    8080
}

fn default_metrics_port() -> u16 {
    9090
}

#[derive(Deserialize)]
struct ConfigFlat {
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_metrics_port")]
    metrics_port: u16,
    fcm_api_key: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub metrics_port: u16,
    pub fcm_api_key: String,
}

impl Config {
    pub fn load() -> Result<Config, Error> {
        let config_flat = envy::from_env::<ConfigFlat>()?;

        Ok(Config {
            port: config_flat.port,
            metrics_port: config_flat.metrics_port,
            fcm_api_key: config_flat.fcm_api_key,
        })
    }
}
