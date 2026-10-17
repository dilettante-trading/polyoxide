//! Types for the `service` route: data freshness.

use serde::{Deserialize, Serialize};

/// One ingestion stream's distance from the furthest-along stream.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CursorLag {
    /// How many blocks this stream trails the furthest-along stream; `0` for
    /// the leader.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub behind_max: i64,
    /// Last block the stream has ingested through.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub block: i64,
    /// The stream this cursor belongs to, as `<contract>_<event>`.
    pub source: String,
}

/// Ingestion health: one cursor per `(contract, event)` stream.
///
/// Every figure here is over the LIVE streams only. Streams that are dormant by
/// design sit arbitrarily far behind forever; they are dropped outright rather
/// than reported and flagged, because there is nothing a consumer could do with
/// them and including them is precisely what makes the list useless.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IngestionFreshness {
    /// The chain id this service is configured for. Echoed so the pairing above
    /// is readable in one response.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub chain_id: i64,
    /// How many live streams were found. `0` means no ingestion cursors are
    /// visible to this API at all.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub cursors: u64,
    /// The furthest-behind live streams, `most_lagged` first.
    pub lagging: Vec<CursorLag>,
    /// The tail: the furthest-along cursor. Every `behind_max` is measured
    /// against it, and so is each serving mechanism's `blocks_behind`.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub max_synced_block: Option<i64>,
    /// The furthest-behind LIVE stream's block. Deliberately asymmetric with
    /// `max_synced_block` above, which is over ALL cursors: the tail must not
    /// move with the dormant cut, or excluding a stream would redefine the
    /// distance every other stream is measured against.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub min_synced_block: Option<i64>,
    /// The single furthest-behind live stream. A stall confined to one stream
    /// while every sibling sits at head is invisible in any aggregate figure.
    pub most_lagged: Option<CursorLag>,
    /// The chain the ingestion cursors were written for, as the datastore
    /// itself names it; NOT as this service is configured. Compared against
    /// `chain_id`, it catches a datastore pointed at the wrong chain, which
    /// otherwise presents as data that is merely wrong. `null` when no stream
    /// declares one.
    pub network: Option<String>,
}

/// `/v2/status` payload.
///
/// Served from a snapshot refreshed in the background, never computed on the
/// request. `computed_at`/`age_seconds` make that explicit rather than implicit:
/// if the refresher wedges or its datastore goes away, the answer keeps being
/// served with a growing age instead of turning into a 500; the right degraded
/// mode for the endpoint you call when something is already wrong.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServiceStatus {
    /// How old the snapshot is, in seconds. Normally under the refresh
    /// interval; a value that keeps climbing means the refresher is not
    /// completing, and the freshness figures below are that stale ON TOP of
    /// whatever lag they report.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub age_seconds: i64,
    /// When this snapshot was taken, RFC 3339 in UTC.
    pub computed_at: String,
    pub ingestion: IngestionFreshness,
    pub serving: ServingFreshness,
}

/// What API consumers actually experience: the projections behind the feeds.
///
/// One headline number and the name of whichever mechanism produced it, so a
/// single stalled projection cannot hide behind two healthy ones; the same
/// shape as `ingestion.most_lagged`, for the same reason.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServingFreshness {
    /// The worst age across `mechanisms`. `null` only when no mechanism
    /// reported at all.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub lag_seconds: Option<i64>,
    /// Every mechanism that produced a candidate freshness row, in a fixed
    /// order; `worst` names the culprit.
    pub mechanisms: Vec<ServingMechanism>,
    /// Which mechanism `lag_seconds` came from.
    pub worst: Option<String>,
}

/// One serving mechanism's freshness.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServingMechanism {
    /// Seconds since it last advanced, by its own clock.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub age_seconds: i64,
    /// How far behind the ingestion tail it has projected. Absent for a
    /// mechanism that records a time but no block.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub blocks_behind: Option<i64>,
    /// What this mechanism produces: `activity_feed`, `custody_balances`, `pnl`.
    pub name: String,
}
