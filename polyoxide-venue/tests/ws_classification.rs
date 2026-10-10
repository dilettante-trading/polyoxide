//! The one socket table (AD-15, Story 4.3): every `Kind` of
//! `polyoxide_ws::WsError` through `impl_ws_classification!`, expanded here as
//! a venue crate expands it. This crate's only path to tungstenite is
//! `polyoxide_ws`'s re-export.

use std::{sync::Arc, time::Duration};

use polyoxide_venue::{class_for_handshake_status, Class};
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{
        self,
        error::{CapacityError, ProtocolError, TlsError, UrlError},
        http, Message,
    },
    Kind, WsError,
};

polyoxide_venue::impl_ws_classification!();

/// The status rule a venue passes to `retrying_can_fix`.
fn venue_rule(status: u16) -> bool {
    class_for_handshake_status(status).is_retriable()
}

fn refused(status: u16, retry_after: Option<&str>) -> WsError {
    let mut response = http::Response::builder().status(status);
    if let Some(value) = retry_after {
        response = response.header("Retry-After", value);
    }
    WsError::from(tungstenite::Error::Http(response.body(None).unwrap()))
}

fn refusal(code: &str) -> Class {
    Class::VenueRefusal {
        code: Some(Arc::from(code)),
    }
}

/// One error of every kind but `Closed` and `Refused`, with the kind it must
/// read as and its class.
fn every_other_kind() -> Vec<(WsError, Kind<'static>, Class)> {
    let after = Duration::from_secs(10);
    let bad_header = http::header::HeaderName::from_bytes(b"in valid").unwrap_err();
    let transport = |err| WsError::from(err);
    vec![
        (
            transport(tungstenite::Error::Io(
                std::io::ErrorKind::UnexpectedEof.into(),
            )),
            Kind::Io,
            Class::Network,
        ),
        (
            transport(tungstenite::Error::Protocol(
                ProtocolError::ResetWithoutClosingHandshake,
            )),
            Kind::Protocol,
            Class::Network,
        ),
        (
            transport(tungstenite::Error::ConnectionClosed),
            Kind::ConnectionClosed,
            Class::Network,
        ),
        (
            transport(tungstenite::Error::WriteBufferFull(Message::Close(None))),
            Kind::Other,
            Class::Network,
        ),
        (
            transport(tungstenite::Error::Utf8),
            Kind::Other,
            Class::Network,
        ),
        (
            WsError::connect_timeout(after),
            Kind::ConnectTimeout { after },
            Class::Network,
        ),
        (
            WsError::no_answer(after),
            Kind::NoAnswer { after },
            Class::Network,
        ),
        (
            transport(tungstenite::Error::Url(UrlError::NoHostName)),
            Kind::Url,
            Class::InvalidRequest,
        ),
        (
            transport(tungstenite::Error::HttpFormat(bad_header.into())),
            Kind::HttpFormat,
            Class::InvalidRequest,
        ),
        (
            transport(tungstenite::Error::Tls(TlsError::InvalidDnsName)),
            Kind::Tls,
            Class::InvalidRequest,
        ),
        (
            transport(tungstenite::Error::AttackAttempt),
            Kind::AttackAttempt,
            Class::InvalidRequest,
        ),
        (
            transport(tungstenite::Error::Capacity(CapacityError::TooManyHeaders)),
            Kind::Capacity,
            Class::InvalidRequest,
        ),
        (
            transport(tungstenite::Error::AlreadyClosed),
            Kind::AlreadyClosed,
            Class::InvalidRequest,
        ),
    ]
}

#[test]
fn every_kind_has_its_class_and_is_a_fault() {
    for (err, kind, class) in every_other_kind() {
        assert_eq!(err.kind(), kind, "{err:?}");
        assert_eq!(ws_class(&err), class, "{err:?}");
        assert!(ws_is_fault(&err), "{err:?}");
    }
}

