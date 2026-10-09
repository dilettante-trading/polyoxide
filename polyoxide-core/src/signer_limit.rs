//! Per-signer token-bucket limits for CLOB trading.
//!
//! Polymarket evaluates order and cancellation requests against token buckets
//! keyed on the **signer address**, independently of the Cloudflare IP limits in
//! [`crate::rate_limit`]. A request must satisfy both layers.
//!
//! The two layers count different things: Cloudflare counts *requests*, this one
//! counts *orders*. For batch endpoints they diverge by the batch size, which is
//! why this module exists at all — a [`RateLimiter`](crate::RateLimiter) charges
//! exactly one token per call and cannot express "this request costs 500", so
//! each signer bucket is a [`CapacityBucket`], which charges a request's cost.
//!
//! Transcribed from <https://docs.polymarket.com/api-reference/trading-rate-limits>
//! as fetched 2026-08-05; mirrored in `docs/specs/clob/trading-rate-limits.md`.

use crate::capacity::CapacityBucket;
use crate::hold::Hold;
use crate::hooks::{LayerId, Refused};
use crate::polymarket::{SIGNER_CANCEL, SIGNER_ORDER};

/// Which of a signer's two buckets a request draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradingBucket {
    /// Order placement.
    Order,
    /// Order cancellation.
    Cancel,
}

/// An account's volume tier, which sets both buckets' rate and capacity.
///
/// Tier is assigned upstream from 30-day volume. Clients cannot compute it, so
/// it is discovered from the `Poly-RateLimit-Tier` response header; until one is
/// seen, [`Tier::Standard`] is assumed because it is the tightest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Tier {
    /// No volume requirement.
    #[default]
    Standard,
    /// $30,000+ 30-day volume.
    Copper,
    /// $50,000+ 30-day volume.
    Bronze,
    /// $100,000+ 30-day volume.
    Silver,
    /// $500,000+ 30-day volume.
    Gold,
    /// $2.5M+ 30-day volume.
    Platinum,
    /// $5M+ 30-day volume.
    Diamond,
    /// $10M+ 30-day volume.
    Elite,
}

impl Tier {
    /// Sustained refill rate for `bucket`, in tokens per second.
    pub fn rate(self, bucket: TradingBucket) -> u32 {
        let (order_rate, _, cancel_rate, _) = self.allowances();
        match bucket {
            TradingBucket::Order => order_rate,
            TradingBucket::Cancel => cancel_rate,
        }
    }

    /// Burst capacity for `bucket` — the most tokens it ever holds.
    ///
    /// A request costing more than this can never be satisfied, however long
    /// the caller waits.
    pub fn burst(self, bucket: TradingBucket) -> u32 {
        let (_, order_burst, _, cancel_burst) = self.allowances();
        match bucket {
            TradingBucket::Order => order_burst,
            TradingBucket::Cancel => cancel_burst,
        }
    }

    /// `(order rate, order burst, cancel rate, cancel burst)` as published.
    ///
    /// Transcribed literally rather than derived. The figures look regular —
    /// cancel is twice order throughout, burst is 1.5x rate — but Diamond
    /// breaks the second pattern (787, not 787.5), and a table copied from
    /// vendor docs is exactly where a clever formula goes stale unnoticed.
    fn allowances(self) -> (u32, u32, u32, u32) {
        match self {
            Tier::Standard => (40, 60, 80, 120),
            Tier::Copper => (60, 90, 120, 180),
            Tier::Bronze => (80, 120, 160, 240),
            Tier::Silver => (200, 300, 400, 600),
            Tier::Gold => (400, 600, 800, 1_200),
            Tier::Platinum => (450, 675, 900, 1_350),
            Tier::Diamond => (525, 787, 1_050, 1_575),
            Tier::Elite => (600, 900, 1_200, 1_800),
        }
    }

    /// Parse the value of a `Poly-RateLimit-Tier` header.
    ///
    /// Matching is case-insensitive; an unrecognised tier returns `None` rather
    /// than guessing, so an upstream addition cannot silently widen a bucket.
    pub fn from_header(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "standard" => Some(Tier::Standard),
            "copper" => Some(Tier::Copper),
            "bronze" => Some(Tier::Bronze),
            "silver" => Some(Tier::Silver),
            "gold" => Some(Tier::Gold),
            "platinum" => Some(Tier::Platinum),
            "diamond" => Some(Tier::Diamond),
            "elite" => Some(Tier::Elite),
            _ => None,
        }
    }
}

