//! Binance's request-weight budget, and the separate limit on the funding routes.
//!
//! Binance charges each REST request a *weight* that depends on its route and
//! its parameters, and refuses an IP whose weight in the current minute passes
//! `exchangeInfo`'s `REQUEST_WEIGHT` limit of 2400. Core's `RateLimiter` counts
//! requests per path, so it cannot model a cost that varies with `limit`;
//! [`WeightBudget`] does. The table is [`Route::cost`]; the measurements behind
//! it are in `docs/specs/binance/OBSERVED.md`.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use polyoxide_core::{
    AttemptInfo, Charge, Hold, LayerCharge, LayerId, Refused, RequestMeta, ResponseMeta, Throttle,
    WindowQuotaTable,
};
use tokio::time::Instant;

use crate::usdm::request::USED_WEIGHT_HEADER;
use crate::usdm::types::DepthLimit;

/// `exchangeInfo`'s `REQUEST_WEIGHT` limit per minute, per IP.
pub const PUBLISHED_WEIGHT_PER_MINUTE: u32 = 2400;

/// Requests per IP every five minutes that `fundingRate` and `fundingInfo`
/// share, as documented. Neither route reports a weight.
pub const PUBLISHED_FUNDING_PER_FIVE_MINUTES: u32 = 500;

/// How long a `418` holds every request when it carries no `Retry-After`: the
/// shortest ban Binance documents.
pub const DEFAULT_BAN: Duration = Duration::from_secs(120);

/// The longest a cooldown can last: the longest ban Binance documents. A
/// `Retry-After` beyond it is clamped.
pub const MAX_COOLDOWN: Duration = Duration::from_secs(3 * 24 * 60 * 60);

/// Reciprocal of the share of a published limit the client leaves unused, as
/// in core's `RESERVED_FRACTION`: aiming at a published quota is a bug, because
/// the server's count and the client's never agree exactly.
const RESERVED_FRACTION: u32 = 10;

const MINUTE_MS: u64 = 60_000;
const FUNDING_PERIOD: Duration = Duration::from_secs(300);

fn after_reserve(published: u32) -> u32 {
    published - published.div_ceil(RESERVED_FRACTION)
}

/// The per-minute request-weight layer of a [`WeightBudget`], as a request's
/// [`polyoxide_core::Cost`] names it.
pub const WEIGHT_LAYER: LayerId = LayerId("binance-weight");

/// The funding routes' own layer of a [`WeightBudget`], as a request's
/// [`polyoxide_core::Cost`] names it.
pub const FUNDING_LAYER: LayerId = LayerId("binance-funding");

/// What one request costs, and which limit it draws on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// Request weight, against the per-minute budget.
    Weight(u32),
    /// One request against the funding routes' own limit.
    Funding,
}

/// The cost a request builder hands core's send loop: its weight against
/// [`WEIGHT_LAYER`], or one request against [`FUNDING_LAYER`]. Both are exact.
impl From<Cost> for polyoxide_core::Cost {
    fn from(cost: Cost) -> Self {
        let (layer, units) = match cost {
            Cost::Weight(weight) => (WEIGHT_LAYER, weight),
            Cost::Funding => (FUNDING_LAYER, 1),
        };
        Self {
            layer,
            units,
            exact: true,
        }
    }
}

/// A REST route, with the parameters its weight depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Route {
    /// `GET /fapi/v1/ping`.
    Ping,
    /// `GET /fapi/v1/time`.
    Time,
    /// `GET /fapi/v1/exchangeInfo`.
    ExchangeInfo,
    /// `GET /fapi/v1/fundingInfo`.
    FundingInfo,
    /// `GET /fapi/v1/ticker/24hr`: one symbol, or every symbol when `all`.
    Ticker24h {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /fapi/v1/premiumIndex`: one symbol, or every symbol when `all`.
    PremiumIndex {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /fapi/v1/klines`.
    Klines {
        /// The `limit` parameter, if sent.
        limit: Option<u32>,
    },
    /// `GET /fapi/v1/fundingRate`.
    FundingRate,
    /// `GET /fapi/v1/openInterest`.
    OpenInterest,
    /// `GET /fapi/v1/aggTrades`.
    AggTrades,
    /// `GET /fapi/v1/depth`.
    Depth {
        /// The `limit` parameter, if sent.
        limit: Option<DepthLimit>,
    },
}

