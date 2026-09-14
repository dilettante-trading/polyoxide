//! The structured error body every unsuccessful v2 response carries.

use std::time::Duration;

use serde::Deserialize;

/// Stable classification of a v2 failure, for programmatic branching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorCode {
    /// `invalid_request` (400)
    InvalidRequest,
    /// `unauthorized` (401)
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
    pub(crate) fn from_parts(status: u16, retry_after: Option<&str>, body: &str) -> Option<Self> {
        let body: Body = serde_json::from_str(body).ok()?;
        Some(Self {
            status,
            code: body.code,
            message: body.error,
            retryable: body.retryable,
            trace_id: body.trace_id,
            parameter: body.parameter,
            retry_after: retry_after
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|secs| secs.is_finite() && *secs >= 0.0)
                .map(Duration::from_secs_f64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from `GET /v2/user-pnl` with no `user`, 2026-09-14.
    const MISSING_USER: &str = r#"{"error":"required query param 'user' not provided","code":"invalid_request","parameter":"user","retryable":false,"trace_id":"8f8b7e5e64d241d1bc6e8d5eef76fc4e"}"#;

    #[test]
    fn parses_every_field_of_a_v2_body() {
        let err = V2Error::from_parts(400, None, MISSING_USER).expect("a v2 body");

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
        assert!(V2Error::from_parts(
            400,
            None,
            r#"{"error":"required query param 'user' not provided"}"#
        )
        .is_none());
        assert!(V2Error::from_parts(429, None, "error code: 1015").is_none());
        assert!(V2Error::from_parts(500, None, "").is_none());
    }

    #[test]
    fn an_unrecognised_code_still_parses() {
        let body = r#"{"error":"new","code":"brand_new_code","retryable":true,"trace_id":"t"}"#;
        assert_eq!(
            V2Error::from_parts(500, None, body).unwrap().code,
            ErrorCode::Unknown
        );
    }

    #[test]
    fn retry_after_is_read_in_seconds() {
        let body = r#"{"error":"slow down","code":"rate_limited","retryable":true,"trace_id":"t"}"#;
        let at = |header| {
            V2Error::from_parts(429, Some(header), body)
                .unwrap()
                .retry_after
        };

        assert_eq!(at("7"), Some(Duration::from_secs(7)));
        assert_eq!(at("1.5"), Some(Duration::from_millis(1500)));
        // The HTTP-date form is valid HTTP but not what this API sends.
        assert_eq!(at("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(at("-1"), None);
    }

    #[test]
    fn display_names_the_code_and_trace_id() {
        let err = V2Error::from_parts(400, None, MISSING_USER).unwrap();
        assert_eq!(
            err.to_string(),
            "Data API 400 invalid_request: required query param 'user' not provided (trace_id 8f8b7e5e64d241d1bc6e8d5eef76fc4e)"
        );
    }
}
