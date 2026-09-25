//! Account module for credential management and signing operations.
//!
//! This module provides a unified abstraction for managing Polymarket CLOB authentication,
//! including wallet management, API credentials, and signing operations.

mod credentials;
mod signer;
mod target;
mod wallet;

use std::path::Path;

use alloy::primitives::Address;
pub use credentials::Credentials;
use serde::{Deserialize, Serialize};
pub use signer::Signer;
pub use target::{DepositWalletRole, SigningTarget};
pub use wallet::{DynSigner, Wallet};

use crate::{
    core::eip712::{sign_clob_auth, sign_order_as},
    error::ClobError,
    types::{Order, SignedOrder},
};

/// Environment variable names for account configuration
pub mod env {
    /// Environment variable holding the hex-encoded private key.
    pub const PRIVATE_KEY: &str = "POLYMARKET_PRIVATE_KEY";
    /// Environment variable holding the L2 API key.
    pub const API_KEY: &str = "POLYMARKET_API_KEY";
    /// Environment variable holding the L2 API secret (base64 encoded).
    pub const API_SECRET: &str = "POLYMARKET_API_SECRET";
    /// Environment variable holding the L2 API passphrase.
    pub const API_PASSPHRASE: &str = "POLYMARKET_API_PASSPHRASE";
}

/// Keychain service name for CLOB credentials.
#[cfg(feature = "keychain")]
pub const KEYCHAIN_SERVICE: &str = "polyoxide-clob";

/// Account configuration for file-based loading
#[derive(Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    pub private_key: String,
    #[serde(flatten)]
    pub credentials: Credentials,
}

impl std::fmt::Debug for AccountConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountConfig")
            .field("private_key", &"[REDACTED]")
            .field("credentials", &self.credentials)
            .finish()
    }
}

/// Unified account primitive for credential management and signing operations.
///
/// `Account` combines wallet (private key), API credentials, and signing capabilities
/// into a single abstraction. It provides factory methods for loading credentials from
/// various sources (environment variables, files) and handles both EIP-712 order signing
/// and HMAC-based L2 API authentication.
///
/// # Example
///
/// ```no_run
/// use polyoxide_clob::Account;
///
/// // Load from environment variables
/// let account = Account::from_env()?;
///
/// // Or load from a JSON file
/// let account = Account::from_file("config/account.json")?;
///
/// // Get the wallet address
/// println!("Address: {:?}", account.address());
/// # Ok::<(), polyoxide_clob::ClobError>(())
/// ```
#[derive(Clone, Debug)]
pub struct Account {
    wallet: Wallet,
    credentials: Credentials,
    signer: Signer,
    target: SigningTarget,
}

impl Account {
    /// Create a new account from private key and credentials.
    ///
    /// # Arguments
    ///
    /// * `private_key` - Hex-encoded private key (with or without 0x prefix)
    /// * `credentials` - API credentials for L2 authentication
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::{Account, Credentials};
    ///
    /// let credentials = Credentials {
    ///     key: "api_key".to_string(),
    ///     secret: "api_secret".to_string(),
    ///     passphrase: "passphrase".to_string(),
    /// };
    ///
    /// let account = Account::new("0x...", credentials)?;
    /// # Ok::<(), polyoxide_clob::ClobError>(())
    /// ```
    pub fn new(
        private_key: impl Into<String>,
        credentials: Credentials,
    ) -> Result<Self, ClobError> {
        Ok(Self::from_parts(
            Wallet::from_private_key(&private_key.into())?,
            credentials,
        ))
    }

    /// Create an account around any `alloy` signer.
    ///
    /// Use this when the key lives outside the process: a KMS or any other
    /// [`alloy::signers::Signer`] implementation that supports `sign_hash`.
    pub fn with_signer<S>(signer: S, credentials: Credentials) -> Self
    where
        S: alloy::signers::Signer + Send + Sync + 'static,
    {
        Self::from_parts(Wallet::from_signer(signer), credentials)
    }

    /// Create an account with L2 credentials and no signing key.
    ///
    /// The holder of an API-key triplet can read, cancel and post already
    /// signed orders, but cannot create or sign orders or the L1 auth message.
    /// `address` is the EOA the triplet was derived for.
    pub fn l2_only(address: Address, credentials: Credentials) -> Self {
        Self::from_parts(Wallet::l2_only(address), credentials)
    }

