//! The `Perps` client and its builder.

use polyoxide_core::{
    polymarket::{self, PolymarketRetryPolicy},
    ClientConfig, HttpClient, RateLimiter,
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

    polyoxide_core::namespaces! { http_client;
        /// Liveness: `ping`, `time`.
        health: Health,
        /// Reference data: exchange, assets, instruments, fees, limit tiers.
        exchange: ExchangeApi,
        /// Market data keyed by instrument.
        market: MarketApi,
        /// Public-by-address lookups: portfolio, position fills, leaderboard, invite.
        public: PublicApi,
    }
}

/// Builder for [`Perps`]. It allows [`DEFAULT_MAX_CONCURRENT`] in-flight
/// requests unless told otherwise.
pub struct PerpsBuilder {
    config: ClientConfig,
    rate_limiter: Option<RateLimiter>,
}

impl PerpsBuilder {
    fn new() -> Self {
        Self {
            config: ClientConfig::new(DEFAULT_BASE_URL, DEFAULT_MAX_CONCURRENT),
            rate_limiter: Some(polymarket::perps_limits()),
        }
    }

    polyoxide_core::client_config_setters!(config);

    /// Replace the rate limiter.
    pub fn with_rate_limiter(mut self, limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<Perps, PerpsError> {
        let mut builder = self
            .config
            .http_builder()
            .with_retry_policy(PolymarketRetryPolicy);
        if let Some(limiter) = self.rate_limiter {
            builder = builder.with_rate_limiter(limiter);
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

    #[tokio::test]
    async fn test_default_concurrency_limit_is_4() {
        let perps = Perps::new().unwrap();
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(perps.http_client.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            perps.http_client.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "5th permit should block with default limit of 4"
        );
    }
}
