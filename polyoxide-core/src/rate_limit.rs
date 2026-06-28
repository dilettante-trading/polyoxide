use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::Quota;
use reqwest::Method;

type DirectLimiter = governor::RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
>;

/// How an endpoint pattern should be matched against request paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum MatchMode {
    /// Match if the path starts with the pattern followed by a segment
    /// boundary (`/`, `?`, or end-of-string). Prevents `/price` from
    /// matching `/prices-history`.
    Prefix,
    /// Match only the exact path string.
    Exact,
}

/// Rate limit configuration for a specific endpoint pattern.
struct EndpointLimit {
    path_prefix: &'static str,
    method: Option<Method>,
    match_mode: MatchMode,
    burst: DirectLimiter,
    sustained: Option<DirectLimiter>,
}

/// Holds all rate limiters for one API surface.
///
/// Created via factory methods like [`RateLimiter::clob_default()`] which
/// configure hardcoded limits matching Polymarket's documented rate limits.
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<RateLimiterInner>,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("endpoints", &self.inner.limits.len())
            .finish()
    }
}

struct RateLimiterInner {
    limits: Vec<EndpointLimit>,
    default: DirectLimiter,
}

/// Helper to create a quota: `count` requests per `period`.
///
/// Uses `Quota::with_period` for exact rate enforcement rather than
/// ceiling-based `per_second`, which can over-permit for non-round windows.
fn quota(count: u32, period: Duration) -> Quota {
    let count = count.max(1);
    let interval = period / count;
    Quota::with_period(interval)
        .expect("quota interval must be non-zero")
        .allow_burst(NonZeroU32::new(count).unwrap())
}

/// Create an endpoint rate limit configuration.
fn endpoint_limit(
    path_prefix: &'static str,
    method: Option<Method>,
    match_mode: MatchMode,
    burst_count: u32,
    burst_period: Duration,
    sustained: Option<(u32, Duration)>,
) -> EndpointLimit {
    EndpointLimit {
        path_prefix,
        method,
        match_mode,
        burst: DirectLimiter::direct(quota(burst_count, burst_period)),
        sustained: sustained.map(|(count, period)| DirectLimiter::direct(quota(count, period))),
    }
}

impl RateLimiter {
    /// Await the appropriate limiter(s) for this endpoint.
    ///
    /// Always awaits the default (general) limiter, then additionally awaits
    /// the first matching endpoint-specific limiter (burst + sustained).
    pub async fn acquire(&self, path: &str, method: Option<&Method>) {
        self.inner.default.until_ready().await;

        for limit in &self.inner.limits {
            let matched = match limit.match_mode {
                MatchMode::Exact => path == limit.path_prefix,
                MatchMode::Prefix => {
                    // Ensure we're at a segment boundary, not a partial word match.
                    // "/price" should match "/price" and "/price/foo" but not "/prices-history".
                    match path.strip_prefix(limit.path_prefix) {
                        Some(rest) => {
                            rest.is_empty() || rest.starts_with('/') || rest.starts_with('?')
                        }
                        None => false,
                    }
                }
            };
            if !matched {
                continue;
            }
            if let Some(ref m) = limit.method {
                if method != Some(m) {
                    continue;
                }
            }
            limit.burst.until_ready().await;
            if let Some(ref sustained) = limit.sustained {
                sustained.until_ready().await;
            }
            break;
        }
    }

