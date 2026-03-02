use polyoxide_core::{HttpClient, QueryBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    error::ClobError,
    request::{AuthMode, Request},
    types::SignedOrder,
};

/// RFQ (Request for Quote) namespace for OTC-style trading
#[derive(Clone)]
pub struct Rfq {
    pub(crate) http_client: HttpClient,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
}

impl Rfq {
    fn l2_auth(&self) -> AuthMode {
        AuthMode::L2 {
            address: self.wallet.address(),
            credentials: self.credentials.clone(),
            signer: self.signer.clone(),
        }
    }

    // ── Create / Cancel ──────────────────────────────────────────

    /// Create a new RFQ request
    pub async fn create_request(
        &self,
        params: &CreateRfqRequestParams,
    ) -> Result<RfqRequestResponse, ClobError> {
        Request::<RfqRequestResponse>::post(
            self.http_client.clone(),
            "/rfq/request".to_string(),
            self.l2_auth(),
            self.chain_id,
        )
        .body(params)?
        .send()
        .await
    }

    /// Cancel an RFQ request
    pub async fn cancel_request(
        &self,
        request_id: impl Into<String>,
    ) -> Result<serde_json::Value, ClobError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            request_id: String,
        }

        Request::<serde_json::Value>::delete(
            self.http_client.clone(),
            "/rfq/request",
            self.l2_auth(),
            self.chain_id,
        )
        .body(&Body {
            request_id: request_id.into(),
        })?
        .send()
        .await
    }

    /// Create a new RFQ quote
    pub async fn create_quote(
        &self,
        params: &CreateRfqQuoteParams,
    ) -> Result<RfqQuoteResponse, ClobError> {
        Request::<RfqQuoteResponse>::post(
            self.http_client.clone(),
            "/rfq/quote".to_string(),
            self.l2_auth(),
            self.chain_id,
        )
        .body(params)?
        .send()
        .await
    }

    /// Cancel an RFQ quote
    pub async fn cancel_quote(
        &self,
        quote_id: impl Into<String>,
    ) -> Result<serde_json::Value, ClobError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            quote_id: String,
        }

        Request::<serde_json::Value>::delete(
            self.http_client.clone(),
            "/rfq/quote",
            self.l2_auth(),
            self.chain_id,
        )
        .body(&Body {
            quote_id: quote_id.into(),
        })?
        .send()
        .await
    }

    // ── Accept / Approve ─────────────────────────────────────────

    /// Accept an RFQ request by submitting a signed order
    pub async fn accept_request(
        &self,
        request_id: impl Into<String>,
        quote_id: impl Into<String>,
        signed_order: &SignedOrder,
    ) -> Result<serde_json::Value, ClobError> {
        let payload = serde_json::json!({
            "requestId": request_id.into(),
            "quoteId": quote_id.into(),
            "owner": self.credentials.key,
            "order": signed_order,
        });

        Request::<serde_json::Value>::post(
            self.http_client.clone(),
            "/rfq/request/accept".to_string(),
            self.l2_auth(),
            self.chain_id,
        )
        .body(&payload)?
        .send()
        .await
    }

    /// Approve an RFQ quote by submitting a signed order
    pub async fn approve_quote(
        &self,
        request_id: impl Into<String>,
        quote_id: impl Into<String>,
        signed_order: &SignedOrder,
    ) -> Result<serde_json::Value, ClobError> {
        let payload = serde_json::json!({
            "requestId": request_id.into(),
            "quoteId": quote_id.into(),
            "owner": self.credentials.key,
            "order": signed_order,
        });

        Request::<serde_json::Value>::post(
            self.http_client.clone(),
            "/rfq/quote/approve".to_string(),
            self.l2_auth(),
            self.chain_id,
        )
        .body(&payload)?
        .send()
        .await
    }

    // ── Queries ──────────────────────────────────────────────────

    /// List RFQ requests with optional filtering
    pub fn list_requests(&self) -> ListRfqRequests {
        let request = Request::get(
            self.http_client.clone(),
            "/rfq/data/requests",
            self.l2_auth(),
            self.chain_id,
        );
        ListRfqRequests { request }
    }

    /// List quotes as the requester
    pub fn requester_quotes(&self) -> ListRfqQuotes {
        let request = Request::get(
            self.http_client.clone(),
            "/rfq/data/requester/quotes",
            self.l2_auth(),
            self.chain_id,
        );
        ListRfqQuotes { request }
    }

    /// List quotes as the quoter
    pub fn quoter_quotes(&self) -> ListRfqQuotes {
        let request = Request::get(
            self.http_client.clone(),
            "/rfq/data/quoter/quotes",
            self.l2_auth(),
            self.chain_id,
        );
        ListRfqQuotes { request }
    }

    /// Get the best quote for an RFQ request
    pub fn best_quote(&self, request_id: impl Into<String>) -> Request<RfqQuote> {
        Request::get(
            self.http_client.clone(),
            "/rfq/data/best-quote",
            self.l2_auth(),
            self.chain_id,
        )
        .query("requestId", request_id.into())
    }

    /// Get RFQ system configuration (no auth required)
    pub fn config(&self) -> Request<RfqConfig> {
        Request::get(
            self.http_client.clone(),
            "/rfq/config",
            AuthMode::None,
            self.chain_id,
        )
    }
}

