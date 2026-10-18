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
use std::sync::{Arc, RwLock};
use std::time::Duration;

use polyoxide_core::{
    AttemptInfo, CapacityBucket, Charge, Hold, LayerCharge, Refused, Request, RequestMeta,
    ResponseMeta, Throttle,
};

use crate::predictions::request::{Route, DEFAULT_COST, READ};
use crate::predictions::{Inner, PredictionsError};

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

impl Inner {
    /// Runs before every send, and confirms the client's costs and Read bucket the first time.
    ///
    /// The first send of a client, and of every clone of it, runs the confirmation, and every
    /// other send, a concurrent one included, waits for it. It reads `/account/endpoint_costs`
    /// and replaces the compiled cost of each of our routes the answer lists, matched on method
    /// and template. With credentials it then reads `/account/limits` and resizes the Read
    /// bucket in place to the account's capacity and refill. It always ends with the bucket
    /// confirmed, so a cost above its capacity is refused and not left waiting for a resize that
    /// never comes.
    ///
    /// It never fails a send. A read that fails, and a limits answer with a zero capacity or
    /// refill (core's bucket panics on a zero refill), log one WARN and keep the compiled costs
    /// or Basic's sizing for the client's life. Basic is the floor of every tier, so such a
    /// client under-sends and never over-sends. A send dropped mid-confirmation leaves it to
    /// the next one.
    ///
    /// The two reads are core requests sent here, not through the route builders, which wait on
    /// this themselves. Each is charged to the Read layer at its own cost.
    pub(crate) async fn before_send(&self) {
        self.confirmation.get_or_init(|| self.confirm()).await;
    }

    async fn confirm(&self) {
        self.read_costs().await;
        self.read_limits().await;
    }

    /// One of the confirmation's reads: signed when the client holds a key, and charged to the
    /// Read layer at the route's cost as it stands, capped at the bucket's capacity.
    ///
    /// The cap is what keeps the confirmation from waiting on itself. The bucket is provisional
    /// until the confirmation ends, so a read costing more than its capacity (the costs answer
    /// listing `/account/limits` above Basic's 600, say) would wait for a resize only this read
    /// can bring, and every send would wait with it. The venue is told the real route, and
    /// charges what it charges; the cap only bounds what this client waits for.
    fn confirmation_request<T>(&self, route: &Route) -> Request<T, PredictionsError> {
        let mut cost = self.read_cost(route);
        cost.units = cost.units.min(self.throttle.bucket().capacity());
        let request = Request::new(self.http.clone(), route.path(&[])).with_cost(cost);
        match &self.auth {
            Some(auth) => request.authenticator(Arc::clone(auth)),
            None => request,
        }
    }

    /// Replaces the compiled cost of each route `/account/endpoint_costs` lists.
    ///
    /// Only listed routes change: an unlisted route keeps the compiled cost, and the list's
    /// `default_cost` is not applied, since the live suite compares it with the compiled one.
    async fn read_costs(&self) {
        use crate::predictions::request::{routes, template_matches};
        use crate::predictions::types::EndpointCosts;

        let route = routes::ENDPOINT_COSTS;
        let template = route.template;
        let listed = match self
            .confirmation_request::<EndpointCosts>(&route)
            .send()
            .await
        {
            Ok(listed) => listed,
            Err(error) => {
                tracing::warn!(
                    "could not read {template}: {error}; keeping the compiled route costs"
                );
                return;
            }
        };
        for entry in &listed.endpoint_costs {
            for ours in routes::ALL {
                if ours.method.eq_ignore_ascii_case(&entry.method)
                    && template_matches(ours.template, &entry.path)
                {
                    self.costs.set(ours, entry.cost);
                }
            }
        }
    }

