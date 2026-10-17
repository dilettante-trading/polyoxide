//! `HttpClient::health` on Polymarket's tables: a 429 a ping sees holds the
//! client's next request.

use std::time::{Duration, Instant};

use mockito::{Mock, Server, ServerGuard};
use polyoxide_core::{ApiError, HttpClientBuilder, RetryConfig};
use polyoxide_polymarket::shared;

/// A caller's error that says whether a response reached its decode.
#[derive(Debug)]
enum PingError {
    Response(u16),
    // Read through `Debug` alone, in the assertion messages.
    #[allow(dead_code)]
    Api(ApiError),
}

/// The caller's one decode: a response becomes `Response`, anything else
/// stays core's.
impl From<ApiError> for PingError {
    fn from(e: ApiError) -> Self {
        match e {
            ApiError::Response(response) => Self::Response(response.status.as_u16()),
            other => Self::Api(other),
        }
    }
}

/// A route answering `statuses` in turn, then the last of them for good.
async fn scripted(server: &mut ServerGuard, path: &str, statuses: &'static [usize]) -> Mock {
    let served = std::sync::atomic::AtomicUsize::new(0);
    server
        .mock("GET", path)
        .with_status_code_from_request(move |_| {
            let n = served.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            statuses[n.min(statuses.len() - 1)]
        })
        .with_body("OK")
        .expect(statuses.len())
        .create_async()
        .await
}

/// A retry schedule whose first backoff is at least 300ms: 330ms after
/// jitter, so a 429's hold still has 300ms left when the next call starts.
fn a_300ms_floor(max_retries: u32) -> RetryConfig {
    RetryConfig {
        max_retries,
        initial_backoff_ms: 440,
        max_backoff_ms: 10_000,
    }
}

#[tokio::test]
async fn a_429_on_health_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let mock = scripted(&mut server, "/status", &[429, 200]).await;
    let http = HttpClientBuilder::new(server.url())
        .with_rate_limiter(shared::gamma_limits())
        .with_retry_config(a_300ms_floor(0))
        .build()
        .unwrap();

    // No retry left: the 429 is the caller's, and it still holds the client.
    let err = http
        .health::<PingError>("/status", &[])
        .await
        .expect_err("a 429 and no retry");
    assert!(matches!(err, PingError::Response(429)), "{err:?}");

    let start = Instant::now();
    let pong = http.health::<PingError>("/status", &[]).await.unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(300),
        "the next ping went after {elapsed:?}, inside the 429's hold"
    );
    assert!(
        elapsed.saturating_sub(pong.round_trip) >= Duration::from_millis(300),
        "the round trip leaves out the hold: {:?} of {elapsed:?}",
        pong.round_trip
    );
    mock.assert_async().await;
}
