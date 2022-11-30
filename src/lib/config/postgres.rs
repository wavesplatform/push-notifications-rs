use serde::Deserialize;
use std::fmt::{Debug, Formatter, Result};

#[derive(Deserialize, Clone)]
pub struct PostgresConfig {
    #[serde(rename = "pghost")]
    pub host: String,

    #[serde(rename = "pgport", default = "default_pgport")]
    pub port: u16,

    #[serde(rename = "pgdatabase")]
    pub database: String,

    #[serde(rename = "pguser")]
    pub user: String,

    #[serde(rename = "pgpassword")]
    pub password: String,
}

fn default_pgport() -> u16 {
    5432
}

impl PostgresConfig {
    pub fn database_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        )
    }
}

impl Debug for PostgresConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        // Intentionally avoid printing password for security reasons
        write!(
            f,
            "Postgres(server={}:{}; database={}; user={})",
            self.host, self.port, self.database, self.user
        )
    }
}
