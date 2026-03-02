use polyoxide_core::{HttpClient, QueryBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    error::ClobError,
    request::{AuthMode, Request},
};

/// Auth namespace for API key management operations
#[derive(Clone)]
pub struct Auth {
    pub(crate) http_client: HttpClient,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
}

impl Auth {
    fn l1_auth(&self, nonce: u32) -> AuthMode {
        AuthMode::L1 {
            wallet: self.wallet.clone(),
            nonce,
        }
    }

    fn l2_auth(&self) -> AuthMode {
        AuthMode::L2 {
            address: self.wallet.address(),
            credentials: self.credentials.clone(),
            signer: self.signer.clone(),
        }
    }

    // --- Standard API keys ---

    /// Create a new API key (L1 auth)
    pub fn create_api_key(&self, nonce: u32) -> Request<ApiKeyResponse> {
        Request::post(
            self.http_client.clone(),
            "/auth/api-key".to_string(),
            self.l1_auth(nonce),
            self.chain_id,
        )
    }

    /// Derive an existing API key (L1 auth)
    pub fn derive_api_key(&self, nonce: u32) -> Request<ApiKeyResponse> {
        Request::get(
            self.http_client.clone(),
            "/auth/derive-api-key",
            self.l1_auth(nonce),
            self.chain_id,
        )
    }

    /// List all API keys (L2 auth)
    pub fn list_api_keys(&self) -> Request<Vec<ApiKeyInfo>> {
        Request::get(
            self.http_client.clone(),
            "/auth/api-keys",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Delete the current API key (L2 auth)
    pub fn delete_api_key(&self) -> Request<serde_json::Value> {
        Request::delete(
            self.http_client.clone(),
            "/auth/api-key",
            self.l2_auth(),
            self.chain_id,
        )
    }

    // --- Read-only API keys ---

    /// Create a new read-only API key (L1 auth)
    pub fn create_readonly_key(&self, nonce: u32) -> Request<ReadonlyApiKeyResponse> {
        Request::post(
            self.http_client.clone(),
            "/auth/readonly-api-key".to_string(),
            self.l1_auth(nonce),
            self.chain_id,
        )
    }

    /// List all read-only API keys (L2 auth)
    pub fn list_readonly_keys(&self) -> Request<Vec<ReadonlyApiKeyResponse>> {
        Request::get(
            self.http_client.clone(),
            "/auth/readonly-api-keys",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Delete a read-only API key (L2 auth)
    pub async fn delete_readonly_key(
        &self,
        key: impl Into<String>,
    ) -> Result<serde_json::Value, ClobError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            api_key: String,
        }

        Request::<serde_json::Value>::delete(
            self.http_client.clone(),
            "/auth/readonly-api-key",
            self.l2_auth(),
            self.chain_id,
        )
        .body(&Body {
            api_key: key.into(),
        })?
        .send()
        .await
    }

    /// Validate a read-only API key (no auth)
    pub fn validate_readonly_key(
        &self,
        address: impl Into<String>,
        key: impl Into<String>,
    ) -> Request<ValidateKeyResponse> {
        Request::get(
            self.http_client.clone(),
            "/auth/validate-readonly-api-key",
            AuthMode::None,
            self.chain_id,
        )
        .query("address", address.into())
        .query("api_key", key.into())
    }

    // --- Builder API keys ---

    /// Create a new builder API key (L1 auth)
    pub fn create_builder_key(&self, nonce: u32) -> Request<ApiKeyResponse> {
        Request::post(
            self.http_client.clone(),
            "/auth/builder-api-key".to_string(),
            self.l1_auth(nonce),
            self.chain_id,
        )
    }

    /// List all builder API keys (L2 auth)
    pub fn list_builder_keys(&self) -> Request<Vec<ApiKeyInfo>> {
        Request::get(
            self.http_client.clone(),
            "/auth/builder-api-key",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Delete the current builder API key (L2 auth)
    pub fn delete_builder_key(&self) -> Request<serde_json::Value> {
        Request::delete(
            self.http_client.clone(),
            "/auth/builder-api-key",
            self.l2_auth(),
            self.chain_id,
        )
    }

    // --- Ban status ---

    /// Check if the account is in closed-only mode
    pub fn closed_only_status(&self) -> Request<ClosedOnlyResponse> {
        Request::get(
            self.http_client.clone(),
            "/auth/ban-status/closed-only",
            self.l2_auth(),
            self.chain_id,
        )
    }
}

/// Response from creating or deriving an API key
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ApiKeyResponse {
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
}

/// API key listing entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ApiKeyInfo {
    pub api_key: String,
}

/// Response from creating a read-only API key
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ReadonlyApiKeyResponse {
    pub api_key: String,
}

/// Response from validating a read-only API key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateKeyResponse {
    pub valid: bool,
}

/// Response from the closed-only ban status check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedOnlyResponse {
    pub closed_only: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_response_deserializes() {
        let json = r#"{
            "apiKey": "key-123",
            "secret": "secret-456",
            "passphrase": "pass-789"
        }"#;
        let resp: ApiKeyResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.api_key, "key-123");
        assert_eq!(resp.secret, "secret-456");
        assert_eq!(resp.passphrase, "pass-789");
    }

    #[test]
    fn readonly_api_key_response_deserializes() {
        let json = r#"{"apiKey": "readonly-key"}"#;
        let resp: ReadonlyApiKeyResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.api_key, "readonly-key");
    }

    #[test]
    fn validate_key_response_deserializes() {
        let json = r#"{"valid": true}"#;
        let resp: ValidateKeyResponse = serde_json::from_str(json).unwrap();
        assert!(resp.valid);

        let json = r#"{"valid": false}"#;
        let resp: ValidateKeyResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.valid);
    }

    #[test]
    fn api_key_info_deserializes() {
        let json = r#"{"apiKey": "key-abc"}"#;
        let info: ApiKeyInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.api_key, "key-abc");
    }

    #[test]
    fn api_key_response_rejects_missing_fields() {
        // Missing secret and passphrase
        let json = r#"{"apiKey": "key-123"}"#;
        assert!(serde_json::from_str::<ApiKeyResponse>(json).is_err());

        // Missing passphrase
        let json = r#"{"apiKey": "k", "secret": "s"}"#;
        assert!(serde_json::from_str::<ApiKeyResponse>(json).is_err());
    }

    #[test]
    fn api_key_response_list_deserializes() {
        let json = r#"[{"apiKey": "k1"}, {"apiKey": "k2"}]"#;
        let list: Vec<ApiKeyInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].api_key, "k1");
        assert_eq!(list[1].api_key, "k2");
    }

    #[test]
    fn closed_only_response_deserializes() {
        let json = r#"{"closed_only": true}"#;
        let resp: ClosedOnlyResponse = serde_json::from_str(json).unwrap();
        assert!(resp.closed_only);

        let json = r#"{"closed_only": false}"#;
        let resp: ClosedOnlyResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.closed_only);
    }
}
