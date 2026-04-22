use std::collections::HashMap;

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

    /// Get neg_risk flag via path parameter (`GET /neg-risk/{token_id}`).
    pub fn neg_risk_path(&self, token_id: impl Into<String>) -> Request<NegRiskResponse> {
        Request::get(
            self.http_client.clone(),
            format!("/neg-risk/{}", urlencoding::encode(&token_id.into())),
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Get fee rate via path parameter (`GET /fee-rate/{token_id}`).
    pub fn fee_rate_path(&self, token_id: impl Into<String>) -> Request<FeeRateResponse> {
        Request::get(
            self.http_client.clone(),
            format!("/fee-rate/{}", urlencoding::encode(&token_id.into())),
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Get tick size via path parameter (`GET /tick-size/{token_id}`).
    pub fn tick_size_path(&self, token_id: impl Into<String>) -> Request<TickSizeResponse> {
        Request::get(
            self.http_client.clone(),
            format!("/tick-size/{}", urlencoding::encode(&token_id.into())),
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Get CLOB-level market details (`GET /clob-markets/{condition_id}`).
    ///
    /// Returns the full set of CLOB parameters for a market: tokens, tick size,
    /// base fees, rewards, RFQ status, and fee-curve details.
    pub fn clob_market_details(
        &self,
        condition_id: impl Into<String>,
    ) -> Request<ClobMarketDetails> {
        Request::get(
            self.http_client.clone(),
            format!(
                "/clob-markets/{}",
                urlencoding::encode(&condition_id.into())
            ),
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Resolve a market by its token ID (`GET /markets-by-token/{token_id}`).
    ///
    /// Returns the condition ID and both token IDs for the market that owns
    /// the given token ID.
    pub fn market_by_token(&self, token_id: impl Into<String>) -> Request<MarketByTokenResponse> {
        Request::get(
            self.http_client.clone(),
            format!(
                "/markets-by-token/{}",
                urlencoding::encode(&token_id.into())
            ),
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Get minimal live-activity data for a single market
    /// (`GET /markets/live-activity/{condition_id}`).
    pub fn live_activity_market(
        &self,
        condition_id: impl Into<String>,
    ) -> Request<LiveActivityMarket> {
        Request::get(
            self.http_client.clone(),
            format!(
                "/markets/live-activity/{}",
                urlencoding::encode(&condition_id.into())
            ),
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Get minimal live-activity data for multiple markets
    /// (`POST /markets/live-activity`).
    pub fn live_activity_bulk(
        &self,
        condition_ids: Vec<String>,
    ) -> Result<Request<Vec<LiveActivityMarket>>, ClobError> {
        Request::<Vec<LiveActivityMarket>>::post(
            self.http_client.clone(),
            "/markets/live-activity".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(&condition_ids)
    }

    /// Get batched historical prices for multiple markets
    /// (`POST /batch-prices-history`).
    pub fn batch_prices_history(
        &self,
        req: &BatchPricesHistoryRequest,
    ) -> Result<Request<BatchPricesHistoryResponse>, ClobError> {
        Request::<BatchPricesHistoryResponse>::post(
            self.http_client.clone(),
            "/batch-prices-history".to_string(),
            AuthMode::None,
            self.chain_id,
        )
        .body(req)
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
    pub question_id: Option<String>,
    pub tokens: Vec<MarketToken>,
    pub rewards: Option<serde_json::Value>,
    pub minimum_order_size: Option<f64>,
    pub minimum_tick_size: Option<f64>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub end_date_iso: Option<String>,
    pub question: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub accepting_orders: Option<bool>,
    pub neg_risk: Option<bool>,
    pub neg_risk_market_id: Option<String>,
    pub enable_order_book: Option<bool>,
}

/// Markets list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMarketsResponse {
    pub data: Vec<Market>,
    pub next_cursor: Option<String>,
}

/// Market token (outcome)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketToken {
    pub token_id: Option<String>,
    pub outcome: String,
    pub price: Option<f64>,
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
    pub min_order_size: Option<String>,
    pub tick_size: Option<String>,
    #[serde(default)]
    pub neg_risk: Option<bool>,
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
    pub token_id: Option<String>,
    pub spread: String,
    pub bid: Option<String>,
    pub ask: Option<String>,
}

/// Last trade price response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastTradePriceResponse {
    pub token_id: Option<String>,
    pub price: Option<String>,
    pub last_trade_price: Option<String>,
    pub side: Option<String>,
    pub timestamp: Option<String>,
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

/// A token in a CLOB market with its ID and outcome label.
///
/// Field names are abbreviated to match the wire format:
/// `t` = token ID, `o` = outcome label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClobToken {
    /// Token ID
    pub t: String,
    /// Outcome label (e.g. "Yes", "No")
    pub o: String,
}

/// Fee curve parameters for a market.
///
/// Field names are abbreviated to match the wire format:
/// `r` = rate, `e` = exponent, `to` = takers only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeDetails {
    /// Fee rate
    pub r: Option<f64>,
    /// Fee curve exponent
    pub e: Option<f64>,
    /// Whether fees apply to takers only
    pub to: Option<bool>,
}

/// Rewards configuration for a market.
///
/// The upstream OpenAPI spec declares this object with `additionalProperties: true`
/// and no explicit fields, so we model it as a free-form map.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClobRewards {
    /// Arbitrary rewards payload. Structure is market-dependent.
    pub extra: HashMap<String, serde_json::Value>,
}

/// CLOB-level parameters for a market.
///
/// Returned by `GET /clob-markets/{condition_id}`. Field names are intentionally
/// abbreviated to match the wire format:
/// `gst` = game start time, `r` = rewards, `t` = tokens, `mos` = minimum order size,
/// `mts` = minimum tick size, `mbf` = maker base fee, `tbf` = taker base fee,
/// `rfqe` = RFQ enabled, `itode` = taker order delay enabled,
/// `ibce` = Blockaid check enabled, `fd` = fee details,
/// `oas` = minimum order age (seconds).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClobMarketDetails {
    /// Game start time (sports markets). ISO 8601 timestamp or null.
    pub gst: Option<String>,
    /// Rewards configuration
    pub r: ClobRewards,
    /// Tokens for this market
    pub t: Vec<ClobToken>,
    /// Minimum order size
    pub mos: f64,
    /// Minimum tick size (price increment)
    pub mts: f64,
    /// Maker base fee (basis points)
    pub mbf: i64,
    /// Taker base fee (basis points)
    pub tbf: i64,
    /// Whether RFQ is enabled for this market
    pub rfqe: bool,
    /// Whether taker order delay is enabled
    pub itode: bool,
    /// Whether Blockaid check is enabled
    pub ibce: bool,
    /// Fee curve parameters
    pub fd: FeeDetails,
    /// Minimum order age in seconds
    pub oas: i32,
}

/// Response for `GET /markets-by-token/{token_id}`: the condition ID and
/// both token IDs of the market containing the given token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketByTokenResponse {
    /// The condition ID of the market containing the given token
    pub condition_id: String,
    /// The primary (Yes) token ID
    pub primary_token_id: String,
    /// The secondary (No) token ID
    pub secondary_token_id: String,
}

/// Minimal market information for live-activity widgets
/// (`GET /markets/live-activity/{condition_id}` and bulk variant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveActivityMarket {
    /// Unique identifier for the market condition
    pub condition_id: Option<String>,
    /// Internal market ID
    pub id: Option<i64>,
    /// The market question being asked
    pub question: Option<String>,
    /// URL-friendly slug for the market
    pub market_slug: Option<String>,
    /// URL-friendly slug for the parent event
    pub event_slug: Option<String>,
    /// URL-friendly slug for the series (if applicable)
    pub series_slug: Option<String>,
    /// URL to the market icon image
    pub icon: Option<String>,
    /// URL to the market image
    pub image: Option<String>,
    /// List of tag slugs associated with this market
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A single price point in `BatchPricesHistoryResponse`.
///
/// Field names are abbreviated to match the wire format:
/// `t` = unix timestamp (seconds), `p` = price.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPrice {
    /// Unix timestamp (seconds)
    pub t: u32,
    /// Price at this point in time
    pub p: f64,
}

/// Request body for `POST /batch-prices-history`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchPricesHistoryRequest {
    /// List of market asset ids to query (maximum 20).
    pub markets: Vec<String>,
    /// Filter by items after this unix timestamp (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ts: Option<f64>,
    /// Filter by items before this unix timestamp (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ts: Option<f64>,
    /// Time interval for data aggregation (`max`, `all`, `1m`, `1w`, `1d`, `6h`, `1h`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Accuracy of the data expressed in minutes. Default is 1 minute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<i32>,
}

/// Response body for `POST /batch-prices-history`: a mapping of market asset
/// id to its list of price points.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchPricesHistoryResponse {
    /// Map of market asset id to array of price data points.
    pub history: HashMap<String, Vec<MarketPrice>>,
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
        assert_eq!(resp.token_id.as_deref(), Some("token-1"));
        assert_eq!(resp.spread, "0.02");
        assert_eq!(resp.bid.as_deref(), Some("0.48"));
        assert_eq!(resp.ask.as_deref(), Some("0.50"));
    }

    #[test]
    fn last_trade_price_response_deserializes() {
        let json = r#"{
            "token_id": "token-1",
            "last_trade_price": "0.55",
            "timestamp": "1700000000"
        }"#;
        let resp: LastTradePriceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token_id.as_deref(), Some("token-1"));
        assert_eq!(resp.last_trade_price.as_deref(), Some("0.55"));
        assert_eq!(resp.timestamp.as_deref(), Some("1700000000"));
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

    #[test]
    fn clob_market_details_roundtrip() {
        // Shape lifted from docs/specs/clob/openapi.yaml ClobMarketDetails example.
        let json = r#"{
            "gst": null,
            "r": {"minSize": 100, "maxSpread": 2.0},
            "t": [
                {"t": "71321045679252212594626385532706912750332728571942532289631379312455583992563", "o": "Yes"},
                {"t": "52114319501245915516055106046884209969926127482827954674443846427813813222426", "o": "No"}
            ],
            "mos": 5.0,
            "mts": 0.01,
            "mbf": 0,
            "tbf": 0,
            "rfqe": true,
            "itode": false,
            "ibce": true,
            "fd": {"r": 0.02, "e": 2.0, "to": true},
            "oas": 0
        }"#;
        let parsed: ClobMarketDetails = serde_json::from_str(json).unwrap();
        assert!(parsed.gst.is_none());
        assert_eq!(parsed.t.len(), 2);
        assert_eq!(parsed.t[0].o, "Yes");
        assert!((parsed.mos - 5.0).abs() < f64::EPSILON);
        assert!((parsed.mts - 0.01).abs() < f64::EPSILON);
        assert_eq!(parsed.mbf, 0);
        assert_eq!(parsed.tbf, 0);
        assert!(parsed.rfqe);
        assert!(!parsed.itode);
        assert!(parsed.ibce);
        assert_eq!(parsed.fd.r, Some(0.02));
        assert_eq!(parsed.fd.e, Some(2.0));
        assert_eq!(parsed.fd.to, Some(true));
        assert_eq!(parsed.oas, 0);
        // Free-form rewards captured via flatten
        assert!(parsed.r.extra.contains_key("minSize"));

        // Ensure roundtrip: serialize then deserialize again produces equivalent data.
        let back = serde_json::to_value(&parsed).unwrap();
        let again: ClobMarketDetails = serde_json::from_value(back).unwrap();
        assert_eq!(again.t.len(), 2);
        assert_eq!(again.fd.r, Some(0.02));
    }

    #[test]
    fn market_by_token_response_deserializes() {
        let json = r#"{
            "condition_id": "0xbd31dc8a",
            "primary_token_id": "713210456",
            "secondary_token_id": "521143195"
        }"#;
        let parsed: MarketByTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.condition_id, "0xbd31dc8a");
        assert_eq!(parsed.primary_token_id, "713210456");
        assert_eq!(parsed.secondary_token_id, "521143195");
    }

    #[test]
    fn live_activity_market_roundtrip() {
        let json = r#"{
            "condition_id": "0xcond",
            "id": 42,
            "question": "Will X happen?",
            "market_slug": "will-x-happen",
            "event_slug": "x-event",
            "series_slug": null,
            "icon": "https://icon",
            "image": "https://image",
            "tags": ["crypto", "sports"]
        }"#;
        let parsed: LiveActivityMarket = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.condition_id.as_deref(), Some("0xcond"));
        assert_eq!(parsed.id, Some(42));
        assert_eq!(parsed.question.as_deref(), Some("Will X happen?"));
        assert_eq!(parsed.market_slug.as_deref(), Some("will-x-happen"));
        assert_eq!(parsed.event_slug.as_deref(), Some("x-event"));
        assert!(parsed.series_slug.is_none());
        assert_eq!(parsed.tags, vec!["crypto", "sports"]);

        // Roundtrip
        let back: LiveActivityMarket =
            serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
        assert_eq!(back.id, Some(42));
    }

    #[test]
    fn batch_prices_history_request_omits_none_fields() {
        let req = BatchPricesHistoryRequest {
            markets: vec!["0xtoken1".into(), "0xtoken2".into()],
            start_ts: Some(1_700_000_000.0),
            end_ts: None,
            interval: Some("1d".into()),
            fidelity: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["markets"][0], "0xtoken1");
        assert!((json["start_ts"].as_f64().unwrap() - 1_700_000_000.0).abs() < f64::EPSILON);
        assert_eq!(json["interval"], "1d");
        assert!(json.get("end_ts").is_none());
        assert!(json.get("fidelity").is_none());
    }

    #[test]
    fn batch_prices_history_response_roundtrip() {
        let json = r#"{
            "history": {
                "0xtokenA": [
                    {"t": 1700000000, "p": 0.55},
                    {"t": 1700001000, "p": 0.60}
                ],
                "0xtokenB": [
                    {"t": 1700000000, "p": 0.30}
                ]
            }
        }"#;
        let parsed: BatchPricesHistoryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.history.len(), 2);
        let a = parsed.history.get("0xtokenA").unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].t, 1_700_000_000);
        assert!((a[0].p - 0.55).abs() < f64::EPSILON);
        let b = parsed.history.get("0xtokenB").unwrap();
        assert_eq!(b.len(), 1);
        assert!((b[0].p - 0.30).abs() < f64::EPSILON);

        // Roundtrip
        let back: BatchPricesHistoryResponse =
            serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
        assert_eq!(back.history.len(), 2);
    }
}
