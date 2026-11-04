//! The send loop on the Data API's client: Cloudflare's 429 with a
//! `Retry-After` of zero, held for the whole client. `docs/MUTANTS.md` cites
//! both tests. The v2 routes' suites are `mock_data_v2.rs` beside it.

use mockito::{Matcher, Server};
use polyoxide_polymarket::data::v2::types::PositionStatus;
use polyoxide_polymarket::data::DataApi;

// ── Rate limit backpressure ──────────────────────────────────────────────────
//
// Reproduces the shape observed against the live Data API: Cloudflare answers a
// tripped limit with 429, `error code: 1015`, and a `Retry-After` that floors to
// zero. Two things went wrong there — the zero was obeyed verbatim, and the 429
// stayed private to the request that saw it while its siblings kept firing.

/// A server that rate-limits everything, exactly as Cloudflare does.
async fn rate_limited_server() -> mockito::ServerGuard {
    let mut server = Server::new_async().await;
    server
        .mock("GET", Matcher::Any)
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body("error code: 1015\n")
        .expect_at_least(1)
        .create_async()
        .await;
    server
}

fn throttled_data(server: &mockito::ServerGuard, backoff_ms: u64, max_retries: u32) -> DataApi {
    DataApi::builder()
        .base_url(server.url())
        .with_retry_config(polyoxide_core::RetryConfig {
            max_retries,
            initial_backoff_ms: backoff_ms,
            max_backoff_ms: 10_000,
        })
        .build()
        .unwrap()
}

#[tokio::test]
async fn retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop() {
    let server = rate_limited_server().await;
    // Two retries at a 400ms base: 300-500ms then 600-1000ms after jitter, so
    // at least 900ms of genuine waiting. The reported failure burned all three
    // attempts in 65ms.
    let data = throttled_data(&server, 400, 2);

    let start = std::time::Instant::now();
    let result = data
        .v2()
        .positions("0xaddr")
        .status(PositionStatus::Closed)
        .send()
        .await;
    let elapsed = start.elapsed();

    assert!(
        result.is_err(),
        "a 429-only server cannot produce a success"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(700),
        "the retries took {elapsed:?}; `Retry-After: 0` was taken literally \
         instead of falling back to the client's own backoff"
    );
}

#[tokio::test]
async fn a_429_makes_the_next_request_wait_even_though_it_never_saw_one() {
    let server = rate_limited_server().await;
    // No retries: this isolates the client-wide cooldown from the per-request
    // backoff, so the wait measured below can only come from the former.
    let data = throttled_data(&server, 600, 0);

    let rate_limited = data
        .v2()
        .positions("0xaddr")
        .status(PositionStatus::Closed)
        .send()
        .await;
    assert!(rate_limited.is_err());

    // A different endpoint, its own token bucket untouched. Before the cooldown
    // existed this went straight out on the wire, which is how four concurrent
    // callers turned one 429 into sixteen doomed requests.
    let start = std::time::Instant::now();
    let _ = data.v2().trades().send().await;
    let elapsed = start.elapsed();

    assert!(
        elapsed >= std::time::Duration::from_millis(400),
        "the next request went out after only {elapsed:?}; the 429 never became \
         backpressure for the rest of the client"
    );
}

#[tokio::test]
async fn the_builder_installs_polymarkets_retry_policy_so_a_425_is_retried() {
    // Core's own default policy does not retry a 425 (AD-17); only the builder's
    // `with_retry_policy(PolymarketRetryPolicy)` makes this succeed.
    let mut server = Server::new_async().await;
    let early = server
        .mock("GET", "/v2/positions")
        .match_query(Matcher::Any)
        .with_status(425)
        .expect(1)
        .create_async()
        .await;
    let ok = server
        .mock("GET", "/v2/positions")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"data":[],"pagination":{"limit":1,"offset":0,"has_more":false,"next_cursor":null}}"#,
        )
        .expect(1)
        .create_async()
        .await;

    let client = DataApi::builder()
        .base_url(server.url())
        .with_retry_config(polyoxide_core::RetryConfig {
            max_retries: 3,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
        })
        .build()
        .unwrap();
    client
        .v2()
        .positions("0xaddr")
        .status(PositionStatus::Closed)
        .send()
        .await
        .expect("a 425 is the matching engine restarting, retried to the success");
    early.assert_async().await;
    ok.assert_async().await;
}
