//! The send loop on a transport the test injects. A recording transport
//! answers every attempt, so nothing is dialled, and the loop's retry, hold
//! and permit rules run on what it returns.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use polyoxide_core::http::{HeaderMap, HeaderValue, Method, StatusCode};
use polyoxide_core::{
    ApiError, AttemptInfo, Charge, HttpClient, HttpClientBuilder, HttpRequest, HttpTransport,
    Refused, RequestMeta, RequestParts, Response, ResponseMeta, RetryConfig, Throttle,
    TransportError, TransportKind,
};
use polyoxide_rt::BoxFuture;
use polyoxide_venue::{Class, Classify};

/// One scripted answer.
#[derive(Debug)]
struct Reply {
    status: u16,
    headers: Vec<(&'static str, &'static str)>,
    body: &'static str,
}

fn reply(status: u16, headers: &[(&'static str, &'static str)], body: &'static str) -> Reply {
    Reply {
        status,
        headers: headers.to_vec(),
        body,
    }
}

/// Answers each request with the next scripted reply and keeps the requests
/// it saw.
#[derive(Debug, Default)]
struct Recording {
    replies: Mutex<VecDeque<Reply>>,
    seen: Mutex<Vec<HttpRequest>>,
}

impl Recording {
    fn answering(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::default(),
        })
    }

    fn seen(&self) -> Vec<HttpRequest> {
        self.seen.lock().unwrap().clone()
    }
}

impl HttpTransport for Recording {
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        let url = request.url.clone();
        self.seen.lock().unwrap().push(request);
        let reply = self.replies.lock().unwrap().pop_front();
        Box::pin(async move {
            let Some(reply) = reply else {
                return Err(TransportError::new(
                    TransportKind::Other,
                    "no scripted reply left",
                ));
            };
            let mut headers = HeaderMap::new();
            for (name, value) in reply.headers {
                headers.insert(name, HeaderValue::from_static(value));
            }
            let body = Bytes::from_static(reply.body.as_bytes());
            Ok(Response::new(
                StatusCode::from_u16(reply.status).expect("a valid status"),
                headers,
                url,
                Box::pin(async move { Ok(body) }),
            ))
        })
    }
}

/// A transport whose every attempt fails, counting them.
#[derive(Debug, Default)]
struct Failing(Mutex<usize>);

impl HttpTransport for Failing {
    fn send(&self, _request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        *self.0.lock().unwrap() += 1;
        Box::pin(async {
            Err(TransportError::new(
                TransportKind::Timeout,
                "no answer in time",
            ))
        })
    }
}

/// Answers 503 with a body that, when it is read, tries to take the client's
/// only concurrency permit and records whether it could.
#[derive(Default)]
struct Probing {
    client: Arc<OnceLock<HttpClient>>,
    permit_was_free: Arc<Mutex<Option<bool>>>,
}

/// Written by hand: the client holds this transport and this holds the client,
/// so a derived `Debug` would print forever.
impl std::fmt::Debug for Probing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Probing").finish_non_exhaustive()
    }
}

impl HttpTransport for Probing {
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        let client = Arc::clone(&self.client);
        let permit_was_free = Arc::clone(&self.permit_was_free);
        Box::pin(async move {
            Ok(Response::new(
                StatusCode::SERVICE_UNAVAILABLE,
                HeaderMap::new(),
                request.url,
                Box::pin(async move {
                    let http = client
                        .get()
                        .expect("the client is set before it sends")
                        .clone();
                    let runtime = http.runtime().clone();
                    let granted = polyoxide_rt::timeout(
                        &*runtime,
                        Duration::from_millis(200),
                        http.acquire_concurrency(),
                    )
                    .await
                    .is_some();
                    *permit_was_free.lock().unwrap() = Some(granted);
                    Ok(Bytes::from_static(b"down"))
                }),
            ))
        })
    }
}

/// A throttle that charges nothing, counts the responses it observes and
/// keeps each hold it is told of.
#[derive(Clone, Default)]
struct Spy {
    holds: Arc<Mutex<Vec<Duration>>>,
    observed: Arc<AtomicUsize>,
}

