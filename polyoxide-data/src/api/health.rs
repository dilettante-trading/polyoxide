use polyoxide_core::{HttpClient, Request, RequestError};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

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
    /// Makes a GET request to the API root and returns the latency. Any
    /// non-error HTTP response (including 3xx redirects) counts as a
    /// successful ping; only 4xx/5xx are surfaced as errors.
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
        let _permit = self.http_client.acquire_concurrency().await;
        self.http_client.acquire_rate_limit("/", None).await;

        let start = Instant::now();
        let response = self
            .http_client
            .client
            .get(self.http_client.base_url.clone())
            .send()
            .await?;
        let latency = start.elapsed();

        let status = response.status();
        if status.is_redirection() {
            tracing::debug!(%status, "Data API ping observed redirect from root");
        }
        if status.is_client_error() || status.is_server_error() {
            return Err(DataApiError::from_response(response).await);
        }

        Ok(latency)
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
