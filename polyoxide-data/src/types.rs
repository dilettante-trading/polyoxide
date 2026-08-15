use serde::{Deserialize, Serialize};

/// User's total position value
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserValue {
    /// User address
    pub user: String,
    /// Total value of positions
    pub value: f64,
}

/// Open interest for a market
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenInterest {
    /// Market condition ID
    pub market: String,
    /// Open interest value
    pub value: f64,
}

/// Sort field options for position queries
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PositionSortBy {
    /// Sort by current value
    Current,
    /// Sort by initial value
    Initial,
    /// Sort by token count
    Tokens,
    /// Sort by cash P&L
    CashPnl,
    /// Sort by percentage P&L
    PercentPnl,
    /// Sort by market title
    Title,
    /// Sort by resolving status
    Resolving,
    /// Sort by price
    Price,
    /// Sort by average price
    AvgPrice,
}

impl std::fmt::Display for PositionSortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Current => write!(f, "CURRENT"),
            Self::Initial => write!(f, "INITIAL"),
            Self::Tokens => write!(f, "TOKENS"),
            Self::CashPnl => write!(f, "CASH_PNL"),
            Self::PercentPnl => write!(f, "PERCENT_PNL"),
            Self::Title => write!(f, "TITLE"),
            Self::Resolving => write!(f, "RESOLVING"),
            Self::Price => write!(f, "PRICE"),
            Self::AvgPrice => write!(f, "AVG_PRICE"),
        }
    }
}

/// Sort direction for queries
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum SortDirection {
    /// Ascending order
    Asc,
    /// Descending order (default)
    #[default]
    Desc,
}

impl std::fmt::Display for SortDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asc => write!(f, "ASC"),
            Self::Desc => write!(f, "DESC"),
        }
    }
}

/// Sort field options for closed position queries
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClosedPositionSortBy {
    /// Sort by realized P&L (default)
    #[default]
    RealizedPnl,
    /// Sort by market title
    Title,
    /// Sort by price
    Price,
    /// Sort by average price
    AvgPrice,
    /// Sort by timestamp
    Timestamp,
}

impl std::fmt::Display for ClosedPositionSortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RealizedPnl => write!(f, "REALIZED_PNL"),
            Self::Title => write!(f, "TITLE"),
            Self::Price => write!(f, "PRICE"),
            Self::AvgPrice => write!(f, "AVG_PRICE"),
            Self::Timestamp => write!(f, "TIMESTAMP"),
        }
    }
}

/// Closed position record
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPosition {
    /// Proxy wallet address
    pub proxy_wallet: String,
    /// Asset identifier (token ID)
    pub asset: String,
    /// Condition ID of the market
    pub condition_id: String,
    /// Average entry price
    pub avg_price: f64,
    /// Total amount bought
    pub total_bought: f64,
    /// Realized profit and loss
    pub realized_pnl: f64,
    /// Current market price
    pub cur_price: f64,
    /// Timestamp when position was closed
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
    /// Market title
    pub title: String,
    /// Market slug
    pub slug: String,
    /// Market icon URL
    pub icon: Option<String>,
    /// Event slug
    pub event_slug: Option<String>,
    /// Outcome name (e.g., "Yes", "No")
    pub outcome: String,
    /// Outcome index (0 or 1 for binary markets)
    pub outcome_index: u32,
    /// Opposite outcome name
    pub opposite_outcome: String,
    /// Opposite outcome asset ID
    pub opposite_asset: String,
    /// Market end date
    pub end_date: Option<String>,
}

/// Trade side (buy or sell)
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TradeSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
    /// Unrecognized trade side (forward-compat). `Trade::side` is deserialized
    /// from live API responses, so an unexpected value falls back here instead
    /// of failing the whole page. Never construct this to send in a request filter.
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for TradeSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Filter type for trade queries
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TradeFilterType {
    /// Filter by cash amount
    Cash,
    /// Filter by token amount
    Tokens,
}

impl std::fmt::Display for TradeFilterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cash => write!(f, "CASH"),
            Self::Tokens => write!(f, "TOKENS"),
        }
    }
}

/// Trade record
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Proxy wallet address
    pub proxy_wallet: String,
    /// Trade side (BUY or SELL)
    pub side: TradeSide,
    /// Asset identifier (token ID)
    pub asset: String,
    /// Condition ID of the market
    pub condition_id: String,
    /// Trade size (number of shares)
    pub size: f64,
    /// Trade price
    pub price: f64,
    /// Trade timestamp
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
    /// Market title
    pub title: String,
    /// Market slug
    pub slug: String,
    /// Market icon URL
    pub icon: Option<String>,
    /// Event slug
    pub event_slug: Option<String>,
    /// Outcome name (e.g., "Yes", "No")
    pub outcome: String,
    /// Outcome index (0 or 1 for binary markets)
    pub outcome_index: u32,
    /// User display name
    pub name: Option<String>,
    /// User pseudonym
    pub pseudonym: Option<String>,
    /// User bio
    pub bio: Option<String>,
    /// User profile image URL
    pub profile_image: Option<String>,
    /// Optimized profile image URL
    pub profile_image_optimized: Option<String>,
    /// Transaction hash
    pub transaction_hash: Option<String>,
}

