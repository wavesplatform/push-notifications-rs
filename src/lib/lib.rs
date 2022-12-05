#![allow(dead_code, unused_variables)]

pub mod config;
pub mod schema;

mod asset;
mod device;
mod error;
mod fcm;
mod localization;
mod message;
mod stream;
mod subscription;

use chrono::{DateTime, Utc};

pub use error::Error;
pub use message::Message;

pub struct WithTimestamp<T> {
    timestamp: DateTime<Utc>,
    value: T,
}
