use diesel_async::pooled_connection::PoolError;
use warp::reject::Reject;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error: {0}")]
    Generic(String),

    #[error("Database query error: {0}")]
    DbQueryError(#[from] diesel::result::Error),

    #[error("Base Waves address: {0}")]
    AddressParseError(String),

    #[error("Bad topic: {0}")]
    BadTopic(#[from] crate::topic::TopicError),

    #[error("Database pool error: {0}")]
    PoolError(#[from] bb8::RunError<PoolError>),

    #[error("Database error: {0}")]
    DatabaseError(#[from] database::error::Error),
}

impl Reject for Error {}
