//! The one socket error (W6), and what kind of failure it is.

use std::{error::Error, fmt, time::Duration};

use tokio_tungstenite::tungstenite::{self, http::header::RETRY_AFTER, protocol::CloseFrame};

/// A WebSocket failure: a transport error, a refused upgrade, a server close,
/// or one of the kit's own timeouts.
///
/// Opaque on purpose: read it through [`kind`](Self::kind), which no later
/// tungstenite release can break, and classify it through
/// `polyoxide_venue::impl_ws_classification!` in the venue crate that wraps
/// it. Its `source` is the transport error it came from, when there is one.
///
/// Built from a `tungstenite::Error` by `From`, and by
/// [`connect_timeout`](Self::connect_timeout), [`closed`](Self::closed),
/// [`from_close_frame`](Self::from_close_frame) and
/// [`no_answer`](Self::no_answer) for the failures the kit detects itself.
///
/// The basis is the boxed `tungstenite::Error` each socket crate wraps:
/// `polyoxide-perps`' and `polyoxide-binance`'s `ws/error.rs`,
/// `polyoxide-rtds`' `error.rs` and `polyoxide-clob`'s `ws/error.rs`, with
/// `polyoxide-sports` boxing at its call sites. One divergence row stays with
/// each venue: **D19**, what to do after [`Kind::AlreadyClosed`] (perps and
/// rtds stop, Binance and sports reconnect).
pub struct WsError(Box<Repr>);

#[derive(Debug, thiserror::Error)]
enum Repr {
    #[error("WebSocket error: {0}")]
    Transport(#[source] tungstenite::Error),
    #[error("no WebSocket connection within {after:?}")]
    ConnectTimeout { after: Duration },
    #[error("the server closed the WebSocket connection{}", describe_close(*.code, .reason))]
    Closed { code: Option<u16>, reason: String },
    #[error("no answer within {after:?}")]
    NoAnswer { after: Duration },
}

/// What kind of failure a [`WsError`] is.
///
/// Matching on it is how a venue classifies, recovers or reports a socket
/// failure without naming tungstenite's own error. New kinds may be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind<'a> {
    /// The connection's I/O failed, a TLS EOF included.
    Io,
    /// The peer broke the WebSocket protocol, such as by resetting without a
    /// closing handshake.
    Protocol,
    /// The connection is closed, after a completed closing handshake.
    ConnectionClosed,
    /// The server closed the connection: with a close frame, whose code and
    /// reason these are, or by ending the stream with none, which reads as no
    /// code and an empty reason. A close frame that carries no code reads the
    /// same as no frame.
    Closed {
        /// The close code, when a close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: &'a str,
    },
    /// The server refused the WebSocket upgrade with an HTTP status.
    Refused {
        /// The status.
        status: u16,
        /// The refusal's `Retry-After` header as sent, when it had one that
        /// is valid text.
        retry_after: Option<&'a str>,
    },
    /// The URL is not a WebSocket URL, or names no host.
    Url,
    /// The upgrade request could not be built, such as from an invalid
    /// header.
    HttpFormat,
    /// The TLS configuration was refused, such as an invalid server name. A
    /// certificate the server presents and rustls rejects arrives as
    /// [`Io`](Self::Io).
    Tls,
    /// tungstenite detected an attack on the connection.
    AttackAttempt,
    /// A message or a header block was larger than the configured limits.
    Capacity,
    /// The connection was used after it closed.
    AlreadyClosed,
    /// The connection did not open within the connect timeout.
    ConnectTimeout {
        /// The timeout that elapsed.
        after: Duration,
    },
    /// No answer arrived within the answer timeout.
    NoAnswer {
        /// The timeout that elapsed.
        after: Duration,
    },
    /// Any other transport failure: a full write buffer, a frame that is not
    /// valid UTF-8, or a failure a later tungstenite adds.
    Other,
}

impl WsError {
    /// The connection did not open within `after`.
    pub fn connect_timeout(after: Duration) -> Self {
        Self(Box::new(Repr::ConnectTimeout { after }))
    }

    /// The server closed the connection with this code and reason; `None`
    /// and an empty reason for a close frame without a code, or an end of the
    /// stream with no close frame.
    pub fn closed(code: Option<u16>, reason: impl Into<String>) -> Self {
        Self(Box::new(Repr::Closed {
            code,
            reason: reason.into(),
        }))
    }

    /// The server closed the connection with this close frame, or with none.
    pub fn from_close_frame(frame: Option<&CloseFrame>) -> Self {
        match frame {
            Some(frame) => Self::closed(Some(u16::from(frame.code)), frame.reason.as_str()),
            None => Self::closed(None, String::new()),
        }
    }

    /// No answer arrived within `after`.
    pub fn no_answer(after: Duration) -> Self {
        Self(Box::new(Repr::NoAnswer { after }))
    }

    /// What kind of failure this is.
    pub fn kind(&self) -> Kind<'_> {
        use tungstenite::Error as Ws;
        match &*self.0 {
            Repr::Transport(err) => match err {
                Ws::Io(_) => Kind::Io,
                Ws::Protocol(_) => Kind::Protocol,
                Ws::ConnectionClosed => Kind::ConnectionClosed,
                Ws::Http(response) => Kind::Refused {
                    status: response.status().as_u16(),
                    retry_after: response
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|value| value.to_str().ok()),
                },
                Ws::Url(_) => Kind::Url,
                Ws::HttpFormat(_) => Kind::HttpFormat,
                Ws::Tls(_) => Kind::Tls,
                Ws::AttackAttempt => Kind::AttackAttempt,
                Ws::Capacity(_) => Kind::Capacity,
                Ws::AlreadyClosed => Kind::AlreadyClosed,
                // WriteBufferFull, Utf8, and any variant a later tungstenite
                // adds.
                _ => Kind::Other,
            },
            Repr::ConnectTimeout { after } => Kind::ConnectTimeout { after: *after },
            Repr::Closed { code, reason } => Kind::Closed {
                code: *code,
                reason,
            },
            Repr::NoAnswer { after } => Kind::NoAnswer { after: *after },
        }
    }
}

