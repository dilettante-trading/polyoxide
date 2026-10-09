use polyoxide_core::ApiError;
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

/// Error types for relay operations.
///
/// Wraps underlying HTTP, serialization, and signing errors. Every failure of
/// a request to the relayer, and every local refusal, is an [`ApiError`] in
/// [`Api`](RelayError::Api).
#[derive(Error, Debug)]
pub enum RelayError {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

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

/// A relayer response that failed, read as core reads one, so a
/// [`HttpClient::health`](polyoxide_core::HttpClient::health) ping fails as
/// every other route does.
impl polyoxide_core::RequestError for RelayError {
    async fn from_response(response: reqwest::Response) -> Self {
        Self::Api(ApiError::from_response(response).await)
    }
}

/// A transport failure by core's reqwest rule, a local signing or URL failure
/// an `InvalidRequest`, a response that did not parse a `Decode`, and `Api`
/// as core classes it: a relayer response by its status, and a local refusal a
/// `VenueRefusal` until Story 3.11 moves it to `InvalidRequest`.
impl Classify for RelayError {
    fn class(&self) -> Class {
        match self {
            Self::Reqwest(err) => polyoxide_core::error::classify_reqwest(err),
            Self::UrlParse(_) | Self::Signer(_) | Self::MissingSigner => Class::InvalidRequest,
            Self::SerdeJson(_) => Class::Decode,
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

    #[test]
    fn test_signer_error_display() {
        let err = RelayError::Signer("bad key".into());
        assert_eq!(format!("{err}"), "Signer error: bad key");
    }

    #[test]
    fn test_api_error_display() {
        let err = RelayError::Api(ApiError::Api {
            status: 500,
            message: "server returned 500".into(),
        });
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
        let relay_err: RelayError = url_err.into();
        match relay_err {
            RelayError::UrlParse(_) => {}
            other => panic!("Expected UrlParse, got: {other:?}"),
        }
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let relay_err: RelayError = json_err.into();
        match relay_err {
            RelayError::SerdeJson(_) => {}
            other => panic!("Expected SerdeJson, got: {other:?}"),
        }
    }

    #[test]
    fn test_from_core_api_error() {
        let core_err = polyoxide_core::ApiError::Timeout;
        let relay_err: RelayError = core_err.into();
        match relay_err {
            RelayError::Api(_) => {}
            other => panic!("Expected Api, got: {other:?}"),
        }
    }

    #[test]
    fn every_variant_classifies() {
        let builder = reqwest::Client::new().get("not a url").build().unwrap_err();
        let rows = [
            (RelayError::Reqwest(builder), Class::InvalidRequest),
            (
                RelayError::UrlParse(url::ParseError::EmptyHost),
                Class::InvalidRequest,
            ),
            (
                RelayError::SerdeJson(serde_json::from_str::<String>("x").unwrap_err()),
                Class::Decode,
            ),
            (RelayError::Signer("bad key".into()), Class::InvalidRequest),
            (
                RelayError::Api(ApiError::Api {
                    status: 503,
                    message: "server returned 503".into(),
                }),
                Class::Unavailable { code: None },
            ),
            (
                RelayError::Api(ApiError::RateLimit("slow down".into())),
                Class::RateLimited { retry_after: None },
            ),
            (RelayError::MissingSigner, Class::InvalidRequest),
            (
                RelayError::validation("idempotency key must not be empty"),
                Class::VenueRefusal { code: None },
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
