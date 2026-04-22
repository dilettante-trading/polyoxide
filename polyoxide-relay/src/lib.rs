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
//! Relay operations require a private key for EIP-712 transaction signing and one of
//! two authentication schemes for relay submission:
//!
//! - **Builder API credentials** — HMAC-SHA256 signed headers (`BUILDER_API_KEY`,
//!   `BUILDER_SECRET`, `BUILDER_PASS_PHRASE`)
//! - **Relayer API keys** — static headers (`RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS`),
//!   a simpler alternative
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
pub use config::{AuthConfig, BuilderConfig, ContractConfig, RelayerApiKeyConfig};
pub use error::RelayError;
pub use types::{
    RelayerApiKey, RelayerTransaction, SafeTransaction, SafeTx, SubmitResponse, TransactionRequest,
    WalletType,
};

mod account;

pub use account::BuilderAccount;

#[cfg(feature = "keychain")]
pub use account::{
    save_builder_config_to_keychain, save_private_key_to_keychain, KEYCHAIN_SERVICE,
};