impl Route {
    /// The route's path on `fapi.binance.com`.
    pub fn path(self) -> &'static str {
        match self {
            Self::Ping => "/fapi/v1/ping",
            Self::Time => "/fapi/v1/time",
            Self::ExchangeInfo => "/fapi/v1/exchangeInfo",
            Self::FundingInfo => "/fapi/v1/fundingInfo",
            Self::Ticker24h { .. } => "/fapi/v1/ticker/24hr",
            Self::PremiumIndex { .. } => "/fapi/v1/premiumIndex",
            Self::Klines { .. } => "/fapi/v1/klines",
            Self::FundingRate => "/fapi/v1/fundingRate",
            Self::OpenInterest => "/fapi/v1/openInterest",
            Self::AggTrades => "/fapi/v1/aggTrades",
            Self::Depth { .. } => "/fapi/v1/depth",
        }
    }

    /// What the request costs, as measured from `X-MBX-USED-WEIGHT-1M` deltas
    /// on 2026-10-07.
    ///
    /// Two rows differ from Binance's page. `klines` bands are inclusive at the
    /// top (a limit of 100 costs 1, 500 costs 2, 1000 costs 5), and omitting
    /// `limit` costs 5 although it returns 500 rows. `depth` without `limit`
    /// costs 1 and returns 500 levels, where an explicit 500 costs 10.
    pub fn cost(self) -> Cost {
        match self {
            Self::Ping | Self::Time | Self::ExchangeInfo | Self::OpenInterest => Cost::Weight(1),
            Self::FundingInfo | Self::FundingRate => Cost::Funding,
            Self::Ticker24h { all } => Cost::Weight(if all { 40 } else { 1 }),
            Self::PremiumIndex { all } => Cost::Weight(if all { 10 } else { 1 }),
            Self::Klines { limit: None } => Cost::Weight(5),
            Self::Klines { limit: Some(limit) } => Cost::Weight(match limit {
                0..=100 => 1,
                101..=500 => 2,
                501..=1000 => 5,
                _ => 10,
            }),
            Self::AggTrades => Cost::Weight(20),
            Self::Depth { limit: None } => Cost::Weight(1),
            Self::Depth { limit: Some(limit) } => Cost::Weight(match limit {
                DepthLimit::Five | DepthLimit::Ten | DepthLimit::Twenty | DepthLimit::Fifty => 2,
                DepthLimit::Hundred => 5,
                DepthLimit::FiveHundred => 10,
                DepthLimit::Thousand => 20,
            }),
        }
    }
}

/// The request-weight budget of one IP, shared by every client that clones it.
///
/// Binance limits weight per IP, not per client, so every
/// [`Usdm`](crate::Usdm) in a process should be built with one budget
/// ([`UsdmBuilder::weight_budget`](crate::UsdmBuilder::weight_budget)).
///
/// - **Window.** The UTC clock minute: the server's count fell to 1 just after
///   each minute boundary on 2026-10-07, where a sliding window would have
///   kept counting.
/// - **Budget.** The published 2400 less a tenth, 2160. A request waits while
///   its weight would take the minute past that.
/// - **Server count.** Every response's `X-MBX-USED-WEIGHT-1M` raises the
///   minute's count to at least what the server reports, because other
///   processes on the same IP spend the same budget. The count never falls
///   within a minute, and a response is applied only to the minute its request
///   was charged in.
/// - **Funding.** `fundingRate` and `fundingInfo` carry no weight and share a
///   documented 500 requests per 5 minutes, paced here at 450 with a depth of
///   one, so they never burst, not even when a cooldown ends.
/// - **Cooldown.** A `429` or `418` holds every request, on both limits, for
///   the delay the server asked for. A cooldown is core's [`Hold`], with a
///   ceiling of [`MAX_COOLDOWN`], so it is only ever extended.
/// - **Throttle.** The budget is the [`Throttle`] every [`Usdm`](crate::Usdm)
///   built with it sends through: a request's weight or funding cost is
///   charged before it is sent, and each response's `X-MBX-USED-WEIGHT-1M` is
///   applied to the minute it was charged in.
/// - **In flight.** A response's header is applied only to the minute its
///   request was charged in, so a request in flight across a minute boundary is
///   counted by the server in a minute this budget no longer sees. The reserve
///   of 240 absorbs that while the weight in flight stays under it: one `Usdm`
///   at its default of 4 concurrent requests has at most 160 in flight (4 × the
///   heaviest route, 40). A higher `max_concurrent`, or several clients sharing
///   one budget, can pass it.
#[derive(Debug, Clone)]
pub struct WeightBudget {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    per_minute: u32,
    funding_interval: Duration,
    clock: Clock,
    hold: Hold,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    /// The UTC minute (milliseconds / 60 000) that `used` counts.
    minute: u64,
    used: u32,
    next_funding: Option<Instant>,
}