// ── Query builders ───────────────────────────────────────────────

/// Request builder for listing RFQ requests
pub struct ListRfqRequests {
    request: Request<RfqPaginatedResponse<RfqRequest>>,
}

impl ListRfqRequests {
    /// Filter by request IDs
    pub fn request_ids(mut self, ids: impl Into<Vec<String>>) -> Self {
        self.request = self.request.query_many("request_ids", ids.into());
        self
    }

    /// Filter by state ("active" or "inactive")
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.request = self.request.query("state", state.into());
        self
    }

    /// Filter by markets (condition IDs)
    pub fn markets(mut self, markets: impl Into<Vec<String>>) -> Self {
        self.request = self.request.query_many("markets", markets.into());
        self
    }

    /// Minimum size filter
    pub fn size_min(mut self, min: f64) -> Self {
        self.request = self.request.query("size_min", min);
        self
    }

    /// Maximum size filter
    pub fn size_max(mut self, max: f64) -> Self {
        self.request = self.request.query("size_max", max);
        self
    }

    /// Minimum USDC size filter
    pub fn size_usdc_min(mut self, min: f64) -> Self {
        self.request = self.request.query("size_usdc_min", min);
        self
    }

    /// Maximum USDC size filter
    pub fn size_usdc_max(mut self, max: f64) -> Self {
        self.request = self.request.query("size_usdc_max", max);
        self
    }

    /// Minimum price filter
    pub fn price_min(mut self, min: f64) -> Self {
        self.request = self.request.query("price_min", min);
        self
    }

    /// Maximum price filter
    pub fn price_max(mut self, max: f64) -> Self {
        self.request = self.request.query("price_max", max);
        self
    }

    /// Sort by field ("price", "expiry", "size", "created")
    pub fn sort_by(mut self, field: impl Into<String>) -> Self {
        self.request = self.request.query("sort_by", field.into());
        self
    }

    /// Sort direction ("asc" or "desc")
    pub fn sort_dir(mut self, dir: impl Into<String>) -> Self {
        self.request = self.request.query("sort_dir", dir.into());
        self
    }

    /// Maximum number of results
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Pagination offset cursor
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.request = self.request.query("offset", offset.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<RfqPaginatedResponse<RfqRequest>, ClobError> {
        self.request.send().await
    }
}

/// Request builder for listing RFQ quotes
pub struct ListRfqQuotes {
    request: Request<RfqPaginatedResponse<RfqQuote>>,
}

impl ListRfqQuotes {
    /// Filter by quote IDs
    pub fn quote_ids(mut self, ids: impl Into<Vec<String>>) -> Self {
        self.request = self.request.query_many("quote_ids", ids.into());
        self
    }

