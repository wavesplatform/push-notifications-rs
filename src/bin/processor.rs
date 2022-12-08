#![allow(unused_imports, unreachable_code, unused_variables)] //TODO cleanup

use std::sync::Arc;

use diesel_async::{AsyncConnection, AsyncPgConnection};

use lib::{
    asset, config::postgres::Config, device, localization, message, processing::MessagePump,
    subscription, Error,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::load()?;
    let assets_service_url = ""; //TODO get from config
    let lokalise_sdk_token = ""; //TODO get from config

    let conn = AsyncPgConnection::establish(&config.database_url()).await?;

    let subscriptions = subscription::Repo {};
    let assets = asset::RemoteGateway::new(assets_service_url);
    let devices = device::Repo {};
    let localizer = localization::Repo::new(lokalise_sdk_token).await?;
    let messages = message::Queue {};

    let processor = MessagePump::new(subscriptions, assets, devices, localizer, messages);
    let processor = Arc::new(processor);
    let events = todo!();
    processor.run_event_loop(events, conn).await;

    Ok(())
}
