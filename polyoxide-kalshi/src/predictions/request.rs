//! What every route builder sends through: the route table, and core's request builder with the
//! route's cost on the Read layer.

use std::fmt::Write as _;
use std::marker::PhantomData;
use std::sync::Arc;

use polyoxide_core::{Cost, LayerId, QueryBuilder, Request};
use serde::de::DeserializeOwned;

use crate::predictions::{error::PredictionsError, Inner};

/// The throttle layer every request of this module is charged to: Kalshi's Read bucket.
pub(crate) const READ: LayerId = LayerId("predictions.read");

/// What a route costs, in tokens, until `GET /account/endpoint_costs` says otherwise: the default
/// cost Kalshi publishes there.
#[doc(hidden)]
pub const DEFAULT_COST: u32 = 10;

/// A route: an HTTP method and a path template in the spelling of `GET /account/endpoint_costs`,
/// where `:name` stands for a segment that varies.
///
/// Every path begins `/trade-api/v2/`, the API root, whatever the host, because that is the path
/// Kalshi's signature covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Route {
    pub(crate) method: &'static str,
    pub(crate) template: &'static str,
}

impl Route {
    /// A `GET` route.
    pub(crate) const fn get(template: &'static str) -> Self {
        Self {
            method: "GET",
            template,
        }
    }

    /// The route's path with each `:name` segment replaced by the next of `params`, in order.
    ///
    /// A parameter is percent-encoded, every byte but an ASCII letter, a digit and `-._~`, so a
    /// caller's text can never add a segment or start a query. A `:name` segment with no parameter
    /// is left as it is, and a parameter with no segment is ignored.
    pub(crate) fn path(&self, params: &[&str]) -> String {
        let mut params = params.iter();
        let segments: Vec<String> = self
            .template
            .split('/')
            .map(|segment| {
                if segment.starts_with(':') {
                    params
                        .next()
                        .map_or_else(|| segment.to_owned(), |param| encode(param))
                } else {
                    segment.to_owned()
                }
            })
            .collect();
        segments.join("/")
    }
}

/// `param`, percent-encoded as one path segment.
fn encode(param: &str) -> String {
    let mut encoded = String::with_capacity(param.len());
    for byte in param.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

/// The eight routes of this module, by name. No later story adds one.
pub(crate) mod routes {
    use super::Route;

    pub(crate) const EXCHANGE_STATUS: Route = Route::get("/trade-api/v2/exchange/status");
    pub(crate) const MARKETS: Route = Route::get("/trade-api/v2/markets");
    pub(crate) const MARKET: Route = Route::get("/trade-api/v2/markets/:ticker");
    pub(crate) const ORDERBOOK: Route = Route::get("/trade-api/v2/markets/:ticker/orderbook");
    pub(crate) const TRADES: Route = Route::get("/trade-api/v2/markets/trades");
    pub(crate) const EVENT: Route = Route::get("/trade-api/v2/events/:event_ticker");
    pub(crate) const ENDPOINT_COSTS: Route = Route::get("/trade-api/v2/account/endpoint_costs");
    pub(crate) const ACCOUNT_LIMITS: Route = Route::get("/trade-api/v2/account/limits");

    /// Every route above.
    pub(crate) const ALL: &[Route] = &[
        EXCHANGE_STATUS,
        MARKETS,
        MARKET,
        ORDERBOOK,
        TRADES,
        EVENT,
        ENDPOINT_COSTS,
        ACCOUNT_LIMITS,
    ];
}

/// Every route this module sends, as its method and its path template.
///
/// The live suite's drift test reads the table through this and `template_matches`; nothing else
/// should.
#[doc(hidden)]
pub fn route_templates() -> impl Iterator<Item = (&'static str, &'static str)> {
    routes::ALL
        .iter()
        .map(|route| (route.method, route.template))
}

/// Whether two path templates name the same route, segment by segment.
///
/// Two templates match when they have as many `/`-separated segments and each pair is equal or both
/// begin with `:`, so a parameter Kalshi spells differently still matches. As an exception, a
/// segment of `listed` beginning with `*` matches all the remaining segments of `ours`, at least
/// one, as `/trade-api/v2/cfbenchmarks/*endpoint` does.
#[doc(hidden)]
pub fn template_matches(ours: &str, listed: &str) -> bool {
    let ours: Vec<&str> = ours.split('/').collect();
    let listed: Vec<&str> = listed.split('/').collect();
    for (index, theirs) in listed.iter().enumerate() {
        if theirs.starts_with('*') {
            return ours.len() > index;
        }
        match ours.get(index) {
            Some(mine) if mine == theirs || (mine.starts_with(':') && theirs.starts_with(':')) => {}
            _ => return false,
        }
    }
    ours.len() == listed.len()
}

/// A route and its query, sent as core's request with the route's cost on the Read layer.
///
/// The route builders wrap one. Setting a query parameter twice replaces it. Every route of this
/// module is a `GET`, which core's request is until told otherwise.
pub(crate) struct Routed<T> {
    inner: Arc<Inner>,
    route: Route,
    path: String,
    query: Vec<(&'static str, String)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Routed<T> {
    /// A request for `route`, its `:name` segments filled from `params` in order.
    pub(crate) fn new(inner: &Arc<Inner>, route: Route, params: &[&str]) -> Self {
        Self {
            inner: Arc::clone(inner),
            route,
            path: route.path(params),
            query: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a query parameter, replacing an earlier value for the same key.
    #[allow(
        dead_code,
        reason = "Story 2's query setters write through it, and the tests below use it"
    )]
    pub(crate) fn query(mut self, key: &'static str, value: impl ToString) -> Self {
        let value = value.to_string();
        match self.query.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.query.push((key, value)),
        }
        self
    }

    /// Whether the client holds an authenticator, so a route that needs a key can refuse before it
    /// sends.
    #[allow(
        dead_code,
        reason = "Story 4's account().limits() refuses without a key through it"
    )]
    pub(crate) fn has_key(&self) -> bool {
        self.inner.auth.is_some()
    }
}

