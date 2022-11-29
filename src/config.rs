use crate::error::Error;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub project_id: String,
    pub lokalise_sdk_token: String,
}

pub fn load() -> Result<Config, Error> {
    envy::from_env::<Config>().map_err(Error::from)
}
