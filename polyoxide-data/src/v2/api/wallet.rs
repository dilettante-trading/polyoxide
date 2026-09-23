//! `wallet` routes: approvals, positions, combo positions, PnL, stats, volume
//! and portfolio value.

use polyoxide_core::{QueryBuilder, Request};

use crate::{
    types::SortDirection,
    v2::{
        envelope::{csv, Envelope, Paged},
        types::{
            Approvals, ComboPosition, ComboPositionSortBy, ComboPositionStatus, FilterType,
            PnlFidelity, PnlInterval, PortfolioValue, Position, PositionAnchor, PositionSortBy,
            PositionStatus, UserPnlSeries, UserStats, UserVolume,
        },
        DataV2,
    },
    DataApiError,
};

impl DataV2 {
    /// `GET /v2/approvals`: a wallet's Polygon token and operator approvals.
    pub fn approvals(&self, user: impl Into<String>) -> GetApprovals {
        GetApprovals {
            request: Request::new(self.http_client.clone(), "/v2/approvals")
                .query("user", user.into()),
        }
    }

    /// `GET /v2/positions`: positions across their whole lifecycle.
    ///
    /// Pass a wallet (`"0x…"` converts into [`PositionAnchor::User`]), a market,
    /// or both. [`ListPositions::status`] selects open, redeemable or closed.
    pub fn positions(&self, anchor: impl Into<PositionAnchor>) -> ListPositions {
        let request = Paged::new(Request::new(self.http_client.clone(), "/v2/positions"));
        let inner = match anchor.into() {
            PositionAnchor::User(user) => request.query("user", user),
            PositionAnchor::Condition(condition) => request.query("condition", condition),
            PositionAnchor::UserInConditions { user, conditions } => {
                let request = request.query("user", user);
                match csv(conditions) {
                    Some(value) => request.query("condition", value),
                    None => request,
                }
            }
        };
        ListPositions { inner }
    }

    /// `GET /v2/positions/combos`: a wallet's combo positions.
    pub fn combo_positions(&self, user: impl Into<String>) -> ListComboPositions {
        ListComboPositions {
            inner: Paged::new(Request::new(
                self.http_client.clone(),
                "/v2/positions/combos",
            ))
            .query("user", user.into()),
        }
    }

    /// `GET /v2/user-pnl`: a wallet's cumulative PnL series.
    ///
    /// Upstream describes `trade_pnl` as the series behind
    /// [`DataApi::pnl`](crate::DataApi::pnl), but the two were measured apart;
    /// see that method before swapping one for the other.
    pub fn user_pnl(&self, user: impl Into<String>) -> GetUserPnl {
        GetUserPnl {
            request: Request::new(self.http_client.clone(), "/v2/user-pnl")
                .query("user", user.into()),
        }
    }

    /// `GET /v2/user-stats`: one wallet's profile card.
    ///
    /// Resolves to `None` when the wallet is not a known user. A known user who
    /// has never traded is `Some` with zeros, which is a different answer.
    pub fn user_stats(&self, user: impl Into<String>) -> GetUserStats {
        GetUserStats {
            request: Request::new(self.http_client.clone(), "/v2/user-stats")
                .query("user", user.into()),
        }
    }

    /// `GET /v2/user-volume`: a wallet's traded volume over a window, in shares
    /// and USDC.
    pub fn user_volume(&self, user: impl Into<String>) -> GetUserVolume {
        GetUserVolume {
            request: Request::new(self.http_client.clone(), "/v2/user-volume")
                .query("user", user.into()),
        }
    }

    /// `GET /v2/value`: a wallet's portfolio value in USDC.
    pub fn value(&self, user: impl Into<String>) -> GetValue {
        GetValue {
            request: Request::new(self.http_client.clone(), "/v2/value").query("user", user.into()),
        }
    }
}

/// Builder for `GET /v2/approvals`.
pub struct GetApprovals {
    request: Request<Envelope<Approvals>, DataApiError>,
}

