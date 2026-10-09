use polyoxide_core::{
    ApiError, Authenticator, DynAuthenticator, HttpClient, Request, RequestError, RequestParts,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::DataApiError;

/// Notes when an attempt is signed, which is just before it is sent, so a
/// ping's latency leaves out the waits for a permit, the throttle and a
/// retry's backoff: the last attempt's round trip, as before the ping ran on
/// the send loop.
#[derive(Default)]
struct Stopwatch(Mutex<Option<Instant>>);

impl Stopwatch {
    /// Time since the last attempt was signed.
    fn elapsed(&self) -> Duration {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .map_or(Duration::ZERO, |sent| sent.elapsed())
    }
}

impl Authenticator for Stopwatch {
    async fn sign(&self, _parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Instant::now());
        Ok(())
    }
}

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
    /// Makes a GET request to the API root and returns the latency.
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
        let path = self.http_client.base_url.path().to_owned();
        let stopwatch = Stopwatch::default();
        let response = self
            .http_client
            .send(
                RequestParts::new(Method::GET, path),
                &[],
                Some(DynAuthenticator::from_ref(&stopwatch)),
            )
            .await?;
        let latency = stopwatch.elapsed();

        if !response.status().is_success() {
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