    /// Assemble an account from its signing half and its L2 credentials.
    fn from_parts(wallet: Wallet, credentials: Credentials) -> Self {
        let signer = Signer::new(&credentials.secret);
        Self {
            wallet,
            credentials,
            signer,
            target: SigningTarget::default(),
        }
    }

    /// Set what this account signs orders for.
    ///
    /// Defaults to [`SigningTarget::Eoa`]. A Deposit Wallet target makes
    /// [`Account::sign_order`] produce the ERC-7739 envelope the venue requires
    /// for `signatureType` 3.
    pub fn with_target(mut self, target: SigningTarget) -> Self {
        self.target = target;
        self
    }

    /// What this account signs orders for.
    pub fn target(&self) -> SigningTarget {
        self.target
    }

    /// Load account from environment variables.
    ///
    /// Reads the following environment variables:
    /// - `POLYMARKET_PRIVATE_KEY`: Hex-encoded private key
    /// - `POLYMARKET_API_KEY`: API key
    /// - `POLYMARKET_API_SECRET`: API secret (base64 encoded)
    /// - `POLYMARKET_API_PASSPHRASE`: API passphrase
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::Account;
    ///
    /// let account = Account::from_env()?;
    /// # Ok::<(), polyoxide_clob::ClobError>(())
    /// ```
    pub fn from_env() -> Result<Self, ClobError> {
        let private_key = std::env::var(env::PRIVATE_KEY).map_err(|_| {
            ClobError::validation(format!(
                "Missing environment variable: {}",
                env::PRIVATE_KEY
            ))
        })?;

        let credentials = Credentials {
            key: std::env::var(env::API_KEY).map_err(|_| {
                ClobError::validation(format!("Missing environment variable: {}", env::API_KEY))
            })?,
            secret: std::env::var(env::API_SECRET).map_err(|_| {
                ClobError::validation(format!("Missing environment variable: {}", env::API_SECRET))
            })?,
            passphrase: std::env::var(env::API_PASSPHRASE).map_err(|_| {
                ClobError::validation(format!(
                    "Missing environment variable: {}",
                    env::API_PASSPHRASE
                ))
            })?,
        };

        Self::new(private_key, credentials)
    }

    /// Load account from a JSON configuration file.
    ///
    /// The file should contain:
    /// ```json
    /// {
    ///     "private_key": "0x...",
    ///     "key": "api_key",
    ///     "secret": "api_secret",
    ///     "passphrase": "passphrase"
    /// }
    /// ```
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::Account;
    ///
    /// let account = Account::from_file("config/account.json")?;
    /// # Ok::<(), polyoxide_clob::ClobError>(())
    /// ```
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ClobError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            ClobError::validation(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        Self::from_json(&content)
    }

    /// Load account from a JSON string.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::Account;
    ///
    /// let json = r#"{
    ///     "private_key": "0x...",
    ///     "key": "api_key",
    ///     "secret": "api_secret",
    ///     "passphrase": "passphrase"
    /// }"#;
    ///
    /// let account = Account::from_json(json)?;
    /// # Ok::<(), polyoxide_clob::ClobError>(())
    /// ```
    pub fn from_json(json: &str) -> Result<Self, ClobError> {
        let config: AccountConfig = serde_json::from_str(json)
            .map_err(|e| ClobError::validation(format!("Failed to parse JSON config: {}", e)))?;

        Self::new(config.private_key, config.credentials)
    }

    /// Load account from the OS keychain.
    ///
    /// Reads from the `polyoxide-clob` keychain service:
    /// - `private_key`: Hex-encoded private key
    /// - `api_key`: API key
    /// - `api_secret`: API secret (base64 encoded)
    /// - `api_passphrase`: API passphrase
    ///
    /// Requires the `keychain` crate feature.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::Account;
    ///
    /// let account = Account::from_keychain()?;
    /// # Ok::<(), polyoxide_clob::ClobError>(())
    /// ```
    #[cfg(feature = "keychain")]
    pub fn from_keychain() -> Result<Self, ClobError> {
        Self::from_keychain_in_service(KEYCHAIN_SERVICE)
    }

