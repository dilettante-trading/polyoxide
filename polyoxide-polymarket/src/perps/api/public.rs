//! Public-by-address lookups and the invite check:
//! `/v1/info/{portfolio,position-fills,leaderboard,invite,registered,position-snapshots}`.

use std::collections::BTreeSet;

use polyoxide_core::{reqwest::Method, ApiError, HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::perps::{
    api::Fetch,
    error::PerpsError,
    types::{
        InstrumentId, LeaderboardSort, LeaderboardWindow, PositionSnapshotStatus, Side,
        SnapshotMarkerKind, SortOrder,
    },
};

/// Most active instruments one `POST /v1/info/position-snapshots` takes.
pub const MAX_SNAPSHOT_ACTIVE: usize = 20;

/// Most historical fills one `POST /v1/info/position-snapshots` takes.
pub const MAX_SNAPSHOT_HISTORY: usize = 4;

/// Public namespace.
#[derive(Clone)]
pub struct PublicApi {
    pub(crate) http_client: HttpClient,
}

impl PublicApi {
    /// `GET /v1/info/portfolio`: an account's open positions and equity.
    pub fn portfolio(&self, address: impl Into<String>) -> Fetch<PublicPortfolio> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/portfolio")
                .query("address", address.into()),
        }
    }

    /// `GET /v1/info/position-fills`: the fills behind an account's current
    /// position in one instrument, cursor-paged.
    pub fn position_fills(
        &self,
        address: impl Into<String>,
        instrument_id: InstrumentId,
    ) -> ListPositionFills {
        ListPositionFills {
            request: Request::new(self.http_client.clone(), "/v1/info/position-fills")
                .query("address", address.into())
                .query("instrument_id", instrument_id),
        }
    }

    /// `GET /v1/info/leaderboard`.
    pub fn leaderboard(&self) -> GetLeaderboard {
        GetLeaderboard {
            request: Request::new(self.http_client.clone(), "/v1/info/leaderboard"),
        }
    }

    /// `GET /v1/info/invite`: whether an invite code is valid.
    pub fn invite(&self, code: impl Into<String>) -> CheckInvite {
        CheckInvite {
            request: Request::new(self.http_client.clone(), "/v1/info/invite")
                .query("code", code.into()),
        }
    }

    /// `GET /v1/info/registered`: whether an address has a Perps account.
    pub fn registered(&self, address: impl Into<String>) -> Fetch<Registered> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/registered")
                .query("address", address.into()),
        }
    }

    /// `POST /v1/info/position-snapshots`: share-card numbers, candles and
    /// trade markers for a registered account's open positions
    /// ([`GetPositionSnapshots::active`]) and the positions behind past fills
    /// ([`GetPositionSnapshots::history`]). Name at least one of either.
    ///
    /// The request is unauthenticated, so `leverage` and `pnl_percent` come
    /// back `None`: the host reveals them only to the account's owner.
    pub fn position_snapshots(&self, address: impl Into<String>) -> GetPositionSnapshots {
        GetPositionSnapshots {
            http_client: self.http_client.clone(),
            body: PositionSnapshotsRequest {
                address: address.into(),
                active_instrument_ids: Vec::new(),
                history_fills: Vec::new(),
            },
        }
    }
}

/// Request builder for `GET /v1/info/position-fills`.
pub struct ListPositionFills {
    request: Request<PositionFills, PerpsError>,
}

