#![allow(dead_code, unused_variables)]

#[macro_use]
extern crate diesel;

pub mod config;

mod asset;
mod device;
mod error;
mod fcm;
mod localization;
mod message;
mod stream;
mod subscription;

use chrono::{DateTime, Utc};

pub struct WithTimestamp<T> {
    timestamp: DateTime<Utc>,
    value: T,
}