    /// Implementation of [`Account::from_keychain`] parameterized by service
    /// name. Tests pass an isolated service so they never read the real
    /// `polyoxide-clob` entries.
    #[cfg(feature = "keychain")]
    fn from_keychain_in_service(service: &str) -> Result<Self, ClobError> {
        use polyoxide_core::keychain;

        let private_key = keychain::get(service, "private_key")
            .map_err(|e| ClobError::validation(format!("Keychain error for private_key: {e}")))?;

        let credentials = Credentials {
            key: keychain::get(service, "api_key")
                .map_err(|e| ClobError::validation(format!("Keychain error for api_key: {e}")))?,
            secret: keychain::get(service, "api_secret").map_err(|e| {
                ClobError::validation(format!("Keychain error for api_secret: {e}"))
            })?,
            passphrase: keychain::get(service, "api_passphrase").map_err(|e| {
                ClobError::validation(format!("Keychain error for api_passphrase: {e}"))
            })?,
        };

        Self::new(private_key, credentials)
    }

    /// Save L2 API credentials to the OS keychain.
    ///
    /// Stores `api_key`, `api_secret`, and `api_passphrase` in the `polyoxide-clob`
    /// keychain service. Does **not** store the private key (it is discarded after
    /// parsing during construction). Use [`save_private_key_to_keychain`] to store
    /// the private key before constructing an `Account`.
    ///
    /// Requires the `keychain` crate feature.
    #[cfg(feature = "keychain")]
    pub fn save_to_keychain(&self) -> Result<(), ClobError> {
        self.save_to_keychain_in_service(KEYCHAIN_SERVICE)
    }

    /// Implementation of [`Account::save_to_keychain`] parameterized by service
    /// name. Tests pass an isolated service so they never overwrite the real
    /// `polyoxide-clob` entries.
    #[cfg(feature = "keychain")]
    fn save_to_keychain_in_service(&self, service: &str) -> Result<(), ClobError> {
        use polyoxide_core::keychain;

        keychain::set(service, "api_key", &self.credentials.key)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        keychain::set(service, "api_secret", &self.credentials.secret)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        keychain::set(service, "api_passphrase", &self.credentials.passphrase)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        Ok(())
    }

    /// Delete L2 API credentials and private key from the OS keychain.
    ///
    /// Requires the `keychain` crate feature.
    #[cfg(feature = "keychain")]
    pub fn delete_from_keychain() -> Result<(), ClobError> {
        Self::delete_from_keychain_in_service(KEYCHAIN_SERVICE)
    }

    /// Implementation of [`Account::delete_from_keychain`] parameterized by
    /// service name. Tests pass an isolated service so they never delete the
    /// real `polyoxide-clob` entries.
    #[cfg(feature = "keychain")]
    fn delete_from_keychain_in_service(service: &str) -> Result<(), ClobError> {
        use polyoxide_core::keychain;

        for key in ["private_key", "api_key", "api_secret", "api_passphrase"] {
            keychain::delete(service, key)
                .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        }
        Ok(())
    }

    /// Get the wallet address.
    pub fn address(&self) -> Address {
        self.wallet.address()
    }

    /// Get a reference to the wallet.
    pub fn wallet(&self) -> &Wallet {
        &self.wallet
    }

    /// Get a reference to the credentials.
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    /// Get a reference to the HMAC signer.
    pub fn signer(&self) -> &Signer {
        &self.signer
    }

    /// Sign an order using EIP-712.
    ///
    /// For a [`SigningTarget::DepositWallet`] the signature is the ERC-7739
    /// envelope (plus the session-signer wrapper for a session key).
    ///
    /// # Arguments
    ///
    /// * `order` - The unsigned order to sign
    /// * `chain_id` - The chain ID for EIP-712 domain
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::{Account, Order};
    ///
    /// async fn example(account: &Account, order: &Order) -> Result<(), Box<dyn std::error::Error>> {
    ///     let signed_order = account.sign_order(order, 137).await?;
    ///     println!("Signature: {}", signed_order.signature);
    ///     Ok(())
    /// }
    /// ```
    pub async fn sign_order(&self, order: &Order, chain_id: u64) -> Result<SignedOrder, ClobError> {
        let signature = sign_order_as(order, self.wallet.signer()?, chain_id, &self.target).await?;

        Ok(SignedOrder {
            order: order.clone(),
            signature,
        })
    }

