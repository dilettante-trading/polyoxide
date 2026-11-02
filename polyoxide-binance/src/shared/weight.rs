//! The minute engine both Binance REST products charge through.
//!
//! Binance refuses an IP whose request weight in the current UTC minute passes
//! the host's published limit. The engine counts that minute at the published
//! limit less a reserved tenth, raised by `X-MBX-USED-WEIGHT-1M` for the minute
//! a request was charged in; holds every request on a client-wide cooldown for
//! a `429` or `418`, which is only ever extended; and, where a product has
//! routes that carry no weight but share a published request limit, paces
//! those. A product's `WeightBudget` is a handle to one engine with its own
//! layer names.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use polyoxide_core::{
    Charge, Hold, LayerCharge, LayerId, RequestMeta, ResponseMeta, WindowQuotaTable,
};
use tokio::time::Instant;

/// The header carrying the IP's weight used in the current minute.
pub const USED_WEIGHT_HEADER: &str = "x-mbx-used-weight-1m";

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

fn after_reserve(published: u32) -> u32 {
    published - published.div_ceil(RESERVED_FRACTION)
}

/// The minute a request was charged in, so its response's header is applied to
/// that minute and no other. `None` for a paced layer, which reports no
/// weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MinuteCharge {
    pub(crate) minute: Option<u64>,
}

/// Where the current UTC minute comes from.
#[derive(Debug, Clone)]
pub(crate) enum Clock {
    /// The system clock, which is what Binance's minute follows.
    System,
    /// Wall time derived from tokio's clock, so a paused-time test controls it.
    #[cfg(test)]
    Tokio { start: Instant, start_ms: u64 },
    /// A clock a test sets by hand, to step it backwards.
    #[cfg(test)]
    Manual(std::sync::Arc<std::sync::atomic::AtomicU64>),
}

impl Clock {
    pub(crate) fn now_ms(&self) -> u64 {
        match self {
            Self::System => polyoxide_venue::UnixMillis::now().0,
            #[cfg(test)]
            Self::Tokio { start, start_ms } => *start_ms + start.elapsed().as_millis() as u64,
            #[cfg(test)]
            Self::Manual(ms) => ms.load(std::sync::atomic::Ordering::SeqCst),
        }
    }
}

/// One IP's request-weight minute, its optional paced layer and its cooldown.
#[derive(Debug)]
pub(crate) struct Engine {
    per_minute: u32,
    paced_interval: Option<Duration>,
    clock: Clock,
    hold: Hold,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    /// The UTC minute (milliseconds / 60 000) that `used` counts.
    minute: u64,
    used: u32,
    next_paced: Option<Instant>,
}

impl Engine {
    /// An engine for a published limit of `published_per_minute` weight, less
    /// the reserved tenth. `paced` is a published request count and its
    /// period for the routes that carry no weight, paced with a depth of
    /// one; `None` for a product with no such routes.
    pub(crate) fn new(
        published_per_minute: u32,
        paced: Option<(u32, Duration)>,
        clock: Clock,
    ) -> Self {
        // One slot of the paced target is the bucket's depth; the rest is
        // paced, so no window admits more than the target. Core's one pacing
        // formula, on this engine's own clock.
        let paced_interval =
            paced.map(|(count, period)| WindowQuotaTable::paced_interval(count, period));
        Self {
            per_minute: after_reserve(published_per_minute),
            paced_interval,
            clock,
            hold: Hold::with_ceiling(MAX_COOLDOWN),
            state: Mutex::new(State {
                minute: 0,
                used: 0,
                next_paced: None,
            }),
        }
    }

    /// The most weight this engine spends in one minute.
    pub(crate) fn per_minute(&self) -> u32 {
        self.per_minute
    }

    /// Weight charged or reported in the current UTC minute.
    pub(crate) fn used(&self) -> u32 {
        let minute = self.clock.now_ms() / MINUTE_MS;
        let state = self.lock();
        if minute > state.minute {
            0
        } else {
            state.used
        }
    }

    /// Waits until a request of `weight` can go, then charges it.
    pub(crate) async fn acquire_weight(&self, weight: u32) -> MinuteCharge {
        loop {
            self.await_cooldown().await;
            match self.try_charge(weight) {
                Ok(charge) => return charge,
                Err(until_next_minute) => tokio::time::sleep(until_next_minute).await,
            }
        }
    }

