use std::sync::Arc;

use polyoxide_core::{DynAuthenticator, HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::{error::ClobError, types::SignatureType};

/// Rewards namespace for liquidity reward operations
#[derive(Clone)]
pub struct Rewards {
    pub(crate) http_client: HttpClient,
    pub(crate) l2: Arc<DynAuthenticator<'static>>,
    pub(crate) signature_type: SignatureType,
}

impl Rewards {
    /// Get user earnings for a specific day (`GET /rewards/user`).
    ///
    /// `date` must be in `YYYY-MM-DD` format (required by the API). The
    /// `signature_type` query parameter is taken from the client configuration.
    pub fn earnings(&self, date: impl Into<String>) -> UserEarningsRequest {
        UserEarningsRequest {
            request: Request::new(self.http_client.clone(), "/rewards/user")
                .authenticator(self.l2.clone())
                .query("date", date.into())
                .query("signature_type", self.signature_type as u8),
        }
    }

    /// Get user total earnings for a specific day (`GET /rewards/user/total`).
    ///
    /// `date` must be in `YYYY-MM-DD` format (required by the API). The endpoint
    /// returns an array of totals grouped by asset address.
    pub fn total_earnings(&self, date: impl Into<String>) -> UserTotalEarningsRequest {
        UserTotalEarningsRequest {
            request: Request::new(self.http_client.clone(), "/rewards/user/total")
                .authenticator(self.l2.clone())
                .query("date", date.into())
                .query("signature_type", self.signature_type as u8),
        }
    }

    /// Get user reward percentages (`GET /rewards/user/percentages`).
    pub fn percentages(&self) -> UserPercentagesRequest {
        UserPercentagesRequest {
            request: Request::new(self.http_client.clone(), "/rewards/user/percentages")
                .authenticator(self.l2.clone())
                .query("signature_type", self.signature_type as u8),
        }
    }

    /// Get user earnings broken down by market (`GET /rewards/user/markets`).
    ///
    /// Returns a paginated envelope; the per-market earnings are in `data`.
    /// Pages hold 100 items by default (max 500); feed `next_cursor` back via
    /// [`ListUserRewardMarkets::next_cursor`] until it reads `"LTE="`.
    pub fn market_earnings(&self) -> ListUserRewardMarkets {
        ListUserRewardMarkets {
            request: Request::new(self.http_client.clone(), "/rewards/user/markets")
                .authenticator(self.l2.clone())
                .query("signature_type", self.signature_type as u8),
        }
    }

    /// The unauthenticated subset of the rewards API.
    fn public(&self) -> PublicRewards {
        PublicRewards {
            http_client: self.http_client.clone(),
        }
    }

    /// Get currently active reward markets (`GET /rewards/markets/current`).
    ///
    /// Returns a paginated envelope (`data`, `next_cursor`, `limit`, `count`);
    /// the reward markets are in the `data` field. Also available without an
    /// account via [`Clob::public_rewards`](crate::Clob::public_rewards).
    pub fn current_markets(&self) -> ListRewardMarkets {
        self.public().current_markets()
    }

    /// Get rewards for a specific market (`GET /rewards/markets/{condition_id}`).
    ///
    /// Also available without an account via
    /// [`Clob::public_rewards`](crate::Clob::public_rewards).
    pub fn market(&self, condition_id: impl Into<String>) -> RewardMarketRequest {
        self.public().market(condition_id)
    }

    /// Search active markets with reward configurations
    /// (`GET /rewards/markets/multi`).
    ///
    /// Also available without an account via
    /// [`Clob::public_rewards`](crate::Clob::public_rewards).
    pub fn multi_markets(&self) -> ListMultiRewardMarkets {
        self.public().multi_markets()
    }

    /// Get current rebated fees for a maker on a given date
    /// (`GET /rebates/current`).
    ///
    /// Also available without an account via
    /// [`Clob::public_rewards`](crate::Clob::public_rewards).
    pub fn current_rebates(
        &self,
        date: impl Into<String>,
        maker_address: impl Into<String>,
    ) -> Request<Vec<RebatedFees>, ClobError> {
        self.public().current_rebates(date, maker_address)
    }
}

/// The unauthenticated subset of the rewards API.
///
/// `GET /rewards/markets/current`, `/rewards/markets/{condition_id}`,
/// `/rewards/markets/multi`, and `/rebates/current` are documented as public
/// upstream, so they are reachable from a client built without an account (for
/// example via `Clob::public()`). The same methods are mirrored on
/// [`Rewards`] for callers that already hold an authenticated namespace.
#[derive(Clone)]
pub struct PublicRewards {
    pub(crate) http_client: HttpClient,
}

impl PublicRewards {
    /// Get currently active reward markets (`GET /rewards/markets/current`).
    ///
    /// Returns a paginated envelope (`data`, `next_cursor`, `limit`, `count`);
    /// the reward markets are in the `data` field.
    pub fn current_markets(&self) -> ListRewardMarkets {
        ListRewardMarkets {
            request: Request::new(self.http_client.clone(), "/rewards/markets/current"),
        }
    }

    /// Get rewards for a specific market (`GET /rewards/markets/{condition_id}`).
    pub fn market(&self, condition_id: impl Into<String>) -> RewardMarketRequest {
        RewardMarketRequest {
            request: Request::new(
                self.http_client.clone(),
                format!(
                    "/rewards/markets/{}",
                    urlencoding::encode(&condition_id.into())
                ),
            ),
        }
    }

    /// Search active markets with reward configurations
    /// (`GET /rewards/markets/multi`).
    ///
    /// Supports text search, tag filtering, numeric filters, and sorting.
    /// Pages hold 100 items by default (max 500); a `next_cursor` of `"LTE="`
    /// marks the last page.
    pub fn multi_markets(&self) -> ListMultiRewardMarkets {
        ListMultiRewardMarkets {
            request: Request::new(self.http_client.clone(), "/rewards/markets/multi"),
        }
    }

    /// Get current rebated fees for a maker on a given date
    /// (`GET /rebates/current`).
    ///
    /// `date` must be in `YYYY-MM-DD` format. Each entry carries the condition
    /// ID, asset address, and the USDC amount rebated.
    pub fn current_rebates(
        &self,
        date: impl Into<String>,
        maker_address: impl Into<String>,
    ) -> Request<Vec<RebatedFees>, ClobError> {
        Request::new(self.http_client.clone(), "/rebates/current")
            .query("date", date.into())
            .query("maker_address", maker_address.into())
    }
}

/// Sort direction for the rewards market listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SortPosition {
    /// Ascending.
    Asc,
    /// Descending.
    Desc,
}

