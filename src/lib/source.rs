//! Blockchain updates

#![allow(unused_imports, unreachable_code, unused_variables)] //TODO cleanup

pub mod prices {
    use tokio::sync::{mpsc, oneshot};

    use super::blockchain_updates::BlockchainUpdatesClient;
    use crate::{
        processing::EventWithFeedback,
        stream::{Event, PriceOHLC, RawPrice},
    };

    pub async fn start(
        blockchain_updates_url: String,
        starting_height: u32,
        sink: mpsc::Sender<EventWithFeedback>,
    ) -> Result<(), anyhow::Error> {
        let client = BlockchainUpdatesClient::connect(blockchain_updates_url).await?;
        let stream = client.stream(starting_height).await?;
        todo!("prices stream impl")
    }
}

//TODO proper logging
mod blockchain_updates {
    use tokio::{
        sync::{mpsc, oneshot},
        task,
    };

    use waves_protobuf_schemas::waves::events::grpc::{
        blockchain_updates_api_client::BlockchainUpdatesApiClient, SubscribeEvent, SubscribeRequest,
    };

    #[derive(Debug)]
    pub(super) enum BlockchainUpdate {
        Append(AppendBlock),
        Rollback(Rollback),
    }

    #[derive(Debug)]
    pub(super) struct AppendBlock {
        pub block_id: String,
        pub height: u32,
        pub timestamp: Option<u64>,
        pub is_microblock: bool,
        //pub transactions: Vec<Transaction>,
    }

    #[derive(Debug)]
    pub(super) struct Rollback {
        pub block_id: String,
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
                    println!("ERROR | Error receiving blockchain updates: {}", err);
                } else {
                    println!("WARN  | GRPC connection closed by the server");
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
        use itertools::Itertools;
        use thiserror::Error;

        use waves_protobuf_schemas::waves::events::BlockchainUpdated;

        use super::{AppendBlock, BlockchainUpdate, Rollback};

        #[derive(Error, Debug)]
        #[error("failed to convert blockchain update: {0}")]
        pub(super) struct ConvertError(&'static str);

        pub(super) fn convert_update(
            src: BlockchainUpdated,
        ) -> Result<BlockchainUpdate, ConvertError> {
            todo!("convert_update")
        }
    }
}
