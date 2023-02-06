//! Interaction with the Data Service

use anyhow::ensure;
use model::{
    asset::{Asset, AssetPair},
    price::Price,
    waves::{Address, AsBase58String},
};
use wavesexchange_apis::{
    bigdecimal::ToPrimitive,
    data_service::{
        dto::{self, Sort},
        DataService,
    },
    HttpClient,
};

pub(super) struct Pair {
    pub pair: AssetPair,
    pub last_price: Price,
}

pub(super) async fn load_pairs(data_service_url: &str) -> anyhow::Result<Vec<Pair>> {
    log::timer!("Pairs loading", level = info);
    let client = HttpClient::<DataService>::from_base_url(data_service_url);
    let pairs = client.pairs().await?;
    let pairs = pairs.items;
    let mut res = Vec::with_capacity(pairs.len());
    for pair in &pairs {
        log::trace!("Loading pair {} / {}", pair.amount_asset, pair.price_asset);
        let pair = convert_pair(pair).await?;
        res.push(pair);
    }
    Ok(res)
}

async fn convert_pair(pair: &dto::Pair) -> anyhow::Result<Pair> {
    let amount_asset = Asset::from_id(&pair.amount_asset).expect("amt asset");
    let price_asset = Asset::from_id(&pair.price_asset).expect("price asset");
    let last_price = pair.data.last_price.to_f64().expect("price fits f64");
    let pair = Pair {
        pair: AssetPair {
            amount_asset,
            price_asset,
        },
        last_price,
    };
    Ok(pair)
}

pub(super) async fn load_current_blockchain_height(
    data_service_url: &str,
    matcher_address: &Address,
) -> anyhow::Result<u32> {
    log::timer!("Current blockchain height loading", level = info);
    let client = HttpClient::<DataService>::from_base_url(data_service_url);
    let matcher_address = matcher_address.as_base58_string();
    let mut res = client
        .transactions_exchange(
            None::<&str>,
            Some(matcher_address),
            None::<&str>,
            None::<&str>,
            None,
            None,
            Sort::Desc,
            1,
            None::<&str>,
        )
        .await?;
    ensure!(
        !res.items.is_empty(),
        "Unable to fetch current blockchain height: no Exchange transactions from the matcher in Data Service"
    );
    assert_eq!(res.items.len(), 1, "Broken DS API - unexpected data");
    let item = res.items.pop().unwrap(); // Unwrap is safe due to the assertion above
    let tx_data = item.data;
    log::info!("Current blockchain height is {}", tx_data.height);
    Ok(tx_data.height)
}
