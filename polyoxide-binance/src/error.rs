//! Error types for the Binance API.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError};
use serde::Deserialize;
use thiserror::Error;

use crate::weight::MAX_COOLDOWN;

/// Error type for Binance API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum BinanceError {
    /// Transport, decoding, or an error body in a shape Binance does not use.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// Binance refused the request with its `{"code", "msg"}` body: an unknown
    /// symbol is `400` with code `-1121`.
    #[error("binance answered {status}: {code} {msg}")]
    Venue {
        /// HTTP status.
        status: u16,
        /// Binance's error code, always negative.
        code: i64,
        /// Binance's message, clipped to 512 bytes.
        msg: String,
    },

    /// Still answered `429` after the retry schedule ran out.
    #[error("binance rate limit (429), retry after {retry_after:?}")]
    RateLimited {
        /// The response's `Retry-After`.
        retry_after: Option<Duration>,
    },

    /// `418`: Binance has banned this IP for continuing after a `429`. Every
    /// request on the same [`WeightBudget`](crate::WeightBudget) waits until the
    /// ban lifts.
    #[error("binance has banned this IP (418), retry after {retry_after:?}")]
    IpBanned {
        /// The response's `Retry-After`. The docs give bans of 2 minutes to 3 days.
        retry_after: Option<Duration>,
    },

    /// `451`: Binance does not serve the caller's location.
    #[error("binance does not serve this location (451): {msg}")]
    RegionBlocked {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },

    /// `403`: Binance's web application firewall refused the request.
    #[error("binance's firewall refused the request (403): {msg}")]
    Forbidden {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
}

#[derive(Deserialize)]
struct VenueBody {
    code: i64,
    msg: String,
}

impl BinanceError {
    /// Classifies an unsuccessful response.
    ///
    /// `418`, `429`, `451` and `403` go by status alone: their bodies were not
    /// observed from a test IP, and the status is what decides what a caller
    /// should do. Anything else with Binance's `{code, msg}` body is
    /// [`Venue`](Self::Venue); any other body is left to core.
    pub(crate) fn from_response_parts(status: u16, retry_after: Option<&str>, body: &str) -> Self {
        let retry_after = retry_after_secs(retry_after);
        match status {
            418 => Self::IpBanned { retry_after },
            429 => Self::RateLimited { retry_after },
            451 => Self::RegionBlocked { msg: clip(body) },
            403 => Self::Forbidden { msg: clip(body) },
            _ => match serde_json::from_str::<VenueBody>(body) {
                Ok(venue) => Self::Venue {
                    status,
                    code: venue.code,
                    msg: clip(&venue.msg),
                },
                Err(_) => Self::Api(ApiError::from_status_and_body(status, &clip(body))),
            },
        }
    }

    /// Whether re-sending the same request could plausibly succeed.
    ///
    /// A `429` and a 5xx are; a ban, a region block and a firewall refusal are
    /// not, because sending again does not change them, and sending after a
    /// `418` lengthens the ban.
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(err) => err.is_retriable(),
            Self::Venue { status, .. } => *status >= 500,
            Self::RateLimited { .. } => true,
            Self::IpBanned { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => false,
        }
    }

    /// Binance's error code, for a [`Venue`](Self::Venue) error.
    pub fn code(&self) -> Option<i64> {
        match self {
            Self::Venue { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// The `Retry-After` delay, for a `429` or a `418` that carried one.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } | Self::IpBanned { retry_after } => *retry_after,
            _ => None,
        }
    }
}

/// Parses `Retry-After` as seconds, whole or fractional. Zero, negative and
/// unparsable values are `None`; anything past [`MAX_COOLDOWN`] is clamped to it.
pub(crate) fn retry_after_secs(value: Option<&str>) -> Option<Duration> {
    let secs = value?.trim().parse::<f64>().ok()?;
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    Some(Duration::from_secs_f64(
        secs.min(MAX_COOLDOWN.as_secs_f64()),
    ))
}

