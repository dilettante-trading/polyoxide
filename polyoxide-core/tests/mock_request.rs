use std::time::Duration;

use mockito::Server;
use polyoxide_core::{ApiError, HttpClientBuilder, Request, RequestError, RetryConfig};
use serde::Deserialize;

/// Simple response type for testing deserialization.
#[derive(Debug, Deserialize)]
struct TestResponse {
    value: String,
}

/// Error wrapper implementing RequestError for tests.
#[derive(Debug)]
struct TestError(ApiError);

impl From<ApiError> for TestError {
    fn from(e: ApiError) -> Self {
        Self(e)
    }
}

impl RequestError for TestError {
    async fn from_response(response: reqwest::Response) -> Self {
        Self(ApiError::from_response(response).await)
    }
}

fn test_request(server: &mockito::ServerGuard, path: &str) -> Request<TestResponse, TestError> {
    let http = HttpClientBuilder::new(server.url()).build().unwrap();
    Request::new(http, path)
}

fn test_request_with_retry(
    server: &mockito::ServerGuard,
    path: &str,
    config: RetryConfig,
) -> Request<TestResponse, TestError> {
    let http = HttpClientBuilder::new(server.url())
        .with_retry_config(config)
        .build()
        .unwrap();
    Request::new(http, path)
}

#[tokio::test]
async fn retries_on_429_then_succeeds() {
    let mut server = Server::new_async().await;

    // mockito matches in reverse creation order (LIFO), so create the success mock first
    // and the 429 mock second. The 429 will be matched first, then removed, leaving the 200.
    let success_mock = server
        .mock("GET", "/retry-test")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"value": "ok"}"#)
        .create_async()
        .await;

    let retry_mock = server
        .mock("GET", "/retry-test")
        .with_status(429)
        .with_header("retry-after", "0")
        .expect_at_most(1)
        .create_async()
        .await;

    let req = test_request(&server, "/retry-test");
    let resp = req.send().await.unwrap();
    assert_eq!(resp.value, "ok");

    retry_mock.assert_async().await;
    success_mock.assert_async().await;
}

#[tokio::test]
async fn exhausts_retries_returns_rate_limit_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/always-429")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body(r#"{"error": "slow down"}"#)
        .expect(2)
        .create_async()
        .await;

    let req = test_request_with_retry(
        &server,
        "/always-429",
        RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 1,
            max_backoff_ms: 1,
        },
    );

    let err = req.send().await.unwrap_err();
    match err.0 {
        ApiError::RateLimit(msg) => {
            assert_eq!(msg, "slow down");
        }
        other => panic!("Expected RateLimit error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn non_429_error_does_not_retry() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/server-error")
        .with_status(500)
        .with_body(r#"{"error": "internal error"}"#)
        .expect(1)
        .create_async()
        .await;

    let req = test_request(&server, "/server-error");
    let err = req.send().await.unwrap_err();

    match err.0 {
        ApiError::Api { status, message } => {
            assert_eq!(status, 500);
            assert_eq!(message, "internal error");
        }
        other => panic!("Expected Api error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn from_response_parses_json_error_field() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/bad-input")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "bad input"}"#)
        .create_async()
        .await;

    let req = test_request(&server, "/bad-input");
    let err = req.send().await.unwrap_err();

    match err.0 {
        ApiError::Validation(msg) => {
            assert_eq!(
                msg, "bad input",
                "Should extract 'error' field from JSON, not raw body"
            );
        }
        other => panic!("Expected Validation error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn error_401_returns_authentication_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/unauthorized")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "unauthorized"}"#)
        .expect(1)
        .create_async()
        .await;

    let req = test_request(&server, "/unauthorized");
    let err = req.send().await.unwrap_err();

    match err.0 {
        ApiError::Authentication(msg) => {
            assert_eq!(msg, "unauthorized");
        }
        other => panic!("Expected Authentication error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn error_403_returns_authentication_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/forbidden")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "forbidden"}"#)
        .expect(1)
        .create_async()
        .await;

    let req = test_request(&server, "/forbidden");
    let err = req.send().await.unwrap_err();

    match err.0 {
        ApiError::Authentication(msg) => {
            assert_eq!(msg, "forbidden");
        }
        other => panic!("Expected Authentication error for 403, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn error_408_returns_timeout_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/timeout")
        .with_status(408)
        .with_body(r#"{"error": "request timeout"}"#)
        .expect(1)
        .create_async()
        .await;

    let req = test_request(&server, "/timeout");
    let err = req.send().await.unwrap_err();

    match err.0 {
        ApiError::Timeout => {}
        other => panic!("Expected Timeout error, got: {:?}", other),
    }

    mock.assert_async().await;
}

// ── Concurrency limiter integration ─────────────────────────────

#[tokio::test]
async fn send_raw_works_with_concurrency_limit() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/concurrent")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"value": "ok"}"#)
        .expect(2)
        .create_async()
        .await;

    let http = HttpClientBuilder::new(server.url())
        .with_max_concurrent(1)
        .build()
        .unwrap();

    // Two concurrent requests with concurrency=1 should both succeed
    let req1 = Request::<TestResponse, TestError>::new(http.clone(), "/concurrent");
    let req2 = Request::<TestResponse, TestError>::new(http, "/concurrent");

    let (r1, r2) = tokio::join!(req1.send(), req2.send());
    assert!(r1.is_ok());
    assert!(r2.is_ok());

    mock.assert_async().await;
}

#[tokio::test]
async fn retry_releases_permit_during_backoff() {
    let mut server = Server::new_async().await;

    // First call returns 429 with 1s retry-after, second returns 200
    let success_mock = server
        .mock("GET", "/retry-permit")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"value": "ok"}"#)
        .create_async()
        .await;

    let retry_mock = server
        .mock("GET", "/retry-permit")
        .with_status(429)
        .with_header("retry-after", "1")
        .expect_at_most(1)
        .create_async()
        .await;

    let http = HttpClientBuilder::new(server.url())
        .with_max_concurrent(1)
        .build()
        .unwrap();
    let http_clone = http.clone();

    // Spawn the retrying request
    let handle = tokio::spawn(async move {
        let req = Request::<TestResponse, TestError>::new(http, "/retry-permit");
        req.send().await.unwrap()
    });

    // Wait for first request to hit 429 and enter backoff
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Should be able to acquire permit during backoff (permit was released)
    let result =
        tokio::time::timeout(Duration::from_millis(100), http_clone.acquire_concurrency()).await;
    assert!(result.is_ok(), "Should acquire permit during retry backoff");
    // Drop permit immediately so the retry can proceed
    drop(result);

    let resp = handle.await.unwrap();
    assert_eq!(resp.value, "ok");

    retry_mock.assert_async().await;
    success_mock.assert_async().await;
}

#[tokio::test]
async fn concurrency_limit_serializes_requests() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/serial")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"value": "ok"}"#)
        .expect(4)
        .create_async()
        .await;

    let http = HttpClientBuilder::new(server.url())
        .with_max_concurrent(2)
        .build()
        .unwrap();

    // 4 concurrent requests with concurrency=2 should all succeed
    let mut handles = Vec::new();
    for _ in 0..4 {
        let h = http.clone();
        handles.push(tokio::spawn(async move {
            Request::<TestResponse, TestError>::new(h, "/serial")
                .send()
                .await
                .unwrap()
        }));
    }

    for h in handles {
        let resp = h.await.unwrap();
        assert_eq!(resp.value, "ok");
    }

    mock.assert_async().await;
}

