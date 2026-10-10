use std::time::Duration;

use polyoxide_venue::{class_for_status, parse_retry_after, Class, Classify};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::StatusCode;
use thiserror::Error;

use crate::hooks::Refused;

/// An unsuccessful response, read whole: what [`ApiError::Response`] carries.
///
/// The send loop builds one when the client's policy fails a response, and a
/// venue's `From<ApiError>` decodes its own body shape from [`body`](Self::body).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ErrorResponse {
    /// The response's status.
    pub status: StatusCode,
    /// The response's headers.
    pub headers: HeaderMap,
    /// The response's body, verbatim, or `""` when it could not be read.
    pub body: String,
    /// The body's `error` field, else its `message` field, else the body
    /// verbatim: the shape Polymarket's hosts answer with.
    pub message: String,
    /// The response's `Retry-After`, read by
    /// [`polyoxide_venue::parse_retry_after`] with no clamp, since it is
    /// surfaced, never slept on. A zero is `None`.
    pub retry_after: Option<Duration>,
}

impl ErrorResponse {
    /// An unsuccessful response from its parts, its message and `Retry-After`
    /// read from them.
    pub fn new(status: StatusCode, headers: HeaderMap, body: impl Into<String>) -> Self {
        let body = body.into();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .or(v.get("message"))
                    .and_then(|m| m.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| body.clone());
        let retry_after = headers
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| parse_retry_after(value, Duration::MAX));
        Self {
            status,
            headers,
            body,
            message,
            retry_after,
        }
    }

    /// Read `response` whole. A body that cannot be read is `""`.
    pub async fn read(response: reqwest::Response) -> Self {
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.text().await.unwrap_or_default();
        tracing::debug!("API error response body: {}", body);
        Self::new(status, headers, body)
    }
}

/// Core API error types shared across Polyoxide clients
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ApiError {
    /// The server answered with an unsuccessful status. Its class is the
    /// status's.
    #[error("API error: {} - {}", .0.status.as_u16(), .0.message)]
    Response(Box<ErrorResponse>),

    /// The client refused the request before sending it: bad input, or
    /// missing configuration.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// URL parsing error
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// The throttle refused a cost no layer can ever hold, and nothing was
    /// sent.
    #[error("Refused before sending: {0}")]
    Refused(#[from] Refused),

    /// An [`Authenticator`](crate::Authenticator) could not sign the request,
    /// and nothing was sent. Carries the venue's own error, which the venue's
    /// `From<ApiError>` takes back out.
    #[error("Signing failed: {0}")]
    Sign(Box<dyn std::error::Error + Send + Sync>),
}

impl ApiError {
    /// Read an unsuccessful response whole, as [`ApiError::Response`].
    pub async fn from_response(response: reqwest::Response) -> Self {
        ErrorResponse::read(response).await.into()
    }
}

impl From<ErrorResponse> for ApiError {
    fn from(response: ErrorResponse) -> Self {
        Self::Response(Box::new(response))
    }
}

/// The class of a transport error from `reqwest`.
///
/// A failure in transit is [`Class::Network`]: a timeout, a failed connect, or
/// any error with an I/O error or a body-read failure in its source chain.
/// reqwest reports a body that broke off mid-read as a decode error, so these
/// are checked first. An error that carries an HTTP status follows the status
/// rule. Then a request that could not be built is [`Class::InvalidRequest`],
/// a body that arrived whole but did not decode is [`Class::Decode`], and
/// anything else is [`Class::Network`].
pub fn classify_reqwest(err: &reqwest::Error) -> Class {
    if failed_in_transit(err) {
        Class::Network
    } else if let Some(status) = err.status() {
        status_class(status.as_u16())
    } else if err.is_builder() {
        Class::InvalidRequest
    } else if err.is_decode() {
        Class::Decode
    } else {
        Class::Network
    }
}

/// Whether a reqwest error is the connection failing, however reqwest labels it.
fn failed_in_transit(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() || err.is_body() {
        return true;
    }
    let mut source = std::error::Error::source(err);
    while let Some(inner) = source {
        if inner.is::<std::io::Error>()
            || inner
                .downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_body)
        {
            return true;
        }
        source = inner.source();
    }
    false
}

