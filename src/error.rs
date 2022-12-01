use wavesexchange_loaders::LoaderError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("LoadConfigError: {0}")]
    LoadConfigError(#[from] envy::Error),

    #[error("TransactionError: {0}")]
    TranslationError(String),

    #[error("UpstreamApiRequestError: {0}")]
    UpstreamApiRequestError(#[from] wavesexchange_apis::Error),

    #[error("WxLoaderFailed: {0}")]
    WxLoaderFailed(String),
}

impl From<LoaderError<Error>> for Error {
    fn from(err: LoaderError<Error>) -> Self {
        match err {
            LoaderError::MissingValues(e) => Error::WxLoaderFailed(e),
            LoaderError::Other(e) => e,
        }
    }
}
