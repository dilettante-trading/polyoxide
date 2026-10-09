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
    polyoxide_core::query_setters! {
        /// Filter by one or more resolution statuses.
        ///
        /// Omit for the default listing (open positions plus resolved positions
        /// with a recorded resolution). [`ComboStatus::Unknown`] is dropped, since
        /// the upstream API has no matching value to filter on.
        status(statuses: impl IntoIterator<Item = ComboStatus>) => csv "status"
            = statuses.into_iter().filter(|s| *s != ComboStatus::Unknown),
        /// Set the sort order (default: `current_value_desc`).
        sort: ComboSort => "sort",
        /// Filter by combo condition ID(s) (`0x` + 62 hex).
        market_id: csv impl IntoIterator<Item = impl ToString> => "market_id",
        /// Set results per page (0-1000, default: 20).
        limit: u32 => "limit",
        /// Set the pagination offset (0-100000, default: 0).
        ///
        /// Ignored when [`cursor`](Self::cursor) is set.
        offset: u32 => "offset",
        /// Incremental-sync watermark (epoch seconds, inclusive): return only rows
        /// whose `updated_at` is at or after this time.
        ///
        /// Positions mutate on resolution and redemption, so this catches changes a
        /// creation-time filter cannot. Pair with [`ComboSort::UpdatedAsc`].
        updated_after: i64 => "updatedAfter",
        /// Optional upper bound (epoch seconds, inclusive) for `updated_at`.
        ///
        /// Clamped to the safety lag; must be greater than or equal to
        /// [`updated_after`](Self::updated_after).
        updated_before: i64 => "updatedBefore",
        /// Continue from a previous response's `pagination.next_cursor`.
        ///
        /// When present this supersedes [`offset`](Self::offset), which is ignored.
        /// Keep the same [`sort`](Self::sort) across pages. Invalid, tampered, or
        /// cross-endpoint tokens return a 400.
        cursor: impl Into<String> => "cursor",
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
    polyoxide_core::query_setters! {
        /// Filter by combo condition ID(s) (`0x` + 62 hex).
        market_id: csv impl IntoIterator<Item = impl ToString> => "market_id",
        /// Set results per page (0-500, default: 50).
        limit: u32 => "limit",
        /// Set the pagination offset (0-10000, default: 0).
        ///
        /// Ignored when [`cursor`](Self::cursor) is set.
        offset: u32 => "offset",
        /// Continue from a previous response's `pagination.next_cursor`.
        ///
        /// When present this supersedes [`offset`](Self::offset), which is ignored.
        /// Invalid, tampered, or cross-endpoint tokens return a 400.
        cursor: impl Into<String> => "cursor",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<CombosActivityResponse, DataApiError> {
        self.request.send().await
    }
}
