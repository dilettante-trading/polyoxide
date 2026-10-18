//! Errors of the Predictions module.

use std::time::Duration;

use polyoxide_core::{ApiError, ErrorResponse};
use polyoxide_venue::{class_for_status, Class, Classify};
use serde_json::Value;
use thiserror::Error;

/// Error type for Predictions operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PredictionsError {
    /// Transport, decoding, a refusal made before sending, or an error body in a shape Kalshi does
    /// not use.
    #[error(transparent)]
    Api(ApiError),

    /// Kalshi refused the request with one of its three error bodies.
    #[error(transparent)]
    Venue(VenueError),
}

/// An unsuccessful response whose body is one of Kalshi's three error shapes.
///
/// The status and the `Retry-After` are the response's; the rest is read from the body, which is
/// `{"error": {"code", "message", "details"}}` (a `404` or a `401`), `{"error": "<text>"}` (a
/// `429`) or `{"msg": "<text>"}` (a parameter-validation `400`). The last two give a message
/// alone.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
#[error("Kalshi answered {status}: {message}")]
pub struct VenueError {
    /// The HTTP status.
    pub status: u16,
    /// Kalshi's error code, such as `not_found`, when the body carries one.
    pub code: Option<String>,
    /// Kalshi's message.
    pub message: String,
    /// Kalshi's additional details, when the body carries them.
    pub details: Option<String>,
    /// The response's `Retry-After`, as core reads it: unclamped, and `None` for a zero. Kalshi
    /// sends none with a `429`.
    pub retry_after: Option<Duration>,
}

impl VenueError {
    /// The error `response` carries, when its body is one of the three shapes.
    ///
    /// The body is read, never the message core derived from it, since core keeps only a string
    /// `error` field. A nested `error` without a `message`, an `error` of another type and a body
    /// of any other shape are not Kalshi's, and give `None`.
    fn read(response: &ErrorResponse) -> Option<Self> {
        let body: Value = serde_json::from_str(&response.body).ok()?;
        let fields = body.as_object()?;
        let (code, message, details) = match fields.get("error") {
            Some(Value::Object(error)) => (
                text(error.get("code")),
                text(error.get("message"))?,
                text(error.get("details")),
            ),
            Some(Value::String(message)) => (None, message.clone(), None),
            Some(_) => return None,
            None => (None, text(fields.get("msg"))?, None),
        };
        Some(Self {
            status: response.status.as_u16(),
            code,
            message,
            details,
            retry_after: response.retry_after,
        })
    }
}

/// A body field, when it is a string.
fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

/// Kalshi's one decode: an unsuccessful response whose body is one of its three error shapes is
/// [`Venue`](PredictionsError::Venue), and every other error stays core's
/// [`Api`](PredictionsError::Api), an unsuccessful response with any other body included.
///
/// A signing failure stays `Api(ApiError::Sign(..))`. Foreign errors enter the module only through
/// [`ApiError`], so a `?` on core's error anywhere decodes the same way.
impl From<ApiError> for PredictionsError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Response(response) => match VenueError::read(&response) {
                Some(venue) => Self::Venue(venue),
                None => Self::Api(ApiError::Response(response)),
            },
            other => Self::Api(other),
        }
    }
}

/// `Api` delegates to core.
impl Classify for PredictionsError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::Venue(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            Self::Venue(err) => err.is_fault(),
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            Self::Venue(err) => Classify::retry_after(err),
        }
    }
}

/// The class goes by the status before the body (AD-15), through the workspace's status map:
/// `401` and `403` are `Unauthorized`, `418` and `451` `Restricted`, `429` `RateLimited`, `408`,
/// `425` and every 5xx `Unavailable`, and any other 4xx a `VenueRefusal`; a status outside 400 to
/// 599 is a `Decode`. Kalshi's `code` is the class's code where the class has room for one, and a
/// `429` carries the response's `Retry-After`.
///
/// A `451` is the venue not serving the caller's region, as designed, so it is not a fault; every
/// other status is.
impl Classify for VenueError {
    fn class(&self) -> Class {
        let class = class_for_status(self.status).unwrap_or(Class::Decode);
        let class = match &self.code {
            Some(code) => class.with_code(code.as_str()),
            None => class,
        };
        class.with_retry_after(self.retry_after)
    }