/// The status rule, with any status outside 400–599 a [`Class::Decode`].
fn status_class(status: u16) -> Class {
    class_for_status(status).unwrap_or(Class::Decode)
}

/// The class each variant reports.
///
/// [`ApiError::Response`] follows the status rule, with any status outside
/// 400–599 a [`Class::Decode`] (a 2xx that a caller found wanting, such as a
/// ping whose body says it is not ok), and a `429`'s wait its `Retry-After`.
/// A 451 is not a fault: the venue does not serve the caller's region, as
/// designed. [`ApiError::Validation`] and [`ApiError::Sign`] are a
/// [`Class::InvalidRequest`]: the client refused the request, or could not
/// sign it, and nothing was sent.
///
/// [`Classify::retry_after`] is a response's `Retry-After` whatever its
/// status, and `None` for every other variant.
impl Classify for ApiError {
    fn class(&self) -> Class {
        match self {
            Self::Response(response) => {
                status_class(response.status.as_u16()).with_retry_after(response.retry_after)
            }
            Self::Validation(_) => Class::InvalidRequest,
            Self::Network(err) => classify_reqwest(err),
            Self::Serialization(_) => Class::Decode,
            Self::Url(_) => Class::InvalidRequest,
            Self::Refused(refused) => refused.class(),
            Self::Sign(_) => Class::InvalidRequest,
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Response(response)
            if response.status == StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS)
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Response(response) => response.retry_after,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response with `status`, `body` and, when given, a `Retry-After`.
    fn response(status: u16, body: &str, retry_after: Option<&str>) -> ErrorResponse {
        let mut headers = HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(RETRY_AFTER, value.parse().unwrap());
        }
        ErrorResponse::new(StatusCode::from_u16(status).unwrap(), headers, body)
    }

    fn api(status: u16) -> ApiError {
        response(status, "", None).into()
    }

    #[test]
    fn test_rate_limit_error_carries_message() {
        let err = ApiError::from(response(
            429,
            r#"{"error":"too many requests, retry after 5s"}"#,
            None,
        ));
        let display = format!("{}", err);
        assert!(
            display.contains("too many requests"),
            "A 429's display should contain the message: {}",
            display
        );
    }

