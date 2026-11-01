//! Mock-server tests for `send_lossy` (issue #77): on each of the eight listings it reads a page
//! with a row the types cannot decode, keeping the rows around it and the cursor, where `send`
//! fails the page; it decodes every captured page as `send` does; and it refuses and fails as
//! `send` does.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{HistoricalSelection, MarketWindow, SkippedRow},
    Predictions, PredictionsError,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::{json, Value};

const MARKETS: &str = "/trade-api/v2/markets";
const TRADES: &str = "/trade-api/v2/markets/trades";
const ORDERBOOKS: &str = "/trade-api/v2/markets/orderbooks";
const EVENTS: &str = "/trade-api/v2/events";
const HISTORICAL_MARKETS: &str = "/trade-api/v2/historical/markets";
const HISTORICAL_TRADES: &str = "/trade-api/v2/historical/trades";

/// The key of the row each page breaks.
const BAD: &str = "KX-BAD";
/// The cursor each page with one is given, so a test sees it kept.
const CURSOR: &str = "next-page";

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn wire(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

fn ticker(text: &str) -> Ticker {
    Ticker::new(text).unwrap()
}

/// A page made from `name`'s capture: its first row, a copy of that row keyed [`BAD`] and broken
/// by `bad`, and its second row, with the cursor, where the page has one, set to [`CURSOR`].
/// Returns the body and the two good rows' keys.
fn bad_page(name: &str, rows: &str, key: &str, bad: impl Fn(&mut Value)) -> (String, Vec<String>) {
    let mut page = wire(name);
    let sent = page[rows].as_array().unwrap().clone();
    let mut broken = sent[0].clone();
    broken[key] = json!(BAD);
    bad(&mut broken);
    page[rows] = json!([sent[0], broken, sent[1]]);
    if page.get("cursor").is_some() {
        page["cursor"] = json!(CURSOR);
    }
    let good = [&sent[0], &sent[1]]
        .iter()
        .map(|row| row[key].as_str().unwrap().to_owned())
        .collect();
    (page.to_string(), good)
}

fn bad_market(row: &mut Value) {
    row["yes_bid_dollars"] = json!("n/a");
}

fn bad_trade(row: &mut Value) {
    row["count_fp"] = json!("lots");
}

/// A route answering `body` twice, once to `send_lossy` and once to `send`, matched on `query`.
async fn serve(server: &mut ServerGuard, path: &str, query: Matcher, body: &str) -> Mock {
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_body(body)
        .expect(2)
        .create_async()
        .await
}

/// These decoded query pairs, and maybe others.
fn pairs(pairs: &[(&str, &str)]) -> Matcher {
    Matcher::AllOf(
        pairs
            .iter()
            .map(|(key, value)| Matcher::UrlEncoded((*key).to_owned(), (*value).to_owned()))
            .collect(),
    )
}

/// The one row skipped is the broken one, at index 1, with its key, and the row as sent.
fn assert_skipped_the_bad_row(skipped: &[SkippedRow], body: &str, rows: &str) {
    let sent: Value = serde_json::from_str(body).unwrap();
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert_eq!(skipped[0].index, 1);
    assert_eq!(skipped[0].key.as_deref(), Some(BAD));
    assert_eq!(skipped[0].row, sent[rows][1]);
}

fn assert_decode<T: std::fmt::Debug>(strict: Result<T, PredictionsError>) {
    let err = strict.unwrap_err();
    assert_eq!(err.class(), Class::Decode, "{err}");
}

#[tokio::test]
async fn a_market_list_by_tickers_skips_a_market_that_does_not_decode() {
    let (body, good) = bad_page("markets_page", "markets", "ticker", bad_market);
    let asked = [ticker(&good[0]), ticker(BAD), ticker(&good[1])];
    let joined = format!("{},{BAD},{}", good[0], good[1]);
    let mut server = Server::new_async().await;
    let mock = serve(&mut server, MARKETS, pairs(&[("tickers", &joined)]), &body).await;
    let kalshi = client(&server);

    let lossy = kalshi
        .markets()
        .list()
        .tickers(&asked)
        .send_lossy()
        .await
        .unwrap();
    let kept: Vec<&str> = lossy
        .page
        .markets
        .iter()
        .map(|m| m.ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_eq!(lossy.page.cursor, CURSOR);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "markets");

    assert_decode(kalshi.markets().list().tickers(&asked).send().await);
    mock.assert_async().await;
}

#[tokio::test]
async fn a_market_window_skips_a_market_that_does_not_decode() {
    let (body, good) = bad_page(
        "catalog/markets_window_settled_page",
        "markets",
        "ticker",
        bad_market,
    );
    let window = MarketWindow::Settled {
        settled_only: true,
        min_ts: Some(100),
        max_ts: None,
    };
    let query = pairs(&[("status", "settled"), ("min_settled_ts", "100")]);
    let mut server = Server::new_async().await;
    let mock = serve(&mut server, MARKETS, query, &body).await;
    let kalshi = client(&server);

    let lossy = kalshi.markets().window(window).send_lossy().await.unwrap();
    let kept: Vec<&str> = lossy
        .page
        .markets
        .iter()
        .map(|m| m.ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_eq!(lossy.page.cursor, CURSOR);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "markets");

    assert_decode(kalshi.markets().window(window).send().await);
    mock.assert_async().await;
}

#[tokio::test]
async fn updated_markets_skip_a_market_that_does_not_decode() {
    let (body, good) = bad_page(
        "catalog/markets_updated_page",
        "markets",
        "ticker",
        bad_market,
    );
    let query = pairs(&[("min_updated_ts", "9"), ("mve_filter", "exclude")]);
    let mut server = Server::new_async().await;
    let mock = serve(&mut server, MARKETS, query, &body).await;
    let kalshi = client(&server);

    let lossy = kalshi.markets().updated(9).send_lossy().await.unwrap();
    let kept: Vec<&str> = lossy
        .page
        .markets
        .iter()
        .map(|m| m.ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_eq!(lossy.page.cursor, CURSOR);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "markets");

    assert_decode(kalshi.markets().updated(9).send().await);
    mock.assert_async().await;
}

#[tokio::test]
async fn trades_skip_a_trade_that_does_not_decode() {
    let (body, good) = bad_page("trades", "trades", "ticker", bad_trade);
    let mut server = Server::new_async().await;
    let mock = serve(&mut server, TRADES, pairs(&[("limit", "3")]), &body).await;
    let kalshi = client(&server);

    let lossy = kalshi
        .markets()
        .trades()
        .limit(3)
        .send_lossy()
        .await
        .unwrap();
    let kept: Vec<&str> = lossy
        .page
        .trades
        .iter()
        .map(|t| t.ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_eq!(lossy.page.cursor, CURSOR);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "trades");

    assert_decode(kalshi.markets().trades().limit(3).send().await);
    mock.assert_async().await;
}

#[tokio::test]
async fn orderbooks_skip_a_book_that_does_not_decode() {
    let (body, good) = bad_page("catalog/orderbooks", "orderbooks", "ticker", |row| {
        row["orderbook_fp"]["yes_dollars"] = json!([["0.42"]]);
    });
    let asked = [ticker(&good[0]), ticker(BAD), ticker(&good[1])];
    // mockito's `UrlEncoded` keeps one value of a repeated key, so the query is matched whole; a
    // ticker's characters need no encoding.
    let query = Matcher::Exact(format!(
        "tickers={}&tickers={BAD}&tickers={}",
        good[0], good[1]
    ));
    let mut server = Server::new_async().await;
    let mock = serve(&mut server, ORDERBOOKS, query, &body).await;
    let kalshi = client(&server);

    let lossy = kalshi
        .markets()
        .orderbooks(&asked)
        .send_lossy()
        .await
        .unwrap();
    let kept: Vec<&str> = lossy
        .page
        .orderbooks
        .iter()
        .map(|b| b.ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "orderbooks");

    assert_decode(kalshi.markets().orderbooks(&asked).send().await);
    mock.assert_async().await;
}

#[tokio::test]
async fn events_skip_an_event_that_does_not_decode() {
    let (body, good) = bad_page("catalog/events_page", "events", "event_ticker", |row| {
        row.as_object_mut().unwrap().remove("title");
    });
    let mut server = Server::new_async().await;
    let mock = serve(&mut server, EVENTS, pairs(&[("limit", "3")]), &body).await;
    let kalshi = client(&server);

    let lossy = kalshi.events().list().limit(3).send_lossy().await.unwrap();
    let kept: Vec<&str> = lossy
        .page
        .events
        .iter()
        .map(|e| e.event_ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_eq!(lossy.page.cursor, CURSOR);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "events");
    assert!(
        lossy.skipped[0].error.contains("title"),
        "{}",
        lossy.skipped[0].error
    );

    assert_decode(kalshi.events().list().limit(3).send().await);
    mock.assert_async().await;
}

#[tokio::test]
async fn archived_markets_skip_a_market_that_does_not_decode() {
    let (body, good) = bad_page("historical/markets_page", "markets", "ticker", bad_market);
    let mut server = Server::new_async().await;
    let query = pairs(&[("mve_filter", "exclude")]);
    let mock = serve(&mut server, HISTORICAL_MARKETS, query, &body).await;
    let kalshi = client(&server);
    let selection = || HistoricalSelection::ExcludeMultivariate;

    let lossy = kalshi
        .historical()
        .markets(selection())
        .send_lossy()
        .await
        .unwrap();
    let kept: Vec<&str> = lossy
        .page
        .markets
        .iter()
        .map(|m| m.ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_eq!(lossy.page.cursor, CURSOR);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "markets");

    assert_decode(kalshi.historical().markets(selection()).send().await);
    mock.assert_async().await;
}

#[tokio::test]
async fn archived_trades_skip_a_trade_that_does_not_decode() {
    let (body, good) = bad_page("historical/trades", "trades", "ticker", bad_trade);
    let mut server = Server::new_async().await;
    let mock = serve(
        &mut server,
        HISTORICAL_TRADES,
        pairs(&[("limit", "3")]),
        &body,
    )
    .await;
    let kalshi = client(&server);

    let lossy = kalshi
        .historical()
        .trades()
        .limit(3)
        .send_lossy()
        .await
        .unwrap();
    let kept: Vec<&str> = lossy
        .page
        .trades
        .iter()
        .map(|t| t.ticker.as_str())
        .collect();
    assert_eq!(kept, good);
    assert_eq!(lossy.page.cursor, CURSOR);
    assert_skipped_the_bad_row(&lossy.skipped, &body, "trades");

    assert_decode(kalshi.historical().trades().limit(3).send().await);
    mock.assert_async().await;
}

/// Serves the capture `$name` on `$path` to the request `$build` makes, through `send_lossy` and
/// then `send`, and holds the two decodes equal, with nothing skipped.
macro_rules! decodes_as_send_does {
    ($name:expr, $path:expr, |$kalshi:ident| $build:expr) => {{
        let mut server = Server::new_async().await;
        let mock = serve(&mut server, $path, Matcher::Any, &fixture($name)).await;
        let $kalshi = client(&server);
        let lossy = $build.send_lossy().await.unwrap();
        let strict = $build.send().await.unwrap();
        assert!(
            lossy.skipped.is_empty(),
            "{}: skipped {:?}",
            $name,
            lossy.skipped
        );
        assert_eq!(lossy.page, strict, "{}", $name);
        mock.assert_async().await;
    }};
}

#[tokio::test]
async fn every_captured_page_decodes_through_send_lossy_as_through_send() {
    let one = || [ticker("KX-ONE")];
    decodes_as_send_does!("markets_page", MARKETS, |k| k.markets().list());
    decodes_as_send_does!("catalog/markets_tickers_page", MARKETS, |k| k
        .markets()
        .list()
        .tickers(&one()));
    decodes_as_send_does!("catalog/markets_updated_page", MARKETS, |k| k
        .markets()
        .updated(0));
    decodes_as_send_does!("catalog/markets_window_settled_page", MARKETS, |k| k
        .markets()
        .window(MarketWindow::Settled {
            settled_only: true,
            min_ts: None,
            max_ts: None,
        }));
    decodes_as_send_does!("trades", TRADES, |k| k.markets().trades());
    decodes_as_send_does!("catalog/orderbooks", ORDERBOOKS, |k| k
        .markets()
        .orderbooks(&one()));
    decodes_as_send_does!("catalog/events_page", EVENTS, |k| k.events().list());
    decodes_as_send_does!("catalog/events_page_nested", EVENTS, |k| k
        .events()
        .list()
        .with_nested_markets(true));
    decodes_as_send_does!("historical/markets_page", HISTORICAL_MARKETS, |k| k
        .historical()
        .markets(HistoricalSelection::All));
    decodes_as_send_does!("historical/trades", HISTORICAL_TRADES, |k| k
        .historical()
        .trades());
}

#[tokio::test]
async fn an_empty_ticker_set_is_refused_through_send_lossy_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;

    let err = client(&server)
        .markets()
        .list()
        .tickers(&[])
        .send_lossy()
        .await
        .unwrap_err();

    assert!(
        matches!(&err, PredictionsError::Api(ApiError::Validation(_))),
        "{err:?}"
    );
    nothing.assert_async().await;
}

#[tokio::test]
async fn kalshis_error_body_is_a_venue_error_through_send_lossy() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", TRADES)
        .match_query(Matcher::Any)
        .with_status(404)
        .with_body(fixture("error_not_found"))
        .expect(1)
        .create_async()
        .await;

    let err = client(&server)
        .markets()
        .trades()
        .send_lossy()
        .await
        .unwrap_err();

    match &err {
        PredictionsError::Venue(venue) => {
            assert_eq!(venue.status, 404);
            assert_eq!(venue.code.as_deref(), Some("not_found"));
        }
        other => panic!("not a venue error: {other:?}"),
    }
    mock.assert_async().await;
}
