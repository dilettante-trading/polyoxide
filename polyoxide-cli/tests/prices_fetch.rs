use std::num::NonZeroU32;
use std::sync::Arc;

use governor::{Quota, RateLimiter};
use mockito::{Matcher, Server};
use polyoxide_cli::commands::clob::prices::fetch::fetch_one;
use polyoxide_clob::{ClobBuilder, PricesHistoryQuery};

fn limiter() -> Arc<governor::DefaultDirectRateLimiter> {
    Arc::new(RateLimiter::direct(Quota::per_second(
        NonZeroU32::new(1000).unwrap(),
    )))
}

#[tokio::test]
async fn fetch_one_retries_until_exhausted_then_errors() {
    let mut server = Server::new_async().await;
    // Always fails (HTTP 500 -> API error). With max_retries = 2,
    // fetch_one makes 3 attempts total.
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(500)
        .expect(3)
        .create_async()
        .await;

    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let result = fetch_one(
        &clob,
        "0xtoken",
        &PricesHistoryQuery::default(),
        &limiter(),
        2,
    )
    .await;

    assert!(result.is_err());
    mock.assert_async().await;
}

#[tokio::test]
async fn fetch_one_succeeds_first_try() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .expect(1)
        .create_async()
        .await;

    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let points = fetch_one(
        &clob,
        "0xtoken",
        &PricesHistoryQuery::default(),
        &limiter(),
        2,
    )
    .await
    .unwrap();

    assert_eq!(points.len(), 1);
    mock.assert_async().await;
}
