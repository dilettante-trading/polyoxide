//! Mock-server tests for the spot module: its routes' paths and exact
//! queries, the decoding of a captured body, error mapping, and the budget's
//! response to the server. Story 5 appends the market routes after the last
//! test.

use std::time::{Duration, Instant};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::spot::{types::Symbol, weight::Cost, Spot, SpotError, WeightBudget};
use polyoxide_core::RetryConfig;
use polyoxide_test_support::{fixtures, minute};
use polyoxide_venue::{Class, Classify};

fn spot(server: &ServerGuard) -> Spot {
    Spot::builder().base_url(server.url()).build().unwrap()
}

fn spot_with_retries(server: &ServerGuard, config: RetryConfig) -> Spot {
    Spot::builder()
        .base_url(server.url())
        .with_retry_config(config)
        .build()
        .unwrap()
}

/// A captured body, by file name without its `.json`, from
/// `tests/fixtures/spot/rest/`.
fn fixture(name: &str) -> String {
    fixtures!("spot/rest").text(name)
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

/// `GET /api/v3/time` failing with `status`, `headers` and `body`, whatever
/// its query.
async fn failing(
    server: &mut ServerGuard,
    status: usize,
    headers: &[(&str, &str)],
    body: &str,
) -> Mock {
    let mut mock = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body);
    for (name, value) in headers {
        mock = mock.with_header(*name, value);
    }
    mock.create_async().await
}

/// Waits out the end of a minute, so a test that holds the budget for a moment
/// cannot see the window roll over underneath it.
async fn clear_of_a_minute_boundary() {
    minute::wait_unless_within(0..=57_000, 100).await;
}

// ── health and exchange ─────────────────────────────────────────

#[tokio::test]
async fn ping_reports_latency() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/api/v3/ping", "", "{}").await;
    let latency = spot(&server).health().ping().await.unwrap();
    mock.assert_async().await;
    assert!(latency < Duration::from_secs(5));
}

#[tokio::test]
async fn a_ping_whose_body_does_not_decode_is_an_error() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/api/v3/ping", "", "<html>").await;
    let result = spot(&server).health().ping().await;
    mock.assert_async().await;
    let err = result.unwrap_err();
    assert_eq!(err.class(), Class::Decode, "{err:?}");
}

#[tokio::test]
async fn a_ping_is_charged_its_weight() {
    // No weight header, so only the client's own charge can count the ping.
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v3/ping")
        .match_query(Matcher::Missing)
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;
    let client = spot(&server);
    client.health().ping().await.unwrap();
    mock.assert_async().await;
    assert_eq!(client.weight_budget().used(), 1);
}

#[tokio::test]
async fn a_retried_ping_reports_the_answering_attempt() {
    // The ping runs on `HttpClient::health`, so its latency is the round trip
    // of the attempt that answered, as every venue's is, not the whole call
    // with the wait for the budget and the retry.
    let mut server = Server::new_async().await;
    let served = std::sync::atomic::AtomicUsize::new(0);
    let mock = server
        .mock("GET", "/api/v3/ping")
        .match_query(Matcher::Missing)
        .with_status_code_from_request(move |_| {
            match served.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => 429,
                _ => 200,
            }
        })
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body("{}")
        .expect(2)
        .create_async()
        .await;
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        },
    );

    let start = Instant::now();
    let latency = client.health().ping().await.expect("retried to the 200");
    let elapsed = start.elapsed();
    mock.assert_async().await;
    assert!(
        elapsed >= Duration::from_millis(300),
        "the call took {elapsed:?}, inside the retry's 300ms floor"
    );
    assert!(
        elapsed.saturating_sub(latency) >= Duration::from_millis(300),
        "the latency is the answering attempt's, without the backoff: {latency:?} of {elapsed:?}"
    );
}

#[tokio::test]
async fn time_returns_the_server_clock() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/api/v3/time", "", &fixture("time")).await;
    let time = spot(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert!(time.server_time > 1_790_000_000_000);
}