fn clip(text: &str) -> String {
    truncate_for_log(text).into_owned()
}

polyoxide_core::impl_api_error_conversions!(BinanceError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_symbol_is_a_venue_error_with_its_code() {
        // Captured 2026-10-07 from `GET /fapi/v1/ticker/24hr?symbol=NOTASYMBOLUSDT`.
        let err = BinanceError::from_response_parts(
            400,
            None,
            r#"{"code":-1121,"msg":"Invalid symbol."}"#,
        );
        assert!(
            matches!(&err, BinanceError::Venue { status: 400, code: -1121, msg } if msg == "Invalid symbol.")
        );
        assert_eq!(err.code(), Some(-1121));
        assert!(!err.is_retriable());
    }

    #[test]
    fn statuses_that_decide_the_caller_s_next_step_win_over_the_body() {
        // A 429 carries a {code, msg} body too; it must still be a rate limit,
        // not a venue error a retry policy would treat as permanent.
        let body = r#"{"code":-1003,"msg":"Too many requests."}"#;
        let limited = BinanceError::from_response_parts(429, Some("7"), body);
        assert!(matches!(limited, BinanceError::RateLimited { .. }));
        assert_eq!(limited.retry_after(), Some(Duration::from_secs(7)));
        assert!(limited.is_retriable());

        let banned = BinanceError::from_response_parts(418, Some("120"), body);
        assert!(matches!(banned, BinanceError::IpBanned { .. }));
        assert_eq!(banned.retry_after(), Some(Duration::from_secs(120)));
        assert!(!banned.is_retriable());

        let region = BinanceError::from_response_parts(451, None, body);
        assert!(matches!(&region, BinanceError::RegionBlocked { msg } if msg.contains("-1003")));
        assert!(!region.is_retriable());

        let firewall = BinanceError::from_response_parts(403, None, "<html>denied</html>");
        assert!(
            matches!(&firewall, BinanceError::Forbidden { msg } if msg == "<html>denied</html>")
        );
        assert!(!firewall.is_retriable());
    }

    #[test]
    fn a_5xx_venue_error_is_retriable_and_a_4xx_is_not() {
        let body = r#"{"code":-1001,"msg":"Internal error; unable to process your request."}"#;
        assert!(BinanceError::from_response_parts(503, None, body).is_retriable());
        assert!(!BinanceError::from_response_parts(400, None, body).is_retriable());
    }

    #[test]
    fn a_body_in_another_shape_is_left_to_core() {
        let err = BinanceError::from_response_parts(502, None, "<html>bad gateway</html>");
        assert!(matches!(
            err,
            BinanceError::Api(ApiError::Api { status: 502, .. })
        ));
        assert!(err.is_retriable());
        assert_eq!(err.code(), None);
    }

    #[test]
    fn bodies_are_clipped_before_they_are_kept() {
        let body = "x".repeat(10_000);
        let BinanceError::Forbidden { msg } = BinanceError::from_response_parts(403, None, &body)
        else {
            panic!("a 403 is Forbidden");
        };
        assert!(msg.len() < 600, "kept {} bytes", msg.len());
    }

    #[test]
    fn retry_after_reads_seconds_and_rejects_what_is_not_a_wait() {
        assert_eq!(retry_after_secs(Some("2")), Some(Duration::from_secs(2)));
        assert_eq!(
            retry_after_secs(Some(" 1.5 ")),
            Some(Duration::from_millis(1500))
        );
        for junk in ["0", "-3", "soon", "NaN", "inf"] {
            assert_eq!(retry_after_secs(Some(junk)), None, "{junk:?}");
        }
        assert_eq!(retry_after_secs(None), None);
        // A week is longer than any documented ban; it is clamped, not trusted.
        assert_eq!(retry_after_secs(Some("604800")), Some(MAX_COOLDOWN));
    }

    #[test]
    fn binance_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BinanceError>();
    }
}
