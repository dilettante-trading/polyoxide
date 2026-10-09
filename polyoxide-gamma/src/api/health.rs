use polyoxide_core::{
    ApiError, Authenticator, DynAuthenticator, HttpClient, RequestError, RequestParts,
};
use reqwest::Method;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::GammaError;

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
        // Health checks are capped like any other route (100/10s), and run on
        // the send loop, so a 429 is retried and holds the client (DRIFT R8).
        let stopwatch = Stopwatch::default();
        let response = self
            .http_client
            .send(
                RequestParts::new(Method::GET, "/status"),
                &[],
                Some(DynAuthenticator::from_ref(&stopwatch)),
            )
            .await?;
        let latency = stopwatch.elapsed();

        if !response.status().is_success() {
            return Err(GammaError::from_response(response).await);
        }

        Ok(latency)
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
