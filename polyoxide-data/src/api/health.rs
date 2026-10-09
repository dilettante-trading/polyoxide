use polyoxide_core::{HttpClient, Request};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::DataApiError;

/// Health namespace for API health operations
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
}

impl Health {
    /// Check API health status
    pub async fn check(&self) -> Result<HealthResponse, DataApiError> {
        Request::<HealthResponse, DataApiError>::new(self.http_client.clone(), "/")
            .send()
            .await
    }

    /// Measure the round-trip time (RTT) to the Polymarket Data API.
    ///
    /// Makes a GET request to the API root and returns the latency of the
    /// attempt that answered, as
    /// [`HttpClient::health`](polyoxide_core::HttpClient::health) times it.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_data::DataApi;
    ///
    /// # async fn example() -> Result<(), polyoxide_data::DataApiError> {
    /// let client = DataApi::new()?;
    /// let latency = client.health().ping().await?;
    /// println!("API latency: {}ms", latency.as_millis());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ping(&self) -> Result<Duration, DataApiError> {
        // On the send loop, so a 429 is retried and holds the client (DRIFT
        // R8). The path is the base URL's own, as the ping has always sent.
        let path = self.http_client.base_url.path();
        let pong = self.http_client.health::<DataApiError>(path, &[]).await?;
        Ok(pong.round_trip)
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Status indicator (returns "OK" when healthy)
    pub data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_health_response() {
        let json = r#"{"data": "OK"}"#;
        let health: HealthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(health.data, "OK");
    }

    #[test]
    fn health_response_roundtrip() {
        let original = HealthResponse {
            data: "OK".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data, original.data);
    }
}
