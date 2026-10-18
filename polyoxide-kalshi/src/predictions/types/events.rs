//! Events: a set of related markets.

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use super::markets::{null_as_default, Market};

/// An event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The event's ticker.
    pub event_ticker: String,
    /// The series it belongs to.
    pub series_ticker: String,
    /// A short descriptive title.
    pub sub_title: String,
    /// The full title.
    pub title: String,
    /// The collateral-return netting type: `MECNET`, `DIRECNET`, or empty for none.
    pub collateral_return_type: String,
    /// Whether at most one of its markets can resolve YES (`MECNET`).
    pub mutually_exclusive: bool,
    /// Deprecated: the event's category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// The date the event is based on, when it has a date strike.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike_date: Option<String>,
    /// The period the event covers, when it has a period strike.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike_period: Option<String>,
    /// Its markets; filled only when the request asked for them nested, and empty otherwise.
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub markets: Vec<Market>,
    /// Additional metadata, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_metadata: Option<Value>,
    /// The sources that determine its markets, as sent.
    #[serde(default)]
    pub settlement_sources: Option<Vec<Value>>,
    /// When its metadata last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_ts: Option<String>,
    /// A fee type that overrides its series'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_type_override: Option<String>,
    /// A fee multiplier that overrides its series'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_multiplier_override: Option<Number>,
    /// The exchange shard the event trades on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_index: Option<u32>,
}

/// `GET /events/{event_ticker}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventResponse {
    /// The event.
    pub event: Event,
    /// Deprecated: the event's markets, which the host leaves empty when they are nested.
    #[serde(default, deserialize_with = "null_as_default")]
    pub markets: Vec<Market>,
}
