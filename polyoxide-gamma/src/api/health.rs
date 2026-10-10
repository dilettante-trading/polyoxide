use polyoxide_core::HttpClient;
use std::time::Duration;

use crate::error::GammaError;

/// Health namespace for API health and latency operations
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
}

impl Health {
    /// Measure the round-trip time (RTT) to the Polymarket Gamma API: that of
    /// the attempt that answered, as
    /// [`HttpClient::health`](polyoxide_core::HttpClient::health) times it.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_gamma::Gamma;
    ///
    /// # async fn example() -> Result<(), polyoxide_gamma::GammaError> {
    /// let client = Gamma::new()?;
    /// let latency = client.health().ping().await?;
    /// println!("API latency: {}ms", latency.as_millis());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ping(&self) -> Result<Duration, GammaError> {
        // Health checks are capped like any other route (100/10s), and run on
        // the send loop, so a 429 is retried and holds the client (DRIFT R8).
        let pong = self
            .http_client
            .health::<GammaError>("/status", &[])
            .await?;
        Ok(pong.round_trip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_core::{polymarket, HttpClientBuilder};

    /// `ping` must go through the same gating as every other request.
    ///
    /// It discards its body, so it calls the send loop rather than going
    /// through `Request`, and only this test shows it respects the gate.
    /// Holding the single concurrency permit is the cheap, deterministic way
    /// to prove it queues: if `ping` bypasses the gate it returns immediately
    /// instead of timing out.
    #[tokio::test]
    async fn ping_waits_on_the_shared_request_gate() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/status")
            .with_status(200)
            .with_body("OK")
            .create_async()
            .await;

        let http_client = HttpClientBuilder::new(server.url())
            .with_rate_limiter(polymarket::gamma_limits())
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
