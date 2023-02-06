//! Source of Order events

use bigdecimal::BigDecimal;
use tokio::sync::{mpsc, oneshot};

use crate::{
    model::{Address, Asset, AssetPair, Timestamp},
    processing::EventWithFeedback,
    stream::{Event, OrderExecution, OrderSide, OrderType},
};

use self::redis_stream::{HandleError, RedisStreamReader};

pub use self::redis_stream::{RedisConnectionConfig, RedisStreamConfig};

/// Config for the Order Execution events stream
pub struct SourceConfig {
    pub connection: RedisConnectionConfig,
    pub stream: RedisStreamConfig,
    pub batch_max_size: u32,
}

/// Source of Order Execution events (based on the Redis feed)
pub struct Source {
    reader: RedisStreamReader,
}

impl Source {
    pub async fn new(config: SourceConfig) -> anyhow::Result<Self> {
        let reader =
            RedisStreamReader::new(config.connection, config.stream, config.batch_max_size).await?;
        let source = Source { reader };
        Ok(source)
    }

    pub async fn run(self, sink: mpsc::Sender<EventWithFeedback>) -> anyhow::Result<()> {
        let process_fn = |message: Vec<u8>| {
            let sink = sink.clone();
            async move {
                let (orders, timestamp) =
                    json::parse_orders(&message).map_err(|e| HandleError::Error(e.into()))?;
                log::debug!("Got {} order updates @ {:?}", orders.len(), timestamp);
                Self::send_order_events(orders, &sink).await
            }
        };
        self.reader.run(process_fn).await
    }

    async fn send_order_events(
        orders: Vec<json::OrderUpdate>,
        sink: &mpsc::Sender<EventWithFeedback>,
    ) -> Result<(), HandleError> {
        for order in orders {
            if let Some(event) = Self::event_from_order_update(order) {
                log::trace!("Sending order event: {:?}", event);
                let (tx, rx) = oneshot::channel();
                let evf = EventWithFeedback {
                    event,
                    result_tx: tx,
                };
                sink.send(evf).await.map_err(|_| HandleError::Terminate)?;
                let result = rx.await.map_err(|_| HandleError::Terminate)?;
                result.map_err(|err| HandleError::Error(err.into()))?;
            }
        }
        Ok(())
    }

    fn event_from_order_update(order: json::OrderUpdate) -> Option<Event> {
        use bigdecimal::ToPrimitive;
        let event = Event::OrderExecuted {
            order_type: match order.order_type {
                json::OrderType::Limit => OrderType::Limit,
                json::OrderType::Market => OrderType::Market,
            },
            side: match order.side {
                json::OrderSide::Buy => OrderSide::Buy,
                json::OrderSide::Sell => OrderSide::Sell,
            },
            asset_pair: AssetPair {
                amount_asset: Asset::from_id(&order.amount_asset).expect("amount asset"),
                price_asset: Asset::from_id(&order.price_asset).expect("price asset"),
            },
            execution: match order.status {
                json::OrderStatus::Filled => OrderExecution::Full,
                json::OrderStatus::PartiallyFilled => OrderExecution::Partial {
                    percentage: {
                        let filled = order.filled_amount_accumulated;
                        let total = order.amount;
                        let ratio = BigDecimal::from(100) * filled / total;
                        ratio.to_f64().expect("percentage")
                    },
                },
                json::OrderStatus::Cancelled => return None,
            },
            address: Address::from_string(&order.owner_address).expect("order owner address"),
            timestamp: Timestamp::from_unix_timestamp_millis(order.event_timestamp),
        };
        Some(event)
    }
}

mod redis_stream {
    use std::{fmt, future::Future, time::Duration};

    use redis::{
        streams::{
            StreamInfoConsumersReply, StreamInfoGroupsReply, StreamInfoStreamReply,
            StreamReadOptions, StreamReadReply,
        },
        AsyncCommands, Value,
    };

    #[derive(Clone)]
    pub struct RedisConnectionConfig {
        pub hostname: String,
        pub port: u16,
        pub user: String,
        pub password: String,
    }

    #[derive(Clone)]
    pub struct RedisStreamConfig {
        pub stream_name: String,
        pub group_name: String,
        pub consumer_name: String,
    }

    impl RedisConnectionConfig {
        pub const DATABASE_ID: u32 = 0;

