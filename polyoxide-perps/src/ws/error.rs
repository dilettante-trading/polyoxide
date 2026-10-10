//! Errors of the WebSocket tiers and how each one recovers.

use std::time::Duration;

use polyoxide_venue::{class_for_handshake_status, Class, Classify};
use thiserror::Error;
use tokio_tungstenite::tungstenite;

use crate::ws::channel::Channel;

/// One channel the server refused, with the identifier it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Refusal {
    /// The refused channel.
    pub channel: Channel,
    /// The server's error identifier, such as `invalid channel` or
    /// `message_rate_limited`.
    pub reason: String,
}

/// What to do about an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Recovery {
    /// Reconnect and resubscribe.
    Reconnect,
    /// Resend the same request after a backoff; the socket is fine.
    Retry,
    /// Skip this frame and keep reading.
    SkipFrame,
    /// Give up: retrying replays the same failure.
    Fatal,
}

/// Error type for the WebSocket tiers.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PerpsWsError {
    /// The connection could not be opened, or failed mid-stream.
    #[error("WebSocket transport error: {0}")]
    Connection(#[from] Box<tokio_tungstenite::tungstenite::Error>),

    /// The server closed the connection, or the stream ended.
    #[error("WebSocket connection closed")]
    ConnectionClosed,

    /// No frame arrived within the configured staleness window.
    #[error("no frame for {elapsed:?}")]
    Stalled {
        /// Time since the last frame.
        elapsed: Duration,
    },

    /// A subscribe or unsubscribe was refused for at least one channel.
    #[error("subscription refused: {}", refused.iter().map(|r| format!("{} ({})", r.channel, r.reason)).collect::<Vec<_>>().join(", "))]
    Refused {
        /// The refused channels with the server's error identifier.
        refused: Vec<Refusal>,
    },

    /// The server's response to a control request was not the documented
    /// shape, or a pong was not `ok`. The supervised tier replaces the
    /// connection when its own ping gets one; a membership call that gets
    /// one is reported to the caller and the connection is kept.
    #[error("malformed response to request {id}: {raw}")]
    Response {
        /// The request id the response answered.
        id: u64,
        /// The frame text.
        raw: String,
    },

    /// A push frame's payload did not decode as its channel's type.
    #[error("frame on {channel} did not decode: {source}")]
    Frame {
        /// The frame's `ch` label.
        channel: String,
        /// The frame text.
        raw: String,
        /// The decode error.
        #[source]
        source: serde_json::Error,
    },

    /// A text frame that was neither a push nor a response.
    #[error("unrecognised frame: {raw}")]
    Unrecognised {
        /// The frame text.
        raw: String,
    },

    /// `connect` was called with no channels.
    #[error("a connection needs at least one channel")]
    EmptySubscription,

    /// The supervised task has stopped; the handle can no longer be used.
    #[error("the supervised connection has stopped")]
    Stopped,
}

impl From<tokio_tungstenite::tungstenite::Error> for PerpsWsError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connection(Box::new(err))
    }
}

impl PerpsWsError {
    /// How the supervised tier treats this error.
    pub fn recovery(&self) -> Recovery {
        use tokio_tungstenite::tungstenite::Error as Ws;
        match self {
            Self::Connection(err) => match &**err {
                // A handshake refused with 5xx or 429 is the host being
                // unwell or throttling; anything else is our request.
                Ws::Http(response) => {
                    let status = response.status();
                    if status.is_server_error() || status.as_u16() == 429 {
                        Recovery::Reconnect
                    } else {
                        Recovery::Fatal
                    }
                }
                Ws::Url(_)
                | Ws::Tls(_)
                | Ws::HttpFormat(_)
                | Ws::AlreadyClosed
                | Ws::AttackAttempt => Recovery::Fatal,
                // `tungstenite::Error` is non_exhaustive; an unknown transport
                // error costs one retry cycle if it turns out permanent.
                _ => Recovery::Reconnect,
            },
            Self::ConnectionClosed | Self::Stalled { .. } => Recovery::Reconnect,
            Self::Refused { refused } => {
                if refused.iter().all(|r| r.reason == "message_rate_limited") {
                    Recovery::Retry
                } else {
                    Recovery::Fatal
                }
            }
            Self::Frame { .. } | Self::Unrecognised { .. } => Recovery::SkipFrame,
            // A control reply that is malformed or not `ok` means this
            // connection's control channel is unreliable; a fresh one is the
            // fix. Fatal would end the supervised stream on one odd pong.
            Self::Response { .. } => Recovery::Reconnect,
            Self::EmptySubscription | Self::Stopped => Recovery::Fatal,
        }
    }
}

