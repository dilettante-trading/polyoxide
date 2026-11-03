//! Binance spot's request-weight budget and the table of what each route costs.
//!
//! Binance charges each REST request a *weight* that depends on its route and
//! its parameters, and refuses an IP whose weight in the current minute passes
//! `exchangeInfo`'s `REQUEST_WEIGHT` limit of 6000. Core's `RateLimiter` counts
//! requests per path, so it cannot model a cost that varies with `limit`;
//! [`WeightBudget`] does, on the engine every Binance module shares. The table is
//! [`Route::cost`]; the measurements behind it are in
//! `docs/specs/binance/OBSERVED.md` (`## Spot`).
//!
//! `exchangeInfo` also lists a `RAW_REQUESTS` limit of 300,000 per five
//! minutes. It is not modelled: every request weighs at least 1, so a budget of
//! 5400 per minute caps a client at 27,000 requests per five minutes, under it.

use std::sync::Arc;
use std::time::Duration;

use polyoxide_core::{AttemptInfo, Charge, LayerId, Refused, RequestMeta, ResponseMeta, Throttle};

use crate::shared::request::Priced;
use crate::shared::weight::{Clock, Engine};

pub use crate::shared::weight::{DEFAULT_BAN, MAX_COOLDOWN};

/// `exchangeInfo`'s `REQUEST_WEIGHT` limit per minute, per IP.
pub const PUBLISHED_WEIGHT_PER_MINUTE: u32 = 6000;

/// The per-minute request-weight layer of a [`WeightBudget`], as a request's
/// [`polyoxide_core::Cost`] names it.
pub const WEIGHT_LAYER: LayerId = LayerId("binance-spot-weight");

/// What one request costs, and which limit it draws on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// Request weight, against the per-minute budget.
    Weight(u32),
}

/// The cost a request builder hands core's send loop: its weight against
/// [`WEIGHT_LAYER`]. It is exact.
impl From<Cost> for polyoxide_core::Cost {
    fn from(cost: Cost) -> Self {
        match cost {
            Cost::Weight(weight) => Self {
                layer: WEIGHT_LAYER,
                units: weight,
                exact: true,
            },
        }
    }
}

/// A REST route, with the parameters its weight depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Route {
    /// `GET /api/v3/ping`.
    Ping,
    /// `GET /api/v3/time`.
    Time,
    /// `GET /api/v3/exchangeInfo`.
    ExchangeInfo,
    /// `GET /api/v3/depth`.
    Depth {
        /// The `limit` parameter, if sent.
        limit: Option<u32>,
    },
    /// `GET /api/v3/trades`.
    Trades,
    /// `GET /api/v3/aggTrades`.
    AggTrades,
    /// `GET /api/v3/klines`.
    Klines,
    /// `GET /api/v3/avgPrice`.
    AvgPrice,
    /// `GET /api/v3/ticker/24hr`: one symbol, or every symbol when `all`.
    Ticker24h {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /api/v3/ticker/price`: one symbol, or every symbol when `all`.
    Price {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /api/v3/ticker/bookTicker`: one symbol, or every symbol when `all`.
    BookTicker {
        /// No `symbol` parameter.
        all: bool,
    },
}

impl Route {
    /// The route's path on `api.binance.com`.
    pub fn path(self) -> &'static str {
        match self {
            Self::Ping => "/api/v3/ping",
            Self::Time => "/api/v3/time",
            Self::ExchangeInfo => "/api/v3/exchangeInfo",
            Self::Depth { .. } => "/api/v3/depth",
            Self::Trades => "/api/v3/trades",
            Self::AggTrades => "/api/v3/aggTrades",
            Self::Klines => "/api/v3/klines",
            Self::AvgPrice => "/api/v3/avgPrice",
            Self::Ticker24h { .. } => "/api/v3/ticker/24hr",
            Self::Price { .. } => "/api/v3/ticker/price",
            Self::BookTicker { .. } => "/api/v3/ticker/bookTicker",
        }
    }

    /// What the request costs, as measured from `X-MBX-USED-WEIGHT-1M` deltas
    /// on 2026-11-02 (`docs/specs/binance/OBSERVED.md`, `## Spot`, `### REST
    /// weights`).
    ///
    /// `depth` costs 5 without a `limit` and for 1 to 100 levels, 25 for 101 to
    /// 500, 50 for 501 to 1000 and 250 above, each band measured
    /// (`OBSERVED.md`, `## Spot`). `exchangeInfo` costs 20 with a `symbol` or without. The
    /// one-symbol tickers and `klines` and `avgPrice` cost 2, `trades` 25 and
    /// `aggTrades` 4, whatever their `limit`.
    pub fn cost(self) -> Cost {
        match self {
            Self::Ping | Self::Time => Cost::Weight(1),
            Self::ExchangeInfo => Cost::Weight(20),
            Self::Depth { limit: None } => Cost::Weight(5),
            Self::Depth { limit: Some(limit) } => Cost::Weight(match limit {
                0..=100 => 5,
                101..=500 => 25,
                501..=1000 => 50,
                _ => 250,
            }),
            Self::Trades => Cost::Weight(25),
            Self::AggTrades => Cost::Weight(4),
            Self::Klines | Self::AvgPrice => Cost::Weight(2),
            Self::Ticker24h { all } => Cost::Weight(if all { 80 } else { 2 }),
            Self::Price { all } | Self::BookTicker { all } => Cost::Weight(if all { 4 } else { 2 }),
        }
    }
}