/// Activity type
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ActivityType {
    /// Trade activity
    Trade,
    /// Split activity
    Split,
    /// Merge activity
    Merge,
    /// Redeem activity
    Redeem,
    /// Reward activity
    Reward,
    /// Conversion activity
    Conversion,
    /// Collateral deposit.
    ///
    /// Upstream excludes deposit rows by default, so filtering on this alone
    /// returns nothing. Pair it with
    /// [`ListActivity::exclude_deposits_withdrawals(false)`](crate::api::users::ListActivity::exclude_deposits_withdrawals).
    Deposit,
    /// Collateral withdrawal.
    ///
    /// Upstream excludes withdrawal rows by default, so filtering on this alone
    /// returns nothing. Pair it with
    /// [`ListActivity::exclude_deposits_withdrawals(false)`](crate::api::users::ListActivity::exclude_deposits_withdrawals).
    Withdrawal,
    /// Yield accrual on collateral
    Yield,
    /// Maker rebate activity
    #[serde(rename = "MAKER_REBATE")]
    MakerRebate,
    /// Referral reward activity
    #[serde(rename = "REFERRAL_REWARD")]
    ReferralReward,
    /// Taker rebate activity
    #[serde(rename = "TAKER_REBATE")]
    TakerRebate,
    /// Unrecognized activity type (forward-compat). Never construct this to
    /// send in a request filter; [`super::api::users::ListActivity::activity_type`]
    /// silently drops it since the upstream API has no matching value to filter on.
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trade => write!(f, "TRADE"),
            Self::Split => write!(f, "SPLIT"),
            Self::Merge => write!(f, "MERGE"),
            Self::Redeem => write!(f, "REDEEM"),
            Self::Reward => write!(f, "REWARD"),
            Self::Conversion => write!(f, "CONVERSION"),
            Self::Deposit => write!(f, "DEPOSIT"),
            Self::Withdrawal => write!(f, "WITHDRAWAL"),
            Self::Yield => write!(f, "YIELD"),
            Self::MakerRebate => write!(f, "MAKER_REBATE"),
            Self::ReferralReward => write!(f, "REFERRAL_REWARD"),
            Self::TakerRebate => write!(f, "TAKER_REBATE"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// An ERC20 allowance as reported by `/v1/approvals`.
///
/// Upstream sends a string that is either the sentinel `"max"` or a decimal
/// amount in the token's base units. `ERC1155` entries carry no amount at all,
/// which is represented by `Option::None` on the containing field rather than
/// by a variant here.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Allowance {
    /// The unlimited-allowance sentinel (`"max"`).
    Max,
    /// A concrete allowance, in the token's base units.
    Amount(rust_decimal::Decimal),
    /// A value that is neither `"max"` nor a decimal `rust_decimal` can hold,
    /// preserved verbatim.
    ///
    /// `rust_decimal` tops out near 7.9e28 while a uint256 allowance can reach
    /// 1.2e77, so an unusually large approval lands here instead of failing
    /// deserialization of the entire response.
    Unknown(String),
}

impl<'de> Deserialize<'de> for Allowance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        if raw == "max" {
            return Ok(Allowance::Max);
        }
        Ok(match raw.parse::<rust_decimal::Decimal>() {
            Ok(amount) => Allowance::Amount(amount),
            Err(_) => Allowance::Unknown(raw),
        })
    }
}

/// Sort field options for activity queries
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivitySortBy {
    /// Sort by timestamp (default)
    #[default]
    Timestamp,
    /// Sort by token amount
    Tokens,
    /// Sort by cash amount
    Cash,
}

impl std::fmt::Display for ActivitySortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timestamp => write!(f, "TIMESTAMP"),
            Self::Tokens => write!(f, "TOKENS"),
            Self::Cash => write!(f, "CASH"),
        }
    }
}

/// User activity record
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Proxy wallet address
    pub proxy_wallet: String,
    /// Activity timestamp
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
    /// Condition ID of the market
    pub condition_id: String,
    /// Activity type
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    /// Token quantity
    pub size: f64,
    /// USD value
    pub usdc_size: f64,
    /// On-chain transaction hash
    pub transaction_hash: Option<String>,
    /// Execution price
    pub price: Option<f64>,
    /// Asset identifier (token ID)
    pub asset: Option<String>,
    // Deserialize into String because the API can return an empty string
    /// Trade side (BUY or SELL)
    pub side: Option<String>,
    /// Outcome index (0 or 1 for binary markets)
    pub outcome_index: Option<u32>,
    /// Market title
    pub title: Option<String>,
    /// Market slug
    pub slug: Option<String>,
    /// Market icon URL
    pub icon: Option<String>,
    /// Outcome name (e.g., "Yes", "No")
    pub outcome: Option<String>,
    /// User display name
    pub name: Option<String>,
    /// User pseudonym
    pub pseudonym: Option<String>,
    /// User bio
    pub bio: Option<String>,
    /// User profile image URL
    pub profile_image: Option<String>,
    /// Optimized profile image URL
    pub profile_image_optimized: Option<String>,
}

/// User position in a market
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Position {
    /// Proxy wallet address
    pub proxy_wallet: String,
    /// Asset identifier (token ID)
    pub asset: String,
    /// Condition ID of the market
    pub condition_id: String,
    /// Position size (number of shares)
    pub size: f64,
    /// Average entry price
    pub avg_price: f64,
    /// Initial value of position
    pub initial_value: f64,
    /// Remaining entry basis including attributed BUY fees.
    ///
    /// [`initial_value`](Self::initial_value) and [`avg_price`](Self::avg_price)
    /// keep their fee-**exclusive** semantics, so the fee-exclusive basis is
    /// `gross_initial_value - entry_fees_usdc`. `None` means upstream omitted
    /// the field — treat that as unavailable, not as zero.
    #[serde(default)]
    pub gross_initial_value: Option<f64>,
    /// Attributed BUY-fee component of [`gross_initial_value`](Self::gross_initial_value).
    ///
    /// SELL fees are exit costs and are never included. Upstream returns an
    /// explicit `0` when the component is zero, so `Some(0.0)` (a measured
    /// zero) and `None` (no data) are different answers.
    #[serde(default)]
    pub entry_fees_usdc: Option<f64>,
    /// Current value of position
    pub current_value: f64,
    /// Cash profit and loss
    pub cash_pnl: f64,
    /// Percentage profit and loss
    pub percent_pnl: f64,
    /// Total amount bought
    pub total_bought: f64,
    /// Realized profit and loss
    pub realized_pnl: f64,
    /// Percentage realized P&L
    pub percent_realized_pnl: f64,
    /// Current market price
    pub cur_price: f64,
    /// Whether position is redeemable
    pub redeemable: bool,
    /// Whether position is mergeable
    pub mergeable: bool,
    /// Market title
    pub title: String,
    /// Market slug
    pub slug: String,
    /// Market icon URL
    pub icon: Option<String>,
    /// Event slug
    pub event_slug: Option<String>,
    /// Outcome name (e.g., "Yes", "No")
    pub outcome: String,
    /// Outcome index (0 or 1 for binary markets)
    pub outcome_index: u32,
    /// Opposite outcome name
    pub opposite_outcome: String,
    /// Opposite outcome asset ID
    pub opposite_asset: String,
    /// Market end date
    pub end_date: Option<String>,
    /// Whether this is a negative risk market
    pub negative_risk: bool,
}

