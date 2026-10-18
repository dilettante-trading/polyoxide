//! Live integration tests against production Kalshi (`external-api.kalshi.com`), unsigned.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d. No credentials
//! are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions -- --ignored
//! ```
//!
//! `live_responses_carry_no_unmodelled_keys` is the host's drift detector for the types, and
//! `live_the_route_table_agrees_with_the_endpoint_costs` the table of costs' (D5). The first
//! sees keys and the length of the positional order-book rows. It does not see a new value of an
//! enum, which decodes as that enum's `Other` variant.

mod common;

use std::time::Duration;

use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    request::{route_templates, template_matches, DEFAULT_COST},
    types::{
        EndpointCosts, EventResponse, ExchangeStatus, MarketResponse, MarketsPage,
        OrderbookResponse, TradesPage,
    },
    Predictions, PredictionsError, PRODUCTION,
};
use polyoxide_test_support::{agreement, fail, ResultExt};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

fn client() -> Predictions {
    Predictions::new().or_fail("kalshi client")
}

#[tokio::test]
#[ignore]
async fn live_the_exchange_status_and_a_ping_answer() {
    let client = client();
    let exchange = client
        .exchange()
        .status()
        .send()
        .await
        .or_fail("exchange status");
    assert!(
        exchange
            .exchange_index_statuses
            .iter()
            .any(|index| index.exchange_index == 0),
        "the default exchange index (0) is listed"
    );
    let pong = client.health().ping().await.or_fail("ping");
    assert!(
        pong.round_trip < Duration::from_secs(10),
        "round trip {:?}",
        pong.round_trip
    );
}

#[tokio::test]
#[ignore]
async fn live_a_two_sided_market_reads_through_every_route() {
    let client = client();
    let market = common::two_sided_market(&client).await;
    let ticker = Ticker::new(&market.ticker).or_fail(&market.ticker);

    let read = client.markets().get(&ticker).send().await.or_fail("market");
    assert_eq!(read.market.ticker, market.ticker);

    // The host lists each side worst first.
    let book = client
        .markets()
        .orderbook(&ticker)
        .send()
        .await
        .or_fail("orderbook")
        .orderbook_fp;
    assert!(
        !book.yes_dollars.is_empty() && !book.no_dollars.is_empty(),
        "{ticker}"
    );
    for levels in [&book.yes_dollars, &book.no_dollars] {
        assert!(
            levels.windows(2).all(|pair| pair[0].price < pair[1].price),
            "{ticker}: levels are not in ascending price"
        );
    }
    let shallow = client
        .markets()
        .orderbook(&ticker)
        .depth(1)
        .send()
        .await
        .or_fail("orderbook, depth 1")
        .orderbook_fp;
    assert!(
        shallow.yes_dollars.len() <= 1 && shallow.no_dollars.len() <= 1,
        "{ticker}"
    );

    let trades = client
        .markets()
        .trades()
        .ticker(&ticker)
        .limit(10)
        .send()
        .await
        .or_fail("trades");
    assert!(trades.trades.iter().all(|t| t.ticker == market.ticker));

    let event = client
        .events()
        .get(&market.event_ticker)
        .with_nested_markets(true)
        .send()
        .await
        .or_fail("event");
    assert_eq!(event.event.event_ticker, market.event_ticker);
    assert!(
        event
            .event
            .markets
            .iter()
            .any(|m| m.ticker == market.ticker),
        "{ticker} is not among its event's nested markets"
    );
}

/// An unlisted ticker is a `404` on the market and a `200` with an empty book on the order book.
#[tokio::test]
#[ignore]
async fn live_an_unknown_ticker_is_a_404_on_the_market_and_an_empty_book() {
    let client = client();
    let missing = Ticker::new("NOSUCHMARKET-0").or_fail("a ticker");

    let result = client.markets().get(&missing).send().await;
    assert!(result.is_err(), "{missing} is listed");
    let Err(err) = result else { return };
    if !matches!(
        &err,
        PredictionsError::Venue(venue)
            if venue.status == 404 && venue.code.as_deref() == Some("not_found")
    ) {
        fail("expected a 404 not_found", &err);
    }

    let book = client
        .markets()
        .orderbook(&missing)
        .send()
        .await
        .or_fail("orderbook of an unknown ticker")
        .orderbook_fp;
    assert!(book.yes_dollars.is_empty() && book.no_dollars.is_empty());
}

