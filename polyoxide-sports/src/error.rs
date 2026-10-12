//! The crate's error type.

use std::time::Duration;

use polyoxide_venue::{class_for_close_code, Class, Classify};
use polyoxide_ws::{Kind, WsError};

polyoxide_venue::impl_ws_classification!();

/// Everything that can go wrong on the sports feed.
///
/// On the supervised stream a lost connection arrives as
/// [`Transport`](Self::Transport), [`Closed`](Self::Closed) or
/// [`Stale`](Self::Stale) inside
/// [`Event::Disconnected`](crate::Event::Disconnected), after the stream has
/// acted on it. Two errors reach the caller as an `Err`:
///
/// - [`Decode`](Self::Decode), for a frame the stream skipped. It carries on.
/// - [`Connect`](Self::Connect), for a reconnect refused in a way retrying
///   cannot fix: an HTTP status other than 408, 425, 429 or 5xx, or a
///   malformed URL. The stream ends after it.
///
/// Any other failed reconnect attempt is retried and logged at `WARN`
/// through `tracing`, not yielded. [`Connect`](Self::Connect) and
/// [`ConnectTimeout`](Self::ConnectTimeout) also come back from
/// [`SportsWsBuilder::connect`](crate::SportsWsBuilder::connect) when the
/// first connection fails, whatever the cause.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SportsError {
    /// The connection could not be opened: the URL, DNS, TCP, TLS or the
    /// WebSocket upgrade failed.
    #[error("could not connect to the sports feed: {source}")]
    Connect {
        /// What the transport reported: the kit's one socket error, read
        /// through its [`Kind`].
        #[source]
        source: WsError,
    },
    /// Opening the connection took longer than the connect timeout.
    #[error("connecting to the sports feed took longer than {after:?}")]
    ConnectTimeout {
        /// The timeout that elapsed.
        after: Duration,
    },
    /// The server closed the connection, or the stream ended.
    #[error("the sports feed closed the connection{}", describe_close(.code, .reason))]
    Closed {
        /// The close status code, when the server's close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A read failed on an open connection.
    #[error("the sports feed connection failed: {source}")]
    Transport {
        /// What the transport reported: the kit's one socket error, read
        /// through its [`Kind`].
        #[source]
        source: WsError,
    },
    /// Nothing arrived, protocol pings included, within the staleness limit.
    #[error("nothing received from the sports feed for {after:?}, pings included")]
    Stale {
        /// The staleness limit that elapsed.
        after: Duration,
    },
    /// A text frame did not parse as a match update.
    #[error("a sports frame did not parse: {source}")]
    Decode {
        /// The frame as received.
        raw: String,
        /// Why it did not parse.
        #[source]
        source: serde_json::Error,
    },
}

impl SportsError {
    /// A connect that failed. A timeout and a close keep their own variants,
    /// as they did before the kit, and anything else is
    /// [`Connect`](Self::Connect).
    pub(crate) fn connect(source: WsError) -> Self {
        Self::from_kind(source, |source| Self::Connect { source })
    }

    /// A read on an open connection that failed, mapped as
    /// [`connect`](Self::connect) maps a connect, and anything else
    /// [`Transport`](Self::Transport).
    pub(crate) fn transport(source: WsError) -> Self {
        Self::from_kind(source, |source| Self::Transport { source })
    }

    fn from_kind(source: WsError, otherwise: impl FnOnce(WsError) -> Self) -> Self {
        match source.kind() {
            Kind::ConnectTimeout { after } => Self::ConnectTimeout { after },
            Kind::Closed { code, reason } => Self::Closed {
                code,
                reason: reason.to_owned(),
            },
            _ => otherwise(source),
        }
    }
}