impl SortPosition {
    /// Wire representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

impl std::fmt::Display for SortPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Sort field for [`Rewards::multi_markets`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiMarketOrderBy {
    /// Market ID.
    MarketId,
    /// Creation time.
    CreatedAt,
    /// 24-hour volume.
    Volume24hr,
    /// Current spread.
    Spread,
    /// Competitiveness score.
    Competitiveness,
    /// Maximum rewarded spread.
    MaxSpread,
    /// Minimum rewarded size.
    MinSize,
    /// Question text.
    Question,
    /// One-day price change.
    OneDayPriceChange,
    /// Reward rate per day.
    RatePerDay,
    /// Current price.
    Price,
    /// Market end date.
    EndDate,
    /// Market start date.
    StartDate,
    /// Reward program end date.
    RewardEndDate,
}

impl MultiMarketOrderBy {
    /// Wire representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MarketId => "market_id",
            Self::CreatedAt => "created_at",
            Self::Volume24hr => "volume_24hr",
            Self::Spread => "spread",
            Self::Competitiveness => "competitiveness",
            Self::MaxSpread => "max_spread",
            Self::MinSize => "min_size",
            Self::Question => "question",
            Self::OneDayPriceChange => "one_day_price_change",
            Self::RatePerDay => "rate_per_day",
            Self::Price => "price",
            Self::EndDate => "end_date",
            Self::StartDate => "start_date",
            Self::RewardEndDate => "reward_end_date",
        }
    }
}

