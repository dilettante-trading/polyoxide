//! What follows each Binance REST response: the retry decision every Binance
//! product's policy makes.

use std::time::Duration;

use polyoxide_core::http::StatusCode;
use polyoxide_core::{AttemptInfo, Decision, Outcome, ResponseMeta, RetryConfig};

use crate::shared::error::retry_after_secs;
use crate::shared::weight::DEFAULT_BAN;

/// What follows `response`, by Binance's rules.
///
/// - A `429` with a retry left is retried, and holds every request on the
///   budget for the longer of the attempt's backoff and its `Retry-After`.
/// - A `429` with none left fails, and holds for its `Retry-After`, or for
///   `until_next_minute` when it has none: sending again into a spent minute
///   is how a `429` becomes a `418` ban.
/// - A `418` (the IP is banned) fails, and holds for its `Retry-After`, or
///   [`DEFAULT_BAN`] when it has none.
/// - A 2xx is done. Everything else fails with no hold, a `425` included:
///   Binance does not document it as retriable.
///
/// A `Retry-After` is clamped to `MAX_COOLDOWN`, the longest ban Binance
/// documents. `until_next_minute` is the time to the next UTC minute on the
/// product's own weight budget.
pub(crate) fn decide(
    response: &ResponseMeta<'_>,
    attempt: &AttemptInfo,
    schedule: &RetryConfig,
    until_next_minute: Duration,
) -> Decision {
    let asked = retry_after_secs(response.retry_after());
    match response.status {
        StatusCode::TOO_MANY_REQUESTS if attempt.retries_left > 0 => Decision {
            // The loop's floor is the attempt's backoff; the hold makes
            // every other request wait at least as long.
            outcome: Outcome::Retry(Duration::ZERO),
            hold: Some(
                schedule
                    .retry_delay(attempt.attempt, response.retry_after())
                    .max(asked.unwrap_or_default()),
            ),
        },
        StatusCode::TOO_MANY_REQUESTS => Decision {
            outcome: Outcome::Fail,
            hold: Some(asked.unwrap_or(until_next_minute)),
        },
        StatusCode::IM_A_TEAPOT => Decision {
            outcome: Outcome::Fail,
            hold: Some(asked.unwrap_or(DEFAULT_BAN)),
        },
        status if status.is_success() => Decision {
            outcome: Outcome::Done,
            hold: None,
        },
        _ => Decision {
            outcome: Outcome::Fail,
            hold: None,
        },
    }
}
