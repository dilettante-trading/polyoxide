//! The requests a soak sends: one route, and a space of URLs that never
//! repeats within a run.
//!
//! Several v2 routes are cached at CloudFront (`OBSERVED.md`): a repeated URL
//! is answered from the cache, with the same trace id, without reaching the
//! origin. A soak that repeats URLs measures the cache and reports a clean run
//! at any rate. So every probe URL is a distinct point in a mixed-radix space
//! over live wallets or markets and harmless parameter variations, and
//! [`SeenUrls`] refuses a repeat as a backstop.

use std::{
    collections::HashSet,
    fmt,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

/// A v2 route under measurement. Each maps to the path its rate-limit row
/// matches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Positions,
    ComboPositions,
    Trades,
    Activity,
    UserPnl,
    HoldersPnl,
}

impl Route {
    pub const ALL: [Route; 6] = [
        Route::Positions,
        Route::ComboPositions,
        Route::Trades,
        Route::Activity,
        Route::UserPnl,
        Route::HoldersPnl,
    ];

    /// The request path, which is also what `RateLimiter::acquire` matches.
    pub fn path(self) -> &'static str {
        match self {
            Route::Positions => "/v2/positions",
            Route::ComboPositions => "/v2/positions/combos",
            Route::Trades => "/v2/trades",
            Route::Activity => "/v2/activity",
            Route::UserPnl => "/v2/user-pnl",
            Route::HoldersPnl => "/v2/holders",
        }
    }

    /// The name used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Route::Positions => "positions",
            Route::ComboPositions => "combo-positions",
            Route::Trades => "trades",
            Route::Activity => "activity",
            Route::UserPnl => "user-pnl",
            Route::HoldersPnl => "holders-pnl",
        }
    }

    /// Whether the route is keyed by market rather than by wallet.
    pub fn needs_conditions(self) -> bool {
        matches!(self, Route::HoldersPnl)
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Route {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Route::ALL
            .into_iter()
            .find(|r| r.name() == s)
            .ok_or_else(|| {
                let names: Vec<_> = Route::ALL.iter().map(|r| r.name()).collect();
                format!("unknown route {s:?}; expected one of {}", names.join(", "))
            })
    }
}

/// Parses `all`, or one or more comma-separated route names.
pub fn parse_routes(raw: &str) -> Result<Vec<Route>, String> {
    if raw == "all" {
        return Ok(Route::ALL.to_vec());
    }
    let routes = raw
        .split(',')
        .map(|name| name.trim().parse::<Route>())
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen = HashSet::new();
    if let Some(repeat) = routes.iter().find(|route| !seen.insert(**route)) {
        return Err(format!("{repeat} is listed twice"));
    }
    Ok(routes)
}

/// Live identifiers the probes are built from.
#[derive(Debug, Clone, Default)]
pub struct Pools {
    pub wallets: Vec<String>,
    pub conditions: Vec<String>,
}

const STATUSES: [&str; 3] = ["OPEN", "REDEEMABLE", "CLOSED"];
const DIRECTIONS: [&str; 2] = ["DESC", "ASC"];
const TAKER_ONLY: [&str; 2] = ["true", "false"];
const PNL_INTERVALS: [&str; 7] = ["max", "all", "1m", "1w", "1d", "12h", "6h"];
const PNL_FIDELITIES: [&str; 5] = ["1d", "18h", "12h", "3h", "1h"];

/// Page sizes varied for uniqueness. A narrow band keeps each request's
/// weight roughly constant across the run.
const LIMIT_BAND: std::ops::RangeInclusive<u64> = 50..=100;
/// `include_pnl=true` caps `limit` at 100 upstream.
const HOLDERS_LIMIT_BAND: std::ops::RangeInclusive<u64> = 1..=100;

fn band_len(band: &std::ops::RangeInclusive<u64>) -> u64 {
    band.end() - band.start() + 1
}

/// Splits `index` into digits of the given radices, least significant first.
fn mixed_radix(mut index: u64, radices: &[u64]) -> Vec<u64> {
    radices
        .iter()
        .map(|radix| {
            let digit = index % radix;
            index /= radix;
            digit
        })
        .collect()
}

