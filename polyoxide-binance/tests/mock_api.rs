//! Mock-server tests: every route's path and exact query, the decoding of a
//! captured body, error mapping, and the budget's response to the server.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::{
    usdm::types::{DepthLimit, Interval, Symbol},
    weight::Cost,
    BinanceError, Usdm,
};
use polyoxide_core::RetryConfig;

fn usdm(server: &ServerGuard) -> Usdm {
    Usdm::builder().base_url(server.url()).build().unwrap()
}

fn usdm_with_retries(server: &ServerGuard, config: RetryConfig) -> Usdm {
    Usdm::builder()
        .base_url(server.url())
        .with_retry_config(config)
        .build()
        .unwrap()
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/rest/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

/// A route answering `body`, matched on its whole query string so a missing
/// or extra parameter fails the request.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body(body)
        .create_async()
        .await
}

/// Waits out the end of a minute, so a test that holds the budget for a moment
/// cannot see the window roll over underneath it.
async fn clear_of_a_minute_boundary() {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        % 60_000;
    if ms > 57_000 {
        tokio::time::sleep(Duration::from_millis(60_100 - ms)).await;
    }
}

// ── health and exchange ─────────────────────────────────────────

#[tokio::test]
async fn ping_reports_latency() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/fapi/v1/ping", "", "{}").await;
    let latency = usdm(&server).health().ping().await.unwrap();
    mock.assert_async().await;
    assert!(latency < Duration::from_secs(5));
}

#[tokio::test]
async fn time_returns_the_server_clock() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/fapi/v1/time", "", &fixture("time")).await;
    let time = usdm(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert!(time.server_time > 1_790_000_000_000);
}

#[tokio::test]
async fn exchange_info_decodes_the_captured_body() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
    )
    .await;
    let info = usdm(&server)
        .exchange()
        .exchange_info()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(info.futures_type, "U_MARGINED");
    assert_eq!(info.symbols.len(), 5);
}

#[tokio::test]
async fn funding_info_decodes_a_null_update_time() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/fundingInfo",
        "",
        &fixture("funding_info"),
    )
    .await;
    let rows = usdm(&server)
        .exchange()
        .funding_info()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert!(rows.iter().any(|row| row.update_time.is_none()));
}

// ── market ──────────────────────────────────────────────────────

#[tokio::test]
async fn ticker_24h_sends_the_symbol_and_tickers_24h_sends_nothing() {
    let mut server = Server::new_async().await;
    let all = fixture("ticker_24hr");
    let rows: Vec<serde_json::Value> = serde_json::from_str(&all).unwrap();
    let one = rows
        .iter()
        .find(|row| row["symbol"] == "BTCUSDT")
        .unwrap()
        .to_string();
    let single = route(&mut server, "/fapi/v1/ticker/24hr", "symbol=BTCUSDT", &one).await;
    let every = route(&mut server, "/fapi/v1/ticker/24hr", "", &all).await;

    let client = usdm(&server);
    let ticker = client.market().ticker_24h(&btc()).send().await.unwrap();
    assert_eq!(ticker.symbol, "BTCUSDT");
    let tickers = client.market().tickers_24h().send().await.unwrap();
    assert_eq!(tickers.len(), 3);
    single.assert_async().await;
    every.assert_async().await;
}

#[tokio::test]
async fn a_chinese_character_symbol_is_percent_encoded() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "symbol=%E5%B8%81%E5%AE%89%E4%BA%BA%E7%94%9FUSDT",
        r#"{"symbol":"币安人生USDT","markPrice":"0.48683834","indexPrice":"0.48711513","estimatedSettlePrice":"0.48795216","lastFundingRate":"0.00005000","interestRate":"0.00005000","nextFundingTime":1791360000000,"time":1791354113000}"#,
    )
    .await;
    let symbol = Symbol::new("币安人生USDT").unwrap();
    let index = usdm(&server)
        .market()
        .premium_index(&symbol)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(index.symbol, symbol.as_str());
}

#[tokio::test]
async fn premium_indices_sends_no_symbol() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "",
        &fixture("premium_index"),
    )
    .await;
    let rows = usdm(&server)
        .market()
        .premium_indices()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
async fn klines_send_every_parameter_once() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/klines",
        "symbol=BTCUSDT&interval=1h&startTime=1791350000000&endTime=1791354000000&limit=24",
        &fixture("klines"),
    )
    .await;
    let candles = usdm(&server)
        .market()
        .klines(&btc(), Interval::H1)
        .start_time(1_791_350_000_000)
        .end_time(1_791_354_000_000)
        .limit(100)
        .limit(24)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(candles.len(), 2);
}

