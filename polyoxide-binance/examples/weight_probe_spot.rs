//! Measures each spot route's weight on the live host and compares it with
//! `Route::cost`, the table the client charges.
//!
//! ```sh
//! cargo run -p polyoxide-binance --no-default-features --features spot --example weight_probe_spot
//! ```
//!
//! A route's weight is the rise in `X-MBX-USED-WEIGHT-1M` across its request,
//! so every case runs back to back inside one UTC minute; the probe waits for a
//! fresh minute first. Another process spending weight on the same IP makes a
//! row read high, and so does one calling `data-api.binance.vision`, which
//! shares the counter. Costs about 870 weight. Exits 1 if any row differs from
//! the table.

use std::time::Duration;

use polyoxide_binance::spot::weight::{Cost, Route};
use polyoxide_test_support::minute;

const BASE: &str = "https://api.binance.com";

/// The depth limits probed: the edges of every band, and one past the
/// documented maximum.
const DEPTH_LIMITS: [u32; 8] = [1, 100, 101, 500, 501, 1000, 1001, 5001];

/// `(route, path and query)`. The query carries exactly the parameters the
/// route names, which `every_case_requests_the_route_it_checks` holds.
fn cases() -> Vec<(Route, String)> {
    let mut cases = vec![
        (Route::Ping, "/api/v3/ping".to_owned()),
        (Route::Time, "/api/v3/time".to_owned()),
        (Route::ExchangeInfo, "/api/v3/exchangeInfo".to_owned()),
        (
            Route::ExchangeInfo,
            "/api/v3/exchangeInfo?symbol=BTCUSDT".to_owned(),
        ),
        (Route::Trades, "/api/v3/trades?symbol=BTCUSDT".to_owned()),
        (
            Route::Trades,
            "/api/v3/trades?symbol=BTCUSDT&limit=1000".to_owned(),
        ),
        (
            Route::AggTrades,
            "/api/v3/aggTrades?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::AggTrades,
            "/api/v3/aggTrades?symbol=BTCUSDT&limit=1000".to_owned(),
        ),
        (
            Route::Klines,
            "/api/v3/klines?symbol=BTCUSDT&interval=1m".to_owned(),
        ),
        (
            Route::Klines,
            "/api/v3/klines?symbol=BTCUSDT&interval=1m&limit=1000".to_owned(),
        ),
        (
            Route::AvgPrice,
            "/api/v3/avgPrice?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Ticker24h { all: false },
            "/api/v3/ticker/24hr?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Ticker24h { all: true },
            "/api/v3/ticker/24hr".to_owned(),
        ),
        (
            Route::Price { all: false },
            "/api/v3/ticker/price?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Price { all: true },
            "/api/v3/ticker/price".to_owned(),
        ),
        (
            Route::BookTicker { all: false },
            "/api/v3/ticker/bookTicker?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::BookTicker { all: true },
            "/api/v3/ticker/bookTicker".to_owned(),
        ),
        (
            Route::Depth { limit: None },
            "/api/v3/depth?symbol=BTCUSDT".to_owned(),
        ),
    ];
    for limit in DEPTH_LIMITS {
        cases.push((
            Route::Depth { limit: Some(limit) },
            format!("/api/v3/depth?symbol=BTCUSDT&limit={limit}"),
        ));
    }
    cases
}

