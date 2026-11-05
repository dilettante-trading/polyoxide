//! The runtime a client is built on, and the plumbing that carries it: from a
//! config or a builder to the client and its `with_base_url` siblings. Tests
//! further down prove that each wait and clock read of core goes through it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use polyoxide_core::{
    ApiError, CapacityBucket, ClientConfig, Hold, HttpClientBuilder, LayerId, RetryConfig,
    WindowQuotaTable,
};
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

    fn yield_now(&self) -> BoxFuture<()> {
        Box::pin(tokio::task::yield_now())
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

#[tokio::test]
async fn a_hold_reads_the_clock_and_sleeps_on_its_runtime() {
    let counting = Arc::new(Counting::default());
    let hold = Hold::unbounded_on(counting.clone());

    hold.extend(Duration::from_millis(60));
    hold.wait().await;

    let slept = counting.sleeps();
    assert!(
        slept
            .iter()
            .any(|d| *d > Duration::ZERO && *d <= Duration::from_millis(60)),
        "the wait for the hold did not sleep on the runtime: {slept:?}"
    );
    assert!(
        counting.reads() > 0,
        "the hold never read the runtime's clock"
    );
}

#[tokio::test]
async fn a_ceiling_clamps_what_the_runtime_sleeps() {
    let counting = Arc::new(Counting::default());
    let hold = Hold::with_ceiling_on(Duration::from_millis(40), counting.clone());

    hold.extend(Duration::from_secs(3_600));
    hold.wait().await;

    let slept = counting.sleeps();
    assert!(
        !slept.is_empty() && slept.iter().all(|d| *d <= Duration::from_millis(40)),
        "an hour's hold was not clamped to the 40 ms ceiling: {slept:?}"
    );
}

#[tokio::test]
async fn a_bucket_refills_and_waits_on_its_holds_runtime() {
    let counting = Arc::new(Counting::default());
    let hold = Hold::unbounded_on(counting.clone());
    let bucket = CapacityBucket::new(LayerId("test"), 1, 20, hold);

    bucket.acquire(1, true).await.unwrap();
    bucket.acquire(1, true).await.unwrap();

    // One token refills in 50 ms at 20 a second.
    let slept = counting.sleeps();
    assert!(
        slept
            .iter()
            .any(|d| *d > Duration::ZERO && *d <= Duration::from_millis(50)),
        "the wait for a token did not sleep on the runtime: {slept:?}"
    );
    assert!(
        counting.reads() > 0,
        "the bucket never read the runtime's clock"
    );
}

#[tokio::test]
async fn a_window_quota_waits_on_its_runtime() {
    let counting = Arc::new(Counting::default());
    let mut table = WindowQuotaTable::new(11, Duration::from_secs(1));
    table.with_runtime(counting.clone());
    let limiter = table.build();

    limiter.acquire("/rows", None).await;
    limiter.acquire("/rows", None).await;

    // The general bucket holds one token and paces the rest.
    let interval = WindowQuotaTable::paced_interval(11, Duration::from_secs(1));
    let slept = counting.sleeps();
    assert!(
        slept.iter().any(|d| *d > Duration::ZERO && *d <= interval),
        "the second request did not wait for its bucket on the runtime: {slept:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_window_quota_paces_on_the_runtimes_clock() {
    let mut table = WindowQuotaTable::new(11, Duration::from_secs(1));
    table.with_runtime(tokio_runtime());
    let limiter = table.build();

    limiter.acquire("/rows", None).await;
    let start = tokio::time::Instant::now();
    limiter.acquire("/rows", None).await;

    // On a paused clock the sleep is the only thing that moves time, so the
    // second request waits one paced interval of virtual time, not the minutes
    // a bucket on another clock spins through.
    let interval = WindowQuotaTable::paced_interval(11, Duration::from_secs(1));
    let waited = start.elapsed();
    assert!(
        waited >= interval && waited <= interval + Duration::from_millis(5),
        "the second request waited {waited:?} of virtual time for an interval of {interval:?}"
    );
}
