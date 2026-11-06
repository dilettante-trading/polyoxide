//! The reconnect delay schedule (W3).

use std::time::Duration;

/// The shortest delay. Zero would never grow, so a dead host would be retried
/// on every timer tick.
pub const MIN_BACKOFF: Duration = Duration::from_millis(1);

/// The reconnect delay schedule: doubling from an initial delay to a ceiling,
/// and back to the start after a connection that delivered something. Each
/// wait is jittered.
///
/// A plain schedule, with no clock and no socket, so the doubling and the
/// ceiling can be checked as arithmetic. As statements interleaved with a
/// `sleep` and a live socket they could only be checked by timing a real
/// reconnect, so neither was, and a schedule pinned at its initial value would
/// have hammered an unwell host without failing a test.
///
/// **Equal jitter.** The schedule's step doubles exactly, but each wait is
/// drawn uniformly from `[step / 2, step]`, so the clients a venue dropped at
/// one instant do not all come back at the same instants (0.5, 1, 2, 4 s, …).
/// Equal rather than full jitter, so a host that is down is never retried in a
/// near-zero loop. [`take_with`](Self::take_with) and
/// [`take_at_least_with`](Self::take_at_least_with) take the draw's position
/// as an argument, so a test pins the schedule exactly; [`take`](Self::take)
/// and [`take_at_least`](Self::take_at_least) draw it at random.
///
/// The basis is one socket crate's former copy (`src/supervisor.rs`),
/// identical to the copies three other socket crates carried. Each copy was
/// deleted for this one in Stories 4.6 to 4.8, and each crate that carried
/// one takes its schedule from here.
/// Two divergence rows stay with each venue:
///
/// - **D6.** What counts as a connection that delivered something follows the
///   venue's liveness rule (D5): any frame, a decoded event, or a ping. The
///   call site decides and passes it to
///   [`after_connection_ended`](Self::after_connection_ended) as a `bool`.
/// - **D13.** `polyoxide-core`'s `RetryConfig::backoff` is a different
///   schedule for HTTP retries, jittered around its step (75 % to 125 %)
///   where this one is jittered below it. The two are never unified.
///
/// ```
/// use std::time::Duration;
/// use polyoxide_ws::Backoff;
///
/// let mut backoff = Backoff::new(Duration::from_millis(10), Duration::from_millis(30));
/// // At the top of each draw, the schedule's steps.
/// assert_eq!(backoff.take_with(1.0), Duration::from_millis(10));
/// assert_eq!(backoff.take_with(1.0), Duration::from_millis(20));
/// // At the bottom, half the step: the ceiling's 30 ms waits 15 ms.
/// assert_eq!(backoff.take_with(0.0), Duration::from_millis(15));
/// backoff.after_connection_ended(true);
/// let wait = backoff.take();
/// assert!(wait >= Duration::from_millis(5) && wait <= Duration::from_millis(10));
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

    /// The step the next draw is taken from: the next wait is in
    /// `[step / 2, step]`, or at least a floor
    /// [`take_at_least`](Self::take_at_least) is given. The schedule as it
    /// stands, for a log line or a test; reading it changes nothing.
    pub fn step(&self) -> Duration {
        self.next
    }

    /// The delay to wait before the next attempt, drawn at random from
    /// `[step / 2, step]`, advancing the schedule. The step after it doubles,
    /// up to the ceiling, saturating rather than overflowing.
    pub fn take(&mut self) -> Duration {
        self.take_with(fastrand::f64())
    }

    /// As [`take`](Self::take), with the draw's position given: `unit` 0 waits
    /// half the step and 1 the whole step, and a value outside `[0, 1]` is
    /// clamped to it (a NaN is 1). The schedule advances as `take`'s does, so
    /// `take_with(1.0)` repeated is the exact doubling.
    pub fn take_with(&mut self, unit: f64) -> Duration {
        let delay = jittered(self.next, unit);
        self.next = self.next.saturating_mul(2).min(self.max);
        delay
    }

    /// As [`take`](Self::take), but at least `floor`, clamped to the
    /// ceiling: the wait a refused attempt asked for, such as its
    /// `Retry-After`, may only extend the delay, never shorten it, and never
    /// past the ceiling (DRIFT R11). The jittered draw is raised to the floor,
    /// so a floor is waited in full however low the draw. The schedule
    /// advances as `take`'s does.
    ///
    /// The rule is `polyoxide-core`'s for HTTP, where `Retry-After` only
    /// extends the retry delay; the two schedules stay apart (D13).
    pub fn take_at_least(&mut self, floor: Duration) -> Duration {
        self.take_at_least_with(floor, fastrand::f64())
    }

    /// As [`take_at_least`](Self::take_at_least), with the draw's position
    /// given as [`take_with`](Self::take_with) takes it.
    pub fn take_at_least_with(&mut self, floor: Duration, unit: f64) -> Duration {
        self.take_with(unit).max(floor.min(self.max))
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
            self.reset();
        }
    }

    /// Start the schedule again from its initial delay, as a connection that
    /// delivered something does. A supervisor calls it when a
    /// [`NetworkWake`](crate::NetworkWake) cuts a wait short: the network
    /// changed, so the failures before it say nothing about the next attempt.
    pub fn reset(&mut self) {
        self.next = self.initial;
    }
}

