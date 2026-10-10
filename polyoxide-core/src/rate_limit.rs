//! Window quotas: [`WindowQuotaTable`] builds a [`RateLimiter`], which counts
//! requests against the buckets a request's method and path resolve to.
//!
//! Polymarket's tables are built in [`polymarket`](crate::polymarket).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::hold::Hold;
use crate::hooks::{AttemptInfo, Charge, Refused, RequestMeta, ResponseMeta, Throttle};
use governor::Quota;
use reqwest::Method;

type DirectLimiter = governor::RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
>;

/// How a row's pattern is matched against a request path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matching {
    /// The path starts with the pattern at a segment boundary: followed by
    /// `/`, `?`, or nothing. `/price` matches `/price`, `/price/1` and
    /// `/price?id=1`, and never `/prices-history`.
    Prefix,
    /// The path is the pattern, and nothing more.
    Exact,
}

/// A quota as published: `count` requests per `period`.
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
    id: BucketId,
    spec: RateSpec,
    /// The quota `limiter` was built from, read back by [`EffectiveQuota`].
    quota: Quota,
    limiter: DirectLimiter,
}

impl Bucket {
    fn new(id: BucketId, count: u32, period: Duration) -> Arc<Self> {
        let quota = quota(count, period);
        Arc::new(Self {
            id,
            spec: RateSpec { count, period },
            quota,
            limiter: DirectLimiter::direct(quota),
        })
    }

    fn effective(&self) -> EffectiveQuota {
        EffectiveQuota {
            bucket: self.id,
            count: self.spec.count,
            period: self.spec.period,
            quota: self.quota,
        }
    }
}

/// Rate limit configuration for a specific endpoint pattern.
struct EndpointLimit {
    path_prefix: &'static str,
    method: Option<Method>,
    match_mode: Matching,
    /// Every bucket a matching request must pass, awaited in order.
    buckets: Vec<Arc<Bucket>>,
}

