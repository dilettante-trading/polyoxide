//! The supervision timings, their builder setters (W4), and the wait until
//! the next deadline (W5).

use std::time::Duration;

use crate::{Backoff, DEFAULT_CONNECT_TIMEOUT};

/// The timings a supervised connection runs on.
///
/// Every field is public, so a venue reads them directly and sets them through
/// [`timing_setters!`](crate::timing_setters). The defaults are the kit's: a
/// ping every 20 s, stale after 30 s of silence, reconnect backoff from 500 ms
/// doubling to 60 s, a 10 s connect timeout, and no rotation. Each venue sets
/// its own: rtds pings after a 5 s quiet tick, sports sends no pings and goes
/// stale after 45 s, and Binance rotates at 23 h 50 min.
///
/// The basis is the setters of `polyoxide-rtds`' `RtdsBuilder`,
/// `polyoxide-perps`' `PerpsWsBuilder`, `polyoxide-binance`'s `UsdmWsBuilder`
/// and `polyoxide-sports`' `SportsWsBuilder`, which store the same five values.
/// What the values mean stays with each venue:
///
/// - **D4, the ping schedule.** rtds pings only after a quiet tick, perps and
///   Binance on the wall clock whatever the traffic, and sports never
///   (`ping_interval: None`); clob pings every 10 s. Each venue sets its own
///   default, and never copies the rtds shape to a host that requires pings.
/// - **D5, liveness.** What restarts the [`stale_after`](Self::stale_after)
///   window: any frame (Binance), a decoded event (rtds), any text frame, a
///   pong included (perps), or pings included (sports).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Timing {
    /// How often to ping, or `None` for never.
    pub ping_interval: Option<Duration>,
    /// Silence after which a connection is presumed dead.
    pub stale_after: Duration,
    /// The first reconnect delay.
    pub initial_backoff: Duration,
    /// The reconnect delay's ceiling.
    pub max_backoff: Duration,
    /// How long one connection attempt may take.
    pub connect_timeout: Duration,
    /// How long a connection may live before it is replaced, or `None` for
    /// as long as it lasts.
    pub max_connection_age: Option<Duration>,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            ping_interval: Some(Duration::from_secs(20)),
            stale_after: Duration::from_secs(30),
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(60),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            max_connection_age: None,
        }
    }
}

impl Timing {
    /// A fresh reconnect schedule from [`initial_backoff`](Self::initial_backoff)
    /// to [`max_backoff`](Self::max_backoff).
    pub fn backoff(&self) -> Backoff {
        Backoff::new(self.initial_backoff, self.max_backoff)
    }

    /// The wait until the first of this connection's deadlines: the next ping,
    /// the staleness limit and the rotation, given the time since the last
    /// ping, the silence so far and the connection's age. See [`next_wait`].
    pub fn next_wait(&self, since_ping: Duration, silent: Duration, age: Duration) -> Duration {
        next_wait([
            (self.ping_interval, since_ping),
            (Some(self.stale_after), silent),
            (self.max_connection_age, age),
        ])
    }
}

/// The wait until the first of several deadlines falls due.
///
/// Each deadline is a limit and the time elapsed against it; a limit of `None`
/// never falls due. The arithmetic saturates: a deadline already past waits
/// zero, a limit of `Duration::MAX` waits about that long, and no deadline at
/// all, or none that is set, gives `Duration::MAX`, which `tokio::time::timeout`
/// treats as never. It is a wait, not an `Instant`, because adding a limit
/// such as `Duration::MAX` to an `Instant` overflows.
///
/// The basis is `polyoxide-binance`'s pump (`usdm/ws/supervised.rs`), the
/// minimum of its ping, staleness and rotation waits; `polyoxide-perps`'
/// former pump (deleted in Story 4.6) took the first two. Which deadlines a venue has follows D4: a venue that sends
/// no pings passes no ping deadline.
///
/// ```
/// use std::time::Duration;
/// use polyoxide_ws::next_wait;
///
/// let secs = Duration::from_secs;
/// // A ping due in 15 s, staleness in 20 s, and no rotation.
/// assert_eq!(next_wait([(Some(secs(20)), secs(5)), (Some(secs(30)), secs(10)), (None, secs(99))]), secs(15));
/// assert_eq!(next_wait([(None, secs(1))]), Duration::MAX);
/// ```
pub fn next_wait(deadlines: impl IntoIterator<Item = (Option<Duration>, Duration)>) -> Duration {
    deadlines
        .into_iter()
        .filter_map(|(limit, elapsed)| limit.map(|limit| limit.saturating_sub(elapsed)))
        .min()
        .unwrap_or(Duration::MAX)
}

