//! The runtime a client is built on, and the plumbing that carries it: from a
//! config or a builder to the client and its `with_base_url` siblings. Tests
//! further down prove that each wait and clock read of core goes through it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use polyoxide_core::{ApiError, ClientConfig, HttpClientBuilder, RetryConfig};
use polyoxide_rt::{BoxFuture, Instant, Runtime, TokioRuntime};

/// A runtime over tokio's that records its sleeps, in order, and counts its
/// clock reads. `sleep_until` is the trait's default, so it counts as a sleep
/// and a read.
#[derive(Debug, Default)]
struct Counting {
    inner: TokioRuntime,
    sleeps: Mutex<Vec<Duration>>,
    reads: AtomicUsize,
}

impl Counting {
    fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.lock().unwrap().clone()
    }

    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl Runtime for Counting {
    fn spawn(&self, task: BoxFuture<()>) {
        self.inner.spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        self.sleeps.lock().unwrap().push(duration);
        self.inner.sleep(duration)
    }

    fn now(&self) -> Instant {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.now()
    }
}

fn tokio_runtime() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}

#[test]
fn a_config_runtime_is_the_clients() {
    let runtime = tokio_runtime();
    let mut config = ClientConfig::new("https://example.test", 4);
    assert!(config.runtime.is_none(), "a new config names no runtime");
    config.runtime = Some(Arc::clone(&runtime));

    let http = config.http_builder().build().unwrap();
    assert!(Arc::ptr_eq(http.runtime(), &runtime));
}

#[test]
fn a_builders_runtime_is_shared_with_its_siblings() {
    let runtime = tokio_runtime();
    let http = HttpClientBuilder::new("https://example.test")
        .runtime(Arc::clone(&runtime))
        .build()
        .unwrap();
    let sibling = http.with_base_url("https://other.example.test").unwrap();

    assert!(Arc::ptr_eq(http.runtime(), &runtime));
    assert!(Arc::ptr_eq(sibling.runtime(), &runtime));
}

#[test]
fn an_unset_runtime_is_the_targets_default() {
    // Native targets default to tokio's runtime.
    let http = HttpClientBuilder::new("https://example.test")
        .build()
        .unwrap();
    assert_eq!(format!("{:?}", http.runtime()), "TokioRuntime");

    let config = ClientConfig::new("https://example.test", 4);
    assert_eq!(format!("{:?}", config.runtime_or_default()), "TokioRuntime");
}

struct Knobs {
    config: ClientConfig,
}

// Only `runtime` is read here; the other setters are the macro's.
#[allow(dead_code)]
impl Knobs {
    polyoxide_core::client_config_setters!(config);
}

#[test]
fn the_runtime_setter_writes_the_config() {
    let runtime = tokio_runtime();
    let knobs = Knobs {
        config: ClientConfig::new("https://example.test", 4),
    }
    .runtime(Arc::clone(&runtime));

    let set = knobs
        .config
        .runtime
        .as_ref()
        .expect("the setter wrote the config");
    assert!(Arc::ptr_eq(set, &runtime));
    assert!(Arc::ptr_eq(&knobs.config.runtime_or_default(), &runtime));
}

#[tokio::test]
async fn a_429_retry_sleeps_through_the_injected_runtime() {
    let mut server = mockito::Server::new_async().await;
    // mockito answers with the first matching mock still short of its expected
    // hits, so the 429 mock goes first with `expect(1)`: it answers once, and
    // the success mock, made second, answers the retry.
    let throttled = server
        .mock("GET", "/rows")
        .with_status(429)
        .expect(1)
        .create_async()
        .await;
    let success = server
        .mock("GET", "/rows")
        .with_status(200)
        .with_body("ok")
        .create_async()
        .await;

    let counting = Arc::new(Counting::default());
    let http = HttpClientBuilder::new(server.url())
        .runtime(counting.clone())
        .with_retry_config(RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 40,
            max_backoff_ms: 1_000,
        })
        .build()
        .unwrap();

    let body = http.get_bytes("/rows", &[]).await.unwrap();
    assert_eq!(body, b"ok".to_vec());
    throttled.assert_async().await;
    success.assert_async().await;

    // The retry's floor is attempt 0's backoff: 40 ms, less or plus a quarter.
    let slept = counting.sleeps();
    assert!(
        slept
            .iter()
            .any(|d| (Duration::from_millis(30)..=Duration::from_millis(50)).contains(d)),
        "no sleep of the retry's backoff went through the runtime: {slept:?}"
    );
}

#[tokio::test]
async fn health_times_its_attempt_on_the_injected_runtime() {
    let mut server = mockito::Server::new_async().await;
    let _status = server
        .mock("GET", "/status")
        .with_status(200)
        .create_async()
        .await;

    let counting = Arc::new(Counting::default());
    let http = HttpClientBuilder::new(server.url())
        .runtime(counting.clone())
        .build()
        .unwrap();

    http.health::<ApiError>("/status", &[]).await.unwrap();

    // The stopwatch reads the clock when the attempt is signed and again when
    // the answer is in.
    assert!(
        counting.reads() >= 2,
        "the stopwatch read the runtime's clock {} times",
        counting.reads()
    );
}