impl EndpointLimit {
    /// Whether this entry governs the given request.
    ///
    /// Shared by [`RateLimiter::acquire`] and [`RateLimiter::effective_quota`]
    /// so the two cannot disagree about which rule applies.
    fn matches(&self, path: &str, method: Option<&Method>) -> bool {
        let path_matches = match self.match_mode {
            Matching::Exact => path == self.path_prefix,
            Matching::Prefix => {
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

/// Window quotas for one API surface: a general bucket every request passes,
/// then the buckets of the first row its method and path match.
///
/// Built by [`WindowQuotaTable`]. Polymarket's tables are
/// [`polymarket::clob_limits`](crate::polymarket::clob_limits) and its
/// siblings. A clone shares the buckets and the [`Hold`].
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
    general: Arc<Bucket>,
    /// What the server said, as against what the buckets model: no request
    /// on this limiter proceeds before it.
    hold: Hold,
}

/// Helper to create a quota: at most `count` requests in *any* window of
/// length `period`.
///
/// **There is deliberately no `allow_burst` call here.** `Quota::with_period`
/// leaves capacity at a single token, and keeping it there is the entire point.
/// A token bucket admits its depth *plus* everything the refill adds, so across
/// a window of length `period` it lets through `burst + rate × period`. Funding
/// a burst of `count` on top of a rate of `count/period` spends the published
/// allowance twice — which is what this function did for every entry in every
/// table until it was measured.
///
/// Depth is not free capacity; it is borrowed against the rate. Satisfying the
/// bound with a burst of `B` costs `B` requests of sustained allowance forever,
/// so the minimum depth is also the maximum throughput: 149/10s here rather
/// than the 135/10s a 10% burst would leave. It is the safer shape too: a
/// request that arrives during a hold waits before any bucket and is paced
/// when the hold ends, where a burst allowance would fire every one of them
/// as a spike immediately after a ban. Only a request already waiting on a
/// bucket when the hold began takes its token during the hold; those are
/// released together when it ends, so that burst is at most the requests in
/// flight then (on the send loop, the concurrency permits).
///
/// `count < 2` degenerates to admitting 2 per window, since a bucket cannot
/// hold less than one token. No published row is that small.
///
/// # Why it aims below the published count
///
/// Because the published count turns out not to be reachable as a rate.
/// Measured against `data-api.polymarket.com` on `/closed-positions`, which
/// publishes 150/10s:
///
/// | Sustained rate | Share of published | Result |
/// |---|---|---|
/// | 14.9/s (149 per 10s) | 100% | refused after 15.7s |
/// | 14.25/s (142.5 per 10s) | 95% | refused after 17.3s |
/// | 13.5/s (135 per 10s) | 90% | clean over 180s, 2,430 requests |
///
/// A one-shot burst of exactly 150 *is* accepted, so this is not the table
/// overstating the cap: the count is reachable as a burst and not as a rate.
/// Cloudflare's sliding-window estimator does not count the way a naive
/// interval count does, and nothing outside the server can observe the
/// difference — so the only safe response is to aim below the line rather than
/// at it. [`RESERVED_FRACTION`] is that margin, measured rather than
/// conventional: 95% is known-refused, 90% is known-clean.
fn quota(count: u32, period: Duration) -> Quota {
    Quota::with_period(WindowQuotaTable::paced_interval(count, period))
        .expect("quota interval must be non-zero")
}

/// Requests `q` admits in the worst-case window of length `period`: the full
/// bucket drained at `t=0`, plus every token the refill adds by `t=period`.
///
/// This is the quantity the published table bounds. Buckets start full, so the
/// worst case is always a fresh limiter.
fn admitted_in_one_window(q: &Quota, period: Duration) -> u128 {
    let refilled = period.as_nanos() / q.replenish_interval().as_nanos();
    u128::from(q.burst_size().get()) + refilled
}

/// Slots per `period` the client actually paces out for a published `count`:
/// the count, less its reserve, less the single token of depth.
///
/// Read through [`WindowQuotaTable::paced_interval`], which the runtime
/// agreement tests share, so their expected pacing cannot drift from what
/// [`quota`] builds. They would still catch a request routed to
/// the wrong bucket — a different `count` yields a different interval — but a
/// hand-copied formula here would silently loosen them the next time the
/// reserve changes.
fn sustained_slots(count: u32) -> u32 {
    let target = count.saturating_sub(count.div_ceil(RESERVED_FRACTION));
    target.max(2) - 1
}

/// Reciprocal of the share of each published quota the client leaves unused:
/// `10` reserves a tenth, so the client targets 90%.
///
/// Measured, not chosen — see the table on [`quota`].
const RESERVED_FRACTION: u32 = 10;

#[cfg(test)]
mod quota_arithmetic {
    //! The bound every bucket has to satisfy, checked as arithmetic.
    //!
    //! `agreement::assert_throttles_after` pins a bucket's *depth*: drain
    //! `count` and the next call has to wait, so capacity is no larger than the
    //! published figure. It says nothing about the refill rate, and depth and
    //! rate are two separate spends of one budget. A bucket holding `count`
    //! tokens that also replenishes `count` per `period` passes that test and
    //! still admits `2 * count` in a single window — each assertion true, the
    //! conjunction they exist to guarantee false.
    //!
    //! Measured against the live host on `/closed-positions` (150/10s): a
    //! one-shot burst of exactly 150 in 0.70s is accepted, while sustained runs
    //! tripped Cloudflare's `error code: 1015` at ~152 cumulative requests —
    //! twice, at different rates, which is the signature of a cumulative cap
    //! rather than a rate one. Upstream's published figure is accurate in both
    //! count and window; the client was spending it twice.

    use super::*;

    /// The four general-purpose default buckets, plus a spread of endpoint
    /// shapes for good measure.
    ///
    /// The defaults are the reason this list exists at all: they are built as
    /// bare `DirectLimiter`s carrying no `RateSpec`, so the sweep below — which
    /// walks the configured tables — cannot see them, and they are the largest
    /// allowance on every surface.
    const PUBLISHED_SHAPES: &[(u32, u64)] = &[
        (9_000, 10),    // clob default
        (4_000, 10),    // gamma default
        (1_000, 10),    // data default / clob /prices-history
        (25, 60),       // relay default
        (150, 10),      // data /closed-positions, /positions
        (200, 10),      // data /trades, clob /balance-allowance
        (300, 10),      // gamma /markets
        (350, 10),      // gamma /public-search
        (500, 10),      // gamma /events
        (100, 10),      // health routes
        (50, 10),       // clob /balance-allowance/update
        (5_000, 10),    // clob /order burst window
        (120_000, 600), // clob /order sustained window
    ];

    #[test]
    fn no_quota_admits_more_than_its_published_count_in_one_window() {
        for &(count, secs) in PUBLISHED_SHAPES {
            let period = Duration::from_secs(secs);
            let q = quota(count, period);
            let admitted = admitted_in_one_window(&q, period);

            assert!(
                admitted <= u128::from(count),
                "{count}/{secs}s admits {admitted} in one window \
                 ({} burst + {} refilled) — the published quota is spent twice",
                q.burst_size(),
                admitted - u128::from(q.burst_size().get()),
            );
        }
    }

    #[test]
    fn every_quota_reserves_headroom_below_the_published_count() {
        // Satisfying the published count exactly is not enough, because the
        // published count is not actually reachable. Measured on
        // `/closed-positions` (150/10s) against the live host: a sustained
        // 142.5/10s — 95% — was refused after 17.3s, and the client's own
        // exactly-100% pacing was refused after 15.7s, while 135/10s ran clean.
        // Cloudflare's sliding-window estimator does not count the way a naive
        // interval count does, and the client cannot observe the difference, so
        // it aims below the line rather than at it.
        for &(count, secs) in PUBLISHED_SHAPES {
            let period = Duration::from_secs(secs);
            let admitted = admitted_in_one_window(&quota(count, period), period);
            let ceiling = u128::from(count - count.div_ceil(RESERVED_FRACTION));

            assert!(
                admitted <= ceiling,
                "{count}/{secs}s admits {admitted} in one window, above the {ceiling} \
                 the reserve allows — no headroom under the published cap"
            );
        }
    }

    #[test]
    fn a_published_150_per_10s_admits_135_per_window() {
        // The reserve's golden value. On `/closed-positions` (150/10s) a
        // sustained 135 per 10s ran clean for 180s and 142.5 was refused. The
        // two sweeps above derive their ceiling from `RESERVED_FRACTION`
        // itself, so a changed reserve still passes them; this pins the 90%.
        let period = Duration::from_secs(10);
        assert_eq!(admitted_in_one_window(&quota(150, period), period), 135);
    }
}

/// One bucket of a [`WindowQuotaTable`], and of the [`RateLimiter`] it builds.
///
/// Rows that name one `BucketId` share one allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BucketId {
    table: u32,
    index: u32,
}

/// Tables made so far, so each [`BucketId`] names the table that made it.
static TABLES: AtomicU32 = AtomicU32::new(0);

/// A quota a request is held to: one bucket, admitting `count` requests per
/// `period` and paced as [`WindowQuotaTable`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveQuota {
    /// The bucket. Two quotas naming one bucket are one allowance.
    pub bucket: BucketId,
    /// The published count.
    pub count: u32,
    /// The published window.
    pub period: Duration,
    quota: Quota,
}

impl EffectiveQuota {
    /// The interval the bucket paces requests at, read from the bucket itself.
    pub fn interval(&self) -> Duration {
        self.quota.replenish_interval()
    }

