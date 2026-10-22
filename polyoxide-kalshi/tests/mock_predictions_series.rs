//! Mock-server tests for the series routes (issue #71): each request and the decoding of its
//! captured body.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_kalshi::predictions::Predictions;
use polyoxide_test_support::fixtures;
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

async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str) -> Mock {
    server
        .mock("GET", path)
        .match_query(Matcher::Exact(query.to_owned()))
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn the_series_list_sends_its_filters_and_decodes_the_captured_body() {
    let name = "catalog/series_list";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/series",
        "category=Economics&min_updated_ts=9",
        &fixture(name),
    )
    .await;
    let list = client(&server)
        .series()
        .list()
        .category("Economics")
        .min_updated_ts(9)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    let rows = sent["series"].as_array().unwrap();
    assert_eq!(list.series.len(), rows.len());
    for (series, row) in list.series.iter().zip(rows) {
        assert_eq!(series.ticker, row["ticker"].as_str().unwrap());
        assert_eq!(series.category, row["category"].as_str().unwrap());
        assert_eq!(series.tags.is_none(), row["tags"].is_null());
        assert_eq!(
            series.fee_multiplier.to_string(),
            row["fee_multiplier"].to_string()
        );
    }
}

#[tokio::test]
async fn a_series_is_read_by_ticker_with_its_volume() {
    let name = "catalog/series";
    let sent = wire(name);
    let ticker = sent["series"]["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/series/{ticker}"),
        "include_volume=true",
        &fixture(name),
    )
    .await;
    let read = client(&server)
        .series()
        .get(ticker)
        .include_volume(true)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.series.ticker, ticker);
    assert_eq!(
        read.series
            .volume_fp
            .map(|volume| volume.to_string())
            .as_deref(),
        sent["series"]["volume_fp"].as_str()
    );
}
