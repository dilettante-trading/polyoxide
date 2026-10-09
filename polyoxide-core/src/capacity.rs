//! [`CapacityBucket`]: a token bucket that holds a published burst, charges
//! integer costs, and can be resized in place.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::Instant;

use crate::hold::Hold;
use crate::hooks::{LayerId, Refused};

/// A token bucket for a venue that publishes a capacity and a refill rate,
/// and charges each request a number of tokens.
///
/// It starts full, never holds more than `capacity`, and refills
/// `refill_per_sec` tokens a second. [`acquire`](Self::acquire) waits until
/// a request's tokens are there and takes them.
///
/// # Capacity is the published burst
///
/// This is the opposite of a window quota, and the two must not be made
/// alike. A venue that publishes a bucket's capacity and rate implements a
/// bucket, so holding the published capacity is a faithful model of it. A
/// window quota publishes a count per window with no capacity term, and a
/// bucket holding that count admits it twice in one window; see
/// [`WindowQuotaTable`](crate::WindowQuotaTable).
///
/// # A cost it can never hold
///
/// A request costing more than `capacity` can never be satisfied, however
/// long it waits. An exact cost is refused at once with [`Refused`], and
/// nothing is sent. An inexact cost (a floor, such as Polymarket's
/// `cancel-all`) is charged at capacity and never refused: the server
/// decides.
///
/// A bucket made [`provisional`](Self::provisional) is sized from a guess,
/// before the venue has said what the account's capacity is. It never
/// refuses: a cost above its capacity waits until [`confirm`](Self::confirm)
/// or [`resize`](Self::resize), and is then charged or refused.
///
/// # The hold
///
/// [`acquire`](Self::acquire) waits out the bucket's [`Hold`] before
/// charging, and again after any wait of its own, so a hold set while it
/// waited for tokens is honoured. Every layer of one throttle shares one
/// `Hold`, and [`resize`](Self::resize) keeps it.
///
/// A clone shares the tokens and the hold.
#[derive(Clone)]
pub struct CapacityBucket {
    inner: Arc<Inner>,
}

struct Inner {
    layer: LayerId,
    hold: Hold,
    state: Mutex<State>,
    /// Woken by `confirm` and `resize`, for costs waiting on a provisional
    /// size.
    sized: Notify,
}

struct State {
    capacity: u32,
    refill_per_sec: u32,
    tokens: f64,
    updated: Instant,
    provisional: bool,
}

impl State {
    /// Add what the refill has brought since the last update, up to capacity.
    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.updated).as_secs_f64();
        self.tokens =
            (self.tokens + elapsed * f64::from(self.refill_per_sec)).min(f64::from(self.capacity));
        self.updated = now;
    }
}

/// What one look at the bucket decided.
enum Step {
    Taken,
    Refused(Refused),
    WaitFor(Duration),
    WaitForSize,
}

impl CapacityBucket {
    /// A full bucket for `layer` holding at most `capacity` tokens and
    /// refilling `refill_per_sec` a second, waiting out `hold`.
    ///
    /// # Panics
    ///
    /// When `refill_per_sec` is zero, since an emptied bucket would never
    /// refill.
    pub fn new(layer: LayerId, capacity: u32, refill_per_sec: u32, hold: Hold) -> Self {
        Self::build(layer, capacity, refill_per_sec, hold, false)
    }

    /// As [`new`](Self::new), but sized provisionally: a cost above
    /// `capacity` waits for [`confirm`](Self::confirm) or
    /// [`resize`](Self::resize) instead of being refused.
    ///
    /// # Panics
    ///
    /// When `refill_per_sec` is zero.
    pub fn provisional(layer: LayerId, capacity: u32, refill_per_sec: u32, hold: Hold) -> Self {
        Self::build(layer, capacity, refill_per_sec, hold, true)
    }

    fn build(
        layer: LayerId,
        capacity: u32,
        refill_per_sec: u32,
        hold: Hold,
        provisional: bool,
    ) -> Self {
        assert!(refill_per_sec > 0, "a bucket that never refills");
        Self {
            inner: Arc::new(Inner {
                layer,
                hold,
                state: Mutex::new(State {
                    capacity,
                    refill_per_sec,
                    tokens: f64::from(capacity),
                    updated: Instant::now(),
                    provisional,
                }),
                sized: Notify::new(),
            }),
        }
    }

    /// The layer this bucket charges.
    pub fn layer(&self) -> LayerId {
        self.inner.layer
    }

    /// The most tokens the bucket holds.
    pub fn capacity(&self) -> u32 {
        self.lock().capacity
    }

