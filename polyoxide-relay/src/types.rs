use alloy::sol;
use serde::{Deserialize, Serialize};

/// Wallet type for the relayer API
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WalletType {
    /// Safe wallet - requires explicit deployment before first transaction
    #[default]
    Safe,
    /// Proxy wallet - auto-deploys on first transaction (Magic Link users)
    Proxy,
}

impl WalletType {
    /// Returns the API string representation ("SAFE" or "PROXY").
    pub fn as_str(&self) -> &'static str {
        match self {
            WalletType::Safe => "SAFE",
            WalletType::Proxy => "PROXY",
        }
    }
}

sol! {
    /// A single transaction to execute through the relayer's Safe.
    ///
    /// This is the primary input to the relay client's `execute*` paths: one or
    /// more of these are batched, signed, and submitted to `POST /submit`.
    #[derive(Debug, PartialEq, Eq)]
    struct SafeTransaction {
        /// Target contract or recipient address.
        address to;
        /// Call type: `0` = CALL, `1` = DELEGATECALL.
        uint8 operation;
        /// ABI-encoded calldata for the call (empty for a plain value transfer).
        bytes data;
        /// Native token amount (in wei) to send with the call.
        uint256 value;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SafeTransactionArgs {
        address from_address;
        uint256 nonce;
        uint256 chain_id;
        SafeTransaction[] transactions;
    }

    /// The Gnosis Safe `execTransaction` payload that is EIP-712 signed.
    ///
    /// Mirrors the canonical Safe transaction struct; the relay client builds and
    /// signs this from a [`SafeTransaction`] before submitting it.
    #[derive(Debug, PartialEq, Eq)]
    struct SafeTx {
        /// Destination address of the Safe transaction.
        address to;
        /// Native token amount (in wei) transferred with the call.
        uint256 value;
        /// ABI-encoded calldata for the call.
        bytes data;
        /// Call type: `0` = CALL, `1` = DELEGATECALL.
        uint8 operation;
        /// Gas that should be used for the Safe transaction itself.
        uint256 safeTxGas;
        /// Gas costs independent of execution (base fee, signature checks, refund).
        uint256 baseGas;
        /// Gas price used for the refund calculation (`0` to disable refunds).
        uint256 gasPrice;
        /// Token used for the gas payment (`0x0` = native token).
        address gasToken;
        /// Address that receives the gas payment refund (`0x0` = `tx.origin`).
        address refundReceiver;
        /// Safe nonce for replay protection.
        uint256 nonce;
    }
}

/// Partial, legacy representation of a relayer submission payload.
///
/// This type is **not** the actual wire body sent to the relayer: the real
/// `POST /submit` payloads are the private `SafeSubmitBody` / `ProxySubmitBody`
/// structs in `client.rs` (built and serialized by the relay client's
/// `execute_safe` / `execute_proxy` paths), which additionally carry
/// `signatureParams`, `nonce`, `value`, and `metadata`. This struct is retained
/// only for backwards compatibility and is exercised solely by its own serde
/// unit tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRequest {
    #[serde(rename = "type")]
    pub type_: String,
    pub from: String,
    pub to: String,
    #[serde(rename = "proxyWallet")]
    pub proxy_wallet: String,
    pub data: String,
    pub signature: String,
    // Add signature params if needed
}

/// Response from `POST /submit` after a transaction submission.
///
/// The OpenAPI spec guarantees only `transactionID` and `state`; the onchain
/// transaction hash must be fetched later via `GET /transaction`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitResponse {
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    /// Current state of the transaction (e.g. `STATE_NEW`).
    pub state: String,
}

/// Full relayer transaction record returned by `GET /transaction`.
///
/// Mirrors the OpenAPI `RelayerTransaction` schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayerTransaction {
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    pub transaction_hash: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub proxy_address: Option<String>,
    pub data: Option<String>,
    pub nonce: Option<String>,
    pub value: Option<String>,
    pub signature: Option<String>,
    /// Current state (e.g. `STATE_NEW`, `STATE_MINED`, `STATE_CONFIRMED`, ...).
    pub state: String,
    /// Transaction type (`SAFE` or `PROXY`).
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub owner: Option<String>,
    pub metadata: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Deserialize a nonce that may be represented as either a JSON number or string.
