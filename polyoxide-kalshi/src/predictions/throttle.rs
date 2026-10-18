//! The Read throttle: one token bucket, sized at Kalshi's Basic tier until the client has read the
//! account's own size, and the per-route costs it charges.
//!
//! Kalshi charges every authenticated request tokens from a Read or a Write bucket, 10 by default.
//! This module sends reads only. A bucket here holds its published capacity: Kalshi publishes a
//! token bucket's capacity and refill rate (Basic's Read refills 200 tokens a second and holds 3
//! seconds of it), and the server implements a bucket, so a bucket of that capacity is a faithful
//! model. That is the opposite of Cloudflare's window quotas, which publish a count per window with
//! no capacity term and must not be given a depth.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use polyoxide_core::{
    AttemptInfo, CapacityBucket, Charge, Hold, LayerCharge, Refused, RequestMeta, ResponseMeta,
    Throttle,
};

use crate::predictions::request::{Route, DEFAULT_COST, READ};

/// Tokens the Basic tier's Read bucket holds: 3 seconds of its refill.
pub(crate) const BASIC_READ_CAPACITY: u32 = 600;

/// Tokens a second the Basic tier's Read bucket refills. No tier's is lower.
pub(crate) const BASIC_READ_REFILL: u32 = 200;

/// Charges each request its cost on the Read layer, and holds every request when the retry policy
/// says to.
///
/// The bucket is [provisional](CapacityBucket::provisional): a cost above its capacity waits for
/// the sizing to be confirmed or replaced instead of being refused. The hold is unbounded, since
/// every hold this module's policy sets is the loop's own `retry_delay(0)`, which
/// `RetryConfig::max_backoff_ms` bounds.
///
/// A clone shares the bucket and the hold.
#[derive(Debug, Clone)]
pub(crate) struct PredictionsThrottle {
    read: CapacityBucket,
    hold: Hold,
}

impl PredictionsThrottle {
    /// A throttle with a full Read bucket at Basic's sizing.
    pub(crate) fn new() -> Self {
        let hold = Hold::unbounded();
        Self {
            read: CapacityBucket::provisional(
                READ,
                BASIC_READ_CAPACITY,
                BASIC_READ_REFILL,
                hold.clone(),
            ),
            hold,
        }
    }

    /// The Read bucket, for resizing or confirming it in place.
    pub(crate) fn bucket(&self) -> &CapacityBucket {
        &self.read
    }
}

impl Throttle for PredictionsThrottle {
    /// Waits out the hold, charges each cost on the Read layer (a cost on any other layer is not
    /// this throttle's), and waits out the hold again, since one set while the bucket made this
    /// request wait for tokens still applies (AD-23).
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.hold.wait().await;
        let mut charge = Charge::none();
        for cost in meta.costs {
            if cost.layer != READ {
                continue;
            }
            self.read.acquire(cost.units, cost.exact).await?;
            charge = charge.with(LayerCharge {
                layer: READ,
                units: cost.units,
                window: None,
            });
        }
        self.hold.wait().await;
        Ok(charge)
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, delay: Duration) {
        self.hold.extend(delay);
    }
}

/// What each route costs the Read layer, when that is not the default.
///
/// The compiled costs are the default, 10, for every route. The first-use confirmation replaces
/// them with what `GET /account/endpoint_costs` lists, so a cost is read when a request is sent,
/// not when its builder is made.
#[derive(Debug, Default)]
pub(crate) struct RouteCosts {
    overrides: RwLock<HashMap<(&'static str, &'static str), u32>>,
}