#[tokio::test]
async fn exchange_info_decodes_the_captured_body() {
    let mut server = Server::new_async().await;
    let body = fixture("exchange_info");
    let mock = route(&mut server, "/api/v3/exchangeInfo", "", &body).await;
    let info = spot(&server)
        .exchange()
        .exchange_info()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let raw: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(info.timezone, "UTC");
    assert_eq!(info.symbols.len(), raw["symbols"].as_array().unwrap().len());
    assert!(info.symbols.iter().any(|s| s.symbol == "BTCUSDT"));
    // Binance's published weight, which the budget's 5400 is a tenth under.
    assert!(
        info.rate_limits
            .iter()
            .any(|l| l.rate_limit_type == "REQUEST_WEIGHT"
                && l.interval == "MINUTE"
                && l.interval_num == 1
                && l.limit == polyoxide_binance::spot::weight::PUBLISHED_WEIGHT_PER_MINUTE),
        "{:?}",
        info.rate_limits
    );
}

#[tokio::test]
async fn exchange_info_sends_a_symbol_only_when_asked() {
    let mut server = Server::new_async().await;
    let body = fixture("exchange_info");
    let one = route(&mut server, "/api/v3/exchangeInfo", "symbol=BTCUSDT", &body).await;
    let all = route(&mut server, "/api/v3/exchangeInfo", "", &body).await;
    let client = spot(&server);
    client
        .exchange()
        .exchange_info()
        .symbol(&btc())
        .send()
        .await
        .unwrap();
    client.exchange().exchange_info().send().await.unwrap();
    one.assert_async().await;
    all.assert_async().await;
}

#[test]
fn health_and_exchange_charge_their_routes() {
    let spot = Spot::new().unwrap();
    assert_eq!(spot.health().time().cost(), Cost::Weight(1));
    assert_eq!(spot.exchange().exchange_info().cost(), Cost::Weight(20));
    assert_eq!(
        spot.exchange().exchange_info().symbol(&btc()).cost(),
        Cost::Weight(20)
    );
}

#[tokio::test]
async fn an_exchange_info_request_is_charged_twenty() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "",
        &fixture("exchange_info"),
    )
    .await;
    let client = spot(&server);
    client.exchange().exchange_info().send().await.unwrap();
    assert_eq!(client.weight_budget().used(), 20);
}

#[tokio::test]
async fn an_unknown_symbol_is_a_venue_error_with_its_code() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v3/exchangeInfo")
        .match_query(Matcher::Exact("symbol=NOPEUSDT".into()))
        .with_status(400)
        .with_body(fixture("error_invalid_symbol"))
        .create_async()
        .await;
    let err = spot(&server)
        .exchange()
        .exchange_info()
        .symbol(&Symbol::new("NOPEUSDT").unwrap())
        .send()
        .await
        .unwrap_err();
    mock.assert_async().await;
    assert!(
        matches!(&err, SpotError::Venue { status: 400, code: -1121, msg } if msg == "Invalid symbol."),
        "{err:?}"
    );
}

// ── errors ──────────────────────────────────────────────────────

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
    let err = spot(&server).health().time().send().await.unwrap_err();
    assert!(
        matches!(
            err,
            SpotError::Venue {
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
    let err = spot(&server).health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::RegionBlocked { .. }), "{err:?}");

    let mut server = Server::new_async().await;
    let _mock = failing(&mut server, 403, &[], "<html>Request blocked</html>").await;
    let err = spot(&server).health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::Forbidden { .. }), "{err:?}");
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
    let client = spot(&server);
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::IpBanned { retry_after: Some(d) } if d == Duration::from_secs(1)),
        "{err:?}"
    );
    banned.assert_async().await;

    let start = Instant::now();
    let _ = client.health().time().send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
}

#[tokio::test]
async fn a_418_with_no_retry_after_is_still_a_failure_and_holds() {
    let mut server = Server::new_async().await;
    let banned = failing(&mut server, 418, &[], r#"{"code":-1003,"msg":"banned"}"#)
        .await
        .expect(1);
    let client = spot(&server);
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::IpBanned { retry_after: None }),
        "{err:?}"
    );
    banned.assert_async().await;
    // The two-minute default ban holds the next request.
    let held =
        tokio::time::timeout(Duration::from_millis(500), client.health().time().send()).await;
    assert!(held.is_err(), "the next request must wait out the ban");
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
    let ok = route(&mut server, "/api/v3/time", "", &fixture("time")).await;

    let start = Instant::now();
    let time = spot(&server).health().time().send().await.unwrap();
    assert!(time.server_time > 1_790_000_000_000);
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
    limited.assert_async().await;
    ok.assert_async().await;
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
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(fixture("time"))
        .create_async()
        .await;
    let client = spot(&server);
    client.health().time().send().await.unwrap();
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
    let ok = route(&mut server, "/api/v3/time", "", &fixture("time")).await;
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_backoff_ms: 100,
            ..RetryConfig::default()
        },
    );
    let start = Instant::now();
    client.health().time().send().await.unwrap();
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
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        },
    );
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(1)),
        "{err:?}"
    );
    let start = Instant::now();
    let _ = client.health().time().send().await;
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
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        },
    );
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::RateLimited { retry_after: None }),
        "{err:?}"
    );
    let held =
        tokio::time::timeout(Duration::from_millis(500), client.health().time().send()).await;
    assert!(
        held.is_err(),
        "the next request must wait for the weight window to reset"
    );
}

