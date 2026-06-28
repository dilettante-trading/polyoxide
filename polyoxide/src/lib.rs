//! # polyoxide
//!
//! Unified Rust client for Polymarket APIs, combining CLOB (trading), Gamma (market data), and Data APIs.
//!
//! ## Features
//!
//! - Unified access to CLOB, Gamma, and Data APIs
//! - Type-safe API with idiomatic Rust patterns
//! - EIP-712 order signing and HMAC authentication
//! - Comprehensive market data and trading operations
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load account from environment variables
//!     let account = Account::from_env()?;
//!
//!     // Create unified client
//!     let polymarket = Polymarket::builder(account)
//!         .chain(Chain::PolygonMainnet)
//!         .build()?;
//!
//!     // Use Gamma API to list markets
//!     let markets = polymarket.gamma.markets()
//!         .list()
//!         .open(true)
//!         .limit(10)
//!         .send()
//!         .await?;
//!
//!     // Use Data API to get user positions
//!     let positions = polymarket.data
//!         .user("0x1234567890123456789012345678901234567890")
//!         .list_positions()
//!         .limit(10)
//!         .send()
//!         .await?;
//!
//!     for position in &positions {
//!         println!("Position: {} - size: {}", position.title, position.size);
//!     }
//!
//!     // Use CLOB API to place an order
//!     if let Some(first_market) = markets.first() {
//!         if let Some(token) = first_market.tokens.first() {
//!             let order_params = CreateOrderParams {
//!                 token_id: token.token_id.clone(),
//!                 price: 0.52,
//!                 size: 100.0,
//!                 side: OrderSide::Buy,
//!                 expiration: None,
//!                 order_type: OrderKind::Gtc,
//!                 post_only: false,
//!                 funder: None,
//!                 signature_type: Some(SignatureType::PolyProxy),
//!             };
//!
//!             let response = polymarket.clob.place_order(&order_params, None).await?;
//!             println!("Order placed: {:?}", response.order_id);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
/// Compiles the crate `README.md` code examples as doctests.
struct ReadmeDoctests;

#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
/// Compiles the workspace root `README.md` code examples as doctests.
struct RootReadmeDoctests;

#[cfg(feature = "clob")]
pub use polyoxide_clob;
#[cfg(feature = "data")]
pub use polyoxide_data;
#[cfg(feature = "gamma")]
pub use polyoxide_gamma;

#[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
use polyoxide_clob::{Account, Chain, Clob, ClobBuilder};
#[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
use polyoxide_data::{DataApi, DataApiBuilder};
#[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
use polyoxide_gamma::Gamma;

/// Prelude module for convenient imports
pub mod prelude {
    #[cfg(feature = "ws")]
    pub use polyoxide_clob::ws;
    #[cfg(feature = "clob")]
    pub use polyoxide_clob::{
        Account, Chain, Clob, ClobBuilder, ClobError, CreateOrderParams, Credentials, OrderKind,
        OrderSide, SignatureType,
    };
    #[cfg(feature = "data")]
    pub use polyoxide_data::{DataApi, DataApiError};
    #[cfg(feature = "gamma")]
    pub use polyoxide_gamma::{Gamma, GammaError};

    #[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
    pub use crate::{Polymarket, PolymarketBuilder, PolymarketError};
}

/// Error types for Polymarket operations
#[derive(Debug, thiserror::Error)]
pub enum PolymarketError {
    /// CLOB API error
    #[cfg(feature = "clob")]
    #[error("CLOB error: {0}")]
    Clob(#[from] polyoxide_clob::ClobError),

    /// Data API error
    #[cfg(feature = "data")]
    #[error("Data error: {0}")]
    Data(#[from] polyoxide_data::DataApiError),

    /// Gamma API error
    #[cfg(feature = "gamma")]
    #[error("Gamma error: {0}")]
    Gamma(#[from] polyoxide_gamma::GammaError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Unified Polymarket client
#[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
#[derive(Clone)]
pub struct Polymarket {
    /// CLOB (trading) API client
    pub clob: Clob,
    /// Gamma (market data) API client
    pub gamma: Gamma,
    /// Data API client (user positions, trades, activity)
    pub data: DataApi,
}

#[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
impl Polymarket {
    /// Create a new client from an Account
    pub fn new(account: Account) -> Result<Self, PolymarketError> {
        PolymarketBuilder::new(account).build()
    }

    /// Create a new builder with an Account
    pub fn builder(account: Account) -> PolymarketBuilder {
        PolymarketBuilder::new(account)
    }
}

/// Builder for Polymarket client
#[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
pub struct PolymarketBuilder {
    clob_base_url: Option<String>,
    gamma_base_url: Option<String>,
    data_base_url: Option<String>,
    timeout_ms: Option<u64>,
    chain: Option<Chain>,
    account: Account,
}

#[cfg(all(feature = "clob", feature = "gamma", feature = "data"))]
impl PolymarketBuilder {
    fn new(account: Account) -> Self {
        Self {
            clob_base_url: None,
            gamma_base_url: None,
            data_base_url: None,
            timeout_ms: None,
            chain: None,
            account,
        }
    }

    /// Set CLOB base URL
    pub fn clob_base_url(mut self, url: impl Into<String>) -> Self {
        self.clob_base_url = Some(url.into());
        self
    }

    /// Set Gamma base URL
    pub fn gamma_base_url(mut self, url: impl Into<String>) -> Self {
        self.gamma_base_url = Some(url.into());
        self
    }

    /// Set Data API base URL
    pub fn data_base_url(mut self, url: impl Into<String>) -> Self {
        self.data_base_url = Some(url.into());
        self
    }

    /// Set request timeout in milliseconds
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = Some(timeout);
        self
    }

    /// Set chain
    pub fn chain(mut self, chain: Chain) -> Self {
        self.chain = Some(chain);
        self
    }

    /// Build the Polymarket client
    pub fn build(self) -> Result<Polymarket, PolymarketError> {
        // Build Gamma client
        let mut gamma_builder = Gamma::builder();

        if let Some(url) = self.gamma_base_url {
            gamma_builder = gamma_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            gamma_builder = gamma_builder.timeout_ms(timeout);
        }

        let gamma = gamma_builder.build()?;

        // Build CLOB client
        let mut clob_builder = ClobBuilder::new().with_account(self.account);

        if let Some(url) = self.clob_base_url {
            clob_builder = clob_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            clob_builder = clob_builder.timeout_ms(timeout);
        }
        if let Some(chain) = self.chain {
            clob_builder = clob_builder.chain(chain);
        }

        let clob = clob_builder.build()?;

        // Build Data API client
        let mut data_builder = DataApiBuilder::default();

        if let Some(url) = self.data_base_url {
            data_builder = data_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            data_builder = data_builder.timeout_ms(timeout);
        }

        let data = data_builder.build()?;

        Ok(Polymarket { clob, gamma, data })
    }
}
