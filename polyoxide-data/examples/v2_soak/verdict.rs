//! What a response, a stage and a whole ramp mean. Pure functions, so the
//! rules that decide a pinned rate limit are unit-tested rather than read off
//! a terminal.

use std::time::Duration;

use crate::common::percentile;

/// Where a 429 came from. The two layers have different windows and different
/// consequences: Cloudflare's `error code: 1015` blocks the whole host for this
/// IP, and traffic during the block prolongs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// The v2 origin's per-client allowance: a JSON body with `code: rate_limited`.
    Origin,
    /// Cloudflare's IP rule: a plain-text `error code: 1015` body.
    Cloudflare,
    /// A 429 in neither shape.
    Unknown,
}

/// One response, as the soak sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Ok,
    /// Served by CloudFront from cache; the origin never saw the request.
    CacheHit,
    Throttled {
        layer: Layer,
        retry_after: Option<Duration>,
    },
    Error(u16),
}

pub fn classify(
    status: u16,
    x_cache: Option<&str>,
    retry_after: Option<&str>,
    body: &str,
) -> Reply {
    if status == 429 {
        let code = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("code").and_then(|c| c.as_str()).map(str::to_owned));
        let layer = match code.as_deref() {
            Some("rate_limited") => Layer::Origin,
            _ if body.contains("1015") => Layer::Cloudflare,
            _ => Layer::Unknown,
        };
        let retry_after = retry_after
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|secs| secs.is_finite() && *secs >= 0.0)
            .map(Duration::from_secs_f64);
        return Reply::Throttled { layer, retry_after };
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

#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// The route's path, so a mixed run can say which route was throttled.
    pub path: &'static str,
    /// From stage start to this response completing.
    pub finished_at: Duration,
    pub latency: Duration,
    pub reply: Reply,
}

/// Why a stage stopped before its planned duration, other than a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abort {
    DuplicateUrl,
    ProbeSpaceExhausted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    /// Requests per second driven by the harness; `None` when the shipped
    /// limiter set the pace.
    pub target_rps: Option<f64>,
    pub planned: Duration,
    pub elapsed: Duration,
    pub samples: Vec<Sample>,
    pub abort: Option<Abort>,
}

impl Stage {
    /// Requests per second over the planned duration. Every request sent
    /// before the deadline yields one sample, but the last ones complete after
    /// it, so dividing by the elapsed time would understate the rate the
    /// harness actually drove.
    pub fn achieved_rps(&self) -> f64 {
        if self.planned.is_zero() {
            0.0
        } else {
            self.samples.len() as f64 / self.planned.as_secs_f64()
        }
    }

    pub fn latencies(&self) -> Vec<Duration> {
        let mut sorted: Vec<Duration> = self.samples.iter().map(|s| s.latency).collect();
        sorted.sort_unstable();
        sorted
    }

    pub fn p50(&self) -> Duration {
        percentile(&self.latencies(), 50.0)
    }

    pub fn p99(&self) -> Duration {
        percentile(&self.latencies(), 99.0)
    }

    pub fn count(&self, pred: impl Fn(&Reply) -> bool) -> usize {
        self.samples.iter().filter(|s| pred(&s.reply)).count()
    }
}

/// A stage whose p99 latency exceeds this multiple of the first stage's is
/// treated as saturated: upstream queues heavy queries for a capacity slot
/// before it answers 429, so latency climbs first.
pub const SATURATION_FACTOR: u32 = 3;

/// A stage that drove less than this share of its target rate did not measure
/// that rate: the harness, not the server, was the bottleneck.
pub const MIN_ACHIEVED_SHARE: f64 = 0.9;

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Clean,
    Throttled {
        path: &'static str,
        layer: Layer,
        retry_after: Option<Duration>,
        at: Duration,
    },
    Saturated {
        p99: Duration,
        baseline_p99: Duration,
    },
    Invalid(String),
}

