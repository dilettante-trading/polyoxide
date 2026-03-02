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
}
