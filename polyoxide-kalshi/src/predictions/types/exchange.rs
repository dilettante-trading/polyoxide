//! The exchange's status: `GET /trade-api/v2/exchange/status`.

use serde::{Deserialize, Serialize};

/// The exchange's status, and the status of each of its exchange indexes.
///
/// The top-level fields are the default index's (index 0). Kalshi sends the status route's body
/// with a 500, 503 or 504 too, so a body of this shape does not mean the exchange is up: read
/// [`exchange_active`](Self::exchange_active) and [`trading_active`](Self::trading_active).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExchangeStatus {
    /// False if the exchange is taking no state changes at all: under maintenance.
    pub exchange_active: bool,
    /// True while trading is permitted: during trading hours, and not during a pause.
    pub trading_active: bool,
    /// True while intra-exchange transfers are permitted.
    pub intra_exchange_transfers_active: Option<bool>,
    /// When the current maintenance window is expected to end, as an RFC 3339 time. Not a
    /// guarantee: the window can be extended.
    pub exchange_estimated_resume_time: Option<String>,
    /// The status of each exchange index; empty when Kalshi sends no breakdown.
    #[serde(default)]
    pub exchange_index_statuses: Vec<ExchangeIndexStatus>,
}

/// The status of one exchange index (a shard of the exchange), such as `Default` or `Combos`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExchangeIndexStatus {
    /// What the index is.
    pub description: String,
    /// False if the index is taking no state changes at all.
    pub exchange_active: bool,
    /// The index's number; markets name theirs in `exchange_index`.
    pub exchange_index: u32,
    /// True while the index permits intra-exchange transfers.
    pub intra_exchange_transfers_active: bool,
    /// True while the index permits trading.
    pub trading_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_with_only_the_required_keys_gives_the_optional_ones_their_defaults() {
        let status: ExchangeStatus =
            serde_json::from_str(r#"{"exchange_active":false,"trading_active":false}"#).unwrap();
        assert!(!status.exchange_active);
        assert!(!status.trading_active);
        assert_eq!(status.intra_exchange_transfers_active, None);
        assert_eq!(status.exchange_estimated_resume_time, None);
        assert!(status.exchange_index_statuses.is_empty());
    }

    #[test]
    fn a_null_or_a_given_resume_time_is_read() {
        let body = |resume: &str| {
            format!(
                r#"{{"exchange_active":false,"trading_active":false,"exchange_estimated_resume_time":{resume}}}"#
            )
        };
        let null: ExchangeStatus = serde_json::from_str(&body("null")).unwrap();
        assert_eq!(null.exchange_estimated_resume_time, None);
        let given: ExchangeStatus =
            serde_json::from_str(&body(r#""2026-10-19T14:00:00Z""#)).unwrap();
        assert_eq!(
            given.exchange_estimated_resume_time.as_deref(),
            Some("2026-10-19T14:00:00Z")
        );
    }

    #[test]
    fn a_body_without_a_required_key_is_refused_naming_it() {
        let err =
            serde_json::from_str::<ExchangeStatus>(r#"{"exchange_active":true}"#).unwrap_err();
        assert!(err.to_string().contains("trading_active"), "{err}");
        let err = serde_json::from_str::<ExchangeIndexStatus>(
            r#"{"description":"Default","exchange_active":true,"exchange_index":0,"trading_active":true}"#,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("intra_exchange_transfers_active"),
            "{err}"
        );
    }

    #[test]
    fn a_status_serializes_back_to_what_it_was_read_from() {
        let status = ExchangeStatus {
            exchange_active: true,
            trading_active: false,
            intra_exchange_transfers_active: Some(true),
            exchange_estimated_resume_time: Some("2026-10-19T14:00:00Z".to_owned()),
            exchange_index_statuses: vec![ExchangeIndexStatus {
                description: "Default".to_owned(),
                exchange_active: true,
                exchange_index: 0,
                intra_exchange_transfers_active: true,
                trading_active: false,
            }],
        };
        let text = serde_json::to_string(&status).unwrap();
        assert_eq!(
            serde_json::from_str::<ExchangeStatus>(&text).unwrap(),
            status
        );
    }
}
