//! Push notifications Processor service executable

#[macro_use]
extern crate tokio;

extern crate wavesexchange_log as log;

use std::sync::Arc;

use diesel_async::{AsyncConnection, AsyncPgConnection};
use tokio::{sync::mpsc, task};

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

    log::info!(
        "Starting push-notifications processor service with {:?}",
        config
    );

    let lokalise_config = localization::LokaliseConfig {
        token: config.lokalise_token,
        project_id: config.lokalise_project_id,
    };

    // Database
    log::info!("Connecting to postgres database: {:?}", pg_config);
    let conn = AsyncPgConnection::establish(&pg_config.database_url()).await?;

    // Repo
    log::info!("Initializing repositories");
    let subscriptions = subscription::Repo {};
    let assets = asset::RemoteGateway::new(config.assets_service_url);
    let devices = device::Repo {};
    let localizer = localization::Repo::new(lokalise_config).await?;
    let messages = message::Queue {};

    // Create event sources
    log::info!("Initializing event sources");
    let prices_source = {
        let factory = source::prices::SourceFactory {
            data_service_url: &config.data_service_url,
            assets: &assets,
            matcher_address: &config.matcher_address,
            blockchain_updates_url: &config.blockchain_updates_url,
            // Starting height in config is mostly for debugging purposes.
            // For production is should not be set so that we can use current blockchain height.
            starting_height: config.starting_height,
        };

        factory.new_source().await?
    };

    // Unified stream of events
    let (events_tx, events_rx) = mpsc::channel(100); // buffer size is rather arbitrary

    // Start event sources
    log::info!("Starting event sources");
    let h_prices_source = task::spawn(prices_source.run(events_tx.clone()));
    //todo start other sources cloning events_tx
    drop(events_tx); // Make sure only sources now have the tx side of the channel

    // Event processor
    log::info!("Initialization finished, starting service");
    let processor = MessagePump::new(subscriptions, assets, devices, localizer, messages);
    let processor = Arc::new(processor);
    let h_processor = task::spawn(async { processor.run_event_loop(events_rx, conn).await });

    // Join all the background tasks
    let ((), r_prices_source) = try_join!(h_processor, h_prices_source)?;
    let () = r_prices_source?;

    Ok(())
}
