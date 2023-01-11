//! Blockchain updates

pub mod prices {
    use std::collections::HashMap;

    use tokio::sync::{mpsc, oneshot};

    use self::aggregator::PriceAggregator;
    use super::{
        blockchain_updates::{AppendBlock, BlockchainUpdate, BlockchainUpdatesClient},
        data_service::load_pairs,
    };
    use crate::{
        asset,
        model::{Address, AssetPair, Timestamp},
        processing::EventWithFeedback,
        stream::{Event, PriceRange, PriceWithDecimals},
    };

    pub struct Source {
        matcher_address: Address,
        aggregators: HashMap<AssetPair, PriceAggregator>,
    }

    impl Source {
        pub fn new(matcher_address: Address) -> Self {
            Source {
                matcher_address,
                aggregators: HashMap::new(),
            }
        }

        //TODO Initialization is an implementation detail. Rework as factory or smth like that.
        pub async fn init_prices(
            &mut self,
            data_service_url: &str,
            assets: asset::RemoteGateway,
        ) -> Result<(), anyhow::Error> {
            log::info!("Loading pairs from data-service");
            let pairs = load_pairs(data_service_url, assets).await?;
            for pair in pairs {
                let aggregator = PriceAggregator::new(pair.last_price);
                self.aggregators.insert(pair.pair, aggregator);
            }
            Ok(())
        }

        pub async fn run(
            &mut self,
            blockchain_updates_url: String,
            starting_height: u32,
            sink: mpsc::Sender<EventWithFeedback>,
        ) -> Result<(), anyhow::Error> {
            log::debug!(
                "Connecting to blockchain-updates: {}",
                blockchain_updates_url
            );
            let client = BlockchainUpdatesClient::connect(blockchain_updates_url).await?;
            log::debug!(
                "Starting receiving blockchain updates from height {}",
                starting_height
            );
            let mut stream = client.stream(starting_height).await?;
            while let Some(upd) = stream.recv().await {
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
            self.send_price_events(block_prices, timestamp, sink).await
        }

        fn aggregate_prices_from_block(
            &mut self,
            block: AppendBlock,
        ) -> Vec<(AssetPair, PriceRange)> {
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
                        decimals: 8, // This is a hard-coded value
                    };
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
            &self,
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
        EventProcessingFailed(crate::error::Error),
    }

    mod aggregator {
        use crate::stream::{PriceRange, PriceWithDecimals};
        use std::mem::take;

        pub(super) struct PriceAggregator {
            prev_block_price: PriceWithDecimals,
            latest_price: PriceWithDecimals,
            current_range: PriceRange,
        }

        impl PriceAggregator {
            pub(super) fn new(last_known_price: PriceWithDecimals) -> Self {
                PriceAggregator {
                    prev_block_price: last_known_price,
                    latest_price: last_known_price,
                    current_range: PriceRange::empty(),
                }
            }

            pub(super) fn reset(&mut self) {
                self.current_range = PriceRange::empty();
            }

            pub(super) fn update(&mut self, new_price: PriceWithDecimals) {
                let current_range = &mut self.current_range;
                *current_range = take(current_range).extend(new_price.value());
                self.latest_price = new_price;
            }

            pub(super) fn finalize(&mut self) {
                let current_range = &mut self.current_range;
                *current_range = take(current_range)
                    .extend(self.prev_block_price.value())
                    .exclude_bound(self.prev_block_price.value());
                self.prev_block_price = self.latest_price;
            }

            pub(super) fn range(&self) -> &PriceRange {
                &self.current_range
            }
        }

        #[test]
        fn test_aggregator() {
            let p = |price, decimals| PriceWithDecimals { price, decimals };
            let mut agg = PriceAggregator::new(p(0, 0));

            let threshold = 5.0;

            // Block 1: range = [4..5], hit threshold 5, close_price = 5
            agg.update(p(400, 2));
            agg.update(p(450, 2));
            agg.update(p(500, 2));
            agg.finalize();
            let range = agg.range();
            assert_eq!(range.contains(threshold), true);

            // Block 2: range = (5..6], threshold 5 not hit again
            agg.reset();
            agg.update(p(550, 2));
            agg.update(p(600, 2));
            agg.finalize();
            let range = agg.range();
            assert_eq!(range.contains(threshold), false);
        }
    }
}

mod data_service {
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
        assets: asset::RemoteGateway,
    ) -> Result<Vec<Pair>, anyhow::Error> {
        log::timer!("Pairs loading", level = info);
        let client = HttpClient::<DataService>::from_base_url(data_service_url);
        let pairs = client.pairs().await?;
        let pairs = pairs.items;
        let mut res = Vec::with_capacity(pairs.len());
        for pair in pairs.into_iter() {
            log::debug!("Loading pair {} / {}", pair.amount_asset, pair.price_asset);
            let pair = convert_pair(&pair, &assets).await?;
            res.push(pair);
        }
        Ok(res)
    }

