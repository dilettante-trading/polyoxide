//! Types for the `wallet` routes: approvals, positions, combo positions,
//! PnL, stats, volume and portfolio value.

use serde::{Deserialize, Serialize};

use super::{OutcomeIndex, PositionStatus};
use crate::types::Allowance;

/// One trusted approval contract in the public response.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApprovalContract {
    /// Current ERC20 allowance: `max` for an unlimited grant, else the raw
    /// integer amount as a string; absent on ERC1155 operator approvals
    /// (all-or-nothing).
    pub amount: Option<Allowance>,
    /// Whether the wallet currently grants this approval.
    pub approved: bool,
    /// The product flow the approval enables (e.g. `trading`).
    pub feature: String,
    /// Catalog identifier of this approval pair (token + spender).
    pub id: String,
    /// The contract approved to spend or operate the token.
    pub spender: String,
    /// Token standard of the pair: `ERC20` or `ERC1155`.
    pub standard: String,
    /// The token contract the approval is granted on.
    pub token: String,
}

/// Non-paginated approval snapshot for one Polygon proxy wallet.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Approvals {
    /// The checked wallet.
    pub address: String,
    /// EVM chain id the state was read on (137 = Polygon).
    pub chain_id: i32,
    /// When the on-chain state was read, RFC3339 UTC (20-second server cache).
    pub checked_at: String,
    /// One row per catalog pair the wallet may need.
    pub contracts: Vec<ApprovalContract>,
}

/// One leg of a combo, with its market and event enrichment.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComboLeg {
    /// On-chain condition id of the leg's market.
    pub leg_condition_id: String,
    /// Live price of the leg outcome (Gamma marks).
    pub leg_current_price: f64,
    /// Position of the leg within the combo, 0-based.
    pub leg_index: i32,
    /// Index of the outcome the combo takes on this leg; `999` means the
    /// outcome could not be labeled.
    pub leg_outcome_index: i32,
    /// Label of the outcome the combo takes on this leg.
    pub leg_outcome_label: String,
    /// Outcome token id of the leg.
    pub leg_position_id: String,
    /// Gamma `closed_time`, RFC3339; `null` while open.
    pub leg_resolved_at: Option<String>,
    /// OPEN / RESOLVED_WIN / RESOLVED_LOSS (live resolution state).
    pub leg_status: String,
    /// The leg's market, with its (single) event nested.
    pub market: ComboLegMarket,
}

/// A leg market's event.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComboLegEvent {
    /// Gamma event id.
    pub event_id: String,
    /// Event image URL.
    pub event_image: String,
    /// Event slug; the URL segment on polymarket.com.
    pub event_slug: String,
    /// Event title.
    pub event_title: String,
}

/// A leg's market, with its (single) event nested.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComboLegMarket {
    /// Gamma market category (e.g. `sports`).
    pub category: String,
    /// Market end date, RFC3339; empty when Gamma has none.
    pub end_date: String,
    /// The market's parent event.
    pub event: ComboLegEvent,
    /// Raw short per-leg label, without the fallback `title` applies; `""`
    /// when Gamma has none.
    pub group_item_title: Option<String>,
    /// Market icon URL.
    pub icon_url: String,
    /// Market image URL.
    pub image_url: String,
    /// The sports line the market is quoted on; `null` when it has none.
    pub line: Option<f64>,
    /// Gamma's own market id; NOT the on-chain condition id (that is the
    /// leg's `leg_condition_id`).
    pub market_id: String,
    /// Label of the leg's outcome on this market.
    pub outcome: String,
    /// The market's outcome labels, in outcome-index order; `[]` when Gamma
    /// has none. Together with `sports_market_type` and `line`, this lets an
    /// executed combo card render both sides of a leg without a Gamma lookup.
    pub outcomes: Option<Vec<String>>,
    /// The market's full question. Serves `""` when Gamma has none.
    ///
    /// The five display-metadata fields below default when absent so cached
    /// payloads written before they existed still deserialize; the query
    /// always serves them.
    pub question: Option<String>,
    /// Market slug; the URL segment on polymarket.com.
    pub slug: String,
    /// Granular sports market type (for example `totals` or
    /// `anytime_touchdowns`); `""` for non-sports markets.
    pub sports_market_type: Option<String>,
    /// Gamma market subcategory.
    pub subcategory: String,
    /// Reserved; always `[]` today.
    pub tags: Vec<String>,
    /// Short per-leg label (`group_item_title`), falling back to the question.
    pub title: String,
}

