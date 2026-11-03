//! Error types for Binance spot.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError};
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

use crate::shared::error::{decode, venue_class, Decoded};

/// Error type for Binance spot API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SpotError {
    /// Transport, decoding, or an error body in a shape Binance does not use.
    #[error(transparent)]
    Api(ApiError),

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
    /// request on the same [`WeightBudget`](crate::spot::WeightBudget) waits
    /// until the ban lifts.
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

impl SpotError {
    /// Binance's error code, for a [`Venue`](Self::Venue) error.
    pub fn code(&self) -> Option<i64> {
        match self {
            Self::Venue { code, .. } => Some(*code),
            _ => None,
        }
    }
}

/// Binance's one decode for spot: an unsuccessful response, by status first,
/// through the crate's shared decode.
///
/// `418`, `429`, `451` and `403` go by status alone; anything else with
/// Binance's `{code, msg}` body is [`Venue`](SpotError::Venue); any other body
/// stays core's [`ApiError::Response`], its message and body clipped and its
/// `Retry-After` clamped to 3 days, as every arm reads it. Every other
/// [`ApiError`] stays core's.
///
/// A `418`'s body is Binance's `-1003` text, which names when the ban ends.
/// The error's fields have no room for it, so it is logged at WARN under
/// `polyoxide_binance::spot::error`; the send loop warns of the hold itself,
/// naming the path.
impl From<ApiError> for SpotError {
    fn from(err: ApiError) -> Self {
        match decode(err) {
            Decoded::Api(err) => Self::Api(err),
            Decoded::Venue { status, code, msg } => Self::Venue { status, code, msg },
            Decoded::RateLimited { retry_after } => Self::RateLimited { retry_after },
            Decoded::IpBanned { retry_after, body } => {
                tracing::warn!("418: IP banned: {}", truncate_for_log(&body));
                Self::IpBanned { retry_after }
            }
            Decoded::RegionBlocked { msg } => Self::RegionBlocked { msg },
            Decoded::Forbidden { msg } => Self::Forbidden { msg },
        }
    }
}

/// `Api` delegates, and `Venue` follows the status rule with Binance's code
/// in decimal. A `429` is `RateLimited`, and a ban, a region block and a
/// firewall refusal are all `Restricted`: Binance documents its `403` as the
/// firewall's, not as a credential failure (D14). So a `429`, and a `408`,
/// `425` or 5xx however its body is shaped, are retriable, and a ban, a region
/// block and a firewall refusal are not: sending again does not change them,
/// and sending after a `418` lengthens the ban.
///
/// A region block is the venue answering as designed, so it is not a fault. A
/// `429`'s or a ban's [`Classify::retry_after`] is its `Retry-After`, clamped
/// to 3 days: for a ban, the time it lifts.
impl Classify for SpotError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::Venue { status, code, .. } => venue_class(*status, *code),
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

#[cfg(test)]
mod tests {
    use polyoxide_core::ErrorResponse;
    use polyoxide_test_support::fixtures;

    use super::*;
    use crate::spot::weight::MAX_COOLDOWN;

