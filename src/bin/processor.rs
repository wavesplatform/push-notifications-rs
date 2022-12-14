use std::sync::Arc;

use diesel_async::{AsyncConnection, AsyncPgConnection};
use tokio::sync::mpsc;

use lib::{
    asset, config::postgres, device, localization, message, model::Address,
    processing::MessagePump, source, subscription,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Configs
    let config = postgres::Config::load()?;
    let assets_service_url = ""; //TODO get from config
    let lokalise_sdk_token = ""; //TODO get from config
    let blockchain_updates_url = "".to_string(); //TODO get from config
    let starting_height = 1_u32; //TODO get from config
    let matcher_address = Address::from_string("").unwrap(); //TODO get from config
    let data_service_url = ""; //TODO get from configs

    // Database
    let conn = AsyncPgConnection::establish(&config.database_url()).await?;

    // Repo
    let subscriptions = subscription::Repo {};
    let assets = asset::RemoteGateway::new(assets_service_url);
    let devices = device::Repo {};
    let localizer = localization::Repo::new(lokalise_sdk_token).await?;
    let messages = message::Queue {};

    // Create event sources
    let mut prices_source = source::prices::Source::new(matcher_address);
    prices_source
        .init_prices(data_service_url, assets.clone())
        .await?;

    // Unified stream of events
    let (events_tx, events_rx) = mpsc::channel(100); // buffer size is rather arbitrary

    // Start event sources
    prices_source
        .start(blockchain_updates_url, starting_height, events_tx.clone())
        .await?;

    // Event processor
    let processor = MessagePump::new(subscriptions, assets, devices, localizer, messages);
    let processor = Arc::new(processor);
    processor.run_event_loop(events_rx, conn).await;

    Ok(())
}
