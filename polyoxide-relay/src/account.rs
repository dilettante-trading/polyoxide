use crate::config::{AuthConfig, BuilderConfig, RelayerApiKeyConfig};
use crate::error::RelayError;
use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;

/// Keychain service name for Relay credentials.
#[cfg(feature = "keychain")]
pub const KEYCHAIN_SERVICE: &str = "polyoxide-relay";

/// Account credentials for authenticated relay operations.
///
/// Combines a private key signer (for EIP-712 transaction signing) with an optional
/// [`AuthConfig`] for relay submission. Two authentication schemes are supported:
/// [`AuthConfig::Builder`] (HMAC-signed builder API credentials) and
/// [`AuthConfig::RelayerApiKey`] (static relayer API key headers). The `Debug`
/// implementation redacts the private key to prevent accidental leakage in logs.
#[derive(Clone)]
pub struct BuilderAccount {
    pub(crate) signer: PrivateKeySigner,
    pub(crate) config: Option<AuthConfig>,
}

fn parse_signer(private_key: impl Into<String>) -> Result<PrivateKeySigner, RelayError> {
    private_key
        .into()
        .parse::<PrivateKeySigner>()
        .map_err(|e| RelayError::Signer(format!("Failed to parse private key: {}", e)))
}

impl std::fmt::Debug for BuilderAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuilderAccount")
            .field("address", &self.signer.address())
            .field("config", &self.config)
            .finish()
    }
}

impl BuilderAccount {
    /// Create a new account from a hex-encoded private key and optional builder config.
    ///
    /// Wraps the `BuilderConfig` in [`AuthConfig::Builder`] internally.
    /// Accepts keys with or without a `0x` prefix.
    pub fn new(
        private_key: impl Into<String>,
        config: Option<BuilderConfig>,
    ) -> Result<Self, RelayError> {
        let signer = parse_signer(private_key)?;
        Ok(Self {
            signer,
            config: config.map(AuthConfig::Builder),
        })
    }

    /// Create a new account from a hex-encoded private key and relayer API key credentials.
    pub fn with_relayer_api_key(
        private_key: impl Into<String>,
        key: String,
        address: String,
    ) -> Result<Self, RelayError> {
        let signer = parse_signer(private_key)?;
        let relayer = RelayerApiKeyConfig::new(key, address)?;
        Ok(Self {
            signer,
            config: Some(AuthConfig::RelayerApiKey(relayer)),
        })
    }

    /// Create a new account from a hex-encoded private key and a pre-built [`AuthConfig`].
    pub fn with_auth_config(
        private_key: impl Into<String>,
        config: Option<AuthConfig>,
    ) -> Result<Self, RelayError> {
        let signer = parse_signer(private_key)?;
        Ok(Self { signer, config })
    }

    /// Returns the Ethereum address derived from the private key.
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Returns a reference to the underlying private key signer.
    pub fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }

    /// Returns the auth config, if one was provided.
    pub fn auth_config(&self) -> Option<&AuthConfig> {
        self.config.as_ref()
    }

    /// Load account from the OS keychain with builder API credentials.
    ///
    /// Reads from the `polyoxide-relay` keychain service:
    /// - `private_key`: Hex-encoded private key (required)
    /// - `api_key`, `api_secret`: Builder API credentials (optional — if `api_key` is
    ///   not found, the account is created without auth config)
    /// - `passphrase`: Builder API passphrase (optional)
    #[cfg(feature = "keychain")]
    pub fn from_keychain() -> Result<Self, RelayError> {
        use polyoxide_core::keychain;

        let private_key = keychain::get(KEYCHAIN_SERVICE, "private_key")
            .map_err(|e| RelayError::Api(format!("Keychain error for private_key: {e}")))?;

        let config = match keychain::get(KEYCHAIN_SERVICE, "api_key") {
            Ok(key) => {
                let secret = keychain::get(KEYCHAIN_SERVICE, "api_secret")
                    .map_err(|e| RelayError::Api(format!("Keychain error for api_secret: {e}")))?;
                let passphrase = keychain::get(KEYCHAIN_SERVICE, "passphrase").ok();
                Some(BuilderConfig::new(key, secret, passphrase))
            }
            Err(polyoxide_core::KeychainError::NotFound { .. }) => None,
            Err(e) => return Err(RelayError::Api(format!("Keychain error: {e}"))),
        };

        Self::new(private_key, config)
    }

    /// Load account from the OS keychain with relayer API key credentials.
    ///
    /// Reads from the `polyoxide-relay` keychain service:
    /// - `private_key`: Hex-encoded private key
    /// - `relayer_api_key`: Static relayer API key
    /// - `relayer_api_key_address`: On-chain address for the relayer API key
    #[cfg(feature = "keychain")]
    pub fn from_keychain_relayer_api_key() -> Result<Self, RelayError> {
        use polyoxide_core::keychain;

        let private_key = keychain::get(KEYCHAIN_SERVICE, "private_key")
            .map_err(|e| RelayError::Api(format!("Keychain error for private_key: {e}")))?;
        let key = keychain::get(KEYCHAIN_SERVICE, "relayer_api_key")
            .map_err(|e| RelayError::Api(format!("Keychain error for relayer_api_key: {e}")))?;
        let address = keychain::get(KEYCHAIN_SERVICE, "relayer_api_key_address").map_err(|e| {
            RelayError::Api(format!("Keychain error for relayer_api_key_address: {e}"))
        })?;

        Self::with_relayer_api_key(private_key, key, address)
    }

    /// Delete all credentials from the OS keychain for this service.
    #[cfg(feature = "keychain")]
    pub fn delete_from_keychain() -> Result<(), RelayError> {
        use polyoxide_core::keychain;

        for key in [
            "private_key",
            "api_key",
            "api_secret",
            "passphrase",
            "relayer_api_key",
            "relayer_api_key_address",
        ] {
            keychain::delete(KEYCHAIN_SERVICE, key)
                .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
        }
        Ok(())
    }
}