/// The minute a request was charged in, so its response's header is applied to
/// that minute and no other. `None` for the funding routes, which report no
/// weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MinuteCharge {
    minute: Option<u64>,
}

/// Where the current UTC minute comes from.
#[derive(Debug, Clone)]
enum Clock {
    /// The system clock, which is what Binance's minute follows.
    System,
    /// Wall time derived from tokio's clock, so a paused-time test controls it.
    #[cfg(test)]
    Tokio { start: Instant, start_ms: u64 },
    /// A clock a test sets by hand, to step it backwards.
    #[cfg(test)]
    Manual(Arc<std::sync::atomic::AtomicU64>),
}

impl Clock {
    fn now_ms(&self) -> u64 {
        match self {
            Self::System => polyoxide_venue::UnixMillis::now().0,
            #[cfg(test)]
            Self::Tokio { start, start_ms } => *start_ms + start.elapsed().as_millis() as u64,
            #[cfg(test)]
            Self::Manual(ms) => ms.load(std::sync::atomic::Ordering::SeqCst),
        }
    }
}

impl Default for WeightBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightBudget {
    /// A budget for Binance's published limits, less the reserve.
    pub fn new() -> Self {
        Self::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::System,
        )
    }

    fn with_clock(weight_per_minute: u32, funding_per_five_minutes: u32, clock: Clock) -> Self {
        Self {
            inner: Arc::new(Inner {
                per_minute: after_reserve(weight_per_minute),
                // One slot of the funding target is the bucket's depth; the
                // rest is paced, so no five-minute window admits more than the
                // target. Core's one pacing formula, on this budget's own
                // clock.
                funding_interval: WindowQuotaTable::paced_interval(
                    funding_per_five_minutes,
                    FUNDING_PERIOD,
                ),
                clock,
                hold: Hold::with_ceiling(MAX_COOLDOWN),
                state: Mutex::new(State {
                    minute: 0,
                    used: 0,
                    next_funding: None,
                }),
            }),
        }
    }

    /// A budget on a clock a test sets by hand, starting at `start_ms`.
    #[cfg(test)]
    pub(crate) fn at_ms(start_ms: u64) -> Self {
        Self::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::Manual(Arc::new(std::sync::atomic::AtomicU64::new(start_ms))),
        )
    }

    /// Whether two handles charge one budget.
    #[cfg(test)]
    pub(crate) fn is_shared_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// The most weight this budget spends in one minute.
    pub fn per_minute(&self) -> u32 {
        self.inner.per_minute
    }

    /// Weight charged or reported in the current UTC minute.
    pub fn used(&self) -> u32 {
        let minute = self.inner.clock.now_ms() / MINUTE_MS;
        let state = self.lock();
        if minute > state.minute {
            0
        } else {
            state.used
        }
    }

    /// Waits until the request can go, then charges it.
    pub(crate) async fn acquire(&self, cost: Cost) -> MinuteCharge {
        match cost {
            Cost::Funding => loop {
                // Wait out a cooldown before taking a slot: a slot taken first
                // and slept on through a cooldown is in the past when it ends,
                // and every request parked that way would go at once.
                self.await_cooldown().await;
                let slot = self.reserve_funding_slot();
                tokio::time::sleep_until(slot).await;
                if !self.in_cooldown() {
                    return MinuteCharge { minute: None };
                }
                // A cooldown began while this request waited for its slot: the
                // slot is spent, and the request queues again behind it.
            },
            Cost::Weight(weight) => loop {
                self.await_cooldown().await;
                match self.try_charge(weight) {
                    Ok(charge) => return charge,
                    Err(until_next_minute) => tokio::time::sleep(until_next_minute).await,
                }
            },
        }
    }

    /// Charges `weight` to the current minute, or says how long until the next.
    ///
    /// An empty minute admits any weight, so a request heavier than the whole
    /// budget is sent alone rather than held forever.
    fn try_charge(&self, weight: u32) -> Result<MinuteCharge, Duration> {
        let now_ms = self.inner.clock.now_ms();
        let mut state = self.lock();
        // The minute only moves forward. A read that lands in an earlier minute,
        // from a stale read racing a boundary or a clock stepped back, charges
        // the newest minute seen instead of resetting its count.
        let minute = now_ms / MINUTE_MS;
        if minute > state.minute {
            state.minute = minute;
            state.used = 0;
        }
        if state.used == 0 || state.used.saturating_add(weight) <= self.inner.per_minute {
            state.used = state.used.saturating_add(weight);
            Ok(MinuteCharge {
                minute: Some(state.minute),
            })
        } else {
            Err(Duration::from_millis(MINUTE_MS - now_ms % MINUTE_MS))
        }
    }

    /// Raises the minute's count to what the server reported for it.
    pub(crate) fn record_used(&self, charge: MinuteCharge, used: u32) {
        let Some(minute) = charge.minute else { return };
        let mut state = self.lock();
        if state.minute == minute && used > state.used {
            state.used = used;
        }
    }

    /// Holds every request for `delay`. Extends a cooldown in force, never
    /// shortens one: concurrent requests see the same `429` milliseconds
    /// apart, and taking the latest delay would release them all early.
    pub(crate) fn begin_cooldown(&self, delay: Duration) {
        self.inner.hold.extend(delay);
    }

    /// Holds every request until the next UTC minute, when the weight window
    /// resets: what a `429` with no `Retry-After` and no retry left calls for.
    /// Returns the hold. The client's retry policy asks for the same hold
    /// through the send loop, with [`until_next_minute`](Self::until_next_minute).
    #[cfg(test)]
    pub(crate) fn hold_until_next_minute(&self) -> Duration {
        let hold = self.until_next_minute();
        self.begin_cooldown(hold);
        hold
    }

    /// Time to the next UTC minute on this budget's clock.
    pub(crate) fn until_next_minute(&self) -> Duration {
        let now_ms = self.inner.clock.now_ms();
        Duration::from_millis(MINUTE_MS - now_ms % MINUTE_MS)
    }

    /// Waits out the cooldown, and any extension of it made meanwhile.
    async fn await_cooldown(&self) {
        self.inner.hold.wait().await;
    }

    fn in_cooldown(&self) -> bool {
        self.inner.hold.is_held()
    }

    fn reserve_funding_slot(&self) -> Instant {
        let now = Instant::now();
        let mut state = self.lock();
        let slot = state.next_funding.map_or(now, |next| next.max(now));
        state.next_funding = Some(slot + self.inner.funding_interval);
        slot
    }

    /// A poison-tolerant lock: a panic elsewhere must not turn the budget into
    /// a permanent outage.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The budget as core's send loop sees it.
