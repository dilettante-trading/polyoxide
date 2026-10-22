//! Series: the templates Kalshi's recurring events follow, with their categories and tags.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

/// A source a series' markets settle against (`SettlementSource`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementSource {
    /// The source's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where it publishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A series (`Series`): the template of a recurring event, such as a daily temperature.
///
/// A list Kalshi may send as `null` is `None` here and written back as `null`, so the value
/// round-trips: on 2026-10-22, 2,617 of 15,336 series sent `tags: null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Series {
    /// The series' ticker, the prefix of its events' tickers.
    pub ticker: String,
    /// How often its events recur, in words: `daily`, `weekly`, `one-off` and so on.
    pub frequency: String,
    /// Its title.
    pub title: String,
    /// Its primary category.
    pub category: String,
    /// Every discovery category it is listed under; `series().list().category(..)` matches any.
    pub categories: Vec<String>,
    /// The subjects it relates to.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// The official sources its markets settle against.
    #[serde(default)]
    pub settlement_sources: Option<Vec<SettlementSource>>,
    /// The original filing of the contract.
    pub contract_url: String,
    /// The current terms of the contract.
    pub contract_terms_url: String,
    /// Internal metadata, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_metadata: Option<Value>,
    /// The fee structure: `quadratic`, `quadratic_with_maker_fees`,
    /// `quadratic_with_combo_maker_fees` or `flat`.
    pub fee_type: String,
    /// The multiplier applied to the fee, as sent (an integer or a float).
    pub fee_multiplier: Number,
    /// Further trading prohibitions.
    #[serde(default)]
    pub additional_prohibitions: Option<Vec<String>>,
    /// Contracts traded across its events; sent only when asked for with `include_volume`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub volume_fp: Option<Decimal>,
    /// When its metadata last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_ts: Option<String>,
    /// The exchange shard its events trade on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_index: Option<u32>,
}

/// `GET /series` (`GetSeriesListResponse`): every series the filters admit, in one body with no
/// cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesList {
    /// The series.
    pub series: Vec<Series>,
}

/// `GET /series/{series_ticker}` (`GetSeriesResponse`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesResponse {
    /// The series.
    pub series: Series,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_list_round_trips_as_null_and_a_number_keeps_its_spelling() {
        let text = r#"{"ticker":"KXT","frequency":"daily","title":"T","category":"C","categories":[],"tags":null,"settlement_sources":null,"contract_url":"u","contract_terms_url":"v","fee_type":"flat","fee_multiplier":1,"additional_prohibitions":null}"#;
        let series: Series = serde_json::from_str(text).unwrap();
        assert!(series.tags.is_none() && series.settlement_sources.is_none());
        let back = serde_json::to_value(&series).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(text).unwrap());
    }
}
