//! The one `Retry-After` parser, and the rule that it only lengthens a wait.

use std::time::Duration;

/// Reads a `Retry-After` value as decimal seconds, whole or fractional, capped
/// at `clamp`.
///
/// The value is trimmed first. `None` for anything that is not a wait: zero,
/// a negative number, NaN, an infinity (`1e400` included, which overflows to
/// one), an HTTP-date, an empty string or any other text, and a positive value
/// so small it rounds to zero. A finite value longer than `clamp` comes back
/// as `clamp`, so a server cannot park a client for longer than the caller
/// allows, and no value panics.
///
/// The result is the server's request, not the delay to use: pass it to
/// [`retry_delay`] with the client's own backoff.
///
/// ```
/// use std::time::Duration;
/// use polyoxide_venue::parse_retry_after;
///
/// let clamp = Duration::from_secs(60);
/// assert_eq!(parse_retry_after(" 1.5 ", clamp), Some(Duration::from_millis(1500)));
/// assert_eq!(parse_retry_after("86400", clamp), Some(clamp));
/// assert_eq!(parse_retry_after("0", clamp), None);
/// ```
pub fn parse_retry_after(value: &str, clamp: Duration) -> Option<Duration> {
    let secs = value.trim().parse::<f64>().ok()?;
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    // A finite value too large for a `Duration` overflows here rather than
    // panicking, and is longer than any clamp.
    let wait = Duration::try_from_secs_f64(secs).map_or(clamp, |wait| wait.min(clamp));
    Some(wait).filter(|wait| !wait.is_zero())
}

/// The delay before the next attempt: the longer of what the server asked for
/// and the client's own backoff.
///
/// A server's `Retry-After` may only lengthen the wait. Cloudflare sends one
/// that floors to zero, and obeying it verbatim sends retries straight back
/// into the ban they are waiting out.
pub fn retry_delay(requested: Option<Duration>, computed: Duration) -> Duration {
    requested.map_or(computed, |requested| requested.max(computed))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAMP: Duration = Duration::from_secs(60);

    #[test]
    fn every_parser_row() {
        let rows = [
            ("1", Some(Duration::from_secs(1))),
            ("1.5", Some(Duration::from_millis(1500))),
            (" 2 ", Some(Duration::from_secs(2))),
            ("\t3\n", Some(Duration::from_secs(3))),
            ("+4", Some(Duration::from_secs(4))),
            ("60", Some(CLAMP)),
            ("61", Some(CLAMP)),
            ("1e20", Some(CLAMP)),
            // Finite as an f64, far past what a `Duration` holds.
            ("1e300", Some(CLAMP)),
            ("0", None),
            ("0.0", None),
            ("-0", None),
            ("-1", None),
            ("NaN", None),
            ("inf", None),
            ("-inf", None),
            ("infinity", None),
            ("1e400", None),
            ("0.0000000004", None),
            ("Wed, 21 Oct 2026 07:28:00 GMT", None),
            ("", None),
            ("   ", None),
            ("abc", None),
            ("5s", None),
        ];
        for (value, wait) in rows {
            assert_eq!(parse_retry_after(value, CLAMP), wait, "{value:?}");
        }
    }

    #[test]
    fn the_clamp_is_the_callers() {
        assert_eq!(
            parse_retry_after("604800", Duration::from_secs(3 * 86_400)),
            Some(Duration::from_secs(3 * 86_400))
        );
        assert_eq!(
            parse_retry_after("1e20", Duration::MAX),
            Some(Duration::MAX),
            "a huge value against the largest clamp neither panics nor wraps"
        );
        assert_eq!(parse_retry_after("5", Duration::ZERO), None);
    }

    #[test]
    fn a_requested_wait_only_ever_lengthens_the_backoff() {
        let backoff = Duration::from_millis(500);
        assert_eq!(retry_delay(None, backoff), backoff);
        assert_eq!(
            retry_delay(Some(Duration::from_millis(100)), backoff),
            backoff
        );
        assert_eq!(retry_delay(Some(Duration::ZERO), backoff), backoff);
        assert_eq!(
            retry_delay(Some(Duration::from_secs(3)), backoff),
            Duration::from_secs(3)
        );
    }
}
