//! [`Hold`]: what the server said, shared by every layer of a throttle.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use polyoxide_rt::{Instant, Runtime};

/// Thirty years: where a deadline that would overflow the clock lands instead.
const FAR_FUTURE: Duration = Duration::from_secs(86_400 * 365 * 30);

/// A deadline before which no request on a throttle may proceed.
///
/// A throttle's buckets model the quota a venue *publishes*; a hold is what
/// the server actually just said. They disagree more often than the tables
/// suggest — Cloudflare's `error code: 1015` is an IP-scoped block with its
/// own window, and it answers 429 no matter how many tokens the buckets still
/// hold. So a 429 holds every request sharing the throttle, not just the one
/// that saw it.
///
/// A hold only ever extends: several concurrent requests typically see the
/// same 429 within a few milliseconds of each other, and taking the most
/// recent value would let whichever response carried the smallest delay
/// release all of them early. [`wait`](Self::wait) re-reads the deadline
/// after waking, so a hold extended mid-wait is honoured in full.
///
/// A hold keeps time on a [`Runtime`](polyoxide_rt::Runtime): it reads its
/// deadline off the runtime's clock and its waits sleep on the runtime.
/// [`unbounded`](Self::unbounded) and [`with_ceiling`](Self::with_ceiling)
/// take the target's default runtime, [`unbounded_on`](Self::unbounded_on) and
/// [`with_ceiling_on`](Self::with_ceiling_on) the one given. Every layer that
/// waits and shares the hold keeps time on the hold's runtime, so one throttle
/// never mixes two clocks.
///
/// A clone shares the deadline. Every layer of one throttle holds a clone of
/// one `Hold`, so replacing or resizing a layer keeps it.
#[derive(Clone)]
pub struct Hold {
    inner: Arc<HoldInner>,
}

struct HoldInner {
    until: Mutex<Option<Instant>>,
    ceiling: Option<Duration>,
    runtime: Arc<dyn Runtime>,
}

impl Hold {
    /// A hold with no ceiling, on the target's default runtime: every
    /// [`extend`](Self::extend) is taken as asked.
    pub fn unbounded() -> Self {
        Self::unbounded_on(polyoxide_rt::default_runtime())
    }

    /// As [`unbounded`](Self::unbounded), keeping time on `runtime`.
    pub fn unbounded_on(runtime: Arc<dyn Runtime>) -> Self {
        Self::new(None, runtime)
    }

    /// A hold that clamps every [`extend`](Self::extend) to `ceiling`, so a
    /// server asking for longer holds for `ceiling`, on the target's default
    /// runtime.
    pub fn with_ceiling(ceiling: Duration) -> Self {
        Self::with_ceiling_on(ceiling, polyoxide_rt::default_runtime())
    }

    /// As [`with_ceiling`](Self::with_ceiling), keeping time on `runtime`.
    pub fn with_ceiling_on(ceiling: Duration, runtime: Arc<dyn Runtime>) -> Self {
        Self::new(Some(ceiling), runtime)
    }

    fn new(ceiling: Option<Duration>, runtime: Arc<dyn Runtime>) -> Self {
        Self {
            inner: Arc::new(HoldInner {
                until: Mutex::new(None),
                ceiling,
                runtime,
            }),
        }
    }

    /// The runtime this hold keeps time on, which every layer sharing the
    /// hold keeps time on too.
    pub fn runtime(&self) -> &Arc<dyn Runtime> {
        &self.inner.runtime
    }

    /// Hold every request for `delay` from now, clamped to the ceiling.
    ///
    /// Extends a hold in force but never shortens one. A delay too large for
    /// the clock holds for thirty years rather than panicking.
    pub fn extend(&self, delay: Duration) {
        let delay = self
            .inner
            .ceiling
            .map_or(delay, |ceiling| delay.min(ceiling));
        let now = self.inner.runtime.now();
        let until = now.checked_add(delay).unwrap_or(now + FAR_FUTURE);
        let mut slot = self.lock();
        if slot.is_none_or(|current| until > current) {
            *slot = Some(until);
        }
    }

    /// Wait out the hold in force, if any.
    pub async fn wait(&self) {
        loop {
            // Read the deadline and release the guard before awaiting. Holding
            // a `std::sync::MutexGuard` across an await makes the future
            // `!Send`, which every throttle's `acquire` needs it to be.
            let deadline = *self.lock();
            let Some(deadline) = deadline else { return };
            if deadline <= self.inner.runtime.now() {
                return;
            }
            // Loop rather than return after sleeping: a sibling's 429 can push
            // the deadline out while we wait, and waking into a still-active
            // block is how the storm restarts.
            self.inner.runtime.sleep_until(deadline).await;
        }
    }

    /// Whether a hold is in force now.
    pub fn is_held(&self) -> bool {
        self.lock()
            .is_some_and(|until| until > self.inner.runtime.now())
    }

    /// A poison-tolerant lock on the deadline.
    ///
    /// A panic elsewhere must not turn the throttle into a permanent outage;
    /// the worst a torn write can cost here is one early or late wakeup.
    fn lock(&self) -> MutexGuard<'_, Option<Instant>> {
        self.inner
            .until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for Hold {
    /// [`Hold::unbounded`].
    fn default() -> Self {
        Self::unbounded()
    }
}

impl std::fmt::Debug for Hold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let remaining =
            (*self.lock()).and_then(|until| until.checked_duration_since(self.inner.runtime.now()));
        f.debug_struct("Hold")
            .field("remaining", &remaining)
            .field("ceiling", &self.inner.ceiling)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use tokio::time::Instant;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_hold_never_shortens() {
        let hold = Hold::unbounded();
        hold.extend(Duration::from_secs(10));
        hold.extend(Duration::from_secs(1));

        let t = Instant::now();
        hold.wait().await;
        assert!(
            t.elapsed() >= Duration::from_secs(10),
            "the longer hold was cut to {:?}",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_extended_mid_wait_is_honoured_in_full() {
        // One call to `wait`, so nothing after it can re-check: a throttle's
        // `acquire` waits again after its buckets, which would hide a `wait`
        // that slept only once.
        let hold = Hold::unbounded();
        hold.extend(Duration::from_secs(2));
        let extender = {
            let hold = hold.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                hold.extend(Duration::from_secs(5));
            })
        };

        let t = Instant::now();
        hold.wait().await;
        extender.await.unwrap();
        assert!(
            t.elapsed() >= Duration::from_secs(6),
            "woke at {:?}, at the deadline it first read",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_ceiling_clamps_a_longer_hold() {
        const DAY: Duration = Duration::from_secs(86_400);
        let hold = Hold::with_ceiling(3 * DAY);
        hold.extend(10 * DAY);

        let t = Instant::now();
        hold.wait().await;
        assert!(
            t.elapsed() >= 3 * DAY && t.elapsed() < 10 * DAY,
            "held {:?}, not the 3-day ceiling",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_huge_delay_saturates_rather_than_panics() {
        let hold = Hold::unbounded();
        hold.extend(Duration::MAX);
        assert!(
            tokio::time::timeout(Duration::from_secs(86_400 * 365), hold.wait())
                .await
                .is_err(),
            "a hold of Duration::MAX released within a year"
        );
    }
}
