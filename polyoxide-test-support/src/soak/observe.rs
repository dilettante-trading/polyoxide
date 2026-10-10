//! Throttle detection for the rate-limit harnesses.
//!
//! A 429 the client retries away is invisible to the caller: the retry loops
//! in `polyoxide-core` log a `WARN` and then return `Ok`. A harness that
//! counts successes alone would report a clean run through an hour of
//! throttling. So detection runs through a `tracing` layer, [`ThrottleLayer`],
//! that counts `polyoxide-core`'s warnings into a [`ThrottleObserver`].
//!
//! The retry loops are the only `warn!` call sites in that crate, so target
//! and level identify them without matching on message text. The text is
//! read afterwards only to tell a 429 from a 425, and to say which path was
//! refused.

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use tracing::field::{Field, Visit};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

/// The target prefix of every event `polyoxide-core` logs.
pub const CORE_TARGET: &str = "polyoxide_core";

const MAX_WARN_SAMPLES: usize = 8;

/// A warning emitted by `polyoxide-core`'s retry loop, classified by whether
/// it is the rate-limit signal these harnesses look for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarnKind {
    /// `Retriable status 429` — upstream throttled us.
    Throttle,
    /// Some other retriable status (e.g. `425 Too Early`), or a warning added
    /// to core after this was written. Reported, but never a failure.
    Other,
}

/// Whether a retry-loop warning is a throttle.
pub fn classify(message: &str) -> WarnKind {
    if message.contains("Retriable status 429") {
        WarnKind::Throttle
    } else {
        WarnKind::Other
    }
}

/// The request path in a retry-loop warning (`Retriable status 429 Too Many
/// Requests on /v1/rows, retry 1 after 500ms`), so a run over several routes
/// says which one was refused.
pub fn throttled_path(message: &str) -> Option<&str> {
    let rest = message.split(" on ").nth(1)?;
    Some(rest.split(',').next()?.trim())
}

/// Shared tally of what the retry loops logged.
///
/// `throttles` is atomic and read on every request iteration so a driver can
/// abort the instant a 429 lands, without contending on the sample mutex.
#[derive(Debug)]
pub struct ThrottleObserver {
    start: Instant,
    throttles: AtomicU64,
    other_warnings: AtomicU64,
    /// Micros since `start` of the first throttle; `u64::MAX` means none yet.
    first_throttle_micros: AtomicU64,
    samples: Mutex<Vec<String>>,
    /// Throttles per request path, `?` when the message names none.
    by_path: Mutex<BTreeMap<String, u64>>,
}

impl ThrottleObserver {
    /// An empty tally, timing throttles from `start`.
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            throttles: AtomicU64::new(0),
            other_warnings: AtomicU64::new(0),
            first_throttle_micros: AtomicU64::new(u64::MAX),
            samples: Mutex::new(Vec::new()),
            by_path: Mutex::new(BTreeMap::new()),
        }
    }

    /// Counts one retry-loop warning.
    pub fn record(&self, message: &str) {
        match classify(message) {
            WarnKind::Throttle => {
                self.throttles.fetch_add(1, Ordering::Relaxed);
                let elapsed = self.start.elapsed().as_micros() as u64;
                // Only the first writer wins; later throttles leave it alone.
                let _ = self.first_throttle_micros.compare_exchange(
                    u64::MAX,
                    elapsed,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
                let path = throttled_path(message).unwrap_or("?").to_owned();
                *lock(&self.by_path).entry(path).or_insert(0) += 1;
            }
            WarnKind::Other => {
                self.other_warnings.fetch_add(1, Ordering::Relaxed);
            }
        }

        let mut samples = lock(&self.samples);
        if samples.len() < MAX_WARN_SAMPLES {
            samples.push(message.to_owned());
        }
    }

    /// Whether any throttle was seen.
    pub fn throttled(&self) -> bool {
        self.throttle_count() > 0
    }

    /// Throttles seen.
    pub fn throttle_count(&self) -> u64 {
        self.throttles.load(Ordering::Relaxed)
    }

    /// Warnings seen that were not throttles.
    pub fn other_warning_count(&self) -> u64 {
        self.other_warnings.load(Ordering::Relaxed)
    }

    /// When the first throttle was seen, from `start`.
    pub fn first_throttle_at(&self) -> Option<Duration> {
        match self.first_throttle_micros.load(Ordering::Relaxed) {
            u64::MAX => None,
            micros => Some(Duration::from_micros(micros)),
        }
    }

    /// The first few warnings, verbatim.
    pub fn warn_samples(&self) -> Vec<String> {
        lock(&self.samples).clone()
    }

    /// Throttles per request path.
    pub fn throttles_by_path(&self) -> BTreeMap<String, u64> {
        lock(&self.by_path).clone()
    }

    /// Clears the tally so one process can run several independent trials,
    /// each judged on its own requests rather than a running total.
    pub fn reset(&self) {
        self.throttles.store(0, Ordering::Relaxed);
        self.other_warnings.store(0, Ordering::Relaxed);
        self.first_throttle_micros
            .store(u64::MAX, Ordering::Relaxed);
        lock(&self.samples).clear();
        lock(&self.by_path).clear();
    }
}

