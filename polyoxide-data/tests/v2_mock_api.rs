//! Mock-server tests for Data API v2 envelopes, paging and errors.
//!
//! Error bodies are the ones the live host returned on 2026-09-14, not
//! invented shapes.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{StreamExt, TryStreamExt};
use mockito::{Matcher, Server, ServerGuard};
use polyoxide_core::{ApiError, RetryConfig};
use polyoxide_data::{
    v2::{types::TradeSide, ErrorCode},
    DataApi, DataApiError,
};

fn client(server: &ServerGuard) -> DataApi {
    DataApi::builder().base_url(server.url()).build().unwrap()
}

/// A client that hands the first 429 straight to error mapping.
fn client_without_retries(server: &ServerGuard) -> DataApi {
    DataApi::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap()
}

/// First row of the live bare `/v2/trades` feed on 2026-09-14. Its
/// `outcome_index` is the `999` sentinel.
const TRADE: &str = r#"{"proxy_wallet":"0x3048d65321be3497164cdfc2996f94f98a2e7537","side":"BUY","token_id":"72093955541185727521455347008976104881454275052560738070668094680183479632220","condition_id":"0xc76e164a38fa0cbe49e5da48c5475b60771af13cf2bdc619dfbdd5f4cf641f7d","size":10.0,"price":0.78,"timestamp":1789373197,"title":"Bitcoin Up or Down - September 14, 4:05AM-4:10AM ET","slug":"btc-updown-5m-1789373100","icon":"https://polymarket-upload.s3.us-east-2.amazonaws.com/BTC+fullsize.png","event_slug":"","outcome":"Up","outcome_index":999,"name":"x-MoneyForWhiskas","pseudonym":"Flashy-Gold","bio":"Trading to pay for my cat’s food.","profile_image":"https://polymarket-upload.s3.us-east-2.amazonaws.com/profile-image-8514165-de41d4be-9f14-4485-8933-a694dac4b7f3.png","profile_image_optimized":"","transaction_hash":"0xcfdca816bb78be8c76eb50a599a849c6526082c1ceead836b094ad9564bec718"}"#;

fn trades_page(rows: usize, next_cursor: Option<&str>) -> String {
    let data = vec![TRADE; rows].join(",");
    let next = next_cursor.map_or("null".to_owned(), |c| format!("\"{c}\""));
    format!(
        r#"{{"data":[{data}],"pagination":{{"limit":1,"offset":0,"has_more":{},"next_cursor":{next}}}}}"#,
        next_cursor.is_some()
    )
}

fn cursor_of(path_and_query: &str) -> Option<String> {
    let url = url::Url::parse(&format!("http://mock{path_and_query}")).unwrap();
    url.query_pairs()
        .find(|(k, _)| k == "cursor")
        .map(|(_, v)| v.into_owned())
}

// ── Envelopes ────────────────────────────────────────────────────────

#[tokio::test]
async fn trades_send_returns_the_page_and_its_pagination() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v2/trades")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(trades_page(1, Some("c1")))
        .create_async()
        .await;

    let page = client(&server).v2().trades().send().await.unwrap();

    assert_eq!(page.data.len(), 1);
    let trade = &page.data[0];
    assert_eq!(trade.side, TradeSide::Buy);
    assert_eq!(trade.outcome_index.raw(), 999);
    assert_eq!(trade.outcome_index.get(), None, "999 means unlabeled");
    assert_eq!(page.pagination.next_cursor.as_deref(), Some("c1"));
    assert!(page.pagination.has_more);
}

#[tokio::test]
async fn user_stats_null_data_is_none() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v2/user-stats")
        .match_query(Matcher::UrlEncoded("user".into(), "0xnobody".into()))
        .with_status(200)
        .with_body(r#"{"data":null}"#)
        .create_async()
        .await;

    let stats = client(&server)
        .v2()
        .user_stats("0xnobody")
        .send()
        .await
        .unwrap();

    assert!(stats.is_none());
}

