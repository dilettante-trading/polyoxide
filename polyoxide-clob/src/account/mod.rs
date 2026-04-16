//! Account module for credential management and signing operations.
//!
//! This module provides a unified abstraction for managing Polymarket CLOB authentication,
//! including wallet management, API credentials, and signing operations.

mod credentials;
mod signer;
mod wallet;

use std::path::Path;

use alloy::primitives::Address;
pub use credentials::Credentials;
use serde::{Deserialize, Serialize};
pub use signer::Signer;
pub use wallet::Wallet;

use crate::{
    core::eip712::{sign_clob_auth, sign_order},
    error::ClobError,
    types::{Order, SignedOrder},
};

/// Environment variable names for account configuration
pub mod env {
    pub const PRIVATE_KEY: &str = "POLYMARKET_PRIVATE_KEY";
    pub const API_KEY: &str = "POLYMARKET_API_KEY";
    pub const API_SECRET: &str = "POLYMARKET_API_SECRET";
    pub const API_PASSPHRASE: &str = "POLYMARKET_API_PASSPHRASE";
}

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
        let wallet = Wallet::from_private_key(&private_key.into())?;
        let signer = Signer::new(&credentials.secret);

        Ok(Self {
            wallet,
            credentials,
            signer,
        })
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
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-clob";

        let private_key = keychain::get(SERVICE, "private_key")
            .map_err(|e| ClobError::validation(format!("Keychain error for private_key: {e}")))?;

        let credentials = Credentials {
            key: keychain::get(SERVICE, "api_key")
                .map_err(|e| ClobError::validation(format!("Keychain error for api_key: {e}")))?,
            secret: keychain::get(SERVICE, "api_secret").map_err(|e| {
                ClobError::validation(format!("Keychain error for api_secret: {e}"))
            })?,
            passphrase: keychain::get(SERVICE, "api_passphrase").map_err(|e| {
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
    #[cfg(feature = "keychain")]
    pub fn save_to_keychain(&self) -> Result<(), ClobError> {
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-clob";

        keychain::set(SERVICE, "api_key", &self.credentials.key)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        keychain::set(SERVICE, "api_secret", &self.credentials.secret)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        keychain::set(SERVICE, "api_passphrase", &self.credentials.passphrase)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
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
        let signature = sign_order(order, self.wallet.signer(), chain_id).await?;

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
        sign_clob_auth(self.wallet.signer(), chain_id, timestamp, nonce).await
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
#[cfg(feature = "keychain")]
pub fn save_private_key_to_keychain(private_key: &str) -> Result<(), ClobError> {
    polyoxide_core::keychain::set("polyoxide-clob", "private_key", private_key)
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

        #[test]
        #[ignore] // Requires OS keychain daemon — run locally with `-- --ignored`
        fn keychain_roundtrip() {
            // Store credentials
            let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
            let credentials = Credentials {
                key: "test_keychain_key".to_string(),
                secret: "c2VjcmV0".to_string(),
                passphrase: "test_keychain_pass".to_string(),
            };

            save_private_key_to_keychain(private_key).unwrap();
            let account = Account::new(private_key, credentials).unwrap();
            account.save_to_keychain().unwrap();

            // Load back
            let loaded = Account::from_keychain().unwrap();
            assert_eq!(loaded.credentials().key, "test_keychain_key");
            assert_eq!(loaded.credentials().secret, "c2VjcmV0");
            assert_eq!(loaded.credentials().passphrase, "test_keychain_pass");
            assert_eq!(loaded.address(), account.address());
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
}
