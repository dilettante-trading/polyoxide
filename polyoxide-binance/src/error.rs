//! Error types for the Binance API.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError, RequestError};
use polyoxide_venue::{class_for_status, Class, Classify};
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
                Err(_) => Self::Api(core_error(status, body)),
            },
        }
    }

    /// Whether re-sending the same request could plausibly succeed.
    ///
    /// A `429`, and a `408`, `425` or 5xx however its body is shaped, are, by
    /// [`polyoxide_venue::class_for_status`], the rule core's
    /// `ApiError::is_retriable` also uses. A ban, a region block and a firewall
    /// refusal are not: sending again does not change them, and sending after a
    /// `418` lengthens the ban.
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(err) => err.is_retriable(),
            Self::Venue { status, .. } => {
                class_for_status(*status).is_some_and(|c| c.is_retriable())
            }
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

/// An unsuccessful response, read whole: its status, `Retry-After` and body.
///
/// A `418`'s body is Binance's `-1003` text, which names when the ban ends.
/// The error's fields have no room for it, so it is logged at WARN under
/// `polyoxide_binance`; the send loop warns of the hold itself.
impl RequestError for BinanceError {
    async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let path = response.url().path().to_owned();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.text().await.unwrap_or_default();
        if status == 418 {
            tracing::warn!("418 on {path}: IP banned: {}", truncate_for_log(&body));
        }
        Self::from_response_parts(status, retry_after.as_deref(), &body)
    }
}

/// Parses `Retry-After` with [`polyoxide_venue::parse_retry_after`], clamped
/// to [`MAX_COOLDOWN`]: seconds, whole or fractional, and `None` for zero,
/// negative, non-finite and unparsable values.
pub(crate) fn retry_after_secs(value: Option<&str>) -> Option<Duration> {
    value.and_then(|value| polyoxide_venue::parse_retry_after(value, MAX_COOLDOWN))
}

/// Core's classification of a body in some other shape, read whole so its
/// `error` or `message` field is found, then clipped.
fn core_error(status: u16, body: &str) -> ApiError {
    match ApiError::from_status_and_body(status, body) {
        ApiError::Api { status, message } => ApiError::Api {
            status,
            message: clip(&message),
        },
        ApiError::Authentication(message) => ApiError::Authentication(clip(&message)),
        ApiError::Validation(message) => ApiError::Validation(clip(&message)),
        ApiError::RateLimit(message) => ApiError::RateLimit(clip(&message)),
        other => other,
    }
}

fn clip(text: &str) -> String {
    truncate_for_log(text).into_owned()
}