    /// Requests the bucket admits in the worst-case window of length
    /// `period`: its whole depth at once, plus every token its refill adds by
    /// the window's end. A fresh bucket is the worst case, since buckets
    /// start full.
    pub fn admitted_in_one_window(&self) -> u128 {
        admitted_in_one_window(&self.quota, self.period)
    }
}

/// One row of a [`RateLimiter`], as [`RateLimiter::rows`] reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaRow<'a> {
    /// The path pattern.
    pub pattern: &'static str,
    /// The method the row is scoped to. `None` matches every method.
    pub method: Option<&'a Method>,
    /// How `pattern` is matched.
    pub matching: Matching,
    /// The row's buckets, in the order a request awaits them.
    pub buckets: Vec<EffectiveQuota>,
}

/// Builds a [`RateLimiter`] from a venue's window quotas: `count` requests in
/// any window of length `period`.
///
/// Every request passes the general bucket, then the buckets of the first row
/// whose pattern and method it matches, in the order the row lists them. Two
/// rows naming one [`BucketId`] share one allowance: Polymarket caps four
/// ledger routes at 900 per 10 s combined, which four buckets of their own
/// would turn into 3,600.
///
/// # How a bucket paces
///
/// Each bucket holds one token and refills one every
/// [`paced_interval`](Self::paced_interval), which aims at nine tenths of
/// `count`. There is no burst setting, on purpose:
///
/// - A token bucket admits its depth *plus* everything its refill adds. A
///   bucket holding `count` tokens and refilling `count` per `period` admits
///   twice the quota in one window. Depth is borrowed against the rate, so
///   the least depth is also the most throughput.
/// - A request that arrives during a hold waits before any bucket, and is
///   paced when the hold ends. A request already waiting on a bucket when the
///   hold begins takes its token, then waits out the hold too, and is
///   released with the others when it ends: that burst is at most the
///   requests in flight when the hold began (on the send loop, the
///   concurrency permits).
/// - A published count is reachable as a burst and not as a rate. On
///   Polymarket's `/closed-positions` (150 per 10 s) a sustained 95% was
///   refused and 90% ran clean for 180 s, so a bucket keeps a tenth back.
///
/// A count below 2 paces as 2 per window, since a bucket holds at least one
/// token.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// use polyoxide_core::{Matching, WindowQuotaTable};
/// use reqwest::Method;
///
/// let ten_seconds = Duration::from_secs(10);
/// let mut table = WindowQuotaTable::new(1_000, ten_seconds);
/// let ledger = table.bucket(900, ten_seconds);
/// table
///     .prefix("/positions", None, 150, ten_seconds)
///     .row(Matching::Prefix, "/trades", Some(Method::GET), &[ledger])
///     .row(Matching::Exact, "/orders", Some(Method::GET), &[ledger]);
/// let limiter = table.build();
///
/// // The general bucket first, then the row's.
/// let trades = limiter.effective_quota(&Method::GET, "/trades");
/// let orders = limiter.effective_quota(&Method::GET, "/orders");
/// assert_eq!(trades[1].bucket, orders[1].bucket);
/// assert_eq!(trades[1].admitted_in_one_window(), 810);
/// // An exact row does not match a sub-path.
/// assert_eq!(limiter.effective_quota(&Method::GET, "/orders/1").len(), 1);
/// ```
pub struct WindowQuotaTable {
    id: u32,
    /// The general bucket first, then every bucket in the order it was made.
    buckets: Vec<Arc<Bucket>>,
    rows: Vec<EndpointLimit>,
    hold: Hold,
}

