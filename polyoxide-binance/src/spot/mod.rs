//! # Spot
//!
//! Client for Binance spot public market data (`api.binance.com`), behind the
//! `spot` feature, which is on by default. No credentials are needed.
//!
//! Every request is charged against a [`WeightBudget`], because Binance limits
//! each IP by request *weight*, which varies by route and parameters, rather
//! than by request count. The budget is 5400 of the published 6000 per minute;
//! the venue's `RAW_REQUESTS` limit is not modelled, because every request
//! weighs at least 1 and the budget therefore caps a client at 27,000 requests
//! per five minutes, under that limit's 300,000.
//!
//! ```no_run
//! use polyoxide_binance::spot::Spot;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let spot = Spot::new()?;
//! let time = spot.health().time().send().await?;
//! println!("server time {}", time.server_time);
//! # Ok(())
//! # }
//! ```

pub mod api;
pub mod error;
mod policy;
pub mod request;
pub mod types;
pub mod weight;

pub use error::SpotError;
pub use weight::WeightBudget;

use polyoxide_core::{ClientConfig, HttpClient};

use crate::spot::{api::health::Health, policy::SpotRetryPolicy};

/// Production spot REST host.
pub const DEFAULT_BASE_URL: &str = "https://api.binance.com";

/// In-flight requests the client allows by default, as in the sibling crates.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for Binance spot public market data. No credentials are needed.
#[derive(Debug, Clone)]
pub struct Spot {
    http: HttpClient,
    budget: WeightBudget,
}

impl Spot {
    /// A client with default settings and its own [`WeightBudget`].
    pub fn new() -> Result<Self, SpotError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> SpotBuilder {
        SpotBuilder::new()
    }

    polyoxide_core::namespaces! { http;
        /// Liveness: `ping`, `time`.
        health: Health,
    }

    /// The budget this client charges.
    pub fn weight_budget(&self) -> &WeightBudget {
        &self.budget
    }
}

/// Builder for [`Spot`].
///
/// It allows [`DEFAULT_MAX_CONCURRENT`] in-flight requests unless told
/// otherwise. Keep the weight in flight under the budget's reserve of 600: at
/// the default, one client has at most 320 in flight on a route at its default
/// (4 × `tickers_24h`'s 80), and four `depth` calls with a `limit` above 1000
/// (250 each, 1000 in all) pass it. `WeightBudget` explains why a request in
/// flight across a minute boundary is not counted by it.
#[derive(Debug)]
pub struct SpotBuilder {
    config: ClientConfig,
    budget: Option<WeightBudget>,
}

impl SpotBuilder {
    fn new() -> Self {
        Self {
            config: ClientConfig::new(DEFAULT_BASE_URL, DEFAULT_MAX_CONCURRENT),
            budget: None,
        }
    }

    polyoxide_core::client_config_setters!(config);

    /// Charge this budget instead of a new one. Binance limits weight per IP,
    /// so every client in a process should share one budget.
    pub fn weight_budget(mut self, budget: WeightBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Build the client.
    ///
    /// It has no core `RateLimiter`: the [`WeightBudget`] is its throttle, and
    /// paces every request instead. Its retry policy holds that budget on a
    /// `429` or a `418`, so every client sharing it waits.
    pub fn build(self) -> Result<Spot, SpotError> {
        let budget = self.budget.unwrap_or_default();
        let http = self
            .config
            .http_builder()
            .with_throttle(budget.clone())
            .with_retry_policy(SpotRetryPolicy {
                budget: budget.clone(),
            })
            .build()?;
        Ok(Spot { http, budget })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_targets_the_production_host() {
        let spot = Spot::new().expect("client builds");
        assert_eq!(spot.http.base_url.as_str(), "https://api.binance.com/");
    }

    #[test]
    fn a_bad_base_url_is_a_url_error() {
        let err = Spot::builder().base_url("not a url").build().unwrap_err();
        assert!(matches!(
            err,
            SpotError::Api(polyoxide_core::ApiError::Url(_))
        ));
    }

    #[test]
    fn clients_given_one_budget_share_it() {
        let budget = WeightBudget::new();
        let a = Spot::builder()
            .weight_budget(budget.clone())
            .build()
            .unwrap();
        let b = Spot::builder().weight_budget(budget).build().unwrap();
        assert!(a.weight_budget().is_shared_with(b.weight_budget()));
        let c = Spot::new().unwrap();
        assert!(!a.weight_budget().is_shared_with(c.weight_budget()));
    }

    #[tokio::test]
    async fn test_default_concurrency_limit_is_4() {
        let spot = Spot::new().unwrap();
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(spot.http.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            spot.http.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "5th permit should block with default limit of 4"
        );
    }
}
