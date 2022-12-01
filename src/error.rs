#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("LoadConfigError: {0}")]
    LoadConfigError(#[from] envy::Error),

    #[error("TransactionError: {0}")]
    TranslationError(String),

    #[error("UpstreamApiRequestError: {0}")]
    UpstreamApiRequestError(#[from] wavesexchange_apis::Error),
}