#[tokio::test]
async fn every_spot_on_one_budget_is_held_by_one_ban() {
    // Binance bans an IP, not a client: a 418 on one Spot holds every Spot
    // built with the same budget, which is the throttle they all send through.
    let mut server = Server::new_async().await;
    let banned = failing(
        &mut server,
        418,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"banned"}"#,
    )
    .await
    .expect(1);
    let budget = WeightBudget::new();
    let client = |budget: &WeightBudget| {
        Spot::builder()
            .base_url(server.url())
            .weight_budget(budget.clone())
            .build()
            .unwrap()
    };
    let (first, second) = (client(&budget), client(&budget));

    let err = first.health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::IpBanned { .. }), "{err:?}");
    banned.assert_async().await;

    let start = Instant::now();
    let _ = second.health().time().send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "the other client went after {held:?}, inside the ban"
    );
}

#[tokio::test]
async fn a_425_and_a_5xx_are_not_retried() {
    // The policy retries only a 429: a 425 or a 5xx reaches the caller after
    // one request, with retries left.
    for status in [425, 500, 503] {
        let mut server = Server::new_async().await;
        let mock = failing(
            &mut server,
            status,
            &[],
            r#"{"code":-1001,"msg":"Internal error; unable to process your request."}"#,
        )
        .await
        .expect(1);
        let err = spot(&server).health().time().send().await.unwrap_err();
        assert!(
            matches!(err, SpotError::Venue { status: s, .. } if usize::from(s) == status),
            "{status}: {err:?}"
        );
        mock.assert_async().await;
    }
}

#[tokio::test]
async fn a_high_used_weight_header_holds_the_next_request() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "5400")
        .with_body(fixture("time"))
        .create_async()
        .await;
    let client = spot(&server);
    client.health().time().send().await.unwrap();
    assert_eq!(
        client.weight_budget().used(),
        5400,
        "another process spent the minute"
    );

    let held =
        tokio::time::timeout(Duration::from_millis(500), client.health().time().send()).await;
    assert!(
        held.is_err(),
        "the next request must wait for the next minute"
    );
}

#[tokio::test]
async fn a_refused_request_s_weight_header_is_recorded() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        400,
        &[("x-mbx-used-weight-1m", "5400")],
        r#"{"code":-1121,"msg":"Invalid symbol."}"#,
    )
    .await;
    let client = spot(&server);
    client.health().time().send().await.unwrap_err();
    assert_eq!(client.weight_budget().used(), 5400);
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
        .mock("GET", "/api/v3/time")
        .match_header("accept-encoding", Matcher::Regex("gzip".into()))
        .with_header("content-encoding", "gzip")
        .with_body(GZIPPED)
        .create_async()
        .await;
    let time = spot(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert_eq!(time.server_time, 1);
}

#[tokio::test]
async fn the_namespaces_share_one_hold() {
    // Both namespaces are cut from one client, so a 418 through one holds a
    // request made through the other.
    let mut server = Server::new_async().await;
    let _banned = failing(
        &mut server,
        418,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"banned"}"#,
    )
    .await;
    let _info = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "",
        &fixture("exchange_info"),
    )
    .await;
    let client = spot(&server);

    let err = client.health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::IpBanned { .. }), "{err:?}");

    let start = Instant::now();
    let _ = client.exchange().exchange_info().send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "the exchange namespace went after {held:?}, inside the ban"
    );
}

// ── market ──────────────────────────────────────────────────────