/// A per-user position in a single market, as returned by `/v1/market-positions`.
///
/// Field names and types follow the upstream `MarketPositionV1` schema in
/// `docs/specs/data/openapi.yaml`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketPositionV1 {
    /// Proxy wallet address of the position holder
    pub proxy_wallet: String,
    /// Display name of the position holder
    pub name: String,
    /// Profile image URL of the position holder
    pub profile_image: Option<String>,
    /// Whether the holder has a verified badge
    pub verified: bool,
    /// Outcome token asset ID
    pub asset: String,
    /// Condition ID of the market
    pub condition_id: String,
    /// Average entry price
    pub avg_price: f64,
    /// Position size (number of shares)
    pub size: f64,
    /// Current market price (OpenAPI field: `currPrice`)
    #[serde(rename = "currPrice")]
    pub curr_price: f64,
    /// Current value of the position
    pub current_value: f64,
    /// Unrealized cash P&L
    pub cash_pnl: f64,
    /// Total amount bought
    pub total_bought: f64,
    /// Realized P&L
    pub realized_pnl: f64,
    /// Total P&L (cash + realized)
    pub total_pnl: f64,
    /// Outcome name (e.g., "Yes", "No")
    pub outcome: String,
    /// Outcome index (0 or 1 for binary markets)
    pub outcome_index: u32,
}

/// Market positions grouped by outcome token.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaMarketPositionV1 {
    /// Outcome token asset ID
    pub token: String,
    /// Positions for this token
    pub positions: Vec<MarketPositionV1>,
}

/// Status filter for `/v1/market-positions`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum MarketPositionStatus {
    /// Only positions with size > 0.01
    Open,
    /// Only positions with size <= 0.01
    Closed,
    /// All positions regardless of size (default)
    #[default]
    All,
}

impl std::fmt::Display for MarketPositionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "OPEN"),
            Self::Closed => write!(f, "CLOSED"),
            Self::All => write!(f, "ALL"),
        }
    }
}

/// Sort field options for `/v1/market-positions`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketPositionSortBy {
    /// Sort by token count
    Tokens,
    /// Sort by unrealized cash P&L
    CashPnl,
    /// Sort by realized P&L
    RealizedPnl,
    /// Sort by total P&L (cash + realized). Default.
    #[default]
    TotalPnl,
}

impl std::fmt::Display for MarketPositionSortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tokens => write!(f, "TOKENS"),
            Self::CashPnl => write!(f, "CASH_PNL"),
            Self::RealizedPnl => write!(f, "REALIZED_PNL"),
            Self::TotalPnl => write!(f, "TOTAL_PNL"),
        }
    }
}

/// Time period for aggregation
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TimePeriod {
    /// Daily aggregation (default)
    #[default]
    Day,
    /// Weekly aggregation
    Week,
    /// Monthly aggregation
    Month,
    /// All time aggregation
    All,
}

impl std::fmt::Display for TimePeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Day => write!(f, "DAY"),
            Self::Week => write!(f, "WEEK"),
            Self::Month => write!(f, "MONTH"),
            Self::All => write!(f, "ALL"),
        }
    }
}

// ---------------------------------------------------------------------------
// Combinatorial (multi-market) positions and activity
//
// Monetary and share fields on these types are kept as `String` rather than
// `f64` deliberately: upstream documents them as "six-decimal
// precision-preserving" values and instructs clients to parse them as decimals,
// never through a float. Round-tripping them through `f64` would silently lose
// precision on large balances.
// ---------------------------------------------------------------------------

/// Resolution state of a combinatorial position.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComboStatus {
    /// No leg has resolved yet.
    Open,
    /// Some legs have resolved; the combo is still live.
    Partial,
    /// Resolved at a fractional payout (e.g. a leg voided 50/50) — redemption
    /// pays the fractional value per share.
    ResolvedPartial,
    /// Resolved in the holder's favour; shares redeem 1:1 at $1.
    ResolvedWin,
    /// Resolved against the holder; shares are worthless.
    ResolvedLoss,
    /// Unrecognized status (forward-compat). Never construct this to send in a
    /// request filter; [`crate::api::combos::ListComboPositions::status`] drops
    /// it since the upstream API has no matching value to filter on.
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for ComboStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "OPEN"),
            Self::Partial => write!(f, "PARTIAL"),
            Self::ResolvedPartial => write!(f, "RESOLVED_PARTIAL"),
            Self::ResolvedWin => write!(f, "RESOLVED_WIN"),
            Self::ResolvedLoss => write!(f, "RESOLVED_LOSS"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Resolution state of a single leg within a combinatorial position.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComboLegStatus {
    /// The leg's market has not resolved.
    Open,
    /// The leg's market resolved with a fractional payout (e.g. a 50/50 void).
    ResolvedPartial,
    /// The leg resolved in the holder's favour.
    ResolvedWin,
    /// The leg resolved against the holder.
    ResolvedLoss,
    /// Unrecognized leg status (forward-compat).
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for ComboLegStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "OPEN"),
            Self::ResolvedPartial => write!(f, "RESOLVED_PARTIAL"),
            Self::ResolvedWin => write!(f, "RESOLVED_WIN"),
            Self::ResolvedLoss => write!(f, "RESOLVED_LOSS"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Sort order for [`crate::api::combos::ListComboPositions`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ComboSort {
    /// Highest current value first (default).
    #[default]
    CurrentValueDesc,
    /// Highest entry cost first.
    EntryCostDesc,
    /// Most recently entered first.
    FirstEntryDesc,
    /// Most recently resolved first.
    ResolvedAtDesc,
    /// Oldest `updated_at` first — the sort to use for incremental sync
    /// alongside [`crate::api::combos::ListComboPositions::updated_after`].
    UpdatedAsc,
}

impl std::fmt::Display for ComboSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CurrentValueDesc => write!(f, "current_value_desc"),
            Self::EntryCostDesc => write!(f, "entry_cost_desc"),
            Self::FirstEntryDesc => write!(f, "first_entry_desc"),
            Self::ResolvedAtDesc => write!(f, "resolved_at_desc"),
            Self::UpdatedAsc => write!(f, "updated_asc"),
        }
    }
}

/// Standard pagination metadata for combo endpoints.
///
/// There is no total count; `has_more` is derived from page fullness.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Page size used for this response.
    pub limit: i64,
    /// Offset used for this response.
    pub offset: i64,
    /// Whether another page is available.
    pub has_more: bool,
    /// Opaque signed cursor for the next page; `None` when `has_more` is false.
    ///
    /// Pass it back verbatim via `cursor(..)`, keeping the same sort. Never
    /// parse or construct it. Using the cursor makes deep pagination O(page)
    /// and stable against concurrent inserts.
    pub next_cursor: Option<String>,
}

/// Event metadata attached to a combo leg's market.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboEvent {
    /// Event identifier.
    pub event_id: Option<String>,
    /// URL slug for the event.
    pub event_slug: Option<String>,
    /// Human-readable event title.
    pub event_title: Option<String>,
    /// Event image URL.
    pub event_image: Option<String>,
}