impl ListPositionFills {
    polyoxide_core::query_setters! {
        /// Resume from a previous page's `cursor`.
        cursor: impl Into<String> => "cursor",
        /// Sort order. The server default is descending.
        sort: SortOrder => "sort",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<PositionFills, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/leaderboard`.
pub struct GetLeaderboard {
    request: Request<Leaderboard, PerpsError>,
}

impl GetLeaderboard {
    polyoxide_core::query_setters! {
        /// Window. The server default is `day`.
        window: LeaderboardWindow => "window",
        /// Ranking key. The server default is `pnl`.
        sort_by: LeaderboardSort => "sort_by",
        /// Page size.
        limit: u32 => "limit",
        /// Page offset.
        offset: u64 => "offset",
        /// Also return this account's own standing as `account`.
        address: impl Into<String> => "address",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Leaderboard, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/invite`.
pub struct CheckInvite {
    request: Request<InviteCheck, PerpsError>,
}

impl CheckInvite {
    polyoxide_core::query_setters! {
        /// The address that would redeem the code.
        address: impl Into<String> => "address",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<InviteCheck, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `POST /v1/info/position-snapshots`.
pub struct GetPositionSnapshots {
    http_client: HttpClient,
    body: PositionSnapshotsRequest,
}

impl GetPositionSnapshots {
    /// Open positions to snapshot, by instrument, in the order the answer's
    /// `active` keeps. At most [`MAX_SNAPSHOT_ACTIVE`].
    pub fn active(mut self, instrument_ids: impl IntoIterator<Item = InstrumentId>) -> Self {
        self.body.active_instrument_ids = instrument_ids.into_iter().collect();
        self
    }

    /// Past fills whose positions to snapshot, in the order the answer's
    /// `history` keeps: each a fill from [`PublicApi::position_fills`] or the
    /// public trades, keyed by its instrument, trade id and time. At most
    /// [`MAX_SNAPSHOT_HISTORY`].
    pub fn history(mut self, fills: impl IntoIterator<Item = PositionSnapshotFill>) -> Self {
        self.body.history_fills = fills.into_iter().collect();
        self
    }

    /// Execute the request.
    ///
    /// # Errors
    ///
    /// `ApiError::Validation`, with nothing sent, when the request names no
    /// instrument and no fill, more than [`MAX_SNAPSHOT_ACTIVE`] instruments or
    /// [`MAX_SNAPSHOT_HISTORY`] fills, or one of either twice: the host refuses
    /// each with a `400`.
    pub async fn send(self) -> Result<PositionSnapshots, PerpsError> {
        self.body.validate()?;
        Request::new(self.http_client, "/v1/info/position-snapshots")
            .method(Method::POST)
            .body(&self.body)?
            .send()
            .await
    }
}

/// The body of `POST /v1/info/position-snapshots`. An empty list is left
/// out, since the host refuses an explicit `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PositionSnapshotsRequest {
    address: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    active_instrument_ids: Vec<InstrumentId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    history_fills: Vec<PositionSnapshotFill>,
}

impl PositionSnapshotsRequest {
    fn validate(&self) -> Result<(), ApiError> {
        let refuse = |why: &str| Err(ApiError::Validation(format!("position snapshots: {why}")));
        if self.active_instrument_ids.is_empty() && self.history_fills.is_empty() {
            return refuse("name at least one active instrument or historical fill");
        }
        if self.active_instrument_ids.len() > MAX_SNAPSHOT_ACTIVE {
            return refuse(&format!(
                "{} active instruments, more than {MAX_SNAPSHOT_ACTIVE}",
                self.active_instrument_ids.len()
            ));
        }
        if self.history_fills.len() > MAX_SNAPSHOT_HISTORY {
            return refuse(&format!(
                "{} historical fills, more than {MAX_SNAPSHOT_HISTORY}",
                self.history_fills.len()
            ));
        }
        let ids: BTreeSet<_> = self.active_instrument_ids.iter().collect();
        if ids.len() < self.active_instrument_ids.len() {
            return refuse("an active instrument is named twice");
        }
        let fills: BTreeSet<_> = self
            .history_fills
            .iter()
            .map(|f| (f.instrument_id, f.trade_id))
            .collect();
        if fills.len() < self.history_fills.len() {
            return refuse("a historical fill is named twice");
        }
        Ok(())
    }
}

/// One past fill to snapshot the position behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PositionSnapshotFill {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Trade id. Sent as a decimal string, as the host requires.
    #[serde(serialize_with = "u64_as_str")]
    pub trade_id: u64,
    /// The fill's public timestamp, Unix ms.
    pub timestamp: u64,
}

impl PositionSnapshotFill {
    /// A fill by its instrument, trade id and public timestamp.
    pub fn new(instrument_id: InstrumentId, trade_id: u64, timestamp: u64) -> Self {
        Self {
            instrument_id,
            trade_id,
            timestamp,
        }
    }
}

impl From<&PositionFill> for PositionSnapshotFill {
    fn from(fill: &PositionFill) -> Self {
        Self::new(fill.instrument_id, fill.trade_id, fill.timestamp)
    }
}

fn u64_as_str<S: serde::Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// `GET /v1/info/registered`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Registered {
    /// Whether the address has a Perps account.
    pub registered: bool,
}

/// `POST /v1/info/position-snapshots`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionSnapshots {
    /// One result per requested active instrument, in request order.
    pub active: Vec<PositionSnapshotResult>,
    /// One result per requested fill, in request order.
    pub history: Vec<PositionSnapshotResult>,
    /// The serving gateway's history horizon, Unix ms: a fill after it is
    /// `not_found`. Gateways can differ.
    pub history_as_of_at: u64,
}