/// A trading request, carrying whatever its token cost depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingRequest {
    /// `POST /order` — always 1 token.
    PostOrder,
    /// `POST /orders` — one token per order in the batch.
    PostOrders {
        /// Number of orders in the batch.
        count: u32,
    },
    /// `DELETE /order` — always 1 token.
    CancelOrder,
    /// `DELETE /orders` — one token per submitted order ID.
    CancelOrders {
        /// Number of order IDs submitted.
        count: u32,
    },
    /// `DELETE /cancel-all` — costs `1 + orders canceled`.
    ///
    /// The true cost is not knowable client-side; see [`TradingRequest::cost`].
    CancelAll,
    /// `DELETE /cancel-market-orders` — costs `1 + matching orders canceled`.
    ///
    /// The true cost is not knowable client-side; see [`TradingRequest::cost`].
    CancelMarketOrders,
}

impl TradingRequest {
    /// Which bucket this request draws from.
    pub fn bucket(self) -> TradingBucket {
        match self {
            TradingRequest::PostOrder | TradingRequest::PostOrders { .. } => TradingBucket::Order,
            TradingRequest::CancelOrder
            | TradingRequest::CancelOrders { .. }
            | TradingRequest::CancelAll
            | TradingRequest::CancelMarketOrders => TradingBucket::Cancel,
        }
    }

    /// The token cost, as far as the client can know it.
    ///
    /// Exact for the four request kinds whose cost is a function of the payload.
    /// For [`CancelAll`](Self::CancelAll) and
    /// [`CancelMarketOrders`](Self::CancelMarketOrders) the published cost is
    /// `1 + orders canceled`, and the client does not know how many orders are
    /// open — so this returns the floor of 1. Those two can therefore overdraw
    /// the real bucket, and a resulting 429 is genuine rather than a client bug.
    pub fn cost(self) -> u32 {
        match self {
            TradingRequest::PostOrder | TradingRequest::CancelOrder => 1,
            TradingRequest::PostOrders { count } | TradingRequest::CancelOrders { count } => count,
            // Floor of the published `1 + orders canceled`.
            TradingRequest::CancelAll | TradingRequest::CancelMarketOrders => 1,
        }
    }

    /// Whether this request's cost is exactly known client-side.
    ///
    /// Only requests where this is true can be safely rejected before sending;
    /// the rest must be attempted and may come back 429.
    pub fn cost_is_exact(self) -> bool {
        !matches!(
            self,
            TradingRequest::CancelAll | TradingRequest::CancelMarketOrders
        )
    }
}

/// What the venue reported about a signer's trading allowance on one response.
///
/// Polymarket returns these on every evaluated order/cancel request. They are
/// the only way a client learns its own tier, since tier derives from 30-day
/// volume that the client cannot compute.
///
/// Every field is optional: the headers are absent on responses that never
/// reached the trading limiter (market data, auth, anything non-trading), and a
/// malformed value is dropped rather than guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RateLimitStatus {
    /// `Poly-RateLimit-Remaining` — token balance after this request.
    pub remaining: Option<u32>,
    /// `Poly-RateLimit-Reset` — Unix timestamp when the wait period ends.
    pub reset: Option<u64>,
    /// `Poly-RateLimit-Tier` — the tier the venue applied.
    pub tier: Option<Tier>,
    /// `Poly-RateLimit-Warning` — true when the venue is in warning mode.
    pub warning: bool,
}

impl RateLimitStatus {
    /// Read the `Poly-RateLimit-*` family from a response's headers.
    ///
    /// Returns an all-`None` value rather than an error when the headers are
    /// absent, so callers need not distinguish "not a trading request" from
    /// "trading request with no telemetry".
    pub fn from_headers(headers: &reqwest::header::HeaderMap) -> Self {
        let get = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());