#[tokio::test]
async fn user_stats_known_user_with_no_history_is_some_with_zeros() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v2/user-stats")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(
            r#"{"data":{"proxy_wallet":"0xnew","trades":0,"biggest_win":0.0,"views":0,"join_date":null,"all_time_pnl":null}}"#,
        )
        .create_async()
        .await;

    let stats = client(&server)
        .v2()
        .user_stats("0xnew")
        .send()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(stats.trades, 0);
    assert_eq!(stats.join_date, None);
    assert!(stats.all_time_pnl.is_none());
}

// ── Paging ───────────────────────────────────────────────────────────

#[tokio::test]
async fn pages_resend_identical_filters_with_each_cursor() {
    let mut server = Server::new_async().await;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&seen);
    server
        .mock("GET", "/v2/trades")
        .match_query(Matcher::Any)
        .match_request(move |request| {
            sink.lock()
                .unwrap()
                .push(request.path_and_query().to_owned());
            true
        })
        .with_status(200)
        .with_body_from_request(|request| {
            match cursor_of(request.path_and_query()).as_deref() {
                None => trades_page(1, Some("c1")),
                // An empty page that still has a cursor does not end the walk.
                Some("c1") => trades_page(0, Some("c2")),
                Some("c2") => trades_page(1, None),
                Some(other) => panic!("unexpected cursor {other}"),
            }
            .into_bytes()
        })
        .expect(3)
        .create_async()
        .await;

    let pages: Vec<_> = client(&server)
        .v2()
        .trades()
        .user("0xuser")
        .side(TradeSide::Buy)
        .limit(1)
        .pages()
        .try_collect()
        .await
        .unwrap();

    assert_eq!(
        pages.iter().map(|p| p.data.len()).collect::<Vec<_>>(),
        [1, 0, 1]
    );

    let seen = seen.lock().unwrap();
    let cursors: Vec<Option<String>> = seen.iter().map(|pq| cursor_of(pq)).collect();
    assert_eq!(cursors, [None, Some("c1".into()), Some("c2".into())]);

    let filters: Vec<Vec<(String, String)>> = seen
        .iter()
        .map(|pq| {
            url::Url::parse(&format!("http://mock{pq}"))
                .unwrap()
                .query_pairs()
                .filter(|(k, _)| k != "cursor")
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .collect();
    let expected: Vec<(String, String)> = vec![
        ("user".into(), "0xuser".into()),
        ("side".into(), "BUY".into()),
        ("limit".into(), "1".into()),
    ];
    for (i, page_filters) in filters.iter().enumerate() {
        assert_eq!(
            page_filters,
            &expected,
            "page {} sent different filters",
            i + 1
        );
    }
}

#[tokio::test]
async fn pages_stop_with_an_error_when_the_server_repeats_a_cursor() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v2/trades")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body_from_request(|_| trades_page(1, Some("stuck")).into_bytes())
        .create_async()
        .await;

    // `take(3)` bounds the walk, so a missing guard fails here instead of looping.
    let results: Vec<_> = client(&server)
        .v2()
        .trades()
        .pages()
        .take(3)
        .collect()
        .await;

    assert_eq!(results.len(), 2, "one page, then the error, then the end");
    assert!(results[0].is_ok());
    assert!(matches!(results[1], Err(DataApiError::Pagination(_))));
}

#[tokio::test]
async fn pages_end_after_yielding_an_error_mid_walk() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/v2/trades")
        .match_query(Matcher::Any)
        .with_status_code_from_request(|request| {
            if cursor_of(request.path_and_query()).is_some() {
                500
            } else {
                200
            }
        })
        .with_body_from_request(|request| {
            match cursor_of(request.path_and_query()) {
                None => trades_page(1, Some("c1")),
                Some(_) => {
                    r#"{"error":"boom","code":"internal","retryable":true,"trace_id":"t-500"}"#
                        .to_owned()
                }
            }
            .into_bytes()
        })
        .create_async()
        .await;

    let results: Vec<_> = client(&server).v2().trades().pages().collect().await;

    assert_eq!(results.len(), 2);
    assert!(results[0].is_ok());
    assert!(matches!(&results[1], Err(DataApiError::V2(e)) if e.code == ErrorCode::Internal));
}

// ── Errors ───────────────────────────────────────────────────────────

