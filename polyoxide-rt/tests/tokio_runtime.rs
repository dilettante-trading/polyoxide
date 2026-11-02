//! The runtime seam, driven through `TokioRuntime` on tokio's paused clock.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use polyoxide_rt::{
    default_runtime, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime, UNIX_EPOCH,
};

/// The paused clock wakes a timer on its millisecond, so a measured wait is its
/// duration plus less than this.
const TICK: Duration = Duration::from_millis(5);

fn assert_near(actual: Duration, expected: Duration) {
    assert!(
        actual >= expected && actual < expected + TICK,
        "measured {actual:?}, expected {expected:?}"
    );
}

/// A runtime that records what it is asked, to pin the trait's own behaviour:
/// its sleeps are always ready, its clock stands ten seconds after `base`, and
/// its spawn is tokio's.
#[derive(Debug)]
struct Scripted {
    base: Instant,
    spawned: AtomicUsize,
    slept: Mutex<Vec<Duration>>,
}

impl Scripted {
    fn new() -> Self {
        Self {
            base: Instant::now(),
            spawned: AtomicUsize::new(0),
            slept: Mutex::new(Vec::new()),
        }
    }
}

impl Runtime for Scripted {
    fn spawn(&self, task: BoxFuture<()>) {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        self.slept.lock().unwrap().push(duration);
        Box::pin(async {})
    }

    fn now(&self) -> Instant {
        self.base + Duration::from_secs(10)
    }
}

#[test]
fn the_clock_types_are_stds_on_native() {
    let _: std::time::Instant = Instant::now();
    let _: std::time::SystemTime = SystemTime::now();
    assert_eq!(UNIX_EPOCH, std::time::UNIX_EPOCH);
}

#[tokio::test(start_paused = true)]
async fn sleep_completes_after_its_duration() {
    let rt = TokioRuntime;
    let start = rt.now();
    rt.sleep(Duration::from_secs(5)).await;
    assert_near(rt.now() - start, Duration::from_secs(5));
}

#[tokio::test(start_paused = true)]
async fn sleep_until_waits_for_the_deadline() {
    let rt = TokioRuntime;
    let start = rt.now();
    rt.sleep_until(start + Duration::from_secs(3)).await;
    assert_near(rt.now() - start, Duration::from_secs(3));
}

#[tokio::test(start_paused = true)]
async fn sleep_until_a_passed_deadline_returns_at_once() {
    let rt = TokioRuntime;
    let deadline = rt.now();
    tokio::time::advance(Duration::from_secs(9)).await;
    let before = rt.now();
    rt.sleep_until(deadline).await;
    assert!(
        rt.now() - before < TICK,
        "a deadline already passed still waited {:?}",
        rt.now() - before
    );
}

#[tokio::test(start_paused = true)]
async fn the_default_runtime_runs_on_tokios_clock() {
    let rt = default_runtime();
    let start = rt.now();
    tokio::time::advance(Duration::from_secs(7)).await;
    assert_near(rt.now() - start, Duration::from_secs(7));
    rt.sleep(Duration::from_secs(2)).await;
    assert_near(rt.now() - start, Duration::from_secs(9));
}

#[test]
fn the_default_sleep_until_sleeps_the_time_left() {
    let rt = Scripted::new();
    drop(rt.sleep_until(rt.base + Duration::from_secs(14)));
    assert_eq!(*rt.slept.lock().unwrap(), vec![Duration::from_secs(4)]);
}

#[test]
fn the_default_sleep_until_a_passed_deadline_sleeps_zero() {
    let rt = Scripted::new();
    drop(rt.sleep_until(rt.base + Duration::from_secs(3)));
    assert_eq!(*rt.slept.lock().unwrap(), vec![Duration::ZERO]);
}
