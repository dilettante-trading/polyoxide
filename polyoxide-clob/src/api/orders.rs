use std::collections::HashMap;

use polyoxide_core::{HttpClient, QueryBuilder, SignerLimiter, TradingRequest};
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    error::ClobError,
    request::{AuthMode, Request},
};

/// Orders namespace for order-related operations
#[derive(Clone)]
pub struct Orders {
    pub(crate) http_client: HttpClient,
    pub(crate) signer_limiter: SignerLimiter,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
}

impl Orders {
    /// List the user's open orders (`GET /data/orders`).
    ///
    /// Results are cursor-paginated; feed `next_cursor` from the response back
    /// via [`ListOrders::next_cursor`] to fetch the next page.
    pub fn list(&self) -> ListOrders {
        ListOrders {
            request: Request::get(
                self.http_client.clone(),
                "/data/orders",
                AuthMode::L2 {
                    address: self.wallet.address(),
                    credentials: self.credentials.clone(),
                    signer: self.signer.clone(),
                },
                self.chain_id,
            ),
        }
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
            signer_limiter: self.signer_limiter.clone(),
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
        .trading(&self.signer_limiter, TradingRequest::CancelAll)
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
        .trading(&self.signer_limiter, TradingRequest::CancelMarketOrders)
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
        .trading(
            &self.signer_limiter,
            TradingRequest::CancelOrders {
                count: ids.len() as u32,
            },
        )
        .body(&ids)?
        .send()
        .await
    }
}

/// Request builder for canceling an order
pub struct CancelOrderRequest {
    http_client: HttpClient,
    signer_limiter: SignerLimiter,
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
            .trading(&self.signer_limiter, TradingRequest::CancelOrder)
            .body(&request)?
            .send()
            .await
    }
}

/// A resting order, as returned by `GET /data/orders` and
/// `GET /data/order/{orderID}` (both return this same shape).
///
/// This is an order *summary*, not a signed order: the venue does not echo back
/// the signed payload, so there is no `token_id`, `maker_amount`,
/// `signature_type`, `metadata`, `builder`, `salt`, or `signature` here. Field
/// names are snake_case on the wire — there is deliberately no `rename_all`.
///
/// Pinned to a body captured from the live venue on 2026-07-24 (see
/// `open_order_deserializes_captured_response` below), cross-checked against
/// the `/data/orders` response schema in `docs/specs/clob/openapi.yaml`.
/// Prior to 0.22.0 this struct declared camelCase names, a flattened
/// [`SignedOrder`](crate::types::SignedOrder), and a string `created_at`, so **any** response containing a
/// real order failed to deserialize with ``missing field `assetId` ``.
///
/// Fields the schema marks required are non-optional here, matching the
/// capture. If the venue ever omits one, deserialization fails loudly rather
/// than silently producing a half-empty order — and
/// [`ListOrders::send_raw`] is the escape hatch for reading the body anyway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenOrder {
    /// Order ID (`0x`-prefixed hash).
    pub id: String,
    /// Order status, e.g. `LIVE`.
    pub status: String,
    /// Owning API key (a UUID, not an address).
    pub owner: String,
    /// Maker address — the proxy/deposit wallet, not the signing EOA.
    pub maker_address: String,
    /// Market condition ID.
    pub market: String,
    /// Asset (token) ID for the outcome being traded.
    pub asset_id: String,
    /// `BUY` or `SELL`.
    pub side: String,
    /// Original order size, as a decimal string.
    pub original_size: String,
    /// Size matched so far, as a decimal string.
    pub size_matched: String,
    /// Limit price, as a decimal string.
    pub price: String,
    /// Outcome label, e.g. `Yes`.
    pub outcome: String,
    /// Expiration as a Unix timestamp string; `"0"` means no expiry.
    pub expiration: String,
    /// Order type, e.g. `GTC`.
    pub order_type: String,
    /// IDs of trades associated with this order.
    #[serde(default)]
    pub associate_trades: Vec<String>,
    /// Creation time as a Unix timestamp in **seconds** — an integer on the
    /// wire, not a string.
    pub created_at: i64,
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

/// Request builder for listing open orders with optional filters.
pub struct ListOrders {
    request: Request<ListOrdersResponse>,
}

impl ListOrders {
    /// Filter by a specific order ID.
    pub fn id(mut self, order_id: impl Into<String>) -> Self {
        self.request = self.request.query("id", order_id.into());
        self
    }

    /// Filter by market (condition ID).
    pub fn market(mut self, condition_id: impl Into<String>) -> Self {
        self.request = self.request.query("market", condition_id.into());
        self
    }

    /// Filter by asset (token ID).
    pub fn asset_id(mut self, token_id: impl Into<String>) -> Self {
        self.request = self.request.query("asset_id", token_id.into());
        self
    }