    /// The hold this bucket waits out.
    pub fn hold(&self) -> &Hold {
        &self.inner.hold
    }

    /// Wait until `units` tokens are there, then take them.
    ///
    /// Waits out the hold first, and again after its own wait. An inexact
    /// cost above capacity is charged at capacity.
    ///
    /// # Errors
    ///
    /// [`Refused`] at once when `exact` and `units` exceed the capacity of a
    /// bucket that is not provisional. Nothing is taken.
    pub async fn acquire(&self, units: u32, exact: bool) -> Result<(), Refused> {
        self.inner.hold.wait().await;
        loop {
            // Made before the look, so a `confirm` between the look and the
            // wait still wakes it.
            let sized = self.inner.sized.notified();
            match self.step(units, exact) {
                Step::Taken => break,
                Step::Refused(refused) => return Err(refused),
                Step::WaitFor(wait) => tokio::time::sleep(wait).await,
                Step::WaitForSize => sized.await,
            }
        }
        // AD-23: a hold set while this call waited for tokens is honoured.
        self.inner.hold.wait().await;
        Ok(())
    }

    /// One look at the bucket: take the tokens, refuse, or say how long to
    /// wait.
    fn step(&self, units: u32, exact: bool) -> Step {
        let mut state = self.lock();
        state.refill(Instant::now());
        let units = if units > state.capacity {
            if state.provisional {
                return Step::WaitForSize;
            }
            if exact {
                return Step::Refused(Refused {
                    layer: self.inner.layer,
                    units,
                    capacity: state.capacity,
                });
            }
            state.capacity
        } else {
            units
        };
        let missing = f64::from(units) - state.tokens;
        // A millionth of a token is rounding in the refill, not a wait: the
        // timer could not sleep that short anyway.
        if missing <= 1e-6 {
            state.tokens = (state.tokens - f64::from(units)).max(0.0);
            Step::Taken
        } else {
            Step::WaitFor(Duration::from_secs_f64(
                missing / f64::from(state.refill_per_sec),
            ))
        }
    }

    /// Resize the bucket in place. The tokens it holds are kept, down to the
    /// new capacity; the hold is kept; the sizing is confirmed.
    ///
    /// # Panics
    ///
    /// When `refill_per_sec` is zero.
    pub fn resize(&self, capacity: u32, refill_per_sec: u32) {
        assert!(refill_per_sec > 0, "a bucket that never refills");
        {
            let mut state = self.lock();
            state.refill(Instant::now());
            state.capacity = capacity;
            state.refill_per_sec = refill_per_sec;
            state.tokens = state.tokens.min(f64::from(capacity));
            state.provisional = false;
        }
        self.inner.sized.notify_waiters();
    }

    /// Confirm a provisional size: a cost above capacity is refused from now
    /// on, and those waiting are charged or refused.
    pub fn confirm(&self) {
        self.lock().provisional = false;
        self.inner.sized.notify_waiters();
    }

    /// A poison-tolerant lock: a panic elsewhere must not make the bucket an
    /// outage.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// `acquire` is `Send`, as every throttle's `acquire` has to be.
const _: fn(&CapacityBucket) = |bucket| {
    fn send<T: Send>(_: T) {}
    send(bucket.acquire(1, true));
};

impl std::fmt::Debug for CapacityBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.lock();
        f.debug_struct("CapacityBucket")
            .field("layer", &self.inner.layer)
            .field("capacity", &state.capacity)
            .field("refill_per_sec", &state.refill_per_sec)
            .field("provisional", &state.provisional)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYER: LayerId = LayerId("test");

    fn bucket(capacity: u32, refill_per_sec: u32) -> CapacityBucket {
        CapacityBucket::new(LAYER, capacity, refill_per_sec, Hold::unbounded())
    }

    /// How long `bucket.acquire(units, true)` took, on the paused clock.
    async fn timed(bucket: &CapacityBucket, units: u32) -> Duration {
        let t = Instant::now();
        bucket.acquire(units, true).await.unwrap();
        t.elapsed()
    }

