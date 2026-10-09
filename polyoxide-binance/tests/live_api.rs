//! Live integration tests against `fapi.binance.com`.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d.
//! No credentials are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --test live_api -- --ignored
//! ```
//!
//! `live_responses_carry_no_unmodelled_keys` is the host's drift detector:
//! Binance publishes no schema, so nothing else notices a new field. It sees
//! keys, and the length of the positional kline and book rows. It does not see
//! a new value of an enum, which decodes as that enum's `Other` variant.

use polyoxide_test_support::agreement as common;

use std::time::Duration;

use polyoxide_binance::{
    usdm::types::{
        AggTrade, ContractType, Depth, DepthLimit, ExchangeInfo, FundingInfo, FundingRate,
        Interval, Kline, OpenInterest, PremiumIndex, ServerTime, Symbol, SymbolStatus, Ticker24h,
    },
    BinanceError, Usdm,
};
use polyoxide_core::ApiError;
use polyoxide_test_support::{environmental, fail, transient, ResultExt};
use serde::{de::DeserializeOwned, Serialize};

fn client() -> Usdm {
    Usdm::new().or_fail("binance client")
}

/// BTCUSDT, a trading TradFi perpetual and a trading Chinese-character
/// perpetual, chosen from today's `exchangeInfo` so a delisting cannot break
/// the suite.
async fn three_kinds_of_symbol(usdm: &Usdm) -> Vec<Symbol> {
    let info = usdm
        .exchange()
        .exchange_info()
        .send()
        .await
        .or_fail("exchangeInfo");
    let trading =
        |s: &&polyoxide_binance::usdm::types::SymbolInfo| s.status == SymbolStatus::Trading;
    let tradfi = info
        .symbols
        .iter()
        .filter(trading)
        .find(|s| s.contract_type == ContractType::TradifiPerpetual)
        .unwrap_or_else(|| {
            environmental("no suitable market: no trading TradFi perpetual is listed")
        });
    let chinese = info
        .symbols
        .iter()
        .filter(trading)
        .find(|s| !s.symbol.as_str().is_ascii())
        .unwrap_or_else(|| {
            environmental("no suitable market: no trading Chinese-character symbol is listed")
        });
    vec![
        Symbol::new("BTCUSDT").or_fail("BTCUSDT"),
        Symbol::new(&*tradfi.symbol).or_fail(&tradfi.symbol),
        Symbol::new(&*chinese.symbol).or_fail(&chinese.symbol),
    ]
}

#[tokio::test]
#[ignore]
async fn live_ping_time_and_the_weight_header() {
    let usdm = client();
    let latency = usdm.health().ping().await.or_fail("ping");
    assert!(latency < Duration::from_secs(10), "latency {latency:?}");

    // Two clients, each with its own budget: the second has charged only its
    // own request, so a count of 2 or more can only be the server's header,
    // which counts the first client's request too. Both counts reset on the
    // minute, so a pair that straddles one is sent again.
    for _ in 0..3 {
        let before = unix_minute();
        let (first, second) = (client(), client());
        let a = first.health().time().send().await.or_fail("time");
        let b = second.health().time().send().await.or_fail("time");
        assert!(b.server_time > 1_790_000_000_000);
        if a.server_time / 60_000 != b.server_time / 60_000 || unix_minute() != before {
            continue;
        }
        let used = second.weight_budget().used();
        assert!(
            used >= 2,
            "X-MBX-USED-WEIGHT-1M was not recorded: the budget counts {used}, and its own charge is 1"
        );
        return;
    }
    // Each pair takes a fraction of a second, so three straddles in a row is a
    // stalled host rather than bad luck, and a retry runs on fresh minutes.
    transient("three pairs of requests each straddled a minute boundary");
}

fn unix_minute() -> u64 {
    polyoxide_venue::UnixMillis::now().0 / 60_000
}

#[tokio::test]
#[ignore]
async fn live_every_market_route_answers_for_three_kinds_of_symbol() {
    let usdm = client();
    for symbol in three_kinds_of_symbol(&usdm).await {
        let market = usdm.market();
        let ticker = market.ticker_24h(&symbol).send().await.or_fail("ticker");
        assert_eq!(ticker.symbol, symbol.as_str());
        let index = market
            .premium_index(&symbol)
            .send()
            .await
            .or_fail("premiumIndex");
        assert!(
            index.mark_price > 0.into(),
            "{symbol} mark {}",
            index.mark_price
        );
        let candles = market
            .klines(&symbol, Interval::M1)
            .limit(2)
            .send()
            .await
            .or_fail("klines");
        assert_eq!(candles.len(), 2, "{symbol}");
        let oi = market
            .open_interest(&symbol)
            .send()
            .await
            .or_fail("openInterest");
        assert_eq!(oi.symbol, symbol.as_str());
        let trades = market
            .agg_trades(&symbol)
            .limit(2)
            .send()
            .await
            .or_fail("aggTrades");
        assert!(trades.len() <= 2, "{symbol}");
        let book = market
            .depth(&symbol)
            .limit(DepthLimit::Five)
            .send()
            .await
            .or_fail("depth");
        assert!(book.bids.len() <= 5 && book.asks.len() <= 5, "{symbol}");
        let funding = market
            .funding_rate()
            .symbol(&symbol)
            .limit(2)
            .send()
            .await
            .or_fail("fundingRate");
        assert!(funding.iter().all(|row| row.symbol == symbol.as_str()));
    }
}