/// Market metadata attached to a combo leg.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboMarket {
    /// Market identifier.
    pub market_id: Option<String>,
    /// URL slug for the market.
    pub slug: Option<String>,
    /// Market title.
    pub title: Option<String>,
    /// Outcome label this leg refers to.
    pub outcome: Option<String>,
    /// Market image URL.
    pub image_url: Option<String>,
    /// Market icon URL.
    pub icon_url: Option<String>,
    /// Market category.
    pub category: Option<String>,
    /// Market subcategory.
    pub subcategory: Option<String>,
    /// Tags applied to the market.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Market end date (RFC3339 UTC).
    pub end_date: Option<String>,
    /// Parent event metadata.
    pub event: Option<ComboEvent>,
}

/// A single leg of a combinatorial position.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboLeg {
    /// Zero-based index of this leg within the combo.
    pub leg_index: i64,
    /// Position identifier for the leg.
    pub leg_position_id: Option<String>,
    /// The leg market's condition ID (distinct from the combo's).
    pub leg_condition_id: Option<String>,
    /// Index of the selected outcome within the leg market.
    pub leg_outcome_index: Option<i64>,
    /// Label of the selected outcome.
    pub leg_outcome_label: Option<String>,
    /// Live per-leg resolution state, derived from the leg market's on-chain
    /// payout vector.
    pub leg_status: Option<ComboLegStatus>,
    /// RFC3339 UTC. Set once the leg's market resolves on-chain, including
    /// fractional resolutions that still report `leg_status` `Open`.
    pub leg_resolved_at: Option<String>,
    /// Live price for the leg outcome (decimal string, 0–1). `"0"` when no
    /// price is available.
    pub leg_current_price: Option<String>,
    /// Market metadata for the leg.
    pub market: Option<ComboMarket>,
}

/// A combinatorial (multi-market) position held by a user.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboPosition {
    /// Combo condition ID (`0x` + 62 hex). Equals the `conditionId` of
    /// `isCombo` rows on `/activity`.
    pub combo_condition_id: String,
    /// Position identifier for the combo.
    pub combo_position_id: Option<String>,
    /// Module identifier; `3` is the Combinatorial module.
    pub module_id: Option<i64>,
    /// Holder's wallet address.
    pub user_address: Option<String>,
    /// Share balance as a precision-preserving decimal string.
    pub shares_balance: Option<String>,
    /// Average entry price in USDC, as a decimal string.
    pub entry_avg_price_usdc: Option<String>,
    /// *Remaining* cost basis (`entry_avg_price × shares_balance`).
    ///
    /// Reads ~0 after a winning combo is redeemed — use
    /// [`total_cost_usdc`](Self::total_cost_usdc) to display what was paid on
    /// closed positions.
    pub entry_cost_usdc: Option<String>,
    /// Gross redemption proceeds (winning shares redeem 1:1 at $1).
    ///
    /// `"0.00"` while open, unredeemed, or resolved-loss; accumulates under
    /// `Partial`. This is gross payout, not net PnL — net =
    /// `realized_payout_usdc − total_cost_usdc`.
    pub realized_payout_usdc: Option<String>,
    /// Original cost basis, surviving redemption burning the shares. Equals
    /// [`entry_cost_usdc`](Self::entry_cost_usdc) while open.
    pub total_cost_usdc: Option<String>,
    /// Exact gross entry basis including attributed BUY fees, as a six-decimal
    /// precision-preserving string (e.g. `"8999.997488"`).
    ///
    /// Tracks the remaining basis while the position is live and freezes once
    /// it is terminal. Exact net basis = `gross_entry_cost_usdc −
    /// entry_fees_usdc`. Parse as a decimal, never through a float.
    pub gross_entry_cost_usdc: Option<String>,
    /// BUY-fee portion of the same basis, as a six-decimal precision-preserving
    /// string. SELL fees are excluded; always ≤ `gross_entry_cost_usdc`.
    pub entry_fees_usdc: Option<String>,
    /// Resolution state of the combo.
    pub status: Option<ComboStatus>,
    /// First entry time (RFC3339 UTC).
    pub first_entry_at: Option<String>,
    /// Resolution time (RFC3339 UTC), or `None` while unresolved.
    pub resolved_at: Option<String>,
    /// Last-modified time (UTC, ISO 8601) — the incremental-sync watermark.
    ///
    /// Bumps on any recompute of the row (trade, redemption, resolution
    /// classification). Omitted on responses served by the legacy backend.
    pub updated_at: Option<String>,
    /// Total number of legs.
    pub legs_total: Option<i64>,
    /// Number of legs that have resolved.
    pub legs_resolved: Option<i64>,
    /// Number of legs still pending.
    pub legs_pending: Option<i64>,
    /// Per-leg breakdown.
    #[serde(default)]
    pub legs: Vec<ComboLeg>,
}

/// Response envelope for `GET /v1/positions/combos`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombosResponse {
    /// The combo positions in this page.
    #[serde(default)]
    pub combos: Vec<ComboPosition>,
    /// Pagination metadata.
    pub pagination: Option<Pagination>,
}

/// A combo lifecycle or redeem event.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboActivity {
    /// Event identifier.
    pub id: Option<String>,
    /// Event type (split, merge, convert, compress, wrap, unwrap, redeem).
    ///
    /// Upstream documents `type` as the replacement for the deprecated
    /// [`event_kind`](Self::event_kind) and [`side`](Self::side) fields, but
    /// does not yet list it in the published schema — treat it as optional
    /// until it appears there.
    #[serde(rename = "type")]
    pub activity_type: Option<String>,
    /// Raw on-chain event name (e.g. `PositionsSplit`).
    #[deprecated(note = "upstream deprecated this field; use `activity_type` instead")]
    pub event_kind: Option<String>,
    /// Normalized rendering label (e.g. `Split`).
    #[deprecated(note = "upstream deprecated this field; use `activity_type` instead")]
    pub side: Option<String>,
    /// Module kind; always `Combinatorial`.
    pub module_kind: Option<String>,
    /// Holder's wallet address.
    pub user_address: Option<String>,
    /// Combo condition ID (`0x` + 62 hex).
    pub combo_condition_id: Option<String>,
    /// Position identifier for the combo.
    pub combo_position_id: Option<String>,
    /// Module identifier.
    pub module_id: Option<i64>,
    /// Lifecycle amount in USDC; `None` on redeems.
    pub amount_usdc: Option<f64>,
    /// Redeem payout in USDC; `None` on lifecycle events.
    pub payout_usdc: Option<f64>,
    /// Event time as a Unix timestamp (seconds).
    pub timestamp: Option<i64>,
    /// Transaction time (RFC3339 UTC).
    pub tx_dttm: Option<String>,
    /// Transaction hash.
    pub tx_hash: Option<String>,
    /// Log index within the transaction.
    pub log_index: Option<i64>,
    /// Block number.
    pub block_number: Option<i64>,
    /// Per-leg breakdown.
    #[serde(default)]
    pub legs: Vec<ComboLeg>,
}

