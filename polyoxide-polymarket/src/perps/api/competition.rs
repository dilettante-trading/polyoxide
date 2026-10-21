//! Trading competitions: `/v1/info/competitions` and
//! `/v1/info/competition{,/leaderboard,/account,/widget}`.
//!
//! A competition scores each account by PnL times the square root of its
//! boosted volume, where a market's boost multiplies its volume and never its
//! PnL; only accounts that reach the minimum unboosted volume are ranked.

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::perps::{
    api::Fetch,
    error::PerpsError,
    types::{CompetitionId, CompetitionRowState, CompetitionStatus, InstrumentId},
};

/// Competitions namespace.
#[derive(Clone)]
pub struct CompetitionApi {
    pub(crate) http_client: HttpClient,
}

impl CompetitionApi {
    /// `GET /v1/info/competitions`: every competition, newest first.
    pub fn list(&self) -> ListCompetitions {
        ListCompetitions {
            request: Request::new(self.http_client.clone(), "/v1/info/competitions"),
        }
    }

    /// `GET /v1/info/competition`: one competition's settings, status and
    /// prize table. An id the host does not know is its `404`.
    pub fn get(&self, competition_id: CompetitionId) -> Fetch<CompetitionDetail> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/competition")
                .query("competition_id", competition_id),
        }
    }

    /// `GET /v1/info/competition/leaderboard`: one competition's ranked rows,
    /// 50 by default and at most 100 a page.
    pub fn leaderboard(&self, competition_id: CompetitionId) -> GetCompetitionLeaderboard {
        GetCompetitionLeaderboard {
            request: Request::new(self.http_client.clone(), "/v1/info/competition/leaderboard")
                .query("competition_id", competition_id),
        }
    }

    /// `GET /v1/info/competition/account`: one account's standing in one
    /// competition. An address with no activity has zero figures and no rank.
    pub fn account(
        &self,
        competition_id: CompetitionId,
        address: impl Into<String>,
    ) -> Fetch<CompetitionAccount> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/competition/account")
                .query("competition_id", competition_id)
                .query("address", address.into()),
        }
    }

    /// `GET /v1/info/competition/widget`: the live competition at a glance.
    pub fn widget(&self) -> GetCompetitionWidget {
        GetCompetitionWidget {
            request: Request::new(self.http_client.clone(), "/v1/info/competition/widget"),
        }
    }
}

/// Request builder for `GET /v1/info/competitions`.
pub struct ListCompetitions {
    request: Request<CompetitionList, PerpsError>,
}

