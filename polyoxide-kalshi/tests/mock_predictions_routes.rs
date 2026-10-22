//! Mock-server tests for the public routes: each route's path and exact query, the decoding of
//! its captured body, the captured error bodies, a 429 retried and then holding every request,
//! and the route table's accessors.

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    request::{route_templates, template_matches},
    types::{
        MarketCandlestick, MarketStatus, MarketStatusFilter, MveFilter, OutcomeSide,
        PeriodInterval, PriceDistribution,
    },
    Predictions, PredictionsError,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use rust_decimal::Decimal;
use serde_json::Value;

/// A schedule that retries nothing and holds the client for about a millisecond after a 429.
const NO_RETRIES: RetryConfig = RetryConfig {
    max_retries: 0,
    initial_backoff_ms: 1,
    max_backoff_ms: 10,
};

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn wire(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    client_with_retries(server, RetryConfig::default())
}

fn client_with_retries(server: &ServerGuard, config: RetryConfig) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(config)
        .build()
        .unwrap()
}

fn ticker(text: &str) -> Ticker {
    Ticker::new(text).unwrap()
}

/// A route answering `body`, matched on its whole query string so a missing or extra parameter
/// fails the request.
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
        .with_body(body)
        .create_async()
        .await
}

// ── markets ─────────────────────────────────────────────────────

#[tokio::test]
async fn the_market_listing_sends_its_filters_and_decodes_the_captured_page() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets",
        "status=open&event_ticker=EV-1&series_ticker=SER-1&mve_filter=exclude&limit=5&cursor=c1",
        &fixture("markets_page"),
    )
    .await;
    let page = client(&server)
        .markets()
        .list()
        .status(MarketStatusFilter::Open)
        .event_ticker("EV-1")
        .series_ticker("SER-1")
        .mve_filter(MveFilter::Exclude)
        .limit(5)
        .cursor("c1")
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire("markets_page");
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    let rows = sent["markets"].as_array().unwrap();
    assert_eq!(page.markets.len(), rows.len());
    for (market, row) in page.markets.iter().zip(rows) {
        assert_eq!(market.ticker, row["ticker"].as_str().unwrap());
        assert_eq!(
            market.status,
            MarketStatus::from_wire(row["status"].as_str().unwrap())
        );
        assert_eq!(
            market.yes_bid_dollars.to_string(),
            row["yes_bid_dollars"].as_str().unwrap()
        );
        assert_eq!(
            market.price_ranges.len(),
            row["price_ranges"].as_array().unwrap().len()
        );
    }
}

#[tokio::test]
async fn a_market_is_read_by_ticker_and_decodes_the_captured_body() {
    let sent = wire("market");
    let text = sent["market"]["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/markets/{text}"),
        "",
        &fixture("market"),
    )
    .await;
    let read = client(&server)
        .markets()
        .get(&ticker(text))
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.market.ticker, text);
    assert_eq!(
        read.market.notional_value_dollars.to_string(),
        sent["market"]["notional_value_dollars"].as_str().unwrap()
    );
    assert_eq!(
        read.market.yes_ask_dollars.to_string(),
        sent["market"]["yes_ask_dollars"].as_str().unwrap()
    );
}

#[tokio::test]
async fn a_dotted_ticker_is_sent_as_written_and_in_capitals() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/KXBTCD-26OCT1806-T82099.99",
        "",
        &fixture("market"),
    )
    .await;
    client(&server)
        .markets()
        .get(&ticker("kxbtcd-26oct1806-t82099.99"))
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn an_order_book_sends_its_depth_and_keeps_every_digit_of_each_level() {
    let sent = wire("orderbook");
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/KXTEST-1/orderbook",
        "",
        &fixture("orderbook"),
    )
    .await;
    let book = client(&server)
        .markets()
        .orderbook(&ticker("KXTEST-1"))
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    mock.assert_async().await;
    for (side, levels) in [
        ("yes_dollars", &book.yes_dollars),
        ("no_dollars", &book.no_dollars),
    ] {
        let rows = sent["orderbook_fp"][side].as_array().unwrap();
        assert_eq!(levels.len(), rows.len(), "{side}");
        for (level, row) in levels.iter().zip(rows) {
            assert_eq!(level.price.to_string(), row[0].as_str().unwrap(), "{side}");
            assert_eq!(level.count.to_string(), row[1].as_str().unwrap(), "{side}");
        }
    }

    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/KXTEST-1/orderbook",
        "depth=1",
        &fixture("orderbook_depth1"),
    )
    .await;
    let shallow = client(&server)
        .markets()
        .orderbook(&ticker("KXTEST-1"))
        .depth(1)
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    mock.assert_async().await;
    assert!(shallow.yes_dollars.len() <= 1 && shallow.no_dollars.len() <= 1);
}