/// Save a private key to the OS keychain under the `polyoxide-relay` service.
#[cfg(feature = "keychain")]
pub fn save_private_key_to_keychain(private_key: &str) -> Result<(), RelayError> {
    polyoxide_core::keychain::set(KEYCHAIN_SERVICE, "private_key", private_key)
        .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
    Ok(())
}

/// Save builder API credentials to the OS keychain under the `polyoxide-relay` service.
///
/// When `config.passphrase` is `None`, any previously stored passphrase is deleted
/// to prevent stale values from persisting.
#[cfg(feature = "keychain")]
pub fn save_builder_config_to_keychain(config: &BuilderConfig) -> Result<(), RelayError> {
    use polyoxide_core::keychain;

    keychain::set(KEYCHAIN_SERVICE, "api_key", &config.key)
        .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
    keychain::set(KEYCHAIN_SERVICE, "api_secret", &config.secret)
        .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
    match &config.passphrase {
        Some(passphrase) => {
            keychain::set(KEYCHAIN_SERVICE, "passphrase", passphrase)
                .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
        }
        None => {
            keychain::delete(KEYCHAIN_SERVICE, "passphrase")
                .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;

    // A well-known test private key (DO NOT use for real funds)
    // Address: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 (anvil/hardhat default #0)
    const TEST_PRIVATE_KEY: &str =
        "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    #[test]
    fn test_new_valid_private_key() {
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, None);
        assert!(account.is_ok());
    }

    #[test]
    fn test_new_with_0x_prefix() {
        let key = format!("0x{}", TEST_PRIVATE_KEY);
        let account = BuilderAccount::new(key, None);
        // alloy accepts 0x-prefixed keys
        assert!(account.is_ok());
    }

    #[test]
    fn test_new_invalid_private_key() {
        let result = BuilderAccount::new("not_a_valid_key", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            RelayError::Signer(msg) => {
                assert!(
                    msg.contains("Failed to parse private key"),
                    "unexpected: {msg}"
                );
            }
            other => panic!("Expected Signer error, got: {other:?}"),
        }
    }

    #[test]
    fn test_new_empty_key() {
        let result = BuilderAccount::new("", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_address_derivation_deterministic() {
        let a1 = BuilderAccount::new(TEST_PRIVATE_KEY, None).unwrap();
        let a2 = BuilderAccount::new(TEST_PRIVATE_KEY, None).unwrap();
        assert_eq!(a1.address(), a2.address());
    }

    #[test]
    fn test_address_matches_known_value() {
        // The first anvil/hardhat default account
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, None).unwrap();
        let expected: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse()
            .unwrap();
        assert_eq!(account.address(), expected);
    }

    #[test]
    fn test_debug_redacts_private_key() {
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, None).unwrap();
        let debug_output = format!("{:?}", account);
        assert!(
            debug_output.contains("address"),
            "Debug should show address, got: {debug_output}"
        );
        assert!(
            !debug_output.contains(TEST_PRIVATE_KEY),
            "Debug should not contain the private key, got: {debug_output}"
        );
    }

    #[test]
    fn test_config_none() {
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, None).unwrap();
        assert!(account.auth_config().is_none());
    }

    #[test]
    fn test_config_some() {
        let config = BuilderConfig::new("key".into(), "secret".into(), None);
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
        assert!(account.auth_config().is_some());
    }

    #[test]
    fn test_with_relayer_api_key() {
        let account = BuilderAccount::with_relayer_api_key(
            TEST_PRIVATE_KEY,
            "my-key".to_string(),
            "0xaddr".to_string(),
        )
        .unwrap();
        assert!(account.auth_config().is_some());
        assert!(matches!(
            account.auth_config(),
            Some(AuthConfig::RelayerApiKey(_))
        ));
    }

    #[test]
    fn test_new_wraps_builder_config_in_auth_config() {
        let config = BuilderConfig::new("key".into(), "secret".into(), None);
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
        assert!(matches!(
            account.auth_config(),
            Some(AuthConfig::Builder(_))
        ));
    }

    #[test]
    fn test_with_auth_config_none() {
        let account = BuilderAccount::with_auth_config(TEST_PRIVATE_KEY, None).unwrap();
        assert!(account.auth_config().is_none());
    }

    #[test]
    fn test_with_auth_config_relayer_api_key_variant() {
        let relayer =
            crate::config::RelayerApiKeyConfig::new("rk".into(), "0xaddr".into()).unwrap();
        let auth = AuthConfig::RelayerApiKey(relayer);
        let account = BuilderAccount::with_auth_config(TEST_PRIVATE_KEY, Some(auth)).unwrap();
        assert!(matches!(
            account.auth_config(),
            Some(AuthConfig::RelayerApiKey(_))
        ));
    }

    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        const TEST_PRIVATE_KEY: &str =
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn builder_account_keychain_roundtrip() {
            save_private_key_to_keychain(TEST_PRIVATE_KEY).unwrap();
            let config = BuilderConfig::new("rk".into(), "rs".into(), Some("rp".into()));
            save_builder_config_to_keychain(&config).unwrap();

            let account = BuilderAccount::from_keychain().unwrap();
            assert_eq!(
                account.address(),
                BuilderAccount::new(TEST_PRIVATE_KEY, None)
                    .unwrap()
                    .address()
            );
            assert!(account.auth_config().is_some());

            // Cleanup
            BuilderAccount::delete_from_keychain().unwrap();
        }

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn builder_account_keychain_no_config() {
            // Clear any leftover builder config entries so from_keychain()
            // exercises the "no api_key found" path.
            use polyoxide_core::keychain;
            let _ = keychain::delete(KEYCHAIN_SERVICE, "api_key");
            let _ = keychain::delete(KEYCHAIN_SERVICE, "api_secret");
            let _ = keychain::delete(KEYCHAIN_SERVICE, "passphrase");

            save_private_key_to_keychain(TEST_PRIVATE_KEY).unwrap();

            let account = BuilderAccount::from_keychain().unwrap();
            assert!(
                account.auth_config().is_none(),
                "Expected no auth config when api_key is absent"
            );

            // Cleanup
            let _ = keychain::delete(KEYCHAIN_SERVICE, "private_key");
        }

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn save_builder_config_none_passphrase_clears_stale() {
            use polyoxide_core::keychain;

            // Store config WITH passphrase
            save_private_key_to_keychain(TEST_PRIVATE_KEY).unwrap();
            let config_with = BuilderConfig::new("k".into(), "s".into(), Some("pp".into()));
            save_builder_config_to_keychain(&config_with).unwrap();

            // Verify passphrase is present
            assert!(keychain::get(KEYCHAIN_SERVICE, "passphrase").is_ok());

            // Overwrite with None passphrase — should delete the stale entry
            let config_without = BuilderConfig::new("k".into(), "s".into(), None);
            save_builder_config_to_keychain(&config_without).unwrap();

            // Verify passphrase has been removed
            let result = keychain::get(KEYCHAIN_SERVICE, "passphrase");
            assert!(
                matches!(result, Err(polyoxide_core::KeychainError::NotFound { .. })),
                "Expected passphrase to be deleted, got: {result:?}"
            );

            // And from_keychain should load account without passphrase in config
            let account = BuilderAccount::from_keychain().unwrap();
            if let Some(AuthConfig::Builder(bc)) = account.auth_config() {
                assert!(
                    bc.passphrase.is_none(),
                    "Expected passphrase=None after clearing"
                );
            } else {
                panic!("Expected Builder auth config");
            }

            // Cleanup
            BuilderAccount::delete_from_keychain().unwrap();
        }

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn relayer_api_key_keychain_roundtrip() {
            use polyoxide_core::keychain;

            // Store relayer API key credentials
            save_private_key_to_keychain(TEST_PRIVATE_KEY).unwrap();
            keychain::set(KEYCHAIN_SERVICE, "relayer_api_key", "test-rk").unwrap();
            keychain::set(KEYCHAIN_SERVICE, "relayer_api_key_address", "0xaddr").unwrap();

            let account = BuilderAccount::from_keychain_relayer_api_key().unwrap();
            assert_eq!(
                account.address(),
                BuilderAccount::new(TEST_PRIVATE_KEY, None)
                    .unwrap()
                    .address()
            );
            assert!(matches!(
                account.auth_config(),
                Some(AuthConfig::RelayerApiKey(_))
            ));

            // Cleanup
            BuilderAccount::delete_from_keychain().unwrap();
        }
    }

    #[test]
    fn test_with_auth_config_invalid_private_key() {
        let result = BuilderAccount::with_auth_config("not_a_valid_key", None);
        assert!(result.is_err());
        match result.unwrap_err() {
            RelayError::Signer(msg) => {
                assert!(
                    msg.contains("Failed to parse private key"),
                    "unexpected: {msg}"
                );
            }
            other => panic!("Expected Signer error, got: {other:?}"),
        }
    }
}