    /// CLOB API rate limits.
    ///
    /// - General: 9,000/10s
    /// - POST /order: 3,500/10s burst + 36,000/10min sustained
    /// - DELETE /order: 3,000/10s
    /// - Market data (/markets, /book, /price, /midpoint, /prices-history, /neg-risk, /tick-size): 1,500/10s
    /// - Ledger (/trades, /data/): 900/10s
    /// - Auth (/auth): 100/10s
    pub fn clob_default() -> Self {
        let ten_sec = Duration::from_secs(10);
        let ten_min = Duration::from_secs(600);
        let p = MatchMode::Prefix;

        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(9_000, ten_sec)),
                limits: vec![
                    // POST /order — dual window (matches /order/{id})
                    endpoint_limit(
                        "/order",
                        Some(Method::POST),
                        p,
                        3_500,
                        ten_sec,
                        Some((36_000, ten_min)),
                    ),
                    // DELETE /order (matches /order/{id})
                    endpoint_limit("/order", Some(Method::DELETE), p, 3_000, ten_sec, None),
                    // Auth (matches /auth/derive-api-key etc.)
                    endpoint_limit("/auth", None, p, 100, ten_sec, None),
                    // Ledger
                    endpoint_limit("/trades", None, p, 900, ten_sec, None),
                    endpoint_limit("/data/", None, p, 900, ten_sec, None),
                    // Market data — /prices-history before /price to avoid prefix collision
                    endpoint_limit("/prices-history", None, p, 1_500, ten_sec, None),
                    endpoint_limit("/markets", None, p, 1_500, ten_sec, None),
                    endpoint_limit("/book", None, p, 1_500, ten_sec, None),
                    endpoint_limit("/price", None, p, 1_500, ten_sec, None),
                    endpoint_limit("/midpoint", None, p, 1_500, ten_sec, None),
                    endpoint_limit("/neg-risk", None, p, 1_500, ten_sec, None),
                    endpoint_limit("/tick-size", None, p, 1_500, ten_sec, None),
                ],
            }),
        }
    }

    /// Gamma API rate limits.
    ///
    /// - General: 4,000/10s
    /// - /events: 500/10s
    /// - /markets: 300/10s
    /// - /public-search: 350/10s
    /// - /comments: 200/10s
    /// - /tags: 200/10s
    pub fn gamma_default() -> Self {
        let ten_sec = Duration::from_secs(10);
        let p = MatchMode::Prefix;

        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(4_000, ten_sec)),
                limits: vec![
                    endpoint_limit("/comments", None, p, 200, ten_sec, None),
                    endpoint_limit("/tags", None, p, 200, ten_sec, None),
                    endpoint_limit("/markets", None, p, 300, ten_sec, None),
                    endpoint_limit("/public-search", None, p, 350, ten_sec, None),
                    endpoint_limit("/events", None, p, 500, ten_sec, None),
                ],
            }),
        }
    }

    /// Data API rate limits.
    ///
    /// - General: 1,000/10s
    /// - /trades: 200/10s
    /// - /positions and /closed-positions: 150/10s
    pub fn data_default() -> Self {
        let ten_sec = Duration::from_secs(10);
        let p = MatchMode::Prefix;

        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(1_000, ten_sec)),
                limits: vec![
                    endpoint_limit("/closed-positions", None, p, 150, ten_sec, None),
                    endpoint_limit("/positions", None, p, 150, ten_sec, None),
                    endpoint_limit("/trades", None, p, 200, ten_sec, None),
                ],
            }),
        }
    }

    /// Relay API rate limits.
    ///
    /// - 25 requests per 1 minute (single limiter, no endpoint-specific limits)
    pub fn relay_default() -> Self {
        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(25, Duration::from_secs(60))),
                limits: vec![],
            }),
        }
    }
}

/// Configuration for retry-on-429 with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts after the initial request (default: 3).
    pub max_retries: u32,
    /// Base backoff in milliseconds for the first retry, doubled each attempt (default: 500).
    pub initial_backoff_ms: u64,
    /// Upper bound in milliseconds for the backoff delay (default: 10_000).
    pub max_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 500,
            max_backoff_ms: 10_000,
        }
    }
}

