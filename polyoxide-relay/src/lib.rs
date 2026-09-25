//! # polyoxide-relay
//!
//! Gasless transaction relay client for Polymarket's Safe, Proxy and Deposit Wallet
//! infrastructure.
//!
//! This crate enables submitting on-chain transactions through Polymarket's relayer service,
//! which pays gas fees on behalf of users. It supports three wallet types:
//!
//! - **Safe wallets** — Gnosis Safe multisig contracts (must be deployed before first use)
//! - **Proxy wallets** — lightweight proxy contracts that auto-deploy on first transaction
//! - **Deposit Wallets** — Polymarket's smart account (default since 2026-05-04); an owner can authorize *session keys* that trade but cannot withdraw
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
//!
//! ## Deposit Wallets and session keys
//!
//! With the owner's key in the process, authorize a session key and wait for the
//! relayer to confirm it:
//!
//! ```no_run
//! use polyoxide_core::SessionSignerScope;
//! use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient, WalletType};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = BuilderConfig::new("key".into(), "secret".into(), Some("passphrase".into()));
//! let owner = BuilderAccount::new("0xprivatekey...", Some(config))?;
//! let wallet = "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".parse()?;
//! let client = RelayClient::builder()?
//!     .with_account(owner)
//!     .wallet_type(WalletType::DepositWallet)
//!     .deposit_wallet(wallet)
//!     .build()?;
//!
//! let session_key = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse()?;
//! let submitted = client
//!     .authorize_session_signer(session_key, vec![SessionSignerScope::Clob])
//!     .await?;
//! loop {
//!     let tx = client.get_gasless_transaction(&submitted.transaction_id).await?;
//!     if tx.state.is_terminal() {
//!         break;
//!     }
//!     tokio::time::sleep(std::time::Duration::from_secs(2)).await;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! With the owner's key in an external wallet, the client only submits. It needs
//! Builder credentials but no key:
//!
//! ```no_run
//! use polyoxide_core::SessionSignerScope;
//! use polyoxide_relay::{AuthConfig, BuilderConfig, RelayClient, WalletType};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let auth = AuthConfig::Builder(BuilderConfig::new("key".into(), "secret".into(), None));
//! let client = RelayClient::builder()?.with_auth(auth).build()?;
//! let owner = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse()?;
//! let wallet = "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".parse()?;
//! let session_key = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse()?;
//!
//! let nonce = client.get_execute_params(owner, WalletType::DepositWallet).await?;
//! let deadline = polyoxide_core::current_timestamp() + polyoxide_relay::DEFAULT_BATCH_DEADLINE_SECS;
//! let (typed_data, request) = client.authorize_session_signer_typed_data(
//!     wallet, session_key, vec![SessionSignerScope::Clob], nonce, deadline,
//! )?;
//! // Hand `typed_data` to the wallet (`eth_signTypedData_v4`) and get its signature back.
//! # let signature = String::new();
//! let submitted = client
//!     .submit_session_signer_authorization(&request, &signature, "my-idempotency-key")
//!     .await?;
//! # let _ = submitted;
//! # Ok(())
//! # }
//! ```
//!
//! Four things behave differently from the Safe and Proxy paths:
//!
//! - **Auth.** Authorizing a session signer needs Builder HMAC credentials. Revoking
//!   one also accepts a Relayer API key, as py-sdk's `_require_gasless_api_key` does.
//! - **Timeouts.** Both session-signer routes wait up to
//!   [`SESSION_SIGNER_REQUEST_TIMEOUT`] (300 s), because the venue validates,
//!   simulates, persists and broadcasts the batch before it answers.
//! - **Retries.** polyoxide retries only `429`. py-sdk also retries `5xx` and
//!   transport errors twice with the same `Idempotency-Key`. A caller who needs that
//!   should use the two-step API above, typed data then submit, with an idempotency
//!   key of its own that it reuses on every attempt.
//! - **Hardware wallets.** Ledger and Trezor signers refuse `sign_hash`, so they
//!   cannot sign a Deposit Wallet batch inside this client. Hand them the typed data
//!   (`eth_signTypedData_v4`) and submit the signature instead.

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod client;
mod config;
mod error;
mod types;

pub use client::{RelayClient, RelayClientBuilder};
pub use config::{AuthConfig, BuilderConfig, ContractConfig, RelayerApiKeyConfig};
pub use deposit_wallet::{
    DepositWalletCall, DEFAULT_BATCH_DEADLINE_SECS, SESSION_KEY_LIFETIME_SECS,
};
pub use error::RelayError;
pub use polyoxide_core::{DepositWalletRole, SessionSignerScope};
pub use session_signers::{
    validate_scopes, SessionSignerAuthorization, SessionSignerAuthorizationBody,
    SessionSignerAuthorizationResponse, SessionSignerAuthorizationStatus, SessionSignerRevocation,
    SessionSignerRevocationBody, SessionSignerRevocationResponse, SessionSignerRevocationStatus,
    SESSION_SIGNER_REQUEST_TIMEOUT,
};
pub use types::{
    GaslessTransaction, RelayerApiKey, RelayerTransaction, SafeTransaction, SafeTx, SubmitResponse,
    TransactionRequest, TransactionState, WalletType,
};
pub use wallet::{
    derive_deposit_wallet_beacon, derive_deposit_wallet_uups, derive_proxy, derive_safe, WalletKind,
};

pub mod deposit_wallet;
pub mod session_signers;
pub mod wallet;

mod account;

pub use account::{BuilderAccount, DynSigner};

#[cfg(feature = "keychain")]
pub use account::{
    save_builder_config_to_keychain, save_private_key_to_keychain, KEYCHAIN_SERVICE,
};
