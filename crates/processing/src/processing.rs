use crate::{asset, localization, error::Error};
use database::{device, message, subscription};
use diesel_async::{AsyncConnection, AsyncPgConnection};
use model::{
    asset::Asset,
    device::{Device, LocaleInfo},
    event::Event,
    message::{LocalizedMessage, Message, MessageData, PreparedMessage},
    order::OrderExecution,
    topic::{SubscriptionMode, Topic},
    waves::AsBase58String,
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use diesel_async::scoped_futures::ScopedFutureExt as _;

pub struct EventWithFeedback {
    pub event: Event,
    pub result_tx: oneshot::Sender<Result<(), Error>>,
}

pub struct MessagePump {
    subscriptions: subscription::Repo,
    assets: asset::RemoteGateway,
    devices: device::Repo,
    localizer: localization::Repo,
    messages: message::Queue,
}

impl MessagePump {
    pub fn new(
        subscriptions: subscription::Repo,
        assets: asset::RemoteGateway,
        devices: device::Repo,
        localizer: localization::Repo,
        messages: message::Queue,
    ) -> Self {
        MessagePump {
            subscriptions,
            assets,
            devices,
            localizer,
            messages,
        }
    }

    pub async fn run_event_loop(
        self: Arc<Self>,
        mut events: mpsc::Receiver<EventWithFeedback>,
        mut conn: AsyncPgConnection,
    ) {
        log::debug!("Starting event processing loop");
        while let Some(event) = events.recv().await {
            let EventWithFeedback { event, result_tx } = event;
            let this = self.clone();
            let res = conn
                .transaction(|conn| {
                    async move {
                        // Asynchronously process this event within a database transaction
                        this.process_event(event, conn).await
                    }
                    .scope_boxed()
                })
                .await;
            result_tx.send(res).expect("ack");
        }
    }

    async fn process_event(&self, event: Event, conn: &mut AsyncPgConnection) -> Result<(), Error> {
        let subscriptions = self.subscriptions.matching(&event, conn).await?;
        if subscriptions.is_empty() {
            log::trace!("Event with no matching subscriptions: {:?}", event);
        } else {
            let n = subscriptions.len();
            log::debug!("Event with {} matching subscriptions: {:?}", n, event);
        }
        for subscription in subscriptions {
            log::debug!("  Subscription: {:?}", subscription);
            let is_oneshot = subscription.mode == SubscriptionMode::Once;
            let msg = self.make_message(&event, &subscription.topic).await?;
            let address = &subscription.subscriber;
            let devices = self.devices.subscribers(address, conn).await?;
            if devices.is_empty() {
                log::warn!(
                    "Subscriber {} has no registered devices - unable to send any messages to him",
                    address.as_base58_string(),
                );
            }
            for device in devices {
                log::debug!("    Device: {:?}", device);
                let message = self.localize(&msg, &device.locale);
                let meta = Self::make_metadata(&event, &device);
                let prepared_message = PreparedMessage {
                    device,
                    message,
                    data: Some(meta),
                    collapse_key: None,
                };
                log::debug!("      Message prepared: {:?}", prepared_message);
                self.messages.enqueue(prepared_message, conn).await?;
            }
            if is_oneshot {
                log::debug!(
                    "Removing completed one-shot subscription: {:?}",
                    subscription
                );
                self.subscriptions
                    .complete_oneshot(subscription, conn)
                    .await?;
            }
        }
        Ok(())
    }

    async fn make_message(&self, event: &Event, topic: &Topic) -> Result<Message, Error> {
        let res = match (event, topic) {
            (
                Event::OrderExecuted {
                    order_type,
                    side,
                    asset_pair: event_assets,
                    execution,
                    address: _,
                    timestamp,
                },
                Topic::OrderFulfilled,
            ) => {
                let (amount_asset, price_asset) = event_assets.assets_as_ref();
                Message::OrderExecuted {
                    order_type: *order_type,
                    side: *side,
                    amount_asset_ticker: self.asset_ticker(amount_asset).await?,
                    price_asset_ticker: self.asset_ticker(price_asset).await?,
                    execution: *execution,
                    timestamp: *timestamp,
                }
            }
            (
                Event::PriceChanged {
                    asset_pair: event_assets,
                    price_range,
                    timestamp,
                },
                Topic::PriceThreshold(topic),
            ) => {
                debug_assert_eq!(event_assets.amount_asset, topic.amount_asset);
                debug_assert_eq!(event_assets.price_asset, topic.price_asset);
                debug_assert!(price_range.contains(topic.price_threshold));
                let (amount_asset, price_asset) = event_assets.assets_as_ref();
                Message::PriceThresholdReached {
                    amount_asset_ticker: self.asset_ticker(amount_asset).await?,
                    price_asset_ticker: self.asset_ticker(price_asset).await?,
                    threshold: topic.price_threshold,
                    timestamp: *timestamp,
                }
            }
            (_, _) => unreachable!("unrecognized combination of subscription and event"),
        };
        Ok(res)
    }

    fn make_metadata(event: &Event, device: &Device) -> MessageData {
        match event {
            Event::OrderExecuted {
                execution: OrderExecution::Full,
                asset_pair,
                ..
            } => MessageData::OrderExecuted {
                amount_asset_id: asset_pair.amount_asset.id(),
                price_asset_id: asset_pair.price_asset.id(),
                address: device.address.as_base58_string(),
            },

            Event::OrderExecuted {
                execution: OrderExecution::Partial { .. },
                asset_pair,
                ..
            } => MessageData::OrderPartiallyExecuted {
                amount_asset_id: asset_pair.amount_asset.id(),
                price_asset_id: asset_pair.price_asset.id(),
                address: device.address.as_base58_string(),
            },

            Event::PriceChanged { asset_pair, .. } => MessageData::PriceThresholdReached {
                amount_asset_id: asset_pair.amount_asset.id(),
                price_asset_id: asset_pair.price_asset.id(),
                address: device.address.as_base58_string(),
            },
        }
    }

    async fn asset_ticker(&self, asset: &Asset) -> Result<String, Error> {
        let maybe_ticker = self.assets.ticker(asset).await.map_err(Error::AssetsApiError)?;
        let ticker = maybe_ticker.unwrap_or_else(|| asset.id());
        Ok(ticker)
    }

    fn localize(&self, message: &Message, locale: &LocaleInfo) -> LocalizedMessage {
        const FALLBACK_LANG: &str = "en";
        let maybe_message = self.localizer.localize(message, locale);
        if let Some(message) = maybe_message {
            message
        } else {
            let fallback_locale = LocaleInfo {
                lang: FALLBACK_LANG.to_string(),
                utc_offset_seconds: locale.utc_offset_seconds,
            };
            self.localizer
                .localize(message, &fallback_locale)
                .expect("fallback translation")
        }
    }
}
