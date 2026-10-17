//! # USDⓈ-M
//!
//! Client for Binance USDⓈ-M futures public market data (`fapi.binance.com`),
//! behind the `usdm` feature, which is on by default. Its market streams on
//! `fstream.binance.com`, the `ws` module, are behind the `usdm-ws` feature.
//! No credentials are needed.
//!
//! Every request is charged against a [`WeightBudget`], because Binance limits
//! each IP by request *weight*, which varies by route and parameters, rather
//! than by request count.
//!
//! ```no_run
//! use polyoxide_binance::usdm::{types::Symbol, Usdm};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let usdm = Usdm::new()?;
//! let btc = Symbol::new("BTCUSDT")?;
//! let index = usdm.market().premium_index(&btc).send().await?;
//! println!("BTCUSDT mark {} funding {}", index.mark_price, index.last_funding_rate);
//! # Ok(())
//! # }
//! ```

pub mod api;
pub mod error;
mod policy;
pub mod request;
pub mod types;
pub mod weight;
#[cfg(feature = "usdm-ws")]
pub mod ws;

use polyoxide_core::{ClientConfig, HttpClient};

pub use error::UsdmError;
pub use weight::WeightBudget;

use crate::usdm::{
    api::{exchange::ExchangeApi, health::Health, market::MarketApi},
    policy::UsdmRetryPolicy,
};

/// Production USDⓈ-M futures REST host.
pub const DEFAULT_BASE_URL: &str = "https://fapi.binance.com";

/// In-flight requests the client allows by default, as in the sibling crates.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for USDⓈ-M futures public market data. No credentials are needed.
#[derive(Debug, Clone)]
pub struct Usdm {
    http: HttpClient,
    budget: WeightBudget,
}

impl Usdm {
    /// A client with default settings and its own [`WeightBudget`].
    pub fn new() -> Result<Self, UsdmError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> UsdmBuilder {
        UsdmBuilder::new()
    }

    polyoxide_core::namespaces! { http;
        /// Liveness: `ping`, `time`.
        health: Health,
        /// Reference data: `exchangeInfo`, `fundingInfo`.
        exchange: ExchangeApi,
        /// Market data: tickers, premium index, klines, funding, open interest,
        /// trades and depth.
        market: MarketApi,
    }

    /// The budget this client charges.
    pub fn weight_budget(&self) -> &WeightBudget {
        &self.budget
    }
}

/// Builder for [`Usdm`].
///
/// It allows [`DEFAULT_MAX_CONCURRENT`] in-flight requests unless told
/// otherwise. Keep the weight in flight under the budget's reserve of 240: at
/// the default, one client has at most 160 (4 × the heaviest route, 40).
/// `WeightBudget` explains why a request in flight across a minute boundary
/// is not counted by it.
#[derive(Debug)]
pub struct UsdmBuilder {
    config: ClientConfig,
    budget: Option<WeightBudget>,
}

impl UsdmBuilder {
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
    pub fn build(self) -> Result<Usdm, UsdmError> {
        let budget = self.budget.unwrap_or_default();
        let http = self
            .config
            .http_builder()
            .with_throttle(budget.clone())
            .with_retry_policy(UsdmRetryPolicy {
                budget: budget.clone(),
            })
            .build()?;
        Ok(Usdm { http, budget })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_targets_the_production_host() {
        let usdm = Usdm::new().expect("client builds");
        assert_eq!(usdm.http.base_url.as_str(), "https://fapi.binance.com/");
    }

    #[test]
    fn a_bad_base_url_is_a_url_error() {
        let err = Usdm::builder().base_url("not a url").build().unwrap_err();
        assert!(matches!(
            err,
            UsdmError::Api(polyoxide_core::ApiError::Url(_))
        ));
    }

    #[test]
    fn clients_given_one_budget_share_it() {
        let budget = WeightBudget::new();
        let a = Usdm::builder()
            .weight_budget(budget.clone())
            .build()
            .unwrap();
        let b = Usdm::builder().weight_budget(budget).build().unwrap();
        assert!(a.weight_budget().is_shared_with(b.weight_budget()));
        let c = Usdm::new().unwrap();
        assert!(!a.weight_budget().is_shared_with(c.weight_budget()));
    }

    #[tokio::test]
    async fn test_default_concurrency_limit_is_4() {
        let usdm = Usdm::new().unwrap();
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(usdm.http.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            usdm.http.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "5th permit should block with default limit of 4"
        );
    }
}