#[tokio::test]
async fn an_unlisted_ticker_has_an_empty_book_whatever_form_the_empty_sides_take() {
    for body in [
        r#"{"orderbook_fp":{"no_dollars":[],"yes_dollars":[]}}"#,
        r#"{"orderbook_fp":{"no_dollars":null,"yes_dollars":[]}}"#,
        r#"{"orderbook_fp":{}}"#,
    ] {
        let mut server = Server::new_async().await;
        let mock = route(
            &mut server,
            "/trade-api/v2/markets/NOSUCHMARKET-0/orderbook",
            "",
            body,
        )
        .await;
        let book = client(&server)
            .markets()
            .orderbook(&ticker("NOSUCHMARKET-0"))
            .send()
            .await
            .unwrap()
            .orderbook_fp;
        mock.assert_async().await;
        assert!(
            book.yes_dollars.is_empty() && book.no_dollars.is_empty(),
            "{body}"
        );
    }
}

#[tokio::test]
async fn trades_send_their_query_and_decode_the_captured_page() {
    let sent = wire("trades");
    let text = sent["trades"][0]["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/trades",
        &format!("ticker={text}&limit=3&cursor=c1&min_ts=6&max_ts=7"),
        &fixture("trades"),
    )
    .await;
    let page = client(&server)
        .markets()
        .trades()
        .ticker(&ticker(text))
        .limit(3)
        .cursor("c1")
        .min_ts(6)
        .max_ts(7)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let rows = sent["trades"].as_array().unwrap();
    assert_eq!(page.trades.len(), rows.len());
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    for (trade, row) in page.trades.iter().zip(rows) {
        assert_eq!(trade.trade_id, row["trade_id"].as_str().unwrap());
        assert_eq!(
            trade.count_fp.to_string(),
            row["count_fp"].as_str().unwrap()
        );
        assert_eq!(
            trade.yes_price_dollars.to_string(),
            row["yes_price_dollars"].as_str().unwrap()
        );
        assert_eq!(
            trade.taker_outcome_side,
            OutcomeSide::from_wire(row["taker_outcome_side"].as_str().unwrap())
        );
    }
}

// ── events and account ──────────────────────────────────────────

#[tokio::test]
async fn an_event_sends_nested_markets_and_decodes_the_captured_body() {
    let sent = wire("event");
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/events/EV-1",
        "with_nested_markets=true",
        &fixture("event"),
    )
    .await;
    let read = client(&server)
        .events()
        .get("EV-1")
        .with_nested_markets(true)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(
        read.event.event_ticker,
        sent["event"]["event_ticker"].as_str().unwrap()
    );
    let nested = sent["event"]["markets"].as_array().unwrap();
    assert!(!nested.is_empty());
    assert_eq!(read.event.markets.len(), nested.len());
    assert_eq!(
        read.event.markets[0].ticker,
        nested[0]["ticker"].as_str().unwrap()
    );
}

#[tokio::test]
async fn the_endpoint_costs_decode_the_captured_body() {
    let sent = wire("endpoint_costs");
    let mut server = Server::new_async().await;
    // At least once: a client may read this route itself before its first send.
    let mock = server
        .mock("GET", "/trade-api/v2/account/endpoint_costs")
        .match_query(Matcher::Missing)
        .with_status(200)
        .with_body(fixture("endpoint_costs"))
        .expect_at_least(1)
        .create_async()
        .await;
    let costs = client(&server)
        .account()
        .endpoint_costs()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(
        costs.default_cost,
        sent["default_cost"].as_u64().unwrap() as u32
    );
    let rows = sent["endpoint_costs"].as_array().unwrap();
    assert_eq!(costs.endpoint_costs.len(), rows.len());
    for (cost, row) in costs.endpoint_costs.iter().zip(rows) {
        assert_eq!(cost.method, row["method"].as_str().unwrap());
        assert_eq!(cost.path, row["path"].as_str().unwrap());
        assert_eq!(u64::from(cost.cost), row["cost"].as_u64().unwrap());
    }
}