impl ListCompetitions {
    polyoxide_core::query_setters! {
        /// Also list the internal competitions whose allowlist holds this
        /// address. A display filter, not access control.
        address: impl Into<String> => "address",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<CompetitionList, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/competition/leaderboard`.
pub struct GetCompetitionLeaderboard {
    request: Request<CompetitionLeaderboard, PerpsError>,
}

impl GetCompetitionLeaderboard {
    polyoxide_core::query_setters! {
        /// Page size: 50 by default, at most 100.
        limit: u32 => "limit",
        /// Page offset.
        offset: u64 => "offset",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<CompetitionLeaderboard, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/competition/widget`.
pub struct GetCompetitionWidget {
    request: Request<CompetitionWidget, PerpsError>,
}

impl GetCompetitionWidget {
    polyoxide_core::query_setters! {
        /// Also return this account's rank and progress.
        address: impl Into<String> => "address",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<CompetitionWidget, PerpsError> {
        self.request.send().await
    }
}

/// `GET /v1/info/competitions`.
///
/// `live_competition_ids` is on the wire and not in the published schema,
/// which names `live_competition_id` instead (`docs/specs/perps/OBSERVED.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionList {
    /// Answer time, Unix ms.
    pub timestamp: u64,
    /// Competitions, newest first.
    pub competitions: Vec<CompetitionSummary>,
    /// The live competition, when one runs.
    pub live_competition_id: Option<CompetitionId>,
    /// The live competitions. Undocumented; sent where the schema names
    /// `live_competition_id`.
    pub live_competition_ids: Option<Vec<CompetitionId>>,
    /// The competition to open on: the live one, else the next upcoming one,
    /// else the latest ended one.
    pub landing_competition_id: Option<CompetitionId>,
}

/// One competition in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionSummary {
    /// Competition id.
    pub competition_id: CompetitionId,
    /// Sequential number, as in "Competition 3".
    pub number: u64,
    /// Name.
    pub name: String,
    /// Theme or sector label, such as `Semis`; empty when none.
    pub sector_label: String,
    /// Lifecycle status.
    pub status: CompetitionStatus,
    /// Start, inclusive, Unix ms.
    pub start_time: u64,
    /// End, exclusive, Unix ms.
    pub end_time: u64,
    /// Total prize pool in the prize asset.
    #[serde(with = "rust_decimal::serde::str")]
    pub prize_pool: Decimal,
    /// Asset the prizes are paid in.
    pub prize_asset_id: u64,
    /// Whether only its allowlist sees it listed.
    pub internal: bool,
}

/// `GET /v1/info/competition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionDetail {
    /// Competition id.
    pub competition_id: CompetitionId,
    /// Sequential number.
    pub number: u64,
    /// Name.
    pub name: String,
    /// Theme or sector label; empty when none.
    pub sector_label: String,
    /// How the boosts work, for display.
    pub boost_description: String,
    /// Lifecycle status.
    pub status: CompetitionStatus,
    /// Start, inclusive, Unix ms.
    pub start_time: u64,
    /// End, exclusive, Unix ms.
    pub end_time: u64,
    /// Whether every perpetual counts, those listed during the competition
    /// included. When false only `markets` count.
    pub all_markets: bool,
    /// With `all_markets`, the boosted markets only; otherwise every market
    /// that counts, with its boost.
    pub markets: Vec<CompetitionMarket>,
    /// Unboosted notional volume an account needs to be ranked.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_volume: Decimal,
    /// Whether unrealised PnL on open qualifying positions counts.
    pub count_unrealized: bool,
    /// Total prize pool in the prize asset.
    #[serde(with = "rust_decimal::serde::str")]
    pub prize_pool: Decimal,
    /// Asset the prizes are paid in.
    pub prize_asset_id: u64,
    /// The prize table.
    pub prize_tiers: Vec<CompetitionPrizeTier>,
    /// Size of the prize zone: ranks 1 to `paid_places`.
    pub paid_places: u64,
    /// Whether only its allowlist sees it listed.
    pub internal: bool,
    /// Accounts ranked, having reached `min_volume`.
    pub ranked: u64,
    /// Accounts with at least one counted fill.
    pub participants: u64,
    /// Whether the leaderboard shows frozen end-of-competition results.
    pub final_results: bool,
    /// Version of the scoring rules.
    pub scoring_version: u64,
    /// The lowest score in the prize zone.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub prize_cutoff_score: Option<Decimal>,
    /// The market a "Start Trading" button opens: the boosted market with the
    /// most 24-hour volume, else the top market overall.
    pub start_trading_instrument_id: Option<InstrumentId>,
    /// When the standings were last updated, Unix ms.
    pub updated_at: Option<u64>,
}

/// One market in a competition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionMarket {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Multiplier on the market's volume towards boosted volume; `1` when not
    /// boosted.
    #[serde(with = "rust_decimal::serde::str")]
    pub boost: Decimal,
}

/// One band of a prize table: every rank from `from_rank` to `to_rank`,
/// inclusive, is paid `amount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionPrizeTier {
    /// First rank of the band, from 1.
    pub from_rank: u64,
    /// Last rank of the band.
    pub to_rank: u64,
    /// Prize per trader, in the prize asset.
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: Decimal,
}

/// `GET /v1/info/competition/leaderboard`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionLeaderboard {
    /// Competition id.
    pub competition_id: CompetitionId,
    /// Lifecycle status.
    pub status: CompetitionStatus,
    /// Ranked accounts.
    pub total: u64,
    /// This page's offset.
    pub offset: u64,
    /// This page of rows, at most 100.
    pub entries: Vec<CompetitionEntry>,
    /// Whether the rows are frozen end-of-competition results.
    pub final_results: bool,
    /// The event time the standings reflect, Unix ms.
    pub updated_at: Option<u64>,
}