///
/// [`acquire`](Throttle::acquire) charges the request's [`WEIGHT_LAYER`] or
/// [`FUNDING_LAYER`] cost against the minute or the funding pace, after any
/// cooldown, and records the UTC minute a weight was charged in as its
/// charge's `window`.
/// [`observe`](Throttle::observe) applies `X-MBX-USED-WEIGHT-1M` to that minute
/// alone, whatever the status. [`hold`](Throttle::hold) extends the cooldown.
impl Throttle for WeightBudget {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        let cost = meta.costs.iter().find_map(|cost| match cost.layer {
            WEIGHT_LAYER => Some((cost.layer, Cost::Weight(cost.units))),
            FUNDING_LAYER => Some((cost.layer, Cost::Funding)),
            _ => None,
        });
        let Some((layer, cost)) = cost else {
            // Nothing to charge, but a request still waits out a ban.
            self.await_cooldown().await;
            return Ok(Charge::none());
        };
        let charged = WeightBudget::acquire(self, cost).await;
        Ok(Charge::none().with(LayerCharge {
            layer,
            units: match cost {
                Cost::Weight(weight) => weight,
                Cost::Funding => 1,
            },
            window: charged.minute,
        }))
    }

    fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {
        let Some(used) = used_weight(response.headers) else {
            return;
        };
        for layer in charge.layers() {
            if layer.layer == WEIGHT_LAYER {
                self.record_used(
                    MinuteCharge {
                        minute: layer.window,
                    },
                    used,
                );
            }
        }
    }

    fn hold(&self, delay: Duration) {
        self.begin_cooldown(delay);
    }
}

