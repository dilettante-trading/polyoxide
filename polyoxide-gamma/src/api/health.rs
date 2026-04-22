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
        let url = format!("{}/status", self.http_client.base_url);
        let start = Instant::now();
        let response = self.http_client.client.get(url).send().await?;
        let latency = start.elapsed();

        if !response.status().is_success() {
            return Err(GammaError::from_response(response).await);
        }

        Ok(latency)
    }
}