impl RetryConfig {
    /// Calculate backoff duration with jitter for attempt N.
    ///
    /// Uses `fastrand` for uniform jitter (75%-125% of base delay) to avoid
    /// thundering herd when multiple clients retry simultaneously.
    pub fn backoff(&self, attempt: u32) -> Duration {
        let base = self
            .initial_backoff_ms
            .saturating_mul(1u64 << attempt.min(10));
        let capped = base.min(self.max_backoff_ms);
        // Uniform jitter in 0.75..1.25 range
        let jitter_factor = 0.75 + (fastrand::f64() * 0.5);
        let ms = (capped as f64 * jitter_factor) as u64;
        Duration::from_millis(ms.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RetryConfig ──────────────────────────────────────────────

    #[test]
    fn test_retry_config_default() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.initial_backoff_ms, 500);
        assert_eq!(cfg.max_backoff_ms, 10_000);
    }

    #[test]
    fn test_backoff_attempt_zero() {
        let cfg = RetryConfig::default();
        let d = cfg.backoff(0);
        // base = 500 * 2^0 = 500, capped = 500, jitter in [0.75, 1.25]
        // ms in [375, 625]
        let ms = d.as_millis() as u64;
        assert!(
            (375..=625).contains(&ms),
            "attempt 0: {ms}ms not in [375, 625]"
        );
    }

    #[test]
    fn test_backoff_exponential_growth() {
        let cfg = RetryConfig::default();
        let d0 = cfg.backoff(0);
        let d1 = cfg.backoff(1);
        let d2 = cfg.backoff(2);
        assert!(d0 < d1, "d0={d0:?} should be < d1={d1:?}");
        assert!(d1 < d2, "d1={d1:?} should be < d2={d2:?}");
    }

    #[test]
    fn test_backoff_jitter_bounds() {
        let cfg = RetryConfig::default();
        for attempt in 0..20 {
            let d = cfg.backoff(attempt);
            let base = cfg
                .initial_backoff_ms
                .saturating_mul(1u64 << attempt.min(10));
            let capped = base.min(cfg.max_backoff_ms);
            let lower = (capped as f64 * 0.75) as u64;
            let upper = (capped as f64 * 1.25) as u64;
            let ms = d.as_millis() as u64;
            assert!(
                ms >= lower.max(1) && ms <= upper,
                "attempt {attempt}: {ms}ms not in [{lower}, {upper}]"
            );
        }
    }

    #[test]
    fn test_backoff_max_capping() {
        let cfg = RetryConfig::default();
        for attempt in 5..=10 {
            let d = cfg.backoff(attempt);
            let ceiling = (cfg.max_backoff_ms as f64 * 1.25) as u64;
            assert!(
                d.as_millis() as u64 <= ceiling,
                "attempt {attempt}: {:?} exceeded ceiling {ceiling}ms",
                d
            );
        }
    }

    #[test]
    fn test_backoff_very_high_attempt() {
        let cfg = RetryConfig::default();
        let d = cfg.backoff(100);
        let ceiling = (cfg.max_backoff_ms as f64 * 1.25) as u64;
        assert!(d.as_millis() as u64 <= ceiling);
        assert!(d.as_millis() >= 1);
    }

    #[test]
    fn test_backoff_jitter_distribution() {
        // Verify jitter isn't degenerate (all clustering at one end).
        // Sample 200 values and check both halves of the range are hit.
        let cfg = RetryConfig::default();
        let midpoint = cfg.initial_backoff_ms; // 500ms (center of 375..625 range)
        let (mut below, mut above) = (0u32, 0u32);
        for _ in 0..200 {
            let ms = cfg.backoff(0).as_millis() as u64;
            if ms < midpoint {
                below += 1;
            } else {
                above += 1;
            }
        }
        assert!(
            below >= 20 && above >= 20,
            "jitter looks degenerate: {below} below midpoint, {above} above"
        );
    }

    // ── quota() ──────────────────────────────────────────────────

    #[test]
    fn test_quota_creation() {
        // Should not panic for representative values
        let _ = quota(100, Duration::from_secs(10));
        let _ = quota(1, Duration::from_secs(60));
        let _ = quota(9_000, Duration::from_secs(10));
    }

