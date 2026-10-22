//! Mock-server tests for the markets catalog routes (issue #71): each builder's request, the
//! decoding of its captured body, and the ticker sets refused with nothing sent. The setters'
//! exact pairs are `query_setters_predictions.rs`'s.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{MarketWindow, MarketsPage},
    Predictions, PredictionsError, MAX_ORDERBOOK_TICKERS,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::Value;

const MARKETS: &str = "/trade-api/v2/markets";

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

/// A route answering `body` once, matched on these decoded query pairs.
async fn route(server: &mut ServerGuard, path: &str, pairs: &[(&str, &str)], body: &str) -> Mock {
    let query = Matcher::AllOf(
        pairs
            .iter()
            .map(|(key, value)| Matcher::UrlEncoded((*key).to_owned(), (*value).to_owned()))
            .collect(),
    );
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

/// A server that fails the test if it is asked for anything.
async fn silent() -> (ServerGuard, Mock) {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    (server, nothing)
}

fn page_tickers(page: &MarketsPage) -> Vec<&str> {
    page.markets
        .iter()
        .map(|market| market.ticker.as_str())
        .collect()
}

fn wire_tickers(body: &Value) -> Vec<&str> {
    body["markets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|market| market["ticker"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_settled_window_sends_its_state_and_bounds_and_decodes_the_captured_page() {
    let name = "catalog/markets_window_settled_page";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        MARKETS,
        &[
            ("status", "settled"),
            ("min_settled_ts", "100"),
            ("max_settled_ts", "200"),
            ("series_ticker", "SER-1"),
            ("limit", "3"),
        ],
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .markets()
        .window(MarketWindow::Settled {
            settled_only: true,
            min_ts: Some(100),
            max_ts: Some(200),
        })
        .series_ticker("SER-1")
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    assert_eq!(page_tickers(&page), wire_tickers(&sent));
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    for (market, row) in page.markets.iter().zip(sent["markets"].as_array().unwrap()) {
        assert_eq!(
            market.settlement_ts.as_deref(),
            row["settlement_ts"].as_str()
        );
    }
}

#[tokio::test]
async fn updated_excludes_multivariate_combinations_and_decodes_the_captured_page() {
    let name = "catalog/markets_updated_page";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        MARKETS,
        &[
            ("min_updated_ts", "100"),
            ("mve_filter", "exclude"),
            ("max_updated_ts", "200"),
        ],
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .markets()
        .updated(100)
        .max_ts(200)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    assert_eq!(page_tickers(&page), wire_tickers(&sent));
    for (market, row) in page.markets.iter().zip(sent["markets"].as_array().unwrap()) {
        assert_eq!(market.updated_time, row["updated_time"].as_str().unwrap());
    }
}

#[tokio::test]
async fn tickers_are_sent_comma_joined_and_the_captured_page_decodes() {
    let name = "catalog/markets_tickers_page";
    let sent = wire(name);
    let asked: Vec<Ticker> = wire_tickers(&sent).into_iter().map(ticker).collect();
    let joined = asked
        .iter()
        .map(Ticker::as_str)
        .collect::<Vec<_>>()
        .join(",");
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        MARKETS,
        &[("tickers", joined.as_str())],
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .markets()
        .list()
        .tickers(&asked)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(page_tickers(&page), wire_tickers(&sent));
}

#[tokio::test]
async fn orderbooks_sends_one_pair_per_ticker_and_decodes_the_captured_books() {
    let name = "catalog/orderbooks";
    let sent = wire(name);
    let rows = sent["orderbooks"].as_array().unwrap();
    let asked: Vec<Ticker> = rows
        .iter()
        .map(|row| ticker(row["ticker"].as_str().unwrap()))
        .collect();
    // mockito's `UrlEncoded` keeps one value of a repeated key, so the query is matched whole; a
    // ticker's characters need no encoding.
    let query = asked
        .iter()
        .map(|t| format!("tickers={t}"))
        .collect::<Vec<_>>()
        .join("&");
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/trade-api/v2/markets/orderbooks")
        .match_query(Matcher::Exact(query))
        .with_status(200)
        .with_body(fixture(name))
        .expect(1)
        .create_async()
        .await;
    let books = client(&server)
        .markets()
        .orderbooks(&asked)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    assert_eq!(books.orderbooks.len(), rows.len());
    let side = |row: &Value, key: &str| row["orderbook_fp"][key].as_array().map_or(0, Vec::len);
    for (book, row) in books.orderbooks.iter().zip(rows) {
        assert_eq!(book.ticker, row["ticker"].as_str().unwrap());
        assert_eq!(
            book.orderbook_fp.yes_dollars.len(),
            side(row, "yes_dollars")
        );
        assert_eq!(book.orderbook_fp.no_dollars.len(), side(row, "no_dollars"));
    }
}

fn assert_refused(err: &PredictionsError, what: &str) {
    assert!(
        matches!(err, PredictionsError::Api(ApiError::Validation(reason)) if reason.starts_with("tickers: ")),
        "{what}: {err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest, "{what}");
}

#[tokio::test]
async fn an_empty_or_oversized_ticker_set_is_refused_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let kalshi = client(&server);
    // 31 characters each: 250 cost 10,000 bytes, past the 8,000-byte budget.
    let long: Vec<Ticker> = (0..250)
        .map(|i| ticker(&format!("KXLONGTICKER-26OCT22-T{i:09}")))
        .collect();
    let many: Vec<Ticker> = (0..=MAX_ORDERBOOK_TICKERS)
        .map(|i| ticker(&format!("KXT-{i}")))
        .collect();

    let err = kalshi
        .markets()
        .list()
        .tickers(&[])
        .send()
        .await
        .unwrap_err();
    assert_refused(&err, "an empty listing set");
    let err = kalshi
        .markets()
        .list()
        .tickers(&long)
        .send()
        .await
        .unwrap_err();
    assert_refused(&err, "a listing set past the budget");
    let err = kalshi
        .markets()
        .window(MarketWindow::Closed {
            closed_only: true,
            min_ts: None,
            max_ts: None,
        })
        .tickers(&[])
        .send()
        .await
        .unwrap_err();
    assert_refused(&err, "an empty window set");
    let err = kalshi.markets().orderbooks(&[]).send().await.unwrap_err();
    assert_refused(&err, "an empty orderbooks set");
    let err = kalshi.markets().orderbooks(&many).send().await.unwrap_err();
    assert_refused(&err, "101 orderbooks");
    nothing.assert_async().await;
}