    fn is_fault(&self) -> bool {
        self.status != 451
    }

    fn retry_after(&self) -> Option<Duration> {
        self.retry_after.filter(|wait| !wait.is_zero())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use polyoxide_core::reqwest::header::{HeaderMap, RETRY_AFTER};
    use polyoxide_core::reqwest::StatusCode;

    use super::*;

    /// A response with `status`, `body` and, when given, a `Retry-After`, as core builds it.
    fn response(status: u16, retry_after: Option<&str>, body: &str) -> ApiError {
        let mut headers = HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(RETRY_AFTER, value.parse().unwrap());
        }
        ErrorResponse::new(StatusCode::from_u16(status).unwrap(), headers, body).into()
    }

    /// That response, through Kalshi's decode.
    fn decoded(status: u16, retry_after: Option<&str>, body: &str) -> PredictionsError {
        PredictionsError::from(response(status, retry_after, body))
    }

    /// The `VenueError` a response decodes to.
    fn venue(status: u16, retry_after: Option<&str>, body: &str) -> VenueError {
        match decoded(status, retry_after, body) {
            PredictionsError::Venue(venue) => venue,
            other => panic!("expected a Kalshi error, got {other:?}"),
        }
    }

    // Captured from the production host on 2026-10-18.
    const NOT_FOUND: &str = r#"{"error":{"code":"not_found","message":"not found"}}"#;
    const UNAUTHENTICATED: &str = concat!(
        r#"{"error":{"code":"token_authentication_failure","#,
        r#""message":"token authentication failure"}}"#,
    );
    const BAD_PARAMETER: &str = concat!(
        r#"{"msg":"Invalid format for parameter limit: error binding string parameter: "#,
        r#"strconv.ParseInt: parsing \"abc\": invalid syntax"}"#,
    );
    // The body Kalshi's documentation gives a `429`.
    const TOO_MANY: &str = r#"{"error":"too many requests"}"#;

    #[test]
    fn a_nested_error_gives_its_code_message_and_details() {
        let err = venue(404, None, NOT_FOUND);
        assert_eq!(err.status, 404);
        assert_eq!(err.code.as_deref(), Some("not_found"));
        assert_eq!(err.message, "not found");
        assert_eq!(err.details, None);
        assert_eq!(err.retry_after, None);
        assert_eq!(err.to_string(), "Kalshi answered 404: not found");

        let detailed = venue(
            400,
            None,
            r#"{"error":{"code":"invalid_order","message":"bad price","details":"price is 0"}}"#,
        );
        assert_eq!(detailed.code.as_deref(), Some("invalid_order"));
        assert_eq!(detailed.message, "bad price");
        assert_eq!(detailed.details.as_deref(), Some("price is 0"));
    }

    #[test]
    fn a_string_error_gives_its_message_alone() {
        let err = venue(429, None, TOO_MANY);
        assert_eq!(err.status, 429);
        assert_eq!(err.code, None);
        assert_eq!(err.message, "too many requests");
        assert_eq!(err.details, None);
    }

    #[test]
    fn a_msg_body_gives_its_message_alone() {
        let err = venue(400, None, BAD_PARAMETER);
        assert_eq!(err.code, None);
        assert_eq!(err.details, None);
        assert!(
            err.message
                .starts_with("Invalid format for parameter limit"),
            "{err:?}"
        );
        assert!(err.message.ends_with("invalid syntax"), "{err:?}");
        // The shape the spec's first probe recorded.
        let probed = venue(
            400,
            None,
            r#"{"msg":"Parameter validation failed for limit"}"#,
        );
        assert_eq!(probed.message, "Parameter validation failed for limit");
    }

