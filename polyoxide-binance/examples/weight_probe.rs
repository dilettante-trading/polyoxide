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

use std::time::Duration;

use polyoxide_binance::{
    usdm::types::DepthLimit,
    weight::{Cost, Route},
};
use polyoxide_test_support::minute;

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
    let mut previous = used_after(&client, "/fapi/v1/ping").await;
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