#[tokio::test]
async fn depth_sends_a_limit_only_when_asked_and_decodes_the_capture() {
    let mut server = mockito::Server::new_async().await;
    let raw = fixture("depth");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let limited = route(&mut server, "/api/v3/depth", "symbol=BTCUSDT&limit=5", &raw).await;
    let bare = route(&mut server, "/api/v3/depth", "symbol=BTCUSDT", &raw).await;

    let client = spot(&server);
    let book = client.market().depth(&btc()).limit(5).send().await.unwrap();
    assert_eq!(book.last_update_id, wire["lastUpdateId"].as_u64().unwrap());
    assert_eq!(book.bids.len(), wire["bids"].as_array().unwrap().len());
    assert_eq!(book.asks.len(), wire["asks"].as_array().unwrap().len());
    assert!(!book.bids.is_empty() && !book.asks.is_empty());
    assert_eq!(
        book.bids[0].price.to_string(),
        wire["bids"][0][0].as_str().unwrap()
    );
    assert_eq!(
        book.asks[0].quantity.to_string(),
        wire["asks"][0][1].as_str().unwrap()
    );

    client.market().depth(&btc()).send().await.unwrap();
    limited.assert_async().await;
    bare.assert_async().await;
}

#[tokio::test]
async fn trades_send_their_limit_and_decode_the_capture() {
    let mut server = mockito::Server::new_async().await;
    let raw = fixture("trades");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mock = route(
        &mut server,
        "/api/v3/trades",
        "symbol=BTCUSDT&limit=3",
        &raw,
    )
    .await;
    let trades = spot(&server)
        .market()
        .trades(&btc())
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(trades.len(), wire.as_array().unwrap().len());
    assert_eq!(trades[0].id, wire[0]["id"].as_u64().unwrap());
    assert_eq!(trades[0].qty.to_string(), wire[0]["qty"].as_str().unwrap());
}

#[tokio::test]
async fn agg_trades_send_their_filters() {
    let mut server = mockito::Server::new_async().await;
    let raw = fixture("agg_trades");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let first = wire[0]["a"].as_u64().unwrap();
    let by_id = route(
        &mut server,
        "/api/v3/aggTrades",
        &format!("symbol=BTCUSDT&fromId={first}&limit=3"),
        &raw,
    )
    .await;
    let by_time = route(
        &mut server,
        "/api/v3/aggTrades",
        "symbol=BTCUSDT&startTime=1791350000000&endTime=1791353600000&limit=3",
        &raw,
    )
    .await;

    let client = spot(&server);
    let trades = client
        .market()
        .agg_trades(&btc())
        .from_id(first)
        .limit(3)
        .send()
        .await
        .unwrap();
    assert_eq!(trades.len(), wire.as_array().unwrap().len());
    assert_eq!(trades[0].id, first);
    assert_eq!(trades[0].price.to_string(), wire[0]["p"].as_str().unwrap());
    client
        .market()
        .agg_trades(&btc())
        .start_time(1_791_350_000_000)
        .end_time(1_791_353_600_000)
        .limit(3)
        .send()
        .await
        .unwrap();
    by_id.assert_async().await;
    by_time.assert_async().await;
}

#[tokio::test]
async fn klines_send_every_parameter_once_and_the_interval_by_its_wire_name() {
    use polyoxide_binance::spot::types::Interval;

    let mut server = mockito::Server::new_async().await;
    let minutes = fixture("klines");
    let seconds = fixture("klines_1s");
    let hourly = route(
        &mut server,
        "/api/v3/klines",
        "symbol=BTCUSDT&interval=1h&startTime=1791350000000&endTime=1791354000000&limit=2",
        &minutes,
    )
    .await;
    let second = route(
        &mut server,
        "/api/v3/klines",
        "symbol=BTCUSDT&interval=1s&limit=2",
        &seconds,
    )
    .await;

    let client = spot(&server);
    let candles = client
        .market()
        .klines(&btc(), Interval::Hour1)
        .start_time(1_791_350_000_000)
        .end_time(1_791_354_000_000)
        .limit(100)
        .limit(2)
        .send()
        .await
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&minutes).unwrap();
    assert_eq!(candles.len(), wire.as_array().unwrap().len());
    assert_eq!(candles[0].open_time, wire[0][0].as_u64().unwrap());
    assert_eq!(candles[0].close.to_string(), wire[0][4].as_str().unwrap());

    let candles = client
        .market()
        .klines(&btc(), Interval::Second1)
        .limit(2)
        .send()
        .await
        .unwrap();
    assert_eq!(candles[1].open_time - candles[0].open_time, 1_000);
    hourly.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn avg_price_sends_the_symbol_and_decodes_the_capture() {
    let mut server = mockito::Server::new_async().await;
    let raw = fixture("avg_price");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mock = route(&mut server, "/api/v3/avgPrice", "symbol=BTCUSDT", &raw).await;
    let average = spot(&server)
        .market()
        .avg_price(&btc())
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(u64::from(average.mins), wire["mins"].as_u64().unwrap());
    assert_eq!(average.price.to_string(), wire["price"].as_str().unwrap());
    assert_eq!(average.close_time, wire["closeTime"].as_u64().unwrap());
}