impl WindowQuotaTable {
    /// A table whose general bucket admits `count` requests per `period`.
    ///
    /// # Panics
    ///
    /// When `period` is too short to pace `count`, since a bucket that paces
    /// at an interval of zero would never hold anyone back: a zero `period`,
    /// or one shorter than a nanosecond per request it paces.
    pub fn new(count: u32, period: Duration) -> Self {
        let mut table = Self {
            id: TABLES.fetch_add(1, Ordering::Relaxed),
            buckets: Vec::new(),
            rows: Vec::new(),
            hold: Hold::unbounded(),
        };
        table.bucket(count, period);
        table
    }

    /// Make a bucket admitting `count` requests per `period`, for rows to
    /// name.
    ///
    /// # Panics
    ///
    /// When `period` is too short to pace `count`, since a bucket that paces
    /// at an interval of zero would never hold anyone back: a zero `period`,
    /// or one shorter than a nanosecond per request it paces.
    pub fn bucket(&mut self, count: u32, period: Duration) -> BucketId {
        let id = BucketId {
            table: self.id,
            index: u32::try_from(self.buckets.len()).expect("fewer than 2^32 buckets"),
        };
        self.buckets.push(Bucket::new(id, count, period));
        id
    }

    /// Add a row: a request whose path matches `pattern`, and whose method is
    /// `method` when one is given, awaits `buckets` in order.
    ///
    /// Only the first row a request matches applies, so a pattern goes before
    /// any shorter one that also matches it.
    ///
    /// # Panics
    ///
    /// When one of `buckets` was made by another table.
    pub fn row(
        &mut self,
        matching: Matching,
        pattern: &'static str,
        method: Option<Method>,
        buckets: &[BucketId],
    ) -> &mut Self {
        let buckets = buckets
            .iter()
            .map(|id| {
                assert_eq!(id.table, self.id, "{id:?} was made by another table");
                self.buckets[id.index as usize].clone()
            })
            .collect();
        self.rows.push(EndpointLimit {
            path_prefix: pattern,
            method,
            match_mode: matching,
            buckets,
        });
        self
    }