        Self {
            remaining: get("poly-ratelimit-remaining").and_then(|v| v.trim().parse().ok()),
            reset: get("poly-ratelimit-reset").and_then(|v| v.trim().parse().ok()),
            tier: get("poly-ratelimit-tier").and_then(Tier::from_header),
            warning: get("poly-ratelimit-warning")
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
        }
    }

    /// Whether the venue reported anything at all.
    pub fn is_empty(&self) -> bool {
        self.remaining.is_none() && self.reset.is_none() && self.tier.is_none() && !self.warning
    }
}

/// A request whose token cost exceeds its bucket's capacity.
///
/// This is **not** a throttle. A token bucket never holds more than its burst
/// capacity, so a request costing more can never be satisfied no matter how long
/// the caller waits. Splitting the batch is the only remedy, which is why this
/// is a distinct error rather than a 429 the retry loop would burn attempts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "batch costs {cost} tokens but the {bucket:?} bucket at tier {tier:?} holds at most \
     {capacity}; this can never succeed — split it into batches of {capacity} or fewer"
)]
pub struct BurstCapacityExceeded {
    /// Tokens the request would cost.
    pub cost: u32,
    /// The bucket's maximum capacity at the current tier.
    pub capacity: u32,
    /// Tier in force when the request was rejected.
    pub tier: Tier,
    /// Which bucket the request draws from.
    pub bucket: TradingBucket,
}

/// [`polyoxide_venue::Class::InvalidRequest`]: the client refused a batch no
/// bucket can ever hold, and nothing was sent.
impl polyoxide_venue::Classify for BurstCapacityExceeded {
    fn class(&self) -> polyoxide_venue::Class {
        polyoxide_venue::Class::InvalidRequest
    }
}

struct Buckets {
    tier: Tier,
    order: CapacityBucket,
    cancel: CapacityBucket,
}

impl Buckets {
    /// Both buckets at `tier`, full, waiting out `hold`.
    ///
    /// Each holds its published burst. Polymarket publishes a rate *and* a
    /// burst for this layer, and says the burst is the bucket's capacity, so
    /// holding it is a faithful model of a bucket the venue also implements as
    /// one. The Cloudflare tables publish a window count with no capacity
    /// term, and a bucket holding that count spends it twice; the two models
    /// must not be made alike.
    fn for_tier(tier: Tier, hold: &Hold) -> Self {
        let build = |bucket: TradingBucket, layer: LayerId| {
            let capacity = tier.burst(bucket).max(1);
            CapacityBucket::new(layer, capacity, tier.rate(bucket).max(1), hold.clone())
        };
        Self {
            tier,
            order: build(TradingBucket::Order, SIGNER_ORDER),
            cancel: build(TradingBucket::Cancel, SIGNER_CANCEL),
        }
    }

    fn get(&self, bucket: TradingBucket) -> &CapacityBucket {
        match bucket {
            TradingBucket::Order => &self.order,
            TradingBucket::Cancel => &self.cancel,
        }
    }
}

/// Per-signer trading limiter, with the tier discovered from response headers.
///
/// Starts at [`Tier::Standard`] — the tightest — and resizes both buckets the
/// first time a `Poly-RateLimit-Tier` header reports something different.
///
/// # A new tier starts full
///
/// Adopting a new tier replaces both buckets with fresh ones at full capacity,
/// keeping the [`Hold`]. Moving *up* a tier is therefore safe: the venue
/// already permits the wider allowance. Moving *down* briefly permits a full
/// burst at the narrower capacity, which the venue may throttle. Tier changes
/// are rare (they track 30-day volume), so this is preferred to carrying the
/// old bucket's tokens into the new one, which
/// [`CapacityBucket::resize`] would do.
#[derive(Clone)]
pub struct SignerLimiter {
    inner: std::sync::Arc<SignerLimiterInner>,
}

struct SignerLimiterInner {
    hold: Hold,
    buckets: std::sync::RwLock<Buckets>,
    status: std::sync::RwLock<RateLimitStatus>,
}

impl std::fmt::Debug for SignerLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignerLimiter")
            .field("tier", &self.tier())
            .finish()
    }
}