#[tokio::test]
#[ignore]
async fn live_an_unknown_symbol_is_venue_error_1121() {
    let err = client()
        .market()
        .open_interest(&Symbol::new("NOTASYMBOLUSDT").or_fail("NOTASYMBOLUSDT"))
        .send()
        .await
        .unwrap_err(); // live-unwraps: the venue must refuse an unknown symbol

    // Any other error fails by its class, so a 5xx or a dropped connection is
    // retried rather than filed.
    if !matches!(
        err,
        BinanceError::Venue {
            status: 400,
            code: -1121,
            ..
        }
    ) {
        fail("expected venue error -1121", &err);
    }
}

/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("https://fapi.binance.com{path}"))
        .send()
        .await
        .map_err(ApiError::Network)
        .or_fail(path);
    let status = response.status();
    // A refusal fails as core classifies its status, so its tag says what the
    // nightly does with it: a 451 is environmental, a 429 or a 5xx transient,
    // and a 403 from the firewall or a 418 ban real.
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        fail(
            path,
            &ApiError::from_status_and_body(status.as_u16(), &body),
        );
    }
    response
        .text()
        .await
        .map_err(ApiError::Network)
        .or_fail(path)
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("a JSON body") // live-unwraps: an assertion on the response
}

/// Positional rows of any length but `len`. The key comparison cannot see a
/// value appended to a kline or a book level, since neither has keys.
fn rows_not_of_length(path: &str, rows: &serde_json::Value, len: usize) -> Vec<String> {
    rows.as_array()
        .expect("an array of rows") // live-unwraps: an assertion on the response
        .iter()
        .map(|row| row.as_array().expect("a positional row").len()) // live-unwraps: an assertion on the response
        .filter(|&n| n != len)
        .map(|n| format!("{path}: a row of {n} values, not {len}"))
        .collect()
}

fn agrees<T: DeserializeOwned + Serialize>(path: &str, text: &str) -> Vec<String> {
    let diff = common::compare::<T>(path, text);
    // A field the type models but this response omits is not drift: it may
    // be sent only sometimes. A key the type does not model is.
    if !diff.invented.is_empty() {
        eprintln!(
            "{path}: modelled but not sent this time: {:?}",
            diff.invented
        );
    }
    diff.unmodelled
        .into_iter()
        .map(|key| format!("{path}: {key}"))
        .collect()
}

#[tokio::test]
#[ignore]
async fn live_responses_carry_no_unmodelled_keys() {
    let http = reqwest::Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("a reqwest client"); // live-unwraps: builds a local client
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/fapi/v1/time";
    check(agrees::<ServerTime>(path, &raw(&http, path).await));
    let path = "/fapi/v1/exchangeInfo";
    check(agrees::<ExchangeInfo>(path, &raw(&http, path).await));
    let path = "/fapi/v1/fundingInfo";
    check(agrees::<Vec<FundingInfo>>(path, &raw(&http, path).await));
    let path = "/fapi/v1/ticker/24hr";
    check(agrees::<Vec<Ticker24h>>(path, &raw(&http, path).await));
    let path = "/fapi/v1/premiumIndex";
    check(agrees::<Vec<PremiumIndex>>(path, &raw(&http, path).await));
    let path = "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=3";
    let text = raw(&http, path).await;
    check(agrees::<Vec<Kline>>(path, &text));
    check(rows_not_of_length(path, &json(&text), 12));
    let path = "/fapi/v1/fundingRate?limit=100";
    check(agrees::<Vec<FundingRate>>(path, &raw(&http, path).await));
    let path = "/fapi/v1/openInterest?symbol=BTCUSDT";
    check(agrees::<OpenInterest>(path, &raw(&http, path).await));
    let path = "/fapi/v1/aggTrades?symbol=BTCUSDT&limit=10";
    check(agrees::<Vec<AggTrade>>(path, &raw(&http, path).await));
    let path = "/fapi/v1/depth?symbol=BTCUSDT&limit=5";
    let text = raw(&http, path).await;
    check(agrees::<Depth>(path, &text));
    let book = json(&text);
    check(rows_not_of_length(path, &book["bids"], 2));
    check(rows_not_of_length(path, &book["asks"], 2));

    assert!(
        unmodelled.is_empty(),
        "the host sent what the types do not model; model it and record it in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}