    /// Continue from a pagination cursor.
    pub fn next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.request = self.request.query("next_cursor", cursor.into());
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<ListOrdersResponse, ClobError> {
        self.request.send().await
    }

    /// Execute the request and return the raw HTTP response, unparsed.
    ///
    /// An escape hatch for when [`OpenOrder`] cannot represent what the venue
    /// returned. Without it, a typed-struct mismatch is a hard block with no
    /// way for a caller to read the response at all — which is exactly what
    /// happened when `OpenOrder` expected an `assetId` the venue never sends.
    ///
    /// Also the way to capture a body for a test fixture:
    ///
    /// ```no_run
    /// # async fn doctest(clob: polyoxide_clob::Clob) -> Result<(), Box<dyn std::error::Error>> {
    /// let body = clob.orders()?.list().send_raw().await?.text().await?;
    /// println!("{body}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_raw(self) -> Result<reqwest::Response, ClobError> {
        self.request.send_raw().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim `GET /data/orders` body captured from the live venue on
    /// 2026-07-24, with identifiers redacted. Only the identifier *values* were
    /// changed — every key, type, and string/number distinction is as the venue
    /// sent it.
    ///
    /// Recapture with:
    /// `clob.orders()?.list().send_raw().await?.text().await?`
    /// (needs an account holding at least one resting order — with none, the
    /// body is `{"data":[],...}`, which is why this bug survived to 0.21.0).
    const CAPTURED_OPEN_ORDERS: &str = r#"{
        "data": [{
            "id": "0xc566ca3bb8d61f08d649214f7a5daf5041f5577c1e60c9131ae418aa62eff165",
            "status": "LIVE",
            "owner": "aa17dfae-754d-2498-f336-8bd1db84f525",
            "maker_address": "0xb98ad946c7f753596F26396Bf3F34A2EeBc39E86",
            "market": "0x7018d32e315a69c0537fc42f8e574ee4a24b3babaae302cda3d9f5f8e5b0bd6e",
            "asset_id": "84371186359433032934344934234365568331165136826770712345678901234567",
            "side": "BUY",
            "original_size": "5",
            "size_matched": "0",
            "price": "0.01",
            "outcome": "Yes",
            "expiration": "0",
            "order_type": "GTC",
            "associate_trades": [],
            "created_at": 1784930007
        }],
        "next_cursor": "LTE=",
        "limit": 100,
        "count": 1
    }"#;

    #[test]
    fn open_order_deserializes_captured_response() {
        // The regression guard. Before 0.22.0 this failed with
        // `missing field ` + "`assetId`" + `, because the struct declared
        // camelCase names the venue does not send.
        let resp: ListOrdersResponse = serde_json::from_str(CAPTURED_OPEN_ORDERS)
            .expect("captured live body must deserialize");
        assert_eq!(resp.data.len(), 1);

        let o = &resp.data[0];
        assert_eq!(o.status, "LIVE");
        assert_eq!(o.side, "BUY");
        assert_eq!(o.original_size, "5");
        assert_eq!(o.size_matched, "0");
        assert_eq!(o.price, "0.01");
        assert_eq!(o.outcome, "Yes");
        assert_eq!(o.expiration, "0");
        assert_eq!(o.order_type, "GTC");
        assert!(o.associate_trades.is_empty());
        // Integer on the wire, not a string.
        assert_eq!(o.created_at, 1_784_930_007);
        // maker_address is the proxy wallet, distinct from the signing EOA.
        assert!(o.maker_address.starts_with("0x"));
        // owner is an API-key UUID, not an address.
        assert!(!o.owner.starts_with("0x"));
        assert_eq!(resp.next_cursor.as_deref(), Some("LTE="));
    }

    #[test]
    fn open_order_rejects_the_camel_case_shape_it_used_to_expect() {
        // Negative control. If someone reintroduces `rename_all(camelCase)`,
        // the positive test above starts failing and this one starts passing —
        // so the pair cannot both be satisfied by a single convention.
        let json = r#"{"id":"x","status":"LIVE","owner":"o","makerAddress":"0x1",
            "market":"0x2","assetId":"0x3","side":"BUY","originalSize":"5",
            "sizeMatched":"0","price":"0.01","outcome":"Yes","expiration":"0",
            "orderType":"GTC","associateTrades":[],"createdAt":1}"#;
        assert!(
            serde_json::from_str::<OpenOrder>(json).is_err(),
            "camelCase must not deserialize; the venue sends snake_case"
        );
    }

    #[test]
    fn open_order_created_at_must_be_an_integer() {
        // The venue sends a Unix-seconds integer. A string here was part of the
        // original bug and would silently pass if the field were `String`.
        let json = CAPTURED_OPEN_ORDERS.replace("1784930007", "\"2024-01-01T00:00:00Z\"");
        assert!(
            serde_json::from_str::<ListOrdersResponse>(&json).is_err(),
            "a string created_at must be rejected"
        );
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