async fn error_for(status: usize, body: &str, retry_after: Option<&str>) -> DataApiError {
    let mut server = Server::new_async().await;
    let mut mock = server
        .mock("GET", "/v2/user-stats")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body);
    if let Some(value) = retry_after {
        mock = mock.with_header("retry-after", value);
    }
    mock.create_async().await;
    client_without_retries(&server)
        .v2()
        .user_stats("0xuser")
        .send()
        .await
        .unwrap_err()
}

#[tokio::test]
async fn a_v2_error_body_keeps_every_field() {
    let err = error_for(
        400,
        r#"{"error":"'offset' is not a query param on this API: pages are cursor-only, pass 'cursor' from a prior response's next_cursor","code":"invalid_request","retryable":false,"trace_id":"d1673978a6c04d58ab57859d206c6da4"}"#,
        None,
    )
    .await;

    let DataApiError::V2(v2) = &err else {
        panic!("expected V2, got {err:?}");
    };
    assert_eq!(v2.status, 400);
    assert_eq!(v2.code, ErrorCode::InvalidRequest);
    assert!(v2.message.starts_with("'offset' is not a query param"));
    assert!(!v2.retryable);
    assert_eq!(v2.trace_id, "d1673978a6c04d58ab57859d206c6da4");
    assert_eq!(v2.parameter, None);
    assert_eq!(err.trace_id(), Some("d1673978a6c04d58ab57859d206c6da4"));
    assert!(!err.is_retriable());
}

#[tokio::test]
async fn a_v2_validation_error_names_its_parameter() {
    let err = error_for(
        400,
        r#"{"error":"required query param 'user' not provided","code":"invalid_request","parameter":"user","retryable":false,"trace_id":"8f8b7e5e64d241d1bc6e8d5eef76fc4e"}"#,
        None,
    )
    .await;

    assert!(matches!(&err, DataApiError::V2(e) if e.parameter.as_deref() == Some("user")));
}

#[tokio::test]
async fn a_v2_not_found_maps_its_code() {
    let err = error_for(
        404,
        r#"{"error":"Not Found","code":"not_found","retryable":false,"trace_id":"24f7ca3589464454a30a566c10d1216e"}"#,
        None,
    )
    .await;

    assert!(
        matches!(&err, DataApiError::V2(e) if e.code == ErrorCode::NotFound && e.status == 404)
    );
}

#[tokio::test]
async fn a_v1_error_body_stays_an_api_error() {
    let err = error_for(
        400,
        r#"{"error":"required query param 'user' not provided"}"#,
        None,
    )
    .await;

    assert!(
        matches!(&err, DataApiError::Api(ApiError::Validation(m)) if m == "required query param 'user' not provided"),
        "got {err:?}"
    );
    assert_eq!(err.trace_id(), None);
}

#[tokio::test]
async fn a_cloudflare_block_page_stays_a_rate_limit_error() {
    let err = error_for(429, "error code: 1015", None).await;

    assert!(matches!(&err, DataApiError::Api(ApiError::RateLimit(m)) if m == "error code: 1015"));
}

#[tokio::test]
async fn the_servers_retryable_flag_overrides_the_status_heuristic() {
    let body = |retryable: bool| {
        format!(
            r#"{{"error":"datastore unavailable","code":"dependency_unavailable","retryable":{retryable},"trace_id":"t-503"}}"#
        )
    };

    let refused = error_for(503, &body(false), None).await;
    let allowed = error_for(503, &body(true), None).await;

    // A bare 503 is retriable by status alone; the server said otherwise.
    assert!(ApiError::Api {
        status: 503,
        message: String::new()
    }
    .is_retriable());
    assert!(!refused.is_retriable());
    assert!(allowed.is_retriable());
}

#[tokio::test]
async fn a_v2_rate_limit_carries_retry_after() {
    let err = error_for(
        429,
        r#"{"error":"slow down","code":"rate_limited","retryable":true,"trace_id":"t-429"}"#,
        Some("7"),
    )
    .await;

    assert!(matches!(&err, DataApiError::V2(e) if e.code == ErrorCode::RateLimited));
    assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
    assert!(err.is_retriable());
}