    /// Add a [`Matching::Prefix`] row with a bucket of its own, admitting
    /// `count` requests per `period`.
    ///
    /// # Panics
    ///
    /// When `period` is too short to pace `count`, since a bucket that paces
    /// at an interval of zero would never hold anyone back: a zero `period`,
    /// or one shorter than a nanosecond per request it paces.
    pub fn prefix(
        &mut self,
        pattern: &'static str,
        method: Option<Method>,
        count: u32,
        period: Duration,
    ) -> &mut Self {
        let bucket = self.bucket(count, period);
        self.row(Matching::Prefix, pattern, method, &[bucket])
    }

    /// Wait out `hold` instead of a hold of the limiter's own, so that
    /// every layer sharing it holds together. Default: [`Hold::unbounded`].
    pub fn with_hold(&mut self, hold: Hold) -> &mut Self {
        self.hold = hold;
        self
    }

    /// The limiter, with every bucket full.
    pub fn build(self) -> RateLimiter {
        RateLimiter {
            inner: Arc::new(RateLimiterInner {
                general: self.buckets[0].clone(),
                limits: self.rows,
                hold: self.hold,
            }),
        }
    }

    /// The interval a bucket for `count` requests per `period` paces at:
    /// `period` over nine tenths of `count`, less the one token of depth.
    ///
    /// The one place the pacing formula lives; every bucket is built from it.
    pub fn paced_interval(count: u32, period: Duration) -> Duration {
        period / sustained_slots(count)
    }
}

impl RateLimiter {
    /// Hold every request on this limiter for `delay`: [`Hold::extend`] on
    /// its hold, which extends a hold in force but never shortens one.
    ///
    /// The send loop holds through [`Throttle::hold`], which calls this, when
    /// its policy decides a response holds the client. Reach for this directly
    /// only when driving the limiter from a transport this crate does not own.
    pub fn begin_cooldown(&self, delay: Duration) {
        self.inner.hold.extend(delay);
    }

    /// The hold every request on this limiter waits out, to share with
    /// another layer.
    pub fn hold(&self) -> &Hold {
        &self.inner.hold
    }

    /// Await the appropriate limiter(s) for this endpoint.
    ///
    /// Waits out any hold a previous 429 imposed, then awaits the default
    /// (general) limiter, then additionally awaits the first matching
    /// endpoint-specific limiter (burst + sustained), then waits out the hold
    /// again, in case it moved while this call waited on a bucket.
    pub async fn acquire(&self, path: &str, method: Option<&Method>) {
        self.inner.hold.wait().await;
        self.inner.general.limiter.until_ready().await;

        if let Some(limit) = self.row_for(path, method) {
            for bucket in &limit.buckets {
                bucket.limiter.until_ready().await;
            }
        }
        // AD-23: a hold set while this call waited on a bucket is honoured,
        // rather than sending into it.
        self.inner.hold.wait().await;
    }

    /// The quotas a request is held to, in the order
    /// [`acquire`](Self::acquire) awaits them: the general bucket first, then
    /// the buckets of the first row `method` and `path` match.
    pub fn effective_quota(&self, method: &Method, path: &str) -> Vec<EffectiveQuota> {
        std::iter::once(&self.inner.general)
            .chain(
                self.row_for(path, Some(method))
                    .into_iter()
                    .flat_map(|limit| &limit.buckets),
            )
            .map(|bucket| bucket.effective())
            .collect()
    }

    /// Every row, in the order requests are matched against them.
    pub fn rows(&self) -> Vec<QuotaRow<'_>> {
        self.inner
            .limits
            .iter()
            .map(|limit| QuotaRow {
                pattern: limit.path_prefix,
                method: limit.method.as_ref(),
                matching: limit.match_mode,
                buckets: limit.buckets.iter().map(|b| b.effective()).collect(),
            })
            .collect()
    }

    /// The first row a request matches.
    fn row_for(&self, path: &str, method: Option<&Method>) -> Option<&EndpointLimit> {
        self.inner.limits.iter().find(|l| l.matches(path, method))
    }
}

