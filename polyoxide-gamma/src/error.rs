use polyoxide_core::ApiError;
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

/// Error types for gamma API operations
///
/// Gamma answers with Polymarket's `error` or `message` body, which core's
/// [`ErrorResponse`](polyoxide_core::ErrorResponse) already reads, so the
/// derived `From<ApiError>` is gamma's decode.
#[derive(Error, Debug)]
pub enum GammaError {
    /// Core API error
    #[error(transparent)]
    Api(#[from] ApiError),
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

    /// A response with `status` and no body.
    fn response(status: u16) -> ApiError {
        polyoxide_core::ErrorResponse::new(
            reqwest::StatusCode::from_u16(status).unwrap(),
            Default::default(),
            "",
        )
        .into()
    }

    #[test]
    fn every_variant_classifies_as_the_api_error_it_wraps() {
        let rows = [
            (response(408), Class::Unavailable { code: None }),
            (response(400), Class::VenueRefusal { code: None }),
            (ApiError::Validation("bad".into()), Class::InvalidRequest),
            (response(429), Class::RateLimited { retry_after: None }),
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