pub fn deserialize_nonce<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct NonceVisitor;

    impl<'de> de::Visitor<'de> for NonceVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a u64 or string representing a u64")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            v.parse().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(NonceVisitor)
}

/// Response from the relayer's nonce endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceResponse {
    #[serde(deserialize_with = "deserialize_nonce")]
    pub nonce: u64,
}

/// A relayer API key record returned by `GET /relayer/api/keys`.
///
/// Mirrors the OpenAPI `RelayerApiKey` schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayerApiKey {
    /// The relayer API key identifier (UUID).
    pub api_key: String,
    /// The on-chain address that owns this key.
    pub address: String,
    /// RFC3339 timestamp when the key was created.
    pub created_at: String,
    /// RFC3339 timestamp when the key was last updated.
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── deserialize_nonce ───────────────────────────────────────

    #[test]
    fn test_nonce_from_integer() {
        let json = r#"{"nonce": 42}"#;
        let resp: NonceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.nonce, 42);
    }

    #[test]
    fn test_nonce_from_string() {
        let json = r#"{"nonce": "123"}"#;
        let resp: NonceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.nonce, 123);
    }

    #[test]
    fn test_nonce_from_zero_integer() {
        let json = r#"{"nonce": 0}"#;
        let resp: NonceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.nonce, 0);
    }

    #[test]
    fn test_nonce_from_zero_string() {
        let json = r#"{"nonce": "0"}"#;
        let resp: NonceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.nonce, 0);
    }

    #[test]
    fn test_nonce_from_large_integer() {
        let json = r#"{"nonce": 18446744073709551615}"#;
        let resp: NonceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.nonce, u64::MAX);
    }

    #[test]
    fn test_nonce_from_large_string() {
        let json = r#"{"nonce": "18446744073709551615"}"#;
        let resp: NonceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.nonce, u64::MAX);
    }

    #[test]
    fn test_nonce_from_non_numeric_string_fails() {
        let json = r#"{"nonce": "abc"}"#;
        let result = serde_json::from_str::<NonceResponse>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonce_from_empty_string_fails() {
        let json = r#"{"nonce": ""}"#;
        let result = serde_json::from_str::<NonceResponse>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonce_from_null_fails() {
        let json = r#"{"nonce": null}"#;
        let result = serde_json::from_str::<NonceResponse>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonce_missing_field_fails() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<NonceResponse>(json);
        assert!(result.is_err());
    }

    // ── WalletType ──────────────────────────────────────────────

    #[test]
    fn test_wallet_type_as_str() {
        assert_eq!(WalletType::Safe.as_str(), "SAFE");
        assert_eq!(WalletType::Proxy.as_str(), "PROXY");
    }

    #[test]
    fn test_wallet_type_default_is_safe() {
        assert_eq!(WalletType::default(), WalletType::Safe);
    }

    // ── TransactionRequest serde ────────────────────────────────

    #[test]
    fn test_transaction_request_serialization() {
        let tx = TransactionRequest {
            type_: "SAFE".to_string(),
            from: "0xabc".to_string(),
            to: "0xdef".to_string(),
            proxy_wallet: "0x123".to_string(),
            data: "0xdeadbeef".to_string(),
            signature: "0xsig".to_string(),
        };
        let json = serde_json::to_value(&tx).unwrap();
        assert_eq!(json["type"], "SAFE");
        assert_eq!(json["from"], "0xabc");
        assert_eq!(json["proxyWallet"], "0x123");
    }

    #[test]
    fn test_transaction_request_deserialization() {
        let json = r#"{
            "type": "PROXY",
            "from": "0xabc",
            "to": "0xdef",
            "proxyWallet": "0x123",
            "data": "0xdeadbeef",
            "signature": "0xsig"
        }"#;
        let tx: TransactionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(tx.type_, "PROXY");
        assert_eq!(tx.proxy_wallet, "0x123");
    }

    // ── SubmitResponse serde ────────────────────────────────────

    #[test]
    fn test_submit_response_new_state() {
        let json = r#"{
            "transactionID": "tx-123",
            "state": "STATE_NEW"
        }"#;
        let resp: SubmitResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.transaction_id, "tx-123");
        assert_eq!(resp.state, "STATE_NEW");
    }

    // ── RelayerTransaction serde ────────────────────────────────

    #[test]
    fn test_relayer_transaction_full() {
        let json = r#"{
            "transactionID": "0190b317-a1d3-7bec-9b91-eeb6dcd3a620",
            "transactionHash": "0x38cbfbeae8fffa4e2b187ee5978d3ee9cafc53af0363ed90a35b7ea9016535d8",
            "from": "0x6e0c80c90ea6c15917308f820eac91ce2724b5b5",
            "to": "0x2791bca1f2de4661ed88a30c99a7a9449aa84174",
            "proxyAddress": "0x6d8c4e9adf5748af82dabe2c6225207770d6b4fa",
            "data": "0xdeadbeef",
            "nonce": "60",
            "value": "",
            "signature": "0xabc",
            "state": "STATE_CONFIRMED",
            "type": "SAFE",
            "owner": "0x6e0c80c90ea6c15917308f820eac91ce2724b5b5",
            "metadata": "",
            "createdAt": "2024-07-14T21:13:08.819782Z",
            "updatedAt": "2024-07-14T21:13:46.576639Z"
        }"#;
        let resp: RelayerTransaction = serde_json::from_str(json).unwrap();
        assert_eq!(resp.transaction_id, "0190b317-a1d3-7bec-9b91-eeb6dcd3a620");
        assert_eq!(resp.state, "STATE_CONFIRMED");
        assert_eq!(resp.kind.as_deref(), Some("SAFE"));
        assert_eq!(resp.nonce.as_deref(), Some("60"));
        assert!(resp.transaction_hash.is_some());
        assert!(resp.created_at.is_some());
    }

    #[test]
    fn test_relayer_transaction_pending_minimal() {
        let json = r#"{
            "transactionID": "tx-new",
            "state": "STATE_NEW"
        }"#;
        let resp: RelayerTransaction = serde_json::from_str(json).unwrap();
        assert_eq!(resp.transaction_id, "tx-new");
        assert_eq!(resp.state, "STATE_NEW");
        assert!(resp.transaction_hash.is_none());
        assert!(resp.kind.is_none());
    }

    // ── RelayerApiKey serde ─────────────────────────────────────

    #[test]
    fn test_relayer_api_key_deserializes_openapi_example() {
        // Example lifted from docs/specs/relay/openapi.yaml for
        // `/relayer/api/keys` response schema.
        let json = r#"{
            "apiKey": "01967c03-b8c8-7000-8f68-8b8eaec6fd3d",
            "address": "0xabc...",
            "createdAt": "2026-02-24T18:20:11.237485Z",
            "updatedAt": "2026-02-24T18:20:11.237485Z"
        }"#;
        let key: RelayerApiKey = serde_json::from_str(json).unwrap();
        assert_eq!(key.api_key, "01967c03-b8c8-7000-8f68-8b8eaec6fd3d");
        assert_eq!(key.address, "0xabc...");
        assert_eq!(key.created_at, "2026-02-24T18:20:11.237485Z");
        assert_eq!(key.updated_at, "2026-02-24T18:20:11.237485Z");
    }

    #[test]
    fn test_relayer_api_key_roundtrip_preserves_camel_case() {
        let key = RelayerApiKey {
            api_key: "abc".to_string(),
            address: "0xdef".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
        };
        let serialized = serde_json::to_value(&key).unwrap();
        assert_eq!(serialized["apiKey"], "abc");
        assert_eq!(serialized["address"], "0xdef");
        assert_eq!(serialized["createdAt"], "2026-01-01T00:00:00Z");
        assert_eq!(serialized["updatedAt"], "2026-01-02T00:00:00Z");

        let round: RelayerApiKey = serde_json::from_value(serialized).unwrap();
        assert_eq!(round.api_key, "abc");
        assert_eq!(round.updated_at, "2026-01-02T00:00:00Z");
    }

    #[test]
    fn test_relayer_api_key_list_deserializes() {
        let json = r#"[
            {
                "apiKey": "key-1",
                "address": "0xa1",
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z"
            },
            {
                "apiKey": "key-2",
                "address": "0xa2",
                "createdAt": "2026-01-02T00:00:00Z",
                "updatedAt": "2026-01-02T00:00:00Z"
            }
        ]"#;
        let keys: Vec<RelayerApiKey> = serde_json::from_str(json).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].api_key, "key-1");
        assert_eq!(keys[1].api_key, "key-2");
    }
}
