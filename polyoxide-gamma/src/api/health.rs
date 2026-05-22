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
    /// Makes a GET request to the API root and returns the latency. The Gamma
    /// root URL responds with a `301` redirect to `/docs`; since the shared
    /// HTTP client disables redirect-following, that 3xx response is treated
    /// as a successful ping (the server is reachable and timed).
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
            tracing::debug!(%status, "Gamma ping observed redirect from root");
        }
        if status.is_client_error() || status.is_server_error() {
            return Err(GammaError::from_response(response).await);
        }

        Ok(latency)
    }
}
