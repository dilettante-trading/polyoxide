use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::{
    error::DataApiError,
    types::{
        Activity, ActivitySortBy, ActivityType, ClosedPosition, ClosedPositionSortBy, Position,
        PositionSortBy, SortDirection, Trade, TradeFilterType, TradeSide, UserValue,
    },
};

/// User namespace for user-related operations
#[derive(Clone)]
pub struct UserApi {
    pub(crate) http_client: HttpClient,
    pub(crate) user_address: String,
}

impl UserApi {
    /// List positions for this user
    pub fn list_positions(&self) -> ListPositions {
        let mut request = Request::new(self.http_client.clone(), "/positions");
        request = request.query("user", &self.user_address);

        ListPositions { request }
    }

    /// Get total value of this user's positions
    pub fn positions_value(&self) -> GetPositionValue {
        let mut request = Request::new(self.http_client.clone(), "/value");
        request = request.query("user", &self.user_address);

        GetPositionValue { request }
    }

    /// List closed positions for this user
    pub fn closed_positions(&self) -> ListClosedPositions {
        let mut request = Request::new(self.http_client.clone(), "/closed-positions");
        request = request.query("user", &self.user_address);

        ListClosedPositions { request }
    }

    /// List trades for this user
    pub fn trades(&self) -> ListUserTrades {
        let mut request = Request::new(self.http_client.clone(), "/trades");
        request = request.query("user", &self.user_address);

        ListUserTrades { request }
    }

    /// List activity for this user
    pub fn activity(&self) -> ListActivity {
        let mut request = Request::new(self.http_client.clone(), "/activity");
        request = request.query("user", &self.user_address);

        ListActivity { request }
    }

    /// Get total markets traded by this user
    pub async fn traded(&self) -> Result<UserTraded, DataApiError> {
        Request::<UserTraded, DataApiError>::new(self.http_client.clone(), "/traded")
            .query("user", &self.user_address)
            .send()
            .await
    }
}

/// User's total markets traded count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTraded {
    /// User address
    pub user: String,
    /// Total count of distinct markets traded
    pub traded: u64,
}

/// Request builder for listing user positions
pub struct ListPositions {
    request: Request<Vec<Position>, DataApiError>,
}

impl ListPositions {
    /// Filter by specific market condition IDs (comma-separated)
    pub fn market(mut self, condition_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = condition_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("market", ids.join(","));
        }
        self
    }

    /// Filter by event IDs (comma-separated)
    pub fn event_id(mut self, event_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = event_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("eventId", ids.join(","));
        }
        self
    }

    /// Set minimum position size filter (default: 1)
    pub fn size_threshold(mut self, threshold: f64) -> Self {
        self.request = self.request.query("sizeThreshold", threshold);
        self
    }

    /// Filter for redeemable positions only
    pub fn redeemable(mut self, redeemable: bool) -> Self {
        self.request = self.request.query("redeemable", redeemable);
        self
    }

    /// Filter for mergeable positions only
    pub fn mergeable(mut self, mergeable: bool) -> Self {
        self.request = self.request.query("mergeable", mergeable);
        self
    }

    /// Set maximum number of results (0-500, default: 100)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (0-10000, default: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Set sort field
    pub fn sort_by(mut self, sort_by: PositionSortBy) -> Self {
        self.request = self.request.query("sortBy", sort_by);
        self
    }

    /// Set sort direction (default: DESC)
    pub fn sort_direction(mut self, direction: SortDirection) -> Self {
        self.request = self.request.query("sortDirection", direction);
        self
    }

    /// Filter by market title (max 100 chars)
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.request = self.request.query("title", title.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Position>, DataApiError> {
        self.request.send().await
    }
}

/// Request builder for getting total position value
pub struct GetPositionValue {
    request: Request<Vec<UserValue>, DataApiError>,
}

impl GetPositionValue {
    /// Filter by specific market condition IDs (comma-separated)
    pub fn market(mut self, condition_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = condition_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("market", ids.join(","));
        }
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<UserValue>, DataApiError> {
        self.request.send().await
    }
}

/// Request builder for listing closed positions
pub struct ListClosedPositions {
    request: Request<Vec<ClosedPosition>, DataApiError>,
}

impl ListClosedPositions {
    /// Filter by specific market condition IDs (comma-separated)
    pub fn market(mut self, condition_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = condition_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("market", ids.join(","));
        }
        self
    }

    /// Filter by event IDs (comma-separated)
    pub fn event_id(mut self, event_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = event_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("eventId", ids.join(","));
        }
        self
    }

    /// Filter by market title (max 100 chars)
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.request = self.request.query("title", title.into());
        self
    }

    /// Set maximum number of results (0-50, default: 10).
    ///
    /// Unlike most other `limit()` builders in this crate, this cap is
    /// strictly enforced server-side: passing a value above 50 fails the
    /// request with a 400 ("max closed positions limit of 50 exceeded")
    /// rather than being clamped or paginated by the API.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (0-100000, default: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Set sort field (default: REALIZED_PNL)
    pub fn sort_by(mut self, sort_by: ClosedPositionSortBy) -> Self {
        self.request = self.request.query("sortBy", sort_by);
        self
    }

    /// Set sort direction (default: DESC)
    pub fn sort_direction(mut self, direction: SortDirection) -> Self {
        self.request = self.request.query("sortDirection", direction);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<ClosedPosition>, DataApiError> {
        self.request.send().await
    }
}

