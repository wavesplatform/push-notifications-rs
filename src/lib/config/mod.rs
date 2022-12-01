mod postgres;

pub use postgres::PostgresConfig;

use crate::error::Error;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub postgres: PostgresConfig,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        Ok(Self {
            postgres: postgres::PostgresConfig::load()?,
        })
    }
}
