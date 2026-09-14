//! Types for the `markets` routes: holders, live volume, open interest,
//! price history and resolutions.

use serde::{Deserialize, Serialize};

use super::OutcomeIndex;

/// One `/v2/live-volume` row: `taker_volume` is the market's cumulative
/// one-side (taker) volume, truncated to 6 decimal places; qualified because
/// the boards' `volume` is both-sides, and two measures must not share a name.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ConditionVolume {
    /// On-chain condition id of the market (`0x` hex).
    pub condition_id: String,
    /// Cumulative one-side (taker) volume in shares, truncated to 6 decimals.
    pub taker_volume: f64,
}

/// One `/v2/holders` row: a market holder enriched with their public profile,
/// netted across the market's outcome tokens by default or at per-side gross
/// grain with the position economics when `include_pnl=true`.
/// `profile_image_optimized` is always empty.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Holder {
    /// Holding in shares. By default the NET figure (a fully hedged wallet
    /// nets to `0.0` and only appears at `min_balance=0`); with
    /// `include_pnl=true` the per-side GROSS figure, so each side of a hedged
    /// wallet appears under its own token with that side's full size.
    pub amount: f64,
    /// Historical entry price per share. Served only with `include_pnl=true`.
    pub avg_price: Option<f64>,
    /// Profile bio text.
    pub bio: String,
    /// Current price of the held outcome token, bounded to `[0, 1]`. Served
    /// only with `include_pnl=true`.
    pub current_price: Option<f64>,
    /// Mark value of the holding: `amount` times `current_price`. Served only
    /// with `include_pnl=true`.
    pub current_value: Option<f64>,
    /// Whether the profile chose to show its name publicly.
    pub display_username_public: bool,
    /// Cost basis of the held size in USDC, excluding entry fees. Served only
    /// with `include_pnl=true`.
    pub entry_cost_usdc: Option<f64>,
    /// Profile display name of the wallet.
    pub name: String,
    /// Index of the held outcome within the market; `999` means unlabelable.
    pub outcome_index: OutcomeIndex,
    /// Profile image URL.
    pub profile_image: String,
    /// Resized profile image URL, when one exists.
    pub profile_image_optimized: String,
    /// The holding wallet.
    pub proxy_wallet: String,
    /// Generated fallback handle for profiles without a display name.
    pub pseudonym: String,
    /// Profit already locked in by sells and redemptions. Served only with
    /// `include_pnl=true`.
    pub realized_pnl: Option<f64>,
    /// Outcome token held.
    pub token_id: String,
    /// Total profit and loss; always `realized_pnl + unrealized_pnl`. Served
    /// only with `include_pnl=true`.
    pub total_pnl: Option<f64>,
    /// Mark-to-market profit on the held size: `current_value` minus
    /// `entry_cost_usdc`. Served only with `include_pnl=true`.
    pub unrealized_pnl: Option<f64>,
    /// Profile verification badge.
    pub verified: bool,
}

/// `/v2/live-volume`: one entry per market in the requested event(s), ordered
/// by `taker_volume` descending, plus `taker_volume_total`; their sum. Events
/// that resolve to no markets serve `{ taker_volume_total: 0.0, conditions: [] }`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LiveVolume {
    /// One row per market under the requested event(s), `taker_volume`
    /// descending; empty when the events resolve to no markets.
    pub conditions: Vec<ConditionVolume>,
    /// Sum of the rows' `taker_volume`, in shares.
    pub taker_volume_total: f64,
}

/// One outcome token's holder group in `/v2/holders`: the `token_id` and its
/// holders, top-N by net balance. A multi-market request interleaves tokens
/// across the page, so merge groups by `token_id`, not by array position.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MetaHolder {
    /// Top holders of that token, amount descending (net by default,
    /// per-side gross with `include_pnl=true`).
    pub holders: Vec<Holder>,
    /// The outcome token this group ranks.
    pub token_id: String,
}

