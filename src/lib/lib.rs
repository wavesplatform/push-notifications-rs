#![allow(dead_code, unused_variables, unused_imports)] //TODO cleanup

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate diesel;

pub mod asset;
pub mod backoff;
pub mod config;
pub mod device;
pub mod localization;
pub mod message;
pub mod processing;
pub mod schema;
pub mod subscription;

mod error;
mod fcm;
mod model;
mod stream;

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
