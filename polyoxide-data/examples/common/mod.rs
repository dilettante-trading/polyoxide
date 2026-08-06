//! Shared throttle detection for the live rate-limit harnesses.
//!
//! Included by both `closed_positions_soak.rs` and
//! `closed_positions_burst_probe.rs` via `#[path]`. Cargo only auto-discovers
//! `examples/*.rs` and `examples/*/main.rs`, so this file is not itself built
//! as an example.
//!
//! # Why detection needs a tracing subscriber
//!
//! A 429 the client retries away is invisible to the caller: the retry loops
//! in `polyoxide-core/src/{client,request}.rs` log a `WARN` and then return
//! `Ok`. A harness that counts successes alone would report a clean run
//! through an hour of throttling. Everything here exists so that the failure
//! these harnesses look for can actually be seen.

#![allow(dead_code)] // Each example uses a different subset.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use tracing::field::{Field, Visit};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

/// The repo's conventional probe address (see `tests/live_api.rs`). Holds no
/// closed positions, so responses are an empty array; pass a real trader's
/// address to exercise realistic payload sizes.
pub const DEFAULT_USER: &str = "0x0000000000000000000000000000000000000001";

/// Matches `DataApiBuilder`'s own default, so runs measure the shipped
/// configuration rather than a bespoke one.
pub const DEFAULT_CONCURRENCY: usize = 4;

/// Server-enforced ceiling for `/closed-positions` — above 50 it 400s.
pub const MAX_PAGE_LIMIT: u32 = 50;

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

pub fn classify(message: &str) -> WarnKind {
    if message.contains("Retriable status 429") {
        WarnKind::Throttle
    } else {
        WarnKind::Other
    }
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
}

impl ThrottleObserver {
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            throttles: AtomicU64::new(0),
            other_warnings: AtomicU64::new(0),
            first_throttle_micros: AtomicU64::new(u64::MAX),
            samples: Mutex::new(Vec::new()),
        }
    }

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
            }
            WarnKind::Other => {
                self.other_warnings.fetch_add(1, Ordering::Relaxed);
            }
        }

        if let Ok(mut samples) = self.samples.lock() {
            if samples.len() < MAX_WARN_SAMPLES {
                samples.push(message.to_owned());
            }
        }
    }

    pub fn throttled(&self) -> bool {
        self.throttle_count() > 0
    }

    pub fn throttle_count(&self) -> u64 {
        self.throttles.load(Ordering::Relaxed)
    }

    pub fn other_warning_count(&self) -> u64 {
        self.other_warnings.load(Ordering::Relaxed)
    }

    pub fn first_throttle_at(&self) -> Option<Duration> {
        match self.first_throttle_micros.load(Ordering::Relaxed) {
            u64::MAX => None,
            micros => Some(Duration::from_micros(micros)),
        }
    }

    pub fn warn_samples(&self) -> Vec<String> {
        match self.samples.lock() {
            Ok(samples) => samples.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Clears the tally so one process can run several independent trials.
    ///
    /// The burst probe depends on this: each trial needs a verdict about its
    /// own requests, not a running total across the ramp.
    pub fn reset(&self) {
        self.throttles.store(0, Ordering::Relaxed);
        self.other_warnings.store(0, Ordering::Relaxed);
        self.first_throttle_micros
            .store(u64::MAX, Ordering::Relaxed);
        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }
    }
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

/// Counts `WARN` events from `polyoxide-core`.
///
/// The retry loops in `client.rs` and `request.rs` are the only `warn!` call
/// sites in that crate, so target + level identifies them without matching on
/// message text. The text is used afterwards only to tell a 429 from a 425.
struct ThrottleLayer(Arc<ThrottleObserver>);

impl<S: tracing::Subscriber> Layer<S> for ThrottleLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        if !metadata.target().starts_with("polyoxide_core")
            || *metadata.level() != tracing::Level::WARN
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

/// Installs the counting subscriber and returns the observer it feeds.
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

/// Nearest-rank percentile over an ascending slice.
pub fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let last = sorted.len() - 1;
    let rank = (p / 100.0 * last as f64).round() as usize;
    sorted[rank.min(last)]
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
    }

    #[test]
    fn percentile_of_empty_is_zero() {
        assert_eq!(percentile(&[], 50.0), Duration::ZERO);
    }

    #[test]
    fn percentile_picks_by_nearest_rank() {
        let sorted: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        assert_eq!(percentile(&sorted, 50.0), Duration::from_millis(51));
        assert_eq!(percentile(&sorted, 99.0), Duration::from_millis(99));
        assert_eq!(percentile(&sorted, 100.0), Duration::from_millis(100));
    }
}