/// The result for one selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionSnapshotResult {
    /// Whether a snapshot was built, and if not, why.
    pub status: PositionSnapshotStatus,
    /// The selection's instrument.
    pub instrument_id: Option<InstrumentId>,
    /// The selection's trade id, for a historical fill, without leading
    /// zeroes.
    pub trade_id: Option<String>,
    /// The snapshot, present only when `status` is `ok`.
    pub snapshot: Option<PositionCardSnapshot>,
}

/// One position's share card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionCardSnapshot {
    /// Opaque, stable id of the position's cycle; a flip starts a new one.
    pub position_cycle_id: String,
    /// Side.
    pub side: Side,
    /// When the cycle opened, Unix ms.
    pub started_at: u64,
    /// The captured state's time for an open position, the selected fill's
    /// for history, Unix ms.
    pub as_of_at: u64,
    /// Whether the cycle is closed.
    pub is_closed: bool,
    /// Remaining quantity, unsigned; `side` gives its direction.
    #[serde(with = "rust_decimal::serde::str")]
    pub size_after: Decimal,
    /// Weighted entry price of what remains; the last entry when flat.
    #[serde(with = "rust_decimal::serde::str")]
    pub entry_price: Decimal,
    /// The captured mark for an open position, the selected fill's price for
    /// history.
    #[serde(with = "rust_decimal::serde::str")]
    pub as_of_price: Decimal,
    /// Gross PnL: unrealised on what remains while open, realised over the
    /// cycle once closed. Fees, funding and earlier cycles are left out.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// PnL times leverage over entry notional, as a percentage. Only the
    /// authenticated owner sees it; `None` otherwise.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub pnl_percent: Option<Decimal>,
    /// Leverage. Only the authenticated owner sees it, and not when it was
    /// never captured; `None` otherwise.
    pub leverage: Option<u32>,
    /// The card's chart.
    pub chart: PositionSnapshotChart,
}

/// A share card's chart: at most 22 candles and 256 markers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionSnapshotChart {
    /// Candles, left to right.
    pub candles: Vec<PositionSnapshotCandle>,
    /// Trade markers.
    pub markers: Vec<PositionSnapshotMarker>,
}

/// One chart candle. Prices are plain JSON numbers here, for drawing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionSnapshotCandle {
    /// Horizontal position, 0 to 1.
    pub position: f64,
    /// Open price.
    pub open: f64,
    /// High price.
    pub high: f64,
    /// Low price.
    pub low: f64,
    /// Close price.
    pub close: f64,
}

/// One chart marker: a fill that grew or shrank the position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionSnapshotMarker {
    /// Horizontal position, 0 to 1.
    pub position: f64,
    /// Whether the fill increased or decreased the position.
    pub kind: SnapshotMarkerKind,
}

/// `GET /v1/info/portfolio`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PublicPortfolio {
    /// Open positions.
    pub positions: Vec<PublicPortfolioPosition>,
    /// Account equity.
    #[serde(with = "rust_decimal::serde::str")]
    pub equity: Decimal,
    /// Snapshot time, Unix ms.
    pub timestamp: u64,
}

/// One open position on a public portfolio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PublicPortfolioPosition {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Symbol.
    pub symbol: String,
    /// Signed size in the base asset (one contract is one unit); negative is short.
    #[serde(with = "rust_decimal::serde::str")]
    pub size: Decimal,
    /// Average entry price.
    #[serde(with = "rust_decimal::serde::str")]
    pub entry_price: Decimal,
    /// Unrealised PnL. The wire can carry more fractional digits than
    /// `Decimal` holds (28); such values are rounded on decode.
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    /// Return on equity as a fraction.
    #[serde(with = "rust_decimal::serde::str")]
    pub return_on_equity: Decimal,
}

