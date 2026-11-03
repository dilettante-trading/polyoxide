//! [`SpotRetryPolicy`]: what follows each spot response.

use polyoxide_core::{AttemptInfo, Decision, ResponseMeta, RetryConfig, RetryPolicy};

use crate::spot::weight::WeightBudget;

/// Binance spot's retry policy, over the [`WeightBudget`] whose minute it reads.
///
/// It is the USDⓈ-M module's policy, decided by the crate's shared
/// `decide`:
///
/// - A `429` with a retry left is retried, and holds every request on the
///   budget for the longer of the attempt's backoff and its `Retry-After`.
/// - A `429` with none left fails, and holds for its `Retry-After`, or until
///   the next UTC minute when it has none: sending again into a spent minute
///   is how a `429` becomes a `418` ban.
/// - A `418` (the IP is banned) fails, and holds for its `Retry-After`, or
///   [`DEFAULT_BAN`](crate::spot::weight::DEFAULT_BAN) when it has none.
/// - A 2xx is done. Everything else fails with no hold, a `425` included:
///   Binance does not document it as retriable.
///
/// A `Retry-After` is clamped to
/// [`MAX_COOLDOWN`](crate::spot::weight::MAX_COOLDOWN), the longest ban
/// Binance documents.
#[derive(Debug, Clone)]
pub(crate) struct SpotRetryPolicy {
    pub(crate) budget: WeightBudget,
}

impl RetryPolicy for SpotRetryPolicy {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        attempt: &AttemptInfo,
        schedule: &RetryConfig,
    ) -> Decision {
        crate::shared::policy::decide(response, attempt, schedule, self.budget.until_next_minute())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use polyoxide_core::http::{
        header::{HeaderMap, HeaderValue, RETRY_AFTER},
        StatusCode,
    };
    use polyoxide_core::Outcome;

    use super::*;
    use crate::spot::weight::{DEFAULT_BAN, MAX_COOLDOWN};

    /// 2026-11-02 12:00:15 UTC: 45 s before a minute boundary.
    const FIFTEEN_INTO_A_MINUTE: u64 = 1_793_620_815_000;

    fn policy() -> SpotRetryPolicy {
        SpotRetryPolicy {
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
        assert_eq!(
            decide(200, Some("5"), 0),
            Decision {
                outcome: Outcome::Done,
                hold: None
            }
        );
    }

    #[test]
    fn a_429_with_a_retry_left_retries_and_holds_the_longer_of_its_wait_and_retry_after() {
        let asked = decide(429, Some("1"), 0);
        assert_eq!(asked.outcome, Outcome::Retry(Duration::ZERO));
        assert_eq!(asked.hold, Some(Duration::from_secs(1)));
        let unasked = decide(429, None, 0);
        assert_eq!(unasked.outcome, Outcome::Retry(Duration::ZERO));
        let hold = unasked.hold.unwrap().as_millis();
        assert!((75..=125).contains(&hold), "{hold}ms");
    }

    #[test]
    fn a_429_with_none_left_holds_to_the_next_minute_of_this_budget() {
        // Spot's budget reads its own clock: 45 s are left of the minute.
        assert_eq!(
            decide(429, None, 1),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(45))
            }
        );
        assert_eq!(
            decide(429, Some("3"), 1),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(3))
            }
        );
    }

    #[test]
    fn a_418_fails_and_holds_its_retry_after_or_two_minutes() {
        assert_eq!(
            decide(418, Some("1"), 0),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(1))
            }
        );
        assert_eq!(
            decide(418, None, 0),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(DEFAULT_BAN)
            }
        );
        assert_eq!(DEFAULT_BAN, Duration::from_secs(120));
        assert_eq!(
            decide(418, Some("864000"), 0).hold,
            Some(MAX_COOLDOWN),
            "ten days is held for the longest documented ban"
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
