//! [`PolymarketRetryPolicy`], the one retry policy every Polymarket HTTP
//! module installs on [`HttpClient::send`](polyoxide_core::HttpClient::send):
//! gamma, data, perps, clob and relay.

use std::time::Duration;

use polyoxide_core::hooks::{
    AttemptInfo, Decision, DefaultRetryPolicy, Outcome, ResponseMeta, RetryPolicy,
};
use polyoxide_core::rate_limit::RetryConfig;
use polyoxide_core::reqwest::StatusCode;

/// Polymarket's one retry policy: core's [`DefaultRetryPolicy`], plus
/// `425 Too Early`.
///
/// Retries two statuses, both of which upstream documents as "retry with
/// exponential backoff":
///
/// - `429 Too Many Requests` — rate limited. As in [`DefaultRetryPolicy`], it
///   holds every request on the throttle for the schedule's first delay, even
///   when no retry is left.
/// - `425 Too Early` — Polymarket's matching engine is restarting. It returns
///   this with no body, so nothing was processed. It is retried after the
///   loop's floor and holds nothing: only the request that saw it waits.
///
/// With no retry left (`retries_left == 0`), either status is a
/// [`Outcome::Fail`], and a `429` keeps its hold.
///
/// Deliberately narrow: 5xx and 408 are *not* retried. A 5xx is retriable in
/// the [`Classify`](polyoxide_venue::Classify) sense, but it can mean the
/// request was partially applied, and the loop resends non-idempotent
/// writes. 503 in particular is what post-only mode answers, with a
/// `Retry-After` of about 79 s: far too long to block inside a request, and it
/// rejects orders wholesale. The two statuses above are safe because neither
/// reaches the matching engine — and for order placement the resent body is
/// byte-identical, so the order hash is unchanged and the venue rejects a
/// genuine double-submit as a duplicate. Callers wanting broader retry
/// semantics should drive them from the error's class with their own
/// idempotency judgement. Narrowing this set needs a DRIFT row (AD-17).
#[derive(Debug, Clone, Copy, Default)]
pub struct PolymarketRetryPolicy;

impl RetryPolicy for PolymarketRetryPolicy {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        attempt: &AttemptInfo,
        schedule: &RetryConfig,
    ) -> Decision {
        let decision = if response.status == StatusCode::TOO_EARLY {
            Decision {
                outcome: Outcome::Retry(Duration::ZERO),
                hold: None,
            }
        } else {
            DefaultRetryPolicy.decide(response, attempt, schedule)
        };
        match decision.outcome {
            // Out of attempts: the request fails, and a 429's hold stands.
            Outcome::Retry(_) if attempt.retries_left == 0 => Decision {
                outcome: Outcome::Fail,
                ..decision
            },
            _ => decision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_core::reqwest;
    use polyoxide_core::ApiError;

    // ── Polymarket's retry decision and the retry delay ──────────
    //
    // These tests drove `HttpClient::should_retry` until the hand-written
    // loops that called it moved onto the send loop. They keep their names
    // and their answers, read from the policy the loop asks and from
    // `RetryConfig::retry_delay`, the loop's own floor.

    /// Whether Polymarket's policy retries `status` on `attempt` under
    /// `config`. `decide` itself fails a request with no retry left, so this
    /// is the send loop's answer.
    fn retries(config: &RetryConfig, status: StatusCode, attempt: u32) -> bool {
        use polyoxide_core::hooks::{Outcome, ResponseMeta};

        let headers = reqwest::header::HeaderMap::new();
        let response = ResponseMeta {
            status,
            headers: &headers,
        };
        matches!(
            crate::shared::PolymarketRetryPolicy
                .decide(&response, &config.attempt_info(attempt), config)
                .outcome,
            Outcome::Retry(_)
        )
    }

    #[test]
    fn test_should_retry_429_under_max() {
        let config = RetryConfig::default();
        // Default max_retries=3, so attempts 0 and 2 should retry
        assert!(retries(&config, StatusCode::TOO_MANY_REQUESTS, 0));
        assert!(retries(&config, StatusCode::TOO_MANY_REQUESTS, 2));
    }

    #[test]
    fn test_should_retry_429_at_max() {
        let config = RetryConfig::default();
        // attempt == max_retries → no retry
        assert!(!retries(&config, StatusCode::TOO_MANY_REQUESTS, 3));
    }

    #[test]
    fn test_should_retry_425_under_max() {
        let config = RetryConfig::default();
        // 425 Too Early — Polymarket's matching engine restarting.
        assert!(retries(&config, StatusCode::TOO_EARLY, 0));
        assert!(retries(&config, StatusCode::TOO_EARLY, 2));
    }

    #[test]
    fn test_should_retry_425_at_max() {
        let config = RetryConfig::default();
        assert!(!retries(&config, StatusCode::TOO_EARLY, 3));
    }

    #[test]
    fn test_should_retry_ignores_other_statuses() {
        let config = RetryConfig::default();
        for status in [
            StatusCode::OK,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::BAD_REQUEST,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            // 503 is deliberately excluded even though post-only mode sends it
            // with a Retry-After: the documented wait is ~79s, far too long to
            // block inside a request, and it rejects orders wholesale.
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(!retries(&config, status, 0), "{status} was retried");
        }
    }

    #[test]
    fn test_should_retry_5xx_not_retried_despite_being_is_retriable() {
        // The class describes the error; this loop resends writes, so it stays narrower.
        let config = RetryConfig::default();
        assert!(!retries(&config, StatusCode::INTERNAL_SERVER_ERROR, 0));
        let err = ApiError::from(polyoxide_core::error::ErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Default::default(),
            "",
        ));
        assert!(polyoxide_venue::Classify::is_retriable(&err));
    }

    #[test]
    fn test_should_retry_custom_config() {
        let config = RetryConfig {
            max_retries: 1,
            ..RetryConfig::default()
        };
        assert!(retries(&config, StatusCode::TOO_MANY_REQUESTS, 0));
        assert!(!retries(&config, StatusCode::TOO_MANY_REQUESTS, 1));
    }

    // ── should_retry edge cases ─────────────────────────────────
    //
    // Driven through Polymarket's policy, which the send loop asks, since the
    // hand-written loops that called `HttpClient::should_retry` are gone.

    /// Whether Polymarket's policy retries a 429 on `attempt` under `config`.
    fn retries_a_429(config: &RetryConfig, attempt: u32) -> bool {
        use polyoxide_core::hooks::{Outcome, ResponseMeta, RetryPolicy};

        let headers = reqwest::header::HeaderMap::new();
        let response = ResponseMeta {
            status: reqwest::StatusCode::TOO_MANY_REQUESTS,
            headers: &headers,
        };
        matches!(
            crate::shared::PolymarketRetryPolicy
                .decide(&response, &config.attempt_info(attempt), config)
                .outcome,
            Outcome::Retry(_)
        )
    }

    #[test]
    fn test_should_retry_exhaustion() {
        // After max_retries, the policy must not retry
        let config = RetryConfig {
            max_retries: 3,
            ..RetryConfig::default()
        };

        // Attempts 0, 1, 2 should succeed
        for attempt in 0..3 {
            assert!(
                retries_a_429(&config, attempt),
                "attempt {attempt} should allow retry"
            );
        }
        // Attempt 3 should give up
        assert!(
            !retries_a_429(&config, 3),
            "attempt 3 should exhaust retries"
        );
    }

    #[test]
    fn test_should_retry_zero_max_retries_never_retries() {
        let config = RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        };

        assert!(
            !retries_a_429(&config, 0),
            "max_retries=0 should never retry"
        );
    }
}
