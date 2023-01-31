//! Push notifications processor common code library

#[macro_use]
extern crate async_trait;

extern crate wavesexchange_log as log;

mod error;
mod processing;

pub mod asset;
pub mod localization;

pub use crate::{
    error::Error,
    processing::{EventWithFeedback, MessagePump},
};