    async fn convert_pair(
        pair: &dto::Pair,
        assets: &asset::RemoteGateway,
    ) -> Result<Pair, anyhow::Error> {
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

    pub async fn load_current_blockchain_height(
        data_service_url: &str,
        matcher_address: &Address,
    ) -> Result<u32, anyhow::Error> {
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
        anyhow::ensure!(
            !res.items.is_empty(),
            "Unable to fetch current blockchain height: no Exchange transactions in Data Service"
        );
        assert_eq!(res.items.len(), 1, "Broken DS API - unexpected data");
        let item = res.items.pop().unwrap(); // Unwrap is safe due to the assertion above
        let tx_data = item.data;
        log::info!("Current blockchain height is {}", tx_data.height);
        Ok(tx_data.height)
    }
}

pub use data_service::load_current_blockchain_height;

mod blockchain_updates {
    use tokio::{sync::mpsc, task};

    use waves_protobuf_schemas::{
        tonic,
        waves::events::grpc::{
            blockchain_updates_api_client::BlockchainUpdatesApiClient, SubscribeEvent,
            SubscribeRequest,
        },
    };

    use crate::{
        model::{Address, Asset, Timestamp},
        stream::RawPrice,
    };

    #[derive(Debug)]
    pub(super) enum BlockchainUpdate {
        Append(AppendBlock),
        Rollback(Rollback),
    }

    #[allow(dead_code)] // fields `block_id`, `height` and `is_microblock` are never read
    #[derive(Debug)]
    pub(super) struct AppendBlock {
        pub block_id: String,     // Do we needed it?
        pub height: u32,          // Do we need it?
        pub timestamp: Timestamp, // Either block timestamp or current system time for microblock
        pub is_microblock: bool,  // Do we need it?
        pub transactions: Vec<Transaction>,
    }

    #[allow(dead_code)] // field `block_id` is never read
    #[derive(Debug)]
    pub(super) struct Rollback {
        pub block_id: String,
    }

    #[allow(dead_code)] // fields `id`, `height` and `timestamp` are never read
    #[derive(Debug)]
    pub(super) struct Transaction {
        pub id: String,
        pub height: u32,
        pub timestamp: u64, // Not usable as it may be +- several hours from actual
        pub sender: Address,
        pub exchange_tx: TxExchange,
    }

    #[allow(dead_code)] // field `amount` is never read
    #[derive(Debug)]
    pub(super) struct TxExchange {
        pub amount_asset: Asset,
        pub price_asset: Asset,
        pub amount: RawPrice,
        pub price: RawPrice,
    }

    #[derive(Clone)]
    pub(super) struct BlockchainUpdatesClient(
        BlockchainUpdatesApiClient<tonic::transport::Channel>,
    );

    impl BlockchainUpdatesClient {
        pub(super) async fn connect(blockchain_updates_url: String) -> Result<Self, anyhow::Error> {
            let grpc_client = BlockchainUpdatesApiClient::connect(blockchain_updates_url).await?;
            Ok(BlockchainUpdatesClient(grpc_client))
        }

        pub(super) async fn stream(
            self,
            from_height: u32,
        ) -> Result<mpsc::Receiver<BlockchainUpdate>, anyhow::Error> {
            let BlockchainUpdatesClient(mut grpc_client) = self;

            let request = tonic::Request::new(SubscribeRequest {
                from_height: from_height as i32,
                to_height: 0,
            });

            let stream = grpc_client.subscribe(request).await?.into_inner();

            let (tx, rx) = mpsc::channel::<BlockchainUpdate>(1);

            task::spawn(async move {
                let res = pump_messages(stream, tx).await;
                if let Err(err) = res {
                    log::error!("Error receiving blockchain updates: {}", err);
                } else {
                    log::warn!("GRPC connection closed by the server");
                }
            });

            async fn pump_messages(
                mut stream: tonic::Streaming<SubscribeEvent>,
                tx: mpsc::Sender<BlockchainUpdate>,
            ) -> anyhow::Result<()> {
                while let Some(event) = stream.message().await? {
                    if let Some(update) = event.update {
                        let update = convert::convert_update(update)?;
                        tx.send(update).await?;
                    }
                }
                Ok(())
            }

            Ok(rx)
        }
    }

