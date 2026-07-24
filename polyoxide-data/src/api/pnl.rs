use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::DataApiError,
    types::{PnlFidelity, PnlPoint},
};

/// PnL namespace — `GET /user-pnl` on `user-pnl-api.polymarket.com`.
///
/// # Stability
///
/// This host is **not covered by any published Polymarket OpenAPI spec**. It
/// backs the PnL chart on the Polymarket web app and was verified live, but it
/// carries no documented compatibility guarantee — treat it as more likely to
/// change without notice than the rest of this crate. The base URL is
/// configurable via
/// [`DataApiBuilder::pnl_base_url`](crate::DataApiBuilder::pnl_base_url).
#[derive(Clone)]
pub struct PnlApi {
    pub(crate) http_client: HttpClient,
}

impl PnlApi {
    /// Get a user's realized-plus-unrealized PnL time series.
    ///
    /// Returns points of `{ t, p }` — Unix timestamp in seconds and PnL in
    /// USDC. `user_address` is required by the upstream API.
    pub fn history(&self, user_address: impl Into<String>) -> UserPnlRequest {
        UserPnlRequest {
            request: Request::new(self.http_client.clone(), "/user-pnl")
                .query("user_address", user_address.into()),
        }
    }
}

/// Request builder for a user's PnL time series.
pub struct UserPnlRequest {
    request: Request<Vec<PnlPoint>, DataApiError>,
}

impl UserPnlRequest {
    /// Restrict the series to a trailing window (e.g. `"1d"`, `"1w"`, `"all"`).
    ///
    /// Omitted by default, which yields the upstream default window. Unlike
    /// [`fidelity`](Self::fidelity), the accepted values are not enumerated by
    /// the API's own error messages, so this takes a string.
    pub fn interval(mut self, interval: impl Into<String>) -> Self {
        self.request = self.request.query("interval", interval.into());
        self
    }

    /// Set the sampling resolution of the returned series.
    pub fn fidelity(mut self, fidelity: PnlFidelity) -> Self {
        self.request = self.request.query("fidelity", fidelity);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<PnlPoint>, DataApiError> {
        self.request.send().await
    }
}