/// A poisoned tally is still a tally.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Pulls the formatted `message` field out of a `tracing` event.
#[derive(Default)]
struct MessageVisitor(Option<String>);

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

/// Counts `WARN` events whose target starts with [`CORE_TARGET`] into a
/// [`ThrottleObserver`].
#[derive(Debug, Clone)]
pub struct ThrottleLayer(pub Arc<ThrottleObserver>);

impl<S: tracing::Subscriber> Layer<S> for ThrottleLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        if !metadata.target().starts_with(CORE_TARGET) || *metadata.level() != tracing::Level::WARN
        {
            return;
        }

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        if let Some(message) = visitor.0 {
            self.0.record(&message);
        }
    }
}

/// Installs the counting layer as the process's global subscriber and returns
/// the observer it feeds. For a harness's `main`; a test scopes the layer
/// with `tracing::subscriber::with_default` instead.
///
/// No `fmt` layer is installed: the observer captures the warning text, so
/// harness output stays clean.
pub fn install_observer(start: Instant) -> Arc<ThrottleObserver> {
    let observer = Arc::new(ThrottleObserver::new(start));
    tracing_subscriber::registry()
        .with(ThrottleLayer(Arc::clone(&observer)))
        .init();
    observer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_recognises_the_retry_loop_429() {
        assert_eq!(
            classify(
                "Retriable status 429 Too Many Requests on /closed-positions, retry 1 after 0ms"
            ),
            WarnKind::Throttle
        );
    }

    #[test]
    fn classify_does_not_count_425_as_throttling() {
        assert_eq!(
            classify("Retriable status 425 Too Early on /closed-positions, retry 1 after 500ms"),
            WarnKind::Other
        );
    }

    #[test]
    fn observer_keeps_the_first_throttle_timestamp() {
        let observer = ThrottleObserver::new(Instant::now());
        observer.record("Retriable status 429 on /closed-positions, retry 1 after 500ms");
        let first = observer
            .first_throttle_at()
            .expect("first throttle recorded");
        observer.record("Retriable status 429 on /closed-positions, retry 2 after 1000ms");

        assert_eq!(observer.throttle_count(), 2);
        assert_eq!(
            observer.first_throttle_at(),
            Some(first),
            "a later throttle must not overwrite the first timestamp"
        );
    }

    #[test]
    fn observer_separates_other_warnings_from_throttles() {
        let observer = ThrottleObserver::new(Instant::now());
        observer.record("Retriable status 425 Too Early on /trades, retry 1 after 500ms");

        assert!(!observer.throttled(), "a 425 is not upstream rate limiting");
        assert_eq!(observer.other_warning_count(), 1);
        assert_eq!(observer.first_throttle_at(), None);
    }

    #[test]
    fn reset_clears_every_field_so_trials_stay_independent() {
        // A stale `first_throttle_micros` would make a clean trial report a
        // throttle time, and a stale count would fail every later trial.
        let observer = ThrottleObserver::new(Instant::now());
        observer.record("Retriable status 429 on /closed-positions, retry 1 after 500ms");
        observer.record("Retriable status 425 Too Early on /trades, retry 1 after 500ms");

        observer.reset();

        assert_eq!(observer.throttle_count(), 0);
        assert_eq!(observer.other_warning_count(), 0);
        assert_eq!(observer.first_throttle_at(), None);
        assert!(observer.warn_samples().is_empty());
        assert!(observer.throttles_by_path().is_empty());
    }

    #[test]
    fn the_throttled_path_is_read_off_the_retry_loop_message() {
        assert_eq!(
            throttled_path(
                "Retriable status 429 Too Many Requests on /v1/info/trades, retry 1 after 500ms"
            ),
            Some("/v1/info/trades")
        );
        assert_eq!(throttled_path("no path here"), None);
    }

    #[test]
    fn throttles_are_counted_per_path_and_other_warnings_are_not() {
        let observer = ThrottleObserver::new(Instant::now());
        observer.record("Retriable status 429 Too Many Requests on /v1/a, retry 1 after 5ms");
        observer.record("Retriable status 429 Too Many Requests on /v1/a, retry 2 after 9ms");
        observer.record("Retriable status 429 Too Many Requests on /v1/b, retry 1 after 5ms");
        observer.record("Retriable status 425 Too Early on /v1/c, retry 1 after 5ms");
        observer.record("Retriable status 429 with no path");
        assert_eq!(
            observer.throttles_by_path(),
            BTreeMap::from([
                ("/v1/a".to_owned(), 2),
                ("/v1/b".to_owned(), 1),
                ("?".to_owned(), 1)
            ])
        );
        assert_eq!(observer.throttle_count(), 4);
    }
}