impl Throttle for Spy {
    async fn acquire(&self, _meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        Ok(Charge::none())
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {
        self.observed.fetch_add(1, Ordering::SeqCst);
    }

    fn hold(&self, delay: Duration) {
        self.holds.lock().unwrap().push(delay);
    }
}

fn quick(max_retries: u32) -> RetryConfig {
    RetryConfig {
        max_retries,
        initial_backoff_ms: 1,
        max_backoff_ms: 10_000,
    }
}

#[tokio::test]
async fn a_retry_is_sent_on_the_injected_transport_and_nothing_is_dialled() {
    let transport = Recording::answering(vec![reply(429, &[], "slow down"), reply(200, &[], "ok")]);
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .with_retry_config(quick(1))
        .timeout_ms(7_000)
        .build()
        .unwrap();

    let query = [
        ("limit".to_owned(), "2".to_owned()),
        ("q".to_owned(), "x y".to_owned()),
    ];
    let body = http.get_bytes("/v1/rows", &query).await.unwrap();

    assert_eq!(body, b"ok");
    let seen = transport.seen();
    assert_eq!(seen.len(), 2, "the request and its one retry");
    for request in &seen {
        assert_eq!(request.method, Method::GET);
        // The query is form-urlencoded onto the URL, a space as `+`, as
        // reqwest's `.query()` always did.
        assert_eq!(
            request.url.as_str(),
            "https://api.example.test/v1/rows?limit=2&q=x+y"
        );
        assert_eq!(request.timeout, Duration::from_millis(7_000));
        assert_eq!(request.body, None);
    }
}

#[tokio::test]
async fn a_request_carries_its_own_body_headers_and_timeout() {
    let transport = Recording::answering(vec![reply(200, &[], "ok")]);
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .build()
        .unwrap();

    let mut parts = RequestParts::new(Method::POST, "/v1/order");
    parts
        .headers
        .insert("x-signed", HeaderValue::from_static("yes"));
    parts.body = Some(Bytes::from_static(br#"{"size":1}"#));
    parts.timeout = Some(Duration::from_secs(300));
    let response = http.send(parts, &[], None).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.url().as_str(), "https://api.example.test/v1/order");
    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, Method::POST);
    assert_eq!(
        seen[0]
            .headers
            .get("x-signed")
            .and_then(|value| value.to_str().ok()),
        Some("yes")
    );
    assert_eq!(seen[0].body, Some(Bytes::from_static(br#"{"size":1}"#)));
    assert_eq!(
        seen[0].timeout,
        Duration::from_secs(300),
        "the request's own timeout replaces the client's 30 s"
    );
}

#[tokio::test]
async fn a_429_on_the_injected_transport_holds_the_throttle_and_reads_its_body() {
    let transport = Recording::answering(vec![reply(429, &[("retry-after", "2")], "slow down")]);
    let spy = Spy::default();
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .with_throttle(spy.clone())
        .with_retry_config(quick(0))
        .build()
        .unwrap();

    let err = http.get_bytes("/v1/rows", &[]).await.unwrap_err();

    // The head the transport sent reached the policy, which held the throttle
    // for the `Retry-After` it carried.
    assert_eq!(*spy.holds.lock().unwrap(), [Duration::from_secs(2)]);
    // The body the transport left lazy was read for the error.
    match err {
        ApiError::Response(response) => {
            assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.body, "slow down");
            assert_eq!(response.retry_after, Some(Duration::from_secs(2)));
        }
        other => panic!("expected the 429 as ApiError::Response, got {other:?}"),
    }
    assert_eq!(transport.seen().len(), 1, "no retry was left");
}

#[tokio::test]
async fn an_error_body_is_read_after_the_permit_is_released() {
    let probing = Arc::new(Probing::default());
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(probing.clone())
        .with_max_concurrent(1)
        .build()
        .unwrap();
    probing
        .client
        .set(http.clone())
        .expect("the client is set once");

    let err = http.get_bytes("/v1/rows", &[]).await.unwrap_err();

    assert!(
        matches!(&err, ApiError::Response(r)
            if r.status == StatusCode::SERVICE_UNAVAILABLE && r.body == "down"),
        "{err:?}"
    );
    assert_eq!(
        *probing.permit_was_free.lock().unwrap(),
        Some(true),
        "the body was read while the attempt still held the only permit"
    );
}

#[tokio::test]
async fn a_transport_failure_is_not_observed_and_not_retried() {
    let transport = Arc::new(Failing::default());
    let spy = Spy::default();
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .with_throttle(spy.clone())
        .with_retry_config(quick(3))
        .build()
        .unwrap();

    let err = http.get_bytes("/v1/rows", &[]).await.unwrap_err();

    assert!(
        matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Timeout),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::Network);
    assert_eq!(
        err.to_string(),
        "Transport error: timeout: no answer in time"
    );
    assert_eq!(*transport.0.lock().unwrap(), 1, "no retry");
    assert_eq!(spy.observed.load(Ordering::SeqCst), 0, "nothing to observe");
    assert!(spy.holds.lock().unwrap().is_empty());
}
