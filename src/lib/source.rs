//! Blockchain updates

pub mod prices {
    use std::collections::HashMap;

    use tokio::sync::{mpsc, oneshot};

    use super::{
        blockchain_updates::{AppendBlock, BlockchainUpdate, BlockchainUpdatesClient},
        data_service::load_pairs,
    };
    use crate::{
        asset,
        model::{Address, AssetPair},
        processing::EventWithFeedback,
        stream::{Event, PriceLowHigh, PriceWithDecimals},
    };

    pub struct Source {
        matcher_address: Address,
        last_prices: HashMap<AssetPair, PriceWithDecimals>,
    }

    impl Source {
        pub fn new(matcher_address: Address) -> Self {
            Source {
                matcher_address,
                last_prices: HashMap::new(),
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
                self.last_prices.insert(pair.pair, pair.last_price);
            }
            Ok(())
        }

        pub async fn start(
            &mut self,
            blockchain_updates_url: String,
            starting_height: u32,
            sink: mpsc::Sender<EventWithFeedback>,
        ) -> Result<(), anyhow::Error> {
            log::debug!("Connecting to blockchain-updates: {}", blockchain_updates_url);
            let client = BlockchainUpdatesClient::connect(blockchain_updates_url).await?;
            log::debug!("Starting receiving blockchain updates from height {}", starting_height);
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
            let block_prices = self.aggregate_prices_from_block(block);
            self.send_price_events(block_prices, sink).await
        }

        fn aggregate_prices_from_block(
            &mut self,
            block: AppendBlock,
        ) -> HashMap<AssetPair, PriceLowHigh> {
            let mut block_prices = HashMap::<AssetPair, PriceLowHigh>::new();

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
                    if let Some(prev_price) = self.last_prices.get(&asset_pair) {
                        block_prices
                            .entry(asset_pair.clone())
                            .and_modify(|price| *price = price.clone().merge(new_price))
                            .or_insert_with(|| prev_price.merge(new_price));
                    }
                    self.last_prices.insert(asset_pair, new_price);
                }
            }

            block_prices
        }

        async fn send_price_events(
            &self,
            block_prices: HashMap<AssetPair, PriceLowHigh>,
            sink: &mpsc::Sender<EventWithFeedback>,
        ) -> Result<(), Error> {
            for (asset_pair, price_range) in block_prices {
                if !price_range.is_empty() {
                    let event = Event::PriceChanged {
                        asset_pair,
                        price_range,
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
            }
            Ok(())
        }
    }

    enum Error {
        StopProcessing,
        EventProcessingFailed(crate::error::Error),
    }
}

mod data_service {
    use crate::{
        asset,
        model::{Asset, AssetPair},
        stream::PriceWithDecimals,
    };
    use anyhow::ensure;
    use wavesexchange_apis::{
        bigdecimal::ToPrimitive,
        data_service::{dto, DataService},
        HttpClient,
    };

    pub struct Pair {
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
}

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
        model::{Address, Asset},
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
        pub block_id: String,
        pub height: u32,
        pub is_microblock: bool,
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
        pub timestamp: u64,
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
            pub(super) use crate::model::{Address, Asset, AssetId};
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
