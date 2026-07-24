use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::DataApiError,
    types::{ComboSort, ComboStatus, CombosActivityResponse, CombosResponse},
};

/// Combos namespace — combinatorial (multi-market) positions and their
/// lifecycle activity.
///
/// A combo row on `/activity` (where `isCombo` is true) carries a `conditionId`
/// equal to the combo's `combo_condition_id`; pass it to
/// [`market_id`](ListComboPositions::market_id) here to fetch the combo's legs
/// and detail.
#[derive(Clone)]
pub struct CombosApi {
    pub(crate) http_client: HttpClient,
}

impl CombosApi {
    /// List a user's combinatorial positions (`GET /v1/positions/combos`).
    ///
    /// Open positions with a `shares_balance` below 0.001 are omitted (a dust
    /// floor for sub-0.001 remainders left by "sell all" cashouts); resolved
    /// positions are served regardless of balance.
    pub fn positions(&self, user_address: impl Into<String>) -> ListComboPositions {
        ListComboPositions {
            request: Request::new(self.http_client.clone(), "/v1/positions/combos")
                .query("user", user_address.into()),
        }
    }

    /// List a user's combo lifecycle and redeem events
    /// (`GET /v1/activity/combos`).
    ///
    /// Covers split, merge, convert, compress, wrap, unwrap, and redeem — the
    /// combo counterpart to the trade rows on `/activity`.
    pub fn activity(&self, user_address: impl Into<String>) -> ListComboActivity {
        ListComboActivity {
            request: Request::new(self.http_client.clone(), "/v1/activity/combos")
                .query("user", user_address.into()),
        }
    }
}

/// Request builder for listing combo positions.
pub struct ListComboPositions {
    request: Request<CombosResponse, DataApiError>,
}

impl ListComboPositions {
    /// Filter by one or more resolution statuses.
    ///
    /// Omit for the default listing (open positions plus resolved positions
    /// with a recorded resolution). [`ComboStatus::Unknown`] is dropped, since
    /// the upstream API has no matching value to filter on.
    pub fn status(mut self, statuses: impl IntoIterator<Item = ComboStatus>) -> Self {
        let values: Vec<String> = statuses
            .into_iter()
            .filter(|s| *s != ComboStatus::Unknown)
            .map(|s| s.to_string())
            .collect();
        if !values.is_empty() {
            self.request = self.request.query("status", values.join(","));
        }
        self
    }

    /// Set the sort order (default: `current_value_desc`).
    pub fn sort(mut self, sort: ComboSort) -> Self {
        self.request = self.request.query("sort", sort);
        self
    }

    /// Filter by combo condition ID(s) (`0x` + 62 hex).
    pub fn market_id(mut self, ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let values: Vec<String> = ids.into_iter().map(|s| s.to_string()).collect();
        if !values.is_empty() {
            self.request = self.request.query("market_id", values.join(","));
        }
        self
    }

    /// Set results per page (0-1000, default: 20).
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set the pagination offset (0-100000, default: 0).
    ///
    /// Ignored when [`cursor`](Self::cursor) is set.
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Incremental-sync watermark (epoch seconds, inclusive): return only rows
    /// whose `updated_at` is at or after this time.
    ///
    /// Positions mutate on resolution and redemption, so this catches changes a
    /// creation-time filter cannot. Pair with
    /// [`ComboSort::UpdatedAsc`](crate::types::ComboSort::UpdatedAsc).
    pub fn updated_after(mut self, timestamp: i64) -> Self {
        self.request = self.request.query("updatedAfter", timestamp);
        self
    }

    /// Optional upper bound (epoch seconds, inclusive) for `updated_at`.
    ///
    /// Clamped to the safety lag; must be greater than or equal to
    /// [`updated_after`](Self::updated_after).
    pub fn updated_before(mut self, timestamp: i64) -> Self {
        self.request = self.request.query("updatedBefore", timestamp);
        self
    }

    /// Continue from a previous response's `pagination.next_cursor`.
    ///
    /// When present this supersedes [`offset`](Self::offset), which is ignored.
    /// Keep the same [`sort`](Self::sort) across pages. Invalid, tampered, or
    /// cross-endpoint tokens return a 400.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.request = self.request.query("cursor", cursor.into());
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<CombosResponse, DataApiError> {
        self.request.send().await
    }
}

/// Request builder for listing combo activity.
pub struct ListComboActivity {
    request: Request<CombosActivityResponse, DataApiError>,
}

impl ListComboActivity {
    /// Filter by combo condition ID(s) (`0x` + 62 hex).
    pub fn market_id(mut self, ids: impl IntoIterator<Item = impl ToString>) -> Self {
        let values: Vec<String> = ids.into_iter().map(|s| s.to_string()).collect();
        if !values.is_empty() {
            self.request = self.request.query("market_id", values.join(","));
        }
        self
    }

    /// Set results per page (0-500, default: 50).
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set the pagination offset (0-10000, default: 0).
    ///
    /// Ignored when [`cursor`](Self::cursor) is set.
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Continue from a previous response's `pagination.next_cursor`.
    ///
    /// When present this supersedes [`offset`](Self::offset), which is ignored.
    /// Invalid, tampered, or cross-endpoint tokens return a 400.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.request = self.request.query("cursor", cursor.into());
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<CombosActivityResponse, DataApiError> {
        self.request.send().await
    }
}
