use crate::shared::{self, PolymarketRetryPolicy};
use polyoxide_core::{ClientConfig, HttpClient};

use crate::data::{error::DataError, v2::DataV2};

const DEFAULT_BASE_URL: &str = "https://data-api.polymarket.com";

/// Main Data API client
///
/// Every route is on `data-api.polymarket.com`, under `/v2` through
/// [`v2`](Self::v2). Polymarket retired Data API v1 on 2026-11-02, and the
/// `user-pnl-api` and `lb-api` hosts with it.
#[derive(Clone)]
pub struct DataApi {
    pub(crate) http_client: HttpClient,
}

impl DataApi {
    /// Create a new Data API client with default configuration
    pub fn new() -> Result<Self, DataError> {
        Self::builder().build()
    }

    /// Create a builder for configuring the client
    pub fn builder() -> DataApiBuilder {
        DataApiBuilder::new()
    }

    polyoxide_core::namespaces! { http_client;
        /// Data API v2 routes (`/v2/*`).
        ///
        /// Shares this client's connection pool, rate limiter, 429 cooldown and
        /// concurrency budget.
        v2: DataV2,
    }
}

/// Builder for configuring Data API client
///
/// It allows 4 concurrent in-flight requests by default.
pub struct DataApiBuilder {
    config: ClientConfig,
}

impl DataApiBuilder {
    fn new() -> Self {
        Self {
            config: ClientConfig::new(DEFAULT_BASE_URL, 4),
        }
    }

    polyoxide_core::client_config_setters!(config);

    /// Build the Data API client
    pub fn build(self) -> Result<DataApi, DataError> {
        let runtime = self.config.runtime_or_default();
        let http_client = self
            .config
            .http_builder()
            .runtime(runtime.clone())
            .with_rate_limiter(shared::data_limits_on(runtime))
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;

        Ok(DataApi { http_client })
    }
}

impl Default for DataApiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_core::RetryConfig;

    #[test]
    fn the_builders_runtime_is_every_hosts() {
        let runtime: std::sync::Arc<dyn polyoxide_rt::Runtime> =
            std::sync::Arc::new(polyoxide_rt::TokioRuntime);
        let data = DataApi::builder().runtime(runtime.clone()).build().unwrap();
        assert!(std::sync::Arc::ptr_eq(data.http_client.runtime(), &runtime));
    }

    #[test]
    fn test_builder_default() {
        let builder = DataApiBuilder::default();
        assert_eq!(builder.config.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn test_builder_custom_retry_config() {
        let config = RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 1000,
            max_backoff_ms: 30_000,
        };
        let builder = DataApiBuilder::new().with_retry_config(config);
        let config = builder.config.retry_config.unwrap();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_backoff_ms, 1000);
    }

    #[test]
    fn test_builder_custom_max_concurrent() {
        let builder = DataApiBuilder::new().max_concurrent(10);
        assert_eq!(builder.config.max_concurrent, Some(10));
    }

    #[tokio::test]
    async fn test_default_concurrency_limit_is_4() {
        let data = DataApi::new().unwrap();
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(data.http_client.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            data.http_client.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "5th permit should block with default limit of 4"
        );
    }

    #[test]
    fn test_builder_build_success() {
        let data = DataApi::builder().build();
        assert!(data.is_ok());
    }
}
