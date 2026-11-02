//! The Rust twins of the nightly classifier's tag table, for the errors the
//! spot live suite fails with.
//!
//! Each builds an error, checks it renders the text the classifier reads, and
//! asserts the tag it fails a test with, so the tag a live test prints is the
//! one the table expects.

use std::time::Duration;

use polyoxide_binance::spot::SpotError;
use polyoxide_core::{ApiError, ErrorResponse};
use polyoxide_test_support::{tag_for, Tag};
use polyoxide_venue::Classify;

/// The tag `err` fails a test with, through `or_fail` or `fail`.
fn tag(err: &impl Classify) -> Tag {
    tag_for(&err.class(), err.is_fault())
}

fn venue(status: u16, code: i64, msg: &str) -> SpotError {
    SpotError::Venue {
        status,
        code,
        msg: msg.to_owned(),
    }
}

#[test]
fn rate_limited() {
    let err = SpotError::RateLimited {
        retry_after: Some(Duration::from_secs(1)),
    };
    assert_eq!(format!("{err:?}"), "RateLimited { retry_after: Some(1s) }");
    assert_eq!(
        err.to_string(),
        "binance rate limit (429), retry after Some(1s)"
    );
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn venue_5xx() {
    let err = venue(503, -1001, "Internal error");
    assert_eq!(
        format!("{err:?}"),
        r#"Venue { status: 503, code: -1001, msg: "Internal error" }"#
    );
    assert_eq!(
        err.to_string(),
        "binance answered 503: -1001 Internal error"
    );
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn venue_408() {
    let err = venue(408, -1007, "Timeout");
    assert_eq!(err.to_string(), "binance answered 408: -1007 Timeout");
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn venue_4xx() {
    let err = venue(400, -1121, "Invalid symbol.");
    assert_eq!(
        format!("{err:?}"),
        r#"Venue { status: 400, code: -1121, msg: "Invalid symbol." }"#
    );
    assert_eq!(
        err.to_string(),
        "binance answered 400: -1121 Invalid symbol."
    );
    assert_eq!(tag(&err), Tag::Real);
}

/// A ban is the client's own doing, so it files (amendment A2-1).
#[test]
fn ip_banned() {
    let err = SpotError::IpBanned { retry_after: None };
    assert_eq!(format!("{err:?}"), "IpBanned { retry_after: None }");
    assert_eq!(
        err.to_string(),
        "binance has banned this IP (418), retry after None"
    );
    assert_eq!(tag(&err), Tag::Real);
}

/// The firewall's refusal is about how the client behaved, so it files
/// (amendment A2-1).
#[test]
fn forbidden() {
    let err = SpotError::Forbidden {
        msg: "<html>".into(),
    };
    assert_eq!(
        err.to_string(),
        "binance's firewall refused the request (403): <html>"
    );
    assert_eq!(tag(&err), Tag::Real);
}

#[test]
fn region_blocked() {
    let msg = "Service unavailable from a restricted location";
    let err = SpotError::RegionBlocked { msg: msg.into() };
    assert_eq!(
        format!("{err:?}"),
        format!("RegionBlocked {{ msg: {msg:?} }}")
    );
    assert_eq!(
        err.to_string(),
        format!("binance does not serve this location (451): {msg}")
    );
    assert_eq!(tag(&err), Tag::Environmental);
}

/// A live suite's raw fetch fails a refused request with core's reading of
/// its status.
fn raw(status: u16, body: &str) -> ApiError {
    ErrorResponse::new(
        polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),
        Default::default(),
        body,
    )
    .into()
}

#[test]
fn raw_status_503() {
    let err = raw(503, "Service Unavailable");
    assert_eq!(err.to_string(), "API error: 503 - Service Unavailable");
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn raw_status_451() {
    assert_eq!(
        tag(&raw(451, "Unavailable For Legal Reasons")),
        Tag::Environmental
    );
}

#[test]
fn raw_status_403() {
    assert_eq!(tag(&raw(403, "Forbidden")), Tag::Real);
}