/// `Api` delegates, and `Venue` follows the status rule with Binance's code
/// in decimal. A `429` is `RateLimited`, and a ban, a region block and a
/// firewall refusal are all `Restricted`: Binance documents its `403` as the
/// firewall's, not as a credential failure.
///
/// A region block is the venue answering as designed, so it is not a fault. A
/// ban's [`Classify::retry_after`] is still the time it lifts.
impl Classify for BinanceError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::Venue { status, code, .. } => class_for_status(*status)
                .unwrap_or(Class::Decode)
                .with_code(code.to_string()),
            Self::RateLimited { retry_after } => {
                Class::RateLimited { retry_after: None }.with_retry_after(*retry_after)
            }
            Self::IpBanned { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => {
                Class::Restricted
            }
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            Self::RegionBlocked { .. } => false,
            // Unreachable today, since a 451 is `RegionBlocked`, but a region
            // block whichever variant carries it.
            Self::Venue { status, .. } => *status != 451,
            Self::RateLimited { .. } | Self::IpBanned { .. } | Self::Forbidden { .. } => true,
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            Self::RateLimited { retry_after } | Self::IpBanned { retry_after } => {
                retry_after.filter(|wait| !wait.is_zero())
            }
            Self::Venue { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => None,
        }
    }
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

        let firewall_json = BinanceError::from_response_parts(403, None, body);
        assert!(matches!(firewall_json, BinanceError::Forbidden { .. }));
    }

    #[test]
    fn a_5xx_venue_error_is_retriable_and_a_4xx_is_not() {
        let body = r#"{"code":-1001,"msg":"Internal error; unable to process your request."}"#;
        assert!(BinanceError::from_response_parts(503, None, body).is_retriable());
        assert!(BinanceError::from_response_parts(500, None, body).is_retriable());
        assert!(!BinanceError::from_response_parts(499, None, body).is_retriable());
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
    fn every_kept_body_is_clipped() {
        let long = "x".repeat(10_000);
        let venue = format!(r#"{{"code":-1121,"msg":"{long}"}}"#);
        let kept = |err: BinanceError| match err {
            BinanceError::Venue { msg, .. }
            | BinanceError::RegionBlocked { msg }
            | BinanceError::Forbidden { msg } => msg,
            BinanceError::Api(ApiError::Api { message, .. }) => message,
            other => panic!("unexpected {other:?}"),
        };
        for err in [
            BinanceError::from_response_parts(400, None, &venue),
            BinanceError::from_response_parts(451, None, &long),
            BinanceError::from_response_parts(403, None, &long),
            BinanceError::from_response_parts(502, None, &long),
        ] {
            let msg = kept(err);
            assert!(
                msg.len() <= 512 + "... [truncated]".len(),
                "kept {} bytes",
                msg.len()
            );
        }
    }

    #[test]
    fn core_reads_a_long_json_body_before_it_is_clipped() {
        let body = format!(r#"{{"message":"short","pad":"{}"}}"#, "p".repeat(600));
        let err = BinanceError::from_response_parts(502, None, &body);
        assert!(
            matches!(&err, BinanceError::Api(ApiError::Api { status: 502, message }) if message == "short"),
            "{err:?}"
        );
    }

    #[test]
    fn venue_and_api_errors_agree_on_retriability() {
        // A status classifies the same whether or not the body had Binance's
        // shape, so a retry policy does not depend on which layer answered.
        let body = r#"{"code":-1,"msg":"x"}"#;
        for status in [400u16, 401, 404, 408, 409, 425, 500, 502, 503, 504] {
            let venue = BinanceError::from_response_parts(status, None, body);
            let api = ApiError::from_status_and_body(status, "x");
            assert_eq!(venue.is_retriable(), api.is_retriable(), "status {status}");
        }
    }

    #[test]
    fn retry_after_reads_seconds_and_rejects_what_is_not_a_wait() {
        assert_eq!(retry_after_secs(Some("2")), Some(Duration::from_secs(2)));
        assert_eq!(
            retry_after_secs(Some(" 1.5 ")),
            Some(Duration::from_millis(1500))
        );
        for junk in ["0", "-3", "soon", "NaN", "inf", "0.0000000004", "1e400"] {
            assert_eq!(retry_after_secs(Some(junk)), None, "{junk:?}");
        }
        assert_eq!(retry_after_secs(None), None);
        // A week is longer than any documented ban; it is clamped, not trusted.
        assert_eq!(retry_after_secs(Some("604800")), Some(MAX_COOLDOWN));
    }

    #[test]
    fn every_variant_classifies() {
        let body = r#"{"code":-1121,"msg":"Invalid symbol."}"#;
        let parts =
            |status, retry_after| BinanceError::from_response_parts(status, retry_after, body);
        let code = |c: &str| Some(std::sync::Arc::from(c));
        let week = Some(MAX_COOLDOWN);
        // (error, class, is_fault, retry_after(), inherent is_retriable)
        let rows = [
            (
                parts(502, None),
                Class::Unavailable {
                    code: code("-1121"),
                },
                true,
                None,
                true,
            ),
            (
                BinanceError::from_response_parts(502, None, "<html>bad gateway</html>"),
                Class::Unavailable { code: None },
                true,
                None,
                true,
            ),
            (
                parts(400, None),
                Class::VenueRefusal {
                    code: code("-1121"),
                },
                true,
                None,
                false,
            ),
            (
                parts(408, None),
                Class::Unavailable {
                    code: code("-1121"),
                },
                true,
                None,
                true,
            ),
            (
                parts(429, Some("7")),
                Class::RateLimited {
                    retry_after: Some(Duration::from_secs(7)),
                },
                true,
                Some(Duration::from_secs(7)),
                true,
            ),
            (
                parts(429, None),
                Class::RateLimited { retry_after: None },
                true,
                None,
                true,
            ),
            // A ban says when it lifts, though its class has no wait of its own.
            (
                parts(418, Some("604800")),
                Class::Restricted,
                true,
                week,
                false,
            ),
            (parts(451, None), Class::Restricted, false, None, false),
            // Built by hand: a 451 always becomes `RegionBlocked`, but a region
            // block is not a fault whichever variant carries it.
            (
                BinanceError::Venue {
                    status: 451,
                    code: -1,
                    msg: "restricted".into(),
                },
                Class::Restricted,
                false,
                None,
                false,
            ),
            (parts(403, None), Class::Restricted, true, None, false),
        ];
        for (err, class, fault, wait, inherent) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), wait, "{err:?}");
            assert_eq!(
                Classify::is_retriable(&err),
                class.is_retriable(),
                "{err:?}"
            );
            assert_eq!(err.is_retriable(), inherent, "{err:?}");
        }
    }

    #[test]
    fn binance_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BinanceError>();
    }
}
