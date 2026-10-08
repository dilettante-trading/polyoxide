//! The crate's error type.

use std::time::Duration;

use polyoxide_venue::{class_for_close_code, class_for_handshake_status, Class, Classify};
use tokio_tungstenite::tungstenite;

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
        /// What the transport reported.
        #[source]
        source: Box<tungstenite::Error>,
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
        /// What the transport reported.
        #[source]
        source: Box<tungstenite::Error>,
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
    /// Whether a reconnect attempt that failed this way is worth repeating.
    ///
    /// The statuses are the ones polyoxide-core's `is_retriable` treats as
    /// passing: a timeout, the matching engine restarting, a rate limit, a
    /// server fault. polyoxide-perps retries only 429 and 5xx.
    pub(crate) fn retrying_can_fix(&self) -> bool {
        let Self::Connect { source } = self else {
            return true;
        };
        match &**source {
            tungstenite::Error::Http(response) => {
                let status = response.status().as_u16();
                matches!(status, 408 | 425 | 429) || status >= 500
            }
            // `Tls` is only an invalid server name here: rustls reports a
            // certificate it rejects as an IO error, which is retried.
            tungstenite::Error::Url(_)
            | tungstenite::Error::Tls(_)
            | tungstenite::Error::HttpFormat(_)
            | tungstenite::Error::AttackAttempt => false,
            _ => true,
        }
    }
}

/// A transport failure, by the socket table: a refused upgrade by its status,
/// misuse and a bad URL or TLS name an `InvalidRequest`, and everything else,
/// an I/O or protocol error or a closed connection, `Network`.
fn transport_class(err: &tungstenite::Error) -> Class {
    use tungstenite::Error as Ws;
    match err {
        Ws::Http(response) => class_for_handshake_status(response.status().as_u16()),
        Ws::Url(_)
        | Ws::HttpFormat(_)
        | Ws::Tls(_)
        | Ws::AttackAttempt
        | Ws::Capacity(_)
        | Ws::AlreadyClosed => Class::InvalidRequest,
        // Io (a TLS EOF included), Protocol, ConnectionClosed,
        // WriteBufferFull, Utf8 and any later variant.
        _ => Class::Network,
    }
}

/// The transport by the socket table, a server close by its close code, a
/// timeout or a silent feed `Network`, and a frame that did not parse a
/// `Decode`.
impl Classify for SportsError {
    fn class(&self) -> Class {
        match self {
            Self::Connect { source } | Self::Transport { source } => transport_class(source),
            Self::ConnectTimeout { .. } | Self::Stale { .. } => Class::Network,
            Self::Closed { code, .. } => class_for_close_code(*code),
            Self::Decode { .. } => Class::Decode,
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

    fn http_error(status: u16) -> SportsError {
        SportsError::Connect {
            source: Box::new(tungstenite::Error::Http(
                tungstenite::http::Response::builder()
                    .status(status)
                    .body(None)
                    .unwrap(),
            )),
        }
    }

    #[test]
    fn a_refused_request_is_not_retried_but_an_unwell_host_is() {
        for status in [301, 401, 403, 404] {
            assert!(!http_error(status).retrying_can_fix(), "{status}");
        }
        for status in [408, 425, 429, 500, 502, 503] {
            assert!(http_error(status).retrying_can_fix(), "{status}");
        }
    }

    #[test]
    fn a_transport_failure_or_timeout_is_retried() {
        let reset = SportsError::Connect {
            source: Box::new(tungstenite::Error::Io(std::io::Error::from(
                std::io::ErrorKind::ConnectionReset,
            ))),
        };
        assert!(reset.retrying_can_fix());
        let timeout = SportsError::ConnectTimeout {
            after: Duration::from_secs(10),
        };
        assert!(timeout.retrying_can_fix());
    }

    #[test]
    fn a_malformed_url_is_not_retried() {
        let url = SportsError::Connect {
            source: Box::new(tungstenite::Error::Url(
                tungstenite::error::UrlError::NoHostName,
            )),
        };
        assert!(!url.retrying_can_fix());
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
            let err = SportsError::Transport {
                source: Box::new(err),
            };
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
                    source: Box::new(tungstenite::Error::ConnectionClosed),
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
