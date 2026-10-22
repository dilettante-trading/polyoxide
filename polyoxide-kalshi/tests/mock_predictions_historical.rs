//! Mock-server tests for the archive (issue #71): each request and the decoding of its captured
//! body, and an empty ticker selection refused with nothing sent.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{HistoricalSelection, PeriodInterval},
    Predictions, PredictionsError,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::Value;

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

/// A route answering `body` once, matched on its whole query string, or on its having none.
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
        .expect(1)
        .create_async()
        .await
}

/// The archived market the capture holds.
fn archived() -> Ticker {
    Ticker::new(
        wire("historical/market")["market"]["ticker"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn the_cutoff_decodes_the_captured_body() {
    let name = "historical/cutoff";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/historical/cutoff",
        "",
        &fixture(name),
    )
    .await;
    let cutoff = client(&server).historical().cutoff().send().await.unwrap();
    mock.assert_async().await;
    let sent = wire(name);
    assert_eq!(
        cutoff.market_settled_ts,
        sent["market_settled_ts"].as_str().unwrap()
    );
    assert_eq!(
        cutoff.trades_created_ts,
        sent["trades_created_ts"].as_str().unwrap()
    );
    assert_eq!(
        cutoff.orders_updated_ts,
        sent["orders_updated_ts"].as_str().unwrap()
    );
}

#[tokio::test]
async fn a_series_selection_lists_the_captured_page() {
    let name = "historical/markets_page";
    let sent = wire(name);
    let series = sent["markets"][0]["ticker"]
        .as_str()
        .unwrap()
        .split('-')
        .next()
        .unwrap()
        .to_owned();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/historical/markets",
        &format!("series_ticker={series}&limit=3"),
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .historical()
        .markets(HistoricalSelection::Series(series))
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    let tickers: Vec<&str> = page.markets.iter().map(|m| m.ticker.as_str()).collect();
    let rows: Vec<&str> = sent["markets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["ticker"].as_str().unwrap())
        .collect();
    assert_eq!(tickers, rows);
}

#[tokio::test]
async fn an_archived_market_is_read_by_ticker() {
    let ticker = archived();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/historical/markets/{ticker}"),
        "",
        &fixture("historical/market"),
    )
    .await;
    let read = client(&server)
        .historical()
        .market(&ticker)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.market.ticker, ticker.as_str());
    assert!(
        read.market.settlement_ts.is_some(),
        "an archived market has settled"
    );
}

#[tokio::test]
async fn archived_candlesticks_decode_in_their_own_shape() {
    let ticker = archived();
    for (minutes, period) in [
        (1, PeriodInterval::Minute1),
        (60, PeriodInterval::Hour1),
        (1440, PeriodInterval::Day1),
    ] {
        let name = format!("historical/candlesticks_{minutes}");
        let mut server = Server::new_async().await;
        let mock = route(
            &mut server,
            &format!("/trade-api/v2/historical/markets/{ticker}/candlesticks"),
            &format!("start_ts=100&end_ts=200&period_interval={minutes}"),
            &fixture(&name),
        )
        .await;
        let read = client(&server)
            .historical()
            .candlesticks(&ticker, period, 100, 200)
            .send()
            .await
            .unwrap();
        mock.assert_async().await;
        let sent = wire(&name);
        let rows = sent["candlesticks"].as_array().unwrap();
        assert_eq!(read.ticker, ticker.as_str(), "{name}");
        assert_eq!(read.candlesticks.len(), rows.len(), "{name}");
        for (candle, row) in read.candlesticks.iter().zip(rows) {
            assert_eq!(candle.end_period_ts, row["end_period_ts"].as_i64().unwrap());
            assert_eq!(candle.volume.to_string(), row["volume"].as_str().unwrap());
            assert_eq!(
                candle.price.close.map(|p| p.to_string()).as_deref(),
                row["price"]["close"].as_str()
            );
        }
    }
}

#[tokio::test]
async fn archived_trades_decode_the_captured_page() {
    let ticker = archived();
    let name = "historical/trades";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/historical/trades",
        &format!("ticker={ticker}&limit=20"),
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .historical()
        .trades()
        .ticker(&ticker)
        .limit(20)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    let sent = wire(name);
    let rows = sent["trades"].as_array().unwrap();
    assert_eq!(page.trades.len(), rows.len());
    for (trade, row) in page.trades.iter().zip(rows) {
        assert_eq!(trade.trade_id, row["trade_id"].as_str().unwrap());
        assert_eq!(trade.ticker, ticker.as_str());
    }
}

#[tokio::test]
async fn an_empty_ticker_selection_is_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let err = client(&server)
        .historical()
        .markets(HistoricalSelection::Tickers(Vec::new()))
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PredictionsError::Api(ApiError::Validation(reason)) if reason.starts_with("tickers: ")),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest);
    nothing.assert_async().await;
}
