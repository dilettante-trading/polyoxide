//! `feeds` routes: trades, activity and combo activity.

use polyoxide_core::Request;

use crate::v2::{
    envelope::{csv, Paged},
    types::{FilterType, Trade, TradeSide},
    DataV2,
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

    /// Only trades in these Gamma events. An empty list is omitted.
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
