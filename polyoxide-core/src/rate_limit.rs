//! Window quotas: [`WindowQuotaTable`] builds a [`RateLimiter`], which counts
//! requests against the buckets a request's method and path resolve to.
//!
//! Polymarket's tables are built in [`polymarket`](crate::polymarket).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use governor::Quota;
use reqwest::Method;
use tokio::time::Instant;

use crate::hooks::{AttemptInfo, Charge, Refused, RequestMeta, ResponseMeta, Throttle};

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
/// siblings. A clone shares the buckets and the cooldown.
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
    /// Deadline before which no request on this limiter may proceed.
    ///
    /// The buckets above encode the quota Polymarket *publishes*; this encodes
    /// what the server actually just said. They disagree more often than the
    /// tables suggest — Cloudflare's `error code: 1015` is an IP-scoped block
    /// with its own window, and it answers 429 no matter how many tokens the
    /// buckets still hold.
    cooldown_until: Mutex<Option<Instant>>,
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
/// than the 135/10s a 10% burst would leave. It is the safer shape too — the
/// client never concentrates requests into an instant, including on release
/// from a cooldown, when every parked request resumes at once and a burst
/// allowance would fire them as a spike immediately after a ban.
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
    use crate::polymarket;

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
    fn every_configured_bucket_satisfies_the_quota_it_publishes() {
        for (surface, rl) in [
            ("clob", polymarket::clob_limits()),
            ("gamma", polymarket::gamma_limits()),
            ("data", polymarket::data_limits()),
            ("relay", polymarket::relay_limits()),
        ] {
            for row in rl.rows() {
                for bucket in &row.buckets {
                    let EffectiveQuota { count, period, .. } = *bucket;
                    let admitted = bucket.admitted_in_one_window();

                    assert!(
                        admitted <= u128::from(count),
                        "{surface} {} is published as {count}/{period:?} but admits \
                         {admitted} in one window",
                        row.pattern,
                    );
                }
            }
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
///   the least depth is also the most throughput, and nothing parked behind
///   a hold resumes as a spike.
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
}

impl WindowQuotaTable {
    /// A table whose general bucket admits `count` requests per `period`.
    pub fn new(count: u32, period: Duration) -> Self {
        let mut table = Self {
            id: TABLES.fetch_add(1, Ordering::Relaxed),
            buckets: Vec::new(),
            rows: Vec::new(),
        };
        table.bucket(count, period);
        table
    }

    /// Make a bucket admitting `count` requests per `period`, for rows to
    /// name.
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

    /// The limiter, with every bucket full.
    pub fn build(self) -> RateLimiter {
        RateLimiter {
            inner: Arc::new(RateLimiterInner {
                general: self.buckets[0].clone(),
                limits: self.rows,
                cooldown_until: Mutex::new(None),
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
    /// Hold every request on this limiter for `delay`.
    ///
    /// Extends an existing cooldown but never shortens one: several concurrent
    /// requests typically see the same 429 within a few milliseconds of each
    /// other, and taking the most recent value would let whichever response
    /// carried the smallest delay release all of them early.
    ///
    /// The send loop holds through [`Throttle::hold`], which calls this, when
    /// its policy decides a response holds the client. Reach for this directly
    /// only when driving the limiter from a transport this crate does not own.
    pub fn begin_cooldown(&self, delay: Duration) {
        let until = Instant::now() + delay;
        let mut slot = self.lock_cooldown();
        if slot.is_none_or(|current| until > current) {
            *slot = Some(until);
        }
    }

    /// A poison-tolerant lock on the cooldown slot.
    ///
    /// A panic elsewhere must not turn the rate limiter into a permanent
    /// outage; the worst a torn write can cost here is one early or late
    /// wakeup.
    fn lock_cooldown(&self) -> std::sync::MutexGuard<'_, Option<Instant>> {
        self.inner
            .cooldown_until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Wait out any cooldown currently in force.
    async fn await_cooldown(&self) {
        loop {
            // Read the deadline and release the guard before awaiting. Holding
            // a `std::sync::MutexGuard` across an await makes the future
            // `!Send`, which every caller of `acquire` needs it to be.
            let deadline = *self.lock_cooldown();
            let Some(deadline) = deadline else { return };
            if deadline <= Instant::now() {
                return;
            }
            // Loop rather than return after sleeping: a sibling's 429 can push
            // the deadline out while we wait, and waking into a still-active
            // block is how the storm restarts.
            tokio::time::sleep_until(deadline).await;
        }
    }

    /// Await the appropriate limiter(s) for this endpoint.
    ///
    /// Waits out any cooldown a previous 429 imposed, then awaits the default
    /// (general) limiter, then additionally awaits the first matching
    /// endpoint-specific limiter (burst + sustained).
    pub async fn acquire(&self, path: &str, method: Option<&Method>) {
        self.await_cooldown().await;
        self.inner.general.limiter.until_ready().await;

        if let Some(limit) = self.row_for(path, method) {
            for bucket in &limit.buckets {
                bucket.limiter.until_ready().await;
            }
        }
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
/// bucket and its row's buckets, found from the method and path. The hold is
/// the cooldown.
impl Throttle for RateLimiter {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        RateLimiter::acquire(self, meta.path, Some(meta.method)).await;
        Ok(Charge::none())
    }

    /// Nothing to record: the buckets model the published quota, not what a
    /// response says.
    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, delay: Duration) {
        self.begin_cooldown(delay);
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
    /// The send loop's floor and a policy's hold both come from here, so a
    /// response cannot hold the client by one rule and pace its own retry by
    /// another.
    pub fn retry_delay(&self, attempt: u32, retry_after: Option<&str>) -> Duration {
        let computed = self.backoff(attempt);
        let requested = retry_after
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|secs| secs.is_finite() && *secs > 0.0)
            .map(|secs| {
                let ms = (secs * 1000.0) as u64;
                Duration::from_millis(ms.min(self.max_backoff_ms))
            });
        requested.map_or(computed, |r| r.max(computed))
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

    /// A quota as published: `count` requests per `period`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Spec {
        pub count: u32,
        pub period: Duration,
    }

    /// The quotas a request resolves to, read through
    /// [`RateLimiter::effective_quota`].
    pub trait ResolveSpecs {
        /// The quotas a request would be held to beyond the general bucket, in
        /// the order they are awaited.
        ///
        /// Empty when nothing matches — meaning the request is governed only by
        /// the general bucket, which is the shape every over-permit bug in this
        /// table has taken.
        fn resolve_specs(&self, path: &str, method: Option<&Method>) -> Vec<Spec>;
    }

    impl ResolveSpecs for RateLimiter {
        fn resolve_specs(&self, path: &str, method: Option<&Method>) -> Vec<Spec> {
            let method = method.expect("every rule asserted here names its method");
            // `effective_quota` lists the general bucket first; slice it off.
            self.effective_quota(method, path)[1..]
                .iter()
                .map(|q| Spec {
                    count: q.count,
                    period: q.period,
                })
                .collect()
        }
    }

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

    /// Assert `path` is paced at runtime by the quota it publishes.
    ///
    /// Matching a spec is not the same as enforcing it; this is the runtime
    /// half of the agreement. Buckets hold a single token, so one request
    /// empties `path`'s bucket and the next has to wait a full replenish
    /// interval — no `count`-sized drain required, and none wanted: draining
    /// `count` under uniform pacing takes a real `period`, which would put a
    /// 10-second sleep in the unit suite for every row asserted.
    ///
    /// Asserting *how long* the wait is, rather than merely that there was
    /// one, is what makes this specific. The delay identifies which bucket the
    /// request came from: `/closed-positions` (150/10s) paces at ~67ms while
    /// the surface's general bucket paces at ~10ms. The upper bound catches
    /// the inverse failure — a path resolving through some tighter rule that
    /// shadows it, which is the shape every ordering bug in these tables has
    /// taken.
    pub async fn assert_paced_by_its_own_quota(
        rl: &RateLimiter,
        path: &str,
        count: u32,
        period: Duration,
    ) {
        let interval = WindowQuotaTable::paced_interval(count, period);

        rl.acquire(path, Some(&Method::GET)).await;

        let start = std::time::Instant::now();
        rl.acquire(path, Some(&Method::GET)).await;
        let waited = start.elapsed();

        assert!(
            waited >= interval.mul_f64(0.8),
            "the 2nd request to {path} returned in {waited:?}; {count}/{period:?} should pace \
             it at {interval:?} and the cap is not being enforced"
        );
        assert!(
            waited <= interval * 3 + Duration::from_millis(25),
            "the 2nd request to {path} waited {waited:?}, far longer than the {interval:?} its \
             published {count}/{period:?} implies — it is resolving through a tighter rule"
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
    use crate::polymarket;

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
        assert_matches_published(&polymarket::data_limits(), documented(), 1_000);
    }

    /// Data API v2 rows. Upstream publishes no v2 figures; each count is the
    /// highest clean ramp rate recorded in `docs/specs/data-v2/OBSERVED.md`.
    fn measured_v2() -> Vec<DocumentedRule> {
        vec![
            ("/v2/positions", Some(Method::GET), vec![(200, 10)]),
            ("/v2/positions/combos", Some(Method::GET), vec![(400, 10)]),
            ("/v2/trades", Some(Method::GET), vec![(200, 10)]),
            ("/v2/activity", Some(Method::GET), vec![(400, 10)]),
            ("/v2/user-pnl", Some(Method::GET), vec![(400, 10)]),
            ("/v2/holders", Some(Method::GET), vec![(400, 10)]),
        ]
    }

    #[test]
    fn every_v2_route_resolves_to_its_measured_quota() {
        assert_matches_published(&polymarket::data_limits(), measured_v2(), 1_000);
    }

    #[test]
    fn the_health_cap_is_attached_to_the_route_the_host_answers_on() {
        // `/ok` is a 404 on data-api. An entry there caps nothing and leaves
        // the real health route — `/` — on the 10x-looser general bucket.
        assert_unconfigured(&polymarket::data_limits(), "/ok");
    }

    #[test]
    fn the_root_health_rule_does_not_swallow_every_other_route() {
        // `/` under prefix matching could plausibly match everything. The
        // segment-boundary rule saves it: `strip_prefix("/")` on `/positions`
        // leaves `positions`, which starts with neither `/` nor `?`.
        let rl = polymarket::data_limits();
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
        // 150/10s paces one request every ~67ms.
        assert_paced_by_its_own_quota(
            &polymarket::data_limits(),
            "/closed-positions",
            150,
            Duration::from_secs(10),
        )
        .await;
    }

    #[tokio::test]
    async fn closed_positions_and_positions_do_not_share_an_allowance() {
        // Upstream publishes 150/10s for each, not 150/10s combined. Emptying
        // one must leave the other untouched — the inverse of the CLOB ledger
        // group, where sharing *is* the published behaviour.
        //
        // The margin here is ~67ms (shared) against ~10ms (separate, and only
        // that much because the request still passes the surface's general
        // 1,000/10s bucket). Both sides of that gap are load-bearing, so the
        // threshold sits between them rather than at zero.
        let rl = polymarket::data_limits();
        rl.acquire("/closed-positions", Some(&Method::GET)).await;

        let start = std::time::Instant::now();
        rl.acquire("/positions", Some(&Method::GET)).await;
        assert!(
            start.elapsed() < Duration::from_millis(25),
            "/positions was throttled by /closed-positions emptying its own bucket"
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
    use crate::polymarket;

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
        assert_matches_published(&polymarket::gamma_limits(), documented(), 4_000);
    }

    #[test]
    fn the_health_cap_is_attached_to_the_route_the_host_answers_on() {
        assert_unconfigured(&polymarket::gamma_limits(), "/ok");
    }

    #[test]
    fn the_markets_plus_events_group_cap_can_never_bind() {
        // Upstream also publishes a 900/10s cap shared by /markets + /events.
        // It is deliberately not modelled because the per-endpoint caps sum to
        // less than it. If either cap is ever raised, this stops being true and
        // the group bucket has to be added — that is what this test watches.
        let rl = polymarket::gamma_limits();
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
        assert_paced_by_its_own_quota(
            &polymarket::gamma_limits(),
            "/markets",
            300,
            Duration::from_secs(10),
        )
        .await;
    }
}

#[cfg(test)]
mod documented_perps_limits {
    //! Agreement tests for the Perps table. Nothing is published, so the
    //! rows are the measured figures in `docs/specs/perps/OBSERVED.md`.

    use super::agreement::*;
    use super::*;
    use crate::polymarket;

    /// The measured table, transcribed by hand from the OBSERVED.md runs.
    /// This is the golden vector: a second transcription, separate from the
    /// constants inside `perps_default()`, so a typo in either is caught.
    const KLINES: u32 = 30;
    const TRADES: u32 = 10;
    const PORTFOLIO: u32 = 30;
    const BBO: u32 = 50;
    /// Every other `/v1/info/*` route: the lowest measured row.
    const UNSOAKED: u32 = 10;
    /// The client-wide cap the mixed validation run was clean at.
    const GENERAL: u32 = 30;

    fn measured() -> Vec<DocumentedRule> {
        vec![
            ("/v1/info/klines", Some(Method::GET), vec![(KLINES, 10)]),
            ("/v1/info/trades", Some(Method::GET), vec![(TRADES, 10)]),
            (
                "/v1/info/portfolio",
                Some(Method::GET),
                vec![(PORTFOLIO, 10)],
            ),
            ("/v1/info/bbo", Some(Method::GET), vec![(BBO, 10)]),
            (
                "/v1/info/instruments",
                Some(Method::GET),
                vec![(UNSOAKED, 10)],
            ),
            (
                "/v1/info/statistics",
                Some(Method::GET),
                vec![(UNSOAKED, 10)],
            ),
        ]
    }

    #[test]
    fn every_soaked_route_has_its_own_row() {
        assert_matches_published(&polymarket::perps_limits(), measured(), u32::MAX);
    }

    #[test]
    fn an_unsoaked_info_route_is_held_to_the_catch_all_not_the_general_bucket() {
        // The general bucket is 30/10s, so without the catch-all an
        // unmeasured route would be driven three times harder than trades,
        // the tightest route measured.
        let rl = polymarket::perps_limits();
        let specs = rl.resolve_specs("/v1/info/instruments", Some(&Method::GET));
        assert_eq!(
            specs.len(),
            1,
            "instruments matched no row: the /v1/info catch-all is missing or misordered"
        );
        assert_eq!(specs[0].count, UNSOAKED);
        // A soaked route must resolve to its own row, not the catch-all.
        assert_eq!(
            rl.resolve_specs("/v1/info/bbo", Some(&Method::GET))[0].count,
            BBO
        );
        // Outside /v1/info nothing is configured.
        assert_unconfigured(&rl, "/v1/nope");
    }

    #[tokio::test]
    async fn the_general_bucket_caps_the_client_below_the_most_permissive_route() {
        // bbo alone sustains 50/10s, but a mixed run at that cap was
        // throttled: the per-IP budget is partly shared. The general bucket
        // must therefore bind before bbo's own row does. The pacing window
        // accepted here admits a general bucket of roughly 11 to 37, so this
        // proves general < bbo; the exact 30 is pinned by GENERAL against
        // OBSERVED.md, not by timing.
        const _: () = assert!(
            GENERAL < BBO,
            "the general bucket must bind before bbo's row"
        );
        let rl = polymarket::perps_limits();
        assert_paced_by_its_own_quota(&rl, "/v1/info/bbo", GENERAL, Duration::from_secs(10)).await;
    }

    #[tokio::test]
    async fn the_klines_row_actually_paces() {
        let rl = polymarket::perps_limits();
        let count = rl.resolve_specs("/v1/info/klines", Some(&Method::GET))[0].count;
        assert_paced_by_its_own_quota(&rl, "/v1/info/klines", count, Duration::from_secs(10)).await;
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

    use super::agreement::{assert_paced_by_its_own_quota, DocumentedRule, ResolveSpecs};
    use super::*;
    use crate::polymarket;

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
        let rl = polymarket::clob_limits();

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
        let rl = polymarket::clob_limits();
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
        let rl = polymarket::clob_limits();
        let group: Vec<_> = ["/trades", "/orders", "/order", "/notifications"]
            .iter()
            .map(|p| {
                rl.effective_quota(&Method::GET, p)
                    .get(1)
                    .unwrap_or_else(|| panic!("{p} should match a ledger entry"))
                    .bucket
            })
            .collect();

        for other in &group[1..] {
            assert!(
                group[0] == *other,
                "ledger endpoints must share one bucket, not hold copies"
            );
        }
    }

    #[test]
    fn balance_allowance_update_is_not_shadowed_by_its_parent_path() {
        // `/balance-allowance/update` starts with `/balance-allowance` at a
        // segment boundary, so ordering decides which rule wins. The update
        // route is four times tighter.
        let rl = polymarket::clob_limits();
        let update = rl.resolve_specs("/balance-allowance/update", Some(&Method::GET));
        assert_eq!(
            update[0].count, 50,
            "the tighter /balance-allowance/update rule must be ordered first"
        );
    }

    #[tokio::test]
    async fn a_documented_cap_actually_throttles() {
        // Matching specs is not the same as enforcing them. `/tick-size` is
        // 200/10s, which paces one request every ~50ms.
        assert_paced_by_its_own_quota(
            &polymarket::clob_limits(),
            "/tick-size",
            200,
            Duration::from_secs(10),
        )
        .await;
    }

    #[tokio::test]
    async fn the_ledger_group_allowance_is_consumed_jointly() {
        // The runtime counterpart to the shared-bucket check: consuming the group
        // through one endpoint must leave a *different* group member throttled.
        // The shared 900/10s bucket paces at ~11ms; with four independent
        // buckets /orders would only meet the general 9,000/10s one at ~1.1ms.
        let rl = polymarket::clob_limits();
        rl.acquire("/trades", Some(&Method::GET)).await;

        let start = std::time::Instant::now();
        rl.acquire("/orders", Some(&Method::GET)).await;
        let waited = start.elapsed();

        assert!(
            waited >= Duration::from_millis(5),
            "GET /orders returned in {waited:?} after /trades consumed from the shared 900/10s \
             allowance — the group cap is not actually shared"
        );
    }

    #[test]
    fn post_order_is_not_throttled_by_the_ledger_group() {
        // The ledger group names `/order`, but the trading table allows POST
        // /order 5,000/10s. Both can only hold if the group cap is the ledger
        // *read*. Applying it to POST would make the published burst
        // unreachable.
        let rl = polymarket::clob_limits();
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
    use super::agreement::ResolveSpecs;
    use super::*;
    use crate::polymarket;

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
        // The sustained rate is count-1, so 0 and 1 both have to be clamped or
        // the period is divided by zero. Neither appears in any table.
        let _ = quota(0, Duration::from_secs(10));
        let _ = quota(1, Duration::from_secs(10));
    }

    // ── Factory methods ──────────────────────────────────────────

    #[test]
    fn test_clob_default_construction() {
        let rl = polymarket::clob_limits();
        assert_eq!(rl.rows().len(), 27);
        assert!(format!("{:?}", rl).contains("endpoints"));
    }

    #[test]
    fn test_gamma_default_construction() {
        let rl = polymarket::gamma_limits();
        assert_eq!(rl.rows().len(), 6);
    }

    #[test]
    fn test_data_default_construction() {
        let rl = polymarket::data_limits();
        assert_eq!(rl.rows().len(), 11);
    }

    #[test]
    fn test_relay_default_construction() {
        let rl = polymarket::relay_limits();
        assert_eq!(rl.rows().len(), 0);
    }

    #[test]
    fn test_rate_limiter_debug_format() {
        let rl = polymarket::clob_limits();
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
        let rl = polymarket::clob_limits();
        let rows = rl.rows();
        let index_of = |path: &str| {
            rows.iter()
                .position(|l| l.pattern == path)
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
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        rl.acquire("/order", Some(&Method::POST)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_matches_endpoint_by_prefix() {
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        // /order/123 should match the /order prefix
        rl.acquire("/order/123", Some(&Method::POST)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_prefix_respects_segment_boundary() {
        let rl = polymarket::clob_limits();
        let limits = rl.rows();

        // Find the /price entry
        let price_idx = limits
            .iter()
            .position(|l| l.pattern == "/price")
            .expect("/price endpoint exists");

        // /prices-history must NOT match /price — it's a different endpoint
        let prices_history_idx = limits
            .iter()
            .position(|l| l.pattern == "/prices-history")
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
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        // GET /order shouldn't match POST or DELETE /order endpoints — falls to default only
        rl.acquire("/order", Some(&Method::GET)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_no_endpoint_match_uses_default_only() {
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        rl.acquire("/unknown/path", None).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_method_none_matches_any_method() {
        let rl = polymarket::gamma_limits();
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
        let rl = polymarket::clob_limits();
        let limits = rl.rows();

        let price = limits.iter().find(|l| l.pattern == "/price").unwrap();
        let prices_history = limits
            .iter()
            .find(|l| l.pattern == "/prices-history")
            .unwrap();

        // Both should use Prefix mode
        assert_eq!(price.matching, Matching::Prefix);
        assert_eq!(prices_history.matching, Matching::Prefix);

        // Verify "/prices-history" does NOT match the "/price" pattern
        if let Some(rest) = "/prices-history".strip_prefix(price.pattern) {
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
        let rl = polymarket::data_limits();

        let closed = rl.resolve_specs("/closed-positions", Some(&Method::GET));
        let positions = rl.resolve_specs("/positions", Some(&Method::GET));
        assert_eq!(closed, positions, "both are published at 150/10s");

        let bucket_for = |path: &str| {
            rl.effective_quota(&Method::GET, path)
                .get(1)
                .unwrap_or_else(|| panic!("{path} should match a rule"))
                .bucket
        };
        assert!(
            bucket_for("/closed-positions") != bucket_for("/positions"),
            "equal quotas must still be separate buckets — upstream publishes \
             150/10s each, not 150/10s combined"
        );
    }

    #[test]
    fn test_all_clob_endpoints_have_match_mode() {
        let rl = polymarket::clob_limits();
        for limit in &rl.rows() {
            // Every endpoint should have an explicit match mode
            assert!(
                limit.matching == Matching::Prefix || limit.matching == Matching::Exact,
                "endpoint {} has no valid match mode",
                limit.pattern
            );
        }
    }

    // ── Concurrent access tests ─────────────────────────────────

    #[tokio::test]
    async fn concurrent_acquires_are_paced_against_one_shared_allowance() {
        // Concurrency must not multiply the allowance. Ten tasks racing on one
        // limiter have to serialise into ten successive slots, not each take a
        // token of their own — the limiter's state is shared, and this is the
        // assertion that says so.
        //
        // /markets is locally capped at 1,500/10s, pacing at ~6.7ms, so ten
        // acquires occupy ~60ms. The floor is the real assertion; the ceiling
        // only catches a stall.
        const TASKS: u32 = 10;
        let interval = Duration::from_secs(10) / (1_500 - 1);

        let rl = std::sync::Arc::new(polymarket::clob_limits());

        let start = std::time::Instant::now();
        let mut handles = Vec::new();
        for _ in 0..TASKS {
            let rl = rl.clone();
            handles.push(tokio::spawn(async move {
                rl.acquire("/markets", None).await;
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed >= interval * (TASKS - 1) / 2,
            "{TASKS} concurrent acquires completed in {elapsed:?}; pacing at {interval:?} each \
             they cannot, so concurrent tasks are not sharing one allowance"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "{TASKS} concurrent acquires took {elapsed:?} — they are stalling, not pacing"
        );
    }

    #[tokio::test]
    async fn test_acquire_concurrent_different_endpoints() {
        // Concurrent tasks hitting different endpoints should not block each other
        let rl = std::sync::Arc::new(polymarket::clob_limits());

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
        let rl = polymarket::clob_limits();
        let post_order = rl
            .rows()
            .into_iter()
            .find(|l| l.pattern == "/order" && l.method == Some(&Method::POST))
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
        let rl = polymarket::clob_limits();
        let delete_order = rl
            .rows()
            .into_iter()
            .find(|l| l.pattern == "/order" && l.method == Some(&Method::DELETE))
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
        let rl = polymarket::clob_limits();
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

#[cfg(test)]
mod cooldown_tests {
    //! A 429 is a fact about the host, not about the request that saw it.
    //!
    //! The token buckets above model the *published* quota, which is all a
    //! client can know in advance. When the server disagrees — Cloudflare's
    //! `error code: 1015` arrives as a 429 whatever our buckets believe — that
    //! correction has to reach every request sharing the limiter, or the
    //! siblings already in flight keep feeding a ban that is timed, and so gets
    //! longer the more it is hit.

    use super::*;
    use crate::polymarket;

    #[tokio::test(start_paused = true)]
    async fn acquire_is_immediate_without_a_cooldown() {
        let rl = polymarket::data_limits();
        let t = tokio::time::Instant::now();
        rl.acquire("/closed-positions", None).await;
        assert!(
            t.elapsed() < Duration::from_millis(1),
            "an untripped limiter must not delay: waited {:?}",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_holds_back_a_path_that_never_saw_the_429() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(5));

        // /trades has its own bucket, full and untouched. It must wait anyway:
        // the block is on the IP, and every path shares it.
        let t = tokio::time::Instant::now();
        rl.acquire("/trades", None).await;
        assert!(
            t.elapsed() >= Duration::from_secs(5),
            "a sibling path resumed after {:?}, before the cooldown expired",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_requests_all_observe_one_cooldown() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(3));

        // The shape from the report: several /closed-positions calls in flight
        // at once. One 429 has to stop all of them, not just its own caller.
        let t = tokio::time::Instant::now();
        tokio::join!(
            rl.acquire("/closed-positions", None),
            rl.acquire("/closed-positions", None),
            rl.acquire("/closed-positions", None),
            rl.acquire("/closed-positions", None),
        );
        assert!(
            t.elapsed() >= Duration::from_secs(3),
            "concurrent callers resumed after {:?}",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_shorter_cooldown_never_cuts_a_longer_one_short() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(10));
        // A sibling's 429 lands next, carrying a smaller delay. Taking the
        // latest value would let the shortest response win the race and
        // release everyone early.
        rl.begin_cooldown(Duration::from_secs(1));

        let t = tokio::time::Instant::now();
        rl.acquire("/positions", None).await;
        assert!(
            t.elapsed() >= Duration::from_secs(10),
            "the longer cooldown was truncated to {:?}",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_extended_mid_wait_is_honoured_in_full() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(2));

        let extender = {
            let rl = rl.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                rl.begin_cooldown(Duration::from_secs(5));
            })
        };

        let t = tokio::time::Instant::now();
        rl.acquire("/closed-positions", None).await;
        extender.await.unwrap();
        // Extended to 1s + 5s = 6s. Waking at the original 2s deadline and
        // returning would resume straight into the still-active ban.
        assert!(
            t.elapsed() >= Duration::from_secs(6),
            "resumed at {:?}, ignoring the cooldown extension",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_expired_cooldown_stops_delaying() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(2));
        rl.acquire("/closed-positions", None).await;

        let t = tokio::time::Instant::now();
        rl.acquire("/closed-positions", None).await;
        assert!(
            t.elapsed() < Duration::from_millis(1),
            "the limiter stayed blocked for {:?} after the cooldown expired",
            t.elapsed()
        );
    }
}
