//! `feeds` routes: trades, activity and combo activity.

use polyoxide_core::Request;

use crate::{
    types::SortDirection,
    v2::{
        envelope::{csv, Paged},
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
    /// Only trades by this proxy wallet.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.inner = self.inner.query("user", user.into());
        self
    }

    /// Only trades in these markets, by condition id (at most 20). Mutually
    /// exclusive with [`event_ids`](Self::event_ids). An empty list is omitted.
    pub fn conditions<I, S>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        if let Some(value) = csv(conditions) {
            self.inner = self.inner.query("condition", value);
        }
        self
    }

    /// Only trades in these Gamma events (at most 20 distinct ids). An empty
    /// list is omitted.
    pub fn event_ids<I, S>(mut self, event_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        if let Some(value) = csv(event_ids) {
            self.inner = self.inner.query("event_id", value);
        }
        self
    }

    /// Only fills on this side.
    pub fn side(mut self, side: TradeSide) -> Self {
        self.inner = self.inner.query("side", side);
        self
    }

    /// `true` (the upstream default) serves each fill once, on its taker side;
    /// `false` includes the maker rows too.
    pub fn taker_only(mut self, taker_only: bool) -> Self {
        self.inner = self.inner.query("taker_only", taker_only);
        self
    }

    /// Unit of [`filter_amount`](Self::filter_amount). Upstream default: `TOKENS`.
    pub fn filter_type(mut self, filter_type: FilterType) -> Self {
        self.inner = self.inner.query("filter_type", filter_type);
        self
    }

    /// Minimum trade size, in the unit set by [`filter_type`](Self::filter_type).
    /// Upstream default: `0.01`.
    pub fn filter_amount(mut self, amount: f64) -> Self {
        self.inner = self.inner.query("filter_amount", amount);
        self
    }

    /// Window start, epoch seconds, inclusive. Honoured only with
    /// [`user`](Self::user). Omitted or `0` floors to three years back; `1`
    /// asks for full history.
    pub fn start(mut self, start: i64) -> Self {
        self.inner = self.inner.query("start", start);
        self
    }

    /// Window end, epoch seconds, inclusive. Honoured only with
    /// [`user`](Self::user). Omitted or `0` means now plus one day.
    pub fn end(mut self, end: i64) -> Self {
        self.inner = self.inner.query("end", end);
        self
    }

    /// First-page size (at most 1000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(Trade);
}

/// Builder for `GET /v2/activity`.
pub struct ListActivity {
    inner: Paged<Activity>,
}

impl ListActivity {
    /// Only these row types. `TIP` is never in the default set; name it here
    /// to receive tips. An empty list is omitted.
    pub fn types(mut self, types: impl IntoIterator<Item = ActivityType>) -> Self {
        if let Some(value) = csv(types) {
            self.inner = self.inner.query("type", value);
        }
        self
    }

    /// Only activity in these markets, by condition id (at most 20). Mutually
    /// exclusive with [`event_ids`](Self::event_ids). An empty list is omitted.
    pub fn conditions<I, S>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        if let Some(value) = csv(conditions) {
            self.inner = self.inner.query("condition", value);
        }
        self
    }

    /// Only activity in these Gamma events (at most 20 distinct ids). An empty
    /// list is omitted.
    pub fn event_ids<I, S>(mut self, event_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        if let Some(value) = csv(event_ids) {
            self.inner = self.inner.query("event_id", value);
        }
        self
    }

    /// Only trade rows on this side.
    pub fn side(mut self, side: TradeSide) -> Self {
        self.inner = self.inner.query("side", side);
        self
    }

    /// Window start on the block timestamp, epoch seconds, inclusive. Omitted
    /// or `0` floors to three years back; `1` asks for full history.
    pub fn start(mut self, start: i64) -> Self {
        self.inner = self.inner.query("start", start);
        self
    }

    /// Window end, epoch seconds, inclusive. Omitted or `0` means now plus one day.
    pub fn end(mut self, end: i64) -> Self {
        self.inner = self.inner.query("end", end);
        self
    }

    /// Sort key. Only `TIMESTAMP` exists.
    pub fn sort_by(mut self, sort_by: ActivitySortBy) -> Self {
        self.inner = self.inner.query("sort_by", sort_by);
        self
    }

    /// Walk direction. Upstream default: `DESC`. The cursor binds it, so keep it
    /// the same for every page (`.pages()` does).
    pub fn sort_direction(mut self, direction: SortDirection) -> Self {
        self.inner = self.inner.query("sort_direction", direction);
        self
    }

    /// Upstream defaults this to `true`, which hides `DEPOSIT` and `WITHDRAWAL`
    /// rows even when [`types`](Self::types) asks for them.
    pub fn exclude_deposits_withdrawals(mut self, exclude: bool) -> Self {
        self.inner = self.inner.query("exclude_deposits_withdrawals", exclude);
        self
    }

    /// Page size (at most 1000; upstream default 100).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(Activity);
}

/// Builder for `GET /v2/activity/combos`.
pub struct ListComboActivity {
    inner: Paged<ComboActivity>,
}

impl ListComboActivity {
    /// Only these combo condition ids (at most 20). An empty list is omitted.
    pub fn conditions<I, S>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        if let Some(value) = csv(conditions) {
            self.inner = self.inner.query("condition", value);
        }
        self
    }

    /// First-page size (at most 1000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(ComboActivity);
}