impl RouteCosts {
    /// What `route` costs: its override, else the default cost.
    pub(crate) fn cost(&self, route: &Route) -> u32 {
        self.overrides
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&(route.method, route.template))
            .copied()
            .unwrap_or(DEFAULT_COST)
    }

    /// Makes `route` cost `cost` from now on.
    pub(crate) fn set(&self, route: &Route, cost: u32) {
        self.overrides
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert((route.method, route.template), cost);
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
    use polyoxide_core::{Cost, LayerId};
    use tokio::time::Instant;

    use super::*;

    fn read(units: u32) -> Cost {
        Cost {
            layer: READ,
            units,
            exact: true,
        }
    }

    /// Charge one request costing `costs`, as the send loop would.
    async fn charge(throttle: &PredictionsThrottle, costs: &[Cost]) -> Result<Charge, Refused> {
        let method = Method::GET;
        throttle
            .acquire(&RequestMeta {
                method: &method,
                path: "/trade-api/v2/markets",
                query: &[],
                costs,
            })
            .await
    }

    /// Charge `count` reads of ten tokens.
    async fn spend(throttle: &PredictionsThrottle, count: u32) {
        for _ in 0..count {
            charge(throttle, &[read(10)]).await.unwrap();
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_bucket_holds_basics_published_burst_and_refills_at_its_published_rate() {
        // Basic's Read refills 200 tokens a second and holds 3 seconds of it: 600.
        let throttle = PredictionsThrottle::new();
        let start = Instant::now();
        spend(&throttle, 60).await;
        assert_eq!(
            start.elapsed(),
            Duration::ZERO,
            "60 reads are the 600 tokens"
        );

        // The 61st finds it empty, and ten tokens at 200 a second take 50 ms.
        spend(&throttle, 1).await;
        let waited = start.elapsed();
        assert!(
            (Duration::from_millis(50)..Duration::from_millis(60)).contains(&waited),
            "the 61st read waited {waited:?}"
        );

        // Then 400 more tokens take two seconds, not one: the burst is not spent twice.
        let resumed = Instant::now();
        spend(&throttle, 40).await;
        let waited = resumed.elapsed();
        assert!(
            (Duration::from_millis(1900)..Duration::from_millis(2100)).contains(&waited),
            "40 reads waited {waited:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_idle_bucket_never_holds_more_than_its_capacity() {
        let throttle = PredictionsThrottle::new();
        spend(&throttle, 60).await;
        tokio::time::sleep(Duration::from_secs(60)).await;

        let start = Instant::now();
        spend(&throttle, 60).await;
        assert_eq!(start.elapsed(), Duration::ZERO);
        spend(&throttle, 1).await;
        assert!(
            start.elapsed() >= Duration::from_millis(50),
            "a minute idle filled the bucket past 600"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_charge_names_the_layer_and_the_units() {
        let throttle = PredictionsThrottle::new();
        let charged = charge(&throttle, &[read(30)]).await.unwrap();
        assert_eq!(
            charged.layers(),
            [LayerCharge {
                layer: READ,
                units: 30,
                window: None
            }]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cost_on_another_layer_is_not_this_throttles() {
        let throttle = PredictionsThrottle::new();
        let elsewhere = Cost {
            layer: LayerId("elsewhere"),
            units: 1_000_000,
            exact: true,
        };
        let start = Instant::now();
        let charged = charge(&throttle, &[elsewhere]).await.unwrap();
        assert!(charged.layers().is_empty());
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cost_above_the_provisional_capacity_waits_for_the_sizing() {
        let throttle = PredictionsThrottle::new();
        let costs = [read(BASIC_READ_CAPACITY + 1)];

        // Neither charged nor refused while the sizing is a guess.
        let early = tokio::time::timeout(Duration::from_secs(5), charge(&throttle, &costs)).await;
        assert!(early.is_err(), "settled before the sizing was confirmed");

        // Confirmed at 600, it is a cost the bucket can never hold: refused at once.
        throttle.bucket().confirm();
        assert_eq!(
            charge(&throttle, &costs).await.unwrap_err(),
            Refused {
                layer: READ,
                units: 601,
                capacity: 600
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_resize_releases_a_cost_waiting_for_the_sizing() {
        let throttle = PredictionsThrottle::new();
        let costs = [read(BASIC_READ_CAPACITY + 1)];
        let waiting = tokio::spawn({
            let throttle = throttle.clone();
            async move { charge(&throttle, &costs).await }
        });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        throttle.bucket().resize(1_000, 400);
        let charged = waiting.await.unwrap().unwrap();
        assert_eq!(charged.layers().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_is_waited_out_before_a_request_is_charged() {
        let throttle = PredictionsThrottle::new();
        throttle.hold(Duration::from_secs(1));
        let start = Instant::now();
        spend(&throttle, 1).await;
        let waited = start.elapsed();
        assert!(
            (Duration::from_secs(1)..Duration::from_millis(1010)).contains(&waited),
            "waited {waited:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_set_while_a_request_waits_for_tokens_is_waited_out_after_the_wait() {
        let throttle = PredictionsThrottle::new();
        spend(&throttle, 60).await;
        let start = Instant::now();
        let waiting = tokio::spawn({
            let throttle = throttle.clone();
            async move { spend(&throttle, 1).await }
        });

        // The request is waiting 50 ms for tokens when a 429 holds everyone for a second.
        tokio::time::sleep(Duration::from_millis(10)).await;
        throttle.hold(Duration::from_secs(1));
        waiting.await.unwrap();
        let waited = start.elapsed();
        assert!(
            (Duration::from_millis(1010)..Duration::from_millis(1030)).contains(&waited),
            "waited {waited:?}"
        );
    }

    #[test]
    fn a_route_costs_the_default_until_it_is_set() {
        let costs = RouteCosts::default();
        let status = Route::get("/trade-api/v2/exchange/status");
        let markets = Route::get("/trade-api/v2/markets");
        assert_eq!(costs.cost(&status), 10);

        costs.set(&status, 30);
        assert_eq!(costs.cost(&status), 30);
        assert_eq!(costs.cost(&markets), 10, "another route's cost moved");

        costs.set(&status, 2);
        assert_eq!(costs.cost(&status), 2);
    }

    #[test]
    fn a_cost_belongs_to_a_method_and_a_template() {
        let costs = RouteCosts::default();
        let get = Route::get("/x");
        let post = Route {
            method: "POST",
            template: "/x",
        };
        costs.set(&post, 7);
        assert_eq!(costs.cost(&post), 7);
        assert_eq!(costs.cost(&get), 10);
    }
}