// ── errors ──────────────────────────────────────────────────────

#[tokio::test]
async fn each_captured_error_body_decodes_into_its_class() {
    type IsClass = fn(&Class) -> bool;
    let cases: [(&str, usize, IsClass); 4] = [
        ("error_not_found", 404, |c| {
            matches!(c, Class::VenueRefusal { .. })
        }),
        ("error_validation", 400, |c| {
            matches!(c, Class::VenueRefusal { .. })
        }),
        ("error_unauthenticated", 401, |c| {
            matches!(c, Class::Unauthorized)
        }),
        ("error_rate_limited", 429, |c| {
            matches!(c, Class::RateLimited { .. })
        }),
    ];
    for (name, status, is_class) in cases {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/trade-api/v2/markets/trades")
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(fixture(name))
            .create_async()
            .await;
        let err = client_with_retries(&server, NO_RETRIES)
            .markets()
            .trades()
            .send()
            .await
            .unwrap_err();
        let PredictionsError::Venue(venue) = &err else {
            panic!("{name}: not a venue error: {err:?}");
        };

        // The body's own values: `{"msg": …}`, `{"error": "…"}` or `{"error": {"code": …,
        // "message": …}}`.
        let body = wire(name);
        let (code, message) = if let Some(text) = body["msg"].as_str() {
            (None, text)
        } else if let Some(text) = body["error"].as_str() {
            (None, text)
        } else {
            (
                body["error"]["code"].as_str(),
                body["error"]["message"].as_str().unwrap(),
            )
        };
        assert_eq!(usize::from(venue.status), status, "{name}");
        assert_eq!(venue.code.as_deref(), code, "{name}");
        assert_eq!(venue.message, message, "{name}");
        assert!(is_class(&err.class()), "{name}: {:?}", err.class());
    }
}

#[tokio::test]
async fn a_429_is_retried_and_the_retry_waits_out_the_backoff() {
    let mut server = Server::new_async().await;
    let served = AtomicUsize::new(0);
    let mock = server
        .mock("GET", "/trade-api/v2/markets/trades")
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |_| match served.fetch_add(1, Ordering::SeqCst) {
            0 => 429,
            _ => 200,
        })
        .with_body(fixture("trades"))
        .expect(2)
        .create_async()
        .await;
    let client = client_with_retries(
        &server,
        RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        },
    );

    let start = Instant::now();
    let page = client.markets().trades().send().await.unwrap();
    let elapsed = start.elapsed();
    mock.assert_async().await;
    assert!(!page.trades.is_empty());
    // The first backoff is 400 ms less up to a quarter for jitter.
    assert!(
        elapsed >= Duration::from_millis(300) && elapsed < Duration::from_secs(5),
        "{elapsed:?}"
    );
}

#[tokio::test]
async fn a_429_with_no_retry_left_still_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let limited = server
        .mock("GET", "/trade-api/v2/markets/trades")
        .match_query(Matcher::Any)
        .with_status(429)
        .with_body(fixture("error_rate_limited"))
        .expect(1)
        .create_async()
        .await;
    let event = route(
        &mut server,
        "/trade-api/v2/events/EV-1",
        "",
        &fixture("event"),
    )
    .await;
    let client = client_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        },
    );

    let err = client.markets().trades().send().await.unwrap_err();
    limited.assert_async().await;
    assert!(matches!(err.class(), Class::RateLimited { .. }), "{err:?}");

    // A different route of the same client waits out the hold the 429 set: 400 ms less up to a
    // quarter for jitter, and the 429 was a moment ago.
    let start = Instant::now();
    client.events().get("EV-1").send().await.unwrap();
    let held = start.elapsed();
    event.assert_async().await;
    assert!(
        held >= Duration::from_millis(250) && held < Duration::from_secs(5),
        "{held:?}"
    );
}

// ── the route table ─────────────────────────────────────────────

