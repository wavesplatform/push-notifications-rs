use thiserror::Error;

use crate::{asset, localization};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Localization API error: {0}")]
    LocalizationApiError(localization::GatewayError),

    #[error("Assets API error: {0}")]
    AssetsApiError(asset::GatewayError),

    //TODO We have two separate database error cases here because database transactions
    // are not abstracted away but used directly. Need a proper solution for transactions.

    // Comes from `conn.transaction()` calls
    #[error("Database transaction error: {0}")]
    TransactionError(#[from] diesel::result::Error),

    // Comes from database repos
    #[error("Database error: {0}")]
    DatabaseError(#[from] database::error::Error),
}