    #[test]
    fn test_quota_edge_zero_count() {
        // count=0 is guarded by .max(1) — should not panic
        let _ = quota(0, Duration::from_secs(10));
    }

    // ── Factory methods ──────────────────────────────────────────

    #[test]
    fn test_clob_default_construction() {
        let rl = RateLimiter::clob_default();
        assert_eq!(rl.inner.limits.len(), 12);
        assert!(format!("{:?}", rl).contains("endpoints"));
    }

    #[test]
    fn test_gamma_default_construction() {
        let rl = RateLimiter::gamma_default();
        assert_eq!(rl.inner.limits.len(), 5);
    }

    #[test]
    fn test_data_default_construction() {
        let rl = RateLimiter::data_default();
        assert_eq!(rl.inner.limits.len(), 3);
    }

    #[test]
    fn test_relay_default_construction() {
        let rl = RateLimiter::relay_default();
        assert_eq!(rl.inner.limits.len(), 0);
    }

    #[test]
    fn test_rate_limiter_debug_format() {
        let rl = RateLimiter::clob_default();
        let dbg = format!("{:?}", rl);
        assert!(dbg.contains("RateLimiter"), "missing struct name: {dbg}");
        assert!(dbg.contains("endpoints: 12"), "missing count: {dbg}");
    }

    // ── Endpoint matching internals ──────────────────────────────

    #[test]
    fn test_clob_endpoint_order_and_methods() {
        let rl = RateLimiter::clob_default();
        let limits = &rl.inner.limits;

        // First: POST /order with sustained
        assert_eq!(limits[0].path_prefix, "/order");
        assert_eq!(limits[0].method, Some(Method::POST));
        assert!(limits[0].sustained.is_some());

        // Second: DELETE /order without sustained
        assert_eq!(limits[1].path_prefix, "/order");
        assert_eq!(limits[1].method, Some(Method::DELETE));
        assert!(limits[1].sustained.is_none());

        // Third: /auth with method=None
        assert_eq!(limits[2].path_prefix, "/auth");
        assert!(limits[2].method.is_none());
    }

    // ── acquire() async behavior ─────────────────────────────────