/// A wait drawn from `[step / 2, step]` at `unit`'s position in it, and never
/// under [`MIN_BACKOFF`].
///
/// The draw is `step / 2` plus `unit` of the other half. That half is scaled
/// in floating point, which cannot overflow, since it is at most half of
/// `Duration::MAX`; the sum is then held to `step`, so a rounding up at the
/// top of a huge step stays inside it, and `unit` 1 is the step exactly.
fn jittered(step: Duration, unit: f64) -> Duration {
    let unit = if unit.is_nan() {
        1.0
    } else {
        unit.clamp(0.0, 1.0)
    };
    let half = step / 2;
    let rest = step - half;
    let extra = if unit >= 1.0 {
        rest
    } else {
        rest.mul_f64(unit)
    };
    half.saturating_add(extra).min(step).max(MIN_BACKOFF)
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

        /// The schedule's steps: each draw at the top of its range.
        fn take_n(backoff: &mut Backoff, n: usize) -> Vec<Duration> {
            (0..n).map(|_| backoff.take_with(1.0)).collect()
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
                escalated.push(escalating.take_with(1.0));
                recovering.after_connection_ended(true);
                recovered.push(recovering.take_with(1.0));
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
        fn a_floor_extends_the_delay_and_never_shortens_it_drift_r11() {
            // A step of 1 s, a ceiling of 60 s: a 30 s wait is waited in
            // full, and a floor under the step leaves the step alone.
            let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(60));
            assert_eq!(
                backoff.take_at_least(Duration::from_secs(30)),
                Duration::from_secs(30)
            );
            assert_eq!(
                backoff.take_at_least_with(millis(500), 1.0),
                Duration::from_secs(2)
            );
            assert_eq!(
                backoff.take_at_least_with(Duration::ZERO, 1.0),
                Duration::from_secs(4)
            );
        }

        #[test]
        fn a_floor_past_the_ceiling_waits_the_ceiling_drift_r11() {
            let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(60));
            assert_eq!(
                backoff.take_at_least(Duration::from_secs(600)),
                Duration::from_secs(60)
            );
            assert_eq!(
                backoff.take_at_least(Duration::MAX),
                Duration::from_secs(60)
            );
        }

        #[test]
        fn a_floor_does_not_move_the_schedule() {
            let mut floored = Backoff::new(millis(10), millis(80));
            let mut plain = floored;
            floored.take_at_least(millis(70));
            plain.take();
            assert_eq!(take_n(&mut floored, 3), take_n(&mut plain, 3));
        }

        #[test]
        fn a_huge_delay_saturates_rather_than_panicking() {
            let mut backoff = Backoff::new(Duration::MAX, Duration::MAX);
            assert_eq!(take_n(&mut backoff, 2), vec![Duration::MAX, Duration::MAX]);
        }
    }

    /// The jitter: where in `[step / 2, step]` each wait falls (issue #93).
    mod jitter {
        use super::*;

        fn millis(ms: u64) -> Duration {
            Duration::from_millis(ms)
        }

        #[test]
        fn the_bottom_of_a_draw_is_half_the_step_and_the_top_the_step() {
            let mut low = Backoff::new(millis(100), millis(800));
            let mut high = low;
            let lows: Vec<_> = (0..5).map(|_| low.take_with(0.0)).collect();
            let highs: Vec<_> = (0..5).map(|_| high.take_with(1.0)).collect();
            assert_eq!(
                lows,
                vec![
                    millis(50),
                    millis(100),
                    millis(200),
                    millis(400),
                    millis(400)
                ]
            );
            assert_eq!(
                highs,
                vec![
                    millis(100),
                    millis(200),
                    millis(400),
                    millis(800),
                    millis(800)
                ]
            );
            assert_eq!(
                Backoff::new(millis(100), millis(800)).take_with(0.5),
                millis(75)
            );
        }

        #[test]
        fn a_position_outside_the_range_is_clamped() {
            let at = |unit| Backoff::new(millis(100), millis(800)).take_with(unit);
            assert_eq!(at(-3.0), millis(50));
            assert_eq!(at(7.0), millis(100));
            assert_eq!(at(f64::NAN), millis(100));
            assert_eq!(at(f64::INFINITY), millis(100));
            assert_eq!(at(f64::NEG_INFINITY), millis(50));
        }

        #[test]
        fn the_draw_does_not_move_the_schedule() {
            // A low draw must not slow the doubling, nor a high one speed it.
            let mut low = Backoff::new(millis(10), millis(80));
            let mut high = low;
            for _ in 0..3 {
                low.take_with(0.0);
                high.take_with(1.0);
            }
            assert_eq!(low, high);
        }

        #[test]
        fn every_random_draw_is_within_half_the_step_and_the_step() {
            let mut backoff = Backoff::new(millis(100), Duration::from_secs(60));
            let mut step = millis(100);
            for _ in 0..2000 {
                let wait = backoff.take();
                assert!(
                    wait >= step / 2 && wait <= step,
                    "{wait:?} is outside [{:?}, {step:?}]",
                    step / 2
                );
                step = (step * 2).min(Duration::from_secs(60));
                if step == Duration::from_secs(60) {
                    backoff.reset();
                    step = millis(100);
                }
            }
        }

        #[test]
        fn random_draws_spread_across_the_range() {
            // Not degenerate: both halves of the range are drawn from, and
            // the draws are not all one value.
            let draws: Vec<_> = (0..1000)
                .map(|_| Backoff::new(millis(1000), millis(1000)).take())
                .collect();
            let below = draws.iter().filter(|wait| **wait < millis(750)).count();
            assert!(
                (300..=700).contains(&below),
                "{below} of 1000 draws fell in the lower half"
            );
            assert!(draws.iter().any(|wait| *wait != draws[0]));
        }

        #[test]
        fn a_floor_raises_a_low_draw_and_stays_under_the_ceiling_drift_r11() {
            // A 1 s step drawn at its bottom would wait 500 ms; a 700 ms
            // floor is waited in full, and a floor under the draw leaves it.
            let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(4));
            assert_eq!(backoff.take_at_least_with(millis(700), 0.0), millis(700));
            assert_eq!(
                backoff.take_at_least_with(millis(700), 0.0),
                Duration::from_secs(1)
            );
            for _ in 0..100 {
                let wait = backoff.take_at_least(millis(3500));
                assert!(wait >= millis(3500) && wait <= Duration::from_secs(4));
            }
            assert_eq!(backoff.take_at_least(Duration::MAX), Duration::from_secs(4));
        }

        #[test]
        fn a_draw_never_falls_under_the_minimum() {
            // The 1 ms step's bottom is 0.5 ms, under `MIN_BACKOFF`.
            let mut backoff = Backoff::new(Duration::ZERO, millis(80));
            assert_eq!(backoff.take_with(0.0), MIN_BACKOFF);
        }

        #[test]
        fn a_huge_step_drawn_anywhere_stays_inside_it() {
            for unit in [0.0, 0.25, 0.999_999, 1.0] {
                let wait = Backoff::new(Duration::MAX, Duration::MAX).take_with(unit);
                assert!(wait >= Duration::MAX / 2, "{unit}: {wait:?}");
            }
        }

        #[test]
        fn the_step_is_the_schedule_and_reading_it_moves_nothing() {
            let mut backoff = Backoff::new(millis(10), millis(30));
            assert_eq!(backoff.step(), millis(10));
            assert_eq!(backoff.step(), millis(10));
            backoff.take();
            assert_eq!(backoff.step(), millis(20));
            backoff.take_at_least(Duration::MAX);
            assert_eq!(backoff.step(), millis(30));
            backoff.after_connection_ended(true);
            assert_eq!(backoff.step(), millis(10));
        }

        #[test]
        fn a_reset_starts_again_from_the_initial_step() {
            let mut backoff = Backoff::new(millis(10), millis(80));
            for _ in 0..3 {
                backoff.take_with(1.0);
            }
            backoff.reset();
            assert_eq!(backoff.take_with(1.0), millis(10));
            assert_eq!(backoff.take_with(1.0), millis(20));
        }
    }
}
