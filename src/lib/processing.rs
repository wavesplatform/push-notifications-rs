use crate::{
    error::Error,
    message::{self, LocalizedMessage, Message},
    model::AsBase58String,
    stream::{Event, OrderExecution},
    subscription::{self, Subscription, SubscriptionMode, Topic},
    timestamp::WithCurrentTimestamp,
};
use tokio::sync::mpsc;

pub struct MessagePump {
    subscriptions: subscription::Repo,
    messages: message::Queue,
}

impl MessagePump {
    //TODO pub fn new()

    pub async fn run_event_loop(&self, mut events: mpsc::Receiver<Event>) {
        while let Some(event) = events.recv().await {
            let res = self.process_event(&event).await;
            if let Err(err) = res {
                self.handle_error(event, err).await;
            }
        }
    }

    async fn handle_error(&self, _event: Event, _error: Error) {
        todo!("handle error"); // what to do with a failed event? Store for re-processing?
    }

    async fn process_event(&self, event: &Event) -> Result<(), Error> {
        let subscriptions = self.subscriptions.matching(event).await;
        for subscription in subscriptions {
            let is_oneshot = subscription.mode == SubscriptionMode::Once;
            let msg = make_message(event, &subscription.topic);
            let msg = localize_message(msg);
            self.messages.enqueue(msg.with_current_timestamp()).await?;
            if is_oneshot {
                self.subscriptions.cancel(subscription).await;
            }
        }
        Ok(())
    }
}

fn make_message(event: &Event, topic: &Topic) -> Message {
    match (event, topic) {
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
                amount_asset_ticker: amount_asset_id.as_base58_string(),
                price_asset_ticker: price_asset_id.as_base58_string(),
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
            let price_threshold = price_threshold.value() as f64; //TODO need to apply decimals
            debug_assert!(current_price.has_crossed_threshold(previous_price, price_threshold));
            Message::PriceThresholdReached {
                amount_asset_ticker: amount_asset_id.as_base58_string(),
                price_asset_ticker: price_asset_id.as_base58_string(),
                threshold: price_threshold,
            }
        }
        (_, _) => unreachable!("unrecognized combination of subscription and event"),
    }
}

fn localize_message(msg: Message) -> LocalizedMessage {
    todo!("use localization service")
}
