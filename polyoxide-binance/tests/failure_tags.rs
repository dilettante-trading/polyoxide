//! The Rust twins of the nightly classifier's tag table, for the errors the
//! Binance live suites fail with.
//!
//! Each case the classifier's regex-era tests held is now a row in
//! `.github/scripts/tests/test_classify_failures.py`: the tag a live test
//! prints, then the text the old table matched. A row naming an error the
//! tests can build points at a twin here, which builds that error, checks it
//! renders the row's text, and asserts the tag it fails a test with. So the
//! row's tag is the one the live test really prints.

use std::time::Duration;

use polyoxide_binance::BinanceError;
use polyoxide_core::{ApiError, ErrorResponse};
use polyoxide_test_support::{tag_for, Tag};
use polyoxide_venue::Classify;

/// The tag `err` fails a test with, through `or_fail` or `fail`.
fn tag(err: &impl Classify) -> Tag {
    tag_for(&err.class(), err.is_fault())
}

fn venue(status: u16, code: i64, msg: &str) -> BinanceError {
    BinanceError::Venue {
        status,
        code,
        msg: msg.to_owned(),
    }
}

#[test]
fn rate_limited() {
    let err = BinanceError::RateLimited {
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
    assert_eq!(
        format!("{err:?}"),
        r#"Venue { status: 408, code: -1007, msg: "Timeout" }"#
    );
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
    let err = BinanceError::IpBanned { retry_after: None };
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
    let err = BinanceError::Forbidden {
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
    let err = BinanceError::RegionBlocked { msg: msg.into() };
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

/// `live_api`'s `raw()` fails a refused fetch with core's reading of its
/// status.
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

#[cfg(feature = "ws")]
mod streams {
    use std::time::Duration;

    use polyoxide_binance::usdm::ws::UsdmWsError;
    use polyoxide_test_support::Tag;
    use tokio_tungstenite::tungstenite::{self, http};

    use super::tag;

    fn handshake(status: u16) -> UsdmWsError {
        UsdmWsError::from(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    fn closed(code: u16, reason: &str) -> UsdmWsError {
        UsdmWsError::Closed {
            code: Some(code),
            reason: reason.to_owned(),
        }
    }

    #[test]
    fn connect_timeout() {
        let err = UsdmWsError::ConnectTimeout(Duration::from_secs(10));
        assert_eq!(format!("{err:?}"), "ConnectTimeout(10s)");
        assert_eq!(err.to_string(), "no connection within 10s");
        assert_eq!(tag(&err), Tag::Transient);
    }

    #[test]
    fn handshake_503() {
        let err = handshake(503);
        assert_eq!(
            err.to_string(),
            "WebSocket transport error: HTTP error: 503 Service Unavailable"
        );
        assert_eq!(tag(&err), Tag::Transient);
    }

    #[test]
    fn handshake_404() {
        let err = handshake(404);
        assert_eq!(
            err.to_string(),
            "WebSocket transport error: HTTP error: 404 Not Found"
        );
        assert_eq!(tag(&err), Tag::Real);
    }

    #[test]
    fn handshake_451() {
        let err = handshake(451);
        assert_eq!(
            err.to_string(),
            "WebSocket transport error: HTTP error: 451 Unavailable For Legal Reasons"
        );
        assert!(
            format!("{err:?}")
                .starts_with("Connect(Http(Response { status: 451, version: HTTP/1.1, headers: {}"),
            "{err:?}"
        );
        assert_eq!(tag(&err), Tag::Environmental);
    }

    /// Also what `polyoxide-cli`'s live suite fails with when the CLI's
    /// stderr reports this close as an outage marker.
    #[test]
    fn close_1011() {
        let err = closed(1011, "Internal error");
        assert_eq!(
            err.to_string(),
            "the server closed the connection (Some(1011): Internal error)"
        );
        assert_eq!(tag(&err), Tag::Transient);
    }

    /// Also what `polyoxide-cli`'s live suite fails with when the CLI's
    /// stderr reports this close as an outage marker.
    #[test]
    fn close_1008() {
        let err = closed(1008, "Invalid request");
        assert_eq!(
            err.to_string(),
            "the server closed the connection (Some(1008): Invalid request)"
        );
        assert_eq!(tag(&err), Tag::Real);
    }

    /// Wherever it is tagged. `live_ws`'s ping test keeps it untagged, since
    /// an unanswered ping is the fault that test is for.
    #[test]
    fn no_answer() {
        let err = UsdmWsError::NoAnswer {
            id: 1,
            timeout: Duration::from_secs(10),
        };
        assert_eq!(format!("{err:?}"), "NoAnswer { id: 1, timeout: 10s }");
        assert_eq!(err.to_string(), "no answer to request 1 within 10s");
        assert_eq!(tag(&err), Tag::Transient);
    }
}
