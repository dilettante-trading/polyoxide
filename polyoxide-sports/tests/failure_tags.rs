//! The Rust twins of the nightly classifier's tag table, for the errors the
//! sports live suite fails with.
//!
//! Each case the classifier's regex-era tests held is now a row in
//! `.github/scripts/tests/test_classify_failures.py`: the tag a live test
//! prints, then the text the old table matched. A row naming an error the
//! tests can build points at a twin here, which builds that error, checks it
//! renders the row's text, and asserts the tag it fails a test with. The live
//! suite wraps a raw socket's close frame and transport error in the same
//! `SportsError` first, so these cover its raw-socket tests too.

use polyoxide_sports::SportsError;
use polyoxide_test_support::{tag_for, Tag};
use polyoxide_venue::Classify;
use tokio_tungstenite::tungstenite::{self, error::ProtocolError};

/// The tag `err` fails a test with, through `or_fail` or `fail`.
fn tag(err: &SportsError) -> Tag {
    tag_for(&err.class(), err.is_fault())
}

fn closed(code: u16, reason: &str) -> SportsError {
    SportsError::Closed {
        code: Some(code),
        reason: reason.to_owned(),
    }
}

#[test]
fn reset_without_closing_handshake() {
    let err = SportsError::Transport {
        source: Box::new(tungstenite::Error::Protocol(
            ProtocolError::ResetWithoutClosingHandshake,
        )),
    };
    assert_eq!(
        format!("{err:?}"),
        "Transport { source: Protocol(ResetWithoutClosingHandshake) }"
    );
    assert_eq!(
        err.to_string(),
        "the sports feed connection failed: WebSocket protocol error: Connection reset without \
         closing handshake"
    );
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn close_1001() {
    let err = closed(1001, "going away");
    assert_eq!(
        err.to_string(),
        "the sports feed closed the connection with code 1001: going away"
    );
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn close_1012() {
    assert_eq!(tag(&closed(1012, "restarting")), Tag::Transient);
}

#[test]
fn close_1013() {
    let err = closed(1013, "try again later");
    assert_eq!(
        format!("{err:?}"),
        r#"Closed { code: Some(1013), reason: "try again later" }"#
    );
    assert_eq!(tag(&err), Tag::Transient);
}

/// The socket table (AD-14) counts a normal close among the drops, so a
/// server closing mid-test with 1000 is retried. The regex-era table filed it.
#[test]
fn close_1000() {
    assert_eq!(tag(&closed(1000, "")), Tag::Transient);
}

#[test]
fn close_1008() {
    let err = closed(1008, "policy");
    assert_eq!(
        err.to_string(),
        "the sports feed closed the connection with code 1008: policy"
    );
    assert_eq!(tag(&err), Tag::Real);
}

#[test]
fn decode() {
    let raw = r#"{"leagueAbbreviation":"nba"}"#;
    let err = SportsError::Decode {
        raw: raw.to_owned(),
        source: serde_json::from_str::<u8>(raw).unwrap_err(),
    };
    assert!(
        err.to_string().starts_with("a sports frame did not parse"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Real);
}
