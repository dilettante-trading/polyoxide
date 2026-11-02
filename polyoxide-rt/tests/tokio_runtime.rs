//! The runtime seam, driven through `TokioRuntime` on tokio's paused clock.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use polyoxide_rt::{
    default_runtime, spawn, timeout, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime,
    UNIX_EPOCH,
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

#[tokio::test(start_paused = true)]
async fn spawn_goes_through_the_runtime_it_is_given() {
    let rt = Scripted::new();
    let task = spawn(&rt, async { 5 });
    assert_eq!(rt.spawned.load(Ordering::SeqCst), 1);
    assert_eq!(task.await, Some(5));
}

#[tokio::test(start_paused = true)]
async fn a_task_yields_its_output() {
    let task = spawn(&TokioRuntime, async {
        TokioRuntime.sleep(Duration::from_secs(2)).await;
        "done"
    });
    assert_eq!(task.await, Some("done"));
}

#[tokio::test(start_paused = true)]
async fn abort_stops_a_task_before_its_side_effect() {
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let mut task = spawn(&TokioRuntime, async move {
        TokioRuntime.sleep(Duration::from_secs(10)).await;
        flag.store(true, Ordering::SeqCst);
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished(), "the task is still sleeping");

    task.abort();
    assert_eq!((&mut task).await, None);
    TokioRuntime.sleep(Duration::from_secs(60)).await;

    assert!(
        !ran.load(Ordering::SeqCst),
        "an aborted task still ran to its end"
    );
    assert!(task.is_finished(), "an aborted task is not finished");
}

#[tokio::test(start_paused = true)]
async fn is_finished_turns_true_when_the_task_ends() {
    let task = spawn(&TokioRuntime, async {
        TokioRuntime.sleep(Duration::from_secs(5)).await;
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished(), "the task is still sleeping");

    // The task is not polled here, so only the task's own end can have set it.
    TokioRuntime.sleep(Duration::from_secs(6)).await;
    assert!(task.is_finished(), "a task that ended is not finished");
    assert_eq!(task.await, Some(()));
}

#[tokio::test(start_paused = true)]
async fn a_dropped_task_keeps_running() {
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let task = spawn(&TokioRuntime, async move {
        TokioRuntime.sleep(Duration::from_secs(5)).await;
        flag.store(true, Ordering::SeqCst);
    });
    drop(task);

    TokioRuntime.sleep(Duration::from_secs(10)).await;
    assert!(ran.load(Ordering::SeqCst), "dropping the task stopped it");
}

async fn fail_on_purpose() -> u32 {
    panic!("the task fails on purpose")
}

#[tokio::test(start_paused = true)]
async fn a_panicking_task_ends_with_no_output() {
    let task = spawn(&TokioRuntime, fail_on_purpose());
    TokioRuntime.sleep(Duration::from_secs(1)).await;
    assert!(task.is_finished(), "a task that panicked is not finished");
    assert_eq!(task.await, None);
}

#[tokio::test(start_paused = true)]
async fn timeout_returns_the_output_of_a_future_that_finishes_in_time() {
    let start = TokioRuntime.now();
    let output = timeout(&TokioRuntime, Duration::from_secs(10), async {
        TokioRuntime.sleep(Duration::from_secs(2)).await;
        7
    })
    .await;
    assert_eq!(output, Some(7));
    assert_near(TokioRuntime.now() - start, Duration::from_secs(2));
}

#[tokio::test(start_paused = true)]
async fn timeout_gives_up_at_the_deadline() {
    let start = TokioRuntime.now();
    let output = timeout(
        &TokioRuntime,
        Duration::from_secs(10),
        TokioRuntime.sleep(Duration::from_secs(60)),
    )
    .await;
    assert_eq!(output, None);
    assert_near(TokioRuntime.now() - start, Duration::from_secs(10));
}

#[tokio::test(start_paused = true)]
async fn timeout_at_zero_lets_a_ready_future_win() {
    assert_eq!(
        timeout(&TokioRuntime, Duration::ZERO, async { 42 }).await,
        Some(42)
    );
}

#[tokio::test(start_paused = true)]
async fn timeout_at_zero_expires_a_pending_future() {
    assert_eq!(
        timeout(&TokioRuntime, Duration::ZERO, std::future::pending::<()>()).await,
        None
    );
}

#[tokio::test(start_paused = true)]
async fn timeout_accepts_the_maximum_duration() {
    assert_eq!(
        timeout(&TokioRuntime, Duration::MAX, async { 1 }).await,
        Some(1)
    );
    // A timer of the longest length does not fire within a year.
    let year = Duration::from_secs(86_400 * 365);
    let never = timeout(&TokioRuntime, Duration::MAX, std::future::pending::<()>());
    assert!(tokio::time::timeout(year, never).await.is_err());
}

#[tokio::test]
async fn timeout_polls_the_future_before_the_timer() {
    // This runtime's sleeps are always ready, so only the order of the two
    // polls lets the future's output through.
    let rt = Scripted::new();
    assert_eq!(
        timeout(&rt, Duration::from_secs(3), async { 9 }).await,
        Some(9)
    );
}

#[tokio::test]
async fn timeout_sleeps_on_the_runtime_it_is_given() {
    let rt = Scripted::new();
    let output = timeout(&rt, Duration::from_secs(3), std::future::pending::<()>()).await;
    assert_eq!(output, None);
    assert_eq!(*rt.slept.lock().unwrap(), vec![Duration::from_secs(3)]);
}
