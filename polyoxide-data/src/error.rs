use std::time::Duration;

use polyoxide_core::{retry_after_header, ApiError, RequestError};
use polyoxide_venue::{Class, Classify};
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

/// `Api` and `V2` delegate to the error they wrap, and a pagination failure,
/// a page that did not continue the walk, is a [`Class::Decode`].
///
/// For a v2 error the class comes from the status, so it can disagree with
/// [`DataApiError::is_retriable`], which keeps the server's `retryable` flag.
impl Classify for DataApiError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::V2(err) => err.class(),
            Self::Pagination(_) => Class::Decode,
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            Self::V2(err) => err.is_fault(),
            Self::Pagination(_) => true,
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            Self::V2(err) => Classify::retry_after(err),
            Self::Pagination(_) => None,
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
    fn every_variant_classifies() {
        let v2 = |status, retry_after, retryable: bool| {
            let body = format!(
                r#"{{"error":"x","code":"dependency_unavailable","retryable":{retryable},"trace_id":"t"}}"#
            );
            DataApiError::V2(V2Error::from_parts(status, retry_after, &body).unwrap())
        };
        let unavailable = Class::Unavailable {
            code: Some("dependency_unavailable".into()),
        };
        let secs = |n| Some(Duration::from_secs(n));
        // (error, class, is_fault, (trait retry_after, inherent retry_after),
        // inherent is_retriable)
        let rows = [
            (
                DataApiError::from(ApiError::Timeout),
                Class::Unavailable { code: None },
                true,
                (None, None),
                true,
            ),
            (
                DataApiError::from(ApiError::Validation("bad".into())),
                Class::VenueRefusal { code: None },
                true,
                (None, None),
                false,
            ),
            // The server's flag and the class disagree here, and each keeps
            // its own answer.
            (
                v2(503, Some("3"), false),
                unavailable.clone(),
                true,
                (secs(3), secs(3)),
                false,
            ),
            // A zero is no wait, to the trait and the inherent method alike
            // (DRIFT R4).
            (
                v2(503, Some("0"), true),
                unavailable,
                true,
                (None, None),
                true,
            ),
            (
                v2(451, None, false),
                Class::Restricted,
                false,
                (None, None),
                false,
            ),
            (
                DataApiError::Pagination("the server returned the cursor it was sent".into()),
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
        }
    }

    #[test]
    fn data_api_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // DataApiError wraps ApiError which should be Send + Sync
        // This is a compile-time check
        assert_send_sync::<DataApiError>();
    }
}
