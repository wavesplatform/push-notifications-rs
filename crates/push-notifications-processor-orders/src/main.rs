//! Push notifications orders processor service executable

extern crate wavesexchange_log as log;

mod config;
mod source;

use std::sync::Arc;

use diesel_async::{AsyncConnection, AsyncPgConnection};
use tokio::{sync::mpsc, task, try_join};

use wavesexchange_warp::MetricsWarpBuilder;

use database::{device, message, subscription};
use processing::{asset, localization, MessagePump};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Configs
    let pg_config = database::config::Config::load()?;
    let config = config::Config::load()?;

    log::info!(
        "Starting push-notifications orders processor service with {:?}",
        config
    );

    let lokalise_config = localization::LokaliseConfig {
        token: config.lokalise_token,
        project_id: config.lokalise_project_id,
    };

    // Initialization
    //let (init_finished_tx, init_finished_rx) = oneshot::channel(); //TODO readyz

    // Stats & liveness endpoints
    task::spawn(async move {
        MetricsWarpBuilder::new()
            .with_metrics_port(config.metrics_port)
            //.with_readyz_checker(|| async move { init_finished_rx.await }) //TODO readyz
            .run_async()
    });

    // Database
    log::info!("Connecting to postgres database: {:?}", pg_config);
    let conn = AsyncPgConnection::establish(&pg_config.database_url()).await?;

    // Repo
    log::info!("Initializing repositories");
    let subscriptions = subscription::Repo {};
    let assets = asset::RemoteGateway::new(config.assets_service_url);
    let devices = device::Repo {};
    let localizer = task::spawn(localization::Repo::new(lokalise_config));
    let messages = message::Queue {};

    // Create event sources
    log::info!("Initializing orders event source");
    let orders_source = {
        let config = source::orders::SourceConfig {
            connection: source::orders::RedisConnectionConfig {
                hostname: config.redis_hostname,
                port: config.redis_port,
                user: config.redis_user,
                password: config.redis_password,
            },
            stream: source::orders::RedisStreamConfig {
                stream_name: config.redis_stream_name,
                group_name: config.redis_group_name,
                consumer_name: config.redis_consumer_name,
            },
            batch_max_size: config.redis_batch_size,
        };
        source::orders::Source::new(config).await?
    };

    // Unified stream of events
    let (events_tx, events_rx) = mpsc::channel(100); // buffer size is rather arbitrary

    // Start event sources
    log::info!("Starting orders event source");
    let h_orders_source = task::spawn(orders_source.run(events_tx));

    // Await on all remaining initialization tasks running in background
    let localizer = localizer.await??;

    // Event processor
    log::info!("Initialization finished, starting service");
    let processor = MessagePump::new(subscriptions, assets, devices, localizer, messages);
    let processor = Arc::new(processor);
    let h_processor = task::spawn(async { processor.run_event_loop(events_rx, conn).await });

    // Initialization phase finished
    //let () = init_finished_tx.send(()).expect("init"); //TODO readyz

    // Join all the background tasks
    let ((), r_orders_source) = try_join!(h_processor, h_orders_source)?;
    let () = r_orders_source?;

    log::info!("Service finished.");

    Ok(())
}