#[test]
fn the_route_table_holds_the_nineteen_routes_in_endpoint_costs_spelling() {
    let mut routes: Vec<(String, &str)> = route_templates()
        .map(|(method, template)| (method.to_ascii_uppercase(), template))
        .collect();
    routes.sort();
    let expected = [
        ("GET", "/trade-api/v2/account/endpoint_costs"),
        ("GET", "/trade-api/v2/account/limits"),
        ("GET", "/trade-api/v2/events"),
        ("GET", "/trade-api/v2/events/:event_ticker"),
        ("GET", "/trade-api/v2/exchange/status"),
        ("GET", "/trade-api/v2/historical/cutoff"),
        ("GET", "/trade-api/v2/historical/markets"),
        ("GET", "/trade-api/v2/historical/markets/:ticker"),
        (
            "GET",
            "/trade-api/v2/historical/markets/:ticker/candlesticks",
        ),
        ("GET", "/trade-api/v2/historical/trades"),
        ("GET", "/trade-api/v2/markets"),
        ("GET", "/trade-api/v2/markets/:ticker"),
        ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
        ("GET", "/trade-api/v2/markets/candlesticks"),
        ("GET", "/trade-api/v2/markets/orderbooks"),
        ("GET", "/trade-api/v2/markets/trades"),
        ("GET", "/trade-api/v2/series"),
        ("GET", "/trade-api/v2/series/:series_ticker"),
        (
            "GET",
            "/trade-api/v2/series/:series_ticker/markets/:ticker/candlesticks",
        ),
    ];
    let expected: Vec<(String, &str)> = expected
        .into_iter()
        .map(|(method, template)| (method.to_owned(), template))
        .collect();
    assert_eq!(routes, expected);
}

#[test]
fn templates_match_segment_by_segment_and_a_star_takes_the_rest() {
    let ours = "/trade-api/v2/markets/:ticker/orderbook";
    // A parameter Kalshi spells differently still matches.
    assert!(template_matches(
        ours,
        "/trade-api/v2/markets/:market_ticker/orderbook"
    ));
    assert!(template_matches(ours, ours));
    // A literal segment must be equal, and the segment counts must agree.
    assert!(!template_matches(
        ours,
        "/trade-api/v2/markets/:ticker/trades"
    ));
    assert!(!template_matches(ours, "/trade-api/v2/markets/:ticker"));
    assert!(!template_matches(
        ours,
        "/trade-api/v2/markets/:ticker/orderbook/depth"
    ));
    // A parameter does not match a literal.
    assert!(!template_matches(
        "/trade-api/v2/markets/trades",
        "/trade-api/v2/markets/:ticker"
    ));
    // A listed `*segment` matches everything left, and only there.
    assert!(template_matches(
        "/trade-api/v2/cfbenchmarks/a/b",
        "/trade-api/v2/cfbenchmarks/*endpoint"
    ));
    assert!(!template_matches(
        "/trade-api/v2/other/a/b",
        "/trade-api/v2/cfbenchmarks/*endpoint"
    ));
}

// ── candlesticks ────────────────────────────────────────────────

/// A candlestick's `price` field by its wire key.
fn price_of(price: &PriceDistribution, key: &str) -> Option<Decimal> {
    match key {
        "open_dollars" => price.open_dollars,
        "low_dollars" => price.low_dollars,
        "high_dollars" => price.high_dollars,
        "close_dollars" => price.close_dollars,
        "mean_dollars" => price.mean_dollars,
        "previous_dollars" => price.previous_dollars,
        "min_dollars" => price.min_dollars,
        "max_dollars" => price.max_dollars,
        other => panic!("{other} is not a price field"),
    }
}

