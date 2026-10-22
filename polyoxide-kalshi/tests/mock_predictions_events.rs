//! Mock-server tests for the events listing (issue #71): its request, the decoding of its captured
//! pages, and an empty event set refused with nothing sent.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{types::EventStatusFilter, Predictions, PredictionsError};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::Value;

const EVENTS: &str = "/trade-api/v2/events";

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

async fn route(server: &mut ServerGuard, query: &str, body: &str) -> Mock {
    server
        .mock("GET", EVENTS)
        .match_query(Matcher::Exact(query.to_owned()))
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn the_listing_sends_its_filters_and_decodes_the_captured_page() {
    let name = "catalog/events_page";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "limit=3&status=open&min_updated_ts=9",
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .events()
        .list()
        .limit(3)
        .status(EventStatusFilter::Open)
        .min_updated_ts(9)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    let rows = sent["events"].as_array().unwrap();
    assert_eq!(page.events.len(), rows.len());
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    for (event, row) in page.events.iter().zip(rows) {
        assert_eq!(event.event_ticker, row["event_ticker"].as_str().unwrap());
        assert!(
            event.markets.is_empty(),
            "markets are nested only when asked for"
        );
    }
}

#[tokio::test]
async fn a_nested_page_carries_each_events_markets() {
    let name = "catalog/events_page_nested";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "status=open&with_nested_markets=true",
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .events()
        .list()
        .status(EventStatusFilter::Open)
        .with_nested_markets(true)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    for (event, row) in page.events.iter().zip(sent["events"].as_array().unwrap()) {
        let nested = row["markets"].as_array().map_or(0, Vec::len);
        assert_eq!(event.markets.len(), nested, "{}", event.event_ticker);
    }
}

#[tokio::test]
async fn an_empty_event_set_is_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let err = client(&server)
        .events()
        .list()
        .tickers(Vec::<String>::new())
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
