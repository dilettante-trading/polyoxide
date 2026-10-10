//! The strict rulebook: what a response and a stage mean for a harness that
//! ramps one route at fixed rates behind a CDN. Pure functions, unit-tested.
//!
//! Any throttle decides the stage, and any error invalidates it. A stage that
//! fell short of its target rate is under-driven, and one served mostly from
//! the cache, below [`MIN_ORIGIN_SHARE`] from the origin, is saturated: it
//! measured the CDN, not the host. Neither is clean.

use std::time::Duration;

/// Below this share of origin-served replies a stage measured the CDN, not
/// the host.
pub const MIN_ORIGIN_SHARE: f64 = 0.9;

/// One response, as the ramp sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// A 2xx served by the origin.
    Ok,
    /// Served from the CDN's cache.
    CacheHit,
    /// A 429.
    Throttled {
        /// The body's `error` identifier, or `unknown`.
        code: String,
        /// Its `Retry-After`, in whole seconds.
        retry_after: Option<u64>,
    },
    /// Any other status, or `0` when no response arrived.
    Error(u16),
}

/// What a response means: its status, its `x-cache` and `Retry-After`
/// headers, and its body.
pub fn classify(
    status: u16,
    x_cache: Option<&str>,
    retry_after: Option<&str>,
    body: &str,
) -> Reply {
    if status == 429 {
        let code = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("error").and_then(|c| c.as_str()).map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        return Reply::Throttled {
            code,
            retry_after: retry_after.and_then(|v| v.trim().parse().ok()),
        };
    }
    if x_cache.is_some_and(|v| v.to_ascii_lowercase().starts_with("hit")) {
        return Reply::CacheHit;
    }
    if (200..300).contains(&status) {
        Reply::Ok
    } else {
        Reply::Error(status)
    }
}

/// What one stage says.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// No throttle, no error, at rate, from the origin.
    Clean,
    /// The first 429 of the stage.
    Throttled {
        /// When it arrived, from stage start.
        after: Duration,
        /// Its identifier.
        code: String,
    },
    /// Too few replies came from the origin.
    Saturated {
        /// The share that did.
        origin_share: f64,
    },
    /// Some requests failed.
    Invalid {
        /// How many.
        errors: usize,
    },
    /// The harness did not reach its own target rate, so a clean result
    /// would be about a lower rate than the one it is labelled with.
    UnderDriven {
        /// The rate it reached, in requests per second.
        achieved: f64,
    },
}

/// Share of the target rate a stage must actually achieve for a clean
/// verdict to mean anything.
pub const MIN_ACHIEVED_SHARE: f64 = 0.9;

/// Judges one stage of `secs` seconds at a target of `rate` requests per
/// second, from its replies in arrival order.
pub fn judge(replies: &[(Duration, Reply)], rate: f64, secs: u64) -> Verdict {
    if let Some((at, Reply::Throttled { code, .. })) = replies
        .iter()
        .find(|(_, r)| matches!(r, Reply::Throttled { .. }))
    {
        return Verdict::Throttled {
            after: *at,
            code: code.clone(),
        };
    }
    let errors = replies
        .iter()
        .filter(|(_, r)| matches!(r, Reply::Error(_)))
        .count();
    if errors > 0 {
        return Verdict::Invalid { errors };
    }
    let achieved = replies.len() as f64 / secs as f64;
    if achieved < MIN_ACHIEVED_SHARE * rate {
        return Verdict::UnderDriven { achieved };
    }
    let origin = replies.iter().filter(|(_, r)| *r == Reply::Ok).count();
    let share = if replies.is_empty() {
        0.0
    } else {
        origin as f64 / replies.len() as f64
    };
    if share < MIN_ORIGIN_SHARE {
        return Verdict::Saturated {
            origin_share: share,
        };
    }
    Verdict::Clean
}