    /// Sign a CLOB authentication message for API key creation (L1 auth).
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The chain ID for EIP-712 domain
    /// * `timestamp` - Unix timestamp in seconds
    /// * `nonce` - Random nonce value
    pub async fn sign_clob_auth(
        &self,
        chain_id: u64,
        timestamp: u64,
        nonce: u32,
    ) -> Result<String, ClobError> {
        sign_clob_auth(self.wallet.signer()?, chain_id, timestamp, nonce).await
    }

    /// Sign an L2 API request message using HMAC.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - Unix timestamp in seconds
    /// * `method` - HTTP method (GET, POST, DELETE)
    /// * `path` - Request path (e.g., "/order")
    /// * `body` - Optional request body
    pub fn sign_l2_request(
        &self,
        timestamp: u64,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<String, ClobError> {
        let message = Signer::create_message(timestamp, method, path, body);
        self.signer.sign(&message)
    }
}

/// Save a private key to the OS keychain under the `polyoxide-clob` service.
///
/// Call this before [`Account::new`] if you want the private key persisted in the
/// keychain, since `Account` discards the raw key string after parsing.
///
/// Requires the `keychain` crate feature.
#[cfg(feature = "keychain")]
pub fn save_private_key_to_keychain(private_key: &str) -> Result<(), ClobError> {
    save_private_key_to_keychain_in_service(KEYCHAIN_SERVICE, private_key)
}

/// Implementation of [`save_private_key_to_keychain`] parameterized by service
/// name. Tests pass an isolated service so they never overwrite the real
/// `polyoxide-clob` private key.
#[cfg(feature = "keychain")]
fn save_private_key_to_keychain_in_service(
    service: &str,
    private_key: &str,
) -> Result<(), ClobError> {
    polyoxide_core::keychain::set(service, "private_key", private_key)
        .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_json() {
        let json = r#"{
            "private_key": "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            "key": "test_key",
            "secret": "c2VjcmV0",
            "passphrase": "test_pass"
        }"#;

        let account = Account::from_json(json).unwrap();
        assert_eq!(account.credentials().key, "test_key");
        assert_eq!(account.credentials().passphrase, "test_pass");
    }

