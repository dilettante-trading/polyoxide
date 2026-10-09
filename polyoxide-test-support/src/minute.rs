//! Waits that keep a test or a probe clear of a UTC minute boundary.
//!
//! A budget counted per clock minute resets on the boundary, so a test that
//! holds it for a moment, or a probe that reads its counter across several
//! requests, must not straddle one. The window is in milliseconds into the
//! minute; outside it, the wait lands at `land_at` milliseconds into the next
//! minute, or into this one when `land_at` is still ahead.

use std::{ops::RangeInclusive, time::Duration};

/// Milliseconds into the current UTC minute, by the local clock.
pub fn ms_into_minute() -> u64 {
    polyoxide_venue::UnixMillis::now().0 % 60_000
}

/// How long to wait, at `at` milliseconds into the minute, to be inside
/// `window`: `None` when `at` already is, or else until `land_at`
/// milliseconds into the minute.
///
/// ```
/// use std::time::Duration;
/// use polyoxide_test_support::minute::wait_needed;
///
/// assert_eq!(wait_needed(0..=57_000, 100, 12_000), None);
/// assert_eq!(wait_needed(0..=57_000, 100, 58_000), Some(Duration::from_millis(2_100)));
/// assert_eq!(wait_needed(3_000..=20_000, 3_000, 1_000), Some(Duration::from_millis(2_000)));
/// ```
pub fn wait_needed(window: RangeInclusive<u64>, land_at: u64, at: u64) -> Option<Duration> {
    if window.contains(&at) {
        None
    } else {
        Some(Duration::from_millis((60_000 + land_at - at) % 60_000))
    }
}

/// Sleeps until the clock is inside `window`, landing at `land_at`
/// milliseconds into the minute, unless it already is. Returns the wait.
pub async fn wait_unless_within(window: RangeInclusive<u64>, land_at: u64) -> Option<Duration> {
    let wait = wait_needed(window, land_at, ms_into_minute());
    if let Some(wait) = wait {
        tokio::time::sleep(wait).await;
    }
    wait
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_the_window_needs_no_wait() {
        for at in [0, 30_000, 57_000] {
            assert_eq!(wait_needed(0..=57_000, 100, at), None, "{at}");
        }
    }

    #[test]
    fn past_the_window_waits_into_the_next_minute() {
        // The end of a minute: land 100 ms past the boundary.
        assert_eq!(
            wait_needed(0..=57_000, 100, 57_001),
            Some(Duration::from_millis(3_099))
        );
        assert_eq!(
            wait_needed(0..=57_000, 100, 59_999),
            Some(Duration::from_millis(101))
        );
        assert_eq!(
            wait_needed(3_000..=20_000, 3_000, 20_001),
            Some(Duration::from_millis(42_999))
        );
    }

    #[test]
    fn before_the_window_waits_within_this_minute() {
        assert_eq!(
            wait_needed(3_000..=20_000, 3_000, 0),
            Some(Duration::from_millis(3_000))
        );
        assert_eq!(
            wait_needed(3_000..=20_000, 3_000, 2_999),
            Some(Duration::from_millis(1))
        );
    }

    #[test]
    fn the_clock_reads_inside_a_minute() {
        assert!(ms_into_minute() < 60_000);
    }
}