/// One line of reply counts for a stage, plus the first throttle's
/// `Retry-After`, worth recording alongside the 429 body.
pub fn summarize(replies: &[(Duration, Reply)]) -> String {
    let count = |f: &dyn Fn(&Reply) -> bool| replies.iter().filter(|(_, r)| f(r)).count();
    let ok = count(&|r| *r == Reply::Ok);
    let cached = count(&|r| *r == Reply::CacheHit);
    let throttled = count(&|r| matches!(r, Reply::Throttled { .. }));
    let errors = count(&|r| matches!(r, Reply::Error(_)));
    let retry_after = replies.iter().find_map(|(_, r)| match r {
        Reply::Throttled { retry_after, .. } => *retry_after,
        _ => None,
    });
    let last = replies.last().map_or(Duration::ZERO, |(at, _)| *at);
    format!(
        "{ok} origin, {cached} cached, {throttled} throttled, {errors} errors; \
         last reply at {last:.1?}; first Retry-After {retry_after:?}"
    )
}

/// The count to pin for a 10-second window: the highest clean stage rate.
pub fn pin(stages: &[(f64, Verdict)]) -> Option<u32> {
    stages
        .iter()
        .take_while(|(_, v)| *v == Verdict::Clean)
        .last()
        .map(|(rate, _)| (rate * 10.0).floor() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(at: u64) -> (Duration, Reply) {
        (Duration::from_secs(at), Reply::Ok)
    }

    #[test]
    fn a_429_is_throttled_with_its_identifier_and_retry_after() {
        assert_eq!(
            classify(
                429,
                None,
                Some("2"),
                r#"{"status":"err","error":"ip_rate_limited"}"#
            ),
            Reply::Throttled {
                code: "ip_rate_limited".into(),
                retry_after: Some(2)
            }
        );
    }

    #[test]
    fn a_cache_hit_is_not_an_origin_reply() {
        assert_eq!(
            classify(200, Some("Hit from cloudfront"), None, "[]"),
            Reply::CacheHit
        );
        assert_eq!(
            classify(200, Some("Miss from cloudfront"), None, "[]"),
            Reply::Ok
        );
    }

    #[test]
    fn a_stage_with_any_throttle_is_throttled_at_its_first() {
        let replies = vec![
            ok(1),
            (
                Duration::from_secs(7),
                Reply::Throttled {
                    code: "ip_rate_limited".into(),
                    retry_after: None,
                },
            ),
            ok(9),
        ];
        assert_eq!(
            judge(&replies, 1.0, 10),
            Verdict::Throttled {
                after: Duration::from_secs(7),
                code: "ip_rate_limited".into()
            }
        );
    }

    #[test]
    fn a_stage_mostly_served_from_cache_is_saturated_not_clean() {
        let replies: Vec<_> = (0..10)
            .map(|i| {
                if i < 5 {
                    ok(i)
                } else {
                    (Duration::from_secs(i), Reply::CacheHit)
                }
            })
            .collect();
        assert_eq!(
            judge(&replies, 1.0, 10),
            Verdict::Saturated { origin_share: 0.5 }
        );
    }

    #[test]
    fn a_stage_that_did_not_reach_its_rate_is_under_driven_not_clean() {
        // 5 replies in 10 s at a 1 req/s target is half the rate: a clean
        // verdict here would pin a number the host was never asked for.
        let replies: Vec<_> = (0..5).map(ok).collect();
        assert_eq!(
            judge(&replies, 1.0, 10),
            Verdict::UnderDriven { achieved: 0.5 }
        );
        let replies: Vec<_> = (0..10).map(ok).collect();
        assert_eq!(judge(&replies, 1.0, 10), Verdict::Clean);
    }

    #[test]
    fn pin_is_the_last_clean_stage_before_the_first_unclean_one() {
        let stages = vec![
            (5.0, Verdict::Clean),
            (10.0, Verdict::Clean),
            (
                15.0,
                Verdict::Throttled {
                    after: Duration::from_secs(3),
                    code: "x".into(),
                },
            ),
            (20.0, Verdict::Clean),
        ];
        assert_eq!(pin(&stages), Some(100));
        assert_eq!(pin(&[(5.0, Verdict::Invalid { errors: 1 })]), None);
    }
}
