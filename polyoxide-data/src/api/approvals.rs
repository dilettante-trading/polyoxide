use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{error::DataApiError, types::ApprovalsResponse};

/// Approvals namespace (`/v1/approvals`).
///
/// # Upstream status
///
/// As of 2026-08-15 the route is deployed but non-functional: it returns
/// `400` when `user` is missing (so the route exists and validates) yet
/// `{"error":"internal server error"}` with HTTP 500 for **every** valid
/// address tried, including active wallets. The client is modelled from the
/// published spec and covered by mock tests; there is deliberately no live
/// test, because `nightly-behavioral.yml` runs `--ignored` tests and a
/// permanently red one would file a tracking issue every night.
///
/// Add a live test once upstream starts returning `200`.
#[derive(Clone)]
pub struct ApprovalsApi {
    pub(crate) http_client: HttpClient,
}

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
