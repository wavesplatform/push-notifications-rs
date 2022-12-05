pub mod postgres;
pub mod sender;

use crate::error::Error;
use serde::Deserialize;

#[derive(Clone)]
pub struct Config {
    pub postgres: postgres::Config,
    pub sender: sender::Config,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        Ok(Self {
            postgres: postgres::Config::load()?,
            sender: sender::Config::load()?,
        })
    }
}