// ── Method, body, authenticator and costs ───────────────────────

/// What a test authenticator saw and stamped on each attempt.
#[derive(Clone, Default)]
struct Seen(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl Seen {
    fn push(&self, entry: String) {
        self.0.lock().unwrap().push(entry);
    }

    fn entries(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

/// Stamps the attempt on the request and records the body it signs.
struct StampAttempt(Seen);

impl polyoxide_core::Authenticator for StampAttempt {
    async fn sign(
        &self,
        parts: &mut polyoxide_core::RequestParts,
        attempt: u32,
    ) -> Result<(), ApiError> {
        self.0.push(format!(
            "sign {attempt} {}",
            parts.body.as_deref().unwrap_or("-")
        ));
        parts.headers.insert("x-attempt", attempt.into());
        Ok(())
    }
}

/// A mock, not yet created, answering `method path` with `statuses` in turn,
/// then the last of them for good, logging each request as
/// `send <x-attempt> <body>`.
fn scripted(
    server: &mut mockito::ServerGuard,
    method: &str,
    path: &str,
    statuses: &[usize],
    seen: &Seen,
) -> mockito::Mock {
    let statuses = statuses.to_vec();
    let served = std::sync::atomic::AtomicUsize::new(0);
    let seen = seen.clone();
    server
        .mock(method, path)
        .match_query(mockito::Matcher::Any)
        .with_status_code_from_request(move |request| {
            let stamp = request
                .header("x-attempt")
                .first()
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-")
                .to_owned();
            let body = String::from_utf8_lossy(request.body().unwrap()).into_owned();
            seen.push(format!("send {stamp} {body}"));
            let n = served.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            statuses[n.min(statuses.len() - 1)]
        })
        .with_header("content-type", "application/json")
        .with_body(r#"{"value": "ok"}"#)
}

fn quick_retries() -> RetryConfig {
    RetryConfig {
        max_retries: 3,
        initial_backoff_ms: 1,
        max_backoff_ms: 5,
    }
}

#[tokio::test]
async fn a_post_sends_its_body_serialised_once() {
    #[derive(serde::Serialize)]
    struct Order {
        side: &'static str,
        price: &'static str,
    }

    let mut server = Server::new_async().await;
    let seen = Seen::default();
    // Through `serde_json::Value`, whose keys sort: `price` before `side`.
    let wire = r#"{"price":"0.5","side":"BUY"}"#;
    let mock = scripted(&mut server, "POST", "/order", &[429, 200], &seen)
        .match_header("content-type", "application/json")
        .match_body(wire)
        .expect(2)
        .create_async()
        .await;

    let http = HttpClientBuilder::new(server.url())
        .with_retry_config(quick_retries())
        .build()
        .unwrap();
    let auth = polyoxide_core::DynAuthenticator::new_arc(StampAttempt(seen.clone()));
    let resp = Request::<TestResponse, TestError>::new(http, "/order")
        .method(reqwest::Method::POST)
        .authenticator(auth)
        .body(&Order {
            side: "BUY",
            price: "0.5",
        })
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(resp.value, "ok");
    mock.assert_async().await;

    assert_eq!(
        seen.entries(),
        [
            format!("sign 0 {wire}"),
            format!("send 0 {wire}"),
            format!("sign 1 {wire}"),
            format!("send 1 {wire}"),
        ],
        "every attempt signs and sends the bytes serialised once"
    );
}

#[tokio::test]
async fn an_authenticator_signs_every_attempt_of_a_request() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let mock = scripted(&mut server, "GET", "/signed", &[429, 429, 200], &seen)
        .expect(3)
        .create_async()
        .await;

    let http = HttpClientBuilder::new(server.url())
        .with_retry_config(quick_retries())
        .build()
        .unwrap();
    let auth = polyoxide_core::DynAuthenticator::new_arc(StampAttempt(seen.clone()));
    let request = Request::<TestResponse, TestError>::new(http, "/signed").authenticator(auth);
    // A clone carries the authenticator, as a paginated walk's pages do.
    request.clone().send().await.unwrap();
    mock.assert_async().await;

    assert_eq!(
        seen.entries(),
        ["sign 0 -", "send 0 ", "sign 1 -", "send 1 ", "sign 2 -", "send 2 "],
        "each attempt is signed afresh, and the server sees that attempt's signature"
    );
}

/// A throttle that records the costs each attempt carries.
#[derive(Clone, Default)]
struct CostRecorder(std::sync::Arc<std::sync::Mutex<Vec<Vec<polyoxide_core::Cost>>>>);

impl polyoxide_core::Throttle for CostRecorder {
    async fn acquire(
        &self,
        meta: &polyoxide_core::RequestMeta<'_>,
    ) -> Result<polyoxide_core::Charge, polyoxide_core::Refused> {
        self.0.lock().unwrap().push(meta.costs.to_vec());
        Ok(polyoxide_core::Charge::none())
    }

    fn observe(
        &self,
        _charge: &polyoxide_core::Charge,
        _response: &polyoxide_core::ResponseMeta<'_>,
        _attempt: &polyoxide_core::AttemptInfo,
    ) {
    }

    fn hold(&self, _delay: Duration) {}
}

#[tokio::test]
async fn a_request_s_costs_reach_the_throttle() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let mock = scripted(&mut server, "DELETE", "/orders", &[429, 200], &seen)
        .expect(2)
        .create_async()
        .await;

    let throttle = CostRecorder::default();
    let http = HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .with_retry_config(quick_retries())
        .build()
        .unwrap();
    let write = polyoxide_core::Cost {
        layer: polyoxide_core::LayerId("write"),
        units: 7,
        exact: true,
    };
    let read = polyoxide_core::Cost {
        layer: polyoxide_core::LayerId("read"),
        units: 1,
        exact: false,
    };
    Request::<TestResponse, TestError>::new(http, "/orders")
        .method(reqwest::Method::DELETE)
        .with_cost(write)
        .with_cost(read)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    assert_eq!(
        *throttle.0.lock().unwrap(),
        [vec![write, read], vec![write, read]],
        "every attempt is charged the request's costs, in order"
    );
}

#[tokio::test]
async fn a_request_parts_timeout_bounds_its_attempt() {
    // A listener that never accepts: the connection opens and no answer comes.
    let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let http = HttpClientBuilder::new(format!("http://{}", silent.local_addr().unwrap()))
        .build()
        .unwrap();
    let mut parts = polyoxide_core::RequestParts::new(reqwest::Method::GET, "/slow");
    parts.timeout = Some(Duration::from_millis(200));

    let start = std::time::Instant::now();
    let err = http.send(parts, &[], None).await.unwrap_err();
    let elapsed = start.elapsed();
    assert!(
        matches!(&err, ApiError::Network(e) if e.is_timeout()),
        "{err:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(150) && elapsed < Duration::from_secs(5),
        "the attempt ended after {elapsed:?}, not at its own 200ms timeout \
         (the client's is 30s)"
    );
}
