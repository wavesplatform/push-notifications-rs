use std::time::Duration;

use database::config::Config;
use error::Error;
use diesel_async::{
    pooled_connection::{bb8::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};

pub type PgAsyncPool = Pool<AsyncPgConnection>;

pub async fn async_pool(config: &Config) -> Result<PgAsyncPool, Error> {
    let db_url = Config::database_url(config);
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(db_url);

    let pool = Pool::builder()
        .connection_timeout(Duration::from_secs(5))
        .build(config)
        .await
        .map_err(|e| Error::Generic(e.to_string()))?;

    Ok(pool)
}