/// Generates the timing setters a supervised builder offers, each writing the
/// builder's `timing` field, a [`Timing`].
///
/// List the setters the builder wants, each with its own doc comment, since
/// what a ping or the staleness window means differs between venues (D4, D5).
/// The five are `ping_interval(Duration)`, `stale_after(Duration)`,
/// `backoff(initial, max)`, `connect_timeout(Duration)` and
/// `max_connection_age(Duration)`; `ping_interval` and `max_connection_age`
/// store `Some` of the value.
///
/// ```
/// use std::time::Duration;
/// use polyoxide_ws::Timing;
///
/// #[derive(Default)]
/// pub struct FeedBuilder {
///     timing: Timing,
/// }
///
/// impl FeedBuilder {
///     polyoxide_ws::timing_setters! {
///         /// How long the feed may be silent, pings included.
///         stale_after;
///         /// Reconnect delay bounds.
///         backoff;
///     }
/// }
///
/// let builder = FeedBuilder::default().stale_after(Duration::from_secs(45));
/// assert_eq!(builder.timing.stale_after, Duration::from_secs(45));
/// ```
#[macro_export]
macro_rules! timing_setters {
    () => {};
    ($(#[$meta:meta])* ping_interval; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn ping_interval(mut self, interval: ::core::time::Duration) -> Self {
            self.timing.ping_interval = ::core::option::Option::Some(interval);
            self
        }
        $crate::timing_setters! { $($rest)* }
    };
    ($(#[$meta:meta])* stale_after; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn stale_after(mut self, stale_after: ::core::time::Duration) -> Self {
            self.timing.stale_after = stale_after;
            self
        }
        $crate::timing_setters! { $($rest)* }
    };
    ($(#[$meta:meta])* backoff; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn backoff(
            mut self,
            initial: ::core::time::Duration,
            max: ::core::time::Duration,
        ) -> Self {
            self.timing.initial_backoff = initial;
            self.timing.max_backoff = max;
            self
        }
        $crate::timing_setters! { $($rest)* }
    };
    ($(#[$meta:meta])* connect_timeout; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn connect_timeout(mut self, timeout: ::core::time::Duration) -> Self {
            self.timing.connect_timeout = timeout;
            self
        }
        $crate::timing_setters! { $($rest)* }
    };
    ($(#[$meta:meta])* max_connection_age; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn max_connection_age(mut self, age: ::core::time::Duration) -> Self {
            self.timing.max_connection_age = ::core::option::Option::Some(age);
            self
        }
        $crate::timing_setters! { $($rest)* }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A builder that takes all five setters, as a venue's would.
    #[derive(Debug, Default)]
    struct Builder {
        timing: Timing,
    }

    impl Builder {
        crate::timing_setters! {
            /// How often to ping.
            ping_interval;
            /// How long the connection may be silent.
            stale_after;
            /// Reconnect delay bounds.
            backoff;
            /// How long one connection attempt may take.
            connect_timeout;
            /// How long a connection may live.
            max_connection_age;
        }
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn the_defaults_are_the_shared_cadences() {
        let timing = Timing::default();
        assert_eq!(timing.ping_interval, Some(secs(20)));
        assert_eq!(timing.stale_after, secs(30));
        assert_eq!(timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(timing.max_backoff, secs(60));
        assert_eq!(timing.connect_timeout, secs(10));
        assert_eq!(timing.max_connection_age, None);
    }

    #[test]
    fn each_setter_writes_its_own_field() {
        let builder = Builder::default()
            .ping_interval(secs(1))
            .stale_after(secs(2))
            .backoff(Duration::from_millis(3), Duration::from_millis(4))
            .connect_timeout(secs(5))
            .max_connection_age(secs(6));
        assert_eq!(
            builder.timing,
            Timing {
                ping_interval: Some(secs(1)),
                stale_after: secs(2),
                initial_backoff: Duration::from_millis(3),
                max_backoff: Duration::from_millis(4),
                connect_timeout: secs(5),
                max_connection_age: Some(secs(6)),
            }
        );
        assert_eq!(builder.timing.backoff().take(), Duration::from_millis(3));
    }

    #[test]
    fn with_no_ping_and_no_age_the_wait_is_the_staleness_window_alone() {
        let timing = Timing {
            ping_interval: None,
            max_connection_age: None,
            ..Timing::default()
        };
        assert_eq!(timing.next_wait(secs(999), secs(12), secs(999)), secs(18));
    }

    #[test]
    fn the_nearest_deadline_wins() {
        let timing = Timing {
            max_connection_age: Some(secs(100)),
            ..Timing::default()
        };
        // Ping in 5 s, staleness in 10 s, rotation in 1 s.
        assert_eq!(timing.next_wait(secs(15), secs(20), secs(99)), secs(1));
        assert_eq!(timing.next_wait(secs(15), secs(20), secs(0)), secs(5));
        assert_eq!(timing.next_wait(secs(0), secs(25), secs(0)), secs(5));
    }

    #[test]
    fn no_deadline_at_all_waits_for_ever() {
        assert_eq!(next_wait([]), Duration::MAX);
        assert_eq!(
            next_wait([(None, secs(1)), (None, secs(2)), (None, Duration::MAX)]),
            Duration::MAX
        );
    }

    #[test]
    fn a_deadline_already_past_waits_zero() {
        assert_eq!(next_wait([(Some(secs(30)), secs(31))]), Duration::ZERO);
        assert_eq!(
            next_wait([(Some(secs(30)), Duration::MAX), (None, secs(0))]),
            Duration::ZERO
        );
    }

    #[test]
    fn a_limit_of_duration_max_saturates_rather_than_overflowing() {
        // The limit perps' and Binance's suites pass to mean never.
        assert_eq!(
            next_wait([(Some(Duration::MAX), secs(1))]),
            Duration::MAX - secs(1)
        );
        let never = Timing {
            ping_interval: Some(Duration::MAX),
            stale_after: Duration::MAX,
            ..Timing::default()
        };
        assert_eq!(
            never.next_wait(secs(1), secs(1), secs(1)),
            Duration::MAX - secs(1)
        );
    }
}