        pub fn connection_url(&self) -> String {
            format!(
                "redis://{}:{}@{}:{}/{}",
                self.user,
                self.password,
                self.hostname,
                self.port,
                Self::DATABASE_ID
            )
        }
    }

    impl fmt::Debug for RedisConnectionConfig {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // Intentionally avoid printing password for security reasons
            write!(
                f,
                "Redis(server={}:{}; database={}; user={}; password=***)",
                self.hostname,
                self.port,
                Self::DATABASE_ID,
                self.user
            )
        }
    }

    pub(super) struct RedisStreamReader {
        conn: redis::aio::Connection,
        stream: RedisStreamConfig,
        batch_max_size: u32,
    }

    pub(super) enum HandleError {
        Terminate,
        Error(anyhow::Error),
    }

    impl RedisStreamReader {
        pub(super) async fn new(
            conn: RedisConnectionConfig,
            stream: RedisStreamConfig,
            batch_max_size: u32,
        ) -> anyhow::Result<Self> {
            log::info!("Connecting to {:?}", conn);
            let redis_client = redis::Client::open(conn.connection_url())?;
            let mut redis_conn = redis_client.get_async_connection().await?;
            log::info!("Redis connected.");
            prepare(&mut redis_conn, stream.clone()).await?;
            let reader = RedisStreamReader {
                conn: redis_conn,
                stream,
                batch_max_size,
            };
            Ok(reader)
        }

        pub(super) async fn run<F, R>(self, process_fn: F) -> anyhow::Result<()>
        where
            F: FnMut(Vec<u8>) -> R,
            R: Future<Output = Result<(), HandleError>>,
        {
            let res = run(self.conn, self.stream, self.batch_max_size, process_fn).await;
            match res {
                Ok(()) => log::debug!("Redis stream reading loop exited normally"),
                Err(ref err) => {
                    log::error!("Redis stream reading loop exited with error: {}", err)
                }
            }
            res
        }
    }

    const BEGIN_OF_STREAM: &str = "0-0";
    const NEW_MESSAGES: &str = ">";

    async fn prepare(
        con: &mut redis::aio::Connection,
        stream: RedisStreamConfig,
    ) -> anyhow::Result<()> {
        let RedisStreamConfig {
            stream_name,
            group_name,
            consumer_name,
        } = stream;

        // Probe whether the configured Redis stream exists
        log::info!("Querying Redis stream '{}'...", stream_name);
        let reply = con.xinfo_stream(&stream_name).await;
        if reply.is_err() {
            log::error!("Stream not found: '{}'\nPlease create the corresponding stream in Redis and rerun this service.", stream_name);
        }
        log::info!("Stream info: {}", stream_info(reply?));

        // Probe whether the configured Redis consumer group exists, create if necessary
        log::info!("Checking Redis consumer group '{}'...", group_name);
        let reply = con.xinfo_groups(&stream_name).await?;
        if let Some(group_info) = group_info(reply, &group_name) {
            log::info!("Group info: {}", group_info);
        } else {
            log::warn!("Group not found: '{}' - creating", group_name);
            con.xgroup_create(&stream_name, &group_name, BEGIN_OF_STREAM)
                .await?;
            log::info!("Group successfully created: '{}'", group_name);
            let reply = con.xinfo_groups(&stream_name).await?;
            if let Some(group_info) = group_info(reply, &group_name) {
                log::info!("Group info: {}", group_info);
            } else {
                log::warn!("Failed to find the group just created: '{}'", group_name);
            }
        }

        // Query info about the configured Redis stream consumer
        log::info!("Querying Redis stream consumer '{}'...", consumer_name);
        let reply = con.xinfo_consumers(&stream_name, &group_name).await?;
        log::info!("Consumer info: {}", consumer_info(reply, &consumer_name));

        Ok(())
    }

    async fn run<F, R>(
        mut con: redis::aio::Connection,
        stream: RedisStreamConfig,
        batch_max_size: u32,
        mut process_fn: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(Vec<u8>) -> R,
        R: Future<Output = Result<(), HandleError>>,
    {
        let con = &mut con;

        let RedisStreamConfig {
            stream_name,
            group_name,
            consumer_name,
        } = stream;

        log::info!(
            "Starting reading Redis stream `{}` using group `{}` as client `{}`",
            stream_name,
            group_name,
            consumer_name,
        );

        log::debug!("Re-fetching pending messages (not acknowledged since last run)");
        let mut fetching_backlog = true;
        let mut from_id = BEGIN_OF_STREAM.to_string();

        // Without this timeout Redis would not block at all
        // if there are no new messages in the stream,
        // returning empty reply instead, making our loop too busy.
        const MAX_BLOCK_TIME: Duration = Duration::from_secs(6);

        let read_options = StreamReadOptions::default()
            .group(&group_name, &consumer_name)
            .count(batch_max_size as usize)
            .block(MAX_BLOCK_TIME.as_millis() as usize);

        loop {
            log::trace!(
                "Reading up to {} messages starting from '{}'",
                batch_max_size,
                from_id,
            );

            let reply = loop {
                let reply: StreamReadReply = con
                    .xread_options(&[&stream_name], &[&from_id], &read_options)
                    .await?;

                if !reply.keys.is_empty() {
                    break reply;
                }
            };

            // We expect exactly 1 key in the reply, as requested
            let ids = {
                assert_eq!(reply.keys.len(), 1, "Redis misbehaves: {reply:?}");
                let key = reply.keys.into_iter().next().unwrap(); // Unwrap is safe due to assert above
                assert_eq!(key.key, stream_name, "Redis misbehaves: {key:?}");
                key.ids
            };

            if fetching_backlog && ids.is_empty() {
                log::debug!(
                    "Finished fetching pending messages. Starting to receive new messages."
                );
                fetching_backlog = false;
                from_id = NEW_MESSAGES.to_string();
                continue;
            }

            log::trace!("Got {} messages from the stream", ids.len());

            let messages = ids
                .into_iter()
                .map(|item| {
                    let id = item.id;
                    if item.map.len() == 1 {
                        let (key, value) = item.map.into_iter().next().unwrap(); // Unwrap is safe due to length check
                        if key == "event" {
                            match value {
                                Value::Data(bytes) => Ok((id, bytes)),
                                _ => Err(anyhow::anyhow!(
                                    "Item {} has unsupported data format: {:?}",
                                    id,
                                    value,
                                )),
                            }
                        } else {
                            Err(anyhow::anyhow!("Item {} has unrecognized key: {}", id, key))
                        }
                    } else {
                        Err(anyhow::anyhow!(
                            "Item {} has more than one key/value pairs: {}",
                            id,
                            item.map.len(),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;

            for (id, message) in messages {
                log::trace!("Got message '{}' of {} bytes", id, message.len());

                let result = process_fn(message).await;
                match result {
                    Ok(()) => {}
                    Err(HandleError::Terminate) => break,
                    Err(HandleError::Error(err)) => {
                        log::error!("Event processing failed: {}", err);
                        return Err(err.into());
                    }
                }

                con.xack(&stream_name, &group_name, &[&id]).await?;

                con.xdel(&stream_name, &[&id]).await?;

                if fetching_backlog {
                    from_id = id;
                }
            }
        }
    }

    fn stream_info(info: StreamInfoStreamReply) -> String {
        let stream_length = info.length;
        let first_id = info.first_entry.id;
        let last_id = info.last_entry.id;
        let last_generated_id = info.last_generated_id;
        let num_groups = info.groups;

        format!("{stream_length} messages [{first_id} .. {last_id}]; last_generated_id = {last_generated_id}; stream has {num_groups} consumer groups")
    }

    fn group_info(info: StreamInfoGroupsReply, group_name: &str) -> Option<String> {
        info.groups
            .into_iter()
            .find(|g| g.name == group_name)
            .map(|g| {
                format!(
                    "{} consumers; {} pending messages; last delivered id = '{}'",
                    g.consumers, g.pending, g.last_delivered_id
                )
            })
    }

    fn consumer_info(info: StreamInfoConsumersReply, consumer_name: &str) -> String {
        info.consumers
            .into_iter()
            .find(|c| c.name == consumer_name)
            .map(|c| {
                let num_pending = c.pending;
                let idle_time = Duration::from_millis(c.idle as u64);
                format!("{num_pending} pending messages; idle time is {idle_time:?}")
            })
            .unwrap_or_else(|| format!("consumer '{consumer_name}' is not known yet"))
    }
}

mod json {
    use bigdecimal::BigDecimal;
    use serde::Deserialize;

    use crate::model::Timestamp;

    pub(super) fn parse_orders(json: &[u8]) -> serde_json::Result<(Vec<OrderUpdate>, Timestamp)> {
        let envelope = serde_json::from_slice::<Envelope>(json)?;
        let timestamp = Timestamp::from_unix_timestamp_millis(envelope.timestamp);
        if envelope.msg_type == MessageType::OrdersUpdated {
            Ok((envelope.data, timestamp))
        } else {
            log::warn!("Unsupported orders envelope: {:?}", envelope);
            Ok((Vec::new(), timestamp))
        }
    }

    #[derive(Deserialize, Debug, Clone)]
    struct Envelope {
        /// The type of the message: 'osu'.
        #[serde(rename = "T")]
        msg_type: MessageType,

        /// Unix timestamp of this message in milliseconds. It is a Matcher's timestamp.
        /// This parameter is internal to the Matcher and is not recommended to use.
        #[serde(rename = "_")]
        timestamp: i64,

        /// Updated orders
        #[serde(rename = "o")] // o = [o]rders
        data: Vec<OrderUpdate>,
    }

    #[non_exhaustive]
    // The Redis feed only supports "osu" variant as ow now, but other feeds (websockets) supports more.
    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    enum MessageType {
        #[serde(rename = "osu")] // osu = [o]rder[s] [u]pdated
        OrdersUpdated,
    }

    #[allow(dead_code)] // Some fields are never read, but keep them for completeness
    #[derive(Deserialize, Debug, Clone)]
    pub(super) struct OrderUpdate {
        /// The order's id
        #[serde(rename = "i")]
        pub(super) order_id: String,

        /// The address of order's owner
        #[serde(rename = "o")]
        pub(super) owner_address: String,

        /// The specified order timestamp
        #[serde(rename = "t")]
        pub(super) order_timestamp: i64,

        /// The amount asset
        #[serde(rename = "A")]
        pub(super) amount_asset: String,

        /// The price asset
        #[serde(rename = "P")]
        pub(super) price_asset: String,

        /// The order side: BUY | SELL | buy | sell.
        /// Lowercase variants will be in the future versions (2.2.0+). Please support all variants!
        #[serde(rename = "S")]
        pub(super) side: OrderSide, // buy | sell

        /// The order type: LIMIT | MARKET | limit | market.
        /// Lowercase variants will be in the future versions (2.2.0+). Please support all variants!
        #[serde(rename = "T")]
        pub(super) order_type: OrderType, // limit | market

        /// The specified order's price
        #[serde(rename = "p")]
        pub(super) price: BigDecimal,

        /// The total order's amount
        #[serde(rename = "a")]
        pub(super) amount: BigDecimal,

        /// The specified order's fee
        #[serde(rename = "f")]
        pub(super) fee: BigDecimal,

        /// The fee asset
        #[serde(rename = "F")]
        pub(super) fee_asset: String,

        /// The order status: FILLED | Filled | PARTIALLY_FILLED | PartiallyFilled | CANCELLED | Cancelled.
        /// Uppercase variants will be removed in the future versions (2.2.0+). Please support all variants!
        #[serde(rename = "s")]
        pub(super) status: OrderStatus, // Filled | PartiallyFilled | Cancelled

        /// The current filled amount, including this and all previous matches
        #[serde(rename = "q")]
        pub(super) filled_amount_accumulated: BigDecimal,

        /// The current filled fee, including this and all previous matches
        #[serde(rename = "Q")]
        pub(super) filled_fee_accumulated: BigDecimal,

        /// The average filled price among all trades with maximum priceDecimals digits after point.
        /// Can be computed as `average weighed price = E / q`.
        /// This field will be removed in version 2.2.3
        #[serde(rename = "r")]
        pub(super) avg_filled_price: Option<BigDecimal>,

        /// The update event timestamp. It is a Matcher's (almost) timestamp
        #[serde(rename = "Z")]
        pub(super) event_timestamp: i64,

        /// The executed amount during this match, if it is Filled or PartiallyFilled event
        #[serde(rename = "c")]
        pub(super) executed_amount: Option<BigDecimal>,

        /// The executed fee during this match, if it is Filled or PartiallyFilled event
        #[serde(rename = "h")]
        pub(super) executed_fee: Option<BigDecimal>,

        /// The execution price, if it is Filled or PartiallyFilled event
        #[serde(rename = "e")]
        pub(super) executed_price: Option<BigDecimal>,

        /// Total executed price assets. Will be available in version 2.2.3
        #[serde(rename = "E")]
        pub(super) total_executed_price_assets: Option<BigDecimal>,
    }

    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum OrderSide {
        #[serde(rename = "buy")]
        Buy,

        #[serde(rename = "sell")]
        Sell,
    }

    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum OrderType {
        #[serde(rename = "limit")]
        Limit,

        #[serde(rename = "market")]
        Market,
    }

    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum OrderStatus {
        #[serde(rename = "Filled")]
        Filled,

        #[serde(rename = "PartiallyFilled")]
        PartiallyFilled,

        #[serde(rename = "Cancelled")]
        Cancelled,
    }

    #[rustfmt::skip]
    #[test]
    fn test_orders_deserialize() -> anyhow::Result<()> {
        use serde_json::json;
        use std::str::FromStr;

        let big = |x: f64| -> BigDecimal { BigDecimal::from_str(x.to_string().as_str()).unwrap() };

        let check_avg_price = |order: &OrderUpdate| {
            if let (Some(avg_filled_price), Some(total_executed_price_assets)) = (&order.avg_filled_price, &order.total_executed_price_assets) {
                let avg_price = total_executed_price_assets / &order.filled_amount_accumulated;
                assert_eq!(avg_price, *avg_filled_price, "order fields disagree");
            }
        };

        let order_cancel = json!(
            {
              "T" : "osu",
              "_" : 1673428863604_i64,
              "o" : [ {
                "i" : "JX4G8f5ehPyUPfH12DRevvjCGSP7LaRcy9ToddLdqKL",
                "o" : "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq",
                "t" : 1673428862971_i64,
                "A" : "WAVES",
                "P" : "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8",
                "S" : "sell",
                "T" : "limit",
                "p" : "5.0",
                "a" : "1.0",
                "f" : "0.003",
                "F" : "WAVES",
                "s" : "Cancelled",
                "q" : "0.0",
                "Q" : "0.0",
                "r" : "0.0",
                "Z" : 1673428862976_i64
              } ]
            }
        );
        let envelope = serde_json::from_value::<Envelope>(order_cancel)?;
        assert_eq!(envelope.msg_type, MessageType::OrdersUpdated);
        assert_eq!(envelope.timestamp, 1673428863604);
        assert_eq!(envelope.data.len(), 1);

        let order = &envelope.data[0];
        {
            assert_eq!(order.order_id, "JX4G8f5ehPyUPfH12DRevvjCGSP7LaRcy9ToddLdqKL");
            assert_eq!(order.owner_address, "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq");
            assert_eq!(order.order_timestamp, 1673428862971);
            assert_eq!(order.amount_asset, "WAVES");
            assert_eq!(order.price_asset, "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8");
            assert_eq!(order.side, OrderSide::Sell);
            assert_eq!(order.order_type, OrderType::Limit);
            assert_eq!(order.price, big(5.0));
            assert_eq!(order.amount, big(1.0));
            assert_eq!(order.fee, big(0.003));
            assert_eq!(order.fee_asset, "WAVES");
            assert_eq!(order.status, OrderStatus::Cancelled);
            assert_eq!(order.filled_amount_accumulated, big(0.0));
            assert_eq!(order.filled_fee_accumulated, big(0.0));
            assert_eq!(order.avg_filled_price, Some(big(0.0)));
            assert_eq!(order.event_timestamp, 1673428862976);
            assert_eq!(order.executed_amount, None);
            assert_eq!(order.executed_fee, None);
            assert_eq!(order.executed_price, None);
            assert_eq!(order.total_executed_price_assets, None);
            check_avg_price(order);
        }

        let order_fill = json!(
            {
              "T" : "osu",
              "_" : 1673428865504_i64,
              "o" : [ {
                "i" : "DbGrYjRnRazkajgYHpekfB72EHBmmQjVPrgpLSJb3MTq",
                "o" : "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq",
                "t" : 1673428865872_i64,
                "A" : "WAVES",
                "P" : "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8",
                "S" : "buy",
                "T" : "limit",
                "p" : "5.0",
                "a" : "1.0",
                "f" : "0.003",
                "F" : "WAVES",
                "s" : "Filled",
                "q" : "1.0",
                "Q" : "0.003",
                "r" : "5.0",
                "Z" : 1673428865504_i64,
                "c" : "1.0",
                "h" : "0.003",
                "e" : "5.0",
                "E" : "5.0"
              }, {
                "i" : "GR6WbwBxs6q8MXqLaz8a53epGKuqxBaM8fF9RDD5NiLW",
                "o" : "3Q6ujVDbX57oLsXxifqfTcycgb4S8U3DLFz",
                "t" : 1673428865875_i64,
                "A" : "WAVES",
                "P" : "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8",
                "S" : "sell",
                "T" : "limit",
                "p" : "5.0",
                "a" : "5.0",
                "f" : "0.003",
                "F" : "WAVES",
                "s" : "PartiallyFilled",
                "q" : "1.0",
                "Q" : "6.0E-4",
                "r" : "5.0",
                "Z" : 1673428865504_i64,
                "c" : "1.0",
                "h" : "6.0E-4",
                "e" : "5.0",
                "E" : "5.0"
              } ]
            }
        );
        let envelope = serde_json::from_value::<Envelope>(order_fill)?;
        assert_eq!(envelope.msg_type, MessageType::OrdersUpdated);
        assert_eq!(envelope.timestamp, 1673428865504);
        assert_eq!(envelope.data.len(), 2);

        let order = &envelope.data[0];
        {
            assert_eq!(order.order_id, "DbGrYjRnRazkajgYHpekfB72EHBmmQjVPrgpLSJb3MTq");
            assert_eq!(order.owner_address, "3Q6pToUA28zJbMJUfB5xoGgfqqni11H7NPq");
            assert_eq!(order.order_timestamp, 1673428865872);
            assert_eq!(order.amount_asset, "WAVES");
            assert_eq!(order.price_asset, "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8");
            assert_eq!(order.side, OrderSide::Buy);
            assert_eq!(order.order_type, OrderType::Limit);
            assert_eq!(order.price, big(5.0));
            assert_eq!(order.amount, big(1.0));
            assert_eq!(order.fee, big(0.003));
            assert_eq!(order.fee_asset, "WAVES");
            assert_eq!(order.status, OrderStatus::Filled);
            assert_eq!(order.filled_amount_accumulated, big(1.0));
            assert_eq!(order.filled_fee_accumulated, big(0.003));
            assert_eq!(order.avg_filled_price, Some(big(5.0)));
            assert_eq!(order.event_timestamp, 1673428865504);
            assert_eq!(order.executed_amount, Some(big(1.0)));
            assert_eq!(order.executed_fee, Some(big(0.003)));
            assert_eq!(order.executed_price, Some(big(5.0)));
            assert_eq!(order.total_executed_price_assets, Some(big(5.0)));
            check_avg_price(order);
        }

        let order = &envelope.data[1];
        {
            assert_eq!(order.order_id, "GR6WbwBxs6q8MXqLaz8a53epGKuqxBaM8fF9RDD5NiLW");
            assert_eq!(order.owner_address, "3Q6ujVDbX57oLsXxifqfTcycgb4S8U3DLFz");
            assert_eq!(order.order_timestamp, 1673428865875);
            assert_eq!(order.amount_asset, "WAVES");
            assert_eq!(order.price_asset, "GwT5y18jcrrppAuj5VkfnHLG8WRf3TNzmhREQkY4pzd8");
            assert_eq!(order.side, OrderSide::Sell);
            assert_eq!(order.order_type, OrderType::Limit);
            assert_eq!(order.price, big(5.0));
            assert_eq!(order.amount, big(5.0));
            assert_eq!(order.fee, big(0.003));
            assert_eq!(order.fee_asset, "WAVES");
            assert_eq!(order.status, OrderStatus::PartiallyFilled);
            assert_eq!(order.filled_amount_accumulated, big(1.0));
            assert_eq!(order.filled_fee_accumulated, big(0.0006));
            assert_eq!(order.avg_filled_price, Some(big(5.0)));
            assert_eq!(order.event_timestamp, 1673428865504);
            assert_eq!(order.executed_amount, Some(big(1.0)));
            assert_eq!(order.executed_fee, Some(big(0.0006)));
            assert_eq!(order.executed_price, Some(big(5.0)));
            assert_eq!(order.total_executed_price_assets, Some(big(5.0)));
            check_avg_price(order);
        }

        Ok(())
    }
}
