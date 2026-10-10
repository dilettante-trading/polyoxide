use polyoxide_core::{HttpClient, Request};

use crate::{error::DataApiError, types::OpenInterest};

/// OpenInterest namespace for open interest operations
#[derive(Clone)]
pub struct OpenInterestApi {
    pub(crate) http_client: HttpClient,
}

impl OpenInterestApi {
    /// Get open interest for markets
    pub fn get(&self) -> GetOpenInterest {
        GetOpenInterest {
            request: Request::new(self.http_client.clone(), "/oi"),
        }
    }
}

/// Request builder for getting open interest
pub struct GetOpenInterest {
    request: Request<Vec<OpenInterest>, DataApiError>,
}

impl GetOpenInterest {
    polyoxide_core::query_setters! {
        /// Filter by specific market condition IDs
        market: csv impl IntoIterator<Item = impl ToString> => "market",
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<OpenInterest>, DataApiError> {
        self.request.send().await
    }
}