    mod convert {
        use thiserror::Error;

        // The sole purpose of the following two modules is to organize imports
        // and scope types with the same name.

        /// This module reexports all necessary structs from the protobuf crate, for convenience.
        mod proto {
            pub(super) use waves_protobuf_schemas::waves::{
                events::{
                    blockchain_updated::{
                        append::{BlockAppend, Body, MicroBlockAppend},
                        Append, Update,
                    },
                    transaction_metadata::{ExchangeMetadata, Metadata},
                    BlockchainUpdated, TransactionMetadata,
                },
                signed_transaction,
                transaction::Data,
                Block, ExchangeTransactionData, MicroBlock, SignedMicroBlock, SignedTransaction,
                Transaction,
            };
        }

        /// This module reexports all necessary structs from the application model, for convenience.
        mod model {
            pub(super) use super::super::{
                AppendBlock, BlockchainUpdate, Rollback, Transaction, TxExchange,
            };
            pub(super) use crate::model::{Address, Asset, AssetId, Timestamp};
        }

        #[derive(Error, Debug)]
        #[error("failed to convert blockchain update: {0}")]
        pub(super) struct ConvertError(&'static str);

        pub(super) fn convert_update(
            src: proto::BlockchainUpdated,
        ) -> Result<model::BlockchainUpdate, ConvertError> {
            let height = src.height as u32;
            let update = src.update;
            match update {
                Some(proto::Update::Append(append)) => {
                    let body = append.body.ok_or(ConvertError("append body is None"))?;
                    let proto::Append {
                        transaction_ids,
                        transactions_metadata,
                        ..
                    } = append;

                    let is_microblock = extract_is_microblock(&body)
                        .ok_or(ConvertError("failed to extract is_microblock"))?;

                    let id = extract_id(&body, &src.id)
                        .ok_or(ConvertError("failed to extract block id"))?;
                    let id = base58(id);

                    // Only full blocks have timestamp, microblocks doesn't.
                    // But it is okay to use current system time in case of a microblock,
                    // because it will differ from a real microblock timestamp by a negligible margin.
                    // Though, it is a hack.
                    let timestamp = extract_timestamp(&body).unwrap_or_else(current_timestamp);

                    let transactions =
                        extract_transactions(body).ok_or(ConvertError("transactions is None"))?;
                    assert!(
                        transaction_ids.len() == transactions.len()
                            && transactions.len() == transactions_metadata.len()
                    );
                    let transactions = convert_transactions(
                        transaction_ids,
                        transactions,
                        transactions_metadata,
                        height,
                    )?;

                    let append = model::AppendBlock {
                        block_id: id,
                        height,
                        timestamp,
                        is_microblock,
                        transactions,
                    };
                    Ok(model::BlockchainUpdate::Append(append))
                }
                Some(proto::Update::Rollback(_)) => {
                    let rollback_to_block_id = base58(&src.id);
                    let rollback = model::Rollback {
                        block_id: rollback_to_block_id,
                    };
                    Ok(model::BlockchainUpdate::Rollback(rollback))
                }
                _ => Err(ConvertError("failed to parse blockchain update")),
            }
        }

        fn extract_is_microblock(body: &proto::Body) -> Option<bool> {
            match body {
                proto::Body::Block(proto::BlockAppend { block: Some(_), .. }) => Some(false),
                proto::Body::MicroBlock(proto::MicroBlockAppend {
                    micro_block: Some(_),
                    ..
                }) => Some(true),
                _ => None,
            }
        }

        fn extract_id<'a>(body: &'a proto::Body, block_id: &'a Vec<u8>) -> Option<&'a Vec<u8>> {
            match body {
                proto::Body::Block(_) => Some(block_id),
                proto::Body::MicroBlock(proto::MicroBlockAppend {
                    micro_block: Some(proto::SignedMicroBlock { total_block_id, .. }),
                    ..
                }) => Some(total_block_id),
                _ => None,
            }
        }

        fn extract_timestamp(body: &proto::Body) -> Option<model::Timestamp> {
            if let proto::Body::Block(proto::BlockAppend {
                block:
                    Some(proto::Block {
                        header: Some(ref header),
                        ..
                    }),
                ..
            }) = body
            {
                let ts = header.timestamp;
                let ts = model::Timestamp::from_unix_timestamp_millis(ts);
                Some(ts)
            } else {
                None
            }
        }

        fn current_timestamp() -> model::Timestamp {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now();
            // Panics if server is placed inside a time machine
            assert!(now > UNIX_EPOCH, "Current time is before the Unix Epoch");
            // Call to `unwrap()` is safe here because we checked time with the assert
            let ts = now.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
            model::Timestamp::from_unix_timestamp_millis(ts)
        }

        fn extract_transactions(body: proto::Body) -> Option<Vec<proto::SignedTransaction>> {
            match body {
                proto::Body::Block(proto::BlockAppend {
                    block: Some(proto::Block { transactions, .. }),
                    ..
                }) => Some(transactions),
                proto::Body::MicroBlock(proto::MicroBlockAppend {
                    micro_block:
                        Some(proto::SignedMicroBlock {
                            micro_block: Some(proto::MicroBlock { transactions, .. }),
                            ..
                        }),
                    ..
                }) => Some(transactions),
                _ => None,
            }
        }

        fn convert_transactions(
            transaction_ids: Vec<Vec<u8>>,
            transactions: Vec<proto::SignedTransaction>,
            transactions_metadata: Vec<proto::TransactionMetadata>,
            height: u32,
        ) -> Result<Vec<model::Transaction>, ConvertError> {
            let ids = transaction_ids.into_iter();
            let txs = transactions.into_iter();
            let met = transactions_metadata.into_iter();
            let iter = ids.zip(txs).zip(met);
            iter.filter_map(|((id, tx), meta)| convert_tx(id, tx, meta, height).transpose())
                .collect()
        }

        fn convert_tx(
            id: Vec<u8>,
            tx: proto::SignedTransaction,
            meta: proto::TransactionMetadata,
            height: u32,
        ) -> Result<Option<model::Transaction>, ConvertError> {
            let maybe_tx = {
                if is_exchange_transaction(&meta) {
                    let tx = extract_transaction(&tx).ok_or(ConvertError("missing tx"))?;
                    let (data, _meta) = extract_exchange_tx(tx, &meta)?;
                    let asset_pair = data.orders[0]
                        .asset_pair
                        .as_ref()
                        .ok_or(ConvertError("missing asset_pair"))?;
                    let tx = model::Transaction {
                        id: base58(&id),
                        height,
                        timestamp: tx.timestamp as u64,
                        sender: convert_address(&meta.sender_address),
                        exchange_tx: model::TxExchange {
                            amount_asset: convert_asset_id(&asset_pair.amount_asset_id),
                            price_asset: convert_asset_id(&asset_pair.price_asset_id),
                            amount: data.amount as u64,
                            price: data.price as u64,
                        },
                    };
                    Some(tx)
                } else {
                    None
                }
            };

            Ok(maybe_tx)
        }

        fn is_exchange_transaction(meta: &proto::TransactionMetadata) -> bool {
            matches!(meta.metadata, Some(proto::Metadata::Exchange(_)))
        }

        fn extract_transaction(tx: &proto::SignedTransaction) -> Option<&proto::Transaction> {
            match &tx.transaction {
                Some(proto::signed_transaction::Transaction::WavesTransaction(tx)) => Some(tx),
                _ => None,
            }
        }

        fn extract_exchange_tx<'a>(
            tx: &'a proto::Transaction,
            meta: &'a proto::TransactionMetadata,
        ) -> Result<
            (
                &'a proto::ExchangeTransactionData,
                &'a proto::ExchangeMetadata,
            ),
            ConvertError,
        > {
            let data = match tx {
                proto::Transaction {
                    data: Some(proto::Data::Exchange(data)),
                    ..
                } => data,
                _ => {
                    return Err(ConvertError(
                        "unexpected transaction contents - want Exchange",
                    ))
                }
            };

            let meta = match &meta.metadata {
                Some(proto::Metadata::Exchange(meta)) => meta,
                _ => return Err(ConvertError("unexpected metadata contents - want Exchange")),
            };

            Ok((data, meta))
        }

        fn convert_address(address: &Vec<u8>) -> model::Address {
            // Strangely, Address doesn't have `from_bytes` constructor
            model::Address::from_string(&base58(address)).expect("base58 conversion broken")
        }

        fn convert_asset_id(asset_id: &Vec<u8>) -> model::Asset {
            if asset_id.is_empty() {
                model::Asset::Waves
            } else {
                model::Asset::IssuedAsset(model::AssetId::from_bytes(asset_id.clone()))
            }
        }

        fn base58(bytes: &[u8]) -> String {
            bs58::encode(bytes).into_string()
        }
    }
}
