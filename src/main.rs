use wavesexchange_apis::HttpClient;

mod config;
mod error;
mod repo;

use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), error::Error> {
    let config = config::load()?;

    let auth_header = HashMap::from([("X-Api-Token".to_string(), config.lokalise_sdk_token)]);
    let lokalise_client = HttpClient::<()>::builder()
        .with_base_url("https://api.lokalise.co/api2")
        .with_reqwest_builder(|rb| rb.default_headers((&auth_header).try_into().unwrap()))
        .build();
    let repo = repo::lokalise::Repo::new(lokalise_client, config.project_id);

    println!("{}", repo.get("priceAlertMessage", "ru").await?);
    Ok(())
}