    /// Sizes the Read bucket from `/account/limits` when the client holds a key, and confirms it
    /// in every case.
    async fn read_limits(&self) {
        use crate::predictions::request::routes;
        use crate::predictions::types::AccountLimits;

        let bucket = self.throttle.bucket();
        if self.auth.is_none() {
            // No limit for unauthenticated requests is published: Basic's sizing stands.
            bucket.confirm();
            return;
        }
        let route = routes::ACCOUNT_LIMITS;
        let template = route.template;
        match self
            .confirmation_request::<AccountLimits>(&route)
            .send()
            .await
        {
            Ok(limits) if limits.read.bucket_capacity > 0 && limits.read.refill_rate > 0 => {
                bucket.resize(limits.read.bucket_capacity, limits.read.refill_rate);
            }
            Ok(limits) => {
                let (capacity, refill) = (limits.read.bucket_capacity, limits.read.refill_rate);
                tracing::warn!(
                    "{template} reports a Read bucket of {capacity} tokens refilling {refill} a second; keeping Basic's sizing"
                );
                bucket.confirm();
            }
            Err(error) => {
                tracing::warn!("could not read {template}: {error}; keeping Basic's sizing");
                bucket.confirm();
            }
        }
    }
}

#[cfg(test)]
mod confirmation_tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use mockito::{Server, ServerGuard};
    use polyoxide_core::ApiError;
    use polyoxide_test_support::fixtures;

    use crate::predictions::request::{routes, DEFAULT_COST};
    use crate::predictions::{Credentials, Predictions, PredictionsError};

    const COSTS: &str = "/trade-api/v2/account/endpoint_costs";
    const LIMITS: &str = "/trade-api/v2/account/limits";
    const STATUS: &str = "/trade-api/v2/exchange/status";
    const STATUS_BODY: &str = r#"{"exchange_active":true,"trading_active":true}"#;

    /// The Ed25519 key of the signing vectors, which are throwaway ones.
    fn keyed() -> Credentials {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let body = vectors["ed25519"]["pkcs8_body"]
            .as_str()
            .expect("the vectors hold an Ed25519 key body");
        // The fixture holds the base64 lines with no armour line.
        let label = "PRIVATE KEY";
        let pem = format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n");
        Credentials::from_pem("test-key", &pem).unwrap()
    }

    fn client(server: &ServerGuard, credentials: Option<Credentials>) -> Predictions {
        let mut builder = Predictions::builder().base_url(server.url());
        if let Some(credentials) = credentials {
            builder = builder.credentials(credentials);
        }
        builder.build().unwrap()
    }

    /// A mock answering `path` with `status` and `body`, expected `hits` times.
    async fn answer(
        server: &mut ServerGuard,
        path: &str,
        status: usize,
        body: &str,
        hits: usize,
    ) -> mockito::Mock {
        server
            .mock("GET", path)
            .with_status(status)
            .with_header("content-type", "application/json")
            .with_body(body)
            .expect(hits)
            .create_async()
            .await
    }

    /// `/account/endpoint_costs` listing each `(path, cost)` for `GET`.
    fn costs_body(listed: &[(&str, u32)]) -> String {
        let rows: Vec<_> = listed
            .iter()
            .map(|(path, cost)| serde_json::json!({"method": "GET", "path": path, "cost": cost}))
            .collect();
        serde_json::json!({"default_cost": 10, "endpoint_costs": rows}).to_string()
    }

    fn limits_body(capacity: u32, refill: u32) -> String {
        serde_json::json!({
            "usage_tier": "Premier",
            "read": {"refill_rate": refill, "bucket_capacity": capacity},
            "write": {"refill_rate": 100, "bucket_capacity": 100},
            "grants": [],
        })
        .to_string()
    }

    /// Records the target, level and fields of every event.
    #[derive(Clone, Default)]
    struct Events(Arc<Mutex<Vec<(String, tracing::Level, String)>>>);

    struct Fields(String);

    impl tracing::field::Visit for Fields {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0.push_str(&format!("{}={value:?} ", field.name()));
        }
    }

    impl tracing::Subscriber for Events {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            let metadata = event.metadata();
            let mut fields = Fields(String::new());
            event.record(&mut fields);
            self.0.lock().unwrap().push((
                metadata.target().to_owned(),
                *metadata.level(),
                fields.0,
            ));
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }

    impl Events {
        /// The target and fields of every WARN seen so far.
        fn warnings(&self) -> Vec<(String, String)> {
            let events = self.0.lock().unwrap();
            events
                .iter()
                .filter(|(_, level, _)| *level == tracing::Level::WARN)
                .map(|(target, _, fields)| (target.clone(), fields.clone()))
                .collect()
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_keyed_client_reads_each_confirmation_route_once_across_clones_and_concurrent_sends()
    {
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, 200, &costs_body(&[]), 1).await;
        let limits = answer(&mut server, LIMITS, 200, &limits_body(1200, 1200), 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 8).await;
        let client = client(&server, Some(keyed()));

        let sends: Vec<_> = (0..8)
            .map(|_| {
                let client = client.clone();
                tokio::spawn(async move { client.exchange().status().send().await })
            })
            .collect();
        for send in sends {
            send.await.unwrap().unwrap();
        }

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        assert_eq!(
            client.inner.throttle.bucket().capacity(),
            1200,
            "the Read bucket was not resized to the account's capacity"
        );
    }

    #[tokio::test]
    async fn a_listed_route_takes_the_listed_cost_and_an_unlisted_route_keeps_the_compiled_one() {
        let mut server = Server::new_async().await;
        // The listing spells the orderbook's parameter another way, and carries a wildcard entry
        // that is none of ours.
        let listed = costs_body(&[
            (STATUS, 25),
            ("/trade-api/v2/markets/:market_ticker/orderbook", 30),
            ("/trade-api/v2/cfbenchmarks/*endpoint", 99),
        ]);
        let costs = answer(&mut server, COSTS, 200, &listed, 1).await;
        let limits = answer(&mut server, LIMITS, 200, &limits_body(1200, 1200), 0).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let client = client(&server, None);

        client.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        let costs = &client.inner.costs;
        assert_eq!(costs.cost(&routes::EXCHANGE_STATUS), 25);
        assert_eq!(costs.cost(&routes::ORDERBOOK), 30);
        assert_eq!(costs.cost(&routes::MARKETS), DEFAULT_COST);
        assert_eq!(costs.cost(&routes::MARKET), DEFAULT_COST);
        assert_eq!(costs.cost(&routes::TRADES), DEFAULT_COST);
    }

    #[tokio::test]
    async fn a_limits_route_listed_above_basics_capacity_does_not_hang_the_confirmation() {
        let mut server = Server::new_async().await;
        // The venue lists the limits read itself at 700, above the provisional bucket's 600.
        let listed = costs_body(&[(LIMITS, 700)]);
        let costs = answer(&mut server, COSTS, 200, &listed, 1).await;
        let limits = answer(&mut server, LIMITS, 200, &limits_body(1200, 1000), 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let client = client(&server, Some(keyed()));

        tokio::time::timeout(Duration::from_secs(5), client.exchange().status().send())
            .await
            .expect("the confirmation waited for a resize only it could bring")
            .unwrap();

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        assert_eq!(client.inner.throttle.bucket().capacity(), 1200);
        // The listing still applies to the route as the client charges it later.
        assert_eq!(client.inner.costs.cost(&routes::ACCOUNT_LIMITS), 700);
    }

    #[tokio::test]
    async fn the_read_bucket_takes_the_limits_answers_refill_rate() {
        let mut server = Server::new_async().await;
        let _costs = answer(&mut server, COSTS, 200, &costs_body(&[]), 1).await;
        // Distinct numbers, and neither is Basic's 200.
        let _limits = answer(&mut server, LIMITS, 200, &limits_body(1200, 1000), 1).await;
        let _status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let client = client(&server, Some(keyed()));

        client.exchange().status().send().await.unwrap();

        let bucket = format!("{:?}", client.inner.throttle.bucket());
        assert!(bucket.contains("capacity: 1200"), "{bucket}");
        assert!(bucket.contains("refill_per_sec: 1000"), "{bucket}");
        assert!(bucket.contains("provisional: false"), "{bucket}");
    }

    #[tokio::test]
    async fn a_failed_costs_read_still_reads_limits_and_resizes_the_bucket() {
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, 500, "{}", 1).await;
        let limits = answer(&mut server, LIMITS, 200, &limits_body(1200, 1200), 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let client = client(&server, Some(keyed()));

        client.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        assert_eq!(client.inner.throttle.bucket().capacity(), 1200);
        assert_eq!(
            client.inner.costs.cost(&routes::EXCHANGE_STATUS),
            DEFAULT_COST
        );
    }

    #[tokio::test]
    async fn an_anonymous_client_confirms_at_basic_so_a_cost_above_it_is_refused_not_waited_on() {
        let mut server = Server::new_async().await;
        let listed = costs_body(&[(STATUS, 700)]);
        let _costs = answer(&mut server, COSTS, 200, &listed, 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 0).await;
        let client = client(&server, None);

        let outcome =
            tokio::time::timeout(Duration::from_secs(5), client.exchange().status().send())
                .await
                .expect("the send waited for a resize that never comes");

        match outcome {
            Err(PredictionsError::Api(ApiError::Refused(refusal))) => {
                assert_eq!(refusal.units, 700);
                assert_eq!(refusal.capacity, 600);
                assert_eq!(refusal.layer, crate::predictions::request::READ);
            }
            Err(other) => panic!("expected a refusal, got {other:?}"),
            Ok(_) => panic!("expected a refusal, got a success"),
        }
        status.assert_async().await;
    }

    #[tokio::test]
    async fn a_failed_costs_read_warns_once_and_the_next_send_does_not_repeat_it() {
        let events = Events::default();
        let _scope = tracing::subscriber::set_default(events.clone());
        // With one dispatcher registered, tracing-core works out the interest of a callsite
        // another test's thread hits first from that thread's default, which has none, and
        // caches it as never for every thread. A second one makes it ask each dispatcher.
        let _second = tracing::Dispatch::new(Events::default());
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, 500, "{}", 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 2).await;
        let client = client(&server, None);

        client.exchange().status().send().await.unwrap();
        client.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        status.assert_async().await;
        let warnings = events.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].0.starts_with("polyoxide_kalshi"),
            "{warnings:?}"
        );
        assert!(warnings[0].1.contains("endpoint_costs"), "{warnings:?}");
        assert_eq!(
            client.inner.costs.cost(&routes::EXCHANGE_STATUS),
            DEFAULT_COST
        );
        // The bucket is confirmed at Basic: a cost above it is refused and not waited on.
        let admitted = tokio::time::timeout(
            Duration::from_secs(5),
            client.inner.throttle.bucket().acquire(700, true),
        )
        .await
        .expect("the bucket was left provisional");
        assert!(
            admitted.is_err(),
            "a cost above Basic's capacity was admitted"
        );
    }

    #[tokio::test]
    async fn a_failed_limits_read_warns_once_and_keeps_basics_sizing() {
        let events = Events::default();
        let _scope = tracing::subscriber::set_default(events.clone());
        let _second = tracing::Dispatch::new(Events::default());
        let mut server = Server::new_async().await;
        let _costs = answer(&mut server, COSTS, 200, &costs_body(&[]), 1).await;
        let limits = answer(&mut server, LIMITS, 500, "{}", 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let client = client(&server, Some(keyed()));

        client.exchange().status().send().await.unwrap();

        limits.assert_async().await;
        status.assert_async().await;
        let warnings = events.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].0.starts_with("polyoxide_kalshi"),
            "{warnings:?}"
        );
        assert!(warnings[0].1.contains("account/limits"), "{warnings:?}");
        assert_eq!(client.inner.throttle.bucket().capacity(), 600);
        let admitted = tokio::time::timeout(
            Duration::from_secs(5),
            client.inner.throttle.bucket().acquire(700, true),
        )
        .await
        .expect("the bucket was left provisional");
        assert!(
            admitted.is_err(),
            "a cost above Basic's capacity was admitted"
        );
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
