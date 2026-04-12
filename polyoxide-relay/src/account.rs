use crate::config::{AuthConfig, BuilderConfig, RelayerApiKeyConfig};
use crate::error::RelayError;
use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;

/// Account credentials for authenticated relay operations.
///
/// Combines a private key signer (for EIP-712 transaction signing) with optional
/// builder API credentials (for HMAC-authenticated relay submission). The `Debug`
/// implementation redacts the private key to prevent accidental leakage in logs.
#[derive(Clone)]
pub struct BuilderAccount {
    pub(crate) signer: PrivateKeySigner,
    pub(crate) config: Option<AuthConfig>,
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
        let signer = private_key
            .into()
            .parse::<PrivateKeySigner>()
            .map_err(|e| RelayError::Signer(format!("Failed to parse private key: {}", e)))?;

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
        let signer = private_key
            .into()
            .parse::<PrivateKeySigner>()
            .map_err(|e| RelayError::Signer(format!("Failed to parse private key: {}", e)))?;

        Ok(Self {
            signer,
            config: Some(AuthConfig::RelayerApiKey(RelayerApiKeyConfig::new(
                key, address,
            ))),
        })
    }

    /// Create a new account from a hex-encoded private key and a pre-built [`AuthConfig`].
    pub fn with_auth_config(
        private_key: impl Into<String>,
        config: Option<AuthConfig>,
    ) -> Result<Self, RelayError> {
        let signer = private_key
            .into()
            .parse::<PrivateKeySigner>()
            .map_err(|e| RelayError::Signer(format!("Failed to parse private key: {}", e)))?;

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

    /// Returns the auth config, if one was provided.
    ///
    /// This is a backward-compatible accessor. Prefer [`auth_config`](Self::auth_config)
    /// for new code.
    pub fn config(&self) -> Option<&AuthConfig> {
        self.config.as_ref()
    }
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
        assert!(account.config().is_none());
    }

    #[test]
    fn test_config_some() {
        let config = BuilderConfig::new("key".into(), "secret".into(), None);
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
        assert!(account.config().is_some());
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
        assert!(
            matches!(account.auth_config(), Some(AuthConfig::RelayerApiKey(_)))
        );
    }

    #[test]
    fn test_new_wraps_builder_config_in_auth_config() {
        let config = BuilderConfig::new("key".into(), "secret".into(), None);
        let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
        assert!(
            matches!(account.auth_config(), Some(AuthConfig::Builder(_)))
        );
    }
}