impl Priced for Route {
    type Cost = Cost;

    fn path(self) -> &'static str {
        Route::path(self)
    }

    fn cost(self) -> Cost {
        Route::cost(self)
    }
}

/// The request-weight budget of one IP, shared by every client that clones it.
///
/// Binance limits weight per IP, not per client, so every
/// [`Spot`](crate::spot::Spot) in a process should be built with one budget
/// ([`SpotBuilder::weight_budget`](crate::spot::SpotBuilder::weight_budget)).
///
/// - **Window.** The UTC clock minute, as on the USDⓈ-M module: the server's
///   count falls just after each minute boundary, where a sliding window would
///   keep counting.
/// - **Budget.** The published 6000 less a tenth, 5400. A request waits while
///   its weight would take the minute past that.
/// - **Server count.** Every response's `X-MBX-USED-WEIGHT-1M` raises the
///   minute's count to at least what the server reports, because other
///   processes on the same IP spend the same budget. The count never falls
///   within a minute, and a response is applied only to the minute its request
///   was charged in.
/// - **Cooldown.** A `429` or `418` holds every request for the delay the
///   server asked for, with a ceiling of [`MAX_COOLDOWN`], so a hold is only
///   ever extended.
/// - **Throttle.** The budget is the [`Throttle`] every [`Spot`](crate::spot::Spot)
///   built with it sends through: a request's weight is charged before it is
///   sent, and each response's header is applied to the minute it was charged
///   in.
/// - **In flight.** A response's header is applied only to the minute its
///   request was charged in, so a request in flight across a minute boundary is
///   counted by the server in a minute this budget no longer sees. The reserve
///   of 600 absorbs that while the weight in flight stays under it: one `Spot`
///   at its default of 4 concurrent requests has at most 320 in flight on a
///   route at its default (4 × `tickers_24h`'s 80), and four `depth` calls
///   with a `limit` above 1000 (250 each) pass it. A higher `max_concurrent`,
///   or several clients sharing one budget, can pass it too.
/// - **Hosts.** `data-api.binance.vision` shares this counter with
///   `api.binance.com` (`OBSERVED.md`, `### One counter across hosts`), so a
///   client pointed at either is charged against the one budget.
#[derive(Debug, Clone)]
pub struct WeightBudget {
    inner: Arc<Engine>,
}

impl Default for WeightBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightBudget {
    /// A budget for Binance spot's published limit, less the reserve.
    pub fn new() -> Self {
        Self::with_clock(Clock::System)
    }

    fn with_clock(clock: Clock) -> Self {
        Self {
            inner: Arc::new(Engine::new(
                PUBLISHED_WEIGHT_PER_MINUTE,
                None,
                clock,
                polyoxide_rt::default_runtime(),
            )),
        }
    }

    /// A budget on a clock a test sets by hand, starting at `start_ms`.
    #[cfg(test)]
    pub(crate) fn at_ms(start_ms: u64) -> Self {
        Self::with_clock(Clock::Manual(Arc::new(std::sync::atomic::AtomicU64::new(
            start_ms,
        ))))
    }

    /// Whether two handles charge one budget.
    #[cfg(test)]
    pub(crate) fn is_shared_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// The most weight this budget spends in one minute.
    pub fn per_minute(&self) -> u32 {
        self.inner.per_minute()
    }

    /// Weight charged or reported in the current UTC minute.
    pub fn used(&self) -> u32 {
        self.inner.used()
    }

    /// Time left in the current UTC minute: what a `429` with no `Retry-After`
    /// and no retry left holds for.
    pub(crate) fn until_next_minute(&self) -> Duration {
        self.inner.until_next_minute()
    }
}