impl std::fmt::Display for MultiMarketOrderBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Sort field for [`Rewards::market_earnings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRewardMarketOrderBy {
    /// Maximum rewarded spread.
    MaxSpread,
    /// Minimum rewarded size.
    MinSize,
    /// Market end date.
    EndDate,
    /// Share of the market's reward pool earned.
    EarningPercentage,
    /// Reward rate per day.
    RatePerDay,
    /// Earnings for the day.
    Earnings,
    /// Current spread.
    Spread,
    /// Competitiveness score.
    Competitiveness,
    /// Question text.
    Question,
    /// Current price.
    Price,
    /// Market identifier.
    Market,
    /// 24-hour volume.
    Volume24hr,
}

impl UserRewardMarketOrderBy {
    /// Wire representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MaxSpread => "max_spread",
            Self::MinSize => "min_size",
            Self::EndDate => "end_date",
            Self::EarningPercentage => "earning_percentage",
            Self::RatePerDay => "rate_per_day",
            Self::Earnings => "earnings",
            Self::Spread => "spread",
            Self::Competitiveness => "competitiveness",
            Self::Question => "question",
            Self::Price => "price",
            Self::Market => "market",
            Self::Volume24hr => "volume_24hr",
        }
    }
}

impl std::fmt::Display for UserRewardMarketOrderBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Request builder for `GET /rewards/user`.
pub struct UserEarningsRequest {
    request: Request<RewardEarnings, ClobError>,
}

impl UserEarningsRequest {
    polyoxide_core::query_setters! {
        /// Query earnings for a maker address other than the authenticated wallet.
        maker_address: impl Into<String> => "maker_address",
        /// Restrict results to sponsored reward markets (default: `false`).
        sponsored: bool => "sponsored",
        /// Continue from a pagination cursor.
        next_cursor: impl Into<String> => "next_cursor",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<RewardEarnings, ClobError> {
        self.request.send().await
    }
}

/// Request builder for `GET /rewards/user/total`.
pub struct UserTotalEarningsRequest {
    request: Request<Vec<RewardTotalEarnings>, ClobError>,
}

impl UserTotalEarningsRequest {
    polyoxide_core::query_setters! {
        /// Query totals for a maker address other than the authenticated wallet.
        maker_address: impl Into<String> => "maker_address",
        /// Restrict results to sponsored reward markets (default: `false`).
        sponsored: bool => "sponsored",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<RewardTotalEarnings>, ClobError> {
        self.request.send().await
    }
}

/// Request builder for `GET /rewards/user/percentages`.
pub struct UserPercentagesRequest {
    request: Request<RewardPercentages, ClobError>,
}

impl UserPercentagesRequest {
    polyoxide_core::query_setters! {
        /// Query percentages for a maker address other than the authenticated wallet.
        maker_address: impl Into<String> => "maker_address",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<RewardPercentages, ClobError> {
        self.request.send().await
    }
}

/// Request builder for `GET /rewards/markets/current`.
pub struct ListRewardMarkets {
    request: Request<Paginated<RewardMarket>, ClobError>,
}

impl ListRewardMarkets {
    polyoxide_core::query_setters! {
        /// Restrict results to sponsored reward markets (default: `false`).
        sponsored: bool => "sponsored",
        /// Continue from a pagination cursor; `"LTE="` marks the last page.
        next_cursor: impl Into<String> => "next_cursor",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Paginated<RewardMarket>, ClobError> {
        self.request.send().await
    }
}

/// Request builder for `GET /rewards/markets/{condition_id}`.
pub struct RewardMarketRequest {
    request: Request<RewardMarket, ClobError>,
}

impl RewardMarketRequest {
    polyoxide_core::query_setters! {
        /// Restrict results to sponsored reward markets (default: `false`).
        sponsored: bool => "sponsored",
        /// Continue from a pagination cursor.
        next_cursor: impl Into<String> => "next_cursor",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<RewardMarket, ClobError> {
        self.request.send().await
    }
}

/// Request builder for `GET /rewards/markets/multi`.
pub struct ListMultiRewardMarkets {
    request: Request<Paginated<RewardMarket>, ClobError>,
}

impl ListMultiRewardMarkets {
    polyoxide_core::query_setters! {
        /// Free-text search over market questions.
        query_text: impl Into<String> => "q",
        /// Filter by tag slug.
        tag_slug: impl Into<String> => "tag_slug",
        /// Filter by event ID.
        event_id: impl Into<String> => "event_id",
        /// Filter by event title.
        event_title: impl Into<String> => "event_title",
        /// Sort field.
        order_by(order_by: MultiMarketOrderBy) => "order_by" = order_by.as_str(),
        /// Sort direction.
        position(position: SortPosition) => "position" = position.as_str(),
        /// Minimum 24-hour volume.
        min_volume_24hr: f64 => "min_volume_24hr",
        /// Maximum 24-hour volume.
        max_volume_24hr: f64 => "max_volume_24hr",
        /// Minimum spread.
        min_spread: f64 => "min_spread",
        /// Maximum spread.
        max_spread: f64 => "max_spread",
        /// Minimum price.
        min_price: f64 => "min_price",
        /// Maximum price.
        max_price: f64 => "max_price",
        /// Page size (default 100, max 500).
        page_size: u32 => "page_size",
        /// Continue from a pagination cursor; `"LTE="` marks the last page.
        next_cursor: impl Into<String> => "next_cursor",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Paginated<RewardMarket>, ClobError> {
        self.request.send().await
    }
}

/// Request builder for `GET /rewards/user/markets`.
pub struct ListUserRewardMarkets {
    request: Request<Paginated<RewardMarketEarning>, ClobError>,
}

impl ListUserRewardMarkets {
    polyoxide_core::query_setters! {
        /// Restrict to a specific day (`YYYY-MM-DD`).
        date: impl Into<String> => "date",
        /// Query a maker address other than the authenticated wallet.
        maker_address: impl Into<String> => "maker_address",
        /// Restrict results to sponsored reward markets (default: `false`).
        sponsored: bool => "sponsored",
        /// Free-text search over market questions.
        query_text: impl Into<String> => "q",
        /// Filter by tag slug.
        tag_slug: impl Into<String> => "tag_slug",
        /// Restrict to the caller's favorited markets (default: `false`).
        favorite_markets: bool => "favorite_markets",
        /// Restrict to markets with no competing liquidity (default: `false`).
        no_competition: bool => "no_competition",
        /// Restrict to markets with mergeable positions (default: `false`).
        only_mergeable: bool => "only_mergeable",
        /// Restrict to markets where the caller has open orders (default: `false`).
        only_open_orders: bool => "only_open_orders",
        /// Restrict to markets where the caller has open positions (default: `false`).
        only_open_positions: bool => "only_open_positions",
        /// Sort field.
        order_by(order_by: UserRewardMarketOrderBy) => "order_by" = order_by.as_str(),
        /// Sort direction.
        position(position: SortPosition) => "position" = position.as_str(),
        /// Page size (default 100, max 500).
        page_size: u32 => "page_size",
        /// Continue from a pagination cursor; `"LTE="` marks the last page.
        next_cursor: impl Into<String> => "next_cursor",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Paginated<RewardMarketEarning>, ClobError> {
        self.request.send().await
    }
}

/// Rebated fees for a maker on a specific market and date
/// (`GET /rebates/current`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebatedFees {
    /// Date of the rebate (`YYYY-MM-DD`).
    pub date: String,
    /// Condition ID of the market.
    pub condition_id: String,
    /// Asset address (e.g. the USDC contract).
    pub asset_address: String,
    /// Maker's address.
    pub maker_address: String,
    /// Rebated fee amount in USDC, as a decimal string.
    pub rebated_fees_usdc: String,
}

/// User earnings response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardEarnings {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// User total accumulated earnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardTotalEarnings {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// User reward percentages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardPercentages {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Per-market earnings breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardMarketEarning {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Reward market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardMarket {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Pagination envelope used by the rewards list endpoints
/// (`GET /rewards/markets/current` and `GET /rewards/user/markets`).
///
/// These endpoints wrap results in a pagination object rather than returning a
/// bare array, so the items live in [`Self::data`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    /// Items for this page.
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    /// Cursor for the next page; a value of `"LTE="` indicates the last page.
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// Page size limit reported by the server.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Number of items in this page.
    #[serde(default)]
    pub count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_earnings_deserializes() {
        let json = r#"{"amount": "1.5", "day": "2024-01-15"}"#;
        let resp: RewardEarnings = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["amount"], "1.5");
        assert_eq!(resp.data["day"], "2024-01-15");
    }

    #[test]
    fn reward_total_earnings_deserializes() {
        let json = r#"{"total": "42.0"}"#;
        let resp: RewardTotalEarnings = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["total"], "42.0");
    }

    #[test]
    fn reward_total_earnings_list_deserializes() {
        // GET /rewards/user/total returns an array of per-asset totals, not a
        // single object. Regression test for that shape (observed live).
        let json = r#"[{"asset_address": "0xabc", "total": "42.0"}]"#;
        let resp: Vec<RewardTotalEarnings> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 1);
        assert_eq!(resp[0].data["total"], "42.0");
    }

    #[test]
    fn reward_percentages_deserializes() {
        let json = r#"{"maker": "0.5", "taker": "0.3"}"#;
        let resp: RewardPercentages = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["maker"], "0.5");
    }

    #[test]
    fn reward_market_earning_list_deserializes() {
        let json = r#"[
            {"condition_id": "0xabc", "amount": "10.0"},
            {"condition_id": "0xdef", "amount": "5.0"}
        ]"#;
        let resp: Vec<RewardMarketEarning> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 2);
        assert_eq!(resp[0].data["condition_id"], "0xabc");
    }

    #[test]
    fn reward_market_deserializes() {
        let json = r#"{"condition_id": "0xabc", "reward_rate": "0.01"}"#;
        let resp: RewardMarket = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["condition_id"], "0xabc");
    }

    #[test]
    fn reward_market_list_deserializes() {
        let json = r#"[{"condition_id": "0xabc"}, {"condition_id": "0xdef"}]"#;
        let resp: Vec<RewardMarket> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 2);
    }

    #[test]
    fn current_markets_paginated_response_deserializes() {
        // GET /rewards/markets/current wraps results in a pagination envelope,
        // not a bare array. Regression test for that shape (observed live).
        let json = r#"{
            "limit": 500,
            "count": 1,
            "next_cursor": "LTE=",
            "data": [
                {"condition_id": "0xabc", "rewards_max_spread": 99}
            ]
        }"#;
        let page: Paginated<RewardMarket> =
            serde_json::from_str(json).expect("paginated reward markets should deserialize");
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.count, Some(1));
        assert_eq!(page.next_cursor.as_deref(), Some("LTE="));
        assert_eq!(page.data[0].data["condition_id"], "0xabc");
    }

    #[test]
    fn market_earnings_paginated_response_deserializes() {
        // GET /rewards/user/markets wraps results in the same pagination envelope.
        let json = r#"{
            "limit": 100,
            "count": 1,
            "next_cursor": "LTE=",
            "data": [
                {"condition_id": "0xabc", "earnings": 0.237519}
            ]
        }"#;
        let page: Paginated<RewardMarketEarning> =
            serde_json::from_str(json).expect("paginated market earnings should deserialize");
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.count, Some(1));
        assert_eq!(page.data[0].data["condition_id"], "0xabc");
    }

    #[test]
    fn reward_earnings_empty_object_deserializes() {
        let json = r#"{}"#;
        let resp: RewardEarnings = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_object());
    }
}
