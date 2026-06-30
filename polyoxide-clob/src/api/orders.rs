use std::collections::HashMap;

use polyoxide_core::{HttpClient, QueryBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    error::ClobError,
    request::{AuthMode, Request},
    types::SignedOrder,
};

/// Orders namespace for order-related operations
#[derive(Clone)]
pub struct Orders {
    pub(crate) http_client: HttpClient,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
}

impl Orders {
    /// List user's orders
    pub fn list(&self) -> Request<ListOrdersResponse> {
        Request::get(
            self.http_client.clone(),
            "/data/orders",
            AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
    }

    /// Get a specific order by ID
    pub fn get(&self, order_id: impl Into<String>) -> Request<OpenOrder> {
        Request::get(
            self.http_client.clone(),
            format!("/data/order/{}", urlencoding::encode(&order_id.into())),
            AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
    }

    /// Cancel an order
    pub fn cancel(&self, order_id: impl Into<String>) -> CancelOrderRequest {
        CancelOrderRequest {
            http_client: self.http_client.clone(),
            auth: AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            chain_id: self.chain_id,
            order_id: order_id.into(),
        }
    }

    /// Cancel all open orders
    pub async fn cancel_all(&self) -> Result<BatchCancelResponse, ClobError> {
        Request::<BatchCancelResponse>::delete(
            self.http_client.clone(),
            "/cancel-all",
            AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .send()
        .await
    }

    /// Cancel all orders for a specific market and asset
    pub async fn cancel_market(
        &self,
        market: impl Into<String>,
        asset_id: impl Into<String>,
    ) -> Result<BatchCancelResponse, ClobError> {
        #[derive(Serialize)]
        struct Body {
            market: String,
            asset_id: String,
        }

        Request::<BatchCancelResponse>::delete(
            self.http_client.clone(),
            "/cancel-market-orders",
            AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .body(&Body {
            market: market.into(),
            asset_id: asset_id.into(),
        })?
        .send()
        .await
    }

    /// Check if an order is being scored for rewards
    pub fn is_scoring(&self, order_id: impl Into<String>) -> Request<OrderScoringResponse> {
        Request::get(
            self.http_client.clone(),
            "/order-scoring",
            AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .query("order_id", order_id.into())
    }

    /// Check if multiple orders are being scored for rewards
    pub fn are_scoring(
        &self,
        order_ids: impl Into<Vec<String>>,
    ) -> Request<Vec<OrderScoringResponse>> {
        Request::get(
            self.http_client.clone(),
            "/orders-scoring",
            AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .query_many("order_ids", order_ids.into())
    }

    /// Cancel multiple orders by ID (up to 3000)
    pub async fn cancel_many(
        &self,
        order_ids: impl Into<Vec<String>>,
    ) -> Result<BatchCancelResponse, ClobError> {
        let ids: Vec<String> = order_ids.into();

        Request::<BatchCancelResponse>::delete(
            self.http_client.clone(),
            "/orders",
            AuthMode::L2 {
                address: self.wallet.address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .body(&ids)?
        .send()
        .await
    }
}

/// Request builder for canceling an order
pub struct CancelOrderRequest {
    http_client: HttpClient,
    auth: AuthMode,
    chain_id: u64,
    order_id: String,
}

impl CancelOrderRequest {
    /// Execute the cancel request
    pub async fn send(self) -> Result<BatchCancelResponse, ClobError> {
        #[derive(serde::Serialize)]
        struct CancelRequest {
            #[serde(rename = "orderID")]
            order_id: String,
        }

        let request = CancelRequest {
            order_id: self.order_id,
        };

        Request::delete(self.http_client, "/order", self.auth, self.chain_id)
            .body(&request)?
            .send()
            .await
    }
}

/// Open order from API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OpenOrder {
    pub id: String,
    pub market: String,
    pub asset_id: String,
    #[serde(flatten)]
    pub order: SignedOrder,
    pub status: String,
    pub owner: Option<String>,
    pub maker_address: Option<String>,
    pub original_size: Option<String>,
    pub size_matched: Option<String>,
    pub price: Option<String>,
    #[serde(default)]
    pub associate_trades: Vec<String>,
    pub outcome: Option<String>,
    pub order_type: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// Response from posting an order
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OrderResponse {
    pub success: bool,
    pub error_msg: Option<String>,
    #[serde(rename(deserialize = "orderID"))]
    pub order_id: Option<String>,
    #[serde(default, rename(deserialize = "transactionsHashes"))]
    pub transaction_hashes: Vec<String>,
    pub status: Option<String>,
    pub taking_amount: Option<String>,
    pub making_amount: Option<String>,
}

/// Response from order scoring check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderScoringResponse {
    pub order_id: String,
    pub scoring: bool,
}

/// Response from cancel and batch cancel operations.
///
/// The Polymarket API returns this shape for all cancel endpoints:
/// `DELETE /order`, `DELETE /orders`, `DELETE /cancel-all`, `DELETE /cancel-market-orders`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct BatchCancelResponse {
    #[serde(default)]
    pub canceled: Vec<String>,
    #[serde(default)]
    pub not_canceled: HashMap<String, String>,
}

/// Paginated response from `GET /data/orders`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListOrdersResponse {
    pub data: Vec<OpenOrder>,
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use std::str::FromStr;

    #[test]
    fn open_order_deserializes_with_flattened_signed_order() {
        let json = r#"{
            "id": "order-abc",
            "market": "0xcondition123",
            "assetId": "0xtoken456",
            "salt": "999",
            "maker": "0x0000000000000000000000000000000000000001",
            "signer": "0x0000000000000000000000000000000000000002",
            "tokenId": "0xtoken456",
            "makerAmount": "1000",
            "takerAmount": "500",
            "side": "BUY",
            "expiration": "0",
            "signatureType": 0,
            "timestamp": "1700000000000",
            "metadata": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "builder": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "signature": "0xsig",
            "status": "LIVE",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": null
        }"#;
        let order: OpenOrder = serde_json::from_str(json).unwrap();
        assert_eq!(order.id, "order-abc");
        assert_eq!(order.market, "0xcondition123");
        assert_eq!(order.asset_id, "0xtoken456");
        assert_eq!(order.status, "LIVE");
        assert_eq!(order.order.signature, "0xsig");
        assert_eq!(order.order.order.maker_amount, "1000");
        assert_eq!(
            order.order.order.maker,
            Address::from_str("0x0000000000000000000000000000000000000001").unwrap()
        );
        assert!(order.updated_at.is_none());
        // New fields default to None/empty when absent
        assert!(order.owner.is_none());
        assert!(order.maker_address.is_none());
        assert!(order.original_size.is_none());
        assert!(order.price.is_none());
        assert!(order.associate_trades.is_empty());
    }

    #[test]
    fn open_order_with_full_fields() {
        let json = r#"{
            "id": "order-full",
            "market": "0xcond",
            "assetId": "0xtoken",
            "salt": "1",
            "maker": "0x0000000000000000000000000000000000000001",
            "signer": "0x0000000000000000000000000000000000000002",
            "tokenId": "0xtoken",
            "makerAmount": "1000",
            "takerAmount": "500",
            "side": "BUY",
            "expiration": "0",
            "signatureType": 0,
            "timestamp": "1700000000000",
            "metadata": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "builder": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "signature": "0xsig",
            "status": "LIVE",
            "owner": "0xowner",
            "makerAddress": "0xmaker",
            "originalSize": "200.5",
            "sizeMatched": "100.0",
            "price": "0.55",
            "associateTrades": ["trade-1", "trade-2"],
            "outcome": "Yes",
            "orderType": "GTC",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-02T00:00:00Z"
        }"#;
        let order: OpenOrder = serde_json::from_str(json).unwrap();
        assert_eq!(order.owner.as_deref(), Some("0xowner"));
        assert_eq!(order.maker_address.as_deref(), Some("0xmaker"));
        assert_eq!(order.original_size.as_deref(), Some("200.5"));
        assert_eq!(order.size_matched.as_deref(), Some("100.0"));
        assert_eq!(order.price.as_deref(), Some("0.55"));
        assert_eq!(order.associate_trades, vec!["trade-1", "trade-2"]);
        assert_eq!(order.outcome.as_deref(), Some("Yes"));
        assert_eq!(order.order_type.as_deref(), Some("GTC"));
        assert_eq!(order.updated_at.as_deref(), Some("2024-01-02T00:00:00Z"));
    }

    #[test]
    fn order_response_deserializes() {
        let json = r#"{
            "success": true,
            "errorMsg": null,
            "orderID": "order-789",
            "transactionsHashes": ["0xhash1", "0xhash2"],
            "status": "LIVE",
            "takingAmount": "500",
            "makingAmount": "1000"
        }"#;
        let resp: OrderResponse = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        assert!(resp.error_msg.is_none());
        assert_eq!(resp.order_id.as_deref(), Some("order-789"));
        assert_eq!(resp.transaction_hashes.len(), 2);
        assert_eq!(resp.status.as_deref(), Some("LIVE"));
        assert_eq!(resp.taking_amount.as_deref(), Some("500"));
        assert_eq!(resp.making_amount.as_deref(), Some("1000"));
    }

    #[test]
    fn order_response_defaults_transaction_hashes() {
        let json = r#"{"success": false, "errorMsg": "bad order"}"#;
        let resp: OrderResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.success);
        assert_eq!(resp.error_msg.as_deref(), Some("bad order"));
        assert!(resp.transaction_hashes.is_empty());
        assert!(resp.order_id.is_none());
        assert!(resp.status.is_none());
        assert!(resp.taking_amount.is_none());
        assert!(resp.making_amount.is_none());
    }

    #[test]
    fn batch_cancel_response_deserializes() {
        let json = r#"{
            "canceled": ["order-1", "order-2"],
            "notCanceled": {"order-3": "insufficient balance"}
        }"#;
        let resp: BatchCancelResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.canceled, vec!["order-1", "order-2"]);
        assert_eq!(resp.not_canceled.len(), 1);
        assert_eq!(
            resp.not_canceled.get("order-3").unwrap(),
            "insufficient balance"
        );
    }

    #[test]
    fn batch_cancel_response_defaults_empty() {
        let json = r#"{}"#;
        let resp: BatchCancelResponse = serde_json::from_str(json).unwrap();
        assert!(resp.canceled.is_empty());
        assert!(resp.not_canceled.is_empty());
    }

    #[test]
    fn batch_cancel_response_serializes() {
        let resp = BatchCancelResponse {
            canceled: vec!["a".into(), "b".into()],
            not_canceled: HashMap::from([("c".into(), "error".into())]),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["canceled"], serde_json::json!(["a", "b"]));
        assert_eq!(json["not_canceled"]["c"], "error");
    }

    #[test]
    fn list_orders_response_deserializes() {
        let json = r#"{
            "data": [{
                "id": "order-abc",
                "market": "0xcondition123",
                "assetId": "0xtoken456",
                "salt": "1",
                "maker": "0x0000000000000000000000000000000000000001",
                "signer": "0x0000000000000000000000000000000000000002",
                "tokenId": "0xtoken456",
                "makerAmount": "1000",
                "takerAmount": "500",
                "side": "BUY",
                "expiration": "0",
                "signatureType": 0,
                "timestamp": "1700000000000",
                "metadata": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "builder": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "signature": "0xsig",
                "status": "LIVE",
                "createdAt": "2024-01-01T00:00:00Z"
            }],
            "next_cursor": "MQ=="
        }"#;
        let resp: ListOrdersResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "order-abc");
        assert_eq!(resp.next_cursor.as_deref(), Some("MQ=="));
    }

    #[test]
    fn list_orders_response_empty() {
        let json = r#"{"data": [], "next_cursor": "LTE="}"#;
        let resp: ListOrdersResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
        assert_eq!(resp.next_cursor.as_deref(), Some("LTE="));
    }

    #[test]
    fn list_orders_response_null_cursor() {
        let json = r#"{"data": [], "next_cursor": null}"#;
        let resp: ListOrdersResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
        assert!(resp.next_cursor.is_none());
    }

    #[test]
    fn order_scoring_response_deserializes() {
        let json = r#"{"order_id": "order-1", "scoring": true}"#;
        let resp: OrderScoringResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.order_id, "order-1");
        assert!(resp.scoring);
    }

    #[test]
    fn order_scoring_response_batch_deserializes() {
        let json = r#"[
            {"order_id": "order-1", "scoring": true},
            {"order_id": "order-2", "scoring": false}
        ]"#;
        let resp: Vec<OrderScoringResponse> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 2);
        assert!(resp[0].scoring);
        assert!(!resp[1].scoring);
    }
}
