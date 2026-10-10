//! The Rust twins of the nightly classifier's tag table, for polyoxide-core's
//! `ApiError`.
//!
//! The classifier once sorted failures by matching their panic text. Each case
//! its tests held is now a row in `.github/scripts/tests/test_classify_failures.py`:
//! the tag a live test prints, then the text the old table matched. A row that
//! names an error the tests can build points at a twin here, or in the
//! `tests/failure_tags.rs` of the crate that owns the error, which builds that
//! error, checks it renders the row's text, and asserts the tag it fails with.
//! So the row's tag is the one the live test really prints.

use std::time::Duration;

use polyoxide_core::{ApiError, ErrorResponse};
use polyoxide_test_support::{tag_for, Tag};
use polyoxide_venue::Classify;
use reqwest::StatusCode;
use tokio::net::TcpListener;

/// The tag `err` fails a test with, through `or_fail` or `fail`.
fn tag(err: &ApiError) -> Tag {
    tag_for(&err.class(), err.is_fault())
}

/// A response with `status` and the body `{"error": message}`, as core reads
/// one since Story 3.11 replaced the variants each status once built.
fn api(status: u16, message: &str) -> ApiError {
    ErrorResponse::new(
        StatusCode::from_u16(status).unwrap(),
        Default::default(),
        serde_json::json!({ "error": message }).to_string(),
    )
    .into()
}

#[test]
fn api_5xx() {
    let err = api(503, "bad gateway");
    assert!(
        format!("{err:?}").starts_with("Response(ErrorResponse { status: 503,"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "API error: 503 - bad gateway");
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn api_425() {
    let err = api(425, "too early");
    assert!(
        format!("{err:?}").starts_with("Response(ErrorResponse { status: 425,"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "API error: 425 - too early");
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn api_4xx() {
    let err = api(404, "not found");
    assert!(
        format!("{err:?}").starts_with("Response(ErrorResponse { status: 404,"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "API error: 404 - not found");
    assert_eq!(tag(&err), Tag::Real);
}

/// Was `ApiError::RateLimit`, built from a 429.
#[test]
fn rate_limit() {
    let err = api(429, "slow down");
    assert_eq!(err.to_string(), "API error: 429 - slow down");
    assert_eq!(tag(&err), Tag::Transient);
}

/// Was `ApiError::Timeout`, built from a 408.
#[test]
fn timeout() {
    let err = api(408, "request timeout");
    assert_eq!(err.to_string(), "API error: 408 - request timeout");
    assert_eq!(tag(&err), Tag::Transient);
}

/// A server 400, which `ApiError::Validation` carried until Story 3.11, and
/// a local refusal, which it still does: both file.
#[test]
fn validation() {
    let err = ApiError::Validation("required query param 'market' not provided".into());
    assert_eq!(
        format!("{err:?}"),
        r#"Validation("required query param 'market' not provided")"#
    );
    assert_eq!(
        ApiError::Validation("bad request".into()).to_string(),
        "Validation error: bad request"
    );
    assert_eq!(tag(&err), Tag::Real);

    let server = api(400, "required query param 'market' not provided");
    assert_eq!(
        server.to_string(),
        "API error: 400 - required query param 'market' not provided"
    );
    assert_eq!(tag(&server), Tag::Real);
}

/// Was `ApiError::Authentication`, built from a 401 or a 403.
#[test]
fn authentication() {
    for status in [401, 403] {
        let err = api(status, "invalid signature");
        assert_eq!(
            err.to_string(),
            format!("API error: {status} - invalid signature")
        );
        assert_eq!(tag(&err), Tag::Real);
    }
}

#[test]
fn serialization() {
    let source = serde_json::from_str::<u8>("\"x\"").unwrap_err();
    let err = ApiError::Serialization(source);
    assert!(
        err.to_string()
            .starts_with("Serialization error: invalid type"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Real);
}

/// A request to a listener that accepts and never answers, with a timeout
/// shorter than the wait: reqwest's timeout, whose Debug names `TimedOut`.
#[tokio::test]
async fn network_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/ok", listener.local_addr().unwrap());
    let _held = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(socket);
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let source = client.get(&url).send().await.unwrap_err();
    assert!(source.is_timeout(), "{source:?}");
    let err = ApiError::Network(source);
    assert!(format!("{err:?}").contains("TimedOut"), "{err:?}");
    assert!(
        err.to_string().contains("error sending request for url"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Transient);
}

/// A request to a port nothing listens on: reqwest's connect failure, whose
/// Debug names hyper-util's `Connect` kind.
#[tokio::test]
async fn network_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/ok", listener.local_addr().unwrap());
    drop(listener);
    let source = reqwest::get(&url).await.unwrap_err();
    assert!(source.is_connect(), "{source:?}");
    let err = ApiError::Network(source);
    assert!(
        format!("{err:?}").contains("hyper_util::client::legacy::Error(Connect"),
        "{err:?}"
    );
    assert!(
        err.to_string().contains("error sending request for url"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Transient);
}