/// The server's identifier for a subscription refused by its message rate
/// limit.
const MESSAGE_RATE_LIMITED: &str = "message_rate_limited";

/// The reason the client records for a refusal that carried no `error` field.
/// It is not the venue's, so it is never reported as a code.
pub(crate) const NO_REASON: &str = "err";

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

/// The transport by the socket table, a lost or silent connection `Network`,
/// a reply or frame that did not parse a `Decode`, and the client's own
/// refusals an `InvalidRequest`.
///
/// A refusal is `RateLimited` when every refused channel was refused for
/// `message_rate_limited`, the case [`PerpsWsError::recovery`] retries.
/// Otherwise it is a `VenueRefusal` whose code is the first identifier other
/// than `message_rate_limited`, the one that makes retrying pointless, and
/// `None` when every such refusal came without an identifier.
impl Classify for PerpsWsError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => transport_class(err),
            Self::ConnectionClosed | Self::Stalled { .. } => Class::Network,
            Self::Refused { refused } => {
                if refused.iter().all(|r| r.reason == MESSAGE_RATE_LIMITED) {
                    Class::RateLimited { retry_after: None }
                } else {
                    Class::VenueRefusal {
                        code: refused
                            .iter()
                            .map(|r| r.reason.as_str())
                            .find(|reason| ![MESSAGE_RATE_LIMITED, NO_REASON].contains(reason))
                            .map(Into::into),
                    }
                }
            }
            Self::Response { .. } | Self::Frame { .. } | Self::Unrecognised { .. } => Class::Decode,
            Self::EmptySubscription | Self::Stopped => Class::InvalidRequest,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InstrumentId;
    use tokio_tungstenite::tungstenite::{self, http};

    fn refusal(channel: Channel, reason: &str) -> Refusal {
        Refusal {
            channel,
            reason: reason.to_owned(),
        }
    }

    #[test]
    fn a_refused_subscription_lists_every_refusal() {
        let err = PerpsWsError::Refused {
            refused: vec![refusal(Channel::Bbo(InstrumentId(1)), "invalid channel")],
        };
        assert_eq!(err.recovery(), Recovery::Fatal);
        assert!(
            err.to_string().contains("bbo::1 (invalid channel)"),
            "{err}"
        );
    }

    #[test]
    fn two_refusals_are_both_listed() {
        let err = PerpsWsError::Refused {
            refused: vec![
                refusal(Channel::Bbo(InstrumentId(1)), "invalid channel"),
                refusal(Channel::Trades(InstrumentId(2)), "message_rate_limited"),
            ],
        };
        let text = err.to_string();
        assert!(text.contains("bbo::1 (invalid channel)"), "{text}");
        assert!(text.contains("trades::2 (message_rate_limited)"), "{text}");
    }

    #[test]
    fn a_rate_limited_subscription_is_retriable_not_fatal() {
        let err = PerpsWsError::Refused {
            refused: vec![refusal(
                Channel::Bbo(InstrumentId(1)),
                "message_rate_limited",
            )],
        };
        assert_eq!(err.recovery(), Recovery::Retry);
    }

    #[test]
    fn a_mixed_refusal_is_fatal() {
        // Retrying would replay the invalid channel, so the rate limit does
        // not rescue the batch.
        let err = PerpsWsError::Refused {
            refused: vec![
                refusal(Channel::Bbo(InstrumentId(1)), "message_rate_limited"),
                refusal(Channel::Bbo(InstrumentId(2)), "invalid channel"),
            ],
        };
        assert_eq!(err.recovery(), Recovery::Fatal);
    }

    fn http_error(status: u16) -> PerpsWsError {
        PerpsWsError::from(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_but_a_refused_request_is_fatal() {
        assert_eq!(http_error(503).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(429).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(401).recovery(), Recovery::Fatal);
    }

    #[test]
    fn a_transport_close_reconnects() {
        let err = PerpsWsError::from(tungstenite::Error::ConnectionClosed);
        assert!(matches!(err, PerpsWsError::Connection(_)));
        assert_eq!(err.recovery(), Recovery::Reconnect);
    }

    #[test]
    fn transport_loss_reconnects_and_bad_frames_are_skipped() {
        assert_eq!(
            PerpsWsError::ConnectionClosed.recovery(),
            Recovery::Reconnect
        );
        assert_eq!(
            PerpsWsError::Stalled {
                elapsed: Duration::from_secs(9)
            }
            .recovery(),
            Recovery::Reconnect
        );
        let bad = PerpsWsError::Frame {
            channel: "bbo::1".to_owned(),
            raw: "{}".to_owned(),
            source: serde_json::from_str::<u8>("x").unwrap_err(),
        };
        assert_eq!(bad.recovery(), Recovery::SkipFrame);
        assert_eq!(PerpsWsError::EmptySubscription.recovery(), Recovery::Fatal);
    }

    #[test]
    fn a_faulted_control_reply_reconnects_rather_than_ending_the_stream() {
        let odd = PerpsWsError::Response {
            id: 2,
            raw: r#"{"status":"err","error":"x"}"#.to_owned(),
        };
        assert_eq!(odd.recovery(), Recovery::Reconnect);
    }

    #[test]
    fn the_transport_follows_the_socket_table() {
        let bad_header = http::header::HeaderName::from_bytes(b"in valid").unwrap_err();
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
            let err = PerpsWsError::from(err);
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
        }
        for (status, class) in [
            (401, Class::Unauthorized),
            (404, Class::VenueRefusal { code: None }),
            (429, Class::RateLimited { retry_after: None }),
            (503, Class::Unavailable { code: None }),
            (200, Class::Decode),
        ] {
            assert_eq!(http_error(status).class(), class, "{status}");
        }
    }

    #[test]
    fn every_variant_classifies() {
        let rate_limited = refusal(Channel::Bbo(InstrumentId(1)), "message_rate_limited");
        let invalid = refusal(Channel::Bbo(InstrumentId(2)), "invalid channel");
        let unexplained = refusal(Channel::Bbo(InstrumentId(3)), NO_REASON);
        let rows = [
            (http_error(503), Class::Unavailable { code: None }),
            (PerpsWsError::ConnectionClosed, Class::Network),
            (
                PerpsWsError::Stalled {
                    elapsed: Duration::from_secs(9),
                },
                Class::Network,
            ),
            (
                PerpsWsError::Refused {
                    refused: vec![rate_limited.clone(), rate_limited.clone()],
                },
                Class::RateLimited { retry_after: None },
            ),
            (
                PerpsWsError::Refused {
                    refused: vec![rate_limited, invalid.clone()],
                },
                Class::VenueRefusal {
                    code: Some("invalid channel".into()),
                },
            ),
            (
                PerpsWsError::Refused {
                    refused: vec![invalid.clone()],
                },
                Class::VenueRefusal {
                    code: Some("invalid channel".into()),
                },
            ),
            // A refusal without an `error` field has no code of the venue's.
            (
                PerpsWsError::Refused {
                    refused: vec![unexplained.clone()],
                },
                Class::VenueRefusal { code: None },
            ),
            (
                PerpsWsError::Refused {
                    refused: vec![unexplained, invalid],
                },
                Class::VenueRefusal {
                    code: Some("invalid channel".into()),
                },
            ),
            (
                PerpsWsError::Response {
                    id: 2,
                    raw: "{}".to_owned(),
                },
                Class::Decode,
            ),
            (
                PerpsWsError::Frame {
                    channel: "bbo::1".to_owned(),
                    raw: "{}".to_owned(),
                    source: serde_json::from_str::<u8>("x").unwrap_err(),
                },
                Class::Decode,
            ),
            (
                PerpsWsError::Unrecognised {
                    raw: "{}".to_owned(),
                },
                Class::Decode,
            ),
            (PerpsWsError::EmptySubscription, Class::InvalidRequest),
            (PerpsWsError::Stopped, Class::InvalidRequest),
        ];
        for (err, class) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
