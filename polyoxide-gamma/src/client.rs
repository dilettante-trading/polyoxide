use polyoxide_core::{
    HttpClient, HttpClientBuilder, RateLimiter, RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};

use crate::{
    api::{
        comments::Comments, events::Events, health::Health, markets::Markets, search::Search,
        series::Series, sports::Sports, tags::Tags, user::User,
    },
    error::GammaError,
};

const DEFAULT_BASE_URL: &str = "https://gamma-api.polymarket.com";

/// Main Gamma API client
#[derive(Clone)]
pub struct Gamma {
    pub(crate) http_client: HttpClient,
}

impl Gamma {
    /// Create a new Gamma client with default configuration
    pub fn new() -> Result<Self, GammaError> {
        Self::builder().build()
    }

    /// Create a builder for configuring the client
    pub fn builder() -> GammaBuilder {
        GammaBuilder::new()
    }

    /// Get markets namespace
    pub fn markets(&self) -> Markets {
        Markets {
            http_client: self.http_client.clone(),
        }
    }

    /// Get events namespace
    pub fn events(&self) -> Events {
        Events {
            http_client: self.http_client.clone(),
        }
    }

    /// Get series namespace
    pub fn series(&self) -> Series {
        Series {
            http_client: self.http_client.clone(),
        }
    }

    /// Get tags namespace
    pub fn tags(&self) -> Tags {
        Tags {
            http_client: self.http_client.clone(),
        }
    }

    /// Get sports namespace
    pub fn sports(&self) -> Sports {
        Sports {
            http_client: self.http_client.clone(),
        }
    }

    /// Get comments namespace
    pub fn comments(&self) -> Comments {
        Comments {
            http_client: self.http_client.clone(),
        }
    }

    /// Get search namespace
    pub fn search(&self) -> Search {
        Search {
            http_client: self.http_client.clone(),
        }
    }

    /// Get user namespace
    pub fn user(&self) -> User {
        User {
            http_client: self.http_client.clone(),
        }
    }

    /// Get health namespace
    pub fn health(&self) -> Health {
        Health {
            http_client: self.http_client.clone(),
        }
    }
}

/// Builder for configuring Gamma client
pub struct GammaBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    retry_config: Option<RetryConfig>,
    max_concurrent: Option<usize>,
}

impl GammaBuilder {
    fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            retry_config: None,
            max_concurrent: None,
        }
    }

    /// Set base URL for the API
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set request timeout in milliseconds
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = timeout;
        self
    }

    /// Set connection pool size
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Set retry configuration for 429 responses
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Set the maximum number of concurrent in-flight requests.
    ///
    /// Default: 4. Prevents Cloudflare 1015 errors from request bursts.
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Build the Gamma client
    pub fn build(self) -> Result<Gamma, GammaError> {
        let mut builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_rate_limiter(RateLimiter::gamma_default())
            .with_max_concurrent(self.max_concurrent.unwrap_or(4));
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        let http_client = builder.build()?;

        Ok(Gamma { http_client })
    }
}

impl Default for GammaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let builder = GammaBuilder::default();
        assert_eq!(builder.base_url, DEFAULT_BASE_URL);
        assert_eq!(builder.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(builder.pool_size, DEFAULT_POOL_SIZE);
    }

    #[test]
    fn test_builder_custom_url() {
        let builder = GammaBuilder::new().base_url("https://custom.api.com");
        assert_eq!(builder.base_url, "https://custom.api.com");
    }

    #[test]
    fn test_builder_custom_timeout() {
        let builder = GammaBuilder::new().timeout_ms(60_000);
        assert_eq!(builder.timeout_ms, 60_000);
    }

    #[test]
    fn test_builder_custom_pool_size() {
        let builder = GammaBuilder::new().pool_size(20);
        assert_eq!(builder.pool_size, 20);
    }

    #[test]
    fn test_builder_custom_retry_config() {
        let config = RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 1000,
            max_backoff_ms: 30_000,
        };
        let builder = GammaBuilder::new().with_retry_config(config);
        let config = builder.retry_config.unwrap();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_backoff_ms, 1000);
    }

    #[test]
    fn test_builder_build_success() {
        let gamma = Gamma::builder().build();
        assert!(gamma.is_ok());
    }

    #[test]
    fn test_builder_invalid_url() {
        let result = Gamma::builder().base_url("://bad").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_custom_max_concurrent() {
        let builder = GammaBuilder::new().max_concurrent(10);
        assert_eq!(builder.max_concurrent, Some(10));
    }

    #[tokio::test]
    async fn test_default_concurrency_limit_is_4() {
        let gamma = Gamma::new().unwrap();
        // Default concurrency limit is 4
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(gamma.http_client.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        // 5th should block
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            gamma.http_client.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "5th permit should block with default limit of 4"
        );
    }

    #[test]
    fn test_new_creates_client() {
        let gamma = Gamma::new();
        assert!(gamma.is_ok());
    }

    #[test]
    fn test_client_namespaces_accessible() {
        let gamma = Gamma::new().unwrap();
        let _markets = gamma.markets();
        let _events = gamma.events();
        let _series = gamma.series();
        let _tags = gamma.tags();
        let _sports = gamma.sports();
        let _comments = gamma.comments();
        let _search = gamma.search();
        let _user = gamma.user();
        let _health = gamma.health();
    }
}
