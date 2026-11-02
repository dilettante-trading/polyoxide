//! Reference data: `/api/v3/exchangeInfo`.

use polyoxide_core::HttpClient;

use crate::spot::{
    error::SpotError,
    request::Routed,
    types::{ExchangeInfo, Symbol},
    weight::{Cost, Route},
};

/// Exchange namespace.
#[derive(Debug, Clone)]
pub struct ExchangeApi {
    pub(crate) http: HttpClient,
}

impl ExchangeApi {
    /// `GET /api/v3/exchangeInfo` (weight 20 with a symbol or without): every
    /// symbol, its filters, and the IP's limits.
    pub fn exchange_info(&self) -> GetExchangeInfo {
        GetExchangeInfo {
            request: Routed::new(&self.http, Route::ExchangeInfo),
        }
    }
}

/// Request builder for `GET /api/v3/exchangeInfo`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetExchangeInfo {
    request: Routed<ExchangeInfo>,
}

impl GetExchangeInfo {
    polyoxide_core::query_setters! {
        /// Restrict the listing to one symbol.
        symbol: &Symbol => "symbol",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` holds every request on the budget, then this request is
    /// retried on the client's schedule; with no retry left and no
    /// `Retry-After`, the hold runs to the next UTC minute. A `418` holds
    /// every request for its `Retry-After`, or 2 minutes, and is not retried.
    ///
    /// The wait has no deadline of its own: a request can wait out a minute's
    /// spent budget or a ban, which Binance documents at up to 3 days. Wrap
    /// the call in `tokio::time::timeout` where a caller needs a deadline.
    pub async fn send(self) -> Result<ExchangeInfo, SpotError> {
        self.request.send().await
    }
}
