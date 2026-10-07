//! Binance USDⓈ-M futures on `fapi.binance.com`.

pub mod api;
pub mod request;
pub mod types;
#[cfg(feature = "ws")]
pub mod ws;

use polyoxide_core::{
    HttpClient, HttpClientBuilder, RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};

use crate::{
    error::BinanceError,
    usdm::api::{exchange::ExchangeApi, health::Health, market::MarketApi},
    weight::WeightBudget,
};

pub use request::WeightedRequest;

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
            budget: self.budget.clone(),
        }
    }

    /// Reference data: `exchangeInfo`, `fundingInfo`.
    pub fn exchange(&self) -> ExchangeApi {
        ExchangeApi {
            http: self.http.clone(),
            budget: self.budget.clone(),
        }
    }

    /// Market data: tickers, premium index, klines, funding, open interest,
    /// trades and depth.
    pub fn market(&self) -> MarketApi {
        MarketApi {
            http: self.http.clone(),
            budget: self.budget.clone(),
        }
    }

    /// The budget this client charges.
    pub fn weight_budget(&self) -> &WeightBudget {
        &self.budget
    }
}

/// Builder for [`Usdm`].
#[derive(Debug)]
pub struct UsdmBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    retry_config: Option<RetryConfig>,
    max_concurrent: Option<usize>,
    budget: Option<WeightBudget>,
}

impl UsdmBuilder {
    fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            retry_config: None,
            max_concurrent: None,
            budget: None,
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

    /// Replace the retry policy for `429` responses.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Maximum in-flight requests (default 4).
    ///
    /// Keep the weight in flight under the budget's reserve of 240: at the
    /// default, one client has at most 160 (4 × the heaviest route, 40).
    /// `WeightBudget` explains why a request in flight across a minute
    /// boundary is not counted by it.
    ///
    /// At least 1: zero admits no request, so every send waits forever.
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Charge this budget instead of a new one. Binance limits weight per IP,
    /// so every client in a process should share one budget.
    pub fn weight_budget(mut self, budget: WeightBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Build the client.
    ///
    /// It asks for gzip, since `exchangeInfo` is 1.15 MB raw and 51 KB
    /// compressed, and has no core `RateLimiter`: the [`WeightBudget`] paces
    /// every request instead.
    pub fn build(self) -> Result<Usdm, BinanceError> {
        let mut builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_max_concurrent(self.max_concurrent.unwrap_or(DEFAULT_MAX_CONCURRENT))
            .gzip(true);
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        Ok(Usdm {
            http: builder.build()?,
            budget: self.budget.unwrap_or_default(),
        })
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
}
