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
pub use api::{
    account::{
        BalanceAllowanceResponse, BuilderTrade, ListBuilderTrades, ListBuilderTradesResponse,
        ListClobTrades, ListTradesResponse, Trade,
    },
    auth::{
        ApiKeyInfo, ApiKeyResponse, ClosedOnlyResponse, ReadonlyApiKeyResponse, ValidateKeyResponse,
    },
    health::{Health, ServerTimeResponse},
    markets::{
        BookParams, CalculatePriceResponse, LastTradePriceResponse, ListMarketsResponse,
        LiveActivityEvent, Market, MarketToken, MidpointResponse, OrderBook, OrderLevel,
        PriceResponse, SpreadResponse,
    },
    notifications::Notification,
    orders::{
        BatchCancelResponse, ListOrdersResponse, OpenOrder, OrderResponse, OrderScoringResponse,
    },
    rewards::{
        RewardEarnings, RewardMarket, RewardMarketEarning, RewardPercentages, RewardTotalEarnings,
    },
    rfq::{
        CreateRfqQuoteParams, CreateRfqRequestParams, RfqConfig, RfqPaginatedResponse, RfqQuote,
        RfqQuoteResponse, RfqRequest, RfqRequestResponse,
    },
};
pub use client::{Clob, ClobBuilder, CreateOrderParams, SignedOrderPayload};
pub use error::ClobError;
pub use types::{
    Order, OrderKind, OrderSide, ParseTickSizeError, PartialCreateOrderOptions, SignatureType,
    SignedOrder, TickSize,
};