#[tokio::test]
async fn funding_rate_sends_its_filters_and_decodes_old_rows() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/fundingRate",
        "symbol=BTCUSDT&startTime=1568102400000&limit=2",
        &fixture("funding_rate_2019"),
    )
    .await;
    let rows = usdm(&server)
        .market()
        .funding_rate()
        .symbol(&btc())
        .start_time(1_568_102_400_000)
        .limit(2)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert!(rows.iter().all(|row| row.mark_price.is_none()));
}

#[tokio::test]
async fn open_interest_sends_the_symbol() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/openInterest",
        "symbol=BTCUSDT",
        &fixture("open_interest"),
    )
    .await;
    let oi = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(oi.symbol, "BTCUSDT");
}

#[tokio::test]
async fn agg_trades_send_their_filters() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/aggTrades",
        "symbol=BTCUSDT&fromId=3477383749&limit=3",
        &fixture("agg_trades"),
    )
    .await;
    let trades = usdm(&server)
        .market()
        .agg_trades(&btc())
        .from_id(3_477_383_749)
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(trades.len(), 3);
}

#[tokio::test]
async fn depth_sends_a_limit_only_when_asked() {
    let mut server = Server::new_async().await;
    let limited = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT&limit=5",
        &fixture("depth"),
    )
    .await;
    let bare = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
    )
    .await;
    let client = usdm(&server);
    let book = client
        .market()
        .depth(&btc())
        .limit(DepthLimit::Five)
        .send()
        .await
        .unwrap();
    assert_eq!(book.bids.len(), 5);
    client.market().depth(&btc()).send().await.unwrap();
    limited.assert_async().await;
    bare.assert_async().await;
}

#[test]
fn each_builder_charges_its_route() {
    let usdm = Usdm::new().unwrap();
    let market = usdm.market();
    assert_eq!(market.klines(&btc(), Interval::M1).cost(), Cost::Weight(5));
    assert_eq!(
        market.klines(&btc(), Interval::M1).limit(1000).cost(),
        Cost::Weight(5)
    );
    assert_eq!(
        market.klines(&btc(), Interval::M1).limit(1001).cost(),
        Cost::Weight(10)
    );
    assert_eq!(market.depth(&btc()).cost(), Cost::Weight(1));
    assert_eq!(
        market.depth(&btc()).limit(DepthLimit::Thousand).cost(),
        Cost::Weight(20)
    );
    assert_eq!(market.agg_trades(&btc()).limit(1).cost(), Cost::Weight(20));
    assert_eq!(market.tickers_24h().cost(), Cost::Weight(40));
    assert_eq!(market.ticker_24h(&btc()).cost(), Cost::Weight(1));
    assert_eq!(market.premium_indices().cost(), Cost::Weight(10));
    assert_eq!(market.funding_rate().cost(), Cost::Funding);
    assert_eq!(usdm.exchange().funding_info().cost(), Cost::Funding);
}

// ── errors ──────────────────────────────────────────────────────

async fn failing(
    server: &mut ServerGuard,
    status: usize,
    headers: &[(&str, &str)],
    body: &str,
) -> Mock {
    let mut mock = server
        .mock("GET", "/fapi/v1/openInterest")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body);
    for (name, value) in headers {
        mock = mock.with_header(*name, value);
    }
    mock.create_async().await
}

#[tokio::test]
async fn a_code_and_msg_body_is_a_venue_error() {
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        400,
        &[],
        r#"{"code":-1121,"msg":"Invalid symbol."}"#,
    )
    .await;
    let err = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            BinanceError::Venue {
                status: 400,
                code: -1121,
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_451_is_region_blocked_and_a_403_forbidden() {
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        451,
        &[],
        "Service unavailable from a restricted location",
    )
    .await;
    let err = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, BinanceError::RegionBlocked { .. }), "{err:?}");

    let mut server = Server::new_async().await;
    let _mock = failing(&mut server, 403, &[], "<html>Request blocked</html>").await;
    let err = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, BinanceError::Forbidden { .. }), "{err:?}");
}

#[tokio::test]
async fn a_418_is_not_retried_and_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let banned = failing(
        &mut server,
        418,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"banned"}"#,
    )
    .await
    .expect(1);
    let client = usdm(&server);
    let err = client
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(err, BinanceError::IpBanned { retry_after: Some(d) } if d == Duration::from_secs(1))
    );
    banned.assert_async().await;

    let start = Instant::now();
    let _ = client.market().open_interest(&btc()).send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
}

