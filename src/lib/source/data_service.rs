//! Interaction with the Data Service

use crate::{
    asset,
    model::{Address, AsBase58String, Asset, AssetPair},
    stream::PriceWithDecimals,
};
use anyhow::ensure;
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
    pub last_price: PriceWithDecimals,
}

pub(super) async fn load_pairs(
    data_service_url: &str,
    assets: &asset::RemoteGateway,
) -> anyhow::Result<Vec<Pair>> {
    log::timer!("Pairs loading", level = info);
    let client = HttpClient::<DataService>::from_base_url(data_service_url);
    let pairs = client.pairs().await?;
    let pairs = pairs.items;
    let unique_assets = pairs
        .iter()
        .map(|p| vec![&p.amount_asset, &p.price_asset])
        .flatten()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|asset| Asset::from_id(asset).expect("asset"))
        .collect::<Vec<_>>();
    log::debug!(
        "Loading {} unique assets from {} pairs",
        unique_assets.len(),
        pairs.len()
    );
    assets.preload(unique_assets).await?;
    let mut res = Vec::with_capacity(pairs.len());
    for pair in &pairs {
        log::trace!("Loading pair {} / {}", pair.amount_asset, pair.price_asset);
        let pair = convert_pair(pair, assets).await?;
        res.push(pair);
    }
    Ok(res)
}

async fn convert_pair(pair: &dto::Pair, assets: &asset::RemoteGateway) -> anyhow::Result<Pair> {
    let amount_asset = Asset::from_id(&pair.amount_asset).expect("amt asset");
    let price_asset = Asset::from_id(&pair.price_asset).expect("price asset");
    let last_price_raw = pair.data.last_price.to_u64().expect("price fits u64");
    let price_decimals = {
        let amount_asset_decimals = assets.decimals(&amount_asset).await? as i16;
        let price_asset_decimals = assets.decimals(&price_asset).await? as i16;
        let decimals = 8 + price_asset_decimals - amount_asset_decimals;
        ensure!(
            decimals >= 0 && decimals <= 255,
            "Unexpected price_decimals: {decimals} for asset pair {amount_asset}/{price_asset} ({amount_asset_decimals}/{price_asset_decimals})"
        );
        decimals as u8 // Cast is safe due to the check above
    };
    let pair = Pair {
        pair: AssetPair {
            amount_asset,
            price_asset,
        },
        last_price: PriceWithDecimals {
            price: last_price_raw,
            decimals: price_decimals,
        },
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
