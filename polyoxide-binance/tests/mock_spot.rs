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