impl<T: DeserializeOwned> Routed<T> {
    /// Sends on the client's send loop, which waits for the Read bucket, and decodes the response.
    ///
    /// The steps, in order: wait for the client's `before_send`; build core's request with the
    /// route's cost, read now, after any confirmation has replaced it; add the client's
    /// authenticator when it has one, and the query pairs; send.
    pub(crate) async fn send(self) -> Result<T, PredictionsError> {
        self.inner.before_send().await;
        let cost = self.inner.read_cost(&self.route);
        let mut request =
            Request::<T, PredictionsError>::new(self.inner.http.clone(), self.path).with_cost(cost);
        if let Some(auth) = &self.inner.auth {
            request = request.authenticator(Arc::clone(auth));
        }
        for (key, value) in self.query {
            request = request.query(key, value);
        }
        request.send().await
    }
}

impl Inner {
    /// What `route` costs the Read layer now: the confirmed cost when the first-use confirmation
    /// has replaced it, else the compiled one. The cost is exact: Kalshi charges it.
    pub(crate) fn read_cost(&self, route: &Route) -> Cost {
        Cost {
            layer: READ,
            units: self.costs.cost(route),
            exact: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_route_table_is_the_epics_eight_routes() {
        let templates: Vec<(&str, &str)> = route_templates().collect();
        assert_eq!(
            templates,
            [
                ("GET", "/trade-api/v2/exchange/status"),
                ("GET", "/trade-api/v2/markets"),
                ("GET", "/trade-api/v2/markets/:ticker"),
                ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
                ("GET", "/trade-api/v2/markets/trades"),
                ("GET", "/trade-api/v2/events/:event_ticker"),
                ("GET", "/trade-api/v2/account/endpoint_costs"),
                ("GET", "/trade-api/v2/account/limits"),
            ]
        );
    }

    #[test]
    fn every_route_is_written_from_the_api_root_and_none_is_listed_twice() {
        let mut seen = std::collections::HashSet::new();
        for route in routes::ALL {
            assert!(route.template.starts_with("/trade-api/v2/"), "{route:?}");
            assert!(seen.insert((route.method, route.template)), "{route:?}");
        }
    }

    #[test]
    fn a_route_costs_the_ten_tokens_kalshi_publishes_as_its_default() {
        // `default_cost` of `GET /account/endpoint_costs`, probed on 2026-10-18.
        assert_eq!(DEFAULT_COST, 10);
    }

    #[test]
    fn a_path_replaces_each_parameter_segment_in_order() {
        let orderbook = Route::get("/trade-api/v2/markets/:ticker/orderbook");
        assert_eq!(
            orderbook.path(&["KXBTCD-26OCT1806-T82099.99"]),
            "/trade-api/v2/markets/KXBTCD-26OCT1806-T82099.99/orderbook"
        );
        assert_eq!(
            Route::get("/a/:first/b/:second").path(&["1", "2"]),
            "/a/1/b/2"
        );
        assert_eq!(
            routes::EXCHANGE_STATUS.path(&[]),
            "/trade-api/v2/exchange/status"
        );
    }

    #[test]
    fn a_parameter_cannot_add_a_segment_or_start_a_query() {
        let event = routes::EVENT;
        assert_eq!(
            event.path(&["a/b?c d%é"]),
            "/trade-api/v2/events/a%2Fb%3Fc%20d%25%C3%A9"
        );
        // The unreserved characters stay as they are.
        assert_eq!(event.path(&["Az09-._~"]), "/trade-api/v2/events/Az09-._~");
    }

    #[test]
    fn a_missing_parameter_leaves_its_segment_and_an_extra_one_is_ignored() {
        let two = Route::get("/a/:first/b/:second");
        assert_eq!(two.path(&["1"]), "/a/1/b/:second");
        assert_eq!(two.path(&["1", "2", "3"]), "/a/1/b/2");
        assert_eq!(Route::get("/a/b").path(&["1"]), "/a/b");
    }

    #[test]
    fn templates_match_segment_by_segment() {
        let rows = [
            ("/markets/:ticker", "/markets/:ticker", true),
            // A parameter Kalshi spells differently still matches.
            ("/markets/:ticker/orderbook", "/markets/:id/orderbook", true),
            // A literal segment is not a parameter, either way round.
            ("/markets/trades", "/markets/:ticker", false),
            ("/markets/:ticker", "/markets/trades", false),
            ("/markets/a", "/markets/b", false),
            // The segments must be as many.
            ("/markets", "/markets/:ticker", false),
            ("/markets/:ticker", "/markets", false),
            ("/markets/:ticker/orderbook", "/markets/:ticker", false),
            // A listed `*name` takes every remaining segment, and at least one.
            ("/cf/btc", "/cf/*endpoint", true),
            ("/cf/btc/spot", "/cf/*endpoint", true),
            ("/cf/:name/spot", "/cf/*endpoint", true),
            ("/cf", "/cf/*endpoint", false),
            ("/markets/btc", "/cf/*endpoint", false),
        ];
        for (ours, listed, matches) in rows {
            assert_eq!(template_matches(ours, listed), matches, "{ours} / {listed}");
        }
    }

    #[test]
    fn our_routes_find_themselves_and_not_each_other() {
        for route in routes::ALL {
            for other in routes::ALL {
                assert_eq!(
                    template_matches(route.template, other.template),
                    route == other,
                    "{} / {}",
                    route.template,
                    other.template
                );
            }
        }
    }
}

#[cfg(test)]
mod routed_tests {
    use mockito::{Matcher, Server};
    use polyoxide_core::reqwest::header::{HeaderName, HeaderValue};
    use polyoxide_core::{ApiError, Authenticator, DynAuthenticator, RequestParts};
    use polyoxide_test_support::query::{headers_sent, pairs_sent};
    use serde_json::Value;

    use super::*;
    use crate::predictions::Predictions;

    const STATUS: &str = "/trade-api/v2/exchange/status";

    /// Signs by adding a header, so a test can see which requests were signed.
    struct Marker;

    impl Authenticator for Marker {
        async fn sign(&self, parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
            parts.headers.insert(
                HeaderName::from_static("x-marker"),
                HeaderValue::from_static("signed"),
            );
            Ok(())
        }
    }

    /// A client for `url` that signs with [`Marker`].
    fn keyed(url: String) -> Predictions {
        let mut builder = Predictions::builder().base_url(url);
        builder.auth = Some(DynAuthenticator::new_arc(Marker));
        builder.build().unwrap()
    }

    fn marked(headers: &[(String, String)]) -> bool {
        headers.contains(&("x-marker".to_owned(), "signed".to_owned()))
    }

    #[test]
    fn a_read_costs_ten_exact_tokens_on_the_read_layer_until_a_cost_is_set() {
        let kalshi = Predictions::new().unwrap();
        assert_eq!(
            kalshi.inner.read_cost(&routes::MARKETS),
            Cost {
                layer: READ,
                units: 10,
                exact: true
            }
        );

        kalshi.inner.costs.set(&routes::MARKETS, 30);
        assert_eq!(kalshi.inner.read_cost(&routes::MARKETS).units, 30);
        assert_eq!(kalshi.inner.read_cost(&routes::TRADES).units, 10);
    }

    #[test]
    fn a_query_parameter_set_twice_is_replaced_in_place() {
        let kalshi = Predictions::new().unwrap();
        let request = Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[])
            .query("a", 1)
            .query("b", "x")
            .query("a", 2);
        assert_eq!(
            request.query,
            [("a", "2".to_owned()), ("b", "x".to_owned())]
        );
        assert_eq!(request.path, "/trade-api/v2/markets");
    }

    #[test]
    fn a_route_builder_fills_its_path_from_its_parameters() {
        let kalshi = Predictions::new().unwrap();
        let request = Routed::<Value>::new(&kalshi.inner, routes::ORDERBOOK, &["FED-23DEC-T3.00"]);
        assert_eq!(
            request.path,
            "/trade-api/v2/markets/FED-23DEC-T3.00/orderbook"
        );
    }

    #[test]
    fn a_client_has_a_key_only_when_an_authenticator_was_installed() {
        let plain = Predictions::new().unwrap();
        assert!(!Routed::<Value>::new(&plain.inner, routes::MARKETS, &[]).has_key());
        let signed = keyed("http://127.0.0.1:1".to_owned());
        assert!(Routed::<Value>::new(&signed.inner, routes::MARKETS, &[]).has_key());
        // Neither prints what signs, only whether something does.
        assert!(format!("{signed:?}").contains("keyed: true"));
        assert!(format!("{:?}", Predictions::builder()).contains("keyed: false"));
        let mut builder = Predictions::builder();
        builder.auth = Some(DynAuthenticator::new_arc(Marker));
        assert!(format!("{builder:?}").contains("keyed: true"));
    }

    #[tokio::test]
    async fn a_send_carries_the_query_pairs_in_order() {
        let pairs = pairs_sent(STATUS, |url| async move {
            let kalshi = Predictions::builder().base_url(url).build().unwrap();
            let _ = Routed::<Value>::new(&kalshi.inner, routes::EXCHANGE_STATUS, &[])
                .query("a", 1)
                .query("b", "x y")
                .query("a", 2)
                .send()
                .await;
        })
        .await;
        assert_eq!(
            pairs,
            [("a", "2"), ("b", "x y")].map(|(k, v)| (k.to_owned(), v.to_owned()))
        );
    }

    #[tokio::test]
    async fn a_send_is_signed_by_the_clients_authenticator_and_a_ping_is_not() {
        let sent = headers_sent(STATUS, |url| async move {
            let _ = keyed(url).exchange().status().send().await;
        })
        .await;
        assert!(marked(&sent), "{sent:?}");

        let pinged = headers_sent(STATUS, |url| async move {
            let _ = keyed(url).health().ping().await;
        })
        .await;
        assert!(!marked(&pinged), "{pinged:?}");
    }

    #[tokio::test]
    async fn a_read_the_confirmed_bucket_cannot_hold_is_refused_before_anything_is_sent() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", STATUS)
            .match_query(Matcher::Any)
            .expect(0)
            .create_async()
            .await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .build()
            .unwrap();
        kalshi.inner.throttle.bucket().resize(5, 1);

        let err = Routed::<Value>::new(&kalshi.inner, routes::EXCHANGE_STATUS, &[])
            .send()
            .await
            .unwrap_err();
        assert!(
            matches!(&err, PredictionsError::Api(ApiError::Refused(refused))
                if refused.layer == READ && refused.units == 10 && refused.capacity == 5),
            "{err:?}"
        );
        mock.assert_async().await;
    }
}
