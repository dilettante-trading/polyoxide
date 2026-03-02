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

/// Response from the relayer after submitting a transaction.
///
/// The `transaction_hash` is `None` until the transaction is mined on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayerTransactionResponse {
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<String>,
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

/// Response from the relayer's transaction status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStatusResponse {
    pub state: String,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<String>,
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

    // ── RelayerTransactionResponse serde ────────────────────────

    #[test]
    fn test_relayer_response_with_hash() {
        let json = r#"{
            "transactionID": "tx-123",
            "transactionHash": "0xabcdef"
        }"#;
        let resp: RelayerTransactionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.transaction_id, "tx-123");
        assert_eq!(resp.transaction_hash.as_deref(), Some("0xabcdef"));
    }

    #[test]
    fn test_relayer_response_without_hash() {
        let json = r#"{
            "transactionID": "tx-456",
            "transactionHash": null
        }"#;
        let resp: RelayerTransactionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.transaction_id, "tx-456");
        assert!(resp.transaction_hash.is_none());
    }

    #[test]
    fn test_relayer_response_missing_hash_field() {
        let json = r#"{"transactionID": "tx-789"}"#;
        let resp: RelayerTransactionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.transaction_id, "tx-789");
        assert!(resp.transaction_hash.is_none());
    }

    // ── TransactionStatusResponse serde ─────────────────────────

    #[test]
    fn test_transaction_status_response() {
        let json = r#"{
            "state": "CONFIRMED",
            "transactionHash": "0xabc123"
        }"#;
        let resp: TransactionStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.state, "CONFIRMED");
        assert_eq!(resp.transaction_hash.as_deref(), Some("0xabc123"));
    }

    #[test]
    fn test_transaction_status_pending() {
        let json = r#"{
            "state": "PENDING",
            "transactionHash": null
        }"#;
        let resp: TransactionStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.state, "PENDING");
        assert!(resp.transaction_hash.is_none());
    }
}
