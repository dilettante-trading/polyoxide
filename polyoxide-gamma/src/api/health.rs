use polyoxide_core::{HttpClient, RequestError};
use std::time::{Duration, Instant};

use crate::error::GammaError;

/// Health namespace for API health and latency operations
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
}

impl Health {
    /// Measure the round-trip time (RTT) to the Polymarket Gamma API.
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
        let url = self.http_client.base_url.join("/status")?;

        // Health checks are capped like any other route (100/10s). This reaches
        // for `client` directly rather than going through `Request`, so the
        // gating has to be applied by hand — omitting it bypassed both the
        // limiter and the concurrency budget that keeps Cloudflare from seeing
        // a burst from this process.
        let _permit = self.http_client.acquire_concurrency().await;
        self.http_client.acquire_rate_limit("/status", None).await;

        let start = Instant::now();
        let response = self.http_client.client.get(url).send().await?;
        let latency = start.elapsed();

        if !response.status().is_success() {
            return Err(GammaError::from_response(response).await);
        }

        Ok(latency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_core::{HttpClientBuilder, RateLimiter};

    /// `ping` must go through the same gating as every other request.
    ///
    /// It reaches for `http_client.client` directly rather than going through
    /// `Request`, so nothing structural forces it to respect the limiter — only
    /// this test does. Holding the single concurrency permit is the cheap,
    /// deterministic way to prove it queues: if `ping` bypasses the gate it
    /// returns immediately instead of timing out.
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
            .with_rate_limiter(RateLimiter::gamma_default())
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
