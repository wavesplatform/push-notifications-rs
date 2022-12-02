use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("GenericError: {0}")]
    Generic(String),
    #[error("LoadConfigFailed: {0}")]
    LoadConfigFailed(#[from] envy::Error),
    #[error("HttpRequestError")]
    HttpRequestError(Arc<reqwest::Error>),
    #[error("SerdeJsonError: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("FcmUpstreamError: {0}")]
    FcmUpstreamError(#[from] fcm::FcmError),
    #[error("DbConnectionError: {0}")]
    DbConnectionError(#[from] diesel::result::ConnectionError),
    #[error("DbQueryError: {0}")]
    DbQueryError(#[from] diesel::result::Error),
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::HttpRequestError(Arc::new(err))
    }
}
