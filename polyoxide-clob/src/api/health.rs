use polyoxide_core::HttpClient;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use crate::{
    error::ClobError,
    request::{AuthMode, Request},
};

/// Health namespace for API health and latency operations
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
    pub(crate) chain_id: u64,
}

impl Health {
    /// Measure the round-trip time (RTT) to the Polymarket CLOB API.
    ///
    /// Makes a GET request to the API root and returns the latency.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::Clob;
    ///
    /// # async fn example() -> Result<(), polyoxide_clob::ClobError> {
    /// let client = Clob::public();
    /// let latency = client.health().ping().await?;
    /// println!("API latency: {}ms", latency.as_millis());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ping(&self) -> Result<Duration, ClobError> {
        let start = Instant::now();
        let response = self
            .http_client
            .client
            .get(self.http_client.base_url.clone())
            .send()
            .await?;
        let latency = start.elapsed();

        if !response.status().is_success() {
            return Err(ClobError::from_response(response).await);
        }

        Ok(latency)
    }

    /// Get the current server time
    pub fn server_time(&self) -> Request<ServerTimeResponse> {
        Request::get(
            self.http_client.clone(),
            "/time",
            AuthMode::None,
            self.chain_id,
        )
    }
}

/// Response from the server time endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTimeResponse {
    pub time: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_time_response_deserializes() {
        let json = r#"{"time": "1700000000"}"#;
        let resp: ServerTimeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.time, "1700000000");
    }
}