/// A route's probe space: distinct URLs addressed by an index.
pub struct ProbeSource {
    route: Route,
    pools: Pools,
    offset: u64,
    next: AtomicU64,
}

impl ProbeSource {
    /// `offset` shifts where in the space the run starts, so two runs a few
    /// minutes apart do not replay the same URLs into a warm cache.
    pub fn new(route: Route, pools: Pools, offset: u64) -> Result<Self, String> {
        let keys = if route.needs_conditions() {
            pools.conditions.len()
        } else {
            pools.wallets.len()
        };
        if keys == 0 {
            return Err(format!(
                "{route} needs at least one {}",
                if route.needs_conditions() {
                    "condition"
                } else {
                    "wallet"
                }
            ));
        }
        let capacity = Self::radices_for(route, keys).iter().product::<u64>();
        Ok(Self {
            route,
            pools,
            offset: offset % capacity,
            next: AtomicU64::new(0),
        })
    }

    fn radices_for(route: Route, keys: usize) -> Vec<u64> {
        let keys = keys as u64;
        let limits = band_len(&LIMIT_BAND);
        match route {
            Route::Positions => vec![keys, STATUSES.len() as u64, limits],
            Route::ComboPositions => vec![keys, DIRECTIONS.len() as u64, limits],
            Route::Trades => vec![keys, TAKER_ONLY.len() as u64, limits],
            Route::Activity => vec![keys, DIRECTIONS.len() as u64, limits],
            Route::UserPnl => vec![
                keys,
                PNL_INTERVALS.len() as u64,
                PNL_FIDELITIES.len() as u64,
            ],
            Route::HoldersPnl => vec![keys, band_len(&HOLDERS_LIMIT_BAND)],
        }
    }

    fn radices(&self) -> Vec<u64> {
        let keys = if self.route.needs_conditions() {
            self.pools.conditions.len()
        } else {
            self.pools.wallets.len()
        };
        Self::radices_for(self.route, keys)
    }

    /// How many distinct URLs the space holds.
    pub fn capacity(&self) -> u64 {
        self.radices().iter().product()
    }

