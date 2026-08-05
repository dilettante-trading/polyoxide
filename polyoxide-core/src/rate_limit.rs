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

/// A quota as published by Polymarket: `count` requests per `period`.
///
/// Kept alongside the limiter so tests can assert the configured allowance
/// against the documented table rather than merely checking an entry exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RateSpec {
    count: u32,
    period: Duration,
}

/// One token bucket, shareable between endpoint patterns.
///
/// Sharing is what lets several paths sit under a single cap: upstream limits
/// `/trades`, `/orders`, `/notifications` and `/order` to 900/10s *combined*,
/// which four independent buckets would silently turn into 3,600/10s.
struct Bucket {
    /// The configured allowance. Read only by the agreement tests, which check
    /// it against the published table.
    #[cfg_attr(not(test), allow(dead_code))]
    spec: RateSpec,
    limiter: DirectLimiter,
}

impl Bucket {
    fn new(count: u32, period: Duration) -> Arc<Self> {
        Arc::new(Self {
            spec: RateSpec { count, period },
            limiter: DirectLimiter::direct(quota(count, period)),
        })
    }
}

/// Rate limit configuration for a specific endpoint pattern.
struct EndpointLimit {
    path_prefix: &'static str,
    method: Option<Method>,
    match_mode: MatchMode,
    /// Every bucket a matching request must pass, awaited in order.
    buckets: Vec<Arc<Bucket>>,
}

impl EndpointLimit {
    /// Whether this entry governs the given request.
    ///
    /// Shared by [`RateLimiter::acquire`] and the agreement tests so the two
    /// cannot disagree about which rule applies.
    fn matches(&self, path: &str, method: Option<&Method>) -> bool {
        let path_matches = match self.match_mode {
            MatchMode::Exact => path == self.path_prefix,
            MatchMode::Prefix => {
                // Ensure we're at a segment boundary, not a partial word match.
                // "/price" should match "/price" and "/price/foo" but not "/prices-history".
                match path.strip_prefix(self.path_prefix) {
                    Some(rest) => rest.is_empty() || rest.starts_with('/') || rest.starts_with('?'),
                    None => false,
                }
            }
        };
        if !path_matches {
            return false;
        }
        match &self.method {
            Some(expected) => method == Some(expected),
            None => true,
        }
    }
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

/// Create an endpoint rate limit configuration from its own buckets.
fn endpoint_limit(
    path_prefix: &'static str,
    method: Option<Method>,
    buckets: Vec<Arc<Bucket>>,
) -> EndpointLimit {
    EndpointLimit {
        path_prefix,
        method,
        match_mode: MatchMode::Prefix,
        buckets,
    }
}

/// A single-window endpoint limit: `count` requests per `period`.
fn simple_limit(
    path_prefix: &'static str,
    method: Option<Method>,
    count: u32,
    period: Duration,
) -> EndpointLimit {
    endpoint_limit(path_prefix, method, vec![Bucket::new(count, period)])
}

/// A dual-window endpoint limit: a burst window plus a sustained window.
fn dual_limit(
    path_prefix: &'static str,
    method: Method,
    burst: (u32, Duration),
    sustained: (u32, Duration),
) -> EndpointLimit {
    endpoint_limit(
        path_prefix,
        Some(method),
        vec![
            Bucket::new(burst.0, burst.1),
            Bucket::new(sustained.0, sustained.1),
        ],
    )
}

impl RateLimiter {
    /// Await the appropriate limiter(s) for this endpoint.
    ///
    /// Always awaits the default (general) limiter, then additionally awaits
    /// the first matching endpoint-specific limiter (burst + sustained).
    pub async fn acquire(&self, path: &str, method: Option<&Method>) {
        self.inner.default.until_ready().await;

        if let Some(limit) = self.inner.limits.iter().find(|l| l.matches(path, method)) {
            for bucket in &limit.buckets {
                bucket.limiter.until_ready().await;
            }
        }
    }

