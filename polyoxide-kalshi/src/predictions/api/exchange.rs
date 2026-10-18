//! Reference data: the exchange's status.

use std::sync::Arc;

use crate::predictions::{
    error::PredictionsError,
    request::{routes, Routed},
    types::ExchangeStatus,
    Inner,
};

/// Exchange namespace.
#[derive(Debug, Clone)]
pub struct ExchangeApi {
    pub(crate) inner: Arc<Inner>,
}

impl ExchangeApi {
    /// The exchange's status, via `GET /trade-api/v2/exchange/status`.
    pub fn status(&self) -> GetExchangeStatus {
        GetExchangeStatus {
            request: Routed::new(&self.inner, routes::EXCHANGE_STATUS, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/exchange/status`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetExchangeStatus {
    pub(crate) request: Routed<ExchangeStatus>,
}

impl GetExchangeStatus {
    /// Waits for the Read bucket, sends, and decodes the response.
    ///
    /// Kalshi sends this route's body with a 500, 503 or 504 too, which is an error here, as every
    /// non-2xx answer is.
    pub async fn send(self) -> Result<ExchangeStatus, PredictionsError> {
        self.request.send().await
    }
}