/// Judges one stage. `baseline_p99` is the first stage's p99, or `None` for
/// the first stage itself and for a validation run.
pub fn judge(stage: &Stage, baseline_p99: Option<Duration>) -> Verdict {
    if let Some(sample) = stage
        .samples
        .iter()
        .find(|s| matches!(s.reply, Reply::Throttled { .. }))
    {
        if let Reply::Throttled { layer, retry_after } = sample.reply {
            return Verdict::Throttled {
                path: sample.path,
                layer,
                retry_after,
                at: sample.finished_at,
            };
        }
    }
    match stage.abort {
        Some(Abort::DuplicateUrl) => {
            return Verdict::Invalid(
                "a probe URL repeated, so the CDN cache could answer it".into(),
            )
        }
        Some(Abort::ProbeSpaceExhausted) => {
            return Verdict::Invalid(
                "the probe space ran out; bootstrap more wallets or conditions".into(),
            )
        }
        None => {}
    }
    let hits = stage.count(|r| *r == Reply::CacheHit);
    if hits > 0 {
        return Verdict::Invalid(format!("{hits} responses came from the CDN cache"));
    }
    let total = stage.samples.len();
    if total == 0 {
        return Verdict::Invalid("no responses".into());
    }
    let errors = stage.count(|r| matches!(r, Reply::Error(_)));
    if errors * 100 >= total {
        let mut statuses: Vec<u16> = stage
            .samples
            .iter()
            .filter_map(|s| match s.reply {
                Reply::Error(status) => Some(status),
                _ => None,
            })
            .collect();
        statuses.sort_unstable();
        statuses.dedup();
        let statuses: Vec<String> = statuses
            .iter()
            .map(|status| match status {
                0 => "transport".to_owned(),
                status => status.to_string(),
            })
            .collect();
        return Verdict::Invalid(format!(
            "{errors} of {total} requests failed (status {})",
            statuses.join(", ")
        ));
    }
    if stage.elapsed.as_secs_f64() < stage.planned.as_secs_f64() * 0.95 {
        return Verdict::Invalid("the stage ended before its planned duration".into());
    }
    if let Some(target) = stage.target_rps {
        let achieved = stage.achieved_rps();
        if achieved < target * MIN_ACHIEVED_SHARE {
            return Verdict::Invalid(format!(
                "achieved {achieved:.2} of {target} req/s; raise --concurrency"
            ));
        }
    }
    if let Some(baseline_p99) = baseline_p99 {
        let p99 = stage.p99();
        if p99 > baseline_p99 * SATURATION_FACTOR {
            return Verdict::Saturated { p99, baseline_p99 };
        }
    }
    Verdict::Clean
}

/// What a finished ramp says to pin.
#[derive(Debug, Clone, PartialEq)]
pub enum Pin {
    /// Requests per 10 seconds for the route's `simple_limit` row.
    Count(u32),
    /// Not even the lowest stage was clean: re-run with lower stages.
    RetryLower,
    Invalid(String),
}