/// Counts requests: each attempt is charged one request against the general
/// bucket and its row's buckets, found from the method and path, and
/// [`hold`](Throttle::hold) extends the limiter's [`Hold`].
impl Throttle for RateLimiter {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        RateLimiter::acquire(self, meta.path, Some(meta.method)).await;
        Ok(Charge::none())
    }

    /// Nothing to record: the buckets model the published quota, not what a
    /// response says.
    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, delay: Duration) {
        self.inner.hold.extend(delay);
    }
}

#[cfg(test)]
mod window_table {
    //! [`WindowQuotaTable`], through the API a venue outside core sees.

    use super::*;

    const TEN_SECONDS: Duration = Duration::from_secs(10);
    const TEN_MINUTES: Duration = Duration::from_secs(600);

    /// `(count, period)` of each quota, general bucket first.
    fn shape(quotas: &[EffectiveQuota]) -> Vec<(u32, Duration)> {
        quotas.iter().map(|q| (q.count, q.period)).collect()
    }

    #[test]
    fn a_request_awaits_the_general_bucket_then_its_rows_buckets_in_order() {
        let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
        let burst = table.bucket(500, TEN_SECONDS);
        let sustained = table.bucket(6_000, TEN_MINUTES);
        table.row(
            Matching::Prefix,
            "/order",
            Some(Method::POST),
            &[burst, sustained],
        );
        let limiter = table.build();

        let quotas = limiter.effective_quota(&Method::POST, "/order");
        assert_eq!(
            shape(&quotas),
            [
                (1_000, TEN_SECONDS),
                (500, TEN_SECONDS),
                (6_000, TEN_MINUTES)
            ]
        );
        assert_eq!(quotas[1].bucket, burst);
        assert_eq!(quotas[2].bucket, sustained);
        assert_eq!(
            shape(&limiter.effective_quota(&Method::GET, "/elsewhere")),
            [(1_000, TEN_SECONDS)],
            "an unmatched request is held to the general bucket alone"
        );
    }

    #[tokio::test]
    async fn two_rows_naming_one_bucket_share_one_allowance() {
        let mut table = WindowQuotaTable::new(100_000, TEN_SECONDS);
        let shared = table.bucket(100, TEN_SECONDS);
        table
            .row(Matching::Prefix, "/a", None, &[shared])
            .row(Matching::Prefix, "/b", None, &[shared])
            .prefix("/c", None, 100, TEN_SECONDS);
        let limiter = table.build();

        let a = limiter.effective_quota(&Method::GET, "/a")[1].bucket;
        let b = limiter.effective_quota(&Method::GET, "/b")[1].bucket;
        let c = limiter.effective_quota(&Method::GET, "/c")[1].bucket;
        assert_eq!(a, b);
        assert_ne!(a, c, "a row's own bucket is shared with nobody");

        // 100 per 10 s paces at ~112ms: /b waits for the token /a took.
        limiter.acquire("/a", None).await;
        let start = std::time::Instant::now();
        limiter.acquire("/b", None).await;
        assert!(
            start.elapsed() >= Duration::from_millis(80),
            "/b went out after {:?}, so /a and /b are not one allowance",
            start.elapsed()
        );
    }

    #[test]
    fn the_first_matching_row_wins() {
        let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
        table
            .prefix("/data/orders", None, 50, TEN_SECONDS)
            .prefix("/data", None, 500, TEN_SECONDS)
            .prefix("/data/trades", None, 60, TEN_SECONDS);
        let limiter = table.build();

        let count = |path| limiter.effective_quota(&Method::GET, path)[1].count;
        assert_eq!(count("/data/orders"), 50);
        assert_eq!(count("/data"), 500);
        assert_eq!(
            count("/data/trades"),
            500,
            "a row after a shorter pattern that matches it is never reached"
        );
    }

