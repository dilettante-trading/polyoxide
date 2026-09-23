use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{error::DataApiError, types::ApprovalsResponse};

/// Approvals namespace (`/v1/approvals`).
///
/// # Deprecated: the route is gone
///
/// Upstream dropped `GET /v1/approvals` from the published Data API spec, and
/// as of 2026-09-23 the host answers `404 page not found` for it, with or
/// without a `user`. Every call through this namespace fails. The same data
/// is served by [`DataV2::approvals`](crate::v2::DataV2::approvals)
/// (`GET /v2/approvals`), which is live.
///
/// Before its removal the route never worked either: from 2026-08-15 it
/// returned HTTP 500 for every valid address, so it never had a live test.
#[deprecated(
    note = "`/v1/approvals` was removed upstream and now returns 404; use `data.v2().approvals(user)`"
)]
#[derive(Clone)]
pub struct ApprovalsApi {
    pub(crate) http_client: HttpClient,
}

#[allow(deprecated)]
impl ApprovalsApi {
    /// Get token approval state for a wallet (`GET /v1/approvals`).
    ///
    /// Reports whether the wallet has granted the approvals Polymarket needs,
    /// so a client can prompt for the missing ones instead of reading each
    /// allowance onchain. Every tracked token and spender pair is returned,
    /// including pairs the wallet has never approved.
    pub fn get(&self, user_address: impl Into<String>) -> Request<ApprovalsResponse, DataApiError> {
        Request::new(self.http_client.clone(), "/v1/approvals").query("user", user_address.into())
    }
}
