use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database query error: {0}")]
    QueryError(#[from] diesel::result::Error),

    #[error("Database query returned a malformed topic: {0}")]
    BadTopic(#[from] model::topic::TopicError),
}
