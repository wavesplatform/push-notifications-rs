use crate::{
    asset, device,
    error::Error,
    localization,
    message::{self, LocalizedMessage, Message, PreparedMessage},
    stream::{Event, OrderExecution},
    subscription::{self, Subscription, SubscriptionMode, Topic},
    timestamp::WithCurrentTimestamp,
};
use tokio::sync::{mpsc, oneshot};

pub struct EventWithResult {
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

    pub async fn run_event_loop(&self, mut events: mpsc::Receiver<EventWithResult>) {
        while let Some(event) = events.recv().await {
            let EventWithResult { event, result_tx } = event;
            let res = self.process_event(&event).await;
            result_tx.send(res).expect("ack");
        }
    }

    async fn process_event(&self, event: &Event) -> Result<(), Error> {
        let subscriptions = self.subscriptions.matching(event).await;
        for subscription in subscriptions {
            let is_oneshot = subscription.mode == SubscriptionMode::Once;
            let msg = self.make_message(event, &subscription.topic).await?;
            let devices = self.devices.subscribers(&subscription.subscriber).await?;
            for device in devices {
                let message = self.localizer.localize(&msg, &device.lang);
                let prepared_message = PreparedMessage {
                    device,
                    message,
                    data: None,
                    collapse_key: None,
                };
                self.messages
                    .enqueue(prepared_message.with_current_timestamp())
                    .await?;
            }
            if is_oneshot {
                self.subscriptions.cancel(subscription).await;
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
                    amount_asset_id,
                    price_asset_id,
                    execution,
                },
                Topic::OrderFulfilled {
                    amount_asset,
                    price_asset,
                },
            ) => {
                debug_assert_eq!(amount_asset_id, amount_asset.as_ref().expect("amount"));
                debug_assert_eq!(price_asset_id, price_asset.as_ref().expect("price"));
                Message::OrderExecuted {
                    order_type: *order_type,
                    side: *side,
                    amount_asset_ticker: self.assets.ticker(amount_asset_id).await?,
                    price_asset_ticker: self.assets.ticker(price_asset_id).await?,
                    execution: *execution,
                }
            }
            (
                Event::PriceChanged {
                    amount_asset_id,
                    price_asset_id,
                    current_price,
                    previous_price,
                },
                Topic::PriceThreshold {
                    amount_asset,
                    price_threshold,
                },
            ) => {
                debug_assert_eq!(amount_asset_id, amount_asset.as_ref().expect("amount"));
                debug_assert_eq!(
                    price_asset_id,
                    price_threshold.asset_id().as_ref().expect("price")
                );
                debug_assert!(
                    current_price.has_crossed_threshold(previous_price, price_threshold.value())
                );
                let decimals = self
                    .assets
                    .decimals(price_threshold.asset_id().as_ref().expect("price_asset"))
                    .await?;
                let price_threshold = apply_decimals(price_threshold.value(), decimals);
                Message::PriceThresholdReached {
                    amount_asset_ticker: self.assets.ticker(amount_asset_id).await?,
                    price_asset_ticker: self.assets.ticker(price_asset_id).await?,
                    threshold: price_threshold,
                }
            }
            (_, _) => unreachable!("unrecognized combination of subscription and event"),
        };
        Ok(res)
    }
}

fn apply_decimals(value: u64, decimals: u8) -> f64 {
    let divisor = 1_u64.pow(decimals as u32);
    value as f64 / divisor as f64
}
