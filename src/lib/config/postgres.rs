use serde::Deserialize;
use std::{
    fmt,
    fmt::{Debug, Formatter},
};

use crate::error::Error;

#[derive(Deserialize, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
}

fn default_pgport() -> u16 {
    5432
}

impl PostgresConfig {
    pub fn load() -> Result<Self, Error> {
        Ok(envy::prefixed("PG").from_env::<PostgresConfig>()?)
    }

    pub fn database_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        )
    }
}

impl Debug for PostgresConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Intentionally avoid printing password for security reasons
        write!(
            f,
            "Postgres(server={}:{}; database={}; user={})",
            self.host, self.port, self.database, self.user
        )
    }
}