impl GetApprovals {
    /// Fetch the approvals.
    pub async fn send(self) -> Result<Approvals, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/positions`.
pub struct ListPositions {
    inner: Paged<Position>,
}

impl ListPositions {
    /// Lifecycle filter. Upstream default: `OPEN`, which also includes
    /// redeemable rows.
    pub fn status(mut self, status: PositionStatus) -> Self {
        self.inner = self.inner.query("status", status);
        self
    }

    /// Only positions in these Gamma events (at most 20 distinct ids).
    /// Wallet-anchored requests only. An empty list is omitted.
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

    /// Case-insensitive market-title substring (at most 200 characters). SQL
    /// `LIKE` wildcards keep their meaning. The cursor does not carry this
    /// filter, so it must be re-sent on every page; `.pages()` does.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner = self.inner.query("title", title.into());
        self
    }

    /// Unit of [`filter_amount`](Self::filter_amount). Upstream default: `TOKENS`.
    pub fn filter_type(mut self, filter_type: FilterType) -> Self {
        self.inner = self.inner.query("filter_type", filter_type);
        self
    }

    /// Floor on the current holding: shares for `TOKENS` (upstream default
    /// 0.1), mark-to-market USDC for `CASH`.
    pub fn filter_amount(mut self, amount: f64) -> Self {
        self.inner = self.inner.query("filter_amount", amount);
        self
    }

    /// Include positions on archived markets. Not valid with `CLOSED`, which
    /// upstream rejects; every other status accepts it.
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner = self.inner.query("include_archived", include);
        self
    }

    /// Sort key. Upstream default depends on [`status`](Self::status).
    pub fn sort_by(mut self, sort_by: PositionSortBy) -> Self {
        self.inner = self.inner.query("sort_by", sort_by);
        self
    }

    /// Sort direction. Upstream default: `DESC`.
    pub fn sort_direction(mut self, direction: SortDirection) -> Self {
        self.inner = self.inner.query("sort_direction", direction);
        self
    }

    /// Inclusive lower bound on `last_event_at`, epoch seconds. Any bound
    /// excludes positions that have no `last_event_at`.
    pub fn start(mut self, start: i64) -> Self {
        self.inner = self.inner.query("start", start);
        self
    }

    /// Inclusive upper bound on `last_event_at`, epoch seconds.
    pub fn end(mut self, end: i64) -> Self {
        self.inner = self.inner.query("end", end);
        self
    }

    /// First-page size (at most 1000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(Position);
}

/// Builder for `GET /v2/positions/combos`.
pub struct ListComboPositions {
    inner: Paged<ComboPosition>,
}

impl ListComboPositions {
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

    /// Status filter. Several values may be combined, except `REDEEMABLE`,
    /// which upstream requires alone. An empty list is omitted.
    pub fn statuses(mut self, statuses: impl IntoIterator<Item = ComboPositionStatus>) -> Self {
        if let Some(value) = csv(statuses) {
            self.inner = self.inner.query("status", value);
        }
        self
    }

    /// Sort key. Upstream default: `FIRST_ENTRY` (`ENTRY_COST` under `REDEEMABLE`).
    pub fn sort_by(mut self, sort_by: ComboPositionSortBy) -> Self {
        self.inner = self.inner.query("sort_by", sort_by);
        self
    }

    /// Sort direction. Upstream default: `DESC`.
    pub fn sort_direction(mut self, direction: SortDirection) -> Self {
        self.inner = self.inner.query("sort_direction", direction);
        self
    }

    /// Incremental-sync watermark: inclusive lower bound on `updated_at`, epoch seconds.
    pub fn updated_after(mut self, after: i64) -> Self {
        self.inner = self.inner.query("updated_after", after);
        self
    }

    /// Incremental-sync watermark: inclusive upper bound on `updated_at`, epoch seconds.
    pub fn updated_before(mut self, before: i64) -> Self {
        self.inner = self.inner.query("updated_before", before);
        self
    }

    /// First-page size (at most 1000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(ComboPosition);
}

/// Builder for `GET /v2/user-pnl`.
pub struct GetUserPnl {
    request: Request<Envelope<UserPnlSeries>, DataApiError>,
}

impl GetUserPnl {
    /// Window. Upstream default: `1d`.
    pub fn interval(mut self, interval: PnlInterval) -> Self {
        self.request = self.request.query("interval", interval);
        self
    }

    /// Output grid. Upstream default: `1h`.
    pub fn fidelity(mut self, fidelity: PnlFidelity) -> Self {
        self.request = self.request.query("fidelity", fidelity);
        self
    }

    /// Fetch the series.
    pub async fn send(self) -> Result<UserPnlSeries, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/user-stats`.
pub struct GetUserStats {
    request: Request<Envelope<Option<UserStats>>, DataApiError>,
}

impl GetUserStats {
    /// Fetch the stats; `None` for an unknown wallet.
    pub async fn send(self) -> Result<Option<UserStats>, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/user-volume`.
pub struct GetUserVolume {
    request: Request<Envelope<UserVolume>, DataApiError>,
}

impl GetUserVolume {
    /// Window start, epoch seconds, floored to its UTC day. Omitted or `0` is unbounded.
    pub fn start(mut self, start: i64) -> Self {
        self.request = self.request.query("start", start);
        self
    }

    /// Window end, epoch seconds, floored to its UTC day. Omitted or `0` is unbounded.
    pub fn end(mut self, end: i64) -> Self {
        self.request = self.request.query("end", end);
        self
    }

    /// Fetch the volume.
    pub async fn send(self) -> Result<UserVolume, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/value`.
pub struct GetValue {
    request: Request<Envelope<PortfolioValue>, DataApiError>,
}

impl GetValue {
    /// Value only these markets (at most 20). Any condition filter also drops
    /// the portfolio-level combo term. An empty list is omitted.
    pub fn conditions<I, S>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        if let Some(value) = csv(conditions) {
            self.request = self.request.query("condition", value);
        }
        self
    }

    /// Fetch the value.
    pub async fn send(self) -> Result<PortfolioValue, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}
