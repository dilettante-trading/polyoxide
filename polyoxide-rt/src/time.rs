//! Timeouts and intervals, on an injected `Runtime`.

use std::future::{poll_fn, Future};
use std::pin::pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use crate::{Instant, Runtime};

/// A tick this close to its deadline is on time, so scheduling jitter does not
/// shift the grid of `MissedTick::Delay`; tokio's interval makes the same
/// allowance.
const TOLERANCE: Duration = Duration::from_millis(5);

/// What `instant` plus an unrepresentably long wait stands for: thirty years.
const FAR: Duration = Duration::from_secs(86_400 * 365 * 30);

/// Await `future` for at most `duration` on `runtime`'s clock: its output, or
/// `None` when `duration` passed first.
///
/// The duration counts from the first poll, not from the call. `future` is
/// polled before the timer, so a ready future wins even at `Duration::ZERO`. Any
/// duration, `Duration::MAX` included, is valid.
pub async fn timeout<F: Future>(
    runtime: &dyn Runtime,
    duration: Duration,
    future: F,
) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut deadline = runtime.sleep(duration);
    poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        deadline.as_mut().poll(cx).map(|()| None)
    })
    .await
}

/// What an [`Interval`] does about ticks it was too late to take on time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum MissedTick {
    /// Fire the missed ticks one after another at once, then go back to the
    /// original grid.
    #[default]
    Burst,
    /// Start the next period when the late tick fires, so the grid moves.
    Delay,
    /// Drop the missed ticks and fire the next one on the original grid.
    Skip,
}

/// Ticks every `period`, the first at once, on `runtime`'s clock.
///
/// # Panics
///
/// Panics with `interval period must be non-zero` when `period` is zero.
pub fn interval(runtime: Arc<dyn Runtime>, period: Duration) -> Interval {
    assert!(!period.is_zero(), "interval period must be non-zero");
    let next = runtime.now();
    Interval {
        runtime,
        period,
        next,
        behaviour: MissedTick::default(),
    }
}

/// A stream of ticks a period apart; built by [`interval`].
///
/// It is built on the runtime's `sleep_until` and `now`, so a runtime's paused
/// clock drives it.
#[derive(Debug)]
pub struct Interval {
    runtime: Arc<dyn Runtime>,
    period: Duration,
    next: Instant,
    behaviour: MissedTick,
}

impl Interval {
    /// Wait for the next tick and return the deadline it was due at. Dropping
    /// the future before it completes loses nothing: the next call waits for
    /// the same tick.
    pub async fn tick(&mut self) -> Instant {
        let due = self.next;
        self.runtime.sleep_until(due).await;
        self.next = self.after(due, self.runtime.now());
        due
    }

    /// Start a new period now: the next tick is one period from this call.
    pub fn reset(&mut self) {
        self.next = later(self.runtime.now(), self.period);
    }

    /// The time between ticks.
    pub fn period(&self) -> Duration {
        self.period
    }

    /// Choose what to do about ticks that were missed.
    pub fn set_missed_tick(&mut self, behaviour: MissedTick) {
        self.behaviour = behaviour;
    }

    /// The deadline after the tick due at `due`, taken at `now`.
    fn after(&self, due: Instant, now: Instant) -> Instant {
        let next = later(due, self.period);
        if now.saturating_duration_since(due) <= TOLERANCE {
            return next;
        }
        match self.behaviour {
            MissedTick::Burst => next,
            MissedTick::Delay => later(now, self.period),
            MissedTick::Skip => {
                if next > now {
                    return next;
                }
                let behind = now.saturating_duration_since(due).as_nanos();
                let periods = behind / self.period.as_nanos();
                u32::try_from(periods + 1)
                    .ok()
                    .and_then(|whole| self.period.checked_mul(whole))
                    .and_then(|grid| due.checked_add(grid))
                    .unwrap_or_else(|| later(now, self.period))
            }
        }
    }
}

/// `instant` plus `wait`, capped at thirty years where the sum would overflow.
fn later(instant: Instant, wait: Duration) -> Instant {
    instant.checked_add(wait).unwrap_or_else(|| instant + FAR)
}