/// Request builder for listing user trades
pub struct ListUserTrades {
    request: Request<Vec<Trade>, DataApiError>,
}

impl ListUserTrades {
    /// Filter by market condition IDs (comma-separated)
    /// Note: Mutually exclusive with `event_id`
    pub fn market(mut self, condition_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = condition_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("market", ids.join(","));
        }
        self
    }

    /// Filter by event IDs (comma-separated)
    /// Note: Mutually exclusive with `market`
    pub fn event_id(mut self, event_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = event_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("eventId", ids.join(","));
        }
        self
    }

    /// Filter by trade side (BUY or SELL)
    pub fn side(mut self, side: TradeSide) -> Self {
        self.request = self.request.query("side", side);
        self
    }

    /// Filter for taker trades only (default: true)
    pub fn taker_only(mut self, taker_only: bool) -> Self {
        self.request = self.request.query("takerOnly", taker_only);
        self
    }

    /// Set filter type (must be paired with `filter_amount`)
    pub fn filter_type(mut self, filter_type: TradeFilterType) -> Self {
        self.request = self.request.query("filterType", filter_type);
        self
    }

    /// Set filter amount (must be paired with `filter_type`)
    pub fn filter_amount(mut self, amount: f64) -> Self {
        self.request = self.request.query("filterAmount", amount);
        self
    }

    /// Set maximum number of results (0-10000, default: 100)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (0-10000, default: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Trade>, DataApiError> {
        self.request.send().await
    }
}

/// Request builder for listing user activity
pub struct ListActivity {
    request: Request<Vec<Activity>, DataApiError>,
}

impl ListActivity {
    /// Filter by market condition IDs (comma-separated)
    pub fn market(mut self, condition_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = condition_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("market", ids.join(","));
        }
        self
    }

    /// Filter by event IDs (comma-separated)
    pub fn event_id(mut self, event_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let ids: Vec<String> = event_ids.into_iter().map(|s| s.to_string()).collect();
        if !ids.is_empty() {
            self.request = self.request.query("eventId", ids.join(","));
        }
        self
    }

    /// Filter by activity types (comma-separated). `ActivityType::Unknown` is
    /// silently dropped since the upstream API has no matching value to filter on.
    pub fn activity_type(mut self, types: impl IntoIterator<Item = ActivityType>) -> Self {
        let type_strs: Vec<String> = types
            .into_iter()
            .filter(|t| *t != ActivityType::Unknown)
            .map(|t| t.to_string())
            .collect();
        if !type_strs.is_empty() {
            self.request = self.request.query("type", type_strs.join(","));
        }
        self
    }

    /// Filter by trade side (BUY or SELL)
    pub fn side(mut self, side: TradeSide) -> Self {
        self.request = self.request.query("side", side);
        self
    }

    /// Set start timestamp filter
    pub fn start(mut self, timestamp: i64) -> Self {
        self.request = self.request.query("start", timestamp);
        self
    }

    /// Set end timestamp filter
    pub fn end(mut self, timestamp: i64) -> Self {
        self.request = self.request.query("end", timestamp);
        self
    }

    /// Set maximum number of results (0-10000, default: 100)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (0-10000, default: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Set sort field (default: TIMESTAMP)
    pub fn sort_by(mut self, sort_by: ActivitySortBy) -> Self {
        self.request = self.request.query("sortBy", sort_by);
        self
    }

    /// Set sort direction (default: DESC)
    pub fn sort_direction(mut self, direction: SortDirection) -> Self {
        self.request = self.request.query("sortDirection", direction);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Activity>, DataApiError> {
        self.request.send().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_user_traded() {
        let json = r#"{"user": "0xabcdef1234567890", "traded": 42}"#;
        let ut: UserTraded = serde_json::from_str(json).unwrap();
        assert_eq!(ut.user, "0xabcdef1234567890");
        assert_eq!(ut.traded, 42);
    }

    #[test]
    fn deserialize_user_traded_zero() {
        let json = r#"{"user": "0x0000000000000000000000000000000000000001", "traded": 0}"#;
        let ut: UserTraded = serde_json::from_str(json).unwrap();
        assert_eq!(ut.traded, 0);
    }

    #[test]
    fn user_traded_roundtrip() {
        let original = UserTraded {
            user: "0x1234".to_string(),
            traded: 100,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: UserTraded = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user, original.user);
        assert_eq!(deserialized.traded, original.traded);
    }
}