    #[test]
    fn the_status_decides_the_class_before_the_body() {
        let code = |text: &str| Some(Arc::<str>::from(text));
        let refusal = |code| Class::VenueRefusal { code };
        let unavailable = |code| Class::Unavailable { code };
        // (status, body, class, is_fault)
        let rows = [
            (400, BAD_PARAMETER, refusal(None), true),
            (401, UNAUTHENTICATED, Class::Unauthorized, true),
            (403, NOT_FOUND, Class::Unauthorized, true),
            (404, NOT_FOUND, refusal(code("not_found")), true),
            (408, NOT_FOUND, unavailable(code("not_found")), true),
            (418, TOO_MANY, Class::Restricted, true),
            (425, TOO_MANY, unavailable(None), true),
            (
                429,
                TOO_MANY,
                Class::RateLimited { retry_after: None },
                true,
            ),
            (451, NOT_FOUND, Class::Restricted, false),
            (500, NOT_FOUND, unavailable(code("not_found")), true),
            (503, BAD_PARAMETER, unavailable(None), true),
        ];
        for (status, body, class, fault) in rows {
            let err = decoded(status, None, body);
            assert!(
                matches!(err, PredictionsError::Venue(_)),
                "{status}: {err:?}"
            );
            assert_eq!(err.class(), class, "{status}: {err:?}");
            assert_eq!(err.is_fault(), fault, "{status}: {err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{status}");
        }
    }

    #[test]
    fn a_wait_is_the_responses_and_a_zero_is_none() {
        let limited = decoded(429, Some("7"), TOO_MANY);
        assert_eq!(limited.retry_after(), Some(Duration::from_secs(7)));
        assert_eq!(
            limited.class(),
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(7))
            }
        );

        // Any status's `Retry-After` is the error's wait, though only a 429's class carries it.
        let down = decoded(503, Some("2"), NOT_FOUND);
        assert_eq!(down.retry_after(), Some(Duration::from_secs(2)));
        assert_eq!(down.class().retry_after(), None);

        // Core reads a zero as no wait, and a hand-built zero is no wait either.
        assert_eq!(decoded(429, Some("0"), TOO_MANY).retry_after(), None);
        let mut zero = venue(429, None, TOO_MANY);
        zero.retry_after = Some(Duration::ZERO);
        assert_eq!(Classify::retry_after(&zero), None);
        assert_eq!(zero.class(), Class::RateLimited { retry_after: None });
    }

    #[test]
    fn a_body_in_no_known_shape_is_left_to_core() {
        let bodies = [
            "<html>bad gateway</html>",
            "",
            "[]",
            r#"{"message":"down"}"#,
            r#"{"error":5}"#,
            r#"{"error":{"code":"x"}}"#,
            r#"{"error":{"message":5}}"#,
            r#"{"msg":5}"#,
            r#"{"exchange_active":true,"trading_active":true}"#,
        ];
        for body in bodies {
            let err = decoded(502, None, body);
            assert!(
                matches!(&err, PredictionsError::Api(ApiError::Response(r))
                    if r.status.as_u16() == 502 && r.body == body),
                "{body:?}: {err:?}"
            );
            assert_eq!(err.class(), Class::Unavailable { code: None }, "{body:?}");
            assert!(err.is_retriable(), "{body:?}");
        }
    }

    #[test]
    fn an_error_that_is_not_a_response_is_left_to_core() {
        let sign = ApiError::Sign(Box::new(std::io::Error::other("no key")));
        let rows = [
            ApiError::Validation("refused".into()),
            ApiError::Url(url::ParseError::EmptyHost),
            sign,
        ];
        for err in rows {
            let text = err.to_string();
            let err = PredictionsError::from(err);
            assert!(matches!(err, PredictionsError::Api(_)), "{text}");
            assert_eq!(err.to_string(), text);
            assert_eq!(err.class(), Class::InvalidRequest, "{text}");
            assert!(err.is_fault(), "{text}");
        }
    }

    #[test]
    fn a_451_is_a_region_block_and_not_a_fault_whatever_its_body() {
        let blocked = venue(451, None, NOT_FOUND);
        assert_eq!(blocked.class(), Class::Restricted);
        assert!(!blocked.is_fault());
        assert!(!Classify::is_fault(&decoded(451, None, "")));
    }

    #[test]
    fn predictions_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PredictionsError>();
        assert_send_sync::<VenueError>();
    }
}
