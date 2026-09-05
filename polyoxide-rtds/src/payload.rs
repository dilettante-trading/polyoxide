//! Per-topic payload types.
//!
//! [`BinanceUpdate`] and [`ChainlinkSpotUpdate`] carry identical fields and
//! are deliberately **not** merged. Their `full_accuracy_value` differs in
//! scale — plain decimal versus E18 — and keeping two structurally identical
//! things apart is the one thing a type can do that a runtime scale tag
//! cannot.

use rust_decimal::Decimal;

use crate::topic::TwapWindow;

/// A Binance spot price update, from `crypto_prices`.
#[derive(Debug, Clone, PartialEq)]
pub struct BinanceUpdate {
    /// Venue symbol, e.g. `btcusdt`. Always lowercase on the wire.
    pub symbol: String,
    /// Venue observation time, Unix milliseconds (`payload.timestamp`).
    pub observed_at: i64,
    /// When the publisher submitted this to RTDS, Unix milliseconds.
    pub published_at: i64,
    /// Server-side connection handle. Undocumented; present on updates only.
    pub connection_id: Option<String>,
    /// Exact price, decoded from the **plain decimal** `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded `full_accuracy_value`, retained for auditing.
    pub raw: String,
    /// The venue's lossy float. For display only — never use it for
    /// arithmetic; use [`value`](Self::value).
    pub display_value: f64,
}

/// A Chainlink spot price update, from `crypto_prices_chainlink`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainlinkSpotUpdate {
    /// Chainlink symbol, e.g. `btc/usd`.
    pub symbol: String,
    /// Chainlink observation time, Unix milliseconds.
    pub observed_at: i64,
    /// When the publisher submitted this to RTDS, Unix milliseconds.
    pub published_at: i64,
    /// Server-side connection handle.
    pub connection_id: Option<String>,
    /// Exact price, decoded from the **E18** `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded E18 integer string, retained for auditing.
    pub raw: String,
    /// The venue's lossy float. For display only.
    pub display_value: f64,
}

/// A Chainlink TWAP update, from `crypto_prices_twap_thirty` or
/// `crypto_prices_twap_sixty`.
#[derive(Debug, Clone, PartialEq)]
pub struct TwapUpdate {
    /// Chainlink symbol, e.g. `btc/usd`.
    pub symbol: String,
    /// Lookback window. Not optional: this type only exists for TWAP frames,
    /// which always carry `window_s`.
    pub window: TwapWindow,
    /// Chainlink observation time, Unix milliseconds.
    pub observed_at: i64,
    /// When the publisher submitted this to RTDS, Unix milliseconds.
    pub published_at: i64,
    /// Server-side connection handle.
    pub connection_id: Option<String>,
    /// Exact price, decoded from the **E18** `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded E18 integer string, retained for auditing.
    pub raw: String,
    /// The venue's lossy float. For display only.
    pub display_value: f64,
}

/// One point in a backfill that carries no exact value.
///
/// Binance and Chainlink-spot snapshots omit `full_accuracy_value` entirely,
/// so the float is all there is.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayPoint {
    /// Observation time, Unix milliseconds.
    pub observed_at: i64,
    /// The venue's lossy float — the only value available on these points.
    pub display_value: f64,
}

/// One point in a backfill that carries an exact value.
#[derive(Debug, Clone, PartialEq)]
pub struct ExactPoint {
    /// Observation time, Unix milliseconds.
    pub observed_at: i64,
    /// Exact price, decoded from the E18 `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded E18 integer string.
    pub raw_e18: String,
    /// The venue's lossy float. For display only.
    pub display_value: f64,
}

/// The points of a backfill, whose shape depends on the topic.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SnapshotPoints {
    /// Binance and Chainlink spot. No exact values exist in these backfills.
    DisplayOnly(Vec<DisplayPoint>),
    /// Chainlink TWAP. Exact values present.
    Exact(Vec<ExactPoint>),
}

impl SnapshotPoints {
    /// How many points the backfill carried.
    pub fn len(&self) -> usize {
        match self {
            Self::DisplayOnly(points) => points.len(),
            Self::Exact(points) => points.len(),
        }
    }

    /// Whether the backfill was empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The backfill RTDS sends once, immediately after each subscribe.
///
/// Upstream documents that no snapshot exists; it does. Every resubscribe
/// replays one, so a reconnect re-initialises caller state rather than merely
/// resuming the feed — and callers therefore see this event more than once.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The topic backfilled.
    pub topic: crate::topic::Topic,
    /// The symbol backfilled.
    pub symbol: String,
    /// When RTDS sent the backfill, Unix milliseconds.
    pub published_at: i64,
    /// The points, roughly one per second — about 55-59 for the Chainlink
    /// topics and 120 for Binance.
    pub points: SnapshotPoints,
}
