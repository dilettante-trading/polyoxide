//! Mock-server tests: one per builder, checking the path, the query keys the
//! builder sends, and the decoding of a representative body.

use mockito::{Matcher, Server, ServerGuard};
use polyoxide_perps::{Perps, PerpsError};

fn test_perps(server: &ServerGuard) -> Perps {
    Perps::builder().base_url(server.url()).build().unwrap()
}

// ── health ──────────────────────────────────────────────────────

#[tokio::test]
async fn ping_reports_latency_when_the_host_says_ok() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/ping")
        .with_status(200)
        .with_body(r#"{"status":"ok"}"#)
        .create_async()
        .await;

    let latency = test_perps(&server).health().ping().await.expect("ping");
    mock.assert_async().await;
    assert!(latency < std::time::Duration::from_secs(5));
}

#[tokio::test]
async fn time_returns_the_server_clock() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/time")
        .with_status(200)
        .with_body(r#"{"time":1790758431064}"#)
        .create_async()
        .await;

    let time = test_perps(&server)
        .health()
        .time()
        .send()
        .await
        .expect("time");
    mock.assert_async().await;
    assert_eq!(time.time, 1790758431064);
}

#[tokio::test]
async fn a_venue_error_body_maps_to_perps_error_venue() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/info/time")
        .with_status(404)
        .with_body(r#"{"status":"err","error":"not_found"}"#)
        .create_async()
        .await;

    let err = test_perps(&server)
        .health()
        .time()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(&err, PerpsError::Venue(v) if v.code == "not_found" && v.status == 404));
}

#[tokio::test]
async fn a_non_venue_error_body_stays_an_api_error() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/info/time")
        .with_status(502)
        .with_body("<html>bad gateway</html>")
        .create_async()
        .await;

    let err = test_perps(&server)
        .health()
        .time()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, PerpsError::Api(_)));
}
