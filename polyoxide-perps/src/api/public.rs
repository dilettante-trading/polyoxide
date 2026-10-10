//! Public-by-address lookups and the invite check:
//! `/v1/info/{portfolio,position-fills,leaderboard,invite}`.

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    api::Fetch,
    error::PerpsError,
    types::{InstrumentId, LeaderboardSort, LeaderboardWindow, Side, SortOrder},
};

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
    /// Signed size in contracts; negative is short.
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
/// `builder_fee`, `settlement` and `total_fee` are on the wire and not in the
/// published schema (`docs/specs/perps/OBSERVED.md`).
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
    /// Quantity in contracts.
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
    /// Whether this was a settlement fill. Undocumented.
    pub settlement: Option<bool>,
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