/// One combo position (`/v2/positions/combos`); a user's holding in a single
/// combo outcome, with leg rollups and enrichment.
///
/// Paginated on `(first_entry_at_micros, combo_condition_id, outcome_index)`;
/// follow the response's `next_cursor` rather than rebuilding that triple.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComboPosition {
    /// On-chain combo condition id (structural, `0x03`-prefixed).
    pub combo_condition_id: String,
    /// Token id of the combo position.
    pub combo_position_id: String,
    /// Current holding in shares.
    pub current_size: f64,
    /// Weighted-average entry price per share, in USDC.
    pub entry_avg_price_usdc: f64,
    /// Entry cost basis in USDC (rounded weighted-average form).
    pub entry_cost_usdc: f64,
    /// Attributed BUY-fee portion of `gross_entry_cost_usdc`, 6-decimal grain.
    /// SELL fees are exit costs and are excluded.
    pub entry_fees_usdc: f64,
    /// First acquisition time, RFC3339.
    pub first_entry_at: String,
    /// Epoch-micros of `first_entry_at`; `null`/absent on the NULL tail.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub first_entry_at_micros: Option<i64>,
    /// Exact fee-inclusive entry basis at 6-decimal grain; formatting to 6 dp
    /// recovers the stored value. Do NOT reconstruct it as
    /// `entry_cost_usdc + entry_fees_usdc`; `entry_cost_usdc` is rounded WAC;
    /// the fee-exclusive basis is `gross_entry_cost_usdc − entry_fees_usdc`.
    pub gross_entry_cost_usdc: f64,
    /// The combo's legs, in leg order, with market and event enrichment.
    pub legs: Vec<ComboLeg>,
    /// Legs still awaiting resolution.
    pub legs_pending: i32,
    /// Legs whose markets have resolved.
    pub legs_resolved: i32,
    /// Number of legs in the combo.
    pub legs_total: i32,
    /// Index of the combo outcome held; `999` means unlabelable.
    pub outcome_index: OutcomeIndex,
    /// Label of the combo outcome held.
    pub outcome_label: String,
    /// The holder's wallet. `proxy_wallet` on every /v2 response; `user` is
    /// the REQUEST param vocabulary, never a response field (2026-08-19).
    pub proxy_wallet: String,
    /// Gross redemption payout received so far in USDC; turnover, not
    /// profit; net result = payout minus `gross_entry_cost_usdc`.
    pub realized_payout_usdc: f64,
    /// Whether the combo can be redeemed now.
    pub redeemable: bool,
    /// When the combo fully resolved, RFC3339; `null` while any leg is open.
    pub resolved_at: Option<String>,
    /// Lifecycle state of the position (OPEN, REDEEMABLE, RESOLVED_WIN,
    /// RESOLVED_LOSS, RESOLVED_PARTIAL).
    pub status: String,
    /// Last event touching the position, RFC3339.
    pub updated_at: String,
    /// Epoch-micros of `updated_at`.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub updated_at_micros: i64,
}

/// `/v2/value`: the user's portfolio value; single-market holdings marked to
/// market plus unresolved combo positions at cost basis, rounded to 4 decimals. Always
/// exactly one row; a user with no positions returns `{ proxy_wallet, value: 0 }`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PortfolioValue {
    /// The wallet the value was computed for.
    pub proxy_wallet: String,
    /// Portfolio value in USDC, rounded to 4 decimals: holdings marked to
    /// market plus non-terminal combos at cost basis.
    pub value: f64,
}

