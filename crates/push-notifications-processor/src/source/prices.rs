//! Source of Price events

use std::collections::{HashMap, HashSet};

use tokio::{
    sync::{mpsc, oneshot},
    try_join,
};

use database::stream::{Event, PriceRange, PriceWithDecimals};
use model::{Address, AssetPair, Timestamp};

use self::aggregator::PriceAggregator;
use super::{
    blockchain_updates::{AppendBlock, BlockchainUpdate, BlockchainUpdatesClient},
    data_service,
};
use crate::{asset, processing::EventWithFeedback};

/// A factory that creates and initializes instances of `Source`
pub struct SourceFactory<'a> {
    pub data_service_url: &'a str,
    pub assets: &'a asset::RemoteGateway,
    pub matcher_address: &'a Address,
    pub blockchain_updates_url: &'a str,
    pub starting_height: Option<u32>,
}

/// Source of Price Events (based on blockchain-updates)
pub struct Source {
    stream: mpsc::Receiver<BlockchainUpdate>,
    matcher_address: Address,
    aggregators: HashMap<AssetPair, PriceAggregator>,
}

impl SourceFactory<'_> {
    pub async fn new_source(self) -> anyhow::Result<Source> {
        let initial_prices = self.load_initial_prices();
        let starting_height = self.load_starting_height();
        let client = self.create_grpc_client();
        let (client, starting_height) = try_join!(client, starting_height)?;
        let updates_stream = client.stream(starting_height);
        let (initial_prices, updates_stream) = try_join!(initial_prices, updates_stream)?;
        self.preload_assets_from_pairs(initial_prices.keys())
            .await?;
        let res = Source {
            stream: updates_stream,
            matcher_address: self.matcher_address.to_owned(),
            aggregators: initial_prices,
        };
        Ok(res)
    }

    async fn load_initial_prices(&self) -> anyhow::Result<HashMap<AssetPair, PriceAggregator>> {
        log::info!("Loading pairs from data-service");
        let pairs = data_service::load_pairs(self.data_service_url).await?;
        let mut res = HashMap::with_capacity(pairs.len());
        for pair in pairs {
            let aggregator = PriceAggregator::new(pair.last_price);
            res.insert(pair.pair, aggregator);
        }
        log::info!("Loaded {} pairs", res.len());
        Ok(res)
    }

    async fn preload_assets_from_pairs<'a>(
        &self,
        pairs: impl Iterator<Item = &'a AssetPair>,
    ) -> anyhow::Result<()> {
        let unique_assets = pairs
            .map(|p| vec![&p.amount_asset, &p.price_asset])
            .flatten()
            .collect::<HashSet<_>>()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        log::info!(
            "Preloading {} unique assets using Asset Service",
            unique_assets.len()
        );

        self.assets.preload(unique_assets).await?;
        Ok(())
    }

    async fn load_starting_height(&self) -> anyhow::Result<u32> {
        let height = match self.starting_height {
            Some(height) => {
                log::info!("Starting height is configured to be {}", height);
                height
            }
            None => {
                log::info!("Loading height of a latest Exchange transaction from data-service");
                let height = data_service::load_current_blockchain_height(
                    self.data_service_url,
                    self.matcher_address,
                )
                .await?;
                log::info!("Starting height is {}", height);
                height
            }
        };
        Ok(height)
    }

    async fn create_grpc_client(&self) -> anyhow::Result<BlockchainUpdatesClient> {
        let url = self.blockchain_updates_url.to_owned();
        log::debug!("Connecting to blockchain-updates: {}", url);
        let client = BlockchainUpdatesClient::connect(url).await?;
        Ok(client)
    }
}

impl Source {
    pub async fn run(mut self, sink: mpsc::Sender<EventWithFeedback>) -> anyhow::Result<()> {
        while let Some(upd) = self.stream.recv().await {
            match upd {
                BlockchainUpdate::Append(block) => {
                    let result = self.process_block(block, &sink).await;
                    match result {
                        Ok(()) => {}
                        Err(Error::StopProcessing) => break,
                        Err(Error::EventProcessingFailed(err)) => {
                            log::error!("Event processing failed: {}", err);
                            return Err(err.into());
                        }
                    }
                }
                BlockchainUpdate::Rollback(_) => {}
            }
        }
        log::debug!("Blockchain updates loop finished");
        Ok(())
    }