/// Response envelope for `GET /v1/activity/combos`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombosActivityResponse {
    /// The combo activity rows in this page.
    #[serde(default)]
    pub activity: Vec<ComboActivity>,
    /// Pagination metadata.
    pub pagination: Option<Pagination>,
}

// ---------------------------------------------------------------------------
// Undocumented sibling hosts (user-pnl-api, lb-api)
//
// Neither host appears in any published Polymarket OpenAPI spec. The shapes
// below were derived from live responses; the enum variants come from the
// APIs' own validation errors, which enumerate the accepted values.
// ---------------------------------------------------------------------------

/// Sampling resolution for a PnL series.
///
/// Upstream rejects anything outside this set with
/// `"the 'fidelity' value is unkonwn. Known values: '1d', '18h', '12h', '3h', '1h'"`
/// (typo theirs).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PnlFidelity {
    /// One point per day (default).
    #[default]
    #[serde(rename = "1d")]
    OneDay,
    /// One point per 18 hours.
    #[serde(rename = "18h")]
    EighteenHours,
    /// One point per 12 hours.
    #[serde(rename = "12h")]
    TwelveHours,
    /// One point per 3 hours.
    #[serde(rename = "3h")]
    ThreeHours,
    /// One point per hour.
    #[serde(rename = "1h")]
    OneHour,
}

impl std::fmt::Display for PnlFidelity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OneDay => write!(f, "1d"),
            Self::EighteenHours => write!(f, "18h"),
            Self::TwelveHours => write!(f, "12h"),
            Self::ThreeHours => write!(f, "3h"),
            Self::OneHour => write!(f, "1h"),
        }
    }
}

/// A single point on a user's PnL curve.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlPoint {
    /// Unix timestamp in seconds.
    #[serde(rename = "t")]
    pub timestamp: i64,
    /// PnL in USDC at that timestamp. Negative values are losses.
    #[serde(rename = "p")]
    pub pnl: f64,
}

/// Ranking window for the rankings host.
///
/// Upstream rejects anything else with `{"error": "invalid request"}`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RankingWindow {
    /// All-time (default).
    #[default]
    #[serde(rename = "all")]
    All,
    /// Trailing day.
    #[serde(rename = "1d")]
    OneDay,
    /// Trailing week.
    #[serde(rename = "7d")]
    SevenDays,
    /// Trailing 30 days.
    #[serde(rename = "30d")]
    ThirtyDays,
}

impl std::fmt::Display for RankingWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::OneDay => write!(f, "1d"),
            Self::SevenDays => write!(f, "7d"),
            Self::ThirtyDays => write!(f, "30d"),
        }
    }
}

/// One entry in a volume or profit ranking.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingEntry {
    /// Proxy wallet address of the ranked trader.
    pub proxy_wallet: Option<String>,
    /// Ranked amount in USDC — traded volume or realized profit, depending on
    /// which endpoint produced the entry.
    pub amount: Option<f64>,
    /// Display name.
    pub name: Option<String>,
    /// Pseudonym, which falls back to an address-derived string.
    pub pseudonym: Option<String>,
    /// Profile biography.
    pub bio: Option<String>,
    /// Profile image URL.
    pub profile_image: Option<String>,
    /// Optimized profile image URL.
    pub profile_image_optimized: Option<String>,
}

/// "Other" outcome size held by a user in an augmented neg-risk event.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherSize {
    /// Gamma event ID of the augmented neg-risk event.
    pub id: Option<i64>,
    /// User wallet address.
    pub user: Option<String>,
    /// Size of the "Other" position.
    pub size: Option<f64>,
}

/// A single moderated revision of a question.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionEntry {
    /// Revised question text.
    pub revision: Option<String>,
    /// Revision time as a Unix timestamp (seconds).
    pub timestamp: Option<i64>,
}

