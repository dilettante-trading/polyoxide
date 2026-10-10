use polyoxide_core::{HttpClient, Request};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::ClobError;

/// Health namespace for API health and latency operations
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
}

impl Health {
    /// Measure the round-trip time (RTT) to the Polymarket CLOB API.
    ///
    /// Makes a GET request to the API root and returns the latency of the
    /// attempt that answered, as
    /// [`HttpClient::health`](polyoxide_core::HttpClient::health) times it.
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
        let path = self.http_client.base_url.path();
        let pong = self.http_client.health::<ClobError>(path, &[]).await?;
        Ok(pong.round_trip)
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
