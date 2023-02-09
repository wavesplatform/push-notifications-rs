//! Push notifications API service executable

extern crate wavesexchange_log as log;

mod api;
mod config;
mod db;
mod error;
mod topic;

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

    let subscribe_config = subscription::SubscribeConfig {
        max_subscriptions_per_address_per_pair: config.max_subscriptions_per_address_per_pair,
        max_subscriptions_per_address_total: config.max_subscriptions_per_address_total,
    };

    api::start(
        config.port,
        config.metrics_port,
        devices,
        subscriptions,
        subscribe_config,
        pool,
    )
    .await;

    Ok(())
}
