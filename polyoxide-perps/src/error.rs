//! Error types for the Perps API.

use std::time::Duration;

use polyoxide_core::{retry_after_header, ApiError, RequestError};
use serde::Deserialize;
use thiserror::Error;

/// Error type for Perps API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PerpsError {
    /// Transport, decoding, or an error body in some other shape.
    #[error(transparent)]
    Api(#[from] ApiError),

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
    pub(crate) fn from_parts(status: u16, retry_after: Option<&str>, body: &str) -> Option<Self> {
        let body: Body = serde_json::from_str(body).ok()?;
        if body.status != "err" {
            return None;
        }
        Some(Self {
            status,
            code: body.error,
            reference: body.reference,
            retry_after: retry_after
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs),
        })
    }

    /// Whether re-sending the same request could plausibly succeed: a 408,
    /// a 425, a 429 or any 5xx.
    ///
    /// Mirrors [`ApiError::is_retriable`] status for status, so a response
    /// classifies the same whether or not its body had the venue shape;
    /// `venue_and_api_errors_agree_on_retriability` pins the two together.
    pub fn is_retriable(&self) -> bool {
        matches!(self.status, 408 | 425 | 429) || self.status >= 500
    }
}

impl PerpsError {
    /// Whether re-sending the same request could plausibly succeed.
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(err) => err.is_retriable(),
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

impl RequestError for PerpsError {
    async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let retry_after = retry_after_header(&response);
        let body = response.text().await.unwrap_or_default();

        // Told apart by body shape, not by path or status.
        match VenueError::from_parts(status, retry_after.as_deref(), &body) {
            Some(err) => Self::Venue(err),
            None => Self::Api(ApiError::from_status_and_body(status, &body)),
        }
    }
}

polyoxide_core::impl_api_error_conversions!(PerpsError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_404_body_becomes_a_venue_error_with_its_identifier() {
        let err = VenueError::from_parts(404, None, r#"{"status":"err","error":"not_found"}"#)
            .expect("venue shape");
        assert_eq!(err.code, "not_found");
        assert_eq!(err.reference, None);
        assert!(!err.is_retriable());
    }

    #[test]
    fn a_400_body_keeps_the_gateway_reference() {
        // Captured 2026-09-30: validation failures carry `arts`, `ts` and `ref`
        // beyond the schema's two fields.
        let body = r#"{"status":"err","error":"invalid query parameters: missing field `instrument_id`","arts":1790758475821,"ts":1790758475821,"ref":"g-1224ed1744735"}"#;
        let err = VenueError::from_parts(400, None, body).expect("venue shape");
        assert_eq!(err.reference.as_deref(), Some("g-1224ed1744735"));
        assert!(err.code.starts_with("invalid query parameters"));
    }

    #[test]
    fn a_429_is_retriable_and_reads_retry_after_as_whole_seconds() {
        let err = PerpsError::Venue(
            VenueError::from_parts(
                429,
                Some("2"),
                r#"{"status":"err","error":"ip_rate_limited"}"#,
            )
            .unwrap(),
        );
        assert!(err.is_retriable());
        assert_eq!(err.code(), Some("ip_rate_limited"));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(2)));

        let too_early =
            VenueError::from_parts(425, None, r#"{"status":"err","error":"too_early"}"#).unwrap();
        assert!(too_early.is_retriable());
    }

    #[test]
    fn a_body_without_status_err_is_not_a_venue_error() {
        assert_eq!(
            VenueError::from_parts(200, None, r#"{"status":"ok"}"#),
            None
        );
        assert_eq!(
            VenueError::from_parts(502, None, "<html>bad gateway</html>"),
            None
        );
    }

    #[test]
    fn api_errors_have_no_code_or_retry_after() {
        let err = PerpsError::from(ApiError::Timeout);
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
            let venue = VenueError::from_parts(status, None, body).unwrap();
            let api = ApiError::from_status_and_body(status, body);
            assert_eq!(
                venue.is_retriable(),
                api.is_retriable(),
                "status {status}: VenueError says {} but ApiError says {}",
                venue.is_retriable(),
                api.is_retriable()
            );
        }
    }

    #[test]
    fn perps_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PerpsError>();
    }
}