/// The budget as core's send loop sees it.
///
/// [`acquire`](Throttle::acquire) charges the request's [`WEIGHT_LAYER`] cost
/// against the minute, after any cooldown, and records the UTC minute it was
/// charged in. [`observe`](Throttle::observe) applies `X-MBX-USED-WEIGHT-1M`
/// to that minute alone, whatever the status. [`hold`](Throttle::hold) extends
/// the cooldown.
impl Throttle for WeightBudget {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        Ok(self.inner.charge(meta, WEIGHT_LAYER, None).await)
    }

    fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {
        self.inner.observe(charge, response, WEIGHT_LAYER);
    }

    fn hold(&self, delay: Duration) {
        self.inner.begin_cooldown(delay);
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::{
        header::{HeaderMap, HeaderValue},
        Method, StatusCode,
    };

    use super::*;

    /// 2026-11-02 12:00:00 UTC, a minute boundary.
    const ON_A_MINUTE: u64 = 1_793_620_800_000;

    #[test]
    fn documented_weights() {
        let w = Cost::Weight;
        let table = [
            (Route::Ping, w(1)),
            (Route::Time, w(1)),
            (Route::ExchangeInfo, w(20)),
            (Route::Depth { limit: None }, w(5)),
            (Route::Depth { limit: Some(1) }, w(5)),
            (Route::Depth { limit: Some(7) }, w(5)),
            (Route::Depth { limit: Some(100) }, w(5)),
            (Route::Depth { limit: Some(101) }, w(25)),
            (Route::Depth { limit: Some(500) }, w(25)),
            (Route::Depth { limit: Some(501) }, w(50)),
            (Route::Depth { limit: Some(1000) }, w(50)),
            (Route::Depth { limit: Some(1001) }, w(250)),
            (Route::Depth { limit: Some(5000) }, w(250)),
            (Route::Depth { limit: Some(5001) }, w(250)),
            (Route::Trades, w(25)),
            (Route::AggTrades, w(4)),
            (Route::Klines, w(2)),
            (Route::AvgPrice, w(2)),
            (Route::Ticker24h { all: false }, w(2)),
            (Route::Ticker24h { all: true }, w(80)),
            (Route::Price { all: false }, w(2)),
            (Route::Price { all: true }, w(4)),
            (Route::BookTicker { all: false }, w(2)),
            (Route::BookTicker { all: true }, w(4)),
        ];
        for (route, cost) in table {
            assert_eq!(route.cost(), cost, "{route:?}");
        }
    }

    #[test]
    fn every_route_has_its_documented_path() {
        let table = [
            (Route::Ping, "/api/v3/ping"),
            (Route::Time, "/api/v3/time"),
            (Route::ExchangeInfo, "/api/v3/exchangeInfo"),
            (Route::Depth { limit: None }, "/api/v3/depth"),
            (Route::Trades, "/api/v3/trades"),
            (Route::AggTrades, "/api/v3/aggTrades"),
            (Route::Klines, "/api/v3/klines"),
            (Route::AvgPrice, "/api/v3/avgPrice"),
            (Route::Ticker24h { all: true }, "/api/v3/ticker/24hr"),
            (Route::Ticker24h { all: false }, "/api/v3/ticker/24hr"),
            (Route::Price { all: true }, "/api/v3/ticker/price"),
            (Route::Price { all: false }, "/api/v3/ticker/price"),
            (Route::BookTicker { all: true }, "/api/v3/ticker/bookTicker"),
            (
                Route::BookTicker { all: false },
                "/api/v3/ticker/bookTicker",
            ),
        ];
        for (route, path) in table {
            assert_eq!(route.path(), path, "{route:?}");
        }
    }

    #[test]
    fn the_budget_aims_a_tenth_below_the_published_limit() {
        assert_eq!(PUBLISHED_WEIGHT_PER_MINUTE, 6000);
        assert_eq!(WeightBudget::new().per_minute(), 5400);
    }

    #[test]
    fn a_cost_reaches_core_as_an_exact_charge_on_the_spot_layer() {
        let cost = polyoxide_core::Cost::from(Cost::Weight(20));
        assert_eq!(cost.layer, WEIGHT_LAYER);
        assert_eq!(cost.units, 20);
        assert!(cost.exact);
        assert_eq!(WEIGHT_LAYER, LayerId("binance-spot-weight"));
    }

    fn meta<'a>(method: &'a Method, costs: &'a [polyoxide_core::Cost]) -> RequestMeta<'a> {
        RequestMeta {
            method,
            path: "/api/v3/exchangeInfo",
            query: &[],
            costs,
        }
    }

    #[tokio::test]
    async fn the_throttle_charges_the_weight_layer_and_reads_the_header_back() {
        let budget = WeightBudget::at_ms(ON_A_MINUTE);
        let get = Method::GET;
        let costs = [polyoxide_core::Cost::from(Cost::Weight(20))];
        let charge = Throttle::acquire(&budget, &meta(&get, &costs))
            .await
            .unwrap();
        assert_eq!(budget.used(), 20);

        let mut headers = HeaderMap::new();
        headers.insert("x-mbx-used-weight-1m", HeaderValue::from_static("100"));
        let response = ResponseMeta {
            status: StatusCode::OK,
            headers: &headers,
        };
        let attempt = AttemptInfo {
            attempt: 0,
            retries_left: 0,
        };
        budget.observe(&charge, &response, &attempt);
        assert_eq!(budget.used(), 100, "the server's count raises the minute's");
    }

    #[tokio::test]
    async fn a_cost_on_another_layer_is_not_charged() {
        let budget = WeightBudget::at_ms(ON_A_MINUTE);
        let get = Method::GET;
        let costs = [polyoxide_core::Cost {
            layer: LayerId("some-other-layer"),
            units: 50,
            exact: true,
        }];
        Throttle::acquire(&budget, &meta(&get, &costs))
            .await
            .unwrap();
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn a_clone_shares_the_budget() {
        let budget = WeightBudget::at_ms(ON_A_MINUTE);
        let clone = budget.clone();
        assert!(budget.is_shared_with(&clone));
        assert!(!budget.is_shared_with(&WeightBudget::at_ms(ON_A_MINUTE)));
    }
}
