//! Types for the `feeds` routes: trades, activity and combo activity.

use serde::{Deserialize, Serialize};

use super::{ActivitySide, ActivityType, ComboLeg, OutcomeIndex, TradeSide};

/// One activity-feed event (`/v2/activity`); a trade, split, merge, redeem, …
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Activity {
    /// Profile bio text.
    pub bio: String,
    /// On-chain condition id of the market (`0x` hex).
    pub condition_id: String,
    /// Parent event slug.
    pub event_slug: String,
    /// Market icon URL.
    pub icon: String,
    /// Flag only, on V2/V3 combo trade rows. Combo detail lives on the combos
    /// endpoints; omitted from non-combo rows.
    pub is_combo: Option<bool>,
    /// Profile display name of the wallet.
    pub name: String,
    /// Label of the outcome (e.g. `Yes`).
    pub outcome: String,
    /// Index of the outcome within the market; `999` means the outcome
    /// could not be labeled.
    pub outcome_index: OutcomeIndex,
    /// Price per share in USDC (trades; `0` where no price applies).
    pub price: f64,
    /// Profile image URL.
    pub profile_image: String,
    /// Resized profile image URL, when one exists.
    pub profile_image_optimized: String,
    /// Proxy wallet the row belongs to; the address every wallet-keyed
    /// endpoint accepts as `user`.
    pub proxy_wallet: String,
    /// Generated fallback handle for profiles without a display name.
    pub pseudonym: String,
    /// `BUY` or `SELL` on trade rows, from this wallet's perspective; empty
    /// where a side does not apply.
    pub side: ActivitySide,
    /// Share quantity of the action; bare sizes are shares, never USD.
    pub size: f64,
    /// Market slug; the URL segment on polymarket.com.
    pub slug: String,
    /// Block timestamp of the action, epoch seconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
    /// Market question title (Gamma enrichment; empty when unenriched).
    pub title: String,
    /// CLOB asset id of the outcome token the action touched.
    pub token_id: String,
    /// Hash of the settling transaction.
    pub transaction_hash: String,
    /// TRADE, SPLIT, MERGE, REDEEM, REWARD, CONVERSION, …
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    /// Cash value of the action in USDC.
    pub usdc_size: f64,
}

/// One combo lifecycle/redemption event (`/v2/activity/combos`).
///
/// Ordered and paginated by on-chain position; `(block_number, log_index)`.
/// `timestamp` is the event's wall-clock, not the ordering key.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComboActivity {
    /// Cash amount of the action in USDC; `null` where no cash leg applies.
    pub amount_usdc: Option<f64>,
    /// Block number of the action.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub block_number: i64,
    /// On-chain combo condition id (structural, `0x03`-prefixed).
    pub combo_condition_id: String,
    /// Token id of the combo position the action touched.
    pub combo_position_id: String,
    /// `tx_hash-log_index`.
    pub id: String,
    /// The combo's legs, in leg order, with market and event enrichment.
    pub legs: Vec<ComboLeg>,
    /// Redemption payout in USDC on REDEEM rows; `null` otherwise.
    pub payout_usdc: Option<f64>,
    /// Proxy wallet the action belongs to.
    pub proxy_wallet: String,
    /// Event time (epoch seconds).
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
    /// Hash of the settling transaction.
    pub transaction_hash: String,
    /// Canonical action verb; SPLIT / MERGE / CONVERT / COMPRESS / WRAP /
    /// UNWRAP / REDEEM. The only action field on the wire; `timestamp` is the
    /// served event clock.
    #[serde(rename = "type")]
    pub combo_activity_type: String,
}

/// A trade (`/v2/trades`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Trade {
    /// Profile bio text.
    pub bio: String,
    /// On-chain condition id of the market (`0x` hex).
    pub condition_id: String,
    /// Parent event slug.
    pub event_slug: String,
    /// Market icon URL.
    pub icon: String,
    /// Profile display name of the wallet.
    pub name: String,
    /// Label of the traded outcome (e.g. `Yes`).
    pub outcome: String,
    /// Index of the traded outcome within the market; `999` means the
    /// outcome could not be labeled.
    pub outcome_index: OutcomeIndex,
    /// Execution price per share, in USDC.
    pub price: f64,
    /// Profile image URL.
    pub profile_image: String,
    /// Resized profile image URL, when one exists.
    pub profile_image_optimized: String,
    /// Proxy wallet the row belongs to; the address every wallet-keyed
    /// endpoint accepts as `user`.
    pub proxy_wallet: String,
    /// Generated fallback handle for profiles without a display name.
    pub pseudonym: String,
    /// `BUY` or `SELL`, from this wallet's perspective.
    pub side: TradeSide,
    /// Filled quantity in shares; bare sizes are shares, never USD.
    pub size: f64,
    /// Market slug; the URL segment on polymarket.com.
    pub slug: String,
    /// Block timestamp of the fill, epoch seconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub timestamp: i64,
    /// Market question title (Gamma enrichment; empty when unenriched).
    pub title: String,
    /// CLOB asset id of the traded outcome token.
    pub token_id: String,
    /// Hash of the settling transaction.
    pub transaction_hash: String,
}