/// The IP's weight used this minute, as a response reports it.
fn used_weight(headers: &polyoxide_core::reqwest::header::HeaderMap) -> Option<u32> {
    headers
        .get(USED_WEIGHT_HEADER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 2026-10-07 08:35:00 UTC, a minute boundary.
    const ON_A_MINUTE: u64 = 1_791_362_100_000;

    fn budget(weight_per_minute: u32, start_ms: u64) -> WeightBudget {
        WeightBudget::with_clock(
            weight_per_minute,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::Tokio {
                start: Instant::now(),
                start_ms,
            },
        )
    }

    fn manual_budget(start_ms: u64) -> (WeightBudget, Arc<AtomicU64>) {
        let clock = Arc::new(AtomicU64::new(start_ms));
        let budget = WeightBudget::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::Manual(Arc::clone(&clock)),
        );
        (budget, clock)
    }

    #[test]
    fn documented_weights() {
        use DepthLimit::*;
        let w = Cost::Weight;
        let table = [
            (Route::Ping, w(1)),
            (Route::Time, w(1)),
            (Route::ExchangeInfo, w(1)),
            (Route::FundingInfo, Cost::Funding),
            (Route::Ticker24h { all: false }, w(1)),
            (Route::Ticker24h { all: true }, w(40)),
            (Route::PremiumIndex { all: false }, w(1)),
            (Route::PremiumIndex { all: true }, w(10)),
            (Route::Klines { limit: None }, w(5)),
            (Route::Klines { limit: Some(1) }, w(1)),
            (Route::Klines { limit: Some(100) }, w(1)),
            (Route::Klines { limit: Some(101) }, w(2)),
            (Route::Klines { limit: Some(500) }, w(2)),
            (Route::Klines { limit: Some(501) }, w(5)),
            (Route::Klines { limit: Some(1000) }, w(5)),
            (Route::Klines { limit: Some(1001) }, w(10)),
            (Route::Klines { limit: Some(1500) }, w(10)),
            (Route::FundingRate, Cost::Funding),
            (Route::OpenInterest, w(1)),
            (Route::AggTrades, w(20)),
            (Route::Depth { limit: None }, w(1)),
            (Route::Depth { limit: Some(Five) }, w(2)),
            (Route::Depth { limit: Some(Ten) }, w(2)),
            (
                Route::Depth {
                    limit: Some(Twenty),
                },
                w(2),
            ),
            (Route::Depth { limit: Some(Fifty) }, w(2)),
            (
                Route::Depth {
                    limit: Some(Hundred),
                },
                w(5),
            ),
            (
                Route::Depth {
                    limit: Some(FiveHundred),
                },
                w(10),
            ),
            (
                Route::Depth {
                    limit: Some(Thousand),
                },
                w(20),
            ),
        ];
        for (route, cost) in table {
            assert_eq!(route.cost(), cost, "{route:?}");
        }
    }

    #[test]
    fn the_budget_aims_a_tenth_below_the_published_limit() {
        assert_eq!(WeightBudget::new().per_minute(), 2160);
    }

    #[tokio::test(start_paused = true)]
    async fn a_charge_past_the_budget_waits_for_the_next_minute() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE + 15_000);
        for _ in 0..54 {
            budget.acquire(Cost::Weight(40)).await;
        }
        assert_eq!(budget.used(), 2160);

        let start = Instant::now();
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(
            start.elapsed(),
            Duration::from_secs(45),
            "held to the boundary"
        );
        assert_eq!(budget.used(), 1, "the new minute starts from zero");
    }

    #[tokio::test(start_paused = true)]
    async fn a_header_raises_the_count_and_never_lowers_it() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        let charge = budget.acquire(Cost::Weight(1)).await;
        budget.record_used(charge, 2000);
        assert_eq!(budget.used(), 2000, "another process on this IP spent 1999");
        budget.record_used(charge, 3);
        assert_eq!(budget.used(), 2000);

        let start = Instant::now();
        budget.acquire(Cost::Weight(200)).await;
        assert_eq!(
            start.elapsed(),
            Duration::from_secs(60),
            "2000 + 200 passes 2160"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_header_is_applied_only_to_the_minute_its_request_was_charged_in() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE + 59_900);
        let in_flight = budget.acquire(Cost::Weight(5)).await;
        tokio::time::advance(Duration::from_millis(200)).await;
        budget.acquire(Cost::Weight(5)).await;
        // The response to the first request arrives in the new minute carrying
        // the old minute's count. Applying it would hold the new minute for 60 s.
        budget.record_used(in_flight, 2150);
        assert_eq!(budget.used(), 5);
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_heavier_than_the_budget_is_sent_alone() {
        let budget = budget(20, ON_A_MINUTE);
        let start = Instant::now();
        budget.acquire(Cost::Weight(40)).await;
        assert_eq!(start.elapsed(), Duration::ZERO);
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(start.elapsed(), Duration::from_secs(60));
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_holds_both_limits_and_is_only_extended() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        budget.begin_cooldown(Duration::from_secs(10));
        budget.begin_cooldown(Duration::from_secs(2));

        let start = Instant::now();
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(start.elapsed(), Duration::from_secs(10));

        budget.begin_cooldown(Duration::from_secs(3));
        let start = Instant::now();
        budget.acquire(Cost::Funding).await;
        assert_eq!(start.elapsed(), Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_is_clamped_to_the_longest_documented_ban() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        budget.begin_cooldown(Duration::MAX);
        let start = Instant::now();
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(start.elapsed(), MAX_COOLDOWN);
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_lasts_until_the_next_minute_for_both_limits() {
        for cost in [Cost::Weight(1), Cost::Funding] {
            let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE + 15_000);
            assert_eq!(budget.hold_until_next_minute(), Duration::from_secs(45));
            let start = Instant::now();
            budget.acquire(cost).await;
            assert_eq!(start.elapsed(), Duration::from_secs(45), "{cost:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_funding_bucket_admits_450_per_five_minutes() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        let start = Instant::now();
        let mut admitted = 0;
        while start.elapsed() <= FUNDING_PERIOD {
            budget.acquire(Cost::Funding).await;
            if start.elapsed() <= FUNDING_PERIOD {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 450);
    }

    #[tokio::test(start_paused = true)]
    async fn the_funding_bucket_never_bursts() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        let start = Instant::now();
        budget.acquire(Cost::Funding).await;
        budget.acquire(Cost::Funding).await;
        assert!(
            start.elapsed() >= Duration::from_millis(668),
            "{:?}",
            start.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_extended_mid_wait_is_waited_out_in_full() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        budget.begin_cooldown(Duration::from_secs(2));
        let extender = budget.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            extender.begin_cooldown(Duration::from_secs(10));
        });
        let start = Instant::now();
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(start.elapsed(), Duration::from_secs(11));
    }

    #[tokio::test(start_paused = true)]
    async fn funding_requests_parked_by_a_cooldown_are_paced_when_it_ends() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        budget.begin_cooldown(Duration::from_secs(60));
        let start = Instant::now();
        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let budget = budget.clone();
                tokio::spawn(async move {
                    budget.acquire(Cost::Funding).await;
                    start.elapsed()
                })
            })
            .collect();
        let mut released = Vec::new();
        for task in tasks {
            released.push(task.await.unwrap());
        }
        released.sort();
        assert_eq!(released[0], Duration::from_secs(60));
        for pair in released.windows(2) {
            assert!(
                pair[1] - pair[0] >= Duration::from_millis(668),
                "released together: {released:?}"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_counted_minute_never_moves_backwards() {
        let (budget, clock) = manual_budget(ON_A_MINUTE + 1_000);
        budget.acquire(Cost::Weight(2000)).await;
        // The clock steps back into the previous minute, as an NTP step or a
        // read racing the boundary can. Resetting the count there would admit
        // another 2000 in what the server counts as the same minute.
        clock.store(ON_A_MINUTE - 1_000, Ordering::SeqCst);
        assert_eq!(budget.used(), 2000);
        let second =
            tokio::time::timeout(Duration::from_secs(5), budget.acquire(Cost::Weight(2000))).await;
        assert!(second.is_err(), "charged into a minute already counted");
        // A request charged from the stepped-back clock is counted in the
        // newest minute, so its response's header still applies.
        let stale = tokio::time::timeout(Duration::from_secs(5), budget.acquire(Cost::Weight(1)))
            .await
            .expect("2001 fits under 2160");
        budget.record_used(stale, 2100);
        assert_eq!(budget.used(), 2100);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_begun_during_the_slot_wait_holds_the_funding_request() {
        // Sent anyway, a request during a 418 ban can lengthen the ban.
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        budget.acquire(Cost::Funding).await; // slot 0; the next is ~668 ms away
        let start = Instant::now();
        let waiter = {
            let budget = budget.clone();
            tokio::spawn(async move {
                budget.acquire(Cost::Funding).await;
                start.elapsed()
            })
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        budget.begin_cooldown(Duration::from_secs(10));
        assert_eq!(waiter.await.unwrap(), Duration::from_millis(10_100));
    }
}