#[tokio::test]
async fn ticker_24h_sends_the_symbol_and_tickers_24h_sends_nothing() {
    let mut server = mockito::Server::new_async().await;
    let one_raw = fixture("ticker_24hr");
    let all_raw = fixture("tickers_24hr");
    let one_wire: serde_json::Value = serde_json::from_str(&one_raw).unwrap();
    let all_wire: serde_json::Value = serde_json::from_str(&all_raw).unwrap();
    let single = route(
        &mut server,
        "/api/v3/ticker/24hr",
        "symbol=BTCUSDT",
        &one_raw,
    )
    .await;
    let every = route(&mut server, "/api/v3/ticker/24hr", "", &all_raw).await;

    let client = spot(&server);
    let ticker = client.market().ticker_24h(&btc()).send().await.unwrap();
    assert_eq!(ticker.symbol, "BTCUSDT");
    assert_eq!(
        ticker.last_price.to_string(),
        one_wire["lastPrice"].as_str().unwrap()
    );
    let tickers = client.market().tickers_24h().send().await.unwrap();
    assert_eq!(tickers.len(), all_wire.as_array().unwrap().len());
    single.assert_async().await;
    every.assert_async().await;
}

#[tokio::test]
async fn price_and_prices_send_the_symbol_or_nothing() {
    let mut server = mockito::Server::new_async().await;
    let one_raw = fixture("price");
    let all_raw = fixture("prices");
    let one_wire: serde_json::Value = serde_json::from_str(&one_raw).unwrap();
    let all_wire: serde_json::Value = serde_json::from_str(&all_raw).unwrap();
    let single = route(
        &mut server,
        "/api/v3/ticker/price",
        "symbol=BTCUSDT",
        &one_raw,
    )
    .await;
    let every = route(&mut server, "/api/v3/ticker/price", "", &all_raw).await;

    let client = spot(&server);
    let price = client.market().price(&btc()).send().await.unwrap();
    assert_eq!(price.symbol, "BTCUSDT");
    assert_eq!(price.price.to_string(), one_wire["price"].as_str().unwrap());
    let prices = client.market().prices().send().await.unwrap();
    assert_eq!(prices.len(), all_wire.as_array().unwrap().len());
    single.assert_async().await;
    every.assert_async().await;
}

#[tokio::test]
async fn book_ticker_and_book_tickers_send_the_symbol_or_nothing() {
    let mut server = mockito::Server::new_async().await;
    let one_raw = fixture("book_ticker");
    let all_raw = fixture("book_tickers");
    let one_wire: serde_json::Value = serde_json::from_str(&one_raw).unwrap();
    let all_wire: serde_json::Value = serde_json::from_str(&all_raw).unwrap();
    let single = route(
        &mut server,
        "/api/v3/ticker/bookTicker",
        "symbol=BTCUSDT",
        &one_raw,
    )
    .await;
    let every = route(&mut server, "/api/v3/ticker/bookTicker", "", &all_raw).await;

    let client = spot(&server);
    let book = client.market().book_ticker(&btc()).send().await.unwrap();
    assert_eq!(book.symbol, "BTCUSDT");
    assert_eq!(
        book.ask_qty.to_string(),
        one_wire["askQty"].as_str().unwrap()
    );
    let books = client.market().book_tickers().send().await.unwrap();
    assert_eq!(books.len(), all_wire.as_array().unwrap().len());
    single.assert_async().await;
    every.assert_async().await;
}

