use diesel_async::{AsyncConnection, AsyncPgConnection};
use lib::{api, config::Config, device, subscription, Error};
use wavesexchange_log::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::load()?;

    let conn = AsyncPgConnection::establish(&config.postgres.database_url()).await?;

    let repos = api::Repos {
        device: device::Repo {},
        subscriptions: subscription::Repo {},
    };

    info!(
        "Starting push-notifications api service with config: {:?}",
        config
    );

    api::start(config.api.port, config.api.metrics_port, repos, conn).await;

    Ok(())
}
