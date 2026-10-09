use polyoxide_core::{
    ApiError, Authenticator, DynAuthenticator, HttpClient, Request, RequestParts,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::ClobError;

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

/// Health namespace for API health and latency operations
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
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
        // On the send loop, so it waits for a permit and the throttle, and a
        // 429 is retried and holds the client (DRIFT R8). The path is the base
        // URL's own, as the ping has always sent.
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
            return Err(ClobError::from_response(response).await);
        }

        Ok(latency)
    }

    /// Get the current server time
    pub fn server_time(&self) -> Request<ServerTimeResponse, ClobError> {
        Request::new(self.http_client.clone(), "/time")
    }
}

/// Response from the server time endpoint.
///
/// The live API returns a bare integer (e.g. `1700000000`), not a JSON object.
/// This type handles that format via a custom `Deserialize` implementation.
#[derive(Debug, Clone, Serialize)]
pub struct ServerTimeResponse {
    pub time: i64,
}

impl<'de> Deserialize<'de> for ServerTimeResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let time = i64::deserialize(deserializer)?;
        Ok(ServerTimeResponse { time })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_time_response_deserializes() {
        let json = "1700000000";
        let resp: ServerTimeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.time, 1700000000);
    }

    #[test]
    fn server_time_response_rejects_json_object() {
        let json = r#"{"time": 1700000000}"#;
        assert!(serde_json::from_str::<ServerTimeResponse>(json).is_err());
    }

    #[test]
    fn server_time_response_rejects_string() {
        let json = r#""1700000000""#;
        assert!(serde_json::from_str::<ServerTimeResponse>(json).is_err());
    }

    /// `ping` must go through the same gating as every other request: it
    /// used to reach for the reqwest client directly, past the permit and the
    /// throttle. Holding the single concurrency permit is the cheap,
    /// deterministic way to prove it queues.
    #[tokio::test]
    async fn ping_waits_on_the_shared_request_gate() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_body("OK")
            .create_async()
            .await;

        let http_client = polyoxide_core::HttpClientBuilder::new(server.url())
            .with_rate_limiter(polyoxide_core::polymarket::clob_limits())
            .with_max_concurrent(1)
            .build()
            .unwrap();
        let health = Health {
            http_client: http_client.clone(),
        };

        // Hold the only permit, so anything respecting the gate must queue.
        let _permit = http_client.acquire_concurrency().await.unwrap();

        let result = tokio::time::timeout(Duration::from_millis(100), health.ping()).await;
        assert!(
            result.is_err(),
            "ping() completed while the concurrency budget was exhausted — it is \
             bypassing the rate limiting infrastructure"
        );
    }
}