/// Every decoded candlestick holds the digits and the presence of the captured row it came from.
fn rows_agree(candles: &[MarketCandlestick], rows: &[Value], what: &str) {
    assert_eq!(candles.len(), rows.len(), "{what}");
    assert!(!rows.is_empty(), "{what}: the capture holds no candlestick");
    for (candle, row) in candles.iter().zip(rows) {
        assert_eq!(
            candle.end_period_ts,
            row["end_period_ts"].as_i64().unwrap(),
            "{what}"
        );
        assert_eq!(
            candle.volume_fp.to_string(),
            row["volume_fp"].as_str().unwrap(),
            "{what}"
        );
        assert_eq!(
            candle.open_interest_fp.to_string(),
            row["open_interest_fp"].as_str().unwrap(),
            "{what}"
        );
        for (side, offers) in [("yes_bid", &candle.yes_bid), ("yes_ask", &candle.yes_ask)] {
            for (key, value) in [
                ("open_dollars", offers.open_dollars),
                ("low_dollars", offers.low_dollars),
                ("high_dollars", offers.high_dollars),
                ("close_dollars", offers.close_dollars),
            ] {
                assert_eq!(
                    value.to_string(),
                    row[side][key].as_str().unwrap(),
                    "{what} {side}.{key}"
                );
            }
        }
        for key in [
            "open_dollars",
            "low_dollars",
            "high_dollars",
            "close_dollars",
            "mean_dollars",
            "previous_dollars",
            "min_dollars",
            "max_dollars",
        ] {
            assert_eq!(
                price_of(&candle.price, key).map(|price| price.to_string()),
                row["price"][key].as_str().map(str::to_owned),
                "{what} price.{key}"
            );
        }
    }
}

#[tokio::test]
async fn the_batch_candlesticks_send_their_query_and_decode_each_captured_width() {
    for (file, period, minutes) in [
        ("batch_1", PeriodInterval::Minute1, "1"),
        ("batch_60", PeriodInterval::Hour1, "60"),
        ("batch_1440", PeriodInterval::Day1, "1440"),
    ] {
        let name = format!("candlesticks/{file}");
        let sent = wire(&name);
        let market = sent["markets"][0]["market_ticker"].as_str().unwrap();
        let mut server = Server::new_async().await;
        let mock = route(
            &mut server,
            "/trade-api/v2/markets/candlesticks",
            &format!(
                "market_tickers={market}&start_ts=100&end_ts=200&period_interval={minutes}&include_latest_before_start=true"
            ),
            &fixture(&name),
        )
        .await;
        let read = client(&server)
            .markets()
            .candlesticks(&ticker(market), period, 100, 200)
            .include_latest_before_start(true)
            .send()
            .await
            .unwrap();
        mock.assert_async().await;

        assert_eq!(
            read.markets.len(),
            sent["markets"].as_array().unwrap().len(),
            "{file}"
        );
        assert_eq!(read.markets[0].market_ticker, market, "{file}");
        rows_agree(
            &read.markets[0].candlesticks,
            sent["markets"][0]["candlesticks"].as_array().unwrap(),
            file,
        );
    }
}

#[tokio::test]
async fn the_series_candlesticks_send_their_path_and_query_and_decode_the_captured_body() {
    let sent = wire("candlesticks/series_60");
    let market = sent["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/series/KXSER-1/markets/{market}/candlesticks"),
        "start_ts=100&end_ts=200&period_interval=60",
        &fixture("candlesticks/series_60"),
    )
    .await;
    let read = client(&server)
        .markets()
        .series_candlesticks("KXSER-1", &ticker(market), PeriodInterval::Hour1, 100, 200)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.ticker, market);
    rows_agree(
        &read.candlesticks,
        sent["candlesticks"].as_array().unwrap(),
        "series_60",
    );
}

#[tokio::test]
async fn a_width_the_series_route_refuses_is_the_venues_400() {
    let body = wire("candlesticks/error_width");
    let mut server = Server::new_async().await;
    let _mock = server
        .mock(
            "GET",
            "/trade-api/v2/series/KXSER-1/markets/KXTEST-1/candlesticks",
        )
        .match_query(Matcher::Any)
        .with_status(400)
        .with_body(fixture("candlesticks/error_width"))
        .create_async()
        .await;
    let err = client_with_retries(&server, NO_RETRIES)
        .markets()
        .series_candlesticks(
            "KXSER-1",
            &ticker("KXTEST-1"),
            PeriodInterval::Hour1,
            100,
            200,
        )
        .send()
        .await
        .unwrap_err();
    let PredictionsError::Venue(venue) = &err else {
        panic!("not a venue error: {err:?}");
    };
    // The body's own message, whichever of the three shapes the host used.
    let message = body["msg"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .or_else(|| body["error"]["message"].as_str())
        .unwrap();
    assert_eq!(usize::from(venue.status), 400);
    assert_eq!(venue.message, message);
    assert!(
        matches!(err.class(), Class::VenueRefusal { .. }),
        "{:?}",
        err.class()
    );
}
