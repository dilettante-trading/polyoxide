//! `service` route: data freshness.

use polyoxide_core::Request;

use crate::data::{envelope::Envelope, types::ServiceStatus, DataApi, DataError};

impl DataApi {
    /// `GET /v2/status`: how fresh the served data is.
    ///
    /// With v1 retired (2026-11-02) and its `/` gone, a 200 here is also the
    /// host's liveness signal.
    pub fn status(&self) -> GetStatus {
        GetStatus {
            request: Request::new(self.http_client.clone(), "/v2/status"),
        }
    }
}

/// Builder for `GET /v2/status`.
pub struct GetStatus {
    request: Request<Envelope<ServiceStatus>, DataError>,
}

impl GetStatus {
    /// Fetch the status.
    pub async fn send(self) -> Result<ServiceStatus, DataError> {
        Ok(self.request.send().await?.data)
    }
}