#[test]
fn every_close_code_by_the_close_code_table() {
    let rows = [
        (None, Class::Network),
        (Some(1000), Class::Network),
        (Some(1001), Class::Network),
        (Some(1002), refusal("1002")),
        (Some(1003), refusal("1003")),
        (Some(1004), Class::Network),
        (Some(1005), Class::Network),
        (Some(1006), Class::Network),
        (Some(1007), refusal("1007")),
        (Some(1008), refusal("1008")),
        (Some(1009), refusal("1009")),
        (Some(1010), refusal("1010")),
        (Some(1011), Class::Network),
        (Some(1012), Class::Network),
        (Some(1013), Class::Network),
        (Some(4000), refusal("4000")),
        (Some(4999), refusal("4999")),
    ];
    for (code, class) in rows {
        let err = WsError::closed(code, "reason");
        assert_eq!(
            err.kind(),
            Kind::Closed {
                code,
                reason: "reason"
            }
        );
        assert_eq!(ws_class(&err), class, "close code {code:?}");
        assert!(ws_is_fault(&err), "close code {code:?}");
    }
}

#[test]
fn every_refused_status_follows_the_handshake_status_rule() {
    for status in 100..=599 {
        let err = refused(status, None);
        assert_eq!(
            err.kind(),
            Kind::Refused {
                status,
                retry_after: None
            }
        );
        assert_eq!(
            ws_class(&err),
            class_for_handshake_status(status),
            "status {status}"
        );
    }
    // The rows the matrix names.
    assert_eq!(
        ws_class(&refused(503, None)),
        Class::Unavailable { code: None }
    );
    assert_eq!(
        ws_class(&refused(404, None)),
        Class::VenueRefusal { code: None }
    );
    assert_eq!(ws_class(&refused(418, None)), Class::Restricted);
    assert_eq!(ws_class(&refused(451, None)), Class::Restricted);
    assert_eq!(ws_class(&refused(101, None)), Class::Decode);
}

#[test]
fn a_429_carries_its_retry_after_and_a_zero_carries_none() {
    let rows = [
        (Some("7"), Some(Duration::from_secs(7))),
        (Some(" 1.5 "), Some(Duration::from_millis(1500))),
        (Some("0"), None),
        (Some("-1"), None),
        (Some("Wed, 21 Oct 2026 07:28:00 GMT"), None),
        // No clamp: a socket error reports the wait the server asked for.
        (Some("1e20"), Some(Duration::MAX)),
        (None, None),
    ];
    for (header, wait) in rows {
        let class = ws_class(&refused(429, header));
        assert_eq!(
            class,
            Class::RateLimited { retry_after: wait },
            "{header:?}"
        );
        assert_eq!(class.retry_after(), wait, "{header:?}");
    }
    // Only a 429's class has room for a wait.
    assert_eq!(
        ws_class(&refused(503, Some("7"))),
        Class::Unavailable { code: None }
    );
    assert_eq!(ws_class(&refused(418, Some("120"))), Class::Restricted);
}

#[test]
fn only_an_upgrade_refused_with_451_is_not_a_fault() {
    for status in 100..=599 {
        assert_eq!(
            ws_is_fault(&refused(status, None)),
            status != 451,
            "status {status}"
        );
    }
}

#[test]
fn the_reconnect_rule_and_the_class_agree_on_every_refused_status() {
    // The status set has one definition: `retrying_can_fix` is given
    // venue's rule, and the class is read from venue's table.
    for status in 100..=599 {
        let err = refused(status, None);
        assert_eq!(
            retrying_can_fix(&err, venue_rule),
            ws_class(&err).is_retriable(),
            "status {status}"
        );
    }
    assert!(retrying_can_fix(&refused(503, None), venue_rule));
    assert!(retrying_can_fix(&refused(429, None), venue_rule));
    assert!(!retrying_can_fix(&refused(404, None), venue_rule));
    assert!(!retrying_can_fix(&refused(451, None), venue_rule));
}
