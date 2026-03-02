use polyoxide_core::{
    HttpClient, HttpClientBuilder, RateLimiter, RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};

use crate::{
    api::{
        builders::BuildersApi,
        health::Health,
        holders::Holders,
        leaderboard::LeaderboardApi,
        live_volume::LiveVolumeApi,
        open_interest::OpenInterestApi,
        trades::Trades,
        users::{UserApi, UserTraded},
    },
    error::DataApiError,
};

const DEFAULT_BASE_URL: &str = "https://data-api.polymarket.com";

/// Main Data API client
#[derive(Clone)]
pub struct DataApi {
    pub(crate) http_client: HttpClient,
}

impl DataApi {
    /// Create a new Data API client with default configuration
    pub fn new() -> Result<Self, DataApiError> {
        Self::builder().build()
    }

    /// Create a builder for configuring the client
    pub fn builder() -> DataApiBuilder {
        DataApiBuilder::new()
    }

    /// Get health namespace
    pub fn health(&self) -> Health {
        Health {
            http_client: self.http_client.clone(),
        }
    }

    /// Get user namespace for user-specific operations
    pub fn user(&self, user_address: impl Into<String>) -> UserApi {
        UserApi {
            http_client: self.http_client.clone(),
            user_address: user_address.into(),
        }
    }

    /// Alias for `user()` - for backwards compatibility
    pub fn positions(&self, user_address: impl Into<String>) -> UserApi {
        self.user(user_address)
    }

    /// Get traded namespace for backwards compatibility
    pub fn traded(&self, user_address: impl Into<String>) -> Traded {
        Traded {
            user_api: self.user(user_address),
        }
    }

    /// Get trades namespace
    pub fn trades(&self) -> Trades {
        Trades {
            http_client: self.http_client.clone(),
        }
    }

    /// Get holders namespace
    pub fn holders(&self) -> Holders {
        Holders {
            http_client: self.http_client.clone(),
        }
    }

    /// Get open interest namespace
    pub fn open_interest(&self) -> OpenInterestApi {
        OpenInterestApi {
            http_client: self.http_client.clone(),
        }
    }

    /// Get live volume namespace
    pub fn live_volume(&self) -> LiveVolumeApi {
        LiveVolumeApi {
            http_client: self.http_client.clone(),
        }
    }

    /// Get builders namespace
    pub fn builders(&self) -> BuildersApi {
        BuildersApi {
            http_client: self.http_client.clone(),
        }
    }

    /// Get leaderboard namespace
    pub fn leaderboard(&self) -> LeaderboardApi {
        LeaderboardApi {
            http_client: self.http_client.clone(),
        }
    }
}

/// Builder for configuring Data API client
pub struct DataApiBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    retry_config: Option<RetryConfig>,
}

impl DataApiBuilder {
    fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            retry_config: None,
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

    /// Build the Data API client
    pub fn build(self) -> Result<DataApi, DataApiError> {
        let mut builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_rate_limiter(RateLimiter::data_default());
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        let http_client = builder.build()?;

        Ok(DataApi { http_client })
    }
}

impl Default for DataApiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper for backwards compatibility with traded() API
pub struct Traded {
    user_api: UserApi,
}

impl Traded {
    /// Get total markets traded by the user
    pub async fn get(self) -> std::result::Result<UserTraded, DataApiError> {
        self.user_api.traded().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let builder = DataApiBuilder::default();
        assert_eq!(builder.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn test_builder_custom_retry_config() {
        let config = RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 1000,
            max_backoff_ms: 30_000,
        };
        let builder = DataApiBuilder::new().with_retry_config(config);
        let config = builder.retry_config.unwrap();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_backoff_ms, 1000);
    }

    #[test]
    fn test_builder_build_success() {
        let data = DataApi::builder().build();
        assert!(data.is_ok());
    }
}
