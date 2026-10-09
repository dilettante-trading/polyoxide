//! Binance USDⓈ-M futures on `fapi.binance.com`.

pub mod api;
mod policy;
pub mod request;
pub mod types;
#[cfg(feature = "ws")]
pub mod ws;

use polyoxide_core::{ClientConfig, HttpClient};

use crate::{
    error::BinanceError,
    usdm::{
        api::{exchange::ExchangeApi, health::Health, market::MarketApi},
        policy::UsdmRetryPolicy,
    },
    weight::WeightBudget,
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
    pub fn new() -> Result<Self, BinanceError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> UsdmBuilder {
        UsdmBuilder::new()
    }

    /// Liveness: `ping`, `time`.
    pub fn health(&self) -> Health {
        Health {
            http: self.http.clone(),
        }
    }

    /// Reference data: `exchangeInfo`, `fundingInfo`.
    pub fn exchange(&self) -> ExchangeApi {
        ExchangeApi {
            http: self.http.clone(),
        }
    }

    /// Market data: tickers, premium index, klines, funding, open interest,
    /// trades and depth.
    pub fn market(&self) -> MarketApi {
        MarketApi {
            http: self.http.clone(),
        }
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
    pub fn build(self) -> Result<Usdm, BinanceError> {
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
            BinanceError::Api(polyoxide_core::ApiError::Url(_))
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