/// Spec §3.1's table, builder by builder: what a request will be charged
/// before it is sent.
#[test]
fn every_market_builder_charges_its_route() {
    use polyoxide_binance::spot::{types::Interval, weight::Cost, Spot};

    let spot = Spot::new().unwrap();
    let market = spot.market();
    let btc = btc();
    let w = Cost::Weight;

    // depth: 5 without a limit and up to 100, then 25, 50 and 250 by band.
    assert_eq!(market.depth(&btc).cost(), w(5));
    for (limit, weight) in [
        (1, 5),
        (100, 5),
        (101, 25),
        (500, 25),
        (501, 50),
        (1000, 50),
        (1001, 250),
        (5000, 250),
        (5001, 250),
    ] {
        assert_eq!(
            market.depth(&btc).limit(limit).cost(),
            w(weight),
            "limit {limit}"
        );
    }
    // The limit set again re-prices again.
    assert_eq!(market.depth(&btc).limit(1001).limit(100).cost(), w(5));

    // These do not change with a limit.
    assert_eq!(market.trades(&btc).cost(), w(25));
    assert_eq!(market.trades(&btc).limit(1000).cost(), w(25));
    assert_eq!(market.agg_trades(&btc).cost(), w(4));
    assert_eq!(market.agg_trades(&btc).limit(1).cost(), w(4));
    assert_eq!(market.klines(&btc, Interval::Minute1).cost(), w(2));
    assert_eq!(
        market.klines(&btc, Interval::Minute1).limit(1000).cost(),
        w(2)
    );
    assert_eq!(market.avg_price(&btc).cost(), w(2));

    // The tickers: 2 for one symbol; 80, 4 and 4 for all of them.
    assert_eq!(market.ticker_24h(&btc).cost(), w(2));
    assert_eq!(market.tickers_24h().cost(), w(80));
    assert_eq!(market.price(&btc).cost(), w(2));
    assert_eq!(market.prices().cost(), w(4));
    assert_eq!(market.book_ticker(&btc).cost(), w(2));
    assert_eq!(market.book_tickers().cost(), w(4));
}

/// The cost a builder reports is the cost the budget is charged: the re-priced
/// route reaches the send loop, not only `cost()`.
#[tokio::test]
async fn a_deep_depth_is_charged_its_band() {
    clear_of_a_minute_boundary().await;
    let mut server = mockito::Server::new_async().await;
    let _deep = route(
        &mut server,
        "/api/v3/depth",
        "symbol=BTCUSDT&limit=1001",
        &fixture("depth"),
    )
    .await;
    let client = spot(&server);
    client
        .market()
        .depth(&btc())
        .limit(1001)
        .send()
        .await
        .unwrap();
    assert_eq!(client.weight_budget().used(), 250);
}

#[tokio::test]
async fn all_tickers_are_charged_their_weight() {
    clear_of_a_minute_boundary().await;
    let mut server = mockito::Server::new_async().await;
    let _every = route(
        &mut server,
        "/api/v3/ticker/24hr",
        "",
        &fixture("tickers_24hr"),
    )
    .await;
    let client = spot(&server);
    client.market().tickers_24h().send().await.unwrap();
    assert_eq!(client.weight_budget().used(), 80);
}

#[tokio::test]
async fn an_unknown_symbol_on_a_market_route_is_a_venue_error() {
    use polyoxide_binance::spot::{types::Symbol, SpotError};

    let mut server = mockito::Server::new_async().await;
    let raw = fixture("error_invalid_symbol");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mock = server
        .mock("GET", "/api/v3/depth")
        .match_query(mockito::Matcher::Exact("symbol=NOPEUSDT".to_owned()))
        .with_status(400)
        .with_body(&raw)
        .create_async()
        .await;
    let error = spot(&server)
        .market()
        .depth(&Symbol::new("NOPEUSDT").unwrap())
        .send()
        .await
        .unwrap_err();
    mock.assert_async().await;
    match error {
        SpotError::Venue { status, code, msg } => {
            assert_eq!(status, 400);
            assert_eq!(code, wire["code"].as_i64().unwrap());
            assert_eq!(msg, wire["msg"].as_str().unwrap());
        }
        other => panic!("expected a venue error, got {other:?}"),
    }
}