/// One row of a competition leaderboard. Ties go to the higher boosted
/// volume, then the earlier first qualifying fill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionEntry {
    /// Account address.
    pub account: String,
    /// Rank, from 1.
    pub rank: Option<u64>,
    /// Score: PnL times the square root of boosted volume, to six decimal
    /// places. Can be negative.
    #[serde(with = "rust_decimal::serde::str")]
    pub score: Decimal,
    /// Competition PnL after fees, funding and liquidation losses.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Realised PnL.
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    /// Unrealised PnL.
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    /// Unboosted notional volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Boosted notional volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub boosted_volume: Decimal,
    /// How the row is shown.
    pub row_state: CompetitionRowState,
    /// The projected prize while live, the prize won once ended; absent
    /// outside the prize zone.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub prize: Option<Decimal>,
    /// When the prize was paid, Unix ms.
    pub paid_at: Option<u64>,
    /// The prize payment's transaction hash.
    pub payout_tx_hash: Option<String>,
}

/// `GET /v1/info/competition/account`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionAccount {
    /// Competition id.
    pub competition_id: CompetitionId,
    /// Lifecycle status.
    pub status: CompetitionStatus,
    /// Account address.
    pub account: String,
    /// Whether it reached the minimum volume and is ranked.
    pub qualified: bool,
    /// Rank, once qualified.
    pub rank: Option<u64>,
    /// Score.
    #[serde(with = "rust_decimal::serde::str")]
    pub score: Decimal,
    /// Competition PnL.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Realised PnL.
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    /// Unrealised PnL.
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    /// Fees paid, net of rebates.
    #[serde(with = "rust_decimal::serde::str")]
    pub fees: Decimal,
    /// Funding received, net of funding paid.
    #[serde(with = "rust_decimal::serde::str")]
    pub funding: Decimal,
    /// Unboosted notional volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Boosted notional volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub boosted_volume: Decimal,
    /// The competition's minimum unboosted volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_volume: Decimal,
    /// Unboosted volume still needed to be ranked; `0` once qualified.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_to_qualify: Decimal,
    /// Fraction of the minimum reached, `0` to `1`.
    #[serde(with = "rust_decimal::serde::str")]
    pub qualification_progress: Decimal,
    /// How the account's row is shown.
    pub row_state: CompetitionRowState,
    /// Positions open when the competition started, by market, those since
    /// closed included.
    pub locked_positions: Vec<CompetitionLockedPosition>,
    /// The projected or won prize; absent outside the prize zone.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub prize: Option<Decimal>,
    /// Places between this rank and the last paid place; `0` inside the
    /// prize zone, absent when unranked.
    pub spots_from_prize_zone: Option<u64>,
    /// Score still needed to reach the prize zone; `0` inside it, absent
    /// when unranked.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub score_to_prize_zone: Option<Decimal>,
    /// When the prize was paid, Unix ms.
    pub paid_at: Option<u64>,
    /// The prize payment's transaction hash.
    pub payout_tx_hash: Option<String>,
    /// When the standing was last updated, Unix ms.
    pub updated_at: Option<u64>,
}

/// A position open when a competition started. Its market does not count
/// for the account until the position is fully closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionLockedPosition {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Signed size at the start: positive long, negative short.
    #[serde(with = "rust_decimal::serde::str")]
    pub size_at_start: Decimal,
    /// When it was fully closed and the market began to count, Unix ms;
    /// absent while still locked.
    pub released_at: Option<u64>,
}

/// `GET /v1/info/competition/widget`. With no live competition only
/// `timestamp` is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CompetitionWidget {
    /// Answer time, Unix ms.
    pub timestamp: u64,
    /// The live competition.
    pub competition_id: Option<CompetitionId>,
    /// Its sequential number.
    pub number: Option<u64>,
    /// Its end, Unix ms.
    pub end_time: Option<u64>,
    /// Its minimum unboosted volume.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub min_volume: Option<Decimal>,
    /// The requested account's unboosted volume.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub volume: Option<Decimal>,
    /// Whether the requested account is ranked.
    pub qualified: Option<bool>,
    /// Fraction of the minimum the requested account reached, `0` to `1`.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub qualification_progress: Option<Decimal>,
    /// The requested account's rank.
    pub rank: Option<u64>,
    /// Places between the requested account's rank and the last paid place.
    pub spots_from_prize_zone: Option<u64>,
    /// When the standings were last updated, Unix ms.
    pub updated_at: Option<u64>,
}
