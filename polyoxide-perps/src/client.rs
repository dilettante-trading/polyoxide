//! The `Perps` client and its builder.

use polyoxide_core::{
    polymarket::{self, PolymarketRetryPolicy},
    HttpClient, HttpClientBuilder, RateLimiter, RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};

use crate::{
    api::{exchange::ExchangeApi, health::Health, market::MarketApi, public::PublicApi},
    error::PerpsError,
};

/// Production Perps HTTP API host.
pub const DEFAULT_BASE_URL: &str = "https://api.perpetuals.polymarket.com";

/// In-flight requests the client allows by default, matching the sibling
/// read-only crates.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for the public Perps HTTP API. No credentials are needed.
#[derive(Clone)]
pub struct Perps {
    pub(crate) http_client: HttpClient,
}

impl Perps {
    /// A client with default settings.
    pub fn new() -> Result<Self, PerpsError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> PerpsBuilder {
        PerpsBuilder::new()
    }

    /// Liveness: `ping`, `time`.
    pub fn health(&self) -> Health {
        Health {
            http_client: self.http_client.clone(),
        }
    }

    /// Reference data: exchange, assets, instruments, fees, limit tiers.
    pub fn exchange(&self) -> ExchangeApi {
        ExchangeApi {
            http_client: self.http_client.clone(),
        }
    }

    /// Market data keyed by instrument.
    pub fn market(&self) -> MarketApi {
        MarketApi {
            http_client: self.http_client.clone(),
        }
    }

    /// Public-by-address lookups: portfolio, position fills, leaderboard, invite.
    pub fn public(&self) -> PublicApi {
        PublicApi {
            http_client: self.http_client.clone(),
        }
    }
}

/// Builder for [`Perps`].
pub struct PerpsBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    rate_limiter: Option<RateLimiter>,
    retry_config: Option<RetryConfig>,
    max_concurrent: Option<usize>,
}

impl PerpsBuilder {
    fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            rate_limiter: Some(polymarket::perps_limits()),
            retry_config: None,
            max_concurrent: None,
        }
    }

    /// Override the host, for example to point at a mock server.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Request timeout in milliseconds.
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = timeout;
        self
    }

    /// Idle connections kept per host.
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Replace the rate limiter.
    pub fn with_rate_limiter(mut self, limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Replace the retry policy.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Maximum in-flight requests (default 4).
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<Perps, PerpsError> {
        let mut builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_retry_policy(PolymarketRetryPolicy)
            .with_max_concurrent(self.max_concurrent.unwrap_or(DEFAULT_MAX_CONCURRENT));
        if let Some(limiter) = self.rate_limiter {
            builder = builder.with_rate_limiter(limiter);
        }
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        Ok(Perps {
            http_client: builder.build()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_targets_the_production_host() {
        let perps = Perps::new().expect("client builds");
        assert_eq!(
            perps.http_client.base_url.as_str(),
            "https://api.perpetuals.polymarket.com/"
        );
    }

    #[test]
    fn a_bad_base_url_is_a_url_error() {
        let err = Perps::builder()
            .base_url("not a url")
            .build()
            .err()
            .expect("fails");
        assert!(matches!(
            err,
            PerpsError::Api(polyoxide_core::ApiError::Url(_))
        ));
    }
}
