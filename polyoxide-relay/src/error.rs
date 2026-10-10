use polyoxide_core::ApiError;
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

/// Error types for relay operations.
///
/// Wraps underlying HTTP, serialization, and signing errors. Every failure of
/// a request to the relayer, every local refusal, and every transport, URL
/// and JSON failure is an [`ApiError`] in [`Api`](RelayError::Api). The
/// relayer answers with Polymarket's `error` or `message` body, which core's
/// [`ErrorResponse`](polyoxide_core::ErrorResponse) already reads, so the
/// derived `From<ApiError>` is relay's decode.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum RelayError {
    #[error("Signer error: {0}")]
    Signer(String),

    /// A request to the relayer that failed, classed by its status, or a
    /// refusal made before anything was sent ([`ApiError::Validation`]).
    #[error(transparent)]
    Api(#[from] ApiError),

    #[error("Missing signer")]
    MissingSigner,
}

impl RelayError {
    /// A request refused before anything was sent: bad input, missing auth or
    /// configuration, or a header that could not be built.
    pub(crate) fn validation(msg: impl Into<String>) -> Self {
        Self::Api(ApiError::Validation(msg.into()))
    }
}

/// A local signing failure is an `InvalidRequest`, and `Api` is as core
/// classes it: a relayer response by its status, a transport failure by
/// core's reqwest rule, a URL failure or a local refusal an `InvalidRequest`,
/// and JSON that did not parse a `Decode`.
impl Classify for RelayError {
    fn class(&self) -> Class {
        match self {
            Self::Signer(_) | Self::MissingSigner => Class::InvalidRequest,
            Self::Api(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            _ => true,
        }
    }

    fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response with `status` and the body `{"error": message}`.
    fn response(status: u16, message: &str) -> ApiError {
        polyoxide_core::ErrorResponse::new(
            reqwest::StatusCode::from_u16(status).unwrap(),
            Default::default(),
            serde_json::json!({ "error": message }).to_string(),
        )
        .into()
    }

    #[test]
    fn test_signer_error_display() {
        let err = RelayError::Signer("bad key".into());
        assert_eq!(format!("{err}"), "Signer error: bad key");
    }

    #[test]
    fn test_api_error_display() {
        let err = RelayError::Api(response(500, "server returned 500"));
        assert_eq!(format!("{err}"), "API error: 500 - server returned 500");
        let err = RelayError::validation("idempotency key must not be empty");
        assert_eq!(
            format!("{err}"),
            "Validation error: idempotency key must not be empty"
        );
    }

    #[test]
    fn test_missing_signer_display() {
        let err = RelayError::MissingSigner;
        assert_eq!(format!("{err}"), "Missing signer");
    }

    #[test]
    fn test_from_url_parse_error() {
        let url_err: url::ParseError = url::Url::parse("://bad").unwrap_err();
        let relay_err: RelayError = ApiError::from(url_err).into();
        match relay_err {
            RelayError::Api(ApiError::Url(_)) => {}
            other => panic!("Expected Api(Url), got: {other:?}"),
        }
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let relay_err: RelayError = ApiError::from(json_err).into();
        match relay_err {
            RelayError::Api(ApiError::Serialization(_)) => {}
            other => panic!("Expected Api(Serialization), got: {other:?}"),
        }
    }

    #[test]
    fn test_from_core_api_error() {
        let core_err = response(408, "timeout");
        let relay_err: RelayError = core_err.into();
        match relay_err {
            RelayError::Api(_) => {}
            other => panic!("Expected Api, got: {other:?}"),
        }
    }

    #[test]
    fn every_variant_classifies() {
        let builder = reqwest::Client::new().get("not a url").build().unwrap_err();
        // Was `Reqwest`, `UrlParse` and `SerdeJson` until Story 3.11.
        let rows = [
            (
                RelayError::Api(ApiError::Network(builder)),
                Class::InvalidRequest,
            ),
            (
                RelayError::Api(ApiError::Url(url::ParseError::EmptyHost)),
                Class::InvalidRequest,
            ),
            (
                RelayError::Api(ApiError::Serialization(
                    serde_json::from_str::<String>("x").unwrap_err(),
                )),
                Class::Decode,
            ),
            (RelayError::Signer("bad key".into()), Class::InvalidRequest),
            (
                RelayError::Api(response(503, "server returned 503")),
                Class::Unavailable { code: None },
            ),
            (
                RelayError::Api(response(429, "slow down")),
                Class::RateLimited { retry_after: None },
            ),
            (RelayError::MissingSigner, Class::InvalidRequest),
            // A refusal made before sending (Story 3.11).
            (
                RelayError::validation("idempotency key must not be empty"),
                Class::InvalidRequest,
            ),
        ];
        for (err, class) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