#[tokio::test]
async fn a_429_cools_down_then_succeeds() {
    let mut server = Server::new_async().await;
    let limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await
    .expect(1);
    let ok = route(
        &mut server,
        "/fapi/v1/openInterest",
        "symbol=BTCUSDT",
        &fixture("open_interest"),
    )
    .await;

    let start = Instant::now();
    let oi = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap();
    assert_eq!(oi.symbol, "BTCUSDT");
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
    limited.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn a_high_used_weight_header_holds_the_next_request() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/fapi/v1/openInterest")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "2160")
        .with_body(fixture("open_interest"))
        .create_async()
        .await;
    let client = usdm(&server);
    client.market().open_interest(&btc()).send().await.unwrap();
    assert_eq!(
        client.weight_budget().used(),
        2160,
        "another process spent the minute"
    );

    let held = tokio::time::timeout(
        Duration::from_millis(500),
        client.market().open_interest(&btc()).send(),
    )
    .await;
    assert!(
        held.is_err(),
        "the next request must wait for the next minute"
    );
}

#[tokio::test]
async fn a_gzip_body_is_requested_and_decoded() {
    // `{"serverTime":1}` compressed by python3's `gzip.compress(.., mtime=0)`.
    const GZIPPED: &[u8] = &[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xab, 0x56, 0x2a, 0x4e, 0x2d,
        0x2a, 0x4b, 0x2d, 0x0a, 0xc9, 0xcc, 0x4d, 0x55, 0xb2, 0x32, 0xac, 0x05, 0x00, 0xe2, 0x1d,
        0x3e, 0x1a, 0x10, 0x00, 0x00, 0x00,
    ];
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/fapi/v1/time")
        .match_header("accept-encoding", Matcher::Regex("gzip".into()))
        .with_header("content-encoding", "gzip")
        .with_body(GZIPPED)
        .create_async()
        .await;
    let time = usdm(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert_eq!(time.server_time, 1);
}

#[tokio::test]
async fn a_refused_request_s_weight_header_is_recorded() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        400,
        &[("x-mbx-used-weight-1m", "2160")],
        r#"{"code":-1121,"msg":"Invalid symbol."}"#,
    )
    .await;
    let client = usdm(&server);
    client
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert_eq!(client.weight_budget().used(), 2160);
}

#[tokio::test]
async fn a_retry_is_charged_again() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await
    .expect(1);
    let ok = server
        .mock("GET", "/fapi/v1/openInterest")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(fixture("open_interest"))
        .create_async()
        .await;
    let client = usdm(&server);
    client.market().open_interest(&btc()).send().await.unwrap();
    assert_eq!(
        client.weight_budget().used(),
        2,
        "the server charges every attempt"
    );
    limited.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn retry_after_outlasts_a_shorter_backoff() {
    let mut server = Server::new_async().await;
    let limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await
    .expect(1);
    let ok = route(
        &mut server,
        "/fapi/v1/openInterest",
        "symbol=BTCUSDT",
        &fixture("open_interest"),
    )
    .await;
    let client = usdm_with_retries(
        &server,
        RetryConfig {
            max_backoff_ms: 100,
            ..RetryConfig::default()
        },
    );
    let start = Instant::now();
    client.market().open_interest(&btc()).send().await.unwrap();
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
    limited.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let _limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await;
    let client = usdm_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        },
    );
    let err = client
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(err, BinanceError::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(1)),
        "{err:?}"
    );
    let start = Instant::now();
    let _ = client.market().open_interest(&btc()).send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
}

#[tokio::test]
async fn a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _limited = failing(
        &mut server,
        429,
        &[],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await;
    let client = usdm_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        },
    );
    let err = client
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(err, BinanceError::RateLimited { retry_after: None }),
        "{err:?}"
    );
    let held = tokio::time::timeout(
        Duration::from_millis(500),
        client.market().open_interest(&btc()).send(),
    )
    .await;
    assert!(
        held.is_err(),
        "the next request must wait for the weight window to reset"
    );
}

#[tokio::test]
async fn each_route_is_charged_its_own_weight() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = route(
        &mut server,
        "/fapi/v1/ticker/24hr",
        "",
        &fixture("ticker_24hr"),
    )
    .await;
    let client = usdm(&server);
    client.market().tickers_24h().send().await.unwrap();
    assert_eq!(client.weight_budget().used(), 40);
}
