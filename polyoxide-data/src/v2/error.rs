//! The structured error body every unsuccessful v2 response carries.

use std::time::Duration;

use polyoxide_core::ErrorResponse;
use polyoxide_venue::{class_for_status, Class, Classify};
use serde::Deserialize;

/// Stable classification of a v2 failure, for programmatic branching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorCode {
    /// `invalid_request` (400)
    InvalidRequest,
    /// `unauthorized` (401). No longer in the published contract, since every v2
    /// route is public; kept so a body that still carries it keeps its name.
    Unauthorized,
    /// `not_found` (404)
    NotFound,
    /// `method_not_allowed` (405)
    MethodNotAllowed,
    /// `request_timeout` (503): the request deadline or the datastore's statement timeout.
    RequestTimeout,
    /// `rate_limited` (429)
    RateLimited,
    /// `dependency_unavailable` (503)
    DependencyUnavailable,
    /// `internal` (500)
    Internal,
    /// A code this version of the SDK does not recognise.
    #[serde(other)]
    Unknown,
}

impl ErrorCode {
    /// The wire spelling. `Unknown` renders as `"unknown"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::NotFound => "not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::RequestTimeout => "request_timeout",
            Self::RateLimited => "rate_limited",
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::Internal => "internal",
            Self::Unknown => "unknown",
        }
    }
}

/// An unsuccessful v2 response, with every field the server sent.
#[derive(Debug, Clone, thiserror::Error)]
#[error("Data API {status} {}: {message} (trace_id {trace_id})", code.as_str())]
#[non_exhaustive]
pub struct V2Error {
    /// HTTP status.
    pub status: u16,
    /// Stable classification.
    pub code: ErrorCode,
    /// Human-readable message (the wire `error` field).
    pub message: String,
    /// Whether the server says the request may be retried unchanged.
    pub retryable: bool,
    /// Opaque id to quote when reporting a failure; also sent as `x-trace-id`.
    pub trace_id: String,
    /// The query parameter a validation failure concerns, when the server names one.
    pub parameter: Option<String>,
    /// The `Retry-After` delay, when the response carried one in seconds.
    pub retry_after: Option<Duration>,
}

#[derive(Deserialize)]
struct Body {
    error: String,
    code: ErrorCode,
    retryable: bool,
    trace_id: String,
    #[serde(default)]
    parameter: Option<String>,
}

impl V2Error {
    /// Parses a v2 error body; `None` when the body is some other shape (a v1
    /// `{"error"}` body, or Cloudflare's plain-text block page).
    ///
    /// The `Retry-After` is the response's, read by core unclamped: it is
    /// surfaced, never slept on.
    pub(crate) fn from_parts(response: &ErrorResponse) -> Option<Self> {
        let body: Body = serde_json::from_str(&response.body).ok()?;
        Some(Self {
            status: response.status.as_u16(),
            code: body.code,
            message: body.error,
            retryable: body.retryable,
            trace_id: body.trace_id,
            parameter: body.parameter,
            retry_after: response.retry_after,
        })
    }
}