/// Sends one request and returns the minute's count after it. Stops the probe
/// on any answer but 200: a refused request is not a call the client makes,
/// and sending on after a 429 is how a ban starts.
async fn used_after(client: &polyoxide_core::reqwest::Client, path: &str) -> u32 {
    let response = client
        .get(format!("{BASE}{path}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    let status = response.status();
    if status != polyoxide_core::reqwest::StatusCode::OK {
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("none")
            .to_owned();
        let body = response.text().await.unwrap_or_default();
        eprintln!("{path} answered {status} (Retry-After {retry_after}): {body:.300}");
        eprintln!("stopping: nothing further is sent");
        std::process::exit(2);
    }
    response
        .headers()
        .get("x-mbx-used-weight-1m")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("{path} answered {status} with no weight header"))
}

async fn wait_for_a_fresh_minute() {
    // Three seconds past the boundary: the server's count was seen to fall to
    // 1 only within 3 s of it, and the local clock is not the server's.
    if let Some(wait) = minute::wait_needed(3_000..=20_000, 3_000, minute::ms_into_minute()) {
        eprintln!(
            "waiting {} s for a fresh minute",
            wait.as_millis().div_ceil(1000)
        );
        tokio::time::sleep(wait).await;
    }
}

#[tokio::main]
async fn main() {
    let client = polyoxide_core::reqwest::Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    wait_for_a_fresh_minute().await;
    let mut previous = used_after(&client, "/api/v3/ping").await;
    if previous > 1 {
        eprintln!(
            "the minute's count is {previous} after one ping: another process on this IP is \
             spending weight, and rows may differ"
        );
    }
    let mut differing = 0;
    for (route, path) in cases() {
        let now = used_after(&client, &path).await;
        if now < previous {
            eprintln!(
                "{path}: the minute's count fell from {previous} to {now}, so the minute \
                 rolled over mid-run; run the probe again"
            );
            std::process::exit(2);
        }
        let measured = now - previous;
        previous = now;
        let Cost::Weight(table) = route.cost();
        let verdict = if measured == table {
            "ok"
        } else {
            differing += 1;
            "DIFFERS"
        };
        println!("{path:58} table {table:>3}  measured {measured:>3}  {verdict}");
    }
    if differing > 0 {
        eprintln!("{differing} rows differ from Route::cost; run the probe again, and if they still differ, update it and docs/specs/binance/OBSERVED.md");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_requests_the_route_it_checks() {
        for (route, path) in cases() {
            assert_eq!(path.split('?').next(), Some(route.path()), "{path}");
            let limit = path
                .split(['?', '&'])
                .find_map(|kv| kv.strip_prefix("limit="));
            let one_symbol = path.contains("symbol=");
            match route {
                Route::Ping | Route::Time => assert!(!path.contains('?'), "{path}"),
                Route::Depth { limit: expected } => {
                    assert_eq!(limit.map(|l| l.parse::<u32>().unwrap()), expected, "{path}")
                }
                Route::Ticker24h { all } | Route::Price { all } | Route::BookTicker { all } => {
                    assert_eq!(one_symbol, !all, "{path}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn every_depth_band_edge_is_probed() {
        let routes: Vec<Route> = cases().into_iter().map(|(route, _)| route).collect();
        assert!(routes.contains(&Route::Depth { limit: None }));
        for limit in [1, 100, 101, 500, 501, 1000, 1001] {
            assert!(
                routes.contains(&Route::Depth { limit: Some(limit) }),
                "{limit}"
            );
        }
    }

    #[test]
    fn every_route_is_probed_with_and_without_what_does_not_change_its_weight() {
        let cases = cases();
        let probed = |route: Route| cases.iter().any(|(r, _)| *r == route);
        for route in [
            Route::Ping,
            Route::Time,
            Route::ExchangeInfo,
            Route::Trades,
            Route::AggTrades,
            Route::Klines,
            Route::AvgPrice,
            Route::Ticker24h { all: false },
            Route::Ticker24h { all: true },
            Route::Price { all: false },
            Route::Price { all: true },
            Route::BookTicker { all: false },
            Route::BookTicker { all: true },
        ] {
            assert!(probed(route), "{route:?}");
        }
        // The routes whose weight Binance says does not depend on `limit` or
        // `symbol` are probed both ways, so a change in either shows.
        for (route, with) in [
            (Route::Trades, "limit="),
            (Route::AggTrades, "limit="),
            (Route::Klines, "limit="),
            (Route::ExchangeInfo, "symbol="),
        ] {
            let paths: Vec<&String> = cases
                .iter()
                .filter(|(r, _)| *r == route)
                .map(|(_, p)| p)
                .collect();
            assert!(paths.iter().any(|p| p.contains(with)), "{route:?} with");
            assert!(paths.iter().any(|p| !p.contains(with)), "{route:?} without");
        }
    }
}