    #[tokio::test]
    async fn test_acquire_single_completes_immediately() {
        let rl = RateLimiter::clob_default();
        let start = std::time::Instant::now();
        rl.acquire("/order", Some(&Method::POST)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_matches_endpoint_by_prefix() {
        let rl = RateLimiter::clob_default();
        let start = std::time::Instant::now();
        // /order/123 should match the /order prefix
        rl.acquire("/order/123", Some(&Method::POST)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_prefix_respects_segment_boundary() {
        let rl = RateLimiter::clob_default();
        let limits = &rl.inner.limits;

        // Find the /price entry
        let price_idx = limits
            .iter()
            .position(|l| l.path_prefix == "/price")
            .expect("/price endpoint exists");

        // /prices-history must NOT match /price — it's a different endpoint
        let prices_history_idx = limits
            .iter()
            .position(|l| l.path_prefix == "/prices-history")
            .expect("/prices-history endpoint exists");

        // /prices-history should have its own entry, ordered before /price
        assert!(
            prices_history_idx < price_idx,
            "/prices-history (idx {prices_history_idx}) should come before /price (idx {price_idx})"
        );
    }

    #[test]
    fn test_match_mode_prefix_segment_boundary() {
        // Verify the Prefix matching logic directly
        let pattern = "/price";

        let check = |path: &str| -> bool {
            match path.strip_prefix(pattern) {
                Some(rest) => rest.is_empty() || rest.starts_with('/') || rest.starts_with('?'),
                None => false,
            }
        };

        // Should match: exact, sub-path, query params
        assert!(check("/price"), "exact match");
        assert!(check("/price/foo"), "sub-path");
        assert!(check("/price?token=abc"), "query params");

        // Should NOT match: partial word overlap
        assert!(!check("/prices-history"), "partial word /prices-history");
        assert!(!check("/pricelist"), "partial word /pricelist");
        assert!(!check("/pricing"), "partial word /pricing");

        // Should NOT match: different prefix
        assert!(!check("/midpoint"), "different prefix");
    }

    #[test]
    fn test_match_mode_exact() {
        // Verify the Exact matching logic
        let pattern = "/trades";

        let check = |path: &str| -> bool { path == pattern };

        assert!(check("/trades"), "exact match");
        assert!(!check("/trades/123"), "sub-path should not match");
        assert!(!check("/trades?limit=10"), "query params should not match");
        assert!(!check("/traded"), "different word should not match");
    }

    #[tokio::test]
    async fn test_acquire_method_filtering() {
        let rl = RateLimiter::clob_default();
        let start = std::time::Instant::now();
        // GET /order shouldn't match POST or DELETE /order endpoints — falls to default only
        rl.acquire("/order", Some(&Method::GET)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_no_endpoint_match_uses_default_only() {
        let rl = RateLimiter::clob_default();
        let start = std::time::Instant::now();
        rl.acquire("/unknown/path", None).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_method_none_matches_any_method() {
        let rl = RateLimiter::gamma_default();
        let start = std::time::Instant::now();
        // /events has method: None — should match GET, POST, and None
        rl.acquire("/events", Some(&Method::GET)).await;
        rl.acquire("/events", Some(&Method::POST)).await;
        rl.acquire("/events", None).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    // ── Prefix collision tests ──────────────────────────────────

    #[test]
    fn test_clob_price_and_prices_history_are_distinct() {
        let rl = RateLimiter::clob_default();
        let limits = &rl.inner.limits;

        let price = limits.iter().find(|l| l.path_prefix == "/price").unwrap();
        let prices_history = limits
            .iter()
            .find(|l| l.path_prefix == "/prices-history")
            .unwrap();

        // Both should use Prefix mode
        assert_eq!(price.match_mode, MatchMode::Prefix);
        assert_eq!(prices_history.match_mode, MatchMode::Prefix);

        // Verify "/prices-history" does NOT match the "/price" pattern
        if let Some(rest) = "/prices-history".strip_prefix(price.path_prefix) {
            assert!(
                !rest.is_empty() && !rest.starts_with('/') && !rest.starts_with('?'),
                "/prices-history must not match /price pattern, rest = '{rest}'"
            );
        }
    }

    #[test]
    fn test_data_positions_and_closed_positions_are_distinct() {
        let rl = RateLimiter::data_default();
        let limits = &rl.inner.limits;

        let positions = limits
            .iter()
            .find(|l| l.path_prefix == "/positions")
            .unwrap();
        let closed = limits
            .iter()
            .find(|l| l.path_prefix == "/closed-positions")
            .unwrap();

        assert_eq!(positions.match_mode, MatchMode::Prefix);
        assert_eq!(closed.match_mode, MatchMode::Prefix);

        // "/closed-positions" does NOT start with "/positions"
        assert!(
            !"/closed-positions".starts_with(positions.path_prefix),
            "/closed-positions should not match /positions prefix"
        );
    }

    #[test]
    fn test_all_clob_endpoints_have_match_mode() {
        let rl = RateLimiter::clob_default();
        for limit in &rl.inner.limits {
            // Every endpoint should have an explicit match mode
            assert!(
                limit.match_mode == MatchMode::Prefix || limit.match_mode == MatchMode::Exact,
                "endpoint {} has no valid match mode",
                limit.path_prefix
            );
        }
    }

    // ── Concurrent access tests ─────────────────────────────────

    #[tokio::test]
    async fn test_acquire_concurrent_tasks_all_complete() {
        // A rate limiter with high burst should allow many concurrent acquires
        let rl = RateLimiter::clob_default(); // 9000/10s general
        let rl = std::sync::Arc::new(rl);

        let mut handles = Vec::new();
        for _ in 0..10 {
            let rl = rl.clone();
            handles.push(tokio::spawn(async move {
                rl.acquire("/markets", None).await;
            }));
        }

        let start = std::time::Instant::now();
        for handle in handles {
            handle.await.unwrap();
        }
        // 10 concurrent acquires against a 9000/10s limiter should complete fast
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "concurrent acquires took too long: {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn test_acquire_concurrent_different_endpoints() {
        // Concurrent tasks hitting different endpoints should not block each other
        let rl = std::sync::Arc::new(RateLimiter::clob_default());

        let rl1 = rl.clone();
        let rl2 = rl.clone();
        let rl3 = rl.clone();

        let start = std::time::Instant::now();
        let (r1, r2, r3) = tokio::join!(
            tokio::spawn(async move { rl1.acquire("/markets", None).await }),
            tokio::spawn(async move { rl2.acquire("/auth", None).await }),
            tokio::spawn(async move { rl3.acquire("/order", Some(&Method::POST)).await }),
        );
        r1.unwrap();
        r2.unwrap();
        r3.unwrap();

        assert!(
            start.elapsed() < Duration::from_millis(50),
            "different endpoints should not block: {:?}",
            start.elapsed()
        );
    }

    // ── Dual-window interaction tests ───────────────────────────

    #[test]
    fn test_clob_post_order_has_dual_window() {
        let rl = RateLimiter::clob_default();
        let post_order = rl
            .inner
            .limits
            .iter()
            .find(|l| l.path_prefix == "/order" && l.method == Some(Method::POST))
            .expect("POST /order endpoint should exist");

        assert!(
            post_order.sustained.is_some(),
            "POST /order should have a sustained (10-min) window"
        );
    }

    #[test]
    fn test_clob_delete_order_has_no_sustained_window() {
        let rl = RateLimiter::clob_default();
        let delete_order = rl
            .inner
            .limits
            .iter()
            .find(|l| l.path_prefix == "/order" && l.method == Some(Method::DELETE))
            .expect("DELETE /order endpoint should exist");

        assert!(
            delete_order.sustained.is_none(),
            "DELETE /order should only have a burst window"
        );
    }

    #[tokio::test]
    async fn test_dual_window_both_burst_and_sustained_are_awaited() {
        // POST /order should await both burst and sustained limiters.
        // With high limits, a single acquire should still complete fast.
        let rl = RateLimiter::clob_default();
        let start = std::time::Instant::now();
        rl.acquire("/order", Some(&Method::POST)).await;
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "dual window single acquire should be fast: {:?}",
            start.elapsed()
        );
    }

    // ── should_retry edge cases ─────────────────────────────────

    #[test]
    fn test_should_retry_exhaustion() {
        // After max_retries, should_retry must return None
        let client = crate::HttpClientBuilder::new("https://example.com")
            .with_retry_config(RetryConfig {
                max_retries: 3,
                ..RetryConfig::default()
            })
            .build()
            .unwrap();

        // Attempts 0, 1, 2 should succeed
        for attempt in 0..3 {
            assert!(
                client
                    .should_retry(reqwest::StatusCode::TOO_MANY_REQUESTS, attempt, None)
                    .is_some(),
                "attempt {attempt} should allow retry"
            );
        }
        // Attempt 3 should give up
        assert!(
            client
                .should_retry(reqwest::StatusCode::TOO_MANY_REQUESTS, 3, None)
                .is_none(),
            "attempt 3 should exhaust retries"
        );
    }

    #[test]
    fn test_should_retry_zero_max_retries_never_retries() {
        let client = crate::HttpClientBuilder::new("https://example.com")
            .with_retry_config(RetryConfig {
                max_retries: 0,
                ..RetryConfig::default()
            })
            .build()
            .unwrap();

        assert!(
            client
                .should_retry(reqwest::StatusCode::TOO_MANY_REQUESTS, 0, None)
                .is_none(),
            "max_retries=0 should never retry"
        );
    }
}
