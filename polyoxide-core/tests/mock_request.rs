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