/// One `/v2/oi` row: the **priced gross** open interest of a market;
/// `Σ ((shares − fee_receiver_shares)/1e6 · outcome_price)` across every
/// outcome, with no netting. `condition_id` is the market's on-chain condition; the
/// global shape carries `condition_id = "GLOBAL"`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OpenInterest {
    /// Condition id the row answers for; `GLOBAL` on the parameterless
    /// global figure.
    pub condition_id: String,
    /// Priced gross open interest in USDC; `0.0` when nothing is held.
    pub value: f64,
}

/// One served point.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PricePoint {
    /// Price in the range 0…1. Always a JSON number.
    pub price: f64,
    /// Width of the window this price was observed in: `0` for an exact tick,
    /// the bucket width for an aggregate. So the price was observed within
    /// `[timestamp, timestamp + resolution_seconds)`, and a caller can tell
    /// whether that is precise enough.
    ///
    /// Per-point rather than per-response because points in one response
    /// genuinely differ: grid points carry the requested bucket width, the
    /// series' terminal point is an exact tick at `0`, and a tier-degraded
    /// `as_of` carries its tier's width.
    ///
    /// This is the ONLY thing the payload says about provenance, deliberately.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub resolution_seconds: i64,
    /// The OBSERVATION's own time, epoch seconds, never the time that was
    /// asked for. A raw tick reports the tick; an aggregate reports its bucket
    /// start.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
}

/// One non-paginated `/v2/resolutions` row. UMA lifecycle rows populate the
/// numeric-string price fields; direct question lookups omit `condition_id`,
/// while condition/event lookups retain both the selected condition and backing
/// UMA question. Native V2 and terminal CTF rows populate condition lifecycle,
/// payout, provenance, and finality fields where those sources provide them.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Resolution {
    /// Condition id the row answers for; absent on question-keyed rows.
    pub condition_id: Option<String>,
    /// True while a managed proposal sits past its normal expiry in extended
    /// review; always false outside that window.
    pub extended_review: bool,
    /// Latest lifecycle change: an epoch-seconds string on question-keyed
    /// rows, RFC3339 UTC on condition-keyed rows.
    pub last_update_timestamp: String,
    /// Log index of the latest lifecycle event, as a numeric string; empty
    /// where `transaction_hash` is empty.
    pub log_index: String,
    /// BINARY, INCREMENTAL_NEGRISK or ATOMIC_NEGRISK; condition-keyed rows only.
    pub market_type: Option<String>,
    /// Whether the question rules were updated after posing.
    pub new_version_q: bool,
    /// Per-outcome payout in micro-USDC per share, `[outcome0, outcome1]`;
    /// present on resolved condition-keyed rows.
    #[cfg_attr(feature = "specta", specta(type = Option<Vec<f64>>))]
    pub payouts: Option<Vec<i64>>,
    /// Final settlement price, same conventions as `proposed_price`.
    pub price: Option<String>,
    /// Price of the first proposal as a numeric string; `69` means unset.
    /// Present on question-keyed rows only.
    pub proposed_price: Option<String>,
    /// UMA question id serving the row; absent on condition-keyed rows.
    pub question_id: Option<String>,
    /// Reporter family that resolved it: UMA_OO, CHAINLINK or EOA.
    pub reporter: Option<String>,
    /// Price of the second proposal, same conventions as `proposed_price`.
    pub reproposed_price: Option<String>,
    /// `reported` (an oracle reported it) or `derived` (a neg-risk sibling
    /// resolution no client can reconstruct).
    pub resolution_source: Option<String>,
    /// When the condition resolved, RFC3339 UTC.
    pub resolved_at: Option<String>,
    /// Block the condition resolved at.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub resolved_block: Option<i64>,
    /// Lifecycle state: initialized, posed, proposed, challenged, reproposed,
    /// disputed or resolved; condition-keyed rows can also serve active and
    /// arbitration.
    pub status: String,
    /// Transaction of the latest lifecycle event; empty on condition-keyed
    /// rows without one.
    pub transaction_hash: String,
    /// Whether arbitration was triggered on the request.
    pub was_arbitrated: Option<bool>,
    /// Whether the resolution was disputed at any point.
    pub was_disputed: bool,
}