/// The status rule, with `code` as the class's code and any status outside
/// 400–599 a [`Class::Decode`]. A 451 is not a fault: the venue does not serve
/// the caller's region, as designed.
///
/// The server's `retryable` flag is surfaced on the error, not obeyed: the
/// class comes from the status. [`Classify::retry_after`] is the response's
/// `Retry-After` whatever the status, and `None` when it was zero.
impl Classify for V2Error {
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// [`V2Error::from_parts`] on a response with `status`, `body` and, when
    /// given, a `Retry-After`.
    pub(crate) fn parts(status: u16, retry_after: Option<&str>, body: &str) -> Option<V2Error> {
        let mut headers = polyoxide_core::reqwest::header::HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(
                polyoxide_core::reqwest::header::RETRY_AFTER,
                value.parse().unwrap(),
            );
        }
        V2Error::from_parts(&ErrorResponse::new(
            polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),
            headers,
            body,
        ))
    }

    /// Captured from `GET /v2/user-pnl` with no `user`, 2026-09-14.
    const MISSING_USER: &str = r#"{"error":"required query param 'user' not provided","code":"invalid_request","parameter":"user","retryable":false,"trace_id":"8f8b7e5e64d241d1bc6e8d5eef76fc4e"}"#;

    #[test]
    fn parses_every_field_of_a_v2_body() {
        let err = parts(400, None, MISSING_USER).expect("a v2 body");

        assert_eq!(err.status, 400);
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(err.message, "required query param 'user' not provided");
        assert!(!err.retryable);
        assert_eq!(err.trace_id, "8f8b7e5e64d241d1bc6e8d5eef76fc4e");
        assert_eq!(err.parameter.as_deref(), Some("user"));
        assert_eq!(err.retry_after, None);
    }

    #[test]
    fn a_v1_body_or_plain_text_is_not_a_v2_error() {
        assert!(parts(
            400,
            None,
            r#"{"error":"required query param 'user' not provided"}"#
        )
        .is_none());
        assert!(parts(429, None, "error code: 1015").is_none());
        assert!(parts(500, None, "").is_none());
    }

    #[test]
    fn an_unrecognised_code_still_parses() {
        let body = r#"{"error":"new","code":"brand_new_code","retryable":true,"trace_id":"t"}"#;
        assert_eq!(parts(500, None, body).unwrap().code, ErrorCode::Unknown);
    }

    #[test]
    fn retry_after_is_read_in_seconds() {
        let body = r#"{"error":"slow down","code":"rate_limited","retryable":true,"trace_id":"t"}"#;
        let at = |header| parts(429, Some(header), body).unwrap().retry_after;

        assert_eq!(at("7"), Some(Duration::from_secs(7)));
        assert_eq!(at("1.5"), Some(Duration::from_millis(1500)));
        // The HTTP-date form is valid HTTP but not what this API sends.
        assert_eq!(at("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(at("-1"), None);
    }

    #[test]
    fn classifies_by_status_with_the_code_and_the_wait() {
        let body = |code: &str, retryable: bool| {
            format!(r#"{{"error":"x","code":"{code}","retryable":{retryable},"trace_id":"t"}}"#)
        };
        let code = |c: &str| Some(std::sync::Arc::from(c));
        // (status, Retry-After, wire code, server's retryable, class, is_fault,
        // retry_after())
        let rows = [
            (
                400,
                None,
                "invalid_request",
                false,
                Class::VenueRefusal {
                    code: code("invalid_request"),
                },
                true,
                None,
            ),
            (
                401,
                None,
                "unauthorized",
                false,
                Class::Unauthorized,
                true,
                None,
            ),
            (
                404,
                None,
                "not_found",
                false,
                Class::VenueRefusal {
                    code: code("not_found"),
                },
                true,
                None,
            ),
            (
                429,
                Some("7"),
                "rate_limited",
                true,
                Class::RateLimited {
                    retry_after: Some(Duration::from_secs(7)),
                },
                true,
                Some(Duration::from_secs(7)),
            ),
            (
                429,
                Some("0"),
                "rate_limited",
                true,
                Class::RateLimited { retry_after: None },
                true,
                None,
            ),
            // A region block is the venue answering as designed.
            (
                451,
                None,
                "invalid_request",
                false,
                Class::Restricted,
                false,
                None,
            ),
            (
                500,
                None,
                "brand_new_code",
                true,
                Class::Unavailable {
                    code: code("unknown"),
                },
                true,
                None,
            ),
            // The server's flag says no; the class still says a 503 is retriable.
            (
                503,
                Some("3"),
                "dependency_unavailable",
                false,
                Class::Unavailable {
                    code: code("dependency_unavailable"),
                },
                true,
                Some(Duration::from_secs(3)),
            ),
            (302, None, "internal", false, Class::Decode, true, None),
        ];
        for (status, retry_after, wire, retryable, class, fault, wait) in rows {
            let err = parts(status, retry_after, &body(wire, retryable)).unwrap();
            assert_eq!(err.class(), class, "{status} {wire}");
            assert_eq!(err.is_fault(), fault, "{status} {wire}");
            assert_eq!(Classify::retry_after(&err), wait, "{status} {wire}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{status} {wire}");
        }
    }

    #[test]
    fn display_names_the_code_and_trace_id() {
        let err = parts(400, None, MISSING_USER).unwrap();
        assert_eq!(
            err.to_string(),
            "Data API 400 invalid_request: required query param 'user' not provided (trace_id 8f8b7e5e64d241d1bc6e8d5eef76fc4e)"
        );
    }

    #[test]
    fn the_one_retry_after_parser_settles_the_old_disagreements() {
        // DRIFT R4. Data v2 read any finite value from zero up, so a zero or a
        // `-0` was a wait of nothing, and `1e300` panicked in
        // `Duration::from_secs_f64`.
        let body = r#"{"error":"slow down","code":"rate_limited","retryable":true,"trace_id":"t"}"#;
        let at = |header| parts(429, Some(header), body).unwrap().retry_after;
        assert_eq!(at("0"), None);
        assert_eq!(at("-0"), None);
        assert_eq!(at("1e300"), Some(Duration::MAX));
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
