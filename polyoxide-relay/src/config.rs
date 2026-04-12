use alloy::primitives::{address, Address};
use polyoxide_core::{current_timestamp, Base64Format, Signer};
use reqwest::header::{HeaderMap, HeaderValue};

/// On-chain contract addresses and RPC configuration for a specific chain.
#[derive(Clone, Debug)]
pub struct ContractConfig {
    pub safe_factory: Address,
    pub safe_multisend: Address,
    pub proxy_factory: Option<Address>,
    pub relay_hub: Option<Address>,
    pub rpc_url: &'static str,
}

/// Returns contract addresses for a supported chain, or `None` for unknown chain IDs.
///
/// Supported chains: Polygon mainnet (137), Amoy testnet (80002).
pub fn get_contract_config(chain_id: u64) -> Option<ContractConfig> {
    match chain_id {
        137 => Some(ContractConfig {
            safe_factory: address!("aacFeEa03eb1561C4e67d661e40682Bd20E3541b"),
            safe_multisend: address!("A238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761"),
            proxy_factory: Some(address!("aB45c5A4B0c941a2F231C04C3f49182e1A254052")),
            relay_hub: Some(address!("D216153c06E857cD7f72665E0aF1d7D82172F494")),
            rpc_url: "https://polygon.drpc.org",
        }),
        80002 => Some(ContractConfig {
            safe_factory: address!("aacFeEa03eb1561C4e67d661e40682Bd20E3541b"),
            safe_multisend: address!("A238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761"),
            proxy_factory: None, // Proxy not supported on Amoy testnet
            relay_hub: None,
            rpc_url: "https://rpc-amoy.polygon.technology",
        }),
        _ => None,
    }
}

/// API credentials for authenticating relay requests.
///
/// The `Debug` implementation redacts all secret fields to prevent accidental
/// leakage in logs.
#[derive(Clone)]
pub struct BuilderConfig {
    pub key: String,
    pub secret: String,
    pub passphrase: Option<String>,
}

impl std::fmt::Debug for BuilderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuilderConfig")
            .field("key", &"[REDACTED]")
            .field("secret", &"[REDACTED]")
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl BuilderConfig {
    /// Create a new builder config with the given API credentials.
    pub fn new(key: String, secret: String, passphrase: Option<String>) -> Self {
        Self {
            key,
            secret,
            passphrase,
        }
    }

    /// Generate HMAC-authenticated headers for Relay v1 requests.
    ///
    /// Uses the raw secret string for HMAC signing with standard base64 output.
    pub fn generate_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();
        let timestamp = current_timestamp();

        // Create signer from raw string secret (Relay v1 uses raw secrets)
        let signer = Signer::from_raw(&self.secret);
        let message = Signer::create_message(timestamp, method, path, body);
        let signature = signer.sign(&message, Base64Format::Standard)?;

        headers.insert(
            "POLY-API-KEY",
            HeaderValue::from_str(&self.key).map_err(|e| e.to_string())?,
        );
        headers.insert(
            "POLY-TIMESTAMP",
            HeaderValue::from_str(&timestamp.to_string()).map_err(|e| e.to_string())?,
        );
        headers.insert(
            "POLY-SIGNATURE",
            HeaderValue::from_str(&signature).map_err(|e| e.to_string())?,
        );

        if let Some(passphrase) = &self.passphrase {
            headers.insert(
                "POLY-PASSPHRASE",
                HeaderValue::from_str(passphrase).map_err(|e| e.to_string())?,
            );
        }

        Ok(headers)
    }

    /// Generate HMAC-authenticated headers for Relay v2 requests.
    ///
    /// Uses base64-decoded secret for HMAC signing with URL-safe base64 output.
    pub fn generate_relayer_v2_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();
        let timestamp = current_timestamp();

        // Create signer from base64-encoded secret (Relay v2 uses base64 secrets)
        let signer = Signer::new(&self.secret);
        let message = Signer::create_message(timestamp, method, path, body);
        let signature = signer.sign(&message, Base64Format::UrlSafe)?;

        headers.insert(
            "POLY_BUILDER_API_KEY",
            HeaderValue::from_str(&self.key).map_err(|e| e.to_string())?,
        );
        headers.insert(
            "POLY_BUILDER_TIMESTAMP",
            HeaderValue::from_str(&timestamp.to_string()).map_err(|e| e.to_string())?,
        );
        headers.insert(
            "POLY_BUILDER_SIGNATURE",
            HeaderValue::from_str(&signature).map_err(|e| e.to_string())?,
        );

        if let Some(passphrase) = &self.passphrase {
            headers.insert(
                "POLY_BUILDER_PASSPHRASE",
                HeaderValue::from_str(passphrase).map_err(|e| e.to_string())?,
            );
        }

        Ok(headers)
    }
}

/// Relayer API Key credentials for authenticated relay requests.
///
/// A simpler alternative to [`BuilderConfig`] that uses static headers
/// instead of HMAC-signed requests. See
/// <https://docs.polymarket.com/trading/gasless#using-relayer-api-keys>.
///
/// The `Debug` implementation redacts all secret fields to prevent accidental
/// leakage in logs.
#[derive(Clone)]
pub struct RelayerApiKeyConfig {
    pub key: String,
    pub address: String,
}

