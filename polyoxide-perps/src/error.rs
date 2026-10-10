//! Error types for the Perps API.

use std::time::Duration;

use polyoxide_core::{ApiError, ErrorResponse};
use polyoxide_venue::{class_for_status, Class, Classify};
use serde::Deserialize;
use thiserror::Error;

/// Error type for Perps API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PerpsError {
    /// Transport, decoding, or an error body in some other shape.
    #[error(transparent)]
    Api(ApiError),

    /// The venue answered with its `{status: "err", error}` body.
    #[error(transparent)]
    Venue(#[from] VenueError),
}

/// An error the Perps host produced, carrying its stable identifier.
///
/// Upstream calls `error` a machine-readable snake_case identifier that is
/// part of the API contract for domain and transport rejections
/// (`ip_rate_limited`, `not_found`). On a 400 it is a human-readable
/// validation message instead. It stays a `String` because the catalogue is
/// long and grows.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("perps API {status}: {code}")]
#[non_exhaustive]
pub struct VenueError {
    /// HTTP status.
    pub status: u16,
    /// The wire `error` field.
    pub code: String,
    /// The wire `ref` field, a gateway trace id (`g-…`), sent on validation
    /// failures and absent on `not_found`. Quote it when reporting a failure.
    pub reference: Option<String>,
    /// `Retry-After`, whole seconds, sent only on token-bucket rejections.
    pub retry_after: Option<Duration>,
}

#[derive(Deserialize)]
struct Body {
    status: String,
    error: String,
    #[serde(default, rename = "ref")]
    reference: Option<String>,
}

impl VenueError {
    /// Parses a venue error body; `None` when the body is some other shape
    /// (a CDN error page, or a body without `status: "err"`).
    ///
    /// The `Retry-After` is the response's, read by core unclamped: it is
    /// surfaced, never slept on.
    pub(crate) fn from_response(response: &ErrorResponse) -> Option<Self> {
        let body: Body = serde_json::from_str(&response.body).ok()?;
        if body.status != "err" {
            return None;
        }
        Some(Self {
            status: response.status.as_u16(),
            code: body.error,
            reference: body.reference,
            retry_after: response.retry_after,
        })
    }

    /// Whether re-sending the same request could plausibly succeed: a 408,
    /// a 425, a 429 or any 5xx, by [`polyoxide_venue::class_for_status`].
    ///
    /// The same rule as the class of an [`ApiError`] with that status, so a
    /// response classifies the same whether or not its body had the venue
    /// shape; `venue_and_api_errors_agree_on_retriability` pins the two
    /// together.
    pub fn is_retriable(&self) -> bool {
        class_for_status(self.status).is_some_and(|c| c.is_retriable())
    }
}

impl PerpsError {
    /// Whether re-sending the same request could plausibly succeed.
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(err) => Classify::is_retriable(err),
            Self::Venue(err) => err.is_retriable(),
        }
    }

    /// The venue's error identifier, for venue errors.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Venue(err) => Some(&err.code),
            Self::Api(_) => None,
        }
    }

    /// The `Retry-After` delay, for venue errors that carried one.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Venue(err) => err.retry_after,
            Self::Api(_) => None,
        }
    }
}

/// Perps' one decode: a response with the venue's `{status: "err", error,
/// ref}` body is [`PerpsError::Venue`], told apart by body shape, not by path
/// or status, and anything else stays core's.
impl From<ApiError> for PerpsError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Response(response) => match VenueError::from_response(&response) {
                Some(venue) => Self::Venue(venue),
                None => Self::Api(ApiError::Response(response)),
            },
            other => Self::Api(other),
        }
    }
}

/// The status rule, with `code` as the class's code and any status outside
/// 400–599 a [`Class::Decode`]. A 451 is not a fault: the venue does not serve
/// the caller's region, as designed. [`Classify::retry_after`] is the
/// response's `Retry-After` whatever the status, and `None` when it was zero.
impl Classify for VenueError {
    fn class(&self) -> Class {
        class_for_status(self.status)
            .unwrap_or(Class::Decode)
            .with_code(self.code.as_str())
            .with_retry_after(self.retry_after)
    }

