use std::sync::Arc;
use wavesexchange_loaders::LoaderError;

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

    #[error("TranslationError: {0}")]
    TranslationError(String),

    #[error("UpstreamApiRequestError: {0}")]
    UpstreamApiRequestError(#[from] wavesexchange_apis::Error),

    #[error("WxLoaderFailed: {0}")]
    WxLoaderFailed(String),
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::HttpRequestError(Arc::new(err))
    }
}

impl From<LoaderError<Error>> for Error {
    fn from(err: LoaderError<Error>) -> Self {
        match err {
            LoaderError::MissingValues(e) => Error::WxLoaderFailed(e),
            LoaderError::Other(e) => e,
        }
    }
}
