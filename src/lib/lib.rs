#![allow(dead_code, unused_variables)]

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate diesel;

pub mod config;
pub mod schema;

mod asset;
mod device;
mod error;
mod fcm;
mod localization;
mod message;
mod model;
mod processing;
mod stream;
mod subscription;

pub use error::Error;
pub use message::Message;

mod timestamp {
    use chrono::{DateTime, Utc};

    pub struct WithTimestamp<T> {
        timestamp: DateTime<Utc>,
        value: T,
    }

    pub trait WithCurrentTimestamp<T> {
        fn with_current_timestamp(self) -> WithTimestamp<T>;
    }

    impl<T> WithCurrentTimestamp<T> for T {
        fn with_current_timestamp(self) -> WithTimestamp<T> {
            WithTimestamp {
                timestamp: Utc::now(),
                value: self,
            }
        }
    }
}
