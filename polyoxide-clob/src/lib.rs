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
//!
//! ## Order outcomes and retries
//!
//! Polymarket returns HTTP 400 both for genuine faults (malformed payload, banned
//! address, tick-size violation) and for the *defined* kill outcomes of marketable
//! orders — a FAK that matched nothing, a FOK that could not fill in full. Those two
//! are not failures, so they get their own variants rather than collapsing into a
//! generic validation error:
//!
//! - [`ClobError::FakUnmatched`] — nothing on the book matched a Fill-And-Kill order
//! - [`ClobError::FokUnfilled`] — a Fill-Or-Kill order could not be filled entirely
//!
//! Both are deterministic: resubmitting the identical order cannot change the answer.
//! [`ClobError::is_retriable`] reports that, and is the intended input to a caller's
//! retry policy — so retriability never has to be re-derived from status codes or
//! from the venue's prose, which changes without notice.
//!
//! ```
//! use polyoxide_clob::ClobError;
//!
//! fn handle(err: ClobError) {
//!     match err {
//!         // Normal outcomes of a marketable order — report, don't retry, don't alert.
//!         ClobError::FakUnmatched { .. } | ClobError::FokUnfilled { .. } => {}
//!         // Rate limits, timeouts, connection failures, 425, and 5xx.
//!         e if e.is_retriable() => {}
//!         // Deterministic faults: auth, validation, signing.
//!         _ => {}
//!     }
//! }
//! ```
//!
//! This crate's own retry loop only ever retries `429`, so a killed order has never
//! been resent by the SDK itself.
//!
//! ## Deposit Wallets and session keys
//!
//! A Deposit Wallet is Polymarket's smart account (default since 2026-05-04). Its
//! owner can authorize a *session key* that trades but cannot withdraw. Build the
//! account for either key with a [`SigningTarget::DepositWallet`]:
//!
//! ```no_run
//! use polyoxide_clob::{Account, Credentials, DepositWalletRole, SigningTarget};
//!
//! # fn main() -> Result<(), polyoxide_clob::ClobError> {
//! let creds = Credentials { key: "k".into(), secret: "c2VjcmV0".into(), passphrase: "p".into() };
//! // Any loader works the same way: `Account::from_env()?.with_target(..)`.
//! let session = Account::new("0x...", creds)?.with_target(SigningTarget::DepositWallet {
//!     wallet: "0x57ffbc34de23124faeb8387fcd689d314e57accd".parse().unwrap(),
//!     role: DepositWalletRole::SessionKey,
//! });
//! # let _ = session;
//! # Ok(())
//! # }
//! ```
//!
//! The owner's key never enters the process. It signs the L1 auth message out of process:
//!
//! ```no_run
//! # async fn onboarding() -> Result<(), Box<dyn std::error::Error>> {
//! use polyoxide_clob::ClobBuilder;
//! let clob = ClobBuilder::new().build()?;
//! let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse()?;
//! let timestamp = polyoxide_core::current_timestamp();
//! let typed_data = clob.clob_auth_typed_data(owner, timestamp, 0);
//! // Hand `typed_data` to the wallet (`eth_signTypedData_v4`) and get its signature back.
//! # let signature = String::new();
//! let creds = clob.derive_api_key_with_signature(owner, timestamp, 0, signature).await?;
//! # let _ = creds;
//! # Ok(())
//! # }
//! ```
//!
//! `create_order` then sets `maker == signer == wallet` and `signatureType` 3, and
//! `sign_order` produces the ERC-7739 envelope. [`Account::l2_only`] holds a
//! credential triplet with no key, enough to read, cancel and list session
//! signers (`AccountApi::list_session_signers`). [`clob_auth_typed_data`] and
//! `Clob::derive_api_key_with_signature` let a key in an external wallet create
//! credentials without entering the process.

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
pub use core::eip712::clob_auth_typed_data;

pub use account::{
    Account, AccountConfig, Credentials, DepositWalletRole, DynSigner, Signer, SigningTarget,
    Wallet,
};

#[cfg(feature = "keychain")]
pub use account::{save_private_key_to_keychain, KEYCHAIN_SERVICE};
pub use api::{
    account::{
        BalanceAllowanceResponse, BuilderTrade, ListBuilderTrades, ListBuilderTradesResponse,
        ListClobTrades, ListTradesResponse, MakerOrder, SessionSigner, SessionSigners, Trade,
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
pub use polyoxide_core::SessionSignerScope;
pub use types::{
    Order, OrderKind, OrderSide, ParseTickSizeError, PartialCreateOrderOptions, SignatureType,
    SignedOrder, TickSize,
};
