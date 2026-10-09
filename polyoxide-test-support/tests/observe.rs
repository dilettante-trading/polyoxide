//! The soak observer sees the throttle core's client hides.
//!
//! A 429 that the retry loop waits out reaches the caller as `Ok`. The only
//! trace it leaves is a `WARN` under the `polyoxide_core` target, so these
//! tests drive core's `HttpClient` into one, through both of its retry loops,
//! and require the observer to have counted it. The layer is scoped with
//! `with_default`: a global subscriber would leak between tests sharing a
//! process.

use std::{future::Future, sync::Arc, time::Instant};

use mockito::{Server, ServerGuard};
use polyoxide_core::{ApiError, HttpClient, HttpClientBuilder, Request, RequestError, RetryConfig};
use polyoxide_test_support::soak::observe::{ThrottleLayer, ThrottleObserver};
use tracing_subscriber::layer::SubscriberExt;

/// One quick retry, so the test does not wait out a real backoff.
fn client(server: &ServerGuard) -> HttpClient {
    HttpClientBuilder::new(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 1,
            max_backoff_ms: 5,
        })
        .build()
        .unwrap()
}

/// Runs `body` on a current-thread runtime under a subscriber holding only
/// the throttle layer, and returns the observer with the body's output.
fn observed<F: Future>(body: impl FnOnce() -> F) -> (Arc<ThrottleObserver>, F::Output) {
    let observer = Arc::new(ThrottleObserver::new(Instant::now()));
    let subscriber = tracing_subscriber::registry().with(ThrottleLayer(Arc::clone(&observer)));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let output = tracing::subscriber::with_default(subscriber, || runtime.block_on(body()));
    (observer, output)
}

/// Answers `path` with a 429 once, then a 200, and asserts both were served.
async fn throttled_once<T>(path: &str, send: impl AsyncFnOnce(HttpClient) -> T) -> T {
    let mut server = Server::new_async().await;
    let limited = server
        .mock("GET", path)
        .with_status(429)
        .with_body("error code: 1015")
        .expect(1)
        .create_async()
        .await;
    let ok = server
        .mock("GET", path)
        .with_status(200)
        .with_body("[]")
        .expect(1)
        .create_async()
        .await;
    let output = send(client(&server)).await;
    limited.assert_async().await;
    ok.assert_async().await;
    output
}

#[test]
fn a_429_retried_away_by_get_bytes_is_counted_at_warn() {
    let (observer, body) = observed(|| {
        throttled_once("/v1/rows", async |client: HttpClient| {
            client.get_bytes("/v1/rows", &[]).await
        })
    });
    assert_eq!(
        body.unwrap(),
        b"[]",
        "the caller sees only the retried success"
    );
    assert_eq!(
        observer.throttle_count(),
        1,
        "{:?}",
        observer.warn_samples()
    );
    assert_eq!(observer.other_warning_count(), 0);
    assert_eq!(
        observer.throttles_by_path().get("/v1/rows"),
        Some(&1),
        "{:?}",
        observer.warn_samples()
    );
    let sample = &observer.warn_samples()[0];
    assert!(
        sample.starts_with("Retriable status 429 Too Many Requests on /v1/rows, retry 1 after "),
        "{sample}"
    );
}

/// The error type a `Request` needs; the test only ever sees `Ok`.
#[derive(Debug)]
struct Refused(#[allow(dead_code)] ApiError);

impl From<ApiError> for Refused {
    fn from(err: ApiError) -> Self {
        Self(err)
    }
}

impl RequestError for Refused {
    async fn from_response(response: reqwest::Response) -> Self {
        Self(ApiError::from_response(response).await)
    }
}

#[test]
fn a_429_retried_away_by_a_request_is_counted_at_warn() {
    let (observer, rows) = observed(|| {
        throttled_once("/v1/rows", async |client: HttpClient| {
            Request::<Vec<u8>, Refused>::new(client, "/v1/rows")
                .send()
                .await
        })
    });
    assert_eq!(rows.unwrap(), Vec::<u8>::new());
    assert_eq!(
        observer.throttle_count(),
        1,
        "{:?}",
        observer.warn_samples()
    );
    assert_eq!(observer.throttles_by_path().get("/v1/rows"), Some(&1));
}

#[test]
fn a_warning_outside_core_is_not_counted() {
    let (observer, ()) = observed(|| async {
        tracing::warn!("Retriable status 429 Too Many Requests on /v1/rows, retry 1 after 5ms");
        tracing::warn!(
            target: "polyoxide_core::client",
            "Retriable status 425 Too Early on /v1/rows, retry 1 after 5ms"
        );
        tracing::info!(
            target: "polyoxide_core::client",
            "Retriable status 429 Too Many Requests on /v1/rows, retry 1 after 5ms"
        );
    });
    assert_eq!(
        observer.throttle_count(),
        0,
        "another target, or another level"
    );
    assert_eq!(
        observer.other_warning_count(),
        1,
        "a 425 under core is a warning"
    );
}
