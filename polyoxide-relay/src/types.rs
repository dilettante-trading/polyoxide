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
    #[derive(Debug, PartialEq, Eq)]
    struct SafeTransaction {
        address to;
        uint8 operation;
        bytes data;
        uint256 value;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SafeTransactionArgs {
        address from_address;
        uint256 nonce;
        uint256 chain_id;
        SafeTransaction[] transactions;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SafeTx {
        address to;
        uint256 value;
        bytes data;
        uint8 operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }
}

/// Serializable transaction submission payload sent to the relayer.
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
}
