pub mod api;
pub mod lokalise;
pub mod postgres;
pub mod sender;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub postgres: postgres::Config,
    pub sender: sender::Config,
    pub lokalise: lokalise::Config,
    pub api: api::Config,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        Ok(Self {
            postgres: postgres::Config::load()?,
            sender: sender::Config::load()?,
            lokalise: lokalise::Config::load()?,
            api: api::Config::load()?,
        })
    }
}
