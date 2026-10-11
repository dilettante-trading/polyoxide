//! The reconnect delay schedule (W3).

use std::time::Duration;

/// The shortest delay. Zero would never grow, so a dead host would be retried
/// on every timer tick.
pub const MIN_BACKOFF: Duration = Duration::from_millis(1);

/// The reconnect delay schedule: doubling from an initial delay to a ceiling,
/// and back to the start after a connection that delivered something.
///
/// A plain schedule, with no clock and no socket, so the doubling and the
/// ceiling can be checked as arithmetic. As statements interleaved with a
/// `sleep` and a live socket they could only be checked by timing a real
/// reconnect, so neither was, and a schedule pinned at its initial value would
/// have hammered an unwell host without failing a test.
///
/// The basis is `polyoxide-rtds`' former copy (`src/supervisor.rs`),
/// identical to those `polyoxide-perps`, `polyoxide-binance` and
/// `polyoxide-sports` carried; each was deleted for this one (Stories 4.6 to
/// 4.8).
/// Two divergence rows stay with each venue:
///
/// - **D6.** What counts as a connection that delivered something follows the
///   venue's liveness rule (D5): any frame, a decoded event, or a ping. The
///   call site decides and passes it to
///   [`after_connection_ended`](Self::after_connection_ended) as a `bool`.
/// - **D13.** `polyoxide-core`'s `RetryConfig::backoff` is a different
///   schedule, jittered, for HTTP retries. The two are never unified.
///
/// ```
/// use std::time::Duration;
/// use polyoxide_ws::Backoff;
///
/// let mut backoff = Backoff::new(Duration::from_millis(10), Duration::from_millis(30));
/// assert_eq!(backoff.take(), Duration::from_millis(10));
/// assert_eq!(backoff.take(), Duration::from_millis(20));
/// assert_eq!(backoff.take(), Duration::from_millis(30));
/// backoff.after_connection_ended(true);
/// assert_eq!(backoff.take(), Duration::from_millis(10));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
    next: Duration,
}

impl Backoff {
    /// A schedule starting at `initial` and doubling towards `max`.
    ///
    /// Both are raised to [`MIN_BACKOFF`] first, and `initial` is then lowered
    /// to `max`: a builder's `backoff(initial, max)` takes the two
    /// independently, and nothing stops a caller passing them the wrong way
    /// round, which would otherwise wait the larger delay once before the
    /// ceiling took effect.
    pub fn new(initial: Duration, max: Duration) -> Self {
        let max = max.max(MIN_BACKOFF);
        let initial = initial.clamp(MIN_BACKOFF, max);
        Self {
            initial,
            max,
            next: initial,
        }
    }

    /// The delay to wait before the next attempt, advancing the schedule. The
    /// one after it doubles, up to the ceiling, saturating rather than
    /// overflowing.
    pub fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = self.next.saturating_mul(2).min(self.max);
        delay
    }

    /// Note that a connection ended, and whether it delivered anything.
    ///
    /// Resetting on `delivered` rather than on a successful connect is what
    /// stops a host that accepts and immediately closes from pinning the delay
    /// at its initial value. A host that reliably delivers exactly one frame
    /// before dying still resets the schedule each time; most unhealthy hosts
    /// fail before delivering anything.
    pub fn after_connection_ended(&mut self, delivered: bool) {
        if delivered {
            self.next = self.initial;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schedule as arithmetic, with no clock and no socket involved.
    mod backoff_schedule {
        use super::*;

        fn millis(ms: u64) -> Duration {
            Duration::from_millis(ms)
        }

        fn take_n(backoff: &mut Backoff, n: usize) -> Vec<Duration> {
            (0..n).map(|_| backoff.take()).collect()
        }

        #[test]
        fn the_delay_doubles_up_to_the_ceiling_and_then_holds() {
            let mut backoff = Backoff::new(millis(10), millis(80));
            assert_eq!(
                take_n(&mut backoff, 6),
                vec![
                    millis(10),
                    millis(20),
                    millis(40),
                    millis(80),
                    millis(80),
                    millis(80)
                ],
                "a schedule that stops escalating hammers a host that is \
                 already unwell"
            );
        }

        #[test]
        fn the_ceiling_is_a_clamp_rather_than_a_step() {
            // 30 doubles to 60, which is past 50 without ever equalling it. A
            // ceiling implemented as an equality check would run away here.
            let mut backoff = Backoff::new(millis(30), millis(50));
            assert_eq!(
                take_n(&mut backoff, 3),
                vec![millis(30), millis(50), millis(50)]
            );
        }

        #[test]
        fn a_reset_returns_to_the_initial_delay_not_to_zero() {
            let mut backoff = Backoff::new(millis(10), millis(80));
            take_n(&mut backoff, 3);
            backoff.after_connection_ended(true);
            assert_eq!(
                take_n(&mut backoff, 2),
                vec![millis(10), millis(20)],
                "a reset restarts the schedule; it does not remove the wait"
            );
        }

        #[test]
        fn only_a_connection_that_delivered_something_resets_the_schedule() {
            // The distinction the `delivered` flag exists for. A host that
            // accepts and immediately closes must keep escalating.
            let mut escalating = Backoff::new(millis(10), millis(80));
            let mut recovering = Backoff::new(millis(10), millis(80));
            let (mut escalated, mut recovered) = (Vec::new(), Vec::new());

            for _ in 0..4 {
                escalating.after_connection_ended(false);
                escalated.push(escalating.take());
                recovering.after_connection_ended(true);
                recovered.push(recovering.take());
            }

            assert_eq!(
                escalated,
                vec![millis(10), millis(20), millis(40), millis(80)]
            );
            assert_eq!(
                recovered,
                vec![millis(10), millis(10), millis(10), millis(10)]
            );
        }

        #[test]
        fn a_ceiling_below_the_initial_delay_is_still_honoured() {
            // `backoff(initial, max)` takes the two independently, so nothing
            // stops a caller swapping them. Without the clamp the first wait
            // would be a minute regardless of the ceiling.
            let mut backoff = Backoff::new(Duration::from_secs(60), millis(1));
            assert_eq!(take_n(&mut backoff, 2), vec![millis(1), millis(1)]);

            backoff.after_connection_ended(true);
            assert_eq!(backoff.take(), millis(1), "a reset must respect it too");
        }

        #[test]
        fn a_zero_initial_delay_still_grows() {
            // Zero doubles to zero, which would retry a dead host on every
            // timer tick for ever.
            let mut backoff = Backoff::new(Duration::ZERO, millis(80));
            assert_eq!(
                take_n(&mut backoff, 3),
                vec![millis(1), millis(2), millis(4)]
            );
        }

        #[test]
        fn a_huge_delay_saturates_rather_than_panicking() {
            let mut backoff = Backoff::new(Duration::MAX, Duration::MAX);
            assert_eq!(take_n(&mut backoff, 2), vec![Duration::MAX, Duration::MAX]);
        }
    }
}
