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
    polyoxide_core::query_setters! {
        /// Filter by specific market condition IDs (comma-separated)
        market: csv impl IntoIterator<Item = impl ToString> => "market",
        /// Filter by event IDs (comma-separated)
        event_id: csv impl IntoIterator<Item = impl ToString> => "eventId",
        /// Set minimum position size filter (default: 1)
        size_threshold: f64 => "sizeThreshold",
        /// Filter for redeemable positions only
        redeemable: bool => "redeemable",
        /// Filter for mergeable positions only
        mergeable: bool => "mergeable",
        /// Set maximum number of results (0-500, default: 100)
        limit: u32 => "limit",
        /// Set pagination offset (0-10000, default: 0)
        offset: u32 => "offset",
        /// Set sort field
        sort_by: PositionSortBy => "sortBy",
        /// Set sort direction (default: DESC)
        sort_direction: SortDirection => "sortDirection",
        /// Filter by market title (max 100 chars)
        title: impl Into<String> => "title",
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
    polyoxide_core::query_setters! {
        /// Filter by specific market condition IDs (comma-separated)
        market: csv impl IntoIterator<Item = impl ToString> => "market",
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
    polyoxide_core::query_setters! {
        /// Filter by specific market condition IDs (comma-separated)
        market: csv impl IntoIterator<Item = impl ToString> => "market",
        /// Filter by event IDs (comma-separated)
        event_id: csv impl IntoIterator<Item = impl ToString> => "eventId",
        /// Filter by market title (max 100 chars)
        title: impl Into<String> => "title",
        /// Set maximum number of results (0-50, default: 10).
        ///
        /// Unlike most other `limit()` builders in this crate, this cap is
        /// strictly enforced server-side: passing a value above 50 fails the
        /// request with a 400 ("max closed positions limit of 50 exceeded")
        /// rather than being clamped or paginated by the API.
        limit: u32 => "limit",
        /// Set pagination offset (0-100000, default: 0)
        offset: u32 => "offset",
        /// Set sort field (default: REALIZED_PNL)
        sort_by: ClosedPositionSortBy => "sortBy",
        /// Set sort direction (default: DESC)
        sort_direction: SortDirection => "sortDirection",
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
    polyoxide_core::query_setters! {
        /// Filter by market condition IDs (comma-separated)
        /// Note: Mutually exclusive with `event_id`
        market: csv impl IntoIterator<Item = impl ToString> => "market",
        /// Filter by event IDs (comma-separated)
        /// Note: Mutually exclusive with `market`
        event_id: csv impl IntoIterator<Item = impl ToString> => "eventId",
        /// Filter by trade side (BUY or SELL)
        side: TradeSide => "side",
        /// Filter for taker trades only (default: true)
        taker_only: bool => "takerOnly",
        /// Set filter type (must be paired with `filter_amount`)
        filter_type: TradeFilterType => "filterType",
        /// Set filter amount (must be paired with `filter_type`)
        filter_amount: f64 => "filterAmount",
        /// Set maximum number of results (0-10000, default: 100)
        limit: u32 => "limit",
        /// Set pagination offset (0-10000, default: 0)
        offset: u32 => "offset",
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
    polyoxide_core::query_setters! {
        /// Filter by market condition IDs (comma-separated)
        market: csv impl IntoIterator<Item = impl ToString> => "market",
        /// Filter by event IDs (comma-separated)
        event_id: csv impl IntoIterator<Item = impl ToString> => "eventId",
        /// Filter by activity types (comma-separated). `ActivityType::Unknown` is
        /// silently dropped since the upstream API has no matching value to filter on.
        activity_type(types: impl IntoIterator<Item = ActivityType>) => csv "type"
            = types.into_iter().filter(|t| *t != ActivityType::Unknown),
        /// Include deposit and withdrawal rows (`excludeDepositsWithdrawals`).
        ///
        /// Upstream defaults this to `true` and applies the default **even when
        /// [`activity_type`](Self::activity_type) explicitly requests**
        /// [`ActivityType::Deposit`] or [`ActivityType::Withdrawal`], so those two
        /// filters return an empty list unless this is called with `false`.
        ///
        /// Leaving it unset sends no parameter, preserving upstream's default.
        exclude_deposits_withdrawals: bool => "excludeDepositsWithdrawals",
        /// Filter by trade side (BUY or SELL)
        side: TradeSide => "side",
        /// Lower-bound timestamp (epoch seconds) for the activity window.
        ///
        /// Omit or pass `0` for the default window (most recent ~3 years); pass a
        /// positive epoch (e.g. `1`) to retrieve full history. With
        /// [`sort_direction(SortDirection::Asc)`](Self::sort_direction), omitting
        /// `start` anchors paging to the default window's floor.
        start: i64 => "start",
        /// Upper-bound timestamp (epoch seconds) for the activity window.
        ///
        /// Omit for the default (current time); rows newer than `end` are excluded.
        end: i64 => "end",
        /// Set maximum number of results (0-500, default: 100)
        ///
        /// Values above the maximum are clamped to 500 server-side.
        limit: u32 => "limit",
        /// Set pagination offset (0-5000, default: 0)
        ///
        /// Requests past the cap are rejected with a 400 rather than silently
        /// clamped. To read history deeper than offset 5000, page inside successive
        /// [`start`](Self::start)/[`end`](Self::end) windows — each window has its
        /// own offset budget.
        offset: u32 => "offset",
        /// Set sort field (default: TIMESTAMP)
        sort_by: ActivitySortBy => "sortBy",
        /// Set sort direction (default: DESC)
        sort_direction: SortDirection => "sortDirection",
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