    /// The quotas a request would be held to, in the order they are awaited.
    ///
    /// Empty when nothing matches — meaning the request is governed only by the
    /// general bucket, which is the shape every over-permit bug in this table
    /// has taken.
    #[cfg(test)]
    fn resolve_specs(&self, path: &str, method: Option<&Method>) -> Vec<RateSpec> {
        self.inner
            .limits
            .iter()
            .find(|l| l.matches(path, method))
            .map(|l| l.buckets.iter().map(|b| b.spec).collect())
            .unwrap_or_default()
    }

    /// CLOB API rate limits.
    ///
    /// Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
    /// as fetched on 2026-07-25, and pinned by the `documented_limits` tests.
    ///
    /// Two things about the published tables need interpreting:
    ///
    /// - The **ledger group cap** (900/10s across `/trades`, `/orders`,
    ///   `/notifications` and `/order`) is genuinely shared, so those entries
    ///   hold clones of one shared bucket rather than four of their own.
    /// - That group names `/order` and `/orders`, which also appear in the
    ///   trading table at 5,000 and 2,000 per 10s. Both tables can only hold
    ///   simultaneously if the group cap governs the ledger *reads*; a 900/10s
    ///   cap on all methods would make the published trading burst
    ///   unreachable. The group is therefore scoped to `GET`.
    ///
    /// Ordering matters wherever one pattern is a path-segment prefix of
    /// another: `/balance-allowance/update` must precede `/balance-allowance`,
    /// and the specific `/data/*` routes must precede the `/data` catch-all.
    pub fn clob_default() -> Self {
        let ten_sec = Duration::from_secs(10);
        let ten_min = Duration::from_secs(600);
        let get = Some(Method::GET);

        // Shared across the ledger read endpoints — one bucket, four patterns.
        let ledger_group = Bucket::new(900, ten_sec);

        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(9_000, ten_sec)),
                limits: vec![
                    // ── Account. The tighter /update route must come first:
                    // it matches the /balance-allowance prefix at a boundary.
                    simple_limit("/balance-allowance/update", None, 50, ten_sec),
                    simple_limit("/balance-allowance", None, 200, ten_sec),
                    // ── Trading (dual window: burst + sustained).
                    dual_limit("/order", Method::POST, (5_000, ten_sec), (120_000, ten_min)),
                    dual_limit(
                        "/order",
                        Method::DELETE,
                        (5_000, ten_sec),
                        (120_000, ten_min),
                    ),
                    dual_limit("/orders", Method::POST, (2_000, ten_sec), (21_000, ten_min)),
                    dual_limit(
                        "/orders",
                        Method::DELETE,
                        (2_000, ten_sec),
                        (15_000, ten_min),
                    ),
                    dual_limit(
                        "/cancel-all",
                        Method::DELETE,
                        (250, ten_sec),
                        (6_000, ten_min),
                    ),
                    dual_limit(
                        "/cancel-market-orders",
                        Method::DELETE,
                        (1_500, ten_sec),
                        (21_000, ten_min),
                    ),
                    // ── Ledger reads, sharing one 900/10s bucket.
                    // /notifications additionally carries its own 125/10s cap.
                    endpoint_limit(
                        "/notifications",
                        None,
                        vec![ledger_group.clone(), Bucket::new(125, ten_sec)],
                    ),
                    endpoint_limit("/trades", get.clone(), vec![ledger_group.clone()]),
                    endpoint_limit("/orders", get.clone(), vec![ledger_group.clone()]),
                    endpoint_limit("/order", get.clone(), vec![ledger_group]),
                    // Specific /data routes before the catch-all. The previous
                    // pattern here was "/data/", which the segment-boundary
                    // rule can never match — it was dead configuration.
                    simple_limit("/data/orders", None, 500, ten_sec),
                    simple_limit("/data/trades", None, 500, ten_sec),
                    simple_limit("/data", None, 500, ten_sec),
                    // ── Auth (matches /auth/derive-api-key etc.)
                    simple_limit("/auth", None, 100, ten_sec),
                    // ── Market data. The batch forms are 3x tighter than their
                    // singular siblings and do not match them: the boundary
                    // rule means "/books" never resolves through "/book".
                    simple_limit("/prices-history", None, 1_000, ten_sec),
                    simple_limit("/book", None, 1_500, ten_sec),
                    simple_limit("/books", None, 500, ten_sec),
                    simple_limit("/price", None, 1_500, ten_sec),
                    simple_limit("/prices", None, 500, ten_sec),
                    simple_limit("/midpoint", None, 1_500, ten_sec),
                    simple_limit("/midpoints", None, 500, ten_sec),
                    simple_limit("/tick-size", None, 200, ten_sec),
                    // ── Health.
                    simple_limit("/ok", None, 100, ten_sec),
                    // ── Not in the published table. These are local, deliberately
                    // conservative caps kept from earlier revisions; they only
                    // ever permit less than the general bucket would. Listed
                    // last so no documented rule is shadowed by them.
                    simple_limit("/markets", None, 1_500, ten_sec),
                    simple_limit("/neg-risk", None, 1_500, ten_sec),
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
    /// - `/status` (health): 100/10s
    ///
    /// Upstream also lists a 900/10s cap shared by `/markets` + `/events`.
    /// It is not modelled because it can never bind: the per-endpoint caps of
    /// 300 and 500 sum to 800, which is already below it.
    ///
    /// The published table spells the health row `/ok`, but that path answers
    /// **404** on `gamma-api.polymarket.com` — `/status` is the route that
    /// answers 200, and the one `Gamma::health().ping()` requests. `/ok` is
    /// boilerplate repeated into every surface's table; only the CLOB host
    /// serves it.
    pub fn gamma_default() -> Self {
        let ten_sec = Duration::from_secs(10);

        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(4_000, ten_sec)),
                limits: vec![
                    simple_limit("/comments", None, 200, ten_sec),
                    simple_limit("/tags", None, 200, ten_sec),
                    simple_limit("/markets", None, 300, ten_sec),
                    simple_limit("/public-search", None, 350, ten_sec),
                    simple_limit("/events", None, 500, ten_sec),
                    simple_limit("/status", None, 100, ten_sec),
                ],
            }),
        }
    }

    /// Data API rate limits.
    ///
    /// - General: 1,000/10s
    /// - /trades: 200/10s
    /// - /positions and /closed-positions: 150/10s
    /// - `/` (health): 100/10s
    ///
    /// The published table spells the health row `/ok`, but that path answers
    /// **404** on `data-api.polymarket.com` — `/` answers 200 `{"data":"OK"}`,
    /// and is the route this crate requests. `/ok` is boilerplate repeated into
    /// every surface's table; only the CLOB host serves it.
    ///
    /// Matching `/` is safe despite entries being prefix-matched: the
    /// segment-boundary rule means `strip_prefix("/")` on `/positions` leaves
    /// `positions`, which starts with neither `/` nor `?`, so the entry matches
    /// only the bare root and the root with a query string.
    ///
    /// This limiter is shared with the two sibling hosts, so it also carries
    /// their rules:
    ///
    /// - `/user-pnl`: 200/10s, published as the *host-wide* allowance for
    ///   `user-pnl-api.polymarket.com`. Modelled per-path because it is the
    ///   only route polyoxide calls there and matching has no host dimension.
    /// - `lb-api.polymarket.com` (`/volume`, `/profit`) has no published limit,
    ///   so those fall to the general bucket.
    pub fn data_default() -> Self {
        let ten_sec = Duration::from_secs(10);

        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(1_000, ten_sec)),
                limits: vec![
                    simple_limit("/closed-positions", None, 150, ten_sec),
                    simple_limit("/positions", None, 150, ten_sec),
                    simple_limit("/trades", None, 200, ten_sec),
                    simple_limit("/user-pnl", None, 200, ten_sec),
                    simple_limit("/", None, 100, ten_sec),
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
mod agreement {
    //! Shared machinery for the per-surface `documented_*_limits` modules.
    //!
    //! Every API surface pins its published table the same way: assert the
    //! *effective quota* a request resolves to, not merely that some entry
    //! exists. Checking only for presence and ordering is why
    //! `/balance-allowance` could once be absent entirely while every test
    //! passed, and why `/closed-positions` could be set to 66x its published
    //! cap without a single failure.

    use super::*;

    /// One published rule: the request it applies to, and the buckets it must
    /// pass, as `(count, window_secs)` in the order `acquire` awaits them.
    pub type DocumentedRule = (&'static str, Option<Method>, Vec<(u32, u64)>);

    /// Assert every rule resolves to exactly the quota Polymarket publishes.
    ///
    /// `general` is the surface's catch-all allowance. It is only used to make
    /// the failure message name the over-permit factor, since falling through
    /// to the general bucket is the shape every bug in these tables has taken.
    pub fn assert_matches_published(rl: &RateLimiter, rules: Vec<DocumentedRule>, general: u32) {
        for (path, method, expected) in rules {
            let resolved = rl.resolve_specs(path, method.as_ref());
            assert!(
                !resolved.is_empty(),
                "{method:?} {path} matches no endpoint limit — it falls through to the \
                 general {general}/10s bucket, over-permitting by {}x",
                general / expected[0].0.max(1),
            );
            let actual: Vec<(u32, u64)> = resolved
                .iter()
                .map(|s| (s.count, s.period.as_secs()))
                .collect();
            assert_eq!(
                actual, expected,
                "{method:?} {path} resolves to {actual:?}, published limit is {expected:?}"
            );
        }
    }

    /// Assert `path` is governed by nothing but the general bucket.
    ///
    /// Used to pin routes that upstream's table names but the host does not
    /// actually serve, so a dead entry cannot quietly reappear.
    pub fn assert_unconfigured(rl: &RateLimiter, path: &str) {
        assert!(
            rl.resolve_specs(path, Some(&Method::GET)).is_empty(),
            "{path} has an endpoint limit configured, but the host answers 404 there — \
             the entry is dead configuration and the real route is going unlimited"
        );
    }

    /// Drain `count` tokens from `path`, then assert the next call has to wait.
    ///
    /// Matching a spec is not the same as enforcing it; this is the runtime
    /// half of the agreement.
    pub async fn assert_throttles_after(rl: &RateLimiter, path: &str, count: u32) {
        for _ in 0..count {
            rl.acquire(path, Some(&Method::GET)).await;
        }

        let start = std::time::Instant::now();
        rl.acquire(path, Some(&Method::GET)).await;
        let waited = start.elapsed();

        assert!(
            waited >= Duration::from_millis(25),
            "request {} to {path} returned in {waited:?}; the cap is not being enforced",
            count + 1,
        );
    }
}

#[cfg(test)]
mod documented_data_limits {
    //! Agreement tests for the Data API's published table.
    //!
    //! Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
    //! as fetched on 2026-08-05.
    //!
    //! Two rows need interpreting, both verified against the live hosts:
    //!
    //! - The health row is published as `/ok`, but `data-api.polymarket.com/ok`
    //!   answers **404** while `/` answers 200 `{"data":"OK"}`. The `/ok`
    //!   spelling is boilerplate repeated into every surface's table; only
    //!   `clob.polymarket.com` actually serves it. The cap is therefore
    //!   attached to `/`, the route this crate requests and the host answers.
    //! - "User PNL API 200 req/10s" is published as a *host-wide* allowance for
    //!   `user-pnl-api.polymarket.com`. It is modelled as a path rule on
    //!   `/user-pnl` because that is the only route polyoxide calls there and
    //!   the limiter matches on path alone, with no host dimension.

    use super::agreement::*;
    use super::*;

    /// The published table, transcribed by hand. This is the golden vector.
    fn documented() -> Vec<DocumentedRule> {
        vec![
            ("/trades", Some(Method::GET), vec![(200, 10)]),
            ("/positions", Some(Method::GET), vec![(150, 10)]),
            ("/closed-positions", Some(Method::GET), vec![(150, 10)]),
            ("/", Some(Method::GET), vec![(100, 10)]),
            ("/user-pnl", Some(Method::GET), vec![(200, 10)]),
        ]
    }

    #[test]
    fn every_documented_endpoint_resolves_to_its_published_quota() {
        assert_matches_published(&RateLimiter::data_default(), documented(), 1_000);
    }

    #[test]
    fn the_health_cap_is_attached_to_the_route_the_host_answers_on() {
        // `/ok` is a 404 on data-api. An entry there caps nothing and leaves
        // the real health route — `/` — on the 10x-looser general bucket.
        assert_unconfigured(&RateLimiter::data_default(), "/ok");
    }

    #[test]
    fn the_root_health_rule_does_not_swallow_every_other_route() {
        // `/` under prefix matching could plausibly match everything. The
        // segment-boundary rule saves it: `strip_prefix("/")` on `/positions`
        // leaves `positions`, which starts with neither `/` nor `?`.
        let rl = RateLimiter::data_default();
        for (path, expected) in [
            ("/positions", 150),
            ("/closed-positions", 150),
            ("/trades", 200),
            ("/", 100),
        ] {
            let specs = rl.resolve_specs(path, Some(&Method::GET));
            assert_eq!(
                specs[0].count, expected,
                "{path} resolved through the wrong rule — the `/` entry is over-matching"
            );
        }
    }

    #[tokio::test]
    async fn the_closed_positions_cap_actually_throttles() {
        // 150/10s replenishes one token every ~67ms, so the 151st request in a
        // burst cannot return immediately.
        assert_throttles_after(&RateLimiter::data_default(), "/closed-positions", 150).await;
    }

    #[tokio::test]
    async fn closed_positions_and_positions_do_not_share_an_allowance() {
        // Upstream publishes 150/10s for each, not 150/10s combined. Draining
        // one must leave the other untouched — the inverse of the CLOB ledger
        // group, where sharing *is* the published behaviour.
        let rl = RateLimiter::data_default();
        for _ in 0..150 {
            rl.acquire("/closed-positions", Some(&Method::GET)).await;
        }

        let start = std::time::Instant::now();
        rl.acquire("/positions", Some(&Method::GET)).await;
        assert!(
            start.elapsed() < Duration::from_millis(25),
            "/positions was throttled by /closed-positions draining its own bucket"
        );
    }
}

#[cfg(test)]
mod documented_gamma_limits {
    //! Agreement tests for the Gamma API's published table.
    //!
    //! Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
    //! as fetched on 2026-08-05. As with the Data API, the published health row
    //! reads `/ok`, but `gamma-api.polymarket.com/ok` answers 404 — `/status`
    //! is the route that answers 200.

    use super::agreement::*;
    use super::*;

    /// The published table, transcribed by hand. This is the golden vector.
    fn documented() -> Vec<DocumentedRule> {
        vec![
            ("/events", Some(Method::GET), vec![(500, 10)]),
            ("/public-search", Some(Method::GET), vec![(350, 10)]),
            ("/markets", Some(Method::GET), vec![(300, 10)]),
            ("/comments", Some(Method::GET), vec![(200, 10)]),
            ("/tags", Some(Method::GET), vec![(200, 10)]),
            ("/status", Some(Method::GET), vec![(100, 10)]),
        ]
    }

    #[test]
    fn every_documented_endpoint_resolves_to_its_published_quota() {
        assert_matches_published(&RateLimiter::gamma_default(), documented(), 4_000);
    }

    #[test]
    fn the_health_cap_is_attached_to_the_route_the_host_answers_on() {
        assert_unconfigured(&RateLimiter::gamma_default(), "/ok");
    }

    #[test]
    fn the_markets_plus_events_group_cap_can_never_bind() {
        // Upstream also publishes a 900/10s cap shared by /markets + /events.
        // It is deliberately not modelled because the per-endpoint caps sum to
        // less than it. If either cap is ever raised, this stops being true and
        // the group bucket has to be added — that is what this test watches.
        let rl = RateLimiter::gamma_default();
        let markets = rl.resolve_specs("/markets", Some(&Method::GET))[0].count;
        let events = rl.resolve_specs("/events", Some(&Method::GET))[0].count;
        assert!(
            markets + events <= 900,
            "/markets ({markets}) + /events ({events}) now exceeds the published 900/10s \
             group cap, which is no longer unreachable and must be modelled"
        );
    }

    #[tokio::test]
    async fn the_markets_cap_actually_throttles() {
        assert_throttles_after(&RateLimiter::gamma_default(), "/markets", 300).await;
    }
}

#[cfg(test)]
mod documented_limits {
    //! Table-driven agreement tests against Polymarket's published limits.
    //!
    //! Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
    //! as fetched on 2026-07-25, re-confirmed 2026-08-05. These assert the
    //! *effective quota* a request resolves to, not merely that some entry
    //! exists — the previous tests only checked that entries were present and
    //! in the right order, which is why `/balance-allowance` could be absent
    //! entirely while every test passed.

    use super::agreement::DocumentedRule;
    use super::*;

    /// The published table, transcribed by hand. This is the golden vector.
    fn documented() -> Vec<DocumentedRule> {
        vec![
            // ── Account ──
            ("/balance-allowance", Some(Method::GET), vec![(200, 10)]),
            (
                "/balance-allowance/update",
                Some(Method::GET),
                vec![(50, 10)],
            ),
            // ── Trading (dual window) ──
            (
                "/order",
                Some(Method::POST),
                vec![(5_000, 10), (120_000, 600)],
            ),
            (
                "/order",
                Some(Method::DELETE),
                vec![(5_000, 10), (120_000, 600)],
            ),
            (
                "/orders",
                Some(Method::POST),
                vec![(2_000, 10), (21_000, 600)],
            ),
            (
                "/orders",
                Some(Method::DELETE),
                vec![(2_000, 10), (15_000, 600)],
            ),
            (
                "/cancel-all",
                Some(Method::DELETE),
                vec![(250, 10), (6_000, 600)],
            ),
            (
                "/cancel-market-orders",
                Some(Method::DELETE),
                vec![(1_500, 10), (21_000, 600)],
            ),
            // ── Ledger: a cap shared across the group, plus per-endpoint caps ──
            ("/trades", Some(Method::GET), vec![(900, 10)]),
            ("/orders", Some(Method::GET), vec![(900, 10)]),
            ("/order", Some(Method::GET), vec![(900, 10)]),
            (
                "/notifications",
                Some(Method::GET),
                vec![(900, 10), (125, 10)],
            ),
            ("/data/orders", Some(Method::GET), vec![(500, 10)]),
            ("/data/trades", Some(Method::GET), vec![(500, 10)]),
            // ── Market data ──
            ("/book", Some(Method::GET), vec![(1_500, 10)]),
            ("/books", Some(Method::POST), vec![(500, 10)]),
            ("/price", Some(Method::GET), vec![(1_500, 10)]),
            ("/prices", Some(Method::POST), vec![(500, 10)]),
            ("/midpoint", Some(Method::GET), vec![(1_500, 10)]),
            ("/midpoints", Some(Method::POST), vec![(500, 10)]),
            ("/prices-history", Some(Method::GET), vec![(1_000, 10)]),
            ("/tick-size", Some(Method::GET), vec![(200, 10)]),
            // ── Auth & health ──
            ("/auth/api-key", Some(Method::POST), vec![(100, 10)]),
            ("/ok", Some(Method::GET), vec![(100, 10)]),
        ]
    }

    #[test]
    fn every_documented_endpoint_resolves_to_its_published_quota() {
        let rl = RateLimiter::clob_default();

        for (path, method, expected) in documented() {
            let resolved = rl.resolve_specs(path, method.as_ref());
            assert!(
                !resolved.is_empty(),
                "{method:?} {path} matches no endpoint limit — it falls through to the \
                 general {}/10s bucket, over-permitting by {}x",
                9_000,
                9_000 / expected[0].0.max(1),
            );
            let actual: Vec<(u32, u64)> = resolved
                .iter()
                .map(|s| (s.count, s.period.as_secs()))
                .collect();
            assert_eq!(
                actual, expected,
                "{method:?} {path} resolves to {actual:?}, published limit is {expected:?}"
            );
        }
    }

    #[test]
    fn batch_endpoints_do_not_inherit_their_singular_sibling() {
        // `/books` must not resolve through the `/book` rule: they are
        // different endpoints with a 3x difference in allowance.
        let rl = RateLimiter::clob_default();
        for (batch, singular) in [
            ("/books", "/book"),
            ("/prices", "/price"),
            ("/midpoints", "/midpoint"),
        ] {
            let batch_specs = rl.resolve_specs(batch, Some(&Method::POST));
            let singular_specs = rl.resolve_specs(singular, Some(&Method::GET));
            assert_ne!(
                batch_specs, singular_specs,
                "{batch} is being limited as if it were {singular}"
            );
            assert_eq!(batch_specs[0].count, 500, "{batch} should allow 500/10s");
        }
    }

    #[test]
    fn the_ledger_group_cap_is_one_shared_bucket() {
        // Upstream caps `/trades`, `/orders`, `/notifications` and `/order`
        // at 900/10s *combined*. Modelling that as four independent 900/10s
        // buckets would permit 3,600/10s.
        let rl = RateLimiter::clob_default();
        let group: Vec<_> = ["/trades", "/orders", "/order", "/notifications"]
            .iter()
            .map(|p| {
                rl.inner
                    .limits
                    .iter()
                    .find(|l| l.matches(p, Some(&Method::GET)))
                    .unwrap_or_else(|| panic!("{p} should match a ledger entry"))
                    .buckets[0]
                    .clone()
            })
            .collect();

        for other in &group[1..] {
            assert!(
                Arc::ptr_eq(&group[0], other),
                "ledger endpoints must share one bucket, not hold copies"
            );
        }
    }

    #[test]
    fn balance_allowance_update_is_not_shadowed_by_its_parent_path() {
        // `/balance-allowance/update` starts with `/balance-allowance` at a
        // segment boundary, so ordering decides which rule wins. The update
        // route is four times tighter.
        let rl = RateLimiter::clob_default();
        let update = rl.resolve_specs("/balance-allowance/update", Some(&Method::GET));
        assert_eq!(
            update[0].count, 50,
            "the tighter /balance-allowance/update rule must be ordered first"
        );
    }

    #[tokio::test]
    async fn a_documented_cap_actually_throttles() {
        // Matching specs is not the same as enforcing them. `/tick-size` is
        // 200/10s; the 201st request in a burst must wait for a token
        // (200 per 10s replenishes one every ~50ms).
        let rl = RateLimiter::clob_default();
        for _ in 0..200 {
            rl.acquire("/tick-size", Some(&Method::GET)).await;
        }

        let start = std::time::Instant::now();
        rl.acquire("/tick-size", Some(&Method::GET)).await;
        let waited = start.elapsed();

        assert!(
            waited >= Duration::from_millis(25),
            "201st /tick-size request returned in {waited:?}; the cap is not being enforced"
        );
    }

    #[tokio::test]
    async fn the_ledger_group_allowance_is_consumed_jointly() {
        // The runtime counterpart to the Arc::ptr_eq check: draining the group
        // through one endpoint must leave a *different* group member throttled.
        // With four independent buckets this would return instantly.
        let rl = RateLimiter::clob_default();
        for _ in 0..900 {
            rl.acquire("/trades", Some(&Method::GET)).await;
        }

        let start = std::time::Instant::now();
        rl.acquire("/orders", Some(&Method::GET)).await;
        let waited = start.elapsed();

        assert!(
            waited >= Duration::from_millis(5),
            "GET /orders returned in {waited:?} after /trades drained the shared 900/10s \
             allowance — the group cap is not actually shared"
        );
    }

    #[test]
    fn post_order_is_not_throttled_by_the_ledger_group() {
        // The ledger group names `/order`, but the trading table allows POST
        // /order 5,000/10s. Both can only hold if the group cap is the ledger
        // *read*. Applying it to POST would make the published burst
        // unreachable.
        let rl = RateLimiter::clob_default();
        let specs = rl.resolve_specs("/order", Some(&Method::POST));
        assert_eq!(specs[0].count, 5_000);
        assert!(
            !specs.iter().any(|s| s.count == 900),
            "POST /order must not be caught by the ledger read cap"
        );
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
        assert_eq!(rl.inner.limits.len(), 27);
        assert!(format!("{:?}", rl).contains("endpoints"));
    }

    #[test]
    fn test_gamma_default_construction() {
        let rl = RateLimiter::gamma_default();
        assert_eq!(rl.inner.limits.len(), 6);
    }

    #[test]
    fn test_data_default_construction() {
        let rl = RateLimiter::data_default();
        assert_eq!(rl.inner.limits.len(), 5);
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
        assert!(dbg.contains("endpoints: 27"), "missing count: {dbg}");
    }

    // ── Endpoint matching internals ──────────────────────────────

    #[test]
    fn test_clob_tighter_rules_precede_the_prefixes_that_would_shadow_them() {
        // Ordering is only load-bearing where one pattern is a path-segment
        // prefix of another. Asserting on fixed indices made this test brittle
        // and told us nothing; assert the actual constraint instead.
        let rl = RateLimiter::clob_default();
        let index_of = |path: &str| {
            rl.inner
                .limits
                .iter()
                .position(|l| l.path_prefix == path)
                .unwrap_or_else(|| panic!("{path} should be configured"))
        };

        for (specific, general) in [
            ("/balance-allowance/update", "/balance-allowance"),
            ("/data/orders", "/data"),
            ("/data/trades", "/data"),
        ] {
            assert!(
                index_of(specific) < index_of(general),
                "{specific} must be matched before {general} or it can never win"
            );
        }
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
        // This previously asserted `!"/closed-positions".starts_with("/positions")`
        // — a tautology about two string literals that never touched the
        // limiter, and so held even with `/closed-positions` set to 66x its
        // published cap. Ask the limiter instead.
        let rl = RateLimiter::data_default();

        let closed = rl.resolve_specs("/closed-positions", Some(&Method::GET));
        let positions = rl.resolve_specs("/positions", Some(&Method::GET));
        assert_eq!(closed, positions, "both are published at 150/10s");

        let bucket_for = |path: &str| {
            rl.inner
                .limits
                .iter()
                .find(|l| l.matches(path, Some(&Method::GET)))
                .unwrap_or_else(|| panic!("{path} should match a rule"))
                .buckets[0]
                .clone()
        };
        assert!(
            !Arc::ptr_eq(&bucket_for("/closed-positions"), &bucket_for("/positions")),
            "equal quotas must still be separate buckets — upstream publishes \
             150/10s each, not 150/10s combined"
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

        assert_eq!(
            post_order.buckets.len(),
            2,
            "POST /order should have a burst and a sustained window"
        );
    }

    #[test]
    fn test_clob_delete_order_has_a_sustained_window_too() {
        // This previously asserted the *opposite* — that DELETE /order had only
        // a burst window — and so pinned the omission in place. Upstream
        // publishes 5,000/10s burst plus 120,000/10min sustained.
        let rl = RateLimiter::clob_default();
        let delete_order = rl
            .inner
            .limits
            .iter()
            .find(|l| l.path_prefix == "/order" && l.method == Some(Method::DELETE))
            .expect("DELETE /order endpoint should exist");

        assert_eq!(
            delete_order.buckets.len(),
            2,
            "DELETE /order should have both a burst and a sustained window"
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
