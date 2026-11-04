use polyoxide_core::ApiError;
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

/// Error types for gamma API operations
///
/// Gamma answers with Polymarket's `error` or `message` body, which core's
/// [`ErrorResponse`](polyoxide_core::ErrorResponse) already reads, so the
/// derived `From<ApiError>` is gamma's decode.
#[derive(Error, Debug)]
#[non_exhaustive]
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

/// Why [`Market::outcome_ids`](super::types::Market::outcome_ids) found no
/// ids a caller can trade.
///
/// Every variant is a response that arrived but carries no usable ids, so each
/// classifies as [`Class::Decode`]. It is not a [`GammaError`] variant:
/// `GammaError` decodes only through its one `From<ApiError>`.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutcomeIdsError {
    /// The market has no `version`, so the field to read cannot be chosen.
    #[error("the market has no `version`, so its outcome ids cannot be chosen")]
    MissingVersion,
    /// The market's `version` is neither `v1` nor `v2`. Carries the value.
    #[error("the market's protocol version `{0}` is not supported")]
    UnsupportedVersion(String),
    /// The field the version names is absent: `clobTokenIds` for `v1`,
    /// `positionIds` for `v2`.
    #[error("the market carries no outcome ids for its protocol version")]
    MissingIds,
    /// A `v1` market's `clobTokenIds` is not a JSON array of strings.
    #[error("the market's `clobTokenIds` is not a JSON array of strings")]
    MalformedIds,
    /// The market's `outcomes` is absent or not a JSON array of strings.
    #[error("the market's `outcomes` is absent or not a JSON array of strings")]
    MalformedOutcomes,
    /// The market has a different number of outcomes and ids.
    #[error("the market has {outcomes} outcomes and {ids} outcome ids")]
    CountMismatch {
        /// How many labels `outcomes` holds.
        outcomes: usize,
        /// How many ids the version's field holds.
        ids: usize,
    },
    /// An id is empty or not all ASCII digits. Carries the id.
    #[error("the outcome id `{0}` is not a decimal integer")]
    InvalidId(String),
}

/// Every variant is a response that arrived but carries no ids the client can
/// trade.
impl Classify for OutcomeIdsError {
    fn class(&self) -> Class {
        Class::Decode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response with `status` and no body.
    fn response(status: u16) -> ApiError {
        polyoxide_core::ErrorResponse::new(
            polyoxide_core::http::StatusCode::from_u16(status).unwrap(),
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
