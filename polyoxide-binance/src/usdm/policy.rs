//! [`UsdmRetryPolicy`]: what follows each USDⓈ-M response.

use std::time::Duration;

use polyoxide_core::{AttemptInfo, Decision, Outcome, ResponseMeta, RetryConfig, RetryPolicy};
use reqwest::StatusCode;

use crate::{
    error::retry_after_secs,
    weight::{WeightBudget, DEFAULT_BAN},
};

/// Binance's retry policy, over the [`WeightBudget`] whose minute it reads.
///
/// - A `429` with a retry left is retried, and holds every request on the
///   budget for the longer of the attempt's backoff and its `Retry-After`.
/// - A `429` with none left fails, and holds for its `Retry-After`, or until
///   the next UTC minute when it has none: sending again into a spent minute
///   is how a `429` becomes a `418` ban.
/// - A `418` (the IP is banned) fails, and holds for its `Retry-After`, or
///   [`DEFAULT_BAN`] when it has none.
/// - A 2xx is done. Everything else fails with no hold, a `425` included:
///   Binance does not document it as retriable.
///
/// A `Retry-After` is clamped to [`MAX_COOLDOWN`](crate::weight::MAX_COOLDOWN),
/// the longest ban Binance documents.
#[derive(Debug, Clone)]
pub(crate) struct UsdmRetryPolicy {
    pub(crate) budget: WeightBudget,
}

impl RetryPolicy for UsdmRetryPolicy {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        attempt: &AttemptInfo,
        schedule: &RetryConfig,
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
                hold: Some(asked.unwrap_or_else(|| self.budget.until_next_minute())),
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
}

#[cfg(test)]
mod tests {
    use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};

    use super::*;

    /// 2026-10-07 08:35:15 UTC: 45 s before a minute boundary.
    const FIFTEEN_INTO_A_MINUTE: u64 = 1_791_362_115_000;

    fn policy() -> UsdmRetryPolicy {
        UsdmRetryPolicy {
            budget: WeightBudget::at_ms(FIFTEEN_INTO_A_MINUTE),
        }
    }

    /// One retry at a 100ms base, so a retry's own backoff is 75-125ms.
    fn schedule() -> RetryConfig {
        RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 100,
            max_backoff_ms: 10_000,
        }
    }

    fn decide(status: u16, retry_after: Option<&str>, attempt: u32) -> Decision {
        let mut headers = HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(RETRY_AFTER, HeaderValue::from_str(value).unwrap());
        }
        let schedule = schedule();
        policy().decide(
            &ResponseMeta {
                status: StatusCode::from_u16(status).unwrap(),
                headers: &headers,
            },
            &schedule.attempt_info(attempt),
            &schedule,
        )
    }

    #[test]
    fn a_2xx_is_done() {
        for status in [200, 204] {
            assert_eq!(
                decide(status, Some("5"), 0),
                Decision {
                    outcome: Outcome::Done,
                    hold: None
                },
                "{status}"
            );
        }
    }

    #[test]
    fn a_429_with_a_retry_left_holds_the_longer_of_its_wait_and_retry_after() {
        let asked = decide(429, Some("1"), 0);
        assert_eq!(asked.outcome, Outcome::Retry(Duration::ZERO));
        assert_eq!(asked.hold, Some(Duration::from_secs(1)));

        // With no Retry-After, the hold is the attempt's own backoff.
        let unasked = decide(429, None, 0);
        assert_eq!(unasked.outcome, Outcome::Retry(Duration::ZERO));
        let hold = unasked.hold.unwrap().as_millis();
        assert!((75..=125).contains(&hold), "{hold}ms");

        // A Retry-After of 10 days is past the schedule's clamp and is held
        // for the longest documented ban.
        let banned = decide(429, Some("864000"), 0);
        assert_eq!(banned.hold, Some(crate::weight::MAX_COOLDOWN));
    }

    #[test]
    fn a_429_with_none_left_holds_its_retry_after() {
        assert_eq!(
            decide(429, Some("1"), 1),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(1))
            }
        );
    }

    #[test]
    fn a_429_with_none_left_and_no_retry_after_holds_to_the_next_minute() {
        assert_eq!(
            decide(429, None, 1),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(45))
            }
        );
    }

    #[test]
    fn a_418_fails_and_holds_its_retry_after() {
        for attempt in [0, 1] {
            assert_eq!(
                decide(418, Some("1"), attempt),
                Decision {
                    outcome: Outcome::Fail,
                    hold: Some(Duration::from_secs(1))
                },
                "attempt {attempt}"
            );
        }
        // Ten days is held for the longest documented ban, three days.
        assert_eq!(
            decide(418, Some("864000"), 0).hold,
            Some(crate::weight::MAX_COOLDOWN)
        );
    }

    #[test]
    fn a_418_without_retry_after_holds_two_minutes() {
        assert_eq!(
            decide(418, None, 0),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(120))
            }
        );
    }

    #[test]
    fn a_425_and_a_5xx_fail_with_no_hold() {
        for status in [425, 500, 503, 400, 403, 451] {
            assert_eq!(
                decide(status, Some("1"), 0),
                Decision {
                    outcome: Outcome::Fail,
                    hold: None
                },
                "{status}"
            );
        }
    }
}
