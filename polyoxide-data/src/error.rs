use std::time::Duration;

use polyoxide_core::{retry_after_header, ApiError, RequestError};
use thiserror::Error;

use crate::v2::V2Error;

/// Error types for Data API operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum DataApiError {
    /// Core API error: transport, decoding, or a non-v2 error body.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// A Data API v2 error, with the structured fields the server sent.
    #[error(transparent)]
    V2(#[from] V2Error),

    /// A `.pages()` walk could not continue.
    #[error("Pagination error: {0}")]
    Pagination(String),
}

impl DataApiError {
    /// Whether re-sending the same request could plausibly succeed.
    ///
    /// For [`DataApiError::V2`] this is the server's own `retryable` flag, which
    /// takes precedence over any reading of the status code. Otherwise it
    /// defers to [`ApiError::is_retriable`].
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(err) => err.is_retriable(),
            Self::V2(err) => err.retryable,
            Self::Pagination(_) => false,
        }
    }

    /// The server's trace id, for v2 errors.
    pub fn trace_id(&self) -> Option<&str> {
        match self {
            Self::V2(err) => Some(&err.trace_id),
            _ => None,
        }
    }

    /// The `Retry-After` delay, for v2 errors that carried one.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::V2(err) => err.retry_after,
            _ => None,
        }
    }
}

impl RequestError for DataApiError {
    async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let retry_after = retry_after_header(&response);
        let body = response.text().await.unwrap_or_default();

        // Told apart by body shape, not by path: v1 routes send `{"error"}`,
        // v2 routes add `code`, `retryable` and `trace_id`, and Cloudflare's
        // block page is plain text.
        match V2Error::from_parts(status, retry_after.as_deref(), &body) {
            Some(err) => Self::V2(err),
            None => Self::Api(ApiError::from_status_and_body(status, &body)),
        }
    }
}

// Implement standard error conversions using the macro
polyoxide_core::impl_api_error_conversions!(DataApiError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_api_error_from_api_error() {
        let api_err = ApiError::Api {
            status: 404,
            message: "not found".to_string(),
        };
        let data_err = DataApiError::from(api_err);
        let msg = format!("{}", data_err);
        assert!(
            msg.contains("not found"),
            "DataApiError should forward ApiError message: {}",
            msg
        );
    }

    #[test]
    fn a_v2_error_reports_the_servers_retryable_flag_and_trace_id() {
        let body = r#"{"error":"down","code":"dependency_unavailable","retryable":false,"trace_id":"t-1"}"#;
        let err = DataApiError::V2(V2Error::from_parts(503, Some("3"), body).unwrap());

        assert!(
            !err.is_retriable(),
            "the server's flag wins over the 503 status"
        );
        assert_eq!(err.trace_id(), Some("t-1"));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(3)));
    }

    #[test]
    fn other_errors_have_no_trace_id_or_retry_after() {
        let timeout = DataApiError::from(ApiError::Timeout);
        assert!(timeout.is_retriable());
        assert_eq!(timeout.trace_id(), None);
        assert_eq!(timeout.retry_after(), None);

        let walk = DataApiError::Pagination("server returned the cursor it was sent".into());
        assert!(!walk.is_retriable());
    }

    #[test]
    fn data_api_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // DataApiError wraps ApiError which should be Send + Sync
        // This is a compile-time check
        assert_send_sync::<DataApiError>();
    }
}