    /// Filter by request IDs
    pub fn request_ids(mut self, ids: impl Into<Vec<String>>) -> Self {
        self.request = self.request.query_many("request_ids", ids.into());
        self
    }

    /// Filter by state ("active" or "inactive")
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.request = self.request.query("state", state.into());
        self
    }

    /// Filter by markets (condition IDs)
    pub fn markets(mut self, markets: impl Into<Vec<String>>) -> Self {
        self.request = self.request.query_many("markets", markets.into());
        self
    }

    /// Minimum size filter
    pub fn size_min(mut self, min: f64) -> Self {
        self.request = self.request.query("size_min", min);
        self
    }

    /// Maximum size filter
    pub fn size_max(mut self, max: f64) -> Self {
        self.request = self.request.query("size_max", max);
        self
    }

    /// Minimum USDC size filter
    pub fn size_usdc_min(mut self, min: f64) -> Self {
        self.request = self.request.query("size_usdc_min", min);
        self
    }

    /// Maximum USDC size filter
    pub fn size_usdc_max(mut self, max: f64) -> Self {
        self.request = self.request.query("size_usdc_max", max);
        self
    }

    /// Minimum price filter
    pub fn price_min(mut self, min: f64) -> Self {
        self.request = self.request.query("price_min", min);
        self
    }

    /// Maximum price filter
    pub fn price_max(mut self, max: f64) -> Self {
        self.request = self.request.query("price_max", max);
        self
    }

    /// Sort by field ("price", "expiry", "created")
    pub fn sort_by(mut self, field: impl Into<String>) -> Self {
        self.request = self.request.query("sort_by", field.into());
        self
    }

    /// Sort direction ("asc" or "desc")
    pub fn sort_dir(mut self, dir: impl Into<String>) -> Self {
        self.request = self.request.query("sort_dir", dir.into());
        self
    }

    /// Maximum number of results
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Pagination offset cursor
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.request = self.request.query("offset", offset.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<RfqPaginatedResponse<RfqQuote>, ClobError> {
        self.request.send().await
    }
}

// ── Request types ────────────────────────────────────────────────

/// Parameters for creating an RFQ request
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRfqRequestParams {
    pub asset_in: String,
    pub asset_out: String,
    pub amount_in: String,
    pub amount_out: String,
    pub user_type: u32,
}

/// Parameters for creating an RFQ quote
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRfqQuoteParams {
    pub request_id: String,
    pub asset_in: String,
    pub asset_out: String,
    pub amount_in: String,
    pub amount_out: String,
}

// ── Response types ───────────────────────────────────────────────

/// Response from creating an RFQ request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqRequestResponse {
    pub request_id: Option<String>,
    pub error: Option<String>,
}

/// Response from creating an RFQ quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqQuoteResponse {
    pub quote_id: Option<String>,
    pub error: Option<String>,
}

/// Paginated response wrapper for RFQ data endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqPaginatedResponse<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
    pub limit: Option<u32>,
    pub count: Option<u32>,
    pub total_count: Option<u32>,
}