/// The transport by the one socket table
/// (`polyoxide_venue::impl_ws_classification!`), whose only non-fault is an
/// upgrade refused with 451 and whose wait is a refusal's `Retry-After`, a
/// server close by its close code, a timeout or a silent feed `Network`, and
/// a frame that did not parse a `Decode`.
impl Classify for SportsError {
    fn class(&self) -> Class {
        match self {
            Self::Connect { source } | Self::Transport { source } => ws_class(source),
            Self::ConnectTimeout { .. } | Self::Stale { .. } => Class::Network,
            Self::Closed { code, .. } => class_for_close_code(*code),
            Self::Decode { .. } => Class::Decode,
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(
            self,
            Self::Connect { source } | Self::Transport { source } if !ws_is_fault(source)
        )
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Connect { source } | Self::Transport { source } => ws_retry_after(source),
            _ => self.class().retry_after(),
        }
    }
}

/// The tail of the close message: the status code and reason when present.
fn describe_close(code: &Option<u16>, reason: &str) -> String {
    match (code, reason.is_empty()) {
        (Some(code), true) => format!(" with code {code}"),
        (Some(code), false) => format!(" with code {code}: {reason}"),
        (None, true) => " without a status code".to_owned(),
        (None, false) => format!(": {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_ws::tungstenite;

    use super::*;

    #[test]
    fn a_close_without_a_code_says_so() {
        let text = SportsError::Closed {
            code: None,
            reason: String::new(),
        }
        .to_string();
        assert_eq!(
            text,
            "the sports feed closed the connection without a status code"
        );
    }

    #[test]
    fn errors_cross_threads() {
        // The CLI turns this into an eyre::Report, which needs Send + Sync.
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SportsError>();
    }

    #[test]
    fn a_stale_error_says_pings_count() {
        let text = SportsError::Stale {
            after: Duration::from_secs(45),
        }
        .to_string();
        assert!(
            text.contains("45s") && text.contains("pings included"),
            "{text}"
        );
    }

    /// A transport failure as the connection reports it, through the kit.
    fn ws(err: tungstenite::Error) -> WsError {
        WsError::from(err)
    }

    /// An upgrade refused with `status` and these headers.
    fn refused(status: u16, headers: &[(&str, &str)]) -> SportsError {
        let mut response = tungstenite::http::Response::builder().status(status);
        for (name, value) in headers {
            response = response.header(*name, *value);
        }
        SportsError::Connect {
            source: ws(tungstenite::Error::Http(response.body(None).unwrap())),
        }
    }

    fn http_error(status: u16) -> SportsError {
        refused(status, &[])
    }

    #[test]
    fn a_451_refusal_is_restricted_and_not_a_fault() {
        // A region the feed does not serve answers as designed, as a 451 is
        // on every HTTP client in the workspace; a 418 ban stays a fault.
        let err = http_error(451);
        assert_eq!(err.class(), Class::Restricted);
        assert!(!err.is_fault(), "{err:?}");
        assert!(http_error(418).is_fault());
    }

    #[test]
    fn a_429_refusal_carries_its_retry_after() {
        let err = refused(429, &[("Retry-After", "30")]);
        assert_eq!(
            err.class(),
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(30))
            }
        );
        assert_eq!(err.retry_after(), Some(Duration::from_secs(30)));
    }

    #[test]
    fn a_503_refusal_surfaces_its_retry_after() {
        // As HTTP's errors do for every status; only a 429's class carries
        // the wait.
        let err = refused(503, &[("Retry-After", "30")]);
        assert_eq!(err.class(), Class::Unavailable { code: None });
        assert_eq!(err.retry_after(), Some(Duration::from_secs(30)));
    }

    #[test]
    fn a_timeout_and_a_close_keep_their_own_variants() {
        // M-D4: the kit's socket error maps onto sports' variants, so a
        // close's code and a timeout read as they did before the kit.
        let after = Duration::from_secs(10);
        assert!(matches!(
            SportsError::connect(WsError::connect_timeout(after)),
            SportsError::ConnectTimeout { after: timeout } if timeout == after
        ));
        let closed = SportsError::transport(WsError::closed(Some(1001), "going away"));
        assert!(
            matches!(&closed, SportsError::Closed { code: Some(1001), reason } if reason == "going away"),
            "{closed:?}"
        );
        assert!(matches!(
            SportsError::connect(ws(tungstenite::Error::ConnectionClosed)),
            SportsError::Connect { .. }
        ));
        assert!(matches!(
            SportsError::transport(ws(tungstenite::Error::ConnectionClosed)),
            SportsError::Transport { .. }
        ));
    }

    #[test]
    fn a_close_error_names_its_code_and_reason() {
        let text = SportsError::Closed {
            code: Some(1001),
            reason: "going away".into(),
        }
        .to_string();
        assert_eq!(
            text,
            "the sports feed closed the connection with code 1001: going away"
        );
    }

    #[test]
    fn the_transport_follows_the_socket_table() {
        let bad_header =
            tungstenite::http::header::HeaderName::from_bytes(b"in valid").unwrap_err();
        let rows = [
            (tungstenite::Error::ConnectionClosed, Class::Network),
            (
                tungstenite::Error::Io(std::io::ErrorKind::ConnectionReset.into()),
                Class::Network,
            ),
            (
                tungstenite::Error::Protocol(
                    tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                ),
                Class::Network,
            ),
            (
                tungstenite::Error::WriteBufferFull(tungstenite::Message::Close(None)),
                Class::Network,
            ),
            (tungstenite::Error::Utf8, Class::Network),
            (tungstenite::Error::AlreadyClosed, Class::InvalidRequest),
            (
                tungstenite::Error::Url(tungstenite::error::UrlError::NoHostName),
                Class::InvalidRequest,
            ),
            (
                tungstenite::Error::HttpFormat(bad_header.into()),
                Class::InvalidRequest,
            ),
            (
                tungstenite::Error::Tls(tungstenite::error::TlsError::InvalidDnsName),
                Class::InvalidRequest,
            ),
            (tungstenite::Error::AttackAttempt, Class::InvalidRequest),
            (
                tungstenite::Error::Capacity(tungstenite::error::CapacityError::TooManyHeaders),
                Class::InvalidRequest,
            ),
        ];
        for (err, class) in rows {
            let err = SportsError::Transport { source: ws(err) };
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
        }
        for (status, class) in [
            (301, Class::Decode),
            (401, Class::Unauthorized),
            (403, Class::Unauthorized),
            (404, Class::VenueRefusal { code: None }),
            (429, Class::RateLimited { retry_after: None }),
            (503, Class::Unavailable { code: None }),
        ] {
            assert_eq!(http_error(status).class(), class, "{status}");
        }
    }

    #[test]
    fn every_variant_classifies() {
        let rows = [
            (http_error(503), Class::Unavailable { code: None }),
            (
                SportsError::ConnectTimeout {
                    after: Duration::from_secs(10),
                },
                Class::Network,
            ),
            (
                SportsError::Closed {
                    code: Some(1001),
                    reason: "going away".into(),
                },
                Class::Network,
            ),
            (
                SportsError::Closed {
                    code: Some(1008),
                    reason: "policy".into(),
                },
                Class::VenueRefusal {
                    code: Some("1008".into()),
                },
            ),
            (
                SportsError::Closed {
                    code: None,
                    reason: String::new(),
                },
                Class::Network,
            ),
            (
                SportsError::Transport {
                    source: ws(tungstenite::Error::ConnectionClosed),
                },
                Class::Network,
            ),
            (
                SportsError::Stale {
                    after: Duration::from_secs(45),
                },
                Class::Network,
            ),
            (
                SportsError::Decode {
                    raw: "{".into(),
                    source: serde_json::from_str::<u8>("{").unwrap_err(),
                },
                Class::Decode,
            ),
        ];
        for (err, class) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
