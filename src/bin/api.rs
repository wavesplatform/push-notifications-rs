//! Push notifications API service executable

extern crate wavesexchange_log as log;

use lib::{api, config, db, device, subscription, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let pg_config = config::postgres::Config::load()?;
    let config = config::api::Config::load()?;

    let pool = db::async_pool(&pg_config).await?;

    let devices = device::Repo {};
    let subscriptions = subscription::Repo {};

    log::info!(
        "Starting push-notifications api service with config {:?}, postgres {:?}",
        config,
        pg_config
    );

    api::start(
        config.port,
        config.metrics_port,
        devices,
        subscriptions,
        pool,
    )
    .await;

    Ok(())
}