/// An RFQ request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqRequest {
    pub request_id: String,
    pub user_address: String,
    #[serde(default)]
    pub proxy_address: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub complement: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size_in: Option<String>,
    #[serde(default)]
    pub size_out: Option<String>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub accepted_quote_id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub expiry: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// An RFQ quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqQuote {
    pub quote_id: String,
    pub request_id: String,
    pub user_address: String,
    #[serde(default)]
    pub proxy_address: Option<String>,
    #[serde(default)]
    pub complement: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size_in: Option<String>,
    #[serde(default)]
    pub size_out: Option<String>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub expiry: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// RFQ system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqConfig {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_rfq_request_params_serializes() {
        let params = CreateRfqRequestParams {
            asset_in: "0xtoken1".into(),
            asset_out: "0".into(),
            amount_in: "100".into(),
            amount_out: "50".into(),
            user_type: 0,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["assetIn"], "0xtoken1");
        assert_eq!(json["assetOut"], "0");
        assert_eq!(json["amountIn"], "100");
        assert_eq!(json["amountOut"], "50");
        assert_eq!(json["userType"], 0);
    }

    #[test]
    fn create_rfq_quote_params_serializes() {
        let params = CreateRfqQuoteParams {
            request_id: "req-123".into(),
            asset_in: "0xtoken1".into(),
            asset_out: "0".into(),
            amount_in: "100".into(),
            amount_out: "50".into(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["requestId"], "req-123");
        assert_eq!(json["assetIn"], "0xtoken1");
    }

    #[test]
    fn rfq_request_response_deserializes() {
        let json = r#"{"request_id": "req-abc", "error": null}"#;
        let resp: RfqRequestResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.request_id.as_deref(), Some("req-abc"));
        assert!(resp.error.is_none());
    }

    #[test]
    fn rfq_request_response_with_error() {
        let json = r#"{"request_id": null, "error": "invalid params"}"#;
        let resp: RfqRequestResponse = serde_json::from_str(json).unwrap();
        assert!(resp.request_id.is_none());
        assert_eq!(resp.error.as_deref(), Some("invalid params"));
    }

    #[test]
    fn rfq_quote_response_deserializes() {
        let json = r#"{"quote_id": "quote-xyz", "error": null}"#;
        let resp: RfqQuoteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.quote_id.as_deref(), Some("quote-xyz"));
    }

    #[test]
    fn rfq_request_deserializes() {
        let json = r#"{
            "request_id": "req-1",
            "user_address": "0xuser",
            "token": "0xtoken",
            "side": "BUY",
            "size_in": "100",
            "size_out": "50",
            "price": 0.5,
            "state": "active",
            "created_at": "2024-01-01T00:00:00Z"
        }"#;
        let req: RfqRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.request_id, "req-1");
        assert_eq!(req.user_address, "0xuser");
        assert_eq!(req.side.as_deref(), Some("BUY"));
        assert_eq!(req.price, Some(0.5));
        assert_eq!(req.state.as_deref(), Some("active"));
    }

    #[test]
    fn rfq_request_minimal_deserializes() {
        let json = r#"{"request_id": "req-1", "user_address": "0xuser"}"#;
        let req: RfqRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.request_id, "req-1");
        assert!(req.token.is_none());
        assert!(req.side.is_none());
        assert!(req.price.is_none());
    }

    #[test]
    fn rfq_quote_deserializes() {
        let json = r#"{
            "quote_id": "q-1",
            "request_id": "req-1",
            "user_address": "0xquoter",
            "token": "0xtoken",
            "side": "SELL",
            "price": 0.52,
            "state": "active"
        }"#;
        let quote: RfqQuote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.quote_id, "q-1");
        assert_eq!(quote.request_id, "req-1");
        assert_eq!(quote.price, Some(0.52));
    }

    #[test]
    fn rfq_paginated_response_deserializes() {
        let json = r#"{
            "data": [
                {"request_id": "req-1", "user_address": "0xuser1"},
                {"request_id": "req-2", "user_address": "0xuser2"}
            ],
            "next_cursor": "cursor-abc",
            "limit": 10,
            "count": 2,
            "total_count": 50
        }"#;
        let resp: RfqPaginatedResponse<RfqRequest> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].request_id, "req-1");
        assert_eq!(resp.next_cursor.as_deref(), Some("cursor-abc"));
        assert_eq!(resp.count, Some(2));
        assert_eq!(resp.total_count, Some(50));
    }

    #[test]
    fn rfq_paginated_response_empty() {
        let json = r#"{"data": []}"#;
        let resp: RfqPaginatedResponse<RfqQuote> = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
        assert!(resp.next_cursor.is_none());
    }

    #[test]
    fn rfq_config_deserializes() {
        let json = r#"{"min_size": 10, "max_expiry": 3600}"#;
        let config: RfqConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.data["min_size"], 10);
    }
}
