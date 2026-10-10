//! Polymarket's hooks for core's send loop: its retry policy, its rate limit
//! tables, and the CLOB's composed throttle.
//!
//! Every Polymarket client shares them on
//! [`HttpClient::send`](crate::HttpClient::send): gamma, data, perps, clob and
//! relay. The module moves to `polyoxide-polymarket` in S2.

use std::time::Duration;

use reqwest::StatusCode;

use crate::hooks::{AttemptInfo, Decision, DefaultRetryPolicy, Outcome, ResponseMeta, RetryPolicy};
use crate::rate_limit::RetryConfig;

mod limits;
mod throttle;

pub use limits::{clob_limits, data_limits, gamma_limits, perps_limits, relay_limits};
pub use throttle::{
    clob_throttle, signer_cost, ClobThrottle, CLOUDFLARE, SIGNER_CANCEL, SIGNER_ORDER,
};

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
