use lib::{api, config, db, device, subscription, Error};
use wavesexchange_log::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let pg_config = config::postgres::Config::load()?;
    let config = config::api::Config::load()?;

    let pool = db::async_pool(&pg_config).await?;

    let devices = device::Repo {};
    let subscriptions = subscription::Repo {};

    info!(
        "Starting push-notifications api service with config: {:?}",
        config
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