/// One position (`/v2/positions`); a holding in a single outcome token, priced
/// and enriched.
///
/// The shape is **uniform across all three arms** (user OPEN/REDEEMABLE, user
/// CLOSED, market-anchored): clients never branch on which spine answered. On
/// the CLOSED arm `current_size`/`current_value`/`unrealized_pnl` are ~0 by construction, which is exactly
/// what a closed position should report.
///
/// Keyset-paginated on the active `sort_by` key: follow the response's
/// `next_cursor` to page, and expect the ordering to change with `sort_by`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Position {
    /// Whether the market is archived; tells a caller using `includeArchived`
    /// which rows the flag surfaced.
    pub archived: bool,
    /// Weighted-average entry price per share, in USDC.
    pub avg_price: f64,
    /// The on-chain condition id.
    pub condition_id: String,
    /// Live mark per share, in USDC.
    pub current_price: f64,
    /// The CURRENT holding, in shares (~0 residual on the CLOSED arm).
    pub current_size: f64,
    /// `current_size × current_price`, in USDC.
    pub current_value: f64,
    /// Market end date, `YYYY-MM-DD`; `1970-01-01` when Gamma has none.
    pub end_date: String,
    /// The fee-EXCLUSIVE entry basis.
    pub entry_cost_usdc: f64,
    /// Attributed BUY-fee total for the position. Disclosure only:
    /// `entry_cost_usdc` is already fee-exclusive, so never re-deduct this
    /// from a PnL column.
    pub entry_fees_usdc: f64,
    /// Gamma event id of the parent event.
    pub event_id: String,
    /// Parent event slug.
    pub event_slug: String,
    /// Market icon URL.
    pub icon: String,
    /// The row's last economics event, epoch seconds; 0 without native state.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub last_event_at: i64,
    /// Whether the wallet also holds the opposite outcome, so the pair can
    /// merge back into collateral.
    pub mergeable: bool,
    /// Profile display name of the wallet.
    pub name: String,
    /// Whether the market belongs to a neg-risk group.
    pub negative_risk: bool,
    /// Label of the market's other outcome; what a merge pairs with.
    pub opposite_outcome: String,
    /// Token id of the market's other outcome.
    pub opposite_token_id: String,
    /// Label of the held outcome (e.g. `Yes`).
    pub outcome: String,
    /// Index of the held outcome within the market; `999` means the outcome
    /// could not be labeled.
    pub outcome_index: OutcomeIndex,
    /// `(current_value - entry_cost_usdc) / entry_cost_usdc`, as a percent.
    /// Fee-exclusive basis, and the numerator is `unrealized_pnl`; not
    /// `total_pnl / total_cost_usdc`.
    pub percent_pnl: f64,
    /// `(current_value - total_size × avg_price) / (total_size × avg_price)`,
    /// as a percent. A compatibility shape: despite the name, it is not
    /// `realized_pnl` over a basis.
    pub percent_realized_pnl: f64,
    /// Profile image URL.
    pub profile_image: String,
    /// Proxy wallet holding the position.
    pub proxy_wallet: String,
    /// Realized PnL in USDC, cumulative for the position.
    pub realized_pnl: f64,
    /// Whether the position can be redeemed now: its market resolved and the
    /// tokens are still held (losing sides included; redeemable ≠ won).
    pub redeemable: bool,
    /// Market slug; the URL segment on polymarket.com.
    pub slug: String,
    /// The row's actual state; can be narrower than the requested `status`,
    /// since an `OPEN` request also returns `REDEEMABLE` rows.
    pub status: PositionStatus,
    /// Market question title (Gamma enrichment; empty when unenriched).
    pub title: String,
    /// The outcome token id.
    pub token_id: String,
    /// Gross (fee-INCLUSIVE) basis. Always exactly
    /// `entry_cost_usdc + entry_fees_usdc`; the contract sums the two served
    /// columns, so the identity holds on every row of every arm.
    pub total_cost_usdc: f64,
    /// Always equals `realized_pnl + unrealized_pnl`.
    pub total_pnl: f64,
    /// LIFETIME bought shares (the WAC denominator), never the
    /// current balance; that is `current_size`.
    pub total_size: f64,
    /// Unrealized (mark-to-market) PnL: `current_value - entry_cost_usdc`.
    pub unrealized_pnl: f64,
    /// Profile verification badge.
    pub verified: bool,
}