impl Default for SignerLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl SignerLimiter {
    /// Create a limiter at the default (tightest) tier, with a hold of its
    /// own.
    pub fn new() -> Self {
        Self::build(
            Tier::default(),
            RateLimitStatus::default(),
            Hold::unbounded(),
        )
    }

    /// This limiter, waiting out `hold`, so that it holds together with every
    /// other layer sharing it. The tier and telemetry carry over.
    ///
    /// The result is a new limiter: clones of `self` keep their own buckets
    /// and hold.
    pub fn with_hold(self, hold: Hold) -> Self {
        Self::build(self.tier(), self.last_status(), hold)
    }

    fn build(tier: Tier, status: RateLimitStatus, hold: Hold) -> Self {
        Self {
            inner: std::sync::Arc::new(SignerLimiterInner {
                buckets: std::sync::RwLock::new(Buckets::for_tier(tier, &hold)),
                hold,
                status: std::sync::RwLock::new(status),
            }),
        }
    }

    /// The hold both buckets wait out.
    pub fn hold(&self) -> &Hold {
        &self.inner.hold
    }

    /// The tier currently in force.
    pub fn tier(&self) -> Tier {
        self.inner
            .buckets
            .read()
            .expect("lock is never poisoned")
            .tier
    }

    /// The most recent `Poly-RateLimit-*` telemetry seen.
    pub fn last_status(&self) -> RateLimitStatus {
        *self.inner.status.read().expect("lock is never poisoned")
    }

    /// Record the rate-limit headers from a response, adopting a reported tier.
    ///
    /// Responses carrying none of the headers are ignored, so non-trading
    /// requests cannot clear the telemetry.
    pub fn observe(&self, headers: &reqwest::header::HeaderMap) {
        let status = RateLimitStatus::from_headers(headers);
        if status.is_empty() {
            return;
        }
        *self.inner.status.write().expect("lock is never poisoned") = status;

        // An unrecognised tier parses to None and so leaves the buckets alone,
        // keeping the tighter allowance rather than guessing a wider one.
        if let Some(tier) = status.tier {
            let mut buckets = self.inner.buckets.write().expect("lock is never poisoned");
            if buckets.tier != tier {
                tracing::debug!("adopting rate limit tier {tier:?} (was {:?})", buckets.tier);
                *buckets = Buckets::for_tier(tier, &self.inner.hold);
            }
        }
    }

    /// Wait for `request`'s token cost to be available, then consume it.
    ///
    /// Waits out the hold first, and again after waiting for tokens. A cost
    /// that is not exact ([`TradingRequest::cost_is_exact`]) above the
    /// bucket's capacity is charged at capacity.
    ///
    /// # Errors
    ///
    /// [`BurstCapacityExceeded`] when an exact cost exceeds the bucket's
    /// capacity. This returns immediately rather than waiting forever.
    pub async fn acquire(&self, request: TradingRequest) -> Result<(), BurstCapacityExceeded> {
        let bucket = request.bucket();

        let (tier, limiter) = self.bucket(bucket);
        let cost = request.cost().max(1);
        // The bucket refuses an exact cost above its capacity at once: the
        // permanently-impossible case, returned straight away rather than
        // parking the task forever.
        limiter
            .acquire(cost, request.cost_is_exact())
            .await
            .map_err(|_| BurstCapacityExceeded {
                cost,
                capacity: tier.burst(bucket),
                tier,
                bucket,
            })
    }

    /// Charge `bucket` `units` tokens, as [`acquire`](Self::acquire) does for
    /// a request: the per-signer layer of
    /// [`ClobThrottle`](crate::polymarket::ClobThrottle), which is handed a
    /// cost rather than a request.
    pub(crate) async fn charge(
        &self,
        bucket: TradingBucket,
        units: u32,
        exact: bool,
    ) -> Result<(), Refused> {
        let (_, limiter) = self.bucket(bucket);
        limiter.acquire(units, exact).await
    }

    /// The tier in force and a handle on one of its buckets.
    ///
    /// Clones the bucket out under the lock and drops the guard before the
    /// caller awaits: an RwLock guard held across an await would make the
    /// future !Send.
    fn bucket(&self, bucket: TradingBucket) -> (Tier, CapacityBucket) {
        let buckets = self.inner.buckets.read().expect("lock is never poisoned");
        (buckets.tier, buckets.get(bucket).clone())
    }
}

