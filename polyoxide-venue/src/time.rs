//! Wall-clock time as venues send it.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch, the unit venues stamp orders and
/// frames with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct UnixMillis(pub u64);

impl UnixMillis {
    /// Now, by the system clock. A clock that reads before 1970 gives
    /// `UnixMillis(0)`.
    pub fn now() -> Self {
        Self::at(SystemTime::now())
    }

    fn at(time: SystemTime) -> Self {
        Self(time.duration_since(UNIX_EPOCH).map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn now_is_after_2026_and_never_runs_backwards_between_two_calls() {
        // 2026-01-01T00:00:00Z.
        let first = UnixMillis::now();
        assert!(first > UnixMillis(1_767_225_600_000), "{first:?}");
        let second = UnixMillis::now();
        assert!(second >= first, "{second:?} < {first:?}");
    }

    #[test]
    fn a_clock_before_1970_reads_zero() {
        assert_eq!(
            UnixMillis::at(UNIX_EPOCH - Duration::from_secs(1)),
            UnixMillis(0)
        );
        assert_eq!(
            UnixMillis::at(UNIX_EPOCH + Duration::from_millis(1_500)),
            UnixMillis(1_500)
        );
    }
}