impl std::fmt::Debug for RelayerApiKeyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayerApiKeyConfig")
            .field("key", &"[REDACTED]")
            .field("address", &self.address)
            .finish()
    }
}

impl RelayerApiKeyConfig {
    /// Create a new relayer API key config.
    pub fn new(key: String, address: String) -> Self {
        Self { key, address }
    }

    /// Generate static authentication headers for relayer API key requests.
    pub fn generate_headers(&self) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "RELAYER_API_KEY",
            HeaderValue::from_str(&self.key).map_err(|e| e.to_string())?,
        );
        headers.insert(
            "RELAYER_API_KEY_ADDRESS",
            HeaderValue::from_str(&self.address).map_err(|e| e.to_string())?,
        );
        Ok(headers)
    }
}

/// Authentication configuration for relay requests.
///
/// Two authentication schemes are supported:
/// - [`Builder`](AuthConfig::Builder) — HMAC-SHA256 signed headers (builder API credentials)
/// - [`RelayerApiKey`](AuthConfig::RelayerApiKey) — static headers (relayer API key)
#[derive(Clone, Debug)]
pub enum AuthConfig {
    /// HMAC-authenticated builder API credentials.
    Builder(BuilderConfig),
    /// Static relayer API key headers.
    RelayerApiKey(RelayerApiKeyConfig),
}

impl AuthConfig {
    /// Generate authentication headers for Relay v2 requests.
    ///
    /// For [`Builder`](AuthConfig::Builder), this produces HMAC-signed headers.
    /// For [`RelayerApiKey`](AuthConfig::RelayerApiKey), this produces static headers
    /// (the `method`, `path`, and `body` parameters are ignored).
    pub fn generate_relayer_v2_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<HeaderMap, String> {
        match self {
            AuthConfig::Builder(config) => config.generate_relayer_v2_headers(method, path, body),
            AuthConfig::RelayerApiKey(config) => config.generate_headers(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_config_debug_redacts_secrets() {
        let config = BuilderConfig::new(
            "my-api-key".to_string(),
            "my-secret".to_string(),
            Some("my-passphrase".to_string()),
        );
        let debug_output = format!("{:?}", config);

        assert!(debug_output.contains("[REDACTED]"));
        assert!(
            !debug_output.contains("my-api-key"),
            "Debug leaked API key: {}",
            debug_output
        );
        assert!(
            !debug_output.contains("my-secret"),
            "Debug leaked secret: {}",
            debug_output
        );
        assert!(
            !debug_output.contains("my-passphrase"),
            "Debug leaked passphrase: {}",
            debug_output
        );
    }

    #[test]
    fn test_builder_config_debug_without_passphrase() {
        let config = BuilderConfig::new("key".to_string(), "secret".to_string(), None);
        let debug_output = format!("{:?}", config);

        assert!(debug_output.contains("[REDACTED]"));
        assert!(debug_output.contains("passphrase: None"));
    }

    #[test]
    fn test_relayer_api_key_generates_correct_headers() {
        let config = RelayerApiKeyConfig::new("my-relayer-key".to_string(), "0xabc123".to_string());
        let headers = config.generate_headers().unwrap();
        assert_eq!(
            headers.get("RELAYER_API_KEY").unwrap().to_str().unwrap(),
            "my-relayer-key"
        );
        assert_eq!(
            headers
                .get("RELAYER_API_KEY_ADDRESS")
                .unwrap()
                .to_str()
                .unwrap(),
            "0xabc123"
        );
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn test_relayer_api_key_debug_redacts_secrets() {
        let config = RelayerApiKeyConfig::new("my-relayer-key".to_string(), "0xabc123".to_string());
        let debug_output = format!("{:?}", config);
        assert!(debug_output.contains("[REDACTED]"));
        assert!(
            !debug_output.contains("my-relayer-key"),
            "Debug leaked API key: {debug_output}"
        );
    }

    #[test]
    fn test_auth_config_builder_delegates_correctly() {
        let builder = BuilderConfig::new(
            "key".to_string(),
            "c2VjcmV0".to_string(),
            Some("pass".to_string()),
        );
        let auth = AuthConfig::Builder(builder);
        let headers = auth
            .generate_relayer_v2_headers("POST", "/submit", Some("{}"))
            .unwrap();
        assert!(headers.get("POLY_BUILDER_API_KEY").is_some());
        assert!(headers.get("RELAYER_API_KEY").is_none());
    }

    #[test]
    fn test_auth_config_relayer_api_key_delegates_correctly() {
        let relayer = RelayerApiKeyConfig::new("rk".to_string(), "0xaddr".to_string());
        let auth = AuthConfig::RelayerApiKey(relayer);
        let headers = auth
            .generate_relayer_v2_headers("POST", "/submit", Some("{}"))
            .unwrap();
        assert!(headers.get("RELAYER_API_KEY").is_some());
        assert!(headers.get("POLY_BUILDER_API_KEY").is_none());
    }
}
