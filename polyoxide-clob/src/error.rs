use polyoxide_core::ApiError;
use thiserror::Error;

use crate::types::ParseTickSizeError;

/// Error types for CLOB API operations
#[derive(Error, Debug)]
pub enum ClobError {
    /// Core API error
    #[error(transparent)]
    Api(#[from] ApiError),

    /// Cryptographic operation failed
    #[error("Crypto error: {0}")]
    Crypto(String),

    /// Alloy (Ethereum library) error
    #[error("Alloy error: {0}")]
    Alloy(String),

    /// Invalid tick size
    #[error(transparent)]
    InvalidTickSize(#[from] ParseTickSizeError),
}

impl ClobError {
    /// Create error from HTTP response
    pub(crate) async fn from_response(response: reqwest::Response) -> Self {
        Self::Api(ApiError::from_response(response).await)
    }

    /// Create validation error
    pub(crate) fn validation(msg: impl Into<String>) -> Self {
        Self::Api(ApiError::Validation(msg.into()))
    }

    /// Create service error (external dependency failure)
    #[cfg_attr(not(feature = "gamma"), allow(dead_code))]
    pub(crate) fn service(msg: impl Into<String>) -> Self {
        Self::Api(ApiError::Api {
            status: 0,
            message: msg.into(),
        })
    }
}

impl From<alloy::signers::Error> for ClobError {
    fn from(err: alloy::signers::Error) -> Self {
        Self::Alloy(err.to_string())
    }
}

impl From<alloy::hex::FromHexError> for ClobError {
    fn from(err: alloy::hex::FromHexError) -> Self {
        Self::Alloy(err.to_string())
    }
}

impl From<reqwest::Error> for ClobError {
    fn from(err: reqwest::Error) -> Self {
        Self::Api(ApiError::Network(err))
    }
}

impl From<url::ParseError> for ClobError {
    fn from(err: url::ParseError) -> Self {
        Self::Api(ApiError::Url(err))
    }
}

impl From<serde_json::Error> for ClobError {
    fn from(err: serde_json::Error) -> Self {
        Self::Api(ApiError::Serialization(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_error_is_api_not_validation() {
        let err = ClobError::service("Gamma client failed");
        match &err {
            ClobError::Api(ApiError::Api { status, message }) => {
                assert_eq!(*status, 0);
                assert_eq!(message, "Gamma client failed");
            }
            other => panic!("Expected ApiError::Api, got {:?}", other),
        }
    }

    #[test]
    fn test_validation_error() {
        let err = ClobError::validation("bad input");
        match &err {
            ClobError::Api(ApiError::Validation(msg)) => {
                assert_eq!(msg, "bad input");
            }
            other => panic!("Expected ApiError::Validation, got {:?}", other),
        }
    }

    #[test]
    fn test_service_and_validation_are_distinct() {
        let service = ClobError::service("service failure");
        let validation = ClobError::validation("validation failure");

        let service_msg = format!("{}", service);
        let validation_msg = format!("{}", validation);

        // They should produce different Display output
        assert_ne!(service_msg, validation_msg);
        assert!(service_msg.contains("service failure"));
        assert!(validation_msg.contains("validation failure"));
    }

    #[test]
    fn test_crypto_error() {
        let err = ClobError::Crypto("signing failed".into());
        assert!(err.to_string().contains("signing failed"));
        assert!(matches!(err, ClobError::Crypto(_)));
    }

    #[test]
    fn test_alloy_error() {
        let err = ClobError::Alloy("hex decode failed".into());
        assert!(err.to_string().contains("hex decode failed"));
        assert!(matches!(err, ClobError::Alloy(_)));
    }

    #[test]
    fn test_invalid_tick_size_from_str() {
        let err: Result<crate::types::TickSize, _> = "0.5".try_into();
        let clob_err = ClobError::from(err.unwrap_err());
        assert!(matches!(clob_err, ClobError::InvalidTickSize(_)));
        assert!(clob_err.to_string().contains("0.5"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("not valid json").unwrap_err();
        let clob_err = ClobError::from(json_err);
        assert!(matches!(
            clob_err,
            ClobError::Api(ApiError::Serialization(_))
        ));
    }

    #[test]
    fn test_from_url_parse_error() {
        let url_err = url::Url::parse("://bad").unwrap_err();
        let clob_err = ClobError::from(url_err);
        assert!(matches!(clob_err, ClobError::Api(ApiError::Url(_))));
    }
}
