use polyoxide_core::{HttpClient, QueryBuilder};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    error::ClobError,
    request::{AuthMode, Request},
    types::OrderSide,
};

/// Markets namespace for market-related operations
#[derive(Clone)]
pub struct Markets {
    pub(crate) http_client: HttpClient,
    pub(crate) chain_id: u64,
}

impl Markets {
    /// Get a market by condition ID
    pub fn get(&self, condition_id: impl Into<String>) -> Request<Market> {
        Request::get(
            self.http_client.clone(),
            format!("/markets/{}", urlencoding::encode(&condition_id.into())),
            AuthMode::None,
            self.chain_id,
        )
    }

    pub fn get_by_token_ids(
        &self,
        token_ids: impl Into<Vec<String>>,
    ) -> Request<ListMarketsResponse> {
        Request::get(
            self.http_client.clone(),
            "/markets",
            AuthMode::None,
            self.chain_id,
        )
        .query_many("clob_token_ids", token_ids.into())
    }

    /// List all markets
    pub fn list(&self) -> Request<ListMarketsResponse> {
        Request::get(
            self.http_client.clone(),
            "/markets",
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Get order book for a token
    pub fn order_book(&self, token_id: impl Into<String>) -> Request<OrderBook> {
        Request::get(
            self.http_client.clone(),
            "/book",
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    /// Get price for a token and side
    pub fn price(&self, token_id: impl Into<String>, side: OrderSide) -> Request<PriceResponse> {
        Request::get(
            self.http_client.clone(),
            "/price",
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
        .query("side", side.as_str())
    }

    /// Get midpoint price for a token
    pub fn midpoint(&self, token_id: impl Into<String>) -> Request<MidpointResponse> {
        Request::get(
            self.http_client.clone(),
            "/midpoint",
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    /// Get historical prices for a token
    pub fn prices_history(&self, token_id: impl Into<String>) -> Request<PricesHistoryResponse> {
        Request::get(
            self.http_client.clone(),
            "/prices-history",
            AuthMode::None,
            self.chain_id,
        )
        .query("market", token_id.into())
    }

    /// Get neg_risk status for a token
    pub fn neg_risk(&self, token_id: impl Into<String>) -> Request<NegRiskResponse> {
        Request::get(
            self.http_client.clone(),
            "/neg-risk".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    /// Get the current fee rate for a token
    pub fn fee_rate(&self, token_id: impl Into<String>) -> Request<FeeRateResponse> {
        Request::get(
            self.http_client.clone(),
            "/fee-rate",
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    /// Get tick size for a token
    pub fn tick_size(&self, token_id: impl Into<String>) -> Request<TickSizeResponse> {
        Request::get(
            self.http_client.clone(),
            "/tick-size".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    /// Get bid-ask spread for a token
    pub fn spread(&self, token_id: impl Into<String>) -> Request<SpreadResponse> {
        Request::get(
            self.http_client.clone(),
            "/spread",
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    /// Get last trade price for a token
    pub fn last_trade_price(&self, token_id: impl Into<String>) -> Request<LastTradePriceResponse> {
        Request::get(
            self.http_client.clone(),
            "/last-trade-price",
            AuthMode::None,
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    /// Get live activity events for a market
    pub fn live_activity(
        &self,
        condition_id: impl Into<String>,
    ) -> Request<Vec<LiveActivityEvent>> {
        Request::get(
            self.http_client.clone(),
            format!(
                "/live-activity/events/{}",
                urlencoding::encode(&condition_id.into())
            ),
            AuthMode::None,
            self.chain_id,
        )
    }

    /// List simplified markets (reduced payload for performance)
    pub fn simplified(&self) -> Request<ListMarketsResponse> {
        Request::get(
            self.http_client.clone(),
            "/simplified-markets",
            AuthMode::None,
            self.chain_id,
        )
    }

    /// List sampling markets
    pub fn sampling(&self) -> Request<ListMarketsResponse> {
        Request::get(
            self.http_client.clone(),
            "/sampling-markets",
            AuthMode::None,
            self.chain_id,
        )
    }

    /// List sampling simplified markets
    pub fn sampling_simplified(&self) -> Request<ListMarketsResponse> {
        Request::get(
            self.http_client.clone(),
            "/sampling-simplified-markets",
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Calculate estimated execution price for a market order
    pub async fn calculate_price(
        &self,
        token_id: impl Into<String>,
        side: OrderSide,
        amount: impl Into<String>,
    ) -> Result<CalculatePriceResponse, ClobError> {
        Request::<CalculatePriceResponse>::post(
            self.http_client.clone(),
            "/calculate-price".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(&CalculatePriceParams {
            token_id: token_id.into(),
            side,
            amount: amount.into(),
        })?
        .send()
        .await
    }

    /// Get order books for multiple tokens
    pub async fn order_books(&self, params: &[BookParams]) -> Result<Vec<OrderBook>, ClobError> {
        Request::<Vec<OrderBook>>::post(
            self.http_client.clone(),
            "/books".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(params)?
        .send()
        .await
    }

    /// Get prices for multiple tokens
    pub async fn prices(&self, params: &[BookParams]) -> Result<Vec<PriceResponse>, ClobError> {
        Request::<Vec<PriceResponse>>::post(
            self.http_client.clone(),
            "/prices".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(params)?
        .send()
        .await
    }

    /// Get midpoints for multiple tokens
    pub async fn midpoints(
        &self,
        params: &[BookParams],
    ) -> Result<Vec<MidpointResponse>, ClobError> {
        Request::<Vec<MidpointResponse>>::post(
            self.http_client.clone(),
            "/midpoints".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(params)?
        .send()
        .await
    }

    /// Get spreads for multiple tokens
    pub async fn spreads(&self, params: &[BookParams]) -> Result<Vec<SpreadResponse>, ClobError> {
        Request::<Vec<SpreadResponse>>::post(
            self.http_client.clone(),
            "/spreads".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(params)?
        .send()
        .await
    }

    /// Get last trade prices for multiple tokens
    pub async fn last_trade_prices(
        &self,
        params: &[BookParams],
    ) -> Result<Vec<LastTradePriceResponse>, ClobError> {
        Request::<Vec<LastTradePriceResponse>>::post(
            self.http_client.clone(),
            "/last-trades-prices".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(params)?
        .send()
        .await
    }
}

/// Market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub condition_id: String,
    pub question_id: String,
    pub tokens: Vec<MarketToken>,
    #[serde(default)]
    pub rewards: Option<serde_json::Value>,
    pub minimum_order_size: f64,
    pub minimum_tick_size: f64,
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub end_date_iso: Option<String>,
    pub question: String,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    #[serde(default)]
    pub neg_risk: Option<bool>,
    #[serde(default)]
    pub neg_risk_market_id: Option<String>,
    #[serde(default)]
    pub enable_order_book: Option<bool>,
}

/// Markets list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMarketsResponse {
    pub data: Vec<Market>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// Market token (outcome)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketToken {
    #[serde(default)]
    pub token_id: Option<String>,
    pub outcome: String,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub winner: Option<bool>,
}

/// Order book level (price and size)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLevel {
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub size: Decimal,
}

/// Order book data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub market: String,
    pub asset_id: String,
    pub bids: Vec<OrderLevel>,
    pub asks: Vec<OrderLevel>,
    pub timestamp: String,
    pub hash: String,
    #[serde(default)]
    pub min_order_size: Option<String>,
    #[serde(default)]
    pub tick_size: Option<String>,
    #[serde(default)]
    pub neg_risk: Option<bool>,
    #[serde(default)]
    pub last_trade_price: Option<String>,
}

/// Price response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceResponse {
    pub price: String,
}

/// Midpoint price response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidpointResponse {
    pub mid: String,
}

/// A single point in the price history timeseries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHistoryPoint {
    /// Unix timestamp (seconds)
    #[serde(rename = "t")]
    pub timestamp: i64,
    /// Price at this point in time
    #[serde(rename = "p")]
    pub price: f64,
}

/// Response from the prices-history endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricesHistoryResponse {
    pub history: Vec<PriceHistoryPoint>,
}

/// Response from the neg-risk endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegRiskResponse {
    pub neg_risk: bool,
}

/// Response from the fee-rate endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRateResponse {
    pub base_fee: u32,
}

/// Response from the tick-size endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSizeResponse {
    #[serde(deserialize_with = "deserialize_tick_size")]
    pub minimum_tick_size: String,
}

/// Parameters for batch pricing requests
#[derive(Debug, Clone, Serialize)]
pub struct BookParams {
    pub token_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<OrderSide>,
}

/// Spread response (bid-ask spread for a token)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadResponse {
    pub token_id: String,
    pub spread: String,
    pub bid: String,
    pub ask: String,
}

/// Last trade price response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastTradePriceResponse {
    pub token_id: String,
    pub last_trade_price: String,
    pub timestamp: String,
}

/// A live activity event for a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveActivityEvent {
    pub condition_id: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for the calculate-price endpoint
#[derive(Debug, Clone, Serialize)]
pub struct CalculatePriceParams {
    pub token_id: String,
    pub side: OrderSide,
    pub amount: String,
}

/// Response from the calculate-price endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatePriceResponse {
    pub price: String,
}

fn deserialize_tick_size<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom(
            "expected string or number for tick size",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_rate_response_deserializes() {
        let json = r#"{"base_fee": 100}"#;
        let resp: FeeRateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.base_fee, 100);
    }

    #[test]
    fn test_fee_rate_response_deserializes_zero() {
        let json = r#"{"base_fee": 0}"#;
        let resp: FeeRateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.base_fee, 0);
    }

    #[test]
    fn test_fee_rate_response_rejects_missing_field() {
        let json = r#"{"feeRate": "100"}"#;
        let result = serde_json::from_str::<FeeRateResponse>(json);
        assert!(result.is_err(), "Should reject JSON missing base_fee field");
    }

    #[test]
    fn test_fee_rate_response_rejects_empty_json() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<FeeRateResponse>(json);
        assert!(result.is_err(), "Should reject empty JSON object");
    }

    #[test]
    fn book_params_serializes() {
        let params = BookParams {
            token_id: "token-1".into(),
            side: Some(OrderSide::Buy),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["token_id"], "token-1");
        assert_eq!(json["side"], "BUY");
    }

    #[test]
    fn book_params_omits_none_side() {
        let params = BookParams {
            token_id: "token-1".into(),
            side: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["token_id"], "token-1");
        assert!(json.get("side").is_none());
    }

    #[test]
    fn spread_response_deserializes() {
        let json = r#"{
            "token_id": "token-1",
            "spread": "0.02",
            "bid": "0.48",
            "ask": "0.50"
        }"#;
        let resp: SpreadResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token_id, "token-1");
        assert_eq!(resp.spread, "0.02");
        assert_eq!(resp.bid, "0.48");
        assert_eq!(resp.ask, "0.50");
    }

    #[test]
    fn last_trade_price_response_deserializes() {
        let json = r#"{
            "token_id": "token-1",
            "last_trade_price": "0.55",
            "timestamp": "1700000000"
        }"#;
        let resp: LastTradePriceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token_id, "token-1");
        assert_eq!(resp.last_trade_price, "0.55");
        assert_eq!(resp.timestamp, "1700000000");
    }

    #[test]
    fn live_activity_event_deserializes_with_extra_fields() {
        let json = r#"{
            "condition_id": "0xabc123",
            "event_type": "trade",
            "amount": 100
        }"#;
        let event: LiveActivityEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.condition_id, "0xabc123");
        assert_eq!(event.extra["event_type"], "trade");
        assert_eq!(event.extra["amount"], 100);
    }

    #[test]
    fn calculate_price_params_serializes() {
        let params = CalculatePriceParams {
            token_id: "token-1".into(),
            side: OrderSide::Buy,
            amount: "100.0".into(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["token_id"], "token-1");
        assert_eq!(json["side"], "BUY");
        assert_eq!(json["amount"], "100.0");
    }

    #[test]
    fn calculate_price_response_deserializes() {
        let json = r#"{"price": "0.52"}"#;
        let resp: CalculatePriceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.price, "0.52");
    }

    #[test]
    fn order_book_deserializes_with_new_fields() {
        let json = r#"{
            "market": "0xcond",
            "asset_id": "0xtoken",
            "bids": [{"price": "0.48", "size": "100"}],
            "asks": [{"price": "0.52", "size": "200"}],
            "timestamp": "1700000000",
            "hash": "abc123",
            "min_order_size": "5",
            "tick_size": "0.001",
            "neg_risk": false,
            "last_trade_price": "0.50"
        }"#;
        let ob: OrderBook = serde_json::from_str(json).unwrap();
        assert_eq!(ob.market, "0xcond");
        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.asks.len(), 1);
        assert_eq!(ob.min_order_size.as_deref(), Some("5"));
        assert_eq!(ob.tick_size.as_deref(), Some("0.001"));
        assert_eq!(ob.neg_risk, Some(false));
        assert_eq!(ob.last_trade_price.as_deref(), Some("0.50"));
    }

    #[test]
    fn order_book_deserializes_without_new_fields() {
        let json = r#"{
            "market": "0xcond",
            "asset_id": "0xtoken",
            "bids": [],
            "asks": [],
            "timestamp": "1700000000",
            "hash": "abc123"
        }"#;
        let ob: OrderBook = serde_json::from_str(json).unwrap();
        assert_eq!(ob.market, "0xcond");
        assert!(ob.min_order_size.is_none());
        assert!(ob.tick_size.is_none());
        assert!(ob.neg_risk.is_none());
        assert!(ob.last_trade_price.is_none());
    }
}
