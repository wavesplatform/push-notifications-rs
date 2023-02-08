use model::waves::Address;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database query error: {0}")]
    QueryError(#[from] diesel::result::Error),

    #[error("Database query returned a bad address: {0}")]
    BadAddress(String),

    #[error("Database query returned a bad asset: {0}")]
    BadAsset(String),

    #[error("Database query returned a bad subscription mode: {0}")]
    BadTopicType(i32),

    #[error("Subscriptions limit ({1}) exceeded for address {0:?}")]
    LimitExceeded(Address, u32),
}
