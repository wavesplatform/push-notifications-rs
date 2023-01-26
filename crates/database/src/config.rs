//! Postgres database config

use std::fmt;

use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub host: String,
    #[serde(default = "default_pgport")]
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
}

fn default_pgport() -> u16 {
    5432
}

impl Config {
    pub fn load() -> Result<Self, envy::Error> {
        Ok(envy::prefixed("PG").from_env::<Config>()?)
    }

    pub fn database_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        )
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Intentionally avoid printing password for security reasons
        write!(
            f,
            "Postgres(server={}:{}; database={}; user={}; password=***)",
            self.host, self.port, self.database, self.user
        )
    }
}