    #[test]
    fn test_rate_limit_error_display_format() {
        let err = ApiError::from(response(429, r#"{"error":"slow down"}"#, None));
        assert_eq!(format!("{}", err), "API error: 429 - slow down");
    }

    // ── ErrorResponse ───────────────────────────────────────────

    #[test]
    fn test_error_response_reads_error_then_message() {
        let bad = response(400, r#"{"error":"bad limit"}"#, None);
        assert_eq!(bad.message, "bad limit");
        assert_eq!(bad.body, r#"{"error":"bad limit"}"#);
        assert_eq!(response(503, r#"{"message":"down"}"#, None).message, "down");
        assert_eq!(
            response(503, r#"{"error":"first","message":"second"}"#, None).message,
            "first"
        );
    }

    #[test]
    fn test_error_response_keeps_a_non_json_body_verbatim() {
        // Cloudflare's IP block is plain text, not JSON.
        let blocked = response(429, "error code: 1015", None);
        assert_eq!(blocked.message, "error code: 1015");
        assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_error_response_reads_retry_after_unclamped() {
        let at = |value| response(429, "", Some(value)).retry_after;
        assert_eq!(at("7"), Some(Duration::from_secs(7)));
        assert_eq!(at("1.5"), Some(Duration::from_millis(1500)));
        // Surfaced, never slept on, so a week stays a week.
        assert_eq!(at("604800"), Some(Duration::from_secs(604_800)));
        // A zero is no wait (DRIFT R4), and junk is none.
        for junk in ["0", "0.0", "-1", "soon", "Wed, 21 Oct 2026 07:28:00 GMT"] {
            assert_eq!(at(junk), None, "{junk:?}");
        }
        assert_eq!(response(429, "", None).retry_after, None);
    }

    #[test]
    fn test_response_display_is_the_status_and_message() {
        let err = ApiError::from(response(503, r#"{"error":"bad gateway"}"#, None));
        assert_eq!(err.to_string(), "API error: 503 - bad gateway");
        assert_eq!(
            ApiError::Validation("bad request".into()).to_string(),
            "Validation error: bad request"
        );
    }

    // ── retriability, through the class ─────────────────────────

    #[test]
    fn test_retriable_transient_failures() {
        assert!(Classify::is_retriable(&api(429)));
        assert!(Classify::is_retriable(&api(408)));
        for status in [500u16, 502, 503, 504] {
            assert!(
                Classify::is_retriable(&api(status)),
                "{status} should be retriable"
            );
        }
    }

    #[test]
    fn test_retriable_425_too_early() {
        // Polymarket returns 425 while the matching engine restarts and documents
        // it as "retry with exponential backoff".
        assert!(Classify::is_retriable(&api(425)));
    }

    #[test]
    fn test_not_retriable_deterministic_failures() {
        assert!(!Classify::is_retriable(&ApiError::Validation(
            "bad payload".into()
        )));
        assert!(!Classify::is_retriable(&api(400)));
        assert!(!Classify::is_retriable(&api(401)));
        for status in [404u16, 409, 418] {
            assert!(
                !Classify::is_retriable(&api(status)),
                "{status} should not be retriable"
            );
        }
    }

    #[test]
    fn test_not_retriable_local_encode_decode_failures() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        assert!(!Classify::is_retriable(&ApiError::Serialization(json_err)));
        let url_err = url::Url::parse("://bad").unwrap_err();
        assert!(!Classify::is_retriable(&ApiError::Url(url_err)));
    }

    // ── Classify ────────────────────────────────────────────────

    /// One reqwest error of each kind the rule tells apart.
    struct ReqwestErrors {
        builder: reqwest::Error,
        decode: reqwest::Error,
        cut_off: reqwest::Error,
        connect: reqwest::Error,
        timeout: reqwest::Error,
        unavailable: reqwest::Error,
        not_found: reqwest::Error,
    }

    async fn reqwest_errors() -> ReqwestErrors {
        use std::io::{Read, Write};

        let builder = reqwest::Client::new().get("not a url").build().unwrap_err();
        assert!(builder.is_builder());

        // No proxy, so a proxy set in the environment cannot answer for the
        // local addresses below.
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut server = mockito::Server::new_async().await;
        let _not_json = server
            .mock("GET", "/not-json")
            .with_body("not json")
            .create_async()
            .await;
        let _down = server
            .mock("GET", "/down")
            .with_status(503)
            .create_async()
            .await;
        let _missing = server
            .mock("GET", "/missing")
            .with_status(404)
            .create_async()
            .await;
        let get = |path: &str| client.get(format!("{}{path}", server.url())).send();

        let decode = get("/not-json")
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap_err();
        assert!(decode.is_decode());

        let status = |response: reqwest::Response| response.error_for_status().unwrap_err();
        let unavailable = status(get("/down").await.unwrap());
        let not_found = status(get("/missing").await.unwrap());

        // A body that promises 100 bytes and stops after 6. reqwest reports
        // the broken read as a decode error.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let cut_off_url = format!("http://{}", listener.local_addr().unwrap());
        let server_thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0; 1024]);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"a\":");
        });
        let cut_off = client
            .get(&cut_off_url)
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap_err();
        server_thread.join().unwrap();
        assert!(cut_off.is_decode(), "{cut_off:?}");

        // Bind a port, then free it, so nothing is listening there.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let connect = client
            .get(format!("http://127.0.0.1:{port}"))
            .send()
            .await
            .unwrap_err();
        assert!(connect.is_connect());

        // A listener that never accepts: the connection opens and no answer comes.
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let timeout = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_millis(300))
            .build()
            .unwrap()
            .get(format!("http://{}", silent.local_addr().unwrap()))
            .send()
            .await
            .unwrap_err();
        assert!(timeout.is_timeout());

        ReqwestErrors {
            builder,
            decode,
            cut_off,
            connect,
            timeout,
            unavailable,
            not_found,
        }
    }

    #[tokio::test]
    async fn every_variant_classifies() {
        let reqwest = reqwest_errors().await;
        let json = serde_json::from_str::<String>("not json").unwrap_err();
        let url = url::Url::parse("://bad").unwrap_err();
        let unavailable = Class::Unavailable { code: None };
        let refusal = Class::VenueRefusal { code: None };
        let secs = |n| Some(Duration::from_secs(n));

        // (error, class, is_fault, retry_after()). The status decides a
        // response's class, as it did for the variants it replaced.
        let rows = [
            (api(200), Class::Decode, true, None),
            (api(302), Class::Decode, true, None),
            (api(400), refusal.clone(), true, None),
            (api(401), Class::Unauthorized, true, None),
            (api(403), Class::Unauthorized, true, None),
            (api(404), refusal.clone(), true, None),
            (api(408), unavailable.clone(), true, None),
            (api(418), Class::Restricted, true, None),
            (api(425), unavailable.clone(), true, None),
            (
                api(429),
                Class::RateLimited { retry_after: None },
                true,
                None,
            ),
            // A region block is the venue answering as designed.
            (api(451), Class::Restricted, false, None),
            (api(500), unavailable.clone(), true, None),
            (api(503), unavailable.clone(), true, None),
            (api(600), Class::Decode, true, None),
            // Was `Authentication`, built from a 401 or 403.
            (
                response(403, r#"{"error":"no"}"#, None).into(),
                Class::Unauthorized,
                true,
                None,
            ),
            // Was `Validation`, for a server 400 or a local refusal. A local
            // refusal is now `InvalidRequest`, and a server 400 the status's.
            (
                ApiError::Validation("bad".into()),
                Class::InvalidRequest,
                true,
                None,
            ),
            (
                response(400, r#"{"error":"bad"}"#, None).into(),
                refusal.clone(),
                true,
                None,
            ),
            // Was `RateLimit`, which kept no Retry-After. A zero is no wait.
            (
                response(429, "slow", Some("7")).into(),
                Class::RateLimited {
                    retry_after: secs(7),
                },
                true,
                secs(7),
            ),
            (
                response(429, "slow", Some("0")).into(),
                Class::RateLimited { retry_after: None },
                true,
                None,
            ),
            // Any status reports its wait, though only a 429's class has one.
            (
                response(503, "down", Some("5")).into(),
                unavailable.clone(),
                true,
                secs(5),
            ),
            // Was `Timeout`, built only from a 408.
            (
                response(408, "slow", None).into(),
                unavailable.clone(),
                true,
                None,
            ),
            (
                ApiError::Network(reqwest.builder),
                Class::InvalidRequest,
                true,
                None,
            ),
            (ApiError::Network(reqwest.decode), Class::Decode, true, None),
            // Labelled a decode error by reqwest, but the connection broke.
            (
                ApiError::Network(reqwest.cut_off),
                Class::Network,
                true,
                None,
            ),
            (
                ApiError::Network(reqwest.connect),
                Class::Network,
                true,
                None,
            ),
            (
                ApiError::Network(reqwest.timeout),
                Class::Network,
                true,
                None,
            ),
            (
                ApiError::Network(reqwest.unavailable),
                unavailable,
                true,
                None,
            ),
            (ApiError::Network(reqwest.not_found), refusal, true, None),
            (ApiError::Serialization(json), Class::Decode, true, None),
            (ApiError::Url(url), Class::InvalidRequest, true, None),
            (
                ApiError::Refused(Refused {
                    layer: crate::LayerId("order"),
                    units: 2_000,
                    capacity: 120,
                }),
                Class::InvalidRequest,
                true,
                None,
            ),
            (
                ApiError::Sign("no key to sign with".into()),
                Class::InvalidRequest,
                true,
                None,
            ),
        ];
        for (err, class, fault, wait) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), wait, "{err:?}");
            assert_eq!(
                Classify::is_retriable(&err),
                class.is_retriable(),
                "{err:?}"
            );
        }
    }
}