    /// The path and query for position `index` in the space. A bijection on
    /// `0..capacity()`, so distinct indices give distinct URLs.
    pub fn url(&self, index: u64) -> String {
        let d = mixed_radix((index + self.offset) % self.capacity(), &self.radices());
        let limit = |band: &std::ops::RangeInclusive<u64>, digit: u64| band.start() + digit;
        let path = self.route.path();
        match self.route {
            Route::Positions => format!(
                "{path}?user={}&status={}&limit={}",
                self.pools.wallets[d[0] as usize],
                STATUSES[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::ComboPositions => format!(
                "{path}?user={}&sort_direction={}&limit={}",
                self.pools.wallets[d[0] as usize],
                DIRECTIONS[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::Trades => format!(
                "{path}?user={}&taker_only={}&limit={}",
                self.pools.wallets[d[0] as usize],
                TAKER_ONLY[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::Activity => format!(
                "{path}?user={}&sort_direction={}&limit={}",
                self.pools.wallets[d[0] as usize],
                DIRECTIONS[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::UserPnl => format!(
                "{path}?user={}&interval={}&fidelity={}",
                self.pools.wallets[d[0] as usize],
                PNL_INTERVALS[d[1] as usize],
                PNL_FIDELITIES[d[2] as usize]
            ),
            Route::HoldersPnl => format!(
                "{path}?condition={}&include_pnl=true&limit={}",
                self.pools.conditions[d[0] as usize],
                limit(&HOLDERS_LIMIT_BAND, d[1])
            ),
        }
    }

    /// The next unused URL, or `None` once the space is exhausted.
    pub fn next_url(&self) -> Option<String> {
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        (index < self.capacity()).then(|| self.url(index))
    }
}

/// Every URL a run has sent. A repeat would be answered from the CDN cache.
#[derive(Default)]
pub struct SeenUrls(Mutex<HashSet<String>>);

impl SeenUrls {
    /// Records `url`; `false` if it was already sent.
    pub fn insert(&self, url: &str) -> bool {
        let mut seen = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        seen.insert(url.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pools(wallets: usize, conditions: usize) -> Pools {
        Pools {
            wallets: (0..wallets).map(|i| format!("0xw{i}")).collect(),
            conditions: (0..conditions).map(|i| format!("0xc{i}")).collect(),
        }
    }

    #[test]
    fn every_route_round_trips_through_its_name() {
        for route in Route::ALL {
            assert_eq!(route.name().parse::<Route>(), Ok(route));
        }
        assert!("closed-positions".parse::<Route>().is_err());
    }

    #[test]
    fn routes_parse_from_a_list_or_all() {
        assert_eq!(parse_routes("all"), Ok(Route::ALL.to_vec()));
        assert_eq!(
            parse_routes("trades, user-pnl"),
            Ok(vec![Route::Trades, Route::UserPnl])
        );
        assert!(parse_routes("trades,trades").is_err());
        assert!(parse_routes("trades,nope").is_err());
    }

    #[test]
    fn mixed_radix_digits_are_least_significant_first() {
        assert_eq!(mixed_radix(0, &[3, 2]), vec![0, 0]);
        assert_eq!(mixed_radix(1, &[3, 2]), vec![1, 0]);
        assert_eq!(mixed_radix(3, &[3, 2]), vec![0, 1]);
        assert_eq!(mixed_radix(5, &[3, 2]), vec![2, 1]);
    }

    #[test]
    fn every_url_in_a_space_is_distinct() {
        // The property the whole soak rests on: a repeat reaches the cache.
        for route in Route::ALL {
            let source = ProbeSource::new(route, pools(4, 3), 0).unwrap();
            let urls: HashSet<String> = (0..source.capacity()).map(|i| source.url(i)).collect();
            assert_eq!(
                urls.len() as u64,
                source.capacity(),
                "{route} repeated a URL"
            );
        }
    }

    #[test]
    fn urls_target_the_route_path_and_respect_its_limit_band() {
        let source = ProbeSource::new(Route::HoldersPnl, pools(0, 2), 0).unwrap();
        for i in 0..source.capacity() {
            let url = source.url(i);
            assert!(url.starts_with("/v2/holders?condition=0xc"), "{url}");
            let limit: u64 = url.rsplit("limit=").next().unwrap().parse().unwrap();
            assert!(
                (1..=100).contains(&limit),
                "include_pnl caps limit at 100: {url}"
            );
        }
        let positions = ProbeSource::new(Route::Positions, pools(1, 0), 0).unwrap();
        assert_eq!(
            positions.url(0),
            "/v2/positions?user=0xw0&status=OPEN&limit=50"
        );
    }

    #[test]
    fn the_offset_rotates_the_start_without_leaving_the_space() {
        let plain = ProbeSource::new(Route::UserPnl, pools(2, 0), 0).unwrap();
        let shifted = ProbeSource::new(Route::UserPnl, pools(2, 0), 5).unwrap();
        assert_eq!(shifted.url(0), plain.url(5));
        let wrapped = ProbeSource::new(Route::UserPnl, pools(2, 0), plain.capacity() + 1).unwrap();
        assert_eq!(wrapped.url(0), plain.url(1));
    }

    #[test]
    fn next_url_stops_at_capacity() {
        let source = ProbeSource::new(Route::HoldersPnl, pools(0, 1), 0).unwrap();
        let drawn = std::iter::from_fn(|| source.next_url()).count() as u64;
        assert_eq!(drawn, source.capacity());
        assert_eq!(source.next_url(), None);
    }

    #[test]
    fn an_empty_pool_is_refused() {
        assert!(ProbeSource::new(Route::Positions, pools(0, 5), 0).is_err());
        assert!(ProbeSource::new(Route::HoldersPnl, pools(5, 0), 0).is_err());
    }

    #[test]
    fn seen_urls_refuses_a_repeat() {
        let seen = SeenUrls::default();
        assert!(seen.insert("/v2/trades?user=a"));
        assert!(!seen.insert("/v2/trades?user=a"));
        assert!(seen.insert("/v2/trades?user=b"));
    }
}
