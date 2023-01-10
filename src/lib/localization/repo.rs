use std::collections::HashMap;

use crate::{
    error::Error,
    message::{LocalizedMessage, Message},
    model::Lang,
    stream::{OrderExecution, OrderSide},
};

use super::{
    lokalise_gateway::{LokaliseConfig, RemoteGateway, LOCALISE_API_URL},
    template::interpolate,
    translations::TranslationMap,
};

mod lokalise_keys {
    pub const ORDER_FILLED_TITLE: &str = "orderFilledTitle";
    pub const ORDER_FILLED_MSG: &str = "orderFilledMessage";
    pub const ORDER_PART_FILLED_MSG: &str = "orderPartFilledMessage";
    pub const PRICE_ALERT_TITLE: &str = "priceAlertTitle";
    pub const PRICE_ALERT_MSG: &str = "priceAlertMessage";
    pub const BUY: &str = "buy";
    pub const SELL: &str = "sell";
}

pub struct Repo {
    translations: TranslationMap,
}

impl Repo {
    pub async fn new(config: LokaliseConfig) -> Result<Self, Error> {
        let remote_gateway = RemoteGateway::new(LOCALISE_API_URL, &config.token);
        let keys = remote_gateway.keys_for_project(&config.project_id).await?;
        let translations = TranslationMap::build(keys);
        if translations.is_complete() {
            log::trace!("Lokalise translations: {:?}", translations);
        } else {
            log::warn!("Incomplete lokalise translations: {:?}", translations);
        }
        Ok(Self { translations })
    }

    pub fn localize(&self, message: &Message, lang: &Lang) -> Option<LocalizedMessage> {
        let translate = |key| self.translations.translate(key, lang);

        let title_key = match message {
            Message::OrderExecuted { .. } => lokalise_keys::ORDER_FILLED_TITLE,
            Message::PriceThresholdReached { .. } => lokalise_keys::PRICE_ALERT_TITLE,
        };

        let body_key = match message {
            Message::OrderExecuted { execution, .. } => match execution {
                OrderExecution::Full => lokalise_keys::ORDER_FILLED_MSG,
                OrderExecution::Partial { .. } => lokalise_keys::ORDER_PART_FILLED_MSG,
            },
            Message::PriceThresholdReached { .. } => lokalise_keys::PRICE_ALERT_MSG,
        };

        let side_key = match message {
            Message::OrderExecuted { side, .. } => Some(match side {
                OrderSide::Buy => lokalise_keys::BUY,
                OrderSide::Sell => lokalise_keys::SELL,
            }),
            Message::PriceThresholdReached { .. } => None,
        };

        let side = match side_key {
            Some(key) => translate(key)?,
            None => "",
        };

        let (amount_token, price_token) = match message {
            Message::OrderExecuted {
                amount_asset_ticker,
                price_asset_ticker,
                ..
            }
            | Message::PriceThresholdReached {
                amount_asset_ticker,
                price_asset_ticker,
                ..
            } => (amount_asset_ticker, price_asset_ticker),
        };

        let pair = format!("{}/{}", amount_token, price_token);

        let value = match message {
            Message::OrderExecuted { .. } => "".to_string(),
            Message::PriceThresholdReached { threshold, .. } => format!("{}", threshold),
        };

        let (date, time) = match message {
            Message::OrderExecuted { timestamp, .. }
            | Message::PriceThresholdReached { timestamp, .. } => format_date_time(*timestamp),
        };

        let title = translate(title_key)?;
        let body = translate(body_key)?;

        let subst = HashMap::from([
            ("", ""),
            ("amountToken", amount_token),
            ("priceToken", price_token),
            ("pair", &pair),
            ("side", side),
            ("value", &value),
            ("date", &date),
            ("time", &time),
        ]);

        Some(LocalizedMessage {
            notification_title: interpolate(title, &subst),
            notification_body: interpolate(body, &subst),
        })
    }
}

fn format_date_time(unix_timestamp: i64) -> (String, String) {
    use chrono::{TimeZone, Utc};
    if let Some(dt) = Utc.timestamp_millis_opt(unix_timestamp).earliest() {
        let dt = dt.naive_utc();
        let date = dt.date().format("%Y-%m-%d").to_string();
        let time = dt.time().format("%H:%M:%S").to_string();
        (date, time)
    } else {
        ("?".to_string(), "?".to_string())
    }
}