    #[test]
    fn a_prefix_row_matches_only_at_a_segment_boundary() {
        let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
        table.prefix("/price", None, 100, TEN_SECONDS);
        let limiter = table.build();

        for path in ["/price", "/price/1", "/price?token=1"] {
            assert_eq!(
                limiter.effective_quota(&Method::GET, path).len(),
                2,
                "{path}"
            );
        }
        for path in ["/prices-history", "/pricing", "/midpoint"] {
            assert_eq!(
                limiter.effective_quota(&Method::GET, path).len(),
                1,
                "{path}"
            );
        }
    }

    #[test]
    fn an_exact_row_matches_only_its_own_path() {
        let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
        let bucket = table.bucket(100, TEN_SECONDS);
        table.row(Matching::Exact, "/trades", None, &[bucket]);
        let limiter = table.build();

        assert_eq!(limiter.effective_quota(&Method::GET, "/trades").len(), 2);
        for path in ["/trades/1", "/trades?limit=10", "/traded"] {
            assert_eq!(
                limiter.effective_quota(&Method::GET, path).len(),
                1,
                "{path}"
            );
        }
    }

    #[test]
    fn a_method_scoped_row_ignores_other_methods() {
        let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
        table
            .prefix("/order", Some(Method::POST), 500, TEN_SECONDS)
            .prefix("/order", None, 90, TEN_SECONDS);
        let limiter = table.build();

        let count = |method| limiter.effective_quota(&method, "/order")[1].count;
        assert_eq!(count(Method::POST), 500);
        assert_eq!(count(Method::GET), 90);
        assert_eq!(count(Method::DELETE), 90);
    }

