use crate::error::Error;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct LokaliseConfig {
    pub lokalise_sdk_token: String,
}

impl LokaliseConfig {
    pub fn load() -> Result<LokaliseConfig, Error> {
        envy::from_env().map_err(Error::from)
    }
}