impl From<tungstenite::Error> for WsError {
    fn from(err: tungstenite::Error) -> Self {
        Self(Box::new(Repr::Transport(err)))
    }
}

impl fmt::Debug for WsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WsError").field(&self.0).finish()
    }
}

impl fmt::Display for WsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Error for WsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

/// The tail of a close's message: the code and reason when present.
fn describe_close(code: Option<u16>, reason: &str) -> String {
    match (code, reason.is_empty()) {
        (Some(code), true) => format!(" with code {code}"),
        (Some(code), false) => format!(" with code {code}: {reason}"),
        (None, true) => " without a status code".to_owned(),
        (None, false) => format!(": {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use tungstenite::{
        error::{CapacityError, ProtocolError, TlsError, UrlError},
        http,
        protocol::frame::coding::CloseCode,
        Message,
    };

    use super::*;

    fn refused(status: u16, retry_after: Option<&str>) -> WsError {
        let mut response = http::Response::builder().status(status);
        if let Some(value) = retry_after {
            response = response.header("Retry-After", value);
        }
        WsError::from(tungstenite::Error::Http(response.body(None).unwrap()))
    }

    #[test]
    fn every_transport_error_has_its_kind() {
        let bad_header = http::header::HeaderName::from_bytes(b"in valid").unwrap_err();
        let rows = [
            (
                tungstenite::Error::Io(std::io::ErrorKind::UnexpectedEof.into()),
                Kind::Io,
            ),
            (
                tungstenite::Error::Protocol(ProtocolError::ResetWithoutClosingHandshake),
                Kind::Protocol,
            ),
            (tungstenite::Error::ConnectionClosed, Kind::ConnectionClosed),
            (tungstenite::Error::Url(UrlError::NoHostName), Kind::Url),
            (
                tungstenite::Error::HttpFormat(bad_header.into()),
                Kind::HttpFormat,
            ),
            (tungstenite::Error::Tls(TlsError::InvalidDnsName), Kind::Tls),
            (tungstenite::Error::AttackAttempt, Kind::AttackAttempt),
            (
                tungstenite::Error::Capacity(CapacityError::TooManyHeaders),
                Kind::Capacity,
            ),
            (tungstenite::Error::AlreadyClosed, Kind::AlreadyClosed),
            (
                tungstenite::Error::WriteBufferFull(Message::Close(None)),
                Kind::Other,
            ),
            (tungstenite::Error::Utf8, Kind::Other),
        ];
        for (err, kind) in rows {
            let err = WsError::from(err);
            assert_eq!(err.kind(), kind, "{err:?}");
            assert!(err.source().is_some(), "{err:?} lost its transport error");
        }
    }

    #[test]
    fn a_refused_upgrade_keeps_its_status_and_raw_retry_after() {
        assert_eq!(
            refused(429, Some("7")).kind(),
            Kind::Refused {
                status: 429,
                retry_after: Some("7")
            }
        );
        assert_eq!(
            refused(503, None).kind(),
            Kind::Refused {
                status: 503,
                retry_after: None
            }
        );
        // Kept verbatim: parsing it, zero included, is the classifier's job.
        assert_eq!(
            refused(429, Some(" 0 ")).kind(),
            Kind::Refused {
                status: 429,
                retry_after: Some(" 0 ")
            }
        );
    }

    #[test]
    fn the_kits_own_failures_have_their_kinds_and_no_source() {
        let after = Duration::from_secs(10);
        let rows = [
            (
                WsError::connect_timeout(after),
                Kind::ConnectTimeout { after },
            ),
            (WsError::no_answer(after), Kind::NoAnswer { after }),
            (
                WsError::closed(Some(1008), "policy"),
                Kind::Closed {
                    code: Some(1008),
                    reason: "policy",
                },
            ),
        ];
        for (err, kind) in rows {
            assert_eq!(err.kind(), kind);
            assert!(err.source().is_none(), "{err:?}");
        }
    }

    #[test]
    fn a_close_frame_is_captured_with_its_code_and_reason() {
        let frame = CloseFrame {
            code: CloseCode::Error,
            reason: "overloaded".into(),
        };
        assert_eq!(
            WsError::from_close_frame(Some(&frame)).kind(),
            Kind::Closed {
                code: Some(1011),
                reason: "overloaded"
            }
        );
        assert_eq!(
            WsError::from_close_frame(None).kind(),
            Kind::Closed {
                code: None,
                reason: ""
            }
        );
    }

    #[test]
    fn messages_name_what_happened() {
        assert_eq!(
            WsError::closed(Some(1001), "going away").to_string(),
            "the server closed the WebSocket connection with code 1001: going away"
        );
        assert_eq!(
            WsError::closed(None, "").to_string(),
            "the server closed the WebSocket connection without a status code"
        );
        assert_eq!(
            WsError::connect_timeout(Duration::from_secs(10)).to_string(),
            "no WebSocket connection within 10s"
        );
        assert_eq!(
            WsError::no_answer(Duration::from_millis(200)).to_string(),
            "no answer within 200ms"
        );
        assert_eq!(
            refused(503, None).to_string(),
            "WebSocket error: HTTP error: 503 Service Unavailable"
        );
    }

    #[test]
    fn errors_cross_threads() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<WsError>();
    }
}
