use crate::error::Error;
use chrono::Duration;
use serde::Deserialize;
use std::{
    fmt,
    fmt::{Debug, Formatter},
};

#[derive(Clone)]
pub struct Config {
    pub empty_queue_poll_period: Duration,
    pub exponential_backoff_initial_interval: Duration,
    pub exponential_backoff_multiplier: u8,
    pub send_max_attempts: u8,
    pub fcm_api_key: String,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        Ok(envy::from_env::<ConfigFlat>()?.into())
    }
}

impl From<ConfigFlat> for Config {
    fn from(conf: ConfigFlat) -> Self {
        Self {
            empty_queue_poll_period: Duration::milliseconds(
                conf.send_empty_queue_poll_period_millis as i64,
            ),
            exponential_backoff_initial_interval: Duration::milliseconds(
                conf.send_exponential_backoff_initial_interval_millis as i64,
            ),
            exponential_backoff_multiplier: conf.send_exponential_backoff_multiplier,
            send_max_attempts: conf.send_max_attempts,
            fcm_api_key: conf.fcm_api_key,
        }
    }
}

#[derive(Deserialize)]
struct ConfigFlat {
    #[serde(default = "default_empty_queue_poll_period")]
    pub send_empty_queue_poll_period_millis: u32,
    #[serde(default = "default_exponential_backoff_initial_interval_millis")]
    pub send_exponential_backoff_initial_interval_millis: u32,
    #[serde(default = "default_exponential_backoff_multiplier")]
    pub send_exponential_backoff_multiplier: u8,
    #[serde(default = "default_send_max_attempts")]
    pub send_max_attempts: u8,
    pub fcm_api_key: String,
}

fn default_empty_queue_poll_period() -> u32 {
    5000
}

fn default_exponential_backoff_initial_interval_millis() -> u32 {
    5000
}

fn default_exponential_backoff_multiplier() -> u8 {
    3
}

fn default_send_max_attempts() -> u8 {
    5
}