/// `GET /v1/info/position-fills`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionFills {
    /// Fills.
    pub data: Vec<PositionFill>,
    /// Whether another page exists.
    pub more: bool,
    /// Cursor for the next page; absent on the last page.
    pub cursor: Option<String>,
}

/// One fill behind a current position.
///
/// `builder_fee` and `total_fee` are on the wire and not in the published
/// schema (`docs/specs/perps/OBSERVED.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionFill {
    /// Trade id.
    pub trade_id: u64,
    /// Order id.
    pub order_id: u64,
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Side.
    pub side: Side,
    /// Price.
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity in the base asset (one contract is one unit).
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Whether the account was the taker.
    pub taker: bool,
    /// Fee paid.
    #[serde(with = "rust_decimal::serde::str")]
    pub fee: Decimal,
    /// Asset the fee was paid in.
    pub fee_asset: String,
    /// Fee paid to the order's builder. Undocumented.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub builder_fee: Option<Decimal>,
    /// `fee` plus `builder_fee`. Undocumented.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub total_fee: Option<Decimal>,
    /// Whether this fill closed the position at an instrument's permanent
    /// settlement, at the settlement price with no fee or liquidation
    /// penalty. Neither a market trade nor auto-deleveraging.
    pub settlement: bool,
    /// Position size before the fill.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_size: Decimal,
    /// Entry price before the fill.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_entry_price: Decimal,
    /// Realised PnL from the fill.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Whether the fill was a liquidation.
    pub liquidation: bool,
    /// Whether the fill was auto-deleveraging.
    pub adl: bool,
    /// Fill time, Unix ms.
    pub timestamp: u64,
    /// Transaction hash.
    pub hash: String,
}

/// `GET /v1/info/leaderboard`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Leaderboard {
    /// Window the board covers.
    pub window: LeaderboardWindow,
    /// Ranking key.
    pub sort_by: LeaderboardSort,
    /// Snapshot time, Unix ms.
    pub timestamp: u64,
    /// Total ranked accounts.
    pub total: u64,
    /// This page of entries.
    pub entries: Vec<LeaderboardEntry>,
    /// The standing of the `address` the request named; absent otherwise.
    pub account: Option<LeaderboardAccount>,
}

/// One ranked account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LeaderboardEntry {
    /// Rank, 1-based.
    pub rank: u64,
    /// Account address.
    pub account: String,
    /// PnL over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Notional traded over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub notional: Decimal,
    /// Account value.
    #[serde(with = "rust_decimal::serde::str")]
    pub account_value: Decimal,
    /// Return over the window as a fraction (`0.4527` is 45.27%): `pnl` over
    /// the larger of 100 and the starting equity plus the window's peak net
    /// deposits.
    #[serde(with = "rust_decimal::serde::str")]
    pub roi: Decimal,
    /// Instruments the account traded over the window. Undocumented.
    pub instruments_traded: Option<u64>,
    /// The instruments it traded most. Undocumented.
    pub top_instrument_ids: Option<Vec<InstrumentId>>,
}

/// The requesting account's own standing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LeaderboardAccount {
    /// Rank, absent when the account is unranked.
    pub rank: Option<u64>,
    /// Account address.
    pub account: String,
    /// PnL over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Notional traded over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub notional: Decimal,
    /// Account value.
    #[serde(with = "rust_decimal::serde::str")]
    pub account_value: Decimal,
    /// Return over the window as a fraction (`0.4527` is 45.27%): `pnl` over
    /// the larger of 100 and the starting equity plus the window's peak net
    /// deposits.
    #[serde(with = "rust_decimal::serde::str")]
    pub roi: Decimal,
    /// Instruments the account traded over the window. Undocumented.
    pub instruments_traded: Option<u64>,
    /// The instruments it traded most. Undocumented.
    pub top_instrument_ids: Option<Vec<InstrumentId>>,
}

/// `GET /v1/info/invite`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InviteCheck {
    /// Whether the code can be redeemed.
    pub valid: bool,
    /// Why not, when `valid` is false.
    pub error: Option<String>,
}
