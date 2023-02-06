use std::time::Duration;

use crate::config::postgres;
use crate::error::Error;
use diesel_async::{
    pooled_connection::{bb8::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};

pub type PgAsyncPool = Pool<AsyncPgConnection>;

pub async fn async_pool(config: &postgres::Config) -> Result<PgAsyncPool, Error> {
    let db_url = postgres::Config::database_url(config);
    let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);

    let pool = Pool::builder()
        .connection_timeout(Duration::from_secs(5))
        .build(config)
        .await
        .map_err(|e| Error::Generic(e.to_string()))?;

    Ok(pool)
}
