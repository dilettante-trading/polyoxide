//! What every route builder sends through: the route table, and core's request builder with the
//! route's cost on the Read layer.

use std::fmt::Write as _;

use polyoxide_core::LayerId;

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
