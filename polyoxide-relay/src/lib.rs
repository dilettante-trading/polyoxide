//! # polyoxide-relay
//!
//! Gasless transaction relay client for Polymarket's Safe and Proxy wallet infrastructure.
//!
//! This crate enables submitting on-chain transactions through Polymarket's relayer service,
//! which pays gas fees on behalf of users. It supports two wallet types:
//!
//! - **Safe wallets** — Gnosis Safe multisig contracts (must be deployed before first use)
//! - **Proxy wallets** — lightweight proxy contracts that auto-deploy on first transaction
//!
//! ## Authentication
//!
//! Relay operations require a private key for EIP-712 transaction signing and optional
//! builder API credentials (`BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE`)
//! for authenticated relay submission.
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide_relay::{RelayClient, BuilderAccount, BuilderConfig};
//!
//! # async fn example() -> Result<(), polyoxide_relay::RelayError> {
//! let config = BuilderConfig::new("key".into(), "secret".into(), None);
//! let account = BuilderAccount::new("0xprivatekey...", Some(config))?;
//! let client = RelayClient::from_account(account)?;
//!
//! let latency = client.ping().await?;
//! println!("Relay API latency: {}ms", latency.as_millis());
//! # Ok(())
//! # }
//! ```

mod client;
mod config;
mod error;
mod types;

pub use client::RelayClient;
pub use config::{BuilderConfig, ContractConfig};
pub use error::RelayError;
pub use types::{SafeTransaction, SafeTx, TransactionRequest, WalletType};

mod account;

pub use account::BuilderAccount;