    async fn process_block(
        &mut self,
        block: AppendBlock,
        sink: &mpsc::Sender<EventWithFeedback>,
    ) -> Result<(), Error> {
        //log::trace!("Processing block {} at height {}", block.block_id, block.height);
        let timestamp = block.timestamp;
        let block_prices = self.aggregate_prices_from_block(block);
        Self::send_price_events(block_prices, timestamp, sink).await
    }

    fn aggregate_prices_from_block(&mut self, block: AppendBlock) -> Vec<(AssetPair, PriceRange)> {
        self.aggregators
            .values_mut()
            .for_each(PriceAggregator::reset);

        for tx in block.transactions {
            if tx.sender == self.matcher_address {
                let asset_pair = AssetPair {
                    amount_asset: tx.exchange_tx.amount_asset,
                    price_asset: tx.exchange_tx.price_asset,
                };
                let new_price = PriceWithDecimals {
                    price: tx.exchange_tx.price,
                    decimals: 8, // This is a fixed value, same for all assets
                };
                let new_price = new_price.value();
                let aggregator = self
                    .aggregators
                    .entry(asset_pair)
                    .or_insert_with(|| PriceAggregator::new(new_price));
                aggregator.update(new_price);
            }
        }

        self.aggregators
            .values_mut()
            .for_each(PriceAggregator::finalize);

        self.aggregators
            .iter()
            .map(|(pair, agg)| (pair, agg.range()))
            .filter(|&(_pair, range)| !range.is_empty())
            .map(|(pair, range)| (pair.to_owned(), range.to_owned()))
            .collect()
    }

    async fn send_price_events(
        block_prices: Vec<(AssetPair, PriceRange)>,
        timestamp: Timestamp,
        sink: &mpsc::Sender<EventWithFeedback>,
    ) -> Result<(), Error> {
        for (asset_pair, price_range) in block_prices {
            debug_assert_eq!(price_range.is_empty(), false);
            let event = Event::PriceChanged {
                asset_pair,
                price_range,
                timestamp,
            };
            let (tx, rx) = oneshot::channel();
            let evf = EventWithFeedback {
                event,
                result_tx: tx,
            };
            sink.send(evf).await.map_err(|_| Error::StopProcessing)?;
            let result = rx.await.map_err(|_| Error::StopProcessing)?;
            result.map_err(|err| Error::EventProcessingFailed(err))?;
        }
        Ok(())
    }
}

enum Error {
    StopProcessing,
    EventProcessingFailed(error::Error),
}

mod aggregator {
    use database::stream::{Price, PriceRange};
    use std::mem::take;

    pub(super) struct PriceAggregator {
        prev_block_price: Price,
        latest_price: Price,
        current_range: PriceRange,
    }

    impl PriceAggregator {
        pub(super) fn new(last_known_price: Price) -> Self {
            PriceAggregator {
                prev_block_price: last_known_price,
                latest_price: last_known_price,
                current_range: PriceRange::empty(),
            }
        }

        pub(super) fn reset(&mut self) {
            self.current_range = PriceRange::empty();
        }

        pub(super) fn update(&mut self, new_price: Price) {
            let current_range = &mut self.current_range;
            *current_range = take(current_range).extend(new_price);
            self.latest_price = new_price;
        }

        pub(super) fn finalize(&mut self) {
            let current_range = &mut self.current_range;
            *current_range = take(current_range)
                .extend(self.prev_block_price)
                .exclude_bound(self.prev_block_price);
            self.prev_block_price = self.latest_price;
        }

        pub(super) fn range(&self) -> &PriceRange {
            &self.current_range
        }
    }

    #[test]
    fn test_aggregator() {
        let mut agg = PriceAggregator::new(0.0);

        let threshold = 5.0;

        // Block 1: range = [4..5], hit threshold 5, close_price = 5
        agg.update(4.0);
        agg.update(4.5);
        agg.update(5.0);
        agg.finalize();
        let range = agg.range();
        assert_eq!(range.contains(threshold), true);

        // Block 2: range = (5..6], threshold 5 not hit again
        agg.reset();
        agg.update(5.5);
        agg.update(6.0);
        agg.finalize();
        let range = agg.range();
        assert_eq!(range.contains(threshold), false);
    }
}
