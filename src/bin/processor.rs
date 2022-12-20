//! Push notifications Processor service executable

extern crate wavesexchange_log as log;

use std::sync::Arc;

use diesel_async::{AsyncConnection, AsyncPgConnection};
use tokio::sync::mpsc;

use lib::{
    asset,
    config::{postgres, processor},
    device, localization, message,
    processing::MessagePump,
    source, subscription,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Configs
    let pg_config = postgres::Config::load()?;
    let config = processor::Config::load()?;

    log::info!("Starting push-notifications processor service with {:?}", config);

    // Database
    log::info!("Connecting to postgres database: {:?}", pg_config);
    let conn = AsyncPgConnection::establish(&pg_config.database_url()).await?;

    // Repo
    log::info!("Initializing repositories");
    let subscriptions = subscription::Repo {};
    let assets = asset::RemoteGateway::new(config.assets_service_url);
    let devices = device::Repo {};
    let localizer = localization::Repo::new(config.lokalise_sdk_token).await?;
    let messages = message::Queue {};

    // Create event sources
    log::info!("Initializing event sources");
    let mut prices_source = source::prices::Source::new(config.matcher_address);
    prices_source
        .init_prices(&config.data_service_url, assets.clone())
        .await?;

    // Unified stream of events
    let (events_tx, events_rx) = mpsc::channel(100); // buffer size is rather arbitrary

    // Start event sources
    log::info!("Starting event sources");
    prices_source
        .start(
            config.blockchain_updates_url,
            config.starting_height,
            events_tx.clone(),
        )
        .await?;

    // Event processor
    log::info!("Initialization finished, starting service");
    let processor = MessagePump::new(subscriptions, assets, devices, localizer, messages);
    let processor = Arc::new(processor);
    processor.run_event_loop(events_rx, conn).await;

    Ok(())
}