/// One dense cumulative v2 chart point in USDC.
///
/// Nullable values mean the historical source or mark was unavailable. Clients
/// must not coerce them to zero.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserPnlPoint {
    /// `deposits - withdrawals`.
    pub cashflow_net: Option<f64>,
    /// Collateral moved into the wallet.
    pub deposits: Option<f64>,
    /// `position_pnl + wallet_income`; the all-in economic result.
    pub economic_pnl: Option<f64>,
    /// Negative cumulative fee charges, for direct chart composition.
    pub fees: Option<f64>,
    /// Refunds minus charges; a disclosure, not another PnL adjustment.
    pub fees_paid: Option<f64>,
    /// Fee atom: total fees refunded.
    pub fees_refunded: Option<f64>,
    /// Maker-side fee rebates credited.
    pub maker_rebate: Option<f64>,
    /// `realized_pnl + unrealized_pnl`; the position-only result.
    pub position_pnl: Option<f64>,
    /// Realized PnL from combo positions.
    pub realized_combo_pnl: f64,
    /// Realized PnL from AMM liquidity-provision activity.
    pub realized_lp_pnl: f64,
    /// Realized PnL from market positions.
    pub realized_market_pnl: f64,
    /// `realized_market_pnl + realized_lp_pnl + realized_combo_pnl`.
    pub realized_pnl: f64,
    /// Referral income credited.
    pub referral_income: Option<f64>,
    /// Reward-program income credited.
    pub reward_income: Option<f64>,
    /// `realized_pnl + wallet_income`; settled economics, no marks.
    pub settled_pnl: Option<f64>,
    /// Chain block the point was observed at.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub source_block: i64,
    /// `reward_income + yield_income + referral_income`.
    pub sponsored_income: Option<f64>,
    /// Taker-side fee rebates credited.
    pub taker_rebate: Option<f64>,
    /// Point timestamp, in epoch seconds. Every amount below is CUMULATIVE
    /// through this instant, in USDC; `null` means the source is uncovered,
    /// never zero.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
    /// Cumulative maker-attributed canonical exchange fill count.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub trade_count: u64,
    /// `position_pnl - realized_lp_pnl + fees_charged - fees_refunded`; the
    /// compatibility chart series (`p` on the bare user-pnl route).
    pub trade_pnl: Option<f64>,
    /// Mark-to-market of open inventory.
    pub unrealized_pnl: Option<f64>,
    /// Cumulative maker-attributed canonical exchange fill shares.
    pub volume: f64,
    /// Cumulative maker-attributed canonical exchange fill cash in USDC.
    pub volume_usdc: f64,
    /// All income credited to the wallet: rebates (maker + taker) plus
    /// reward, yield and referral income.
    pub wallet_income: Option<f64>,
    /// Collateral moved out of the wallet.
    pub withdrawals: Option<f64>,
    /// Yield income credited.
    pub yield_income: Option<f64>,
}

/// Complete v2 user-PnL response data.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserPnlSeries {
    /// Grid step the points were synthesized on (`1d`, `18h`, `12h`, `3h`, `1h`).
    pub fidelity: String,
    /// The window served (`max`, `all`, `1m`, `1w`, `1d`, `12h`, `6h`).
    pub interval: String,
    /// Dense cumulative points on the requested grid, oldest first.
    pub points: Vec<UserPnlPoint>,
    /// The wallet the series was computed for.
    pub proxy_wallet: String,
    /// Historical MVP observations are daily even when carried onto a finer grid.
    pub source_fidelity: String,
}

/// The profile card for one wallet, plus its newest persisted all-time PnL
/// observation.
///
/// `join_date` is `null` when unknown, which is a real state rather than an
/// error; a meaningful share of accounts have no recorded creation time.
/// `biggest_win` is `0` when the wallet has no win over $1. `all_time_pnl` is
/// `null` when the user is known but has no observation yet; a missing user is
/// represented by the outer response `data: null` instead.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserStats {
    /// Newest persisted cumulative all-time PnL point, or `null` when absent.
    pub all_time_pnl: Option<UserPnlPoint>,
    /// Largest single resolved win, in USDC.
    pub biggest_win: f64,
    /// Epoch seconds, or `null` when the join date is unknown.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub join_date: Option<i64>,
    /// The proxy wallet these lifetime statistics describe.
    pub proxy_wallet: String,
    /// Distinct markets the wallet traded; an exact count.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub trades: u64,
    /// Profile view count.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub views: u64,
}

/// `/v2/user-volume`: one wallet's trading volume over a whole-day window, in
/// both units side by side.
///
/// `volume` is both-sides SHARES; `volume_usdc` is the same measure in USD;
/// the bare name is the natural unit and `_usdc` is the denomination, the same
/// pairing as `current_size` / `entry_cost_usdc` on `/v2/positions`.
/// `trade_count` is the number of trades in the window.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserVolume {
    /// Number of fills in the window.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub trade_count: u64,
    /// Both-sides traded volume over the window, in shares.
    pub volume: f64,
    /// Both-sides cash volume over the window, in USD.
    pub volume_usdc: f64,
}
