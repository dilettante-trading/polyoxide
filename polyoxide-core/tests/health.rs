//! `HttpClient::health`, the one ping every venue's `ping` calls.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mockito::{Mock, Server, ServerGuard};
use polyoxide_core::{
    polymarket, ApiError, AttemptInfo, Charge, Cost, HttpClientBuilder, LayerId, Refused,
    RequestMeta, ResponseMeta, RetryConfig, Throttle,
};

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
async fn health_reports_the_round_trip_of_the_attempt_that_answered() {
    let mut server = Server::new_async().await;
    let mock = scripted(&mut server, "/status", &[429, 200]).await;
    let http = HttpClientBuilder::new(server.url())
        .with_retry_config(a_300ms_floor(1))
        .build()
        .unwrap();

    let start = Instant::now();
    let pong = http.health::<PingError>("/status", &[]).await.unwrap();
    let elapsed = start.elapsed();
    mock.assert_async().await;

    assert_eq!(pong.response.status(), 200);
    assert_eq!(pong.response.text().await.unwrap(), "OK");
    assert!(
        elapsed >= Duration::from_millis(300),
        "the call took {elapsed:?}, inside the retry's backoff"
    );
    assert!(
        pong.round_trip > Duration::ZERO
            && elapsed.saturating_sub(pong.round_trip) >= Duration::from_millis(300),
        "the round trip is the answering attempt's, without the backoff: {:?} of {elapsed:?}",
        pong.round_trip
    );
}

#[tokio::test]
async fn health_waits_for_the_permit() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/status")
        .with_status(200)
        .create_async()
        .await;
    let http = HttpClientBuilder::new(server.url())
        .with_max_concurrent(1)
        .build()
        .unwrap();

    // Hold the only permit, so a ping that respects the gate must queue.
    let _permit = http.acquire_concurrency().await.unwrap();
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        http.health::<PingError>("/status", &[]),
    )
    .await;
    assert!(
        result.is_err(),
        "the ping answered while the only permit was held"
    );
}

#[tokio::test]
async fn a_non_2xx_health_is_the_callers_error() {
    let mut server = Server::new_async().await;
    let mock = scripted(&mut server, "/status", &[503]).await;
    let http = HttpClientBuilder::new(server.url()).build().unwrap();

    let err = http
        .health::<PingError>("/status", &[])
        .await
        .expect_err("a 503 is not a pong");
    assert!(matches!(err, PingError::Response(503)), "{err:?}");
    mock.assert_async().await;
}

#[tokio::test]
async fn a_429_on_health_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let mock = scripted(&mut server, "/status", &[429, 200]).await;
    let http = HttpClientBuilder::new(server.url())
        .with_rate_limiter(polymarket::gamma_limits())
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

/// A throttle that records the costs each attempt is charged.
#[derive(Clone, Default)]
struct CostRecorder(Arc<Mutex<Vec<Vec<Cost>>>>);

impl Throttle for CostRecorder {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.0.lock().unwrap().push(meta.costs.to_vec());
        Ok(Charge::none())
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, _delay: Duration) {}
}

#[tokio::test]
async fn health_charges_its_costs() {
    let mut server = Server::new_async().await;
    let mock = scripted(&mut server, "/fapi/v1/ping", &[429, 200]).await;
    let throttle = CostRecorder::default();
    let http = HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .with_retry_config(RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
        })
        .build()
        .unwrap();
    let weight = Cost {
        layer: LayerId("weight"),
        units: 1,
        exact: true,
    };

    http.health::<PingError>("/fapi/v1/ping", &[weight])
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(
        *throttle.0.lock().unwrap(),
        [vec![weight], vec![weight]],
        "every attempt is charged the ping's costs"
    );
}

/// A throttle that refuses every charge.
struct RefusingThrottle;

impl Throttle for RefusingThrottle {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        Err(Refused {
            layer: meta.costs[0].layer,
            units: meta.costs[0].units,
            capacity: 0,
        })
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, _delay: Duration) {}
}

#[tokio::test]
async fn a_refused_health_cost_is_the_callers_error() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/fapi/v1/ping")
        .expect(0)
        .create_async()
        .await;
    let http = HttpClientBuilder::new(server.url())
        .with_throttle(RefusingThrottle)
        .build()
        .unwrap();
    let weight = Cost {
        layer: LayerId("weight"),
        units: 1,
        exact: true,
    };

    let err = http
        .health::<PingError>("/fapi/v1/ping", &[weight])
        .await
        .expect_err("the throttle refused the ping's cost");
    assert!(
        matches!(&err, PingError::Api(ApiError::Refused(r)) if r.layer == LayerId("weight")),
        "{err:?}"
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn a_transport_failure_on_health_is_the_callers_error() {
    // A port nothing listens on: the connection is refused.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = closed.local_addr().unwrap();
    drop(closed);
    let http = HttpClientBuilder::new(format!("http://{addr}"))
        .build()
        .unwrap();

    let err = http
        .health::<PingError>("/status", &[])
        .await
        .expect_err("nothing listens there");
    assert!(
        matches!(&err, PingError::Api(ApiError::Network(_))),
        "{err:?}"
    );
}