/// Every ticker the host serves must be one `Ticker::new` accepts, or a market cannot be read.
#[tokio::test]
#[ignore]
async fn live_every_ticker_the_host_lists_is_one_the_client_accepts() {
    let client = client();
    let listed = client
        .markets()
        .list()
        .limit(1000)
        .send()
        .await
        .or_fail("markets");
    for market in &listed.markets {
        Ticker::new(&market.ticker).or_fail(&market.ticker);
    }
    let traded = client
        .markets()
        .trades()
        .limit(1000)
        .send()
        .await
        .or_fail("trades");
    for trade in &traded.trades {
        Ticker::new(&trade.ticker).or_fail(&trade.ticker);
    }
}

/// D5's drift test: the crate charges every route the default cost, so the host's default is
/// the crate's, and a route it lists must be listed at that cost.
#[tokio::test]
#[ignore]
async fn live_the_route_table_agrees_with_the_endpoint_costs() {
    let costs = client()
        .account()
        .endpoint_costs()
        .send()
        .await
        .or_fail("endpoint costs");
    assert_eq!(costs.default_cost, DEFAULT_COST, "the host's default cost");
    assert!(!costs.endpoint_costs.is_empty());

    let mut checked = 0;
    for (method, template) in route_templates() {
        checked += 1;
        for listed in costs.endpoint_costs.iter().filter(|c| {
            c.method.eq_ignore_ascii_case(method) && template_matches(template, &c.path)
        }) {
            assert_eq!(
                listed.cost, DEFAULT_COST,
                "{method} {template} is listed at {} tokens as {}, and the crate charges {DEFAULT_COST}",
                listed.cost, listed.path
            );
        }
    }
    assert_eq!(checked, 8, "the route table holds eight routes");
}

/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("{PRODUCTION}{path}"))
        .send()
        .await
        .map_err(ApiError::Network)
        .or_fail(path);
    // A refusal fails as core classifies its status, so its tag says what the nightly does with
    // it: a 429 or a 5xx is transient.
    if !response.status().is_success() {
        fail(path, &ApiError::from_response(response).await);
    }
    response
        .text()
        .await
        .map_err(ApiError::Network)
        .or_fail(path)
}

fn json(path: &str, text: &str) -> Value {
    serde_json::from_str(text)
        .map_err(ApiError::Serialization)
        .or_fail(path)
}

/// Positional rows of any length but `len`. The key comparison cannot see a value appended to an
/// order-book level, which has no keys.
fn rows_not_of_length(path: &str, rows: &Value, len: usize) -> Vec<String> {
    rows.as_array()
        .into_iter()
        .flatten()
        .map(|row| row.as_array().map_or(0, Vec::len))
        .filter(|&n| n != len)
        .map(|n| format!("{path}: a row of {n} values, not {len}"))
        .collect()
}

fn compare_live<T: DeserializeOwned + Serialize>(path: &str, text: &str) -> Vec<String> {
    let diff = agreement::compare::<T>(path, text);
    // A field the type models but this response omits is not drift: it may be sent only
    // sometimes. A key the type does not model is.
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
    let client = client();
    let market = common::two_sided_market(&client).await;
    let ticker = &market.ticker;
    let event_ticker = &market.event_ticker;

    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(ApiError::Network)
        .or_fail("a reqwest client");
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/trade-api/v2/exchange/status";
    check(compare_live::<ExchangeStatus>(
        path,
        &raw(&http, path).await,
    ));
    let path = "/trade-api/v2/account/endpoint_costs";
    check(compare_live::<EndpointCosts>(path, &raw(&http, path).await));
    // The same filtered listing the capture and `two_sided_market` page.
    let path = "/trade-api/v2/markets?status=open&mve_filter=exclude&limit=5";
    check(compare_live::<MarketsPage>(path, &raw(&http, path).await));
    let path = format!("/trade-api/v2/markets/{ticker}");
    check(compare_live::<MarketResponse>(
        &path,
        &raw(&http, &path).await,
    ));
    let path = format!("/trade-api/v2/markets/{ticker}/orderbook");
    let text = raw(&http, &path).await;
    check(compare_live::<OrderbookResponse>(&path, &text));
    let book = json(&path, &text);
    check(rows_not_of_length(
        &path,
        &book["orderbook_fp"]["yes_dollars"],
        2,
    ));
    check(rows_not_of_length(
        &path,
        &book["orderbook_fp"]["no_dollars"],
        2,
    ));
    let path = format!("/trade-api/v2/markets/trades?ticker={ticker}&limit=10");
    check(compare_live::<TradesPage>(&path, &raw(&http, &path).await));
    let path = format!("/trade-api/v2/events/{event_ticker}?with_nested_markets=true");
    check(compare_live::<EventResponse>(
        &path,
        &raw(&http, &path).await,
    ));

    assert!(
        unmodelled.is_empty(),
        "the host sent what the types do not model; model it as an optional field, excuse it \
         in OBSERVED_EXTRA and record it in docs/specs/kalshi/OBSERVED.md: {unmodelled:#?}"
    );
}