/// Pins the highest clean rate below the first stage that was throttled or
/// saturated, as a count per 10 seconds. `quota()` then reserves a tenth of
/// that count, so the client runs below a rate that was itself clean. Stages
/// must be ascending; the ramp stops at the first non-clean stage.
pub fn pin(stages: &[(f64, Verdict)]) -> Pin {
    let mut clean = None;
    for (rate, verdict) in stages {
        match verdict {
            Verdict::Clean => clean = Some(*rate),
            Verdict::Throttled { .. } | Verdict::Saturated { .. } => break,
            Verdict::Invalid(why) => return Pin::Invalid(format!("stage at {rate} req/s: {why}")),
        }
    }
    match clean {
        Some(rate) => Pin::Count((rate * 10.0).floor() as u32),
        None if stages.is_empty() => Pin::Invalid("no stages ran".into()),
        None => Pin::RetryLower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN_429: &str =
        r#"{"error":"slow down","code":"rate_limited","retryable":true,"trace_id":"t-429"}"#;

    fn sample(at_ms: u64, latency_ms: u64, reply: Reply) -> Sample {
        Sample {
            path: "/v2/trades",
            finished_at: Duration::from_millis(at_ms),
            latency: Duration::from_millis(latency_ms),
            reply,
        }
    }

    fn stage(target: Option<f64>, secs: u64, samples: Vec<Sample>) -> Stage {
        Stage {
            target_rps: target,
            planned: Duration::from_secs(secs),
            elapsed: Duration::from_secs(secs),
            samples,
            abort: None,
        }
    }

    fn steady(n: u64, latency_ms: u64) -> Vec<Sample> {
        (0..n)
            .map(|i| sample(i * 100, latency_ms, Reply::Ok))
            .collect()
    }

    // ── classify ────────────────────────────────────────────────

    #[test]
    fn a_json_rate_limited_body_is_the_origin() {
        assert_eq!(
            classify(429, None, Some("7"), ORIGIN_429),
            Reply::Throttled {
                layer: Layer::Origin,
                retry_after: Some(Duration::from_secs(7))
            }
        );
    }

    #[test]
    fn a_1015_page_is_cloudflare_even_with_retry_after_zero() {
        assert_eq!(
            classify(429, None, Some("0"), "error code: 1015"),
            Reply::Throttled {
                layer: Layer::Cloudflare,
                retry_after: Some(Duration::ZERO)
            }
        );
    }

    #[test]
    fn an_unrecognised_429_is_still_a_throttle() {
        assert!(matches!(
            classify(429, None, None, "busy"),
            Reply::Throttled {
                layer: Layer::Unknown,
                retry_after: None
            }
        ));
    }

    #[test]
    fn a_cache_hit_is_not_a_success() {
        assert_eq!(
            classify(200, Some("Hit from cloudfront"), None, "{}"),
            Reply::CacheHit
        );
        assert_eq!(
            classify(200, Some("Miss from cloudfront"), None, "{}"),
            Reply::Ok
        );
        assert_eq!(classify(200, None, None, "{}"), Reply::Ok);
        assert_eq!(
            classify(503, Some("Miss from cloudfront"), None, "{}"),
            Reply::Error(503)
        );
    }

    // ── judge ───────────────────────────────────────────────────

    #[test]
    fn a_steady_stage_at_its_target_is_clean() {
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(100, 80)), None),
            Verdict::Clean
        );
    }

    #[test]
    fn the_first_429_decides_the_stage() {
        let mut samples = steady(50, 80);
        samples.push(sample(
            5_100,
            80,
            classify(429, None, Some("3"), ORIGIN_429),
        ));
        samples.push(sample(
            5_200,
            80,
            classify(429, None, None, "error code: 1015"),
        ));
        assert_eq!(
            judge(&stage(Some(10.0), 10, samples), None),
            Verdict::Throttled {
                path: "/v2/trades",
                layer: Layer::Origin,
                retry_after: Some(Duration::from_secs(3)),
                at: Duration::from_millis(5_100)
            }
        );
    }

    #[test]
    fn any_cache_hit_invalidates_the_stage() {
        let mut samples = steady(100, 80);
        samples[40].reply = Reply::CacheHit;
        assert!(matches!(
            judge(&stage(Some(10.0), 10, samples), None),
            Verdict::Invalid(_)
        ));
    }

    #[test]
    fn a_duplicate_url_invalidates_the_stage() {
        let mut s = stage(Some(10.0), 10, steady(100, 80));
        s.abort = Some(Abort::DuplicateUrl);
        assert!(matches!(judge(&s, None), Verdict::Invalid(_)));
    }

    #[test]
    fn one_percent_errors_invalidates_the_stage_and_less_does_not() {
        let mut samples = steady(100, 80);
        samples[0].reply = Reply::Error(400);
        assert!(matches!(
            judge(&stage(Some(10.0), 10, samples.clone()), None),
            Verdict::Invalid(_)
        ));
        samples.extend(steady(100, 80));
        assert_eq!(judge(&stage(Some(10.0), 20, samples), None), Verdict::Clean);
    }

    #[test]
    fn a_failed_stage_names_the_statuses_it_saw() {
        let mut samples = steady(10, 80);
        samples[1].reply = Reply::Error(400);
        samples[2].reply = Reply::Error(0);
        samples[3].reply = Reply::Error(400);
        assert_eq!(
            judge(&stage(Some(1.0), 10, samples), None),
            Verdict::Invalid("3 of 10 requests failed (status transport, 400)".into())
        );
    }

    #[test]
    fn a_harness_that_cannot_reach_its_target_measured_nothing() {
        // 80 requests in 10s is 8 req/s against a 10 req/s target.
        assert!(matches!(
            judge(&stage(Some(10.0), 10, steady(80, 80)), None),
            Verdict::Invalid(_)
        ));
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(90, 80)), None),
            Verdict::Clean
        );
    }

    #[test]
    fn requests_finishing_after_the_deadline_do_not_dilute_the_rate() {
        // Found by the first live smoke run: 8 requests at 1 req/s over an 8s
        // stage, with the last response landing at 8.9s, read as 0.89 req/s.
        let mut s = stage(Some(1.0), 8, steady(8, 250));
        s.elapsed = Duration::from_millis(8_900);
        assert_eq!(judge(&s, None), Verdict::Clean);
    }

    #[test]
    fn a_validation_stage_has_no_target_to_reach() {
        assert_eq!(
            judge(&stage(None, 10, steady(30, 80)), None),
            Verdict::Clean
        );
    }

    #[test]
    fn latency_tripling_over_the_baseline_is_saturation() {
        let baseline = Some(Duration::from_millis(100));
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(100, 300)), baseline),
            Verdict::Clean
        );
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(100, 301)), baseline),
            Verdict::Saturated {
                p99: Duration::from_millis(301),
                baseline_p99: Duration::from_millis(100)
            }
        );
    }

    #[test]
    fn a_stage_cut_short_is_invalid() {
        let mut s = stage(Some(10.0), 10, steady(100, 80));
        s.elapsed = Duration::from_secs(5);
        assert!(matches!(judge(&s, None), Verdict::Invalid(_)));
    }

    // ── pin ─────────────────────────────────────────────────────

    fn throttled() -> Verdict {
        Verdict::Throttled {
            path: "/v2/trades",
            layer: Layer::Origin,
            retry_after: None,
            at: Duration::ZERO,
        }
    }

    #[test]
    fn pins_the_highest_clean_stage_below_the_first_throttle() {
        let ramp = [
            (10.0, Verdict::Clean),
            (15.0, Verdict::Clean),
            (20.0, throttled()),
        ];
        assert_eq!(pin(&ramp), Pin::Count(150));
    }

    #[test]
    fn saturation_stops_the_ramp_like_a_throttle() {
        let saturated = Verdict::Saturated {
            p99: Duration::from_secs(4),
            baseline_p99: Duration::from_secs(1),
        };
        let ramp = [(10.0, Verdict::Clean), (15.0, saturated)];
        assert_eq!(pin(&ramp), Pin::Count(100));
    }

    #[test]
    fn a_ramp_clean_to_the_top_pins_the_top() {
        let ramp = [(10.0, Verdict::Clean), (40.0, Verdict::Clean)];
        assert_eq!(pin(&ramp), Pin::Count(400));
    }

    #[test]
    fn fractional_rates_round_down() {
        assert_eq!(pin(&[(13.5, Verdict::Clean)]), Pin::Count(135));
        assert_eq!(pin(&[(2.55, Verdict::Clean)]), Pin::Count(25));
    }

    #[test]
    fn a_throttled_first_stage_asks_for_lower_stages() {
        assert_eq!(pin(&[(10.0, throttled())]), Pin::RetryLower);
    }

    #[test]
    fn an_invalid_stage_invalidates_the_ramp() {
        let ramp = [
            (10.0, Verdict::Clean),
            (15.0, Verdict::Invalid("cache".into())),
        ];
        assert!(matches!(pin(&ramp), Pin::Invalid(_)));
        assert!(matches!(pin(&[]), Pin::Invalid(_)));
    }
}
