//! The account's limits: `GET /trade-api/v2/account/limits`.

use polyoxide_core::ApiError;

use super::AccountApi;
use crate::predictions::{
    error::PredictionsError,
    request::{routes, Routed},
    types::AccountLimits,
};

impl AccountApi {
    /// `GET /trade-api/v2/account/limits`: the account's Read and Write token buckets and the
    /// grants behind its tier. It needs credentials.
    pub fn limits(&self) -> GetAccountLimits {
        GetAccountLimits {
            request: Routed::new(&self.inner, routes::ACCOUNT_LIMITS, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/account/limits`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetAccountLimits {
    pub(crate) request: Routed<AccountLimits>,
}

impl GetAccountLimits {
    /// Signs, sends and decodes the request.
    ///
    /// # Errors
    ///
    /// Without credentials it sends nothing and spends no token: the error is a validation
    /// refusal, an `InvalidRequest`.
    pub async fn send(self) -> Result<AccountLimits, PredictionsError> {
        if !self.request.has_key() {
            return Err(PredictionsError::Api(ApiError::Validation(
                "account().limits() needs credentials: build the client with PredictionsBuilder::credentials"
                    .into(),
            )));
        }
        self.request.send().await
    }
}
