use polyoxide_venue::{class_for_status, Class, Classify};
use thiserror::Error;

use crate::hooks::Refused;

/// Core API error types shared across Polyoxide clients
#[derive(Error, Debug)]
pub enum ApiError {
    /// HTTP request failed
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Authentication failed (401/403)
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Request validation failed (400)
    #[error("Validation error: {0}")]
    Validation(String),

    /// Rate limit exceeded (429)
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Request timeout
    #[error("Request timeout")]
    Timeout,

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
    /// Create error from HTTP response
    pub async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();

        // Get the raw response text first for debugging
        let body_text = response.text().await.unwrap_or_default();
        tracing::debug!("API error response body: {}", body_text);

        Self::from_status_and_body(status, &body_text)
    }

    /// Classify an unsuccessful response from its status and body text.
    ///
    /// The body-reading half of [`from_response`](Self::from_response), for
    /// callers that have to read the body themselves first, for example to
    /// check it for a richer error shape before falling back to this one.
    pub fn from_status_and_body(status: u16, body: &str) -> Self {
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .or(v.get("message"))
                    .and_then(|m| m.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| body.to_owned());

        match status {
            401 | 403 => Self::Authentication(message),
            400 => Self::Validation(message),
            429 => Self::RateLimit(message),
            408 => Self::Timeout,
            _ => Self::Api { status, message },
        }
    }

    /// Whether re-sending the same request could plausibly produce a different result.
    ///
    /// A caller's retry policy should use
    /// [`polyoxide_venue::Classify::is_retriable`] instead, which reads the
    /// error's class and answers the same way for every crate. This method
    /// remains until Epic 3 removes it, and may disagree with the class: it
    /// calls a transport failure that is neither a connect nor a timeout
    /// final.
    ///
    /// Retriable: rate limits, timeouts, connection failures, and the statuses
    /// [`polyoxide_venue::class_for_status`] classes retriable: `408`, `425 Too
    /// Early` (Polymarket's matching engine restarting), `429` and any 5xx. Not
    /// retriable: authentication failures, validation failures, and local
    /// encode/decode errors — all of which are deterministic for a given request.
    ///
    /// Note this describes the *error*, not the *operation*: a retriable error on a
    /// non-idempotent request (order placement) still needs caller-side judgement
    /// about whether resubmitting is safe.
    pub fn is_retriable(&self) -> bool {
        match self {
            // polyoxide-venue's one status rule: 425 Too Early is the matching
            // engine restarting and 5xx a server fault, both documented upstream
            // as "retry with exponential backoff".
            Self::Api { status, .. } => class_for_status(*status).is_some_and(|c| c.is_retriable()),
            Self::RateLimit(_) | Self::Timeout => true,
            Self::Network(e) => e.is_timeout() || e.is_connect(),
            Self::Authentication(_) | Self::Validation(_) => false,
            Self::Serialization(_) | Self::Url(_) => false,
            Self::Refused(_) | Self::Sign(_) => false,
        }
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

/// The status rule, with `0` a [`Class::VenueRefusal`] and any other status
/// outside 400–599 a [`Class::Decode`].
fn status_class(status: u16) -> Class {
    class_for_status(status).unwrap_or(match status {
        0 => Class::VenueRefusal { code: None },
        _ => Class::Decode,
    })
}

/// The class each variant reports.
///
/// [`ApiError::Api`] follows the status rule, with any status outside 400–599
/// a [`Class::Decode`], except `0`. polyoxide-clob builds a status of `0` for
/// any failure of its Gamma dependency, a 4xx and a local failure included,
/// so it is a [`Class::VenueRefusal`] and not retried. A 451 is not a fault:
/// the venue does not serve the caller's region, as designed.
/// [`ApiError::Validation`] is a [`Class::VenueRefusal`] even when clob raised
/// it locally, until the two are separate variants. [`ApiError::Timeout`] is
/// built only from a 408, so it is [`Class::Unavailable`]. [`ApiError::Sign`]
/// is a [`Class::InvalidRequest`]: the client could not sign, and nothing was
/// sent.
///
/// Where this disagrees with [`ApiError::is_retriable`], the inherent method
/// keeps its answer.
impl Classify for ApiError {
    fn class(&self) -> Class {
        match self {
            Self::Api { status, .. } => status_class(*status),
            Self::Authentication(_) => Class::Unauthorized,
            Self::Validation(_) => Class::VenueRefusal { code: None },
            Self::RateLimit(_) => Class::RateLimited { retry_after: None },
            Self::Timeout => Class::Unavailable { code: None },
            Self::Network(err) => classify_reqwest(err),
            Self::Serialization(_) => Class::Decode,
            Self::Url(_) => Class::InvalidRequest,
            Self::Refused(refused) => refused.class(),
            Self::Sign(_) => Class::InvalidRequest,
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Api { status: 451, .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_error_carries_message() {
        let err = ApiError::RateLimit("too many requests, retry after 5s".to_string());
        let display = format!("{}", err);
        assert!(
            display.contains("too many requests"),
            "RateLimit display should contain the message: {}",
            display
        );
    }

    #[test]
    fn test_rate_limit_error_display_format() {
        let err = ApiError::RateLimit("slow down".to_string());
        assert_eq!(format!("{}", err), "Rate limit exceeded: slow down");
    }

    // ── from_status_and_body ────────────────────────────────────

    #[test]
    fn test_from_status_and_body_reads_error_then_message() {
        assert!(matches!(
            ApiError::from_status_and_body(400, r#"{"error":"bad limit"}"#),
            ApiError::Validation(m) if m == "bad limit"
        ));
        assert!(matches!(
            ApiError::from_status_and_body(503, r#"{"message":"down"}"#),
            ApiError::Api { status: 503, message } if message == "down"
        ));
    }

    #[test]
    fn test_from_status_and_body_keeps_a_non_json_body_verbatim() {
        // Cloudflare's IP block is plain text, not JSON.
        assert!(matches!(
            ApiError::from_status_and_body(429, "error code: 1015"),
            ApiError::RateLimit(m) if m == "error code: 1015"
        ));
    }

    // ── is_retriable ────────────────────────────────────────────

    #[test]
    fn test_retriable_transient_failures() {
        assert!(ApiError::RateLimit("slow down".into()).is_retriable());
        assert!(ApiError::Timeout.is_retriable());
        for status in [500u16, 502, 503, 504] {
            assert!(
                ApiError::Api {
                    status,
                    message: String::new()
                }
                .is_retriable(),
                "{status} should be retriable"
            );
        }
    }

    #[test]
    fn test_retriable_425_too_early() {
        // Polymarket returns 425 while the matching engine restarts and documents
        // it as "retry with exponential backoff".
        assert!(ApiError::Api {
            status: 425,
            message: String::new()
        }
        .is_retriable());
    }

    #[test]
    fn test_not_retriable_deterministic_failures() {
        assert!(!ApiError::Validation("bad payload".into()).is_retriable());
        assert!(!ApiError::Authentication("Invalid API key".into()).is_retriable());
        for status in [404u16, 409, 418] {
            assert!(
                !ApiError::Api {
                    status,
                    message: String::new()
                }
                .is_retriable(),
                "{status} should not be retriable"
            );
        }
    }

    #[test]
    fn test_not_retriable_local_encode_decode_failures() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        assert!(!ApiError::Serialization(json_err).is_retriable());
        let url_err = url::Url::parse("://bad").unwrap_err();
        assert!(!ApiError::Url(url_err).is_retriable());
    }

    // ── Classify ────────────────────────────────────────────────

    fn api(status: u16) -> ApiError {
        ApiError::Api {
            status,
            message: String::new(),
        }
    }

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

        // (error, class, is_fault, inherent is_retriable). No ApiError carries
        // a wait, since `RateLimit` keeps no Retry-After.
        let rows = [
            // Clob's failed Gamma dependency, whatever it was.
            (api(0), refusal.clone(), true, false),
            (api(200), Class::Decode, true, false),
            (api(302), Class::Decode, true, false),
            (api(400), refusal.clone(), true, false),
            (api(401), Class::Unauthorized, true, false),
            (api(403), Class::Unauthorized, true, false),
            (api(404), refusal.clone(), true, false),
            (api(408), unavailable.clone(), true, true),
            (api(418), Class::Restricted, true, false),
            (api(425), unavailable.clone(), true, true),
            (
                api(429),
                Class::RateLimited { retry_after: None },
                true,
                true,
            ),
            // A region block is the venue answering as designed.
            (api(451), Class::Restricted, false, false),
            (api(500), unavailable.clone(), true, true),
            (api(503), unavailable.clone(), true, true),
            (api(600), Class::Decode, true, false),
            (
                ApiError::Authentication("no".into()),
                Class::Unauthorized,
                true,
                false,
            ),
            (
                ApiError::Validation("bad".into()),
                refusal.clone(),
                true,
                false,
            ),
            (
                ApiError::RateLimit("slow".into()),
                Class::RateLimited { retry_after: None },
                true,
                true,
            ),
            (ApiError::Timeout, unavailable.clone(), true, true),
            (
                ApiError::Network(reqwest.builder),
                Class::InvalidRequest,
                true,
                false,
            ),
            (
                ApiError::Network(reqwest.decode),
                Class::Decode,
                true,
                false,
            ),
            // Labelled a decode error by reqwest, but the connection broke.
            (
                ApiError::Network(reqwest.cut_off),
                Class::Network,
                true,
                false,
            ),
            (
                ApiError::Network(reqwest.connect),
                Class::Network,
                true,
                true,
            ),
            (
                ApiError::Network(reqwest.timeout),
                Class::Network,
                true,
                true,
            ),
            (
                ApiError::Network(reqwest.unavailable),
                unavailable,
                true,
                false,
            ),
            (ApiError::Network(reqwest.not_found), refusal, true, false),
            (ApiError::Serialization(json), Class::Decode, true, false),
            (ApiError::Url(url), Class::InvalidRequest, true, false),
            (
                ApiError::Refused(Refused {
                    layer: crate::LayerId("order"),
                    units: 2_000,
                    capacity: 120,
                }),
                Class::InvalidRequest,
                true,
                false,
            ),
            (
                ApiError::Sign("no key to sign with".into()),
                Class::InvalidRequest,
                true,
                false,
            ),
        ];
        for (err, class, fault, inherent) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), None, "{err:?}");
            assert_eq!(
                Classify::is_retriable(&err),
                class.is_retriable(),
                "{err:?}"
            );
            // Pinned separately: the two disagree on a transport failure that
            // is neither a connect nor a timeout, until Epic 3 removes the
            // inherent method.
            assert_eq!(err.is_retriable(), inherent, "{err:?}");
        }
    }
}