#[cfg(test)]
mod limiter_tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::time::Duration;

    fn tier_header(tier: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(
            HeaderName::from_static("poly-ratelimit-tier"),
            HeaderValue::from_str(tier).unwrap(),
        );
        map
    }

    #[test]
    fn starts_at_the_tightest_tier() {
        assert_eq!(SignerLimiter::new().tier(), Tier::Standard);
    }

    #[test]
    fn observing_a_tier_header_adopts_it() {
        let limiter = SignerLimiter::new();
        limiter.observe(&tier_header("gold"));
        assert_eq!(limiter.tier(), Tier::Gold);
        assert_eq!(limiter.last_status().tier, Some(Tier::Gold));
    }

    #[test]
    fn an_unrecognised_tier_leaves_the_current_one_in_force() {
        // Adopting an unknown tier would mean guessing a capacity. Staying put
        // keeps the tighter, known-safe allowance.
        let limiter = SignerLimiter::new();
        limiter.observe(&tier_header("silver"));
        limiter.observe(&tier_header("titanium"));
        assert_eq!(limiter.tier(), Tier::Silver);
    }

    #[test]
    fn a_response_without_the_headers_does_not_clear_telemetry() {
        let limiter = SignerLimiter::new();
        limiter.observe(&tier_header("silver"));
        limiter.observe(&HeaderMap::new());
        assert_eq!(limiter.tier(), Tier::Silver);
        assert_eq!(limiter.last_status().tier, Some(Tier::Silver));
    }

    #[tokio::test]
    async fn an_over_capacity_batch_is_rejected_immediately_not_queued() {
        // The headline case: 2,000 IDs exceeds every tier's cancel burst, so
        // waiting can never help. It must come back as an error, fast.
        let limiter = SignerLimiter::new();
        let request = TradingRequest::CancelOrders { count: 2_000 };

        let result = tokio::time::timeout(Duration::from_millis(100), limiter.acquire(request))
            .await
            .expect("must not hang waiting for capacity that can never exist");

        let err = result.expect_err("2,000 tokens exceeds Standard's 120 cancel burst");
        assert_eq!(err.cost, 2_000);
        assert_eq!(err.capacity, 120);
        assert_eq!(err.bucket, TradingBucket::Cancel);
    }

    #[tokio::test]
    async fn a_batch_within_capacity_is_admitted() {
        let limiter = SignerLimiter::new();
        limiter
            .acquire(TradingRequest::CancelOrders { count: 100 })
            .await
            .expect("100 fits Standard's 120 cancel burst");
    }

    #[tokio::test]
    async fn adopting_a_higher_tier_admits_a_batch_that_was_impossible() {
        // Proves the resize actually changes capacity rather than just the
        // reported tier: 500 IDs is impossible at Standard (120) and fine at
        // Gold (1,200).
        let limiter = SignerLimiter::new();
        let batch = TradingRequest::CancelOrders { count: 500 };
        assert!(limiter.acquire(batch).await.is_err());

        limiter.observe(&tier_header("gold"));
        limiter
            .acquire(batch)
            .await
            .expect("500 fits Gold's 1,200 cancel burst");
    }

    #[tokio::test]
    async fn the_order_and_cancel_buckets_are_independent() {
        // Draining orders must not throttle cancels — they are separate buckets
        // upstream, and conflating them would block cancels during a burst of
        // order placement, which is exactly when cancelling matters most.
        let limiter = SignerLimiter::new();
        limiter
            .acquire(TradingRequest::PostOrders { count: 60 })
            .await
            .expect("60 fills Standard's order burst exactly");

        let start = std::time::Instant::now();
        limiter
            .acquire(TradingRequest::CancelOrder)
            .await
            .expect("cancel bucket is untouched");
        assert!(
            start.elapsed() < Duration::from_millis(25),
            "cancelling was throttled by order placement"
        );
    }

    #[tokio::test]
    async fn batch_cost_is_charged_in_full_not_as_one_request() {
        // The whole point of this layer. Draining the order burst with one
        // 60-order batch must leave the next single order waiting — if batches
        // were charged as 1, this would return instantly.
        let limiter = SignerLimiter::new();
        limiter
            .acquire(TradingRequest::PostOrders { count: 60 })
            .await
            .unwrap();

        let start = std::time::Instant::now();
        limiter.acquire(TradingRequest::PostOrder).await.unwrap();
        assert!(
            start.elapsed() >= Duration::from_millis(10),
            "a 60-order batch was charged as a single token"
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(
                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        map
    }

    #[test]
    fn reads_the_full_header_family() {
        let status = RateLimitStatus::from_headers(&headers(&[
            ("poly-ratelimit-remaining", "57"),
            ("poly-ratelimit-reset", "1767225660"),
            ("poly-ratelimit-tier", "silver"),
            ("poly-ratelimit-warning", "true"),
        ]));

        assert_eq!(status.remaining, Some(57));
        assert_eq!(status.reset, Some(1_767_225_660));
        assert_eq!(status.tier, Some(Tier::Silver));
        assert!(status.warning);
        assert!(!status.is_empty());
    }

    #[test]
    fn header_names_are_matched_case_insensitively() {
        // reqwest lowercases header names on receipt, but a HeaderMap built by
        // hand (or a proxy preserving case) must resolve identically.
        let status = RateLimitStatus::from_headers(&headers(&[
            ("Poly-RateLimit-Tier", "GOLD"),
            ("POLY-RATELIMIT-REMAINING", "3"),
        ]));
        assert_eq!(status.tier, Some(Tier::Gold));
        assert_eq!(status.remaining, Some(3));
    }

    #[test]
    fn absent_headers_yield_an_empty_status_not_an_error() {
        let status = RateLimitStatus::from_headers(&HeaderMap::new());
        assert!(status.is_empty());
        assert_eq!(status, RateLimitStatus::default());
    }

    #[test]
    fn malformed_values_are_dropped_rather_than_guessed() {
        // A garbage tier must not fall back to Standard here — that would be
        // indistinguishable from the venue actually reporting Standard, and the
        // limiter would resize a bucket on noise.
        let status = RateLimitStatus::from_headers(&headers(&[
            ("poly-ratelimit-remaining", "not-a-number"),
            ("poly-ratelimit-reset", ""),
            ("poly-ratelimit-tier", "titanium"),
            ("poly-ratelimit-warning", "false"),
        ]));

        assert_eq!(status.remaining, None);
        assert_eq!(status.reset, None);
        assert_eq!(status.tier, None);
        assert!(!status.warning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published tier table, transcribed by hand. This is the golden vector.
    ///
    /// `(tier, order rate, order burst, cancel rate, cancel burst)`
    fn published() -> Vec<(Tier, u32, u32, u32, u32)> {
        vec![
            (Tier::Standard, 40, 60, 80, 120),
            (Tier::Copper, 60, 90, 120, 180),
            (Tier::Bronze, 80, 120, 160, 240),
            (Tier::Silver, 200, 300, 400, 600),
            (Tier::Gold, 400, 600, 800, 1_200),
            (Tier::Platinum, 450, 675, 900, 1_350),
            (Tier::Diamond, 525, 787, 1_050, 1_575),
            (Tier::Elite, 600, 900, 1_200, 1_800),
        ]
    }

    #[test]
    fn every_tier_matches_the_published_table() {
        for (tier, o_rate, o_burst, c_rate, c_burst) in published() {
            assert_eq!(
                tier.rate(TradingBucket::Order),
                o_rate,
                "{tier:?} order rate"
            );
            assert_eq!(
                tier.burst(TradingBucket::Order),
                o_burst,
                "{tier:?} order burst"
            );
            assert_eq!(
                tier.rate(TradingBucket::Cancel),
                c_rate,
                "{tier:?} cancel rate"
            );
            assert_eq!(
                tier.burst(TradingBucket::Cancel),
                c_burst,
                "{tier:?} cancel burst"
            );
        }
    }

    #[test]
    fn the_default_tier_is_the_tightest_one() {
        // An unconfigured client must never assume more allowance than it has.
        let default = Tier::default();
        for (tier, ..) in published() {
            assert!(
                default.rate(TradingBucket::Order) <= tier.rate(TradingBucket::Order),
                "default tier {default:?} is looser than {tier:?}"
            );
            assert!(
                default.burst(TradingBucket::Cancel) <= tier.burst(TradingBucket::Cancel),
                "default tier {default:?} bursts higher than {tier:?}"
            );
        }
    }

    #[test]
    fn tier_headers_parse_case_insensitively() {
        assert_eq!(Tier::from_header("standard"), Some(Tier::Standard));
        assert_eq!(Tier::from_header("Silver"), Some(Tier::Silver));
        assert_eq!(Tier::from_header("ELITE"), Some(Tier::Elite));
    }

    #[test]
    fn an_unknown_tier_header_is_not_guessed() {
        // Guessing would widen a bucket on an upstream addition we know nothing
        // about. Returning None keeps the current (tighter) tier in force.
        assert_eq!(Tier::from_header("titanium"), None);
        assert_eq!(Tier::from_header(""), None);
    }

    #[test]
    fn batch_costs_scale_with_the_payload() {
        assert_eq!(TradingRequest::PostOrder.cost(), 1);
        assert_eq!(TradingRequest::PostOrders { count: 40 }.cost(), 40);
        assert_eq!(TradingRequest::CancelOrder.cost(), 1);
        assert_eq!(TradingRequest::CancelOrders { count: 250 }.cost(), 250);
    }

    #[test]
    fn requests_draw_from_the_right_bucket() {
        assert_eq!(TradingRequest::PostOrder.bucket(), TradingBucket::Order);
        assert_eq!(
            TradingRequest::PostOrders { count: 2 }.bucket(),
            TradingBucket::Order
        );
        assert_eq!(TradingRequest::CancelOrder.bucket(), TradingBucket::Cancel);
        assert_eq!(
            TradingRequest::CancelOrders { count: 2 }.bucket(),
            TradingBucket::Cancel
        );
        assert_eq!(TradingRequest::CancelAll.bucket(), TradingBucket::Cancel);
        assert_eq!(
            TradingRequest::CancelMarketOrders.bucket(),
            TradingBucket::Cancel
        );
    }

    #[test]
    fn cancel_all_reports_a_floor_cost_and_says_so() {
        // Published cost is 1 + orders canceled, which the client cannot know.
        // Reporting 1 while flagging it inexact is the honest answer; claiming
        // exactness here would let the guard reject or admit batches wrongly.
        assert_eq!(TradingRequest::CancelAll.cost(), 1);
        assert!(!TradingRequest::CancelAll.cost_is_exact());
        assert_eq!(TradingRequest::CancelMarketOrders.cost(), 1);
        assert!(!TradingRequest::CancelMarketOrders.cost_is_exact());

        for exact in [
            TradingRequest::PostOrder,
            TradingRequest::PostOrders { count: 3 },
            TradingRequest::CancelOrder,
            TradingRequest::CancelOrders { count: 3 },
        ] {
            assert!(exact.cost_is_exact(), "{exact:?} cost is computable");
        }
    }

    #[test]
    fn a_batch_larger_than_elite_burst_is_impossible_on_every_tier() {
        // The finding that motivated this module: a token bucket never holds
        // more than its capacity, so an over-capacity batch is permanently
        // rejected rather than throttled. 2,000 IDs exceeds every cancel burst.
        let batch = TradingRequest::CancelOrders { count: 2_000 };
        for (tier, ..) in published() {
            assert!(
                batch.cost() > tier.burst(batch.bucket()),
                "{tier:?} could absorb a 2,000-ID batch — check the published table"
            );
        }
    }

    #[test]
    fn an_over_capacity_batch_is_an_invalid_request_and_not_retriable() {
        use polyoxide_venue::{Class, Classify};

        let err = BurstCapacityExceeded {
            cost: 2_000,
            capacity: 120,
            tier: Tier::Standard,
            bucket: TradingBucket::Cancel,
        };
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(err.is_fault());
        assert_eq!(err.retry_after(), None);
        assert!(!err.is_retriable());
    }
}
