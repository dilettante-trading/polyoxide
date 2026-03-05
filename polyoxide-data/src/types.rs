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
    #[serde(default)]
    pub icon: Option<String>,
    /// Event slug
    #[serde(default)]
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
    #[serde(default)]
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
}

impl std::fmt::Display for TradeSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
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
    #[serde(default)]
    pub icon: Option<String>,
    /// Event slug
    #[serde(default)]
    pub event_slug: Option<String>,
    /// Outcome name (e.g., "Yes", "No")
    pub outcome: String,
    /// Outcome index (0 or 1 for binary markets)
    pub outcome_index: u32,
    /// User display name
    #[serde(default)]
    pub name: Option<String>,
    /// User pseudonym
    #[serde(default)]
    pub pseudonym: Option<String>,
    /// User bio
    #[serde(default)]
    pub bio: Option<String>,
    /// User profile image URL
    #[serde(default)]
    pub profile_image: Option<String>,
    /// Optimized profile image URL
    #[serde(default)]
    pub profile_image_optimized: Option<String>,
    /// Transaction hash
    #[serde(default)]
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
        }
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
    #[serde(default)]
    pub transaction_hash: Option<String>,
    /// Execution price
    #[serde(default)]
    pub price: Option<f64>,
    /// Asset identifier (token ID)
    #[serde(default)]
    pub asset: Option<String>,
    // ! Deserialize into String because the API can return an empty string
    /// Trade side (BUY or SELL)
    #[serde(default)]
    pub side: Option<String>,
    /// Outcome index (0 or 1 for binary markets)
    #[serde(default)]
    pub outcome_index: Option<u32>,
    /// Market title
    #[serde(default)]
    pub title: Option<String>,
    /// Market slug
    #[serde(default)]
    pub slug: Option<String>,
    /// Market icon URL
    #[serde(default)]
    pub icon: Option<String>,
    /// Outcome name (e.g., "Yes", "No")
    #[serde(default)]
    pub outcome: Option<String>,
    /// User display name
    #[serde(default)]
    pub name: Option<String>,
    /// User pseudonym
    #[serde(default)]
    pub pseudonym: Option<String>,
    /// User bio
    #[serde(default)]
    pub bio: Option<String>,
    /// User profile image URL
    #[serde(default)]
    pub profile_image: Option<String>,
    /// Optimized profile image URL
    #[serde(default)]
    pub profile_image_optimized: Option<String>,
}

/// User position in a market
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    #[serde(default)]
    pub icon: Option<String>,
    /// Event slug
    #[serde(default)]
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
    #[serde(default)]
    pub end_date: Option<String>,
    /// Whether this is a negative risk market
    pub negative_risk: bool,
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
        let variants = [TradeSide::Buy, TradeSide::Sell];
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
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: ActivityType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn activity_type_rejects_unknown_variant() {
        let result = serde_json::from_str::<ActivityType>("\"UNKNOWN\"");
        assert!(result.is_err(), "should reject unknown activity type");
    }

    #[test]
    fn activity_type_rejects_lowercase() {
        let result = serde_json::from_str::<ActivityType>("\"trade\"");
        assert!(result.is_err(), "should reject lowercase activity type");
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
}
