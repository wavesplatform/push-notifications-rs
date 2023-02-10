use crate::error::Error;
use database::config::Config;
use diesel_async::{
    pooled_connection::{bb8::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use std::time::Duration;

pub type PgAsyncPool = Pool<AsyncPgConnection>;

pub async fn async_pool(
    config: &Config,
    connection_timeout: Duration,
) -> Result<PgAsyncPool, Error> {
    let db_url = Config::database_url(config);
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(db_url);

    let pool = Pool::builder()
        .connection_timeout(connection_timeout)
        .build(config)
        .await
        .map_err(|e| Error::Generic(e.to_string()))?;

    Ok(pool)
}
