//! `service` route: data freshness.

use polyoxide_core::Request;

use crate::{
    v2::{envelope::Envelope, types::ServiceStatus, DataV2},
    DataApiError,
};

impl DataV2 {
    /// `GET /v2/status`: how fresh the served data is.
    ///
    /// This reports freshness, not liveness. For a liveness check use
    /// [`DataApi::health`](crate::DataApi::health).
    pub fn status(&self) -> GetStatus {
        GetStatus {
            request: Request::new(self.http_client.clone(), "/v2/status"),
        }
    }
}

/// Builder for `GET /v2/status`.
pub struct GetStatus {
    request: Request<Envelope<ServiceStatus>, DataApiError>,
}

impl GetStatus {
    /// Fetch the status.
    pub async fn send(self) -> Result<ServiceStatus, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}