    #[test]
    fn test_account_config_debug_redacts_private_key() {
        let config = AccountConfig {
            private_key: "0xdeadbeef_super_secret_key".to_string(),
            credentials: Credentials {
                key: "api_key".to_string(),
                secret: "api_secret".to_string(),
                passphrase: "pass".to_string(),
            },
        };
        let debug_output = format!("{:?}", config);
        assert!(
            debug_output.contains("[REDACTED]"),
            "Debug should contain [REDACTED], got: {debug_output}"
        );
        assert!(
            !debug_output.contains("deadbeef"),
            "Debug should not contain the private key, got: {debug_output}"
        );
    }

    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        // Each test uses its OWN isolated keychain service (never the real
        // `polyoxide-clob` service), so it can neither read, overwrite, nor
        // delete a developer's stored credentials. Because no two tests share a
        // service, they also can't clobber each other's entries — making them
        // safe to run concurrently without serialization.
        #[test]
        #[ignore] // Requires OS keychain daemon — run locally with `-- --ignored`
        fn keychain_roundtrip() {
            const SERVICE: &str = "polyoxide-clob-test-account-roundtrip";

            let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
            let credentials = Credentials {
                key: "test_keychain_key".to_string(),
                secret: "c2VjcmV0".to_string(),
                passphrase: "test_keychain_pass".to_string(),
            };

            save_private_key_to_keychain_in_service(SERVICE, private_key).unwrap();
            let account = Account::new(private_key, credentials).unwrap();
            account.save_to_keychain_in_service(SERVICE).unwrap();

            // Load back
            let loaded = Account::from_keychain_in_service(SERVICE).unwrap();
            assert_eq!(loaded.credentials().key, "test_keychain_key");
            assert_eq!(loaded.credentials().secret, "c2VjcmV0");
            assert_eq!(loaded.credentials().passphrase, "test_keychain_pass");
            assert_eq!(loaded.address(), account.address());

            // Cleanup
            Account::delete_from_keychain_in_service(SERVICE).unwrap();
        }

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn keychain_delete_removes_all_entries() {
            const SERVICE: &str = "polyoxide-clob-test-account-delete";

            let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
            let credentials = Credentials {
                key: "del_test_key".to_string(),
                secret: "c2VjcmV0".to_string(),
                passphrase: "del_test_pass".to_string(),
            };

            // Store then delete
            save_private_key_to_keychain_in_service(SERVICE, private_key).unwrap();
            Account::new(private_key, credentials)
                .unwrap()
                .save_to_keychain_in_service(SERVICE)
                .unwrap();
            Account::delete_from_keychain_in_service(SERVICE).unwrap();

            // Verify all entries are gone
            let err = Account::from_keychain_in_service(SERVICE).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("Keychain entry not found"),
                "Expected NotFound after delete, got: {msg}"
            );
        }
    }

    #[test]
    fn test_sign_l2_request() {
        let json = r#"{
            "private_key": "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            "key": "test_key",
            "secret": "c2VjcmV0",
            "passphrase": "test_pass"
        }"#;

        let account = Account::from_json(json).unwrap();
        let signature = account
            .sign_l2_request(1234567890, "GET", "/api/test", None)
            .unwrap();

        // Should be URL-safe base64
        assert!(!signature.contains('+'));
        assert!(!signature.contains('/'));
    }

    const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    const TEST_ADDR: alloy::primitives::Address =
        alloy::primitives::address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
    const DW: alloy::primitives::Address =
        alloy::primitives::address!("57ffbc34de23124faeb8387fcd689d314e57accd");

    fn creds() -> Credentials {
        Credentials {
            key: "k".into(),
            secret: "c2VjcmV0".into(),
            passphrase: "p".into(),
        }
    }

    #[test]
    fn new_account_targets_its_own_eoa() {
        let account = Account::new(TEST_KEY, creds()).unwrap();
        assert_eq!(account.target(), SigningTarget::Eoa);
        assert_eq!(account.address(), TEST_ADDR);
    }

    #[test]
    fn with_signer_accepts_an_alloy_signer() {
        let signer: alloy::signers::local::PrivateKeySigner = TEST_KEY.parse().unwrap();
        let account = Account::with_signer(signer, creds());
        assert_eq!(account.address(), TEST_ADDR);
        assert!(account.wallet().has_signer());
    }

    #[test]
    fn with_target_records_the_deposit_wallet() {
        let account =
            Account::new(TEST_KEY, creds())
                .unwrap()
                .with_target(SigningTarget::DepositWallet {
                    wallet: DW,
                    role: DepositWalletRole::SessionKey,
                });
        assert_eq!(
            account.target().deposit_wallet(),
            Some((DW, DepositWalletRole::SessionKey))
        );
        // The signing EOA is unchanged; only what it signs for moved.
        assert_eq!(account.address(), TEST_ADDR);
    }

    #[tokio::test]
    async fn l2_only_account_signs_hmac_but_not_eip712() {
        let account = Account::l2_only(TEST_ADDR, creds());
        assert_eq!(account.address(), TEST_ADDR);
        assert!(!account.wallet().has_signer());
        assert!(account.sign_l2_request(1, "GET", "/x", None).is_ok());
        let err = account
            .sign_clob_auth(137, 1, 0)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("L2-only"), "{err}");
    }

    #[tokio::test]
    async fn sign_order_uses_the_deposit_wallet_target() {
        let account =
            Account::new(TEST_KEY, creds())
                .unwrap()
                .with_target(SigningTarget::DepositWallet {
                    wallet: DW,
                    role: DepositWalletRole::Owner,
                });
        let order = Order {
            salt: "1".into(),
            maker: DW,
            signer: DW,
            token_id: "1".into(),
            maker_amount: "1000000".into(),
            taker_amount: "500000".into(),
            side: crate::types::OrderSide::Buy,
            expiration: "0".into(),
            signature_type: crate::types::SignatureType::Poly1271,
            timestamp: "0".into(),
            metadata: alloy::primitives::B256::ZERO,
            builder: alloy::primitives::B256::ZERO,
            neg_risk: false,
        };
        let signed = account.sign_order(&order, 137).await.unwrap();
        // Anvil key #0 on py-sdk's golden fixture against the V2 exchange: the
        // full 317-byte owner signature from tests/fixtures/session_keys/order_vectors.json.
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/session_keys/order_vectors.json"
        ))
        .unwrap();
        assert_eq!(
            signed.signature,
            vectors["v2_exchange"]["wrapped_signature"]
                .as_str()
                .unwrap()
        );
    }
}
