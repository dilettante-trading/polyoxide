use thiserror::Error;

/// Error types for relay operations.
///
/// Wraps underlying HTTP, serialization, and signing errors, plus relay-specific
/// API failures and rate limiting.
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

    #[error("Relayer API error: {0}")]
    Api(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Missing signer")]
    MissingSigner,

    #[error("Core API error: {0}")]
    Core(#[from] polyoxide_core::ApiError),
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
        let err = RelayError::Api("server returned 500".into());
        assert_eq!(format!("{err}"), "Relayer API error: server returned 500");
    }

    #[test]
    fn test_rate_limit_display() {
        let err = RelayError::RateLimit;
        assert_eq!(format!("{err}"), "Rate limit exceeded");
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
            RelayError::Core(_) => {}
            other => panic!("Expected Core, got: {other:?}"),
        }
    }
}
