//! The browser's runtime, and the sleep arithmetic its tests pin on the host.

#[cfg(any(target_arch = "wasm32", test))]
mod chunks {
    use std::time::Duration;

    /// The next `setTimeout` for a sleep with `remaining` left, in
    /// milliseconds: the rest rounded up to a whole millisecond, at most
    /// `i32::MAX`, since the browser fires a timer with a larger delay at once.
    pub(super) fn step_ms(remaining: Duration) -> u32 {
        let millis = remaining.as_nanos().div_ceil(1_000_000);
        millis.min(i32::MAX as u128) as u32
    }

    /// The timers that, run one after another, sleep `duration`: none for no
    /// time, one per `i32::MAX` milliseconds, the last rounded up.
    pub(super) fn steps(duration: Duration) -> impl Iterator<Item = u32> {
        let mut remaining = duration;
        std::iter::from_fn(move || {
            if remaining.is_zero() {
                return None;
            }
            let step = step_ms(remaining);
            remaining = remaining.saturating_sub(Duration::from_millis(u64::from(step)));
            Some(step)
        })
    }

    #[cfg(test)]
    mod tests {
        use std::time::Duration;

        use super::{step_ms, steps};

        /// The longest delay `setTimeout` takes: a larger one fires at once.
        const LONGEST: u32 = i32::MAX as u32;

        #[test]
        fn a_step_is_the_remainder_rounded_up_to_a_millisecond() {
            assert_eq!(step_ms(Duration::ZERO), 0);
            assert_eq!(step_ms(Duration::from_nanos(1)), 1);
            assert_eq!(step_ms(Duration::from_millis(1)), 1);
            assert_eq!(step_ms(Duration::from_nanos(1_000_001)), 2);
            assert_eq!(step_ms(Duration::from_secs(90)), 90_000);
        }

        #[test]
        fn a_step_never_exceeds_what_set_timeout_accepts() {
            assert_eq!(step_ms(Duration::from_millis(u64::from(LONGEST))), LONGEST);
            assert_eq!(
                step_ms(Duration::from_millis(u64::from(LONGEST) + 1)),
                LONGEST
            );
            assert_eq!(step_ms(Duration::MAX), LONGEST);
        }

        #[test]
        fn a_long_sleep_is_chained_in_whole_steps() {
            let total = Duration::from_millis(2 * u64::from(LONGEST) + 5);
            assert_eq!(steps(total).collect::<Vec<_>>(), [LONGEST, LONGEST, 5]);
        }

        #[test]
        fn the_last_step_is_rounded_up_so_a_sleep_is_never_short() {
            let total = Duration::from_nanos(u64::from(LONGEST) * 1_000_000 + 1);
            assert_eq!(steps(total).collect::<Vec<_>>(), [LONGEST, 1]);
        }

        #[test]
        fn no_time_is_no_step_and_a_sub_millisecond_sleep_is_one() {
            assert_eq!(steps(Duration::ZERO).count(), 0);
            assert_eq!(steps(Duration::from_micros(10)).collect::<Vec<_>>(), [1]);
        }

        #[test]
        fn the_longest_sleep_starts_with_full_steps() {
            let first: Vec<u32> = steps(Duration::MAX).take(2).collect();
            assert_eq!(first, [LONGEST, LONGEST]);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::time::Duration;

    use gloo_timers::future::TimeoutFuture;
    use send_wrapper::SendWrapper;

    use super::chunks::steps;
    use crate::{BoxFuture, Instant, Runtime};

    /// The runtime of a browser page or Worker: `spawn_local` for tasks,
    /// `setTimeout` for sleeps and `performance.now` for the clock.
    ///
    /// A timer's JS handle is `!Send`, so each sleep sits in a
    /// `SendWrapper`: the `Send` bound of [`Runtime`] holds, and the handle is
    /// only ever touched on the thread that made it. That is the one thread a
    /// browser build has; a build with atomics (threads) is unsupported, since
    /// the wrapper panics off its thread.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct BrowserRuntime;

    impl Runtime for BrowserRuntime {
        fn spawn(&self, task: BoxFuture<()>) {
            wasm_bindgen_futures::spawn_local(task);
        }

        fn sleep(&self, duration: Duration) -> BoxFuture<()> {
            Box::pin(SendWrapper::new(async move {
                // `setTimeout` takes an `i32`, so a long sleep is a chain.
                for step in steps(duration) {
                    TimeoutFuture::new(step).await;
                }
            }))
        }

        fn now(&self) -> Instant {
            Instant::now()
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::BrowserRuntime;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod native_default {
    #[test]
    fn the_host_default_stays_tokio() {
        assert_eq!(format!("{:?}", crate::default_runtime()), "TokioRuntime");
    }
}