    #[tokio::test(start_paused = true)]
    async fn it_starts_full() {
        let bucket = bucket(10, 1);
        assert_eq!(timed(&bucket, 10).await, Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn it_never_holds_more_than_its_capacity() {
        let bucket = bucket(5, 10);
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(timed(&bucket, 5).await, Duration::ZERO);
        assert!(
            timed(&bucket, 1).await >= Duration::from_millis(90),
            "a minute idle filled the bucket past its capacity of 5"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn it_paces_the_refill() {
        let bucket = bucket(10, 10);
        timed(&bucket, 10).await;
        let waited = timed(&bucket, 5).await;
        assert!(
            (Duration::from_millis(500)..Duration::from_millis(510)).contains(&waited),
            "5 tokens at 10 a second took {waited:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_exact_cost_above_capacity_is_refused_at_once() {
        let bucket = bucket(10, 1);
        let t = Instant::now();
        assert_eq!(
            bucket.acquire(11, true).await,
            Err(Refused {
                layer: LAYER,
                units: 11,
                capacity: 10
            })
        );
        assert_eq!(t.elapsed(), Duration::ZERO);
        assert_eq!(
            timed(&bucket, 10).await,
            Duration::ZERO,
            "a refusal took tokens"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_inexact_cost_above_capacity_is_charged_at_capacity() {
        let bucket = bucket(10, 10);
        let t = Instant::now();
        bucket.acquire(50, false).await.unwrap();
        assert_eq!(t.elapsed(), Duration::ZERO);
        let waited = timed(&bucket, 1).await;
        assert!(
            (Duration::from_millis(100)..Duration::from_millis(110)).contains(&waited),
            "the inexact cost did not drain exactly the capacity: {waited:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_provisional_bucket_waits_then_proceeds_after_a_resize() {
        let bucket = CapacityBucket::provisional(LAYER, 10, 10, Hold::unbounded());
        let waiting = tokio::spawn({
            let bucket = bucket.clone();
            async move { bucket.acquire(20, true).await }
        });
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(
            !waiting.is_finished(),
            "a provisional bucket refused or admitted 20 of 10"
        );

        bucket.resize(30, 10);
        assert_eq!(waiting.await.unwrap(), Ok(()));
    }

    #[tokio::test(start_paused = true)]
    async fn a_provisional_bucket_waits_then_refuses_after_confirm() {
        let bucket = CapacityBucket::provisional(LAYER, 10, 10, Hold::unbounded());
        let waiting = tokio::spawn({
            let bucket = bucket.clone();
            async move { bucket.acquire(20, true).await }
        });
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(!waiting.is_finished());

        bucket.confirm();
        assert_eq!(
            waiting.await.unwrap(),
            Err(Refused {
                layer: LAYER,
                units: 20,
                capacity: 10
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn resize_keeps_the_tokens_clamped_and_the_hold() {
        // Growing keeps the 6 tokens left, rather than filling to 100.
        let grown = bucket(10, 1);
        timed(&grown, 4).await;
        grown.resize(100, 1);
        assert_eq!(grown.capacity(), 100);
        assert_eq!(timed(&grown, 6).await, Duration::ZERO);
        assert!(timed(&grown, 1).await >= Duration::from_millis(990));

        // Shrinking clamps them to the new capacity.
        let shrunk = bucket(10, 1);
        shrunk.resize(3, 1);
        assert_eq!(timed(&shrunk, 3).await, Duration::ZERO);
        assert!(timed(&shrunk, 1).await >= Duration::from_millis(990));

        // The hold set before the resize still holds.
        let held = bucket(10, 1);
        held.hold().extend(Duration::from_secs(5));
        held.resize(20, 2);
        assert!(timed(&held, 1).await >= Duration::from_secs(5));
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_acquirers_never_over_admit() {
        // 5 tokens to start and 10 a second: the k-th of 25 single-token
        // requests cannot go before (k - 5) / 10 seconds.
        let bucket = bucket(5, 10);
        let start = Instant::now();
        let tasks: Vec<_> = (0..25)
            .map(|_| {
                let bucket = bucket.clone();
                tokio::spawn(async move {
                    bucket.acquire(1, true).await.unwrap();
                    start.elapsed()
                })
            })
            .collect();
        let mut done = Vec::new();
        for task in tasks {
            done.push(task.await.unwrap());
        }
        done.sort();
        for (k, at) in done.iter().enumerate() {
            let earliest = Duration::from_millis(100) * (k as u32 + 1).saturating_sub(5);
            assert!(
                *at + Duration::from_micros(1) >= earliest,
                "request {} went at {at:?}, before the {earliest:?} the bucket allows",
                k + 1
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_set_during_a_wait_for_tokens_is_honoured() {
        let bucket = bucket(1, 1);
        timed(&bucket, 1).await;
        let holder = {
            let hold = bucket.hold().clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                hold.extend(Duration::from_secs(5));
            })
        };
        let waited = timed(&bucket, 1).await;
        holder.await.unwrap();
        assert!(
            waited >= Duration::from_millis(5_500),
            "the token came at 1s and the request went at {waited:?}, inside the hold"
        );
    }
}
