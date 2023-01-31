//! Push notifications API service executable

extern crate wavesexchange_log as log;

mod api;
mod config;
mod db;
mod error;

use database::{device, subscription};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let pg_config = database::config::Config::load()?;
    let config = config::Config::load()?;
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
