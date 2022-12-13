use lib::{api, config::Config, db, device, subscription, Error};
use wavesexchange_log::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::load()?;

    let pool = db::async_pool(&config.postgres).await?;

    let repos = api::Repos {
        device: device::Repo {},
        subscriptions: subscription::Repo {},
    };

    info!(
        "Starting push-notifications api service with config: {:?}",
        config
    );

    api::start(config.api.port, config.api.metrics_port, repos, pool).await;

    Ok(())
}
