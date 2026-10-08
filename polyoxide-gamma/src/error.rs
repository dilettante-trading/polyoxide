use polyoxide_core::{ApiError, RequestError};
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

/// Error types for gamma API operations
#[derive(Error, Debug)]
pub enum GammaError {
    /// Core API error
    #[error(transparent)]
    Api(#[from] ApiError),
}

impl RequestError for GammaError {
    async fn from_response(response: reqwest::Response) -> Self {
        Self::Api(ApiError::from_response(response).await)
    }
}

/// Delegates to [`ApiError`]'s classification.
impl Classify for GammaError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
        }
    }

    fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
        }
    }
}

// Implement standard error conversions using the macro
polyoxide_core::impl_api_error_conversions!(GammaError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_classifies_as_the_api_error_it_wraps() {
        let rows = [
            (ApiError::Timeout, Class::Unavailable { code: None }),
            (
                ApiError::Validation("bad".into()),
                Class::VenueRefusal { code: None },
            ),
            (
                ApiError::RateLimit("slow".into()),
                Class::RateLimited { retry_after: None },
            ),
        ];
        for (api, class) in rows {
            let err = GammaError::from(api);
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