/// Moderated revisions for a question.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionPayload {
    /// Question ID (`0x` + 64 hex).
    #[serde(rename = "questionID")]
    pub question_id: Option<String>,
    /// Revisions recorded for the question, oldest first.
    #[serde(default)]
    pub revisions: Vec<RevisionEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify Display matches serde serialization for all PositionSortBy variants.
    #[test]
    fn position_sort_by_display_matches_serde() {
        let variants = [
            PositionSortBy::Current,
            PositionSortBy::Initial,
            PositionSortBy::Tokens,
            PositionSortBy::CashPnl,
            PositionSortBy::PercentPnl,
            PositionSortBy::Title,
            PositionSortBy::Resolving,
            PositionSortBy::Price,
            PositionSortBy::AvgPrice,
        ];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    /// Verify Display matches serde serialization for all ClosedPositionSortBy variants.
    #[test]
    fn closed_position_sort_by_display_matches_serde() {
        let variants = [
            ClosedPositionSortBy::RealizedPnl,
            ClosedPositionSortBy::Title,
            ClosedPositionSortBy::Price,
            ClosedPositionSortBy::AvgPrice,
            ClosedPositionSortBy::Timestamp,
        ];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn activity_sort_by_display_matches_serde() {
        let variants = [
            ActivitySortBy::Timestamp,
            ActivitySortBy::Tokens,
            ActivitySortBy::Cash,
        ];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn sort_direction_display_matches_serde() {
        let variants = [SortDirection::Asc, SortDirection::Desc];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn trade_side_display_matches_serde() {
        let variants = [TradeSide::Buy, TradeSide::Sell, TradeSide::Unknown];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn trade_side_falls_back_to_unknown_for_future_values() {
        let result: TradeSide = serde_json::from_str("\"SOME_FUTURE_SIDE\"").unwrap();
        assert_eq!(result, TradeSide::Unknown);
    }

    #[test]
    fn trade_filter_type_display_matches_serde() {
        let variants = [TradeFilterType::Cash, TradeFilterType::Tokens];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn activity_type_display_matches_serde() {
        let variants = [
            ActivityType::Trade,
            ActivityType::Split,
            ActivityType::Merge,
            ActivityType::Redeem,
            ActivityType::Reward,
            ActivityType::Conversion,
            ActivityType::MakerRebate,
            ActivityType::ReferralReward,
            ActivityType::TakerRebate,
            ActivityType::Unknown,
        ];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn activity_type_roundtrip_serde() {
        for variant in [
            ActivityType::Trade,
            ActivityType::Split,
            ActivityType::Merge,
            ActivityType::Redeem,
            ActivityType::Reward,
            ActivityType::Conversion,
            ActivityType::MakerRebate,
            ActivityType::ReferralReward,
            ActivityType::TakerRebate,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: ActivityType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn activity_type_falls_back_to_unknown_for_future_types() {
        // A hypothetical future activity type Polymarket hasn't documented yet.
        let result: ActivityType = serde_json::from_str("\"SOME_FUTURE_TYPE\"").unwrap();
        assert_eq!(result, ActivityType::Unknown);

        // Case mismatches also fall back rather than poisoning the whole page.
        let result: ActivityType = serde_json::from_str("\"trade\"").unwrap();
        assert_eq!(result, ActivityType::Unknown);
    }

    #[test]
    fn sort_direction_default_is_desc() {
        assert_eq!(SortDirection::default(), SortDirection::Desc);
    }

    #[test]
    fn closed_position_sort_by_default_is_realized_pnl() {
        assert_eq!(
            ClosedPositionSortBy::default(),
            ClosedPositionSortBy::RealizedPnl
        );
    }

    #[test]
    fn activity_sort_by_default_is_timestamp() {
        assert_eq!(ActivitySortBy::default(), ActivitySortBy::Timestamp);
    }

    #[test]
    fn position_sort_by_serde_roundtrip() {
        for variant in [
            PositionSortBy::Current,
            PositionSortBy::Initial,
            PositionSortBy::Tokens,
            PositionSortBy::CashPnl,
            PositionSortBy::PercentPnl,
            PositionSortBy::Title,
            PositionSortBy::Resolving,
            PositionSortBy::Price,
            PositionSortBy::AvgPrice,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: PositionSortBy = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn deserialize_position_from_json() {
        let json = r#"{
            "proxyWallet": "0xabc123",
            "asset": "token123",
            "conditionId": "cond456",
            "size": 100.5,
            "avgPrice": 0.65,
            "initialValue": 65.0,
            "currentValue": 70.0,
            "cashPnl": 5.0,
            "percentPnl": 7.69,
            "totalBought": 100.5,
            "realizedPnl": 2.0,
            "percentRealizedPnl": 3.08,
            "curPrice": 0.70,
            "redeemable": false,
            "mergeable": true,
            "title": "Will X happen?",
            "slug": "will-x-happen",
            "icon": "https://example.com/icon.png",
            "eventSlug": "x-event",
            "outcome": "Yes",
            "outcomeIndex": 0,
            "oppositeOutcome": "No",
            "oppositeAsset": "token789",
            "endDate": "2025-12-31",
            "negativeRisk": false
        }"#;

        let pos: Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.proxy_wallet, "0xabc123");
        assert_eq!(pos.asset, "token123");
        assert_eq!(pos.condition_id, "cond456");
        assert!((pos.size - 100.5).abs() < f64::EPSILON);
        assert!((pos.avg_price - 0.65).abs() < f64::EPSILON);
        assert!((pos.initial_value - 65.0).abs() < f64::EPSILON);
        assert!((pos.current_value - 70.0).abs() < f64::EPSILON);
        assert!((pos.cash_pnl - 5.0).abs() < f64::EPSILON);
        assert!(!pos.redeemable);
        assert!(pos.mergeable);
        assert_eq!(pos.title, "Will X happen?");
        assert_eq!(pos.outcome, "Yes");
        assert_eq!(pos.outcome_index, 0);
        assert_eq!(pos.opposite_outcome, "No");
        assert!(!pos.negative_risk);
        assert_eq!(pos.icon, Some("https://example.com/icon.png".to_string()));
        assert_eq!(pos.event_slug, Some("x-event".to_string()));
    }

    #[test]
    fn deserialize_position_with_null_optionals() {
        let json = r#"{
            "proxyWallet": "0xabc123",
            "asset": "token123",
            "conditionId": "cond456",
            "size": 0.0,
            "avgPrice": 0.0,
            "initialValue": 0.0,
            "currentValue": 0.0,
            "cashPnl": 0.0,
            "percentPnl": 0.0,
            "totalBought": 0.0,
            "realizedPnl": 0.0,
            "percentRealizedPnl": 0.0,
            "curPrice": 0.0,
            "redeemable": false,
            "mergeable": false,
            "title": "Test",
            "slug": "test",
            "icon": null,
            "eventSlug": null,
            "outcome": "No",
            "outcomeIndex": 1,
            "oppositeOutcome": "Yes",
            "oppositeAsset": "token000",
            "endDate": null,
            "negativeRisk": true
        }"#;

        let pos: Position = serde_json::from_str(json).unwrap();
        assert!(pos.icon.is_none());
        assert!(pos.event_slug.is_none());
        assert!(pos.end_date.is_none());
        assert!(pos.negative_risk);
    }

    #[test]
    fn deserialize_closed_position_from_json() {
        let json = r#"{
            "proxyWallet": "0xdef456",
            "asset": "token_closed",
            "conditionId": "cond_closed",
            "avgPrice": 0.45,
            "totalBought": 200.0,
            "realizedPnl": -10.0,
            "curPrice": 0.35,
            "timestamp": 1700000000,
            "title": "Closed market?",
            "slug": "closed-market",
            "icon": null,
            "eventSlug": "closed-event",
            "outcome": "No",
            "outcomeIndex": 1,
            "oppositeOutcome": "Yes",
            "oppositeAsset": "token_opp",
            "endDate": "2024-06-30"
        }"#;

        let closed: ClosedPosition = serde_json::from_str(json).unwrap();
        assert_eq!(closed.proxy_wallet, "0xdef456");
        assert!((closed.avg_price - 0.45).abs() < f64::EPSILON);
        assert!((closed.realized_pnl - (-10.0)).abs() < f64::EPSILON);
        assert_eq!(closed.timestamp, 1700000000);
        assert_eq!(closed.outcome, "No");
        assert_eq!(closed.outcome_index, 1);
        assert!(closed.icon.is_none());
        assert_eq!(closed.event_slug, Some("closed-event".to_string()));
    }

    #[test]
    fn deserialize_trade_from_json() {
        let json = r#"{
            "proxyWallet": "0x1234",
            "side": "BUY",
            "asset": "token_buy",
            "conditionId": "cond_trade",
            "size": 50.0,
            "price": 0.72,
            "timestamp": 1700001000,
            "title": "Trade market?",
            "slug": "trade-market",
            "icon": "https://example.com/trade.png",
            "eventSlug": null,
            "outcome": "Yes",
            "outcomeIndex": 0,
            "name": "TraderOne",
            "pseudonym": "t1",
            "bio": "A trader",
            "profileImage": null,
            "profileImageOptimized": null,
            "transactionHash": "0xhash123"
        }"#;

        let trade: Trade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.proxy_wallet, "0x1234");
        assert_eq!(trade.side, TradeSide::Buy);
        assert!((trade.size - 50.0).abs() < f64::EPSILON);
        assert!((trade.price - 0.72).abs() < f64::EPSILON);
        assert_eq!(trade.timestamp, 1700001000);
        assert_eq!(trade.name, Some("TraderOne".to_string()));
        assert_eq!(trade.transaction_hash, Some("0xhash123".to_string()));
        assert!(trade.profile_image.is_none());
    }

    #[test]
    fn deserialize_trade_sell_side() {
        let json = r#"{
            "proxyWallet": "0x5678",
            "side": "SELL",
            "asset": "token_sell",
            "conditionId": "cond_sell",
            "size": 25.0,
            "price": 0.30,
            "timestamp": 1700002000,
            "title": "Sell test",
            "slug": "sell-test",
            "icon": null,
            "eventSlug": null,
            "outcome": "No",
            "outcomeIndex": 1,
            "name": null,
            "pseudonym": null,
            "bio": null,
            "profileImage": null,
            "profileImageOptimized": null,
            "transactionHash": null
        }"#;

        let trade: Trade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.side, TradeSide::Sell);
        assert!(trade.name.is_none());
        assert!(trade.transaction_hash.is_none());
    }

    #[test]
    fn deserialize_activity_from_json() {
        let json = r#"{
            "proxyWallet": "0xact123",
            "timestamp": 1700003000,
            "conditionId": "cond_act",
            "type": "TRADE",
            "size": 10.0,
            "usdcSize": 7.50,
            "transactionHash": "0xacthash",
            "price": 0.75,
            "asset": "token_act",
            "side": "BUY",
            "outcomeIndex": 0,
            "title": "Activity market",
            "slug": "activity-market",
            "icon": null,
            "outcome": "Yes",
            "name": null,
            "pseudonym": null,
            "bio": null,
            "profileImage": null,
            "profileImageOptimized": null
        }"#;

        let activity: Activity = serde_json::from_str(json).unwrap();
        assert_eq!(activity.proxy_wallet, "0xact123");
        assert_eq!(activity.activity_type, ActivityType::Trade);
        assert!((activity.size - 10.0).abs() < f64::EPSILON);
        assert!((activity.usdc_size - 7.50).abs() < f64::EPSILON);
        assert_eq!(activity.side, Some("BUY".to_string()));
        assert_eq!(activity.outcome_index, Some(0));
    }

    #[test]
    fn deserialize_activity_merge_type() {
        let json = r#"{
            "proxyWallet": "0xmerge",
            "timestamp": 1700004000,
            "conditionId": "cond_merge",
            "type": "MERGE",
            "size": 5.0,
            "usdcSize": 3.0,
            "transactionHash": null,
            "price": null,
            "asset": null,
            "side": "",
            "outcomeIndex": null,
            "title": null,
            "slug": null,
            "icon": null,
            "outcome": null,
            "name": null,
            "pseudonym": null,
            "bio": null,
            "profileImage": null,
            "profileImageOptimized": null
        }"#;

        let activity: Activity = serde_json::from_str(json).unwrap();
        assert_eq!(activity.activity_type, ActivityType::Merge);
        // Side is an empty string from the API, stored as Some("")
        assert_eq!(activity.side, Some("".to_string()));
        assert!(activity.price.is_none());
        assert!(activity.asset.is_none());
        assert!(activity.title.is_none());
    }

    #[test]
    fn deserialize_user_value() {
        let json = r#"{"user": "0xuser", "value": 1234.56}"#;
        let uv: UserValue = serde_json::from_str(json).unwrap();
        assert_eq!(uv.user, "0xuser");
        assert!((uv.value - 1234.56).abs() < f64::EPSILON);
    }

    #[test]
    fn deserialize_open_interest() {
        let json = r#"{"market": "0xcond", "value": 50000.0}"#;
        let oi: OpenInterest = serde_json::from_str(json).unwrap();
        assert_eq!(oi.market, "0xcond");
        assert!((oi.value - 50000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn market_position_status_display_matches_serde() {
        for variant in [
            MarketPositionStatus::Open,
            MarketPositionStatus::Closed,
            MarketPositionStatus::All,
        ] {
            let serialized = serde_json::to_value(variant).unwrap();
            assert_eq!(format!("\"{}\"", variant), serialized.to_string());
        }
    }

    #[test]
    fn market_position_status_default_is_all() {
        assert_eq!(MarketPositionStatus::default(), MarketPositionStatus::All);
    }

    #[test]
    fn market_position_sort_by_display_matches_serde() {
        for variant in [
            MarketPositionSortBy::Tokens,
            MarketPositionSortBy::CashPnl,
            MarketPositionSortBy::RealizedPnl,
            MarketPositionSortBy::TotalPnl,
        ] {
            let serialized = serde_json::to_value(variant).unwrap();
            assert_eq!(format!("\"{}\"", variant), serialized.to_string());
        }
    }

    #[test]
    fn market_position_sort_by_default_is_total_pnl() {
        assert_eq!(
            MarketPositionSortBy::default(),
            MarketPositionSortBy::TotalPnl
        );
    }

    #[test]
    fn deserialize_market_position_v1() {
        // Field names lifted from `MarketPositionV1` in docs/specs/data/openapi.yaml.
        let json = r#"{
            "proxyWallet": "0xabc",
            "name": "Alice",
            "profileImage": "https://example.com/a.png",
            "verified": true,
            "asset": "token_a",
            "conditionId": "cond_mp",
            "avgPrice": 0.42,
            "size": 1234.5,
            "currPrice": 0.51,
            "currentValue": 629.60,
            "cashPnl": 110.0,
            "totalBought": 520.0,
            "realizedPnl": 15.5,
            "totalPnl": 125.5,
            "outcome": "Yes",
            "outcomeIndex": 0
        }"#;

        let pos: MarketPositionV1 = serde_json::from_str(json).unwrap();
        assert_eq!(pos.proxy_wallet, "0xabc");
        assert_eq!(pos.name, "Alice");
        assert_eq!(
            pos.profile_image.as_deref(),
            Some("https://example.com/a.png")
        );
        assert!(pos.verified);
        assert_eq!(pos.asset, "token_a");
        assert_eq!(pos.condition_id, "cond_mp");
        assert!((pos.avg_price - 0.42).abs() < f64::EPSILON);
        assert!((pos.size - 1234.5).abs() < f64::EPSILON);
        assert!((pos.curr_price - 0.51).abs() < f64::EPSILON);
        assert!((pos.current_value - 629.60).abs() < f64::EPSILON);
        assert!((pos.cash_pnl - 110.0).abs() < f64::EPSILON);
        assert!((pos.total_bought - 520.0).abs() < f64::EPSILON);
        assert!((pos.realized_pnl - 15.5).abs() < f64::EPSILON);
        assert!((pos.total_pnl - 125.5).abs() < f64::EPSILON);
        assert_eq!(pos.outcome, "Yes");
        assert_eq!(pos.outcome_index, 0);
    }

    #[test]
    fn market_position_v1_roundtrip() {
        let original = MarketPositionV1 {
            proxy_wallet: "0xabc".into(),
            name: "Alice".into(),
            profile_image: None,
            verified: false,
            asset: "token_a".into(),
            condition_id: "cond_mp".into(),
            avg_price: 0.5,
            size: 10.0,
            curr_price: 0.6,
            current_value: 6.0,
            cash_pnl: 1.0,
            total_bought: 5.0,
            realized_pnl: 0.0,
            total_pnl: 1.0,
            outcome: "No".into(),
            outcome_index: 1,
        };
        let json = serde_json::to_string(&original).unwrap();
        // Ensure currPrice is used over snake_case in the wire format.
        assert!(json.contains("\"currPrice\""));
        let back: MarketPositionV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.proxy_wallet, original.proxy_wallet);
        assert_eq!(back.outcome_index, original.outcome_index);
        assert!((back.curr_price - original.curr_price).abs() < f64::EPSILON);
    }

    #[test]
    fn deserialize_meta_market_position_v1() {
        let json = r#"{
            "token": "token_a",
            "positions": [
                {
                    "proxyWallet": "0xabc",
                    "name": "Alice",
                    "profileImage": null,
                    "verified": false,
                    "asset": "token_a",
                    "conditionId": "cond_mp",
                    "avgPrice": 0.42,
                    "size": 100.0,
                    "currPrice": 0.51,
                    "currentValue": 51.0,
                    "cashPnl": 9.0,
                    "totalBought": 42.0,
                    "realizedPnl": 0.0,
                    "totalPnl": 9.0,
                    "outcome": "Yes",
                    "outcomeIndex": 0
                }
            ]
        }"#;

        let meta: MetaMarketPositionV1 = serde_json::from_str(json).unwrap();
        assert_eq!(meta.token, "token_a");
        assert_eq!(meta.positions.len(), 1);
        assert_eq!(meta.positions[0].name, "Alice");
        assert!(meta.positions[0].profile_image.is_none());
    }

    #[test]
    fn deserialize_position_fee_basis() {
        // `entryFeesUsdc: 0` is a *measured* zero and must not collapse to None:
        // upstream returns an explicit 0 when the fee component is zero, and
        // omits the field entirely when the data is unavailable.
        let json = r#"{
            "proxyWallet": "0xabc123",
            "asset": "token123",
            "conditionId": "cond456",
            "size": 100.5,
            "avgPrice": 0.65,
            "initialValue": 65.0,
            "grossInitialValue": 65.5,
            "entryFeesUsdc": 0,
            "currentValue": 70.0,
            "cashPnl": 5.0,
            "percentPnl": 7.69,
            "totalBought": 100.5,
            "realizedPnl": 2.0,
            "percentRealizedPnl": 3.08,
            "curPrice": 0.70,
            "redeemable": false,
            "mergeable": true,
            "title": "Will X happen?",
            "slug": "will-x-happen",
            "outcome": "Yes",
            "outcomeIndex": 0,
            "oppositeOutcome": "No",
            "oppositeAsset": "token789",
            "negativeRisk": false
        }"#;

        let pos: Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.gross_initial_value, Some(65.5));
        assert_eq!(pos.entry_fees_usdc, Some(0.0));
        // initialValue keeps fee-exclusive semantics, so it is NOT the gross figure.
        assert!((pos.initial_value - 65.0).abs() < f64::EPSILON);
    }

    #[test]
    fn deserialize_position_without_fee_basis_is_none() {
        // Older payloads omit both fields; None means "unavailable", not zero.
        let json = r#"{
            "proxyWallet": "0xabc123",
            "asset": "token123",
            "conditionId": "cond456",
            "size": 100.5,
            "avgPrice": 0.65,
            "initialValue": 65.0,
            "currentValue": 70.0,
            "cashPnl": 5.0,
            "percentPnl": 7.69,
            "totalBought": 100.5,
            "realizedPnl": 2.0,
            "percentRealizedPnl": 3.08,
            "curPrice": 0.70,
            "redeemable": false,
            "mergeable": true,
            "title": "Will X happen?",
            "slug": "will-x-happen",
            "outcome": "Yes",
            "outcomeIndex": 0,
            "oppositeOutcome": "No",
            "oppositeAsset": "token789",
            "negativeRisk": false
        }"#;

        let pos: Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.gross_initial_value, None);
        assert_eq!(pos.entry_fees_usdc, None);
    }

    #[test]
    fn deserialize_allowance_max_sentinel() {
        let v: Allowance = serde_json::from_str(r#""max""#).unwrap();
        assert_eq!(v, Allowance::Max);
    }

    #[test]
    fn deserialize_allowance_decimal_amount() {
        let v: Allowance = serde_json::from_str(r#""1000000""#).unwrap();
        assert_eq!(v, Allowance::Amount(rust_decimal::Decimal::new(1_000_000, 0)));
    }

    #[test]
    fn deserialize_allowance_beyond_decimal_range_is_unknown() {
        // rust_decimal tops out near 7.9e28; a uint256 allowance can reach
        // 1.2e77. Without the Unknown arm this would fail the whole response.
        let huge = "1".repeat(40);
        let json = format!(r#""{huge}""#);
        let v: Allowance = serde_json::from_str(&json).unwrap();
        assert_eq!(v, Allowance::Unknown(huge));
    }

    #[test]
    fn deserialize_allowance_unrecognized_sentinel_is_unknown() {
        let v: Allowance = serde_json::from_str(r#""unlimited""#).unwrap();
        assert_eq!(v, Allowance::Unknown("unlimited".to_string()));
    }
}