    /// Waits for the next slot of the paced layer, then takes it. On an
    /// engine with no paced layer it waits out the cooldown and returns.
    pub(crate) async fn acquire_paced(&self) -> MinuteCharge {
        let Some(interval) = self.paced_interval else {
            self.await_cooldown().await;
            return MinuteCharge { minute: None };
        };
        loop {
            // Wait out a cooldown before taking a slot: a slot taken first
            // and slept on through a cooldown is in the past when it ends,
            // and every request parked that way would go at once.
            self.await_cooldown().await;
            let slot = self.reserve_paced_slot(interval);
            tokio::time::sleep_until(slot).await;
            if !self.in_cooldown() {
                return MinuteCharge { minute: None };
            }
            // A cooldown began while this request waited for its slot: the
            // slot is spent, and the request queues again behind it.
        }
    }

    /// Charges `weight` to the current minute, or says how long until the next.
    ///
    /// An empty minute admits any weight, so a request heavier than the whole
    /// budget is sent alone rather than held forever.
    fn try_charge(&self, weight: u32) -> Result<MinuteCharge, Duration> {
        let now_ms = self.clock.now_ms();
        let mut state = self.lock();
        // The minute only moves forward. A read that lands in an earlier minute,
        // from a stale read racing a boundary or a clock stepped back, charges
        // the newest minute seen instead of resetting its count.
        let minute = now_ms / MINUTE_MS;
        if minute > state.minute {
            state.minute = minute;
            state.used = 0;
        }
        if state.used == 0 || state.used.saturating_add(weight) <= self.per_minute {
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
        self.hold.extend(delay);
    }

    /// Time to the next UTC minute on this engine's clock.
    pub(crate) fn until_next_minute(&self) -> Duration {
        let now_ms = self.clock.now_ms();
        Duration::from_millis(MINUTE_MS - now_ms % MINUTE_MS)
    }

    /// Waits out the cooldown, and any extension of it made meanwhile.
    pub(crate) async fn await_cooldown(&self) {
        self.hold.wait().await;
    }

    fn in_cooldown(&self) -> bool {
        self.hold.is_held()
    }

    fn reserve_paced_slot(&self, interval: Duration) -> Instant {
        let now = Instant::now();
        let mut state = self.lock();
        let slot = state.next_paced.map_or(now, |next| next.max(now));
        state.next_paced = Some(slot + interval);
        slot
    }

    /// A poison-tolerant lock: a panic elsewhere must not turn the budget into
    /// a permanent outage.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Charges one request on its product's layers: the cost it carries for
    /// `weight`, or else the one it carries for `paced`. A request that
    /// carries neither is charged nothing, but still waits out a cooldown.
    ///
    /// The weight layer's charge records the UTC minute it was taken in as
    /// its `window`; the paced layer's records none.
    pub(crate) async fn charge(
        &self,
        meta: &RequestMeta<'_>,
        weight: LayerId,
        paced: Option<LayerId>,
    ) -> Charge {
        let found = meta.costs.iter().find_map(|cost| {
            if cost.layer == weight {
                Some((cost.layer, Some(cost.units)))
            } else if Some(cost.layer) == paced {
                Some((cost.layer, None))
            } else {
                None
            }
        });
        let Some((layer, weighed)) = found else {
            // Nothing to charge, but a request still waits out a ban.
            self.await_cooldown().await;
            return Charge::none();
        };
        let (units, window) = match weighed {
            Some(units) => (units, self.acquire_weight(units).await.minute),
            None => (1, self.acquire_paced().await.minute),
        };
        Charge::none().with(LayerCharge {
            layer,
            units,
            window,
        })
    }

    /// Applies a response's `X-MBX-USED-WEIGHT-1M` to the minute its request
    /// was charged in on the `weight` layer, whatever the status.
    pub(crate) fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, weight: LayerId) {
        let Some(used) = used_weight(response.headers) else {
            return;
        };
        for layer in charge.layers() {
            if layer.layer == weight {
                self.record_used(
                    MinuteCharge {
                        minute: layer.window,
                    },
                    used,
                );
            }
        }
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
    use polyoxide_core::reqwest::header::{HeaderMap, HeaderValue};
    use polyoxide_core::reqwest::{Method, StatusCode};

    use super::*;

    /// 2026-10-07 08:35:00 UTC, a minute boundary: minute 29 856 035.
    const ON_A_MINUTE: u64 = 1_791_362_100_000;
    const WEIGHT: LayerId = LayerId("test-weight");
    const PACED: LayerId = LayerId("test-paced");
    const FIVE_MINUTES: Duration = Duration::from_secs(300);

    fn new_engine(paced: Option<(u32, Duration)>) -> Engine {
        Engine::new(
            2400,
            paced,
            Clock::Tokio {
                start: Instant::now(),
                start_ms: ON_A_MINUTE,
            },
        )
    }

    fn cost(layer: LayerId, units: u32) -> polyoxide_core::Cost {
        polyoxide_core::Cost {
            layer,
            units,
            exact: true,
        }
    }

    /// `engine` charging a request that carries `costs`, with `WEIGHT` as its
    /// weight layer and `paced` as its paced one.
    async fn charge(
        engine: &Engine,
        costs: &[polyoxide_core::Cost],
        paced: Option<LayerId>,
    ) -> Charge {
        let method = Method::GET;
        let meta = RequestMeta {
            method: &method,
            path: "/probe",
            query: &[],
            costs,
        };
        engine.charge(&meta, WEIGHT, paced).await
    }

    /// `engine` seeing a response that reports `used` in the weight header,
    /// for the weight layer `layer`.
    fn observe(engine: &Engine, charge: &Charge, used: &str, layer: LayerId) {
        let mut headers = HeaderMap::new();
        headers.insert(USED_WEIGHT_HEADER, HeaderValue::from_str(used).unwrap());
        engine.observe(
            charge,
            &ResponseMeta {
                status: StatusCode::OK,
                headers: &headers,
            },
            layer,
        );
    }

    #[test]
    fn the_budget_is_the_published_limit_less_a_tenth() {
        // 2400 is the futures host's published limit and 6000 the spot
        // host's; the reserve is the same tenth of each.
        for (published, budget) in [(2400, 2160), (6000, 5400)] {
            let engine = Engine::new(published, None, Clock::System);
            assert_eq!(engine.per_minute(), budget, "{published}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_charge_carries_the_layer_it_was_given_its_weight_and_its_minute() {
        let engine = new_engine(None);
        let charged = charge(&engine, &[cost(WEIGHT, 7)], None).await;
        assert_eq!(
            charged,
            Charge::none().with(LayerCharge {
                layer: WEIGHT,
                units: 7,
                window: Some(29_856_035),
            })
        );
        assert_eq!(engine.used(), 7);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cost_on_another_layer_charges_nothing_and_still_waits_out_a_cooldown() {
        let engine = new_engine(None);
        engine.begin_cooldown(Duration::from_secs(3));
        let start = Instant::now();
        let charged = charge(&engine, &[cost(LayerId("other-weight"), 5)], None).await;
        assert_eq!(charged, Charge::none());
        assert_eq!(start.elapsed(), Duration::from_secs(3));
        assert_eq!(engine.used(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_paced_cost_takes_one_unit_and_no_minute() {
        let paced = new_engine(Some((500, FIVE_MINUTES)));
        let charged = charge(&paced, &[cost(PACED, 99)], Some(PACED)).await;
        assert_eq!(
            charged,
            Charge::none().with(LayerCharge {
                layer: PACED,
                units: 1,
                window: None,
            })
        );
        assert_eq!(paced.used(), 0, "a paced request carries no weight");

        // The same cost is nothing to a request that names no paced layer.
        let unpaced = new_engine(None);
        let ignored = charge(&unpaced, &[cost(PACED, 99)], None).await;
        assert_eq!(ignored, Charge::none());
    }

    #[tokio::test(start_paused = true)]
    async fn an_engine_with_no_paced_layer_waits_out_a_cooldown_and_admits() {
        let engine = new_engine(None);
        engine.begin_cooldown(Duration::from_secs(10));
        let start = Instant::now();
        let charged = engine.acquire_paced().await;
        assert_eq!(start.elapsed(), Duration::from_secs(10));
        assert_eq!(charged, MinuteCharge { minute: None });
    }

    #[tokio::test(start_paused = true)]
    async fn a_header_raises_the_layer_the_caller_names_and_no_other() {
        let engine = new_engine(None);
        let charged = charge(&engine, &[cost(WEIGHT, 1)], None).await;

        observe(&engine, &charged, "100", LayerId("other-weight"));
        assert_eq!(engine.used(), 1, "the charge holds no such layer");

        observe(&engine, &charged, "100", WEIGHT);
        assert_eq!(engine.used(), 100);

        observe(&engine, &charged, "soon", WEIGHT);
        assert_eq!(engine.used(), 100, "not a count");

        observe(&engine, &charged, " 150 ", WEIGHT);
        assert_eq!(engine.used(), 150, "whitespace around the count is ignored");
    }
}
