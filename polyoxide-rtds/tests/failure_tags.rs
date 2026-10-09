//! The Rust twins of the nightly classifier's tag table, for the errors the
//! RTDS live suite fails with.
//!
//! Each case the classifier's regex-era tests held is now a row in
//! `.github/scripts/tests/test_classify_failures.py`: the tag a live test
//! prints, then the text the old table matched. A row naming an error the
//! tests can build points at a twin here, which builds that error, checks it
//! renders the row's text, and asserts the tag it fails a test with.

use std::io;

use polyoxide_rtds::RtdsError;
use polyoxide_test_support::{tag_for, Tag};
use polyoxide_venue::Classify;
use tokio_tungstenite::tungstenite;

/// rustls's report of a peer that closed TCP without a TLS `close_notify`.
const UNEXPECTED_EOF: &str = "peer closed connection without sending TLS close_notify: \
                              https://docs.rs/rustls/latest/rustls/manual/_03_howto/index.html#unexpected-eof";

#[test]
fn tls_eof() {
    let err = RtdsError::Connection(Box::new(tungstenite::Error::Io(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        UNEXPECTED_EOF,
    ))));
    assert_eq!(
        err.to_string(),
        format!("RTDS connection error: IO error: {UNEXPECTED_EOF}")
    );
    assert_eq!(tag_for(&err.class(), err.is_fault()), Tag::Transient);
}
