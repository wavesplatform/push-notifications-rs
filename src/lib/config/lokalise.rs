use crate::error::Error;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub lokalise_sdk_token: String,
}

impl Config {
    pub fn load() -> Result<Config, Error> {
        envy::from_env().map_err(Error::from)
    }
}
