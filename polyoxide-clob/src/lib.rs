//! # polyoxide-clob
//!
//! Rust client library for Polymarket CLOB (Centralized Limit Order Book) API.
//!
//! ## Features
//!
//! - Order creation, signing, and posting with EIP-712
//! - Market data and order book retrieval
//! - Account balance and trade history
//! - HMAC-based L2 authentication
//! - Type-safe API with idiomatic Rust patterns
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide_clob::{Account, Chain, ClobBuilder, CreateOrderParams, OrderKind, OrderSide};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load account from environment variables
//!     let account = Account::from_env()?;
//!
//!     // Create CLOB client
//!     let clob = ClobBuilder::new()
//!         .with_account(account)
//!         .chain(Chain::PolygonMainnet)
//!         .build()?;
//!
//!     // Place an order
//!     let params = CreateOrderParams {
//!         token_id: "token_id".to_string(),
//!         price: 0.52,
//!         size: 100.0,
//!         side: OrderSide::Buy,
//!         order_type: OrderKind::Gtc,
//!         post_only: false,
//!         expiration: None,
//!         funder: None,
//!         signature_type: None,
//!     };
//!
//!     let response = clob.place_order(&params, None).await?;
//!     println!("Order ID: {:?}", response.order_id);
//!
//!     Ok(())
//! }
//! ```

/// Doctest-only anchor that compiles every fenced `rust` example in the crate
/// README, so broken examples fail CI. Exists only under `cfg(doctest)`, so it
/// never appears in `cargo doc` output or normal builds.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod account;
pub mod api;
pub mod client;
pub mod core;
pub mod error;
pub mod request;
pub mod types;
pub mod utils;

#[cfg(feature = "ws")]
pub mod ws;

pub use core::chain::{Chain, Contracts};

pub use account::{Account, AccountConfig, Credentials, Signer, Wallet};

#[cfg(feature = "keychain")]
pub use account::{save_private_key_to_keychain, KEYCHAIN_SERVICE};
pub use api::{
    account::{
        BalanceAllowanceResponse, BuilderTrade, ListBuilderTrades, ListBuilderTradesResponse,
        ListClobTrades, ListTradesResponse, MakerOrder, Trade,
    },
    auth::{
        ApiKeyInfo, ApiKeyResponse, BuilderApiKeyResponse, ClosedOnlyResponse,
        ReadonlyApiKeyResponse, ValidateKeyResponse,
    },
    health::{Health, ServerTimeResponse},
    markets::{
        BatchPricesHistoryRequest, BatchPricesHistoryResponse, BookParams, CalculatePriceResponse,
        ClobMarketDetails, ClobRewards, ClobToken, FeeDetails, LastTradePriceResponse,
        ListClobMarkets, ListMarketsResponse, LiveActivityMarket, Market, MarketByTokenResponse,
        MarketPrice, MarketToken, MidpointResponse, OrderBook, OrderLevel, PriceHistoryPoint,
        PriceResponse, PricesHistoryQuery, PricesHistoryResponse, SpreadResponse,
    },
    notifications::Notification,
    orders::{
        BatchCancelResponse, ListOrders, ListOrdersResponse, OpenOrder, OrderResponse,
        OrderScoringResponse,
    },
    rewards::{
        ListMultiRewardMarkets, ListRewardMarkets, ListUserRewardMarkets, MultiMarketOrderBy,
        Paginated, PublicRewards, RebatedFees, RewardEarnings, RewardMarket, RewardMarketEarning,
        RewardMarketRequest, RewardPercentages, RewardTotalEarnings, SortPosition,
        UserEarningsRequest, UserPercentagesRequest, UserRewardMarketOrderBy,
        UserTotalEarningsRequest,
    },
};
pub use client::{Clob, ClobBuilder, CreateOrderParams, SignedOrderPayload};
pub use error::ClobError;
pub use types::{
    Order, OrderKind, OrderSide, ParseTickSizeError, PartialCreateOrderOptions, SignatureType,
    SignedOrder, TickSize,
};
