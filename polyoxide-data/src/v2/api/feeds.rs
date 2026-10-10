//! `feeds` routes: trades, activity and combo activity.

use polyoxide_core::Request;

use crate::{
    types::SortDirection,
    v2::{
        envelope::Paged,
        types::{
            Activity, ActivitySortBy, ActivityType, ComboActivity, FilterType, Trade, TradeSide,
        },
        DataV2,
    },
};

impl DataV2 {
    /// `GET /v2/trades`: the trade feed, newest first.
    ///
    /// With no filter this is every trade in the rolling current-plus-previous
    /// month. [`ListTrades::user`] enables the [`start`](ListTrades::start) /
    /// [`end`](ListTrades::end) window; the market and event shapes serve a
    /// fixed three-year window and ignore both bounds.
    ///
    /// The first page is served from a CDN cache for up to five minutes, so it
    /// may lag the feed. Later pages are keyed by cursor and are not affected.
    pub fn trades(&self) -> ListTrades {
        ListTrades {
            inner: Paged::new(Request::new(self.http_client.clone(), "/v2/trades")),
        }
    }

    /// `GET /v2/activity`: one wallet's activity feed.
    pub fn activity(&self, user: impl Into<String>) -> ListActivity {
        ListActivity {
            inner: Paged::new(Request::new(self.http_client.clone(), "/v2/activity"))
                .query("user", user.into()),
        }
    }

    /// `GET /v2/activity/combos`: one wallet's combo lifecycle events.
    pub fn combo_activity(&self, user: impl Into<String>) -> ListComboActivity {
        ListComboActivity {
            inner: Paged::new(Request::new(
                self.http_client.clone(),
                "/v2/activity/combos",
            ))
            .query("user", user.into()),
        }
    }
}

/// Builder for `GET /v2/trades`.
pub struct ListTrades {
    inner: Paged<Trade>,
}

impl ListTrades {
    polyoxide_core::query_setters! { self.inner;
        /// Only trades by this proxy wallet.
        user: impl Into<String> => "user",
        /// Only trades in these markets, by condition id (at most 20). Mutually
        /// exclusive with [`event_ids`](Self::event_ids). An empty list is omitted.
        conditions: csv<I, S> => "condition",
        /// Only trades in these Gamma events (at most 20 distinct ids). An empty
        /// list is omitted.
        event_ids: csv<I, S> => "event_id",
        /// Only fills on this side.
        side: TradeSide => "side",
        /// `true` (the upstream default) serves each fill once, on its taker side;
        /// `false` includes the maker rows too.
        taker_only: bool => "taker_only",
        /// Unit of [`filter_amount`](Self::filter_amount). Upstream default: `TOKENS`.
        filter_type: FilterType => "filter_type",
        /// Minimum trade size, in the unit set by [`filter_type`](Self::filter_type).
        /// Upstream default: `0.01`.
        filter_amount: f64 => "filter_amount",
        /// Window start, epoch seconds, inclusive. Honoured only with
        /// [`user`](Self::user). Omitted or `0` floors to three years back; `1`
        /// asks for full history.
        start: i64 => "start",
        /// Window end, epoch seconds, inclusive. Honoured only with
        /// [`user`](Self::user). Omitted or `0` means now plus one day.
        end: i64 => "end",
        /// First-page size (at most 1000).
        limit: u32 => "limit",
    }

    paged_builder_methods!(Trade);
}

/// Builder for `GET /v2/activity`.
pub struct ListActivity {
    inner: Paged<Activity>,
}

impl ListActivity {
    polyoxide_core::query_setters! { self.inner;
        /// Only these row types. `TIP` is never in the default set; name it here
        /// to receive tips. An empty list is omitted.
        types: csv impl IntoIterator<Item = ActivityType> => "type",
        /// Only activity in these markets, by condition id (at most 20). Mutually
        /// exclusive with [`event_ids`](Self::event_ids). An empty list is omitted.
        conditions: csv<I, S> => "condition",
        /// Only activity in these Gamma events (at most 20 distinct ids). An empty
        /// list is omitted.
        event_ids: csv<I, S> => "event_id",
        /// Only trade rows on this side.
        side: TradeSide => "side",
        /// Window start on the block timestamp, epoch seconds, inclusive. Omitted
        /// or `0` floors to three years back; `1` asks for full history.
        start: i64 => "start",
        /// Window end, epoch seconds, inclusive. Omitted or `0` means now plus one day.
        end: i64 => "end",
        /// Sort key. Only `TIMESTAMP` exists.
        sort_by: ActivitySortBy => "sort_by",
        /// Walk direction. Upstream default: `DESC`. The cursor binds it, so keep it
        /// the same for every page (`.pages()` does).
        sort_direction: SortDirection => "sort_direction",
        /// Upstream defaults this to `true`, which hides `DEPOSIT` and `WITHDRAWAL`
        /// rows even when [`types`](Self::types) asks for them.
        exclude_deposits_withdrawals: bool => "exclude_deposits_withdrawals",
        /// Page size (at most 1000; upstream default 100).
        limit: u32 => "limit",
    }

    paged_builder_methods!(Activity);
}

/// Builder for `GET /v2/activity/combos`.
pub struct ListComboActivity {
    inner: Paged<ComboActivity>,
}

impl ListComboActivity {
    polyoxide_core::query_setters! { self.inner;
        /// Only these combo condition ids (at most 20). An empty list is omitted.
        conditions: csv<I, S> => "condition",
        /// First-page size (at most 1000).
        limit: u32 => "limit",
    }

    paged_builder_methods!(ComboActivity);
}
