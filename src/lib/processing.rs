use crate::{
    asset, device,
    error::Error,
    localization,
    message::{self, LocalizedMessage, Message, PreparedMessage},
    model::{Asset, Lang},
    stream::{Event, OrderExecution, PriceWithDecimals},
    subscription::{self, Subscription, SubscriptionMode, Topic},
};
use diesel_async::{AsyncConnection, AsyncPgConnection};
use futures::FutureExt;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

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
        while let Some(event) = events.recv().await {
            let EventWithFeedback { event, result_tx } = event;
            let this = self.clone();
            let res = conn
                .transaction(|conn| {
                    async move {
                        // Asynchronously process this event within a database transaction
                        this.process_event(event, conn).await
                    }
                    .boxed()
                })
                .await;
            result_tx.send(res).expect("ack");
        }
    }

    async fn process_event(&self, event: Event, conn: &mut AsyncPgConnection) -> Result<(), Error> {
        let subscriptions = self.subscriptions.matching(&event, conn).await?;
        for subscription in subscriptions {
            let is_oneshot = subscription.mode == SubscriptionMode::Once;
            let msg = self.make_message(&event, &subscription.topic).await?;
            let address = &subscription.subscriber;
            let devices = self.devices.subscribers(address, conn).await?;
            for device in devices {
                let message = self.localize(&msg, &device.lang);
                let prepared_message = PreparedMessage {
                    device,
                    message,
                    data: None,
                    collapse_key: None,
                };
                self.messages.enqueue(prepared_message, conn).await?;
            }
            if is_oneshot {
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
                },
                Topic::OrderFulfilled {
                    amount_asset: topic_amount_asset,
                    price_asset: topic_price_asset,
                },
            ) => {
                debug_assert_eq!(event_assets.amount_asset, *topic_amount_asset);
                debug_assert_eq!(event_assets.price_asset, *topic_price_asset);
                let (amount_asset, price_asset) = event_assets.assets_as_ref();
                Message::OrderExecuted {
                    order_type: *order_type,
                    side: *side,
                    amount_asset_ticker: self.asset_ticker(amount_asset).await?,
                    price_asset_ticker: self.asset_ticker(price_asset).await?,
                    execution: *execution,
                }
            }
            (
                Event::PriceChanged {
                    asset_pair: event_assets,
                    price_range,
                },
                Topic::PriceThreshold {
                    amount_asset: topic_amount_asset,
                    price_asset: topic_price_asset,
                    price_threshold,
                },
            ) => {
                debug_assert_eq!(event_assets.amount_asset, *topic_amount_asset);
                debug_assert_eq!(event_assets.price_asset, *topic_price_asset);
                debug_assert!(price_range.contains(*price_threshold));
                let (amount_asset, price_asset) = event_assets.assets_as_ref();
                Message::PriceThresholdReached {
                    amount_asset_ticker: self.asset_ticker(amount_asset).await?,
                    price_asset_ticker: self.asset_ticker(price_asset).await?,
                    threshold: *price_threshold,
                }
            }
            (_, _) => unreachable!("unrecognized combination of subscription and event"),
        };
        Ok(res)
    }

    async fn asset_ticker(&self, asset: &Asset) -> Result<String, Error> {
        let maybe_ticker = self.assets.ticker(asset).await?;
        let ticker = maybe_ticker.unwrap_or_else(|| asset.id());
        Ok(ticker)
    }

    fn localize(&self, message: &Message, lang: &Lang) -> LocalizedMessage {
        const FALLBACK_LANG: &str = "en-US";
        let maybe_message = self.localizer.localize(message, lang);
        if let Some(message) = maybe_message {
            message
        } else {
            let fallback_lang = FALLBACK_LANG.to_string();
            self.localizer
                .localize(message, &fallback_lang)
                .expect("fallback translation")
        }
    }
}