    #[test]
    fn rows_reports_every_row_in_match_order() {
        let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
        let shared = table.bucket(900, TEN_SECONDS);
        table
            .prefix("/balance-allowance/update", None, 50, TEN_SECONDS)
            .row(Matching::Exact, "/trades", Some(Method::GET), &[shared]);
        let limiter = table.build();

        let rows = limiter.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pattern, "/balance-allowance/update");
        assert_eq!(rows[0].method, None);
        assert_eq!(rows[0].matching, Matching::Prefix);
        assert_eq!(shape(&rows[0].buckets), [(50, TEN_SECONDS)]);
        assert_eq!(rows[1].pattern, "/trades");
        assert_eq!(rows[1].method, Some(&Method::GET));
        assert_eq!(rows[1].matching, Matching::Exact);
        assert_eq!(rows[1].buckets[0].bucket, shared);
        assert_eq!(format!("{limiter:?}"), "RateLimiter { endpoints: 2 }");
    }

    #[test]
    fn every_bucket_paces_at_its_paced_interval_and_keeps_a_tenth_back() {
        for (count, period) in [
            (25, TEN_SECONDS * 6),
            (150, TEN_SECONDS),
            (9_000, TEN_SECONDS),
        ] {
            let mut table = WindowQuotaTable::new(count, period);
            table.prefix("/row", None, count, period);
            let limiter = table.build();

            for quota in limiter.effective_quota(&Method::GET, "/row") {
                assert_eq!(
                    quota.interval(),
                    WindowQuotaTable::paced_interval(count, period)
                );
                assert!(
                    quota.admitted_in_one_window() <= u128::from(count - count.div_ceil(10)),
                    "{count}/{period:?} admits {} in one window",
                    quota.admitted_in_one_window()
                );
            }
        }
        assert_eq!(
            WindowQuotaTable::paced_interval(150, TEN_SECONDS),
            TEN_SECONDS / 134
        );
    }

    #[tokio::test]
    async fn a_hold_set_during_a_bucket_wait_is_honoured() {
        // 2 per 300ms paces one request every 300ms. The second request waits
        // for its token; 50ms in, a 429 elsewhere holds the limiter for 600ms.
        // Going out when the token arrives would send into that hold.
        let mut table = WindowQuotaTable::new(100_000, TEN_SECONDS);
        table.prefix("/slow", None, 2, Duration::from_millis(300));
        let limiter = table.build();
        limiter.acquire("/slow", None).await;

        let start = std::time::Instant::now();
        let holder = {
            let limiter = limiter.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                limiter.begin_cooldown(Duration::from_millis(600));
            })
        };
        limiter.acquire("/slow", None).await;
        holder.await.unwrap();
        assert!(
            start.elapsed() >= Duration::from_millis(600),
            "the request went out after {:?}, inside a hold set while it waited for its \
             token",
            start.elapsed()
        );
    }

    #[test]
    #[should_panic(expected = "was made by another table")]
    fn a_bucket_from_another_table_is_refused() {
        let mut other = WindowQuotaTable::new(1_000, TEN_SECONDS);
        let foreign = other.bucket(100, TEN_SECONDS);
        let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
        table.bucket(100, TEN_SECONDS);
        table.row(Matching::Prefix, "/a", None, &[foreign]);
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

    /// The delay before the attempt after `attempt`: its [`backoff`](Self::backoff),
    /// lengthened by the server's `Retry-After`.
    ///
    /// `Retry-After` can only *extend* the wait, never shorten it. A server
    /// asking for longer than the client-computed backoff is obeyed (clamped to
    /// `max_backoff_ms`); one asking for less — including the zero that
    /// Cloudflare returns alongside `error code: 1015` — leaves the exponential
    /// backoff in place. Taking the header verbatim made a tripped Cloudflare
    /// limit self-perpetuating: `Duration::from_millis(0)` is not a backoff, and
    /// the three retries landed inside 65ms, extending the ban they were waiting
    /// on. Values that do not parse as a float (e.g. the HTTP-date form) are
    /// ignored the same way.
    ///
    /// The header is read by [`polyoxide_venue::parse_retry_after`], the one
    /// parser every crate uses, clamped to `max_backoff_ms`, and the longer
    /// wait is taken by [`polyoxide_venue::retry_delay`].
    ///
    /// The send loop's floor and a policy's hold both come from here, so a
    /// response cannot hold the client by one rule and pace its own retry by
    /// another.
    pub fn retry_delay(&self, attempt: u32, retry_after: Option<&str>) -> Duration {
        let clamp = Duration::from_millis(self.max_backoff_ms);
        let requested = retry_after.and_then(|v| polyoxide_venue::parse_retry_after(v, clamp));
        polyoxide_venue::retry_delay(requested, self.backoff(attempt))
    }

    /// What [`HttpClient::send`](crate::HttpClient::send) tells its hooks
    /// about `attempt`: the attempt, from 0, and the retries this schedule
    /// still allows after it.
    pub fn attempt_info(&self, attempt: u32) -> AttemptInfo {
        AttemptInfo {
            attempt,
            retries_left: self.max_retries.saturating_sub(attempt),
        }
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

    #[test]
    fn the_one_retry_after_parser_settles_the_old_disagreements() {
        // DRIFT R4. Core read the header untrimmed, so a padded value was
        // ignored and the client fell back to its own backoff.
        let cfg = RetryConfig::default();
        assert_eq!(cfg.retry_delay(0, Some(" 2 ")), Duration::from_secs(2));
        // Unchanged: the clamp is `max_backoff_ms`, and junk is no wait.
        assert_eq!(
            cfg.retry_delay(0, Some("604800")),
            Duration::from_millis(cfg.max_backoff_ms)
        );
        for junk in [
            "Wed, 21 Oct 2026 07:28:00 GMT",
            "abc",
            "NaN",
            "inf",
            "-1",
            "",
        ] {
            let ms = cfg.retry_delay(0, Some(junk)).as_millis();
            assert!((375..=625).contains(&ms), "{junk:?} waited {ms}ms");
        }
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
        // The sustained rate is count-1, so 0 and 1 both have to be clamped or
        // the period is divided by zero. Neither appears in any table.
        let _ = quota(0, Duration::from_secs(10));
        let _ = quota(1, Duration::from_secs(10));
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

    // ── should_retry edge cases ─────────────────────────────────
    //
    // Driven through Polymarket's policy, which the send loop asks, since the
    // hand-written loops that called `HttpClient::should_retry` are gone.

    /// Whether Polymarket's policy retries a 429 on `attempt` under `config`.
    fn retries_a_429(config: &RetryConfig, attempt: u32) -> bool {
        use crate::hooks::{Outcome, ResponseMeta, RetryPolicy};

        let headers = reqwest::header::HeaderMap::new();
        let response = ResponseMeta {
            status: reqwest::StatusCode::TOO_MANY_REQUESTS,
            headers: &headers,
        };
        matches!(
            crate::polymarket::PolymarketRetryPolicy
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
