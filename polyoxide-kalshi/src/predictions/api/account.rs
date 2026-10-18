//! The account namespace: the routes' token costs, which answer without a key.

use std::sync::Arc;

use crate::predictions::{
    request::{routes, Routed},
    types::EndpointCosts,
    Inner, PredictionsError,
};

/// Account namespace.
#[derive(Debug, Clone)]
pub struct AccountApi {
    pub(crate) inner: Arc<Inner>,
}

impl AccountApi {
    /// `GET /trade-api/v2/account/endpoint_costs`: the routes whose token cost differs from the
    /// default.
    pub fn endpoint_costs(&self) -> GetEndpointCosts {
        GetEndpointCosts {
            request: Routed::new(&self.inner, routes::ENDPOINT_COSTS, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/account/endpoint_costs`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetEndpointCosts {
    pub(crate) request: Routed<EndpointCosts>,
}

impl GetEndpointCosts {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<EndpointCosts, PredictionsError> {
        self.request.send().await
    }
}
