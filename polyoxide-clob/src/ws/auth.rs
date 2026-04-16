//! API credentials for WebSocket authentication.

use std::fmt;

use serde::{Deserialize, Serialize};

/// API credentials for WebSocket user channel authentication.
///
/// These credentials can be obtained from your Polymarket account settings
/// or derived using the CLOB API.
///
/// # Example
///
/// ```
/// use polyoxide_clob::ws::ApiCredentials;
///
/// let creds = ApiCredentials::new("api_key", "api_secret", "passphrase");
/// ```
#[derive(Clone, Serialize, Deserialize)]
pub struct ApiCredentials {
    /// API key
    #[serde(rename = "apiKey")]
    pub api_key: String,
    /// API secret
    pub secret: String,
    /// API passphrase
    pub passphrase: String,
}

impl ApiCredentials {
    /// Create new API credentials.
    pub fn new(
        api_key: impl Into<String>,
        secret: impl Into<String>,
        passphrase: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            secret: secret.into(),
            passphrase: passphrase.into(),
        }
    }

    /// Load credentials from environment variables.
    ///
    /// Reads:
    /// - `POLYMARKET_API_KEY`
    /// - `POLYMARKET_API_SECRET`
    /// - `POLYMARKET_API_PASSPHRASE`
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            api_key: std::env::var("POLYMARKET_API_KEY")?,
            secret: std::env::var("POLYMARKET_API_SECRET")?,
            passphrase: std::env::var("POLYMARKET_API_PASSPHRASE")?,
        })
    }

    /// Load credentials from the OS keychain.
    ///
    /// Reads from the `polyoxide-clob` keychain service:
    /// - `api_key`
    /// - `api_secret`
    /// - `api_passphrase`
    #[cfg(feature = "keychain")]
    pub fn from_keychain() -> Result<Self, polyoxide_core::KeychainError> {
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-clob";

        Ok(Self {
            api_key: keychain::get(SERVICE, "api_key")?,
            secret: keychain::get(SERVICE, "api_secret")?,
            passphrase: keychain::get(SERVICE, "api_passphrase")?,
        })
    }
}

impl fmt::Debug for ApiCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiCredentials")
            .field("api_key", &"<redacted>")
            .field("secret", &"<redacted>")
            .field("passphrase", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_credentials_new() {
        let creds = ApiCredentials::new("key123", "secret456", "pass789");
        assert_eq!(creds.api_key, "key123");
        assert_eq!(creds.secret, "secret456");
        assert_eq!(creds.passphrase, "pass789");
    }

    #[test]
    fn api_credentials_new_accepts_string() {
        let creds = ApiCredentials::new(
            String::from("key"),
            String::from("secret"),
            String::from("pass"),
        );
        assert_eq!(creds.api_key, "key");
    }

    #[test]
    fn debug_redacts_all_fields() {
        let creds = ApiCredentials::new(
            "super-secret-key",
            "ultra-secret-secret",
            "mega-secret-passphrase",
        );
        let debug = format!("{:?}", creds);

        assert!(
            !debug.contains("super-secret-key"),
            "Debug must not leak api_key: {}",
            debug
        );
        assert!(
            !debug.contains("ultra-secret-secret"),
            "Debug must not leak secret: {}",
            debug
        );
        assert!(
            !debug.contains("mega-secret-passphrase"),
            "Debug must not leak passphrase: {}",
            debug
        );
        assert!(
            debug.contains("<redacted>"),
            "Debug should show <redacted>: {}",
            debug
        );
    }

    #[test]
    fn serialize_uses_api_key_rename() {
        let creds = ApiCredentials::new("my_key", "my_secret", "my_pass");
        let json = serde_json::to_value(&creds).unwrap();

        // Should use "apiKey" not "api_key"
        assert!(
            json.get("apiKey").is_some(),
            "Should serialize api_key as apiKey: {}",
            json
        );
        assert!(
            json.get("api_key").is_none(),
            "Should not have api_key field: {}",
            json
        );
        assert_eq!(json["apiKey"], "my_key");
        assert_eq!(json["secret"], "my_secret");
        assert_eq!(json["passphrase"], "my_pass");
    }

    #[test]
    fn deserialize_with_api_key_rename() {
        let json = r#"{"apiKey": "k", "secret": "s", "passphrase": "p"}"#;
        let creds: ApiCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(creds.api_key, "k");
        assert_eq!(creds.secret, "s");
        assert_eq!(creds.passphrase, "p");
    }

    #[test]
    fn serde_roundtrip() {
        let original = ApiCredentials::new("key", "secret", "pass");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ApiCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.api_key, original.api_key);
        assert_eq!(deserialized.secret, original.secret);
        assert_eq!(deserialized.passphrase, original.passphrase);
    }

    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn api_credentials_from_keychain() {
            // Pre-populate keychain (reuses values from Account tests)
            polyoxide_core::keychain::set("polyoxide-clob", "api_key", "kc_key").unwrap();
            polyoxide_core::keychain::set("polyoxide-clob", "api_secret", "kc_secret").unwrap();
            polyoxide_core::keychain::set("polyoxide-clob", "api_passphrase", "kc_pass").unwrap();

            let creds = ApiCredentials::from_keychain().unwrap();
            assert_eq!(creds.api_key, "kc_key");
            assert_eq!(creds.secret, "kc_secret");
            assert_eq!(creds.passphrase, "kc_pass");
        }
    }

    #[test]
    fn clone_produces_independent_copy() {
        let original = ApiCredentials::new("key", "secret", "pass");
        let cloned = original.clone();
        assert_eq!(cloned.api_key, original.api_key);
        assert_eq!(cloned.secret, original.secret);
        assert_eq!(cloned.passphrase, original.passphrase);
    }
}
