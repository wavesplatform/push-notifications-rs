mod postgres;

use serde::Deserialize;

pub use postgres::PostgresConfig;

#[derive(Deserialize, Clone)]
pub struct Config {
    postgres: PostgresConfig,
}