    fn is_fault(&self) -> bool {
        self.status != 451
    }

    fn retry_after(&self) -> Option<Duration> {
        self.retry_after.filter(|wait| !wait.is_zero())
    }
}

/// Delegates to the error each variant wraps.
impl Classify for PerpsError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::Venue(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            Self::Venue(err) => err.is_fault(),
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            Self::Venue(err) => Classify::retry_after(err),
        }
    }
}

polyoxide_core::impl_api_error_conversions!(PerpsError);

#[cfg(test)]
mod tests {
    use super::*;

    /// A response with `status`, `body` and, when given, a `Retry-After`.
    fn response(status: u16, retry_after: Option<&str>, body: &str) -> ErrorResponse {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(reqwest::header::RETRY_AFTER, value.parse().unwrap());
        }
        ErrorResponse::new(
            reqwest::StatusCode::from_u16(status).unwrap(),
            headers,
            body,
        )
    }

    /// [`VenueError::from_response`] on that response.
    fn parts(status: u16, retry_after: Option<&str>, body: &str) -> Option<VenueError> {
        VenueError::from_response(&response(status, retry_after, body))
    }

    #[test]
    fn a_404_body_becomes_a_venue_error_with_its_identifier() {
        let err = parts(404, None, r#"{"status":"err","error":"not_found"}"#).expect("venue shape");
        assert_eq!(err.code, "not_found");
        assert_eq!(err.reference, None);
        assert!(!err.is_retriable());
    }

    #[test]
    fn a_400_body_keeps_the_gateway_reference() {
        // Captured 2026-09-30: validation failures carry `arts`, `ts` and `ref`
        // beyond the schema's two fields.
        let body = r#"{"status":"err","error":"invalid query parameters: missing field `instrument_id`","arts":1790758475821,"ts":1790758475821,"ref":"g-1224ed1744735"}"#;
        let err = parts(400, None, body).expect("venue shape");
        assert_eq!(err.reference.as_deref(), Some("g-1224ed1744735"));
        assert!(err.code.starts_with("invalid query parameters"));
    }

    #[test]
    fn a_429_is_retriable_and_reads_retry_after_as_whole_seconds() {
        let err = PerpsError::Venue(
            parts(
                429,
                Some("2"),
                r#"{"status":"err","error":"ip_rate_limited"}"#,
            )
            .unwrap(),
        );
        assert!(err.is_retriable());
        assert_eq!(err.code(), Some("ip_rate_limited"));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(2)));

        let too_early = parts(425, None, r#"{"status":"err","error":"too_early"}"#).unwrap();
        assert!(too_early.is_retriable());
    }

    #[test]
    fn a_body_without_status_err_is_not_a_venue_error() {
        assert_eq!(parts(200, None, r#"{"status":"ok"}"#), None);
        assert_eq!(parts(502, None, "<html>bad gateway</html>"), None);
    }

    #[test]
    fn api_errors_have_no_code_or_retry_after() {
        let err = PerpsError::from(ApiError::from(response(408, None, "timeout")));
        assert!(err.is_retriable());
        assert_eq!(err.code(), None);
        assert_eq!(err.retry_after(), None);
    }

    #[test]
    fn venue_and_api_errors_agree_on_retriability() {
        // The same status must classify the same whether the body had the
        // venue shape or not; otherwise a retry policy would depend on which
        // proxy layer answered.
        let body = r#"{"status":"err","error":"x"}"#;
        for status in [
            400u16, 401, 403, 404, 408, 409, 413, 422, 425, 429, 500, 502, 503, 504,
        ] {
            let venue = parts(status, None, body).unwrap();
            let api = ApiError::from(response(status, None, "x"));
            assert_eq!(
                venue.is_retriable(),
                Classify::is_retriable(&api),
                "status {status}: VenueError says {} but ApiError says {}",
                venue.is_retriable(),
                Classify::is_retriable(&api)
            );
        }
    }

    #[test]
    fn every_variant_classifies() {
        let venue = |status, retry_after, code: &str| {
            let body = format!(r#"{{"status":"err","error":"{code}"}}"#);
            PerpsError::Venue(parts(status, retry_after, &body).unwrap())
        };
        let code = |c: &str| Some(std::sync::Arc::from(c));
        let secs = |n| Some(Duration::from_secs(n));
        // (error, class, is_fault, (trait retry_after, inherent retry_after),
        // inherent is_retriable)
        let rows = [
            (
                PerpsError::from(ApiError::from(response(408, None, "timeout"))),
                Class::Unavailable { code: None },
                true,
                (None, None),
                true,
            ),
            (
                PerpsError::from(ApiError::from(response(
                    502,
                    None,
                    "<html>bad gateway</html>",
                ))),
                Class::Unavailable { code: None },
                true,
                (None, None),
                true,
            ),
            (
                venue(400, None, "invalid query parameters"),
                Class::VenueRefusal {
                    code: code("invalid query parameters"),
                },
                true,
                (None, None),
                false,
            ),
            (
                venue(404, None, "not_found"),
                Class::VenueRefusal {
                    code: code("not_found"),
                },
                true,
                (None, None),
                false,
            ),
            (
                venue(429, Some("2"), "ip_rate_limited"),
                Class::RateLimited {
                    retry_after: secs(2),
                },
                true,
                (secs(2), secs(2)),
                true,
            ),
            // A zero is no wait, to the trait and the inherent method alike
            // (DRIFT R4).
            (
                venue(429, Some("0"), "ip_rate_limited"),
                Class::RateLimited { retry_after: None },
                true,
                (None, None),
                true,
            ),
            (
                venue(503, Some("5"), "unavailable"),
                Class::Unavailable {
                    code: code("unavailable"),
                },
                true,
                (secs(5), secs(5)),
                true,
            ),
            // A region block is the venue answering as designed.
            (
                venue(451, None, "restricted"),
                Class::Restricted,
                false,
                (None, None),
                false,
            ),
            (
                venue(200, None, "odd"),
                Class::Decode,
                true,
                (None, None),
                false,
            ),
        ];
        for (err, class, fault, (wait, inherent_wait), inherent) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), wait, "{err:?}");
            assert_eq!(err.retry_after(), inherent_wait, "{err:?}");
            assert_eq!(
                Classify::is_retriable(&err),
                class.is_retriable(),
                "{err:?}"
            );
            assert_eq!(err.is_retriable(), inherent, "{err:?}");
            if let PerpsError::Venue(venue) = &err {
                assert_eq!(venue.class(), class, "{venue:?}");
                assert_eq!(venue.is_fault(), fault, "{venue:?}");
                assert_eq!(venue.is_retriable(), inherent, "{venue:?}");
            }
        }
    }

    #[test]
    fn perps_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PerpsError>();
    }

    #[test]
    fn the_one_retry_after_parser_settles_the_old_disagreements() {
        // DRIFT R4. Perps read whole seconds only, so a fraction was no wait,
        // a zero was a wait of nothing, and a value past `u64` was no wait.
        let body = r#"{"status":"err","error":"ip_rate_limited"}"#;
        let at = |header| parts(429, Some(header), body).unwrap().retry_after;
        assert_eq!(at("1.5"), Some(Duration::from_millis(1500)));
        assert_eq!(at("0"), None);
        assert_eq!(at("99999999999999999999"), Some(Duration::MAX));
        // Unchanged: no clamp short of what a `Duration` holds, and junk is no
        // wait.
        assert_eq!(at("604800"), Some(Duration::from_secs(604_800)));
        for junk in [
            "Wed, 21 Oct 2026 07:28:00 GMT",
            "abc",
            "NaN",
            "inf",
            "-1",
            "",
        ] {
            assert_eq!(at(junk), None, "{junk:?}");
        }
    }
}
