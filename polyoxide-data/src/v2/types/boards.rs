//! Types for the `boards` routes: biggest winners, the builder boards and
//! the trader leaderboard.

use serde::{Deserialize, Serialize};

/// One `/v2/biggest-winners` row: a single winning POSITION, not a user total.
///
/// `kind` is `market` or `combo`. Combo rows carry a `' / '`-joined title of
/// their legs and have no Gamma event; `event_id` is `0` and `event_slug` is
/// empty; so branch on `kind` before building an event link.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BiggestWinner {
    /// On-chain condition id of the market (combo rows: the combo condition).
    pub condition_id: String,
    /// Gamma event id of the parent event; `0` on combo rows.
    pub event_id: i32,
    /// Parent event slug; empty on combo rows.
    pub event_slug: String,
    /// Parent event title; on combo rows, the `' / '`-joined leg questions.
    pub event_title: String,
    /// Value at resolution, in USDC.
    pub final_value: f64,
    /// Cost basis of the winning position, in USDC.
    pub initial_value: f64,
    /// `market` or `combo`; combo rows carry no Gamma event (`event_id` 0,
    /// empty `event_slug`), so branch on this before building event links.
    pub kind: String,
    /// `final_value - initial_value`, in USDC.
    pub pnl: f64,
    /// Token id of the winning position.
    pub position_id: String,
    /// Profile image URL.
    pub profile_image: String,
    /// Unix seconds; when the position resolved.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub resolved_at: i64,
    /// The winning wallet.
    pub user_id: String,
    /// Profile display name of the wallet.
    pub user_name: String,
    /// Unique 1-based ordinal within the window/category; `row_number()`, so
    /// equal PnL does not share a rank (unlike the leaderboard's `rank`).
    pub win_rank: u32,
}

/// One `/v2/builders/leaderboard` row: a builder's standing for the window.
///
/// `volume` is in SHARES, not USDC; the same unit as `/v2/leaderboard`'s `volume`.
/// `active_users` counts distinct makers in the window. `builder_name` is a
/// display name that falls back to `builder_code` when the builder has no
/// profile; `profile_image` is the avatar, the same spelling every payload
/// carrying one uses.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BuilderStanding {
    /// Distinct active users attributed to the builder in the window.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub active_users: u64,
    /// Stable identifier of the builder.
    pub builder_code: String,
    /// Builder display name; cosmetic; key on `builder_code`.
    pub builder_name: String,
    /// Builder profile image URL.
    pub profile_image: String,
    /// Board rank in the requested window; ties share a rank and the next skips.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub rank: u64,
    /// Whether the builder is verified.
    pub verified: bool,
    /// Volume attributed to the builder in the window, in shares.
    pub volume: f64,
}

/// One `/v2/builders/volume` row: a builder's volume in ONE bucket of the
/// series, not a running total.
///
/// `date` is the bucket start (`YYYY-MM-DD`) and its width is the request's
/// `interval`; daily, weekly, monthly, or yearly for `all`. `rank` is the
/// builder's placing **within that bucket**, so it moves from bucket to bucket.
/// `volume` is in SHARES, as on the leaderboard.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BuilderVolumePoint {
    /// Distinct active users attributed in that bucket.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub active_users: u64,
    /// Stable identifier of the builder.
    pub builder_code: String,
    /// Builder display name; cosmetic; key on `builder_code`.
    pub builder_name: String,
    /// Bucket start date, `YYYY-MM-DD` (UTC); `interval` sets the width.
    pub date: String,
    /// Builder profile image URL.
    pub profile_image: String,
    /// Builder's rank within that bucket.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub rank: u64,
    /// Whether the builder is verified.
    pub verified: bool,
    /// Volume attributed in that bucket, in shares.
    pub volume: f64,
}

/// One `/v2/leaderboard` row: a user's standing on the ranked board.
///
/// `pnl` is realized USDC over the window, base and combo positions unified.
/// `volume` is both-sides SHARES, not USDC. `rank` is a competition
/// rank; tied users share one, and the next rank skips.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LeaderboardEntry {
    /// Window PnL in USDC. Finite windows (day/week/month) are the MARKED
    /// equity change net of flows; realized plus mark moves; `all` is the
    /// realized-only lifetime ledger. The two deliberately differ.
    pub pnl: f64,
    /// Profile image URL.
    pub profile_image: String,
    /// Rank on the requested board; ties share a rank and the next skips.
    pub rank: u32,
    /// The ranked wallet.
    pub user_id: String,
    /// Profile display name of the wallet.
    pub user_name: String,
    /// Profile verification badge.
    pub verified: bool,
    /// Both-sides traded volume in shares; never USD.
    pub volume: f64,
    /// Linked X handle, when one exists.
    pub x_username: String,
}

/// `/v2/leaderboard?user=`; one user's standing, carrying BOTH ranks.
///
/// The ranked board is materialised per sort, so it can only answer "where does
/// this user place by PnL" or "by volume", one at a time. This shape comes from
/// the by-user pivot instead and answers both at once.
///
/// A `null` rank means **unranked** for that sort: the user exists in the pivot
/// but is filtered out of that board.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LeaderboardUserEntry {
    /// Window PnL in USDC, same semantics as the board rows (finite windows
    /// marked, `all` realized-only).
    pub pnl: f64,
    /// Profile image URL.
    pub profile_image: String,
    /// `None` when unranked on the PnL board.
    pub rank_pnl: Option<u32>,
    /// `None` when unranked on the volume board.
    pub rank_volume: Option<u32>,
    /// The looked-up wallet.
    pub user_id: String,
    /// Profile display name of the wallet.
    pub user_name: String,
    /// Profile verification badge.
    pub verified: bool,
    /// Both-sides traded volume in shares; never USD.
    pub volume: f64,
    /// Linked X handle, when one exists.
    pub x_username: String,
}
