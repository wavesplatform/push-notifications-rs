//! Interaction with the Blockchain-updates

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