    /// A response with `status`, `body` and, when given, a `Retry-After`.
    fn response(status: u16, retry_after: Option<&str>, body: &str) -> ApiError {
        let mut headers = polyoxide_core::reqwest::header::HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(
                polyoxide_core::reqwest::header::RETRY_AFTER,
                value.parse().unwrap(),
            );
        }
        ErrorResponse::new(
            polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),
            headers,
            body,
        )
        .into()
    }

    /// That response, through the spot module's decode.
    fn parts(status: u16, retry_after: Option<&str>, body: &str) -> SpotError {
        SpotError::from(response(status, retry_after, body))
    }

    #[test]
    fn the_captured_invalid_symbol_body_is_a_venue_error_with_its_code() {
        // Captured from production: `GET /api/v3/exchangeInfo?symbol=NOPEUSDT`
        // answers 400 (`tests/fixtures/spot/PROVENANCE.md`).
        let body = fixtures!("spot/rest").text("error_invalid_symbol");
        let err = parts(400, None, &body);
        assert!(
            matches!(&err, SpotError::Venue { status: 400, code: -1121, msg } if msg == "Invalid symbol."),
            "{err:?}"
        );
        assert_eq!(err.code(), Some(-1121));
        assert!(!err.is_retriable());
    }

    #[test]
    fn statuses_that_decide_the_caller_s_next_step_win_over_the_body() {
        // A 429 carries a {code, msg} body too; it must still be a rate limit,
        // not a venue error a retry policy would treat as permanent.
        let body = r#"{"code":-1003,"msg":"Too many requests."}"#;
        let limited = parts(429, Some("7"), body);
        assert!(matches!(limited, SpotError::RateLimited { .. }));
        assert_eq!(limited.retry_after(), Some(Duration::from_secs(7)));
        assert!(limited.is_retriable());

        let banned = parts(418, Some("120"), body);
        assert!(matches!(banned, SpotError::IpBanned { .. }));
        assert_eq!(banned.retry_after(), Some(Duration::from_secs(120)));
        assert!(!banned.is_retriable());

        let region = parts(451, None, body);
        assert!(matches!(&region, SpotError::RegionBlocked { msg } if msg.contains("-1003")));
        assert!(!region.is_retriable());

        let firewall = parts(403, None, "<html>denied</html>");
        assert!(matches!(&firewall, SpotError::Forbidden { msg } if msg == "<html>denied</html>"));
        assert!(!firewall.is_retriable());

        let firewall_json = parts(403, None, body);
        assert!(matches!(firewall_json, SpotError::Forbidden { .. }));
    }

    #[test]
    fn a_body_in_another_shape_is_left_to_core() {
        let err = parts(502, None, "<html>bad gateway</html>");
        assert!(matches!(
            &err,
            SpotError::Api(ApiError::Response(r)) if r.status.as_u16() == 502
        ));
        assert!(err.is_retriable());
        assert_eq!(err.code(), None);
    }

    #[test]
    fn a_core_error_that_is_not_a_response_stays_core_s() {
        let err = SpotError::from(ApiError::Validation("no symbol".into()));
        assert!(
            matches!(err, SpotError::Api(ApiError::Validation(_))),
            "{err:?}"
        );
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[test]
    fn every_kept_body_is_clipped() {
        let long = "x".repeat(10_000);
        let venue = format!(r#"{{"code":-1121,"msg":"{long}"}}"#);
        let kept = |err: SpotError| match err {
            SpotError::Venue { msg, .. }
            | SpotError::RegionBlocked { msg }
            | SpotError::Forbidden { msg } => vec![msg],
            SpotError::Api(ApiError::Response(r)) => vec![r.message, r.body],
            other => panic!("unexpected {other:?}"),
        };
        for err in [
            parts(400, None, &venue),
            parts(451, None, &long),
            parts(403, None, &long),
            parts(502, None, &long),
        ] {
            for msg in kept(err) {
                assert!(
                    msg.len() <= 512 + "... [truncated]".len(),
                    "kept {} bytes",
                    msg.len()
                );
            }
        }
    }

    #[test]
    fn a_core_response_keeps_binance_s_clamped_wait() {
        let err = parts(503, Some("604800"), "<html>down</html>");
        match &err {
            SpotError::Api(ApiError::Response(r)) => {
                assert_eq!(r.retry_after, Some(MAX_COOLDOWN));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(Classify::retry_after(&err), Some(MAX_COOLDOWN));
    }

    #[test]
    fn every_variant_classifies() {
        let body = r#"{"code":-1121,"msg":"Invalid symbol."}"#;
        let coded = |status, retry_after| parts(status, retry_after, body);
        let code = |c: &str| Some(std::sync::Arc::from(c));
        let week = Some(MAX_COOLDOWN);
        // (error, class, is_fault, retry_after())
        let rows = [
            (
                coded(502, None),
                Class::Unavailable {
                    code: code("-1121"),
                },
                true,
                None,
            ),
            (
                coded(400, None),
                Class::VenueRefusal {
                    code: code("-1121"),
                },
                true,
                None,
            ),
            (
                coded(408, None),
                Class::Unavailable {
                    code: code("-1121"),
                },
                true,
                None,
            ),
            (
                coded(429, Some("7")),
                Class::RateLimited {
                    retry_after: Some(Duration::from_secs(7)),
                },
                true,
                Some(Duration::from_secs(7)),
            ),
            (
                coded(429, None),
                Class::RateLimited { retry_after: None },
                true,
                None,
            ),
            // A ban says when it lifts, though its class has no wait of its own.
            (coded(418, Some("604800")), Class::Restricted, true, week),
            (coded(451, None), Class::Restricted, false, None),
            // Built by hand: a 451 always becomes `RegionBlocked`, but a region
            // block is not a fault whichever variant carries it.
            (
                SpotError::Venue {
                    status: 451,
                    code: -1,
                    msg: "restricted".into(),
                },
                Class::Restricted,
                false,
                None,
            ),
            (coded(403, None), Class::Restricted, true, None),
        ];
        for (err, class, fault, wait) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), wait, "{err:?}");
            assert_eq!(
                Classify::is_retriable(&err),
                class.is_retriable(),
                "{err:?}"
            );
        }
    }

    #[test]
    fn spot_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SpotError>();
    }
}
