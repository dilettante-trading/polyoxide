//! Measures each route's weight on the live host and compares it with
//! `Route::cost`, the table the client charges.
//!
//! ```sh
//! cargo run -p polyoxide-binance --example weight_probe
//! ```
//!
//! A route's weight is the rise in `X-MBX-USED-WEIGHT-1M` across its request,
//! so every case runs back to back inside one UTC minute; the probe waits for a
//! fresh minute first. Another process spending weight on the same IP makes a
//! row read high. Costs about 150 weight. The funding routes report no header
//! and are not probed. Exits 1 if any row differs from the table.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use polyoxide_binance::{
    usdm::types::DepthLimit,
    weight::{Cost, Route},
};

const BASE: &str = "https://fapi.binance.com";

/// `(route, path and query)`. The query carries exactly the parameters the
/// route names, which `every_case_requests_the_route_it_checks` holds.
fn cases() -> Vec<(Route, String)> {
    let mut cases = vec![
        (Route::Ping, "/fapi/v1/ping".to_owned()),
        (Route::Time, "/fapi/v1/time".to_owned()),
        (Route::ExchangeInfo, "/fapi/v1/exchangeInfo".to_owned()),
        (
            Route::Ticker24h { all: false },
            "/fapi/v1/ticker/24hr?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Ticker24h { all: true },
            "/fapi/v1/ticker/24hr".to_owned(),
        ),
        (
            Route::PremiumIndex { all: false },
            "/fapi/v1/premiumIndex?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::PremiumIndex { all: true },
            "/fapi/v1/premiumIndex".to_owned(),
        ),
        (
            Route::OpenInterest,
            "/fapi/v1/openInterest?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::AggTrades,
            "/fapi/v1/aggTrades?symbol=BTCUSDT&limit=1".to_owned(),
        ),
        (
            Route::Klines { limit: None },
            "/fapi/v1/klines?symbol=BTCUSDT&interval=1m".to_owned(),
        ),
        (
            Route::Depth { limit: None },
            "/fapi/v1/depth?symbol=BTCUSDT".to_owned(),
        ),
    ];
    for limit in [100u32, 101, 500, 501, 1000, 1001] {
        cases.push((
            Route::Klines { limit: Some(limit) },
            format!("/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit={limit}"),
        ));
    }
    for limit in DepthLimit::ALL {
        cases.push((
            Route::Depth {
                limit: Some(*limit),
            },
            format!("/fapi/v1/depth?symbol=BTCUSDT&limit={limit}"),
        ));
    }
    cases
}

async fn used_after(client: &reqwest::Client, path: &str) -> u32 {
    let response = client
        .get(format!("{BASE}{path}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    response
        .headers()
        .get("x-mbx-used-weight-1m")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            panic!(
                "{path} answered {} with no weight header",
                response.status()
            )
        })
}

async fn wait_for_a_fresh_minute() {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        % 60_000;
    if ms > 20_000 {
        let wait = 61_000 - ms;
        eprintln!("waiting {} s for a fresh minute", wait / 1000);
        tokio::time::sleep(Duration::from_millis(wait)).await;
    }
}

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder().gzip(true).build().unwrap();
    wait_for_a_fresh_minute().await;
    let mut previous = used_after(&client, "/fapi/v1/ping").await;
    let mut differing = 0;
    for (route, path) in cases() {
        let now = used_after(&client, &path).await;
        let measured = now.saturating_sub(previous);
        previous = now;
        let Cost::Weight(table) = route.cost() else {
            continue;
        };
        let verdict = if measured == table {
            "ok"
        } else {
            differing += 1;
            "DIFFERS"
        };
        println!("{path:58} table {table:>3}  measured {measured:>3}  {verdict}");
    }
    if differing > 0 {
        eprintln!("{differing} rows differ from Route::cost; update it and docs/specs/binance/OBSERVED.md");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_requests_the_route_it_checks() {
        for (route, path) in cases() {
            assert!(path.starts_with(route.path()), "{path} is not {route:?}");
            let limit = path
                .split(['?', '&'])
                .find_map(|kv| kv.strip_prefix("limit="));
            let one_symbol = path.contains("symbol=");
            match route {
                Route::Klines { limit: expected } => {
                    assert_eq!(limit.map(|l| l.parse::<u32>().unwrap()), expected, "{path}")
                }
                Route::Depth { limit: expected } => {
                    assert_eq!(limit, expected.map(DepthLimit::as_str), "{path}")
                }
                Route::Ticker24h { all } | Route::PremiumIndex { all } => {
                    assert_eq!(one_symbol, !all, "{path}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn every_depth_limit_and_klines_band_edge_is_probed() {
        let routes: Vec<Route> = cases().into_iter().map(|(route, _)| route).collect();
        for limit in DepthLimit::ALL {
            assert!(
                routes.contains(&Route::Depth {
                    limit: Some(*limit)
                }),
                "{limit}"
            );
        }
        for limit in [100, 101, 500, 501, 1000, 1001] {
            assert!(
                routes.contains(&Route::Klines { limit: Some(limit) }),
                "{limit}"
            );
        }
    }
}
