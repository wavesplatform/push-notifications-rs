//! Push notifications API service executable

extern crate wavesexchange_log as log;

use lib::{api, config, db, device, subscription, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let pg_config = config::postgres::Config::load()?;
    let config = config::api::Config::load()?;
    log::info!("Starting push-notifications api service with {:?}", config);

    log::info!("Connecting to postgres database: {:?}", pg_config);
    let pool = db::async_pool(&pg_config).await?;

    let devices = device::Repo {};
    let subscriptions = subscription::Repo {};

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
