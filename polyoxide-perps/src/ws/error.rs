//! Errors of the WebSocket tiers, and their classes.

use std::time::Duration;

use polyoxide_venue::{Class, Classify};
use polyoxide_ws::WsError;
use thiserror::Error;

use crate::ws::channel::Channel;

polyoxide_venue::impl_ws_classification!();

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

/// Error type for the WebSocket tiers.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PerpsWsError {
    /// The connection could not be opened, or failed mid-stream: the kit's
    /// one socket error, read through its [`Kind`](polyoxide_ws::Kind).
    #[error("WebSocket transport error: {0}")]
    Connection(#[from] WsError),

    /// The server closed the connection, or the stream ended.
    #[error("WebSocket connection closed")]
    ConnectionClosed,

    /// No frame arrived within the configured staleness window, or the
    /// server left a membership change or a replay unanswered for it.
    #[error("no frame for {elapsed:?}")]
    Stalled {
        /// Time since the last frame: the silence, or how long the change or
        /// replay waited.
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

/// The server's identifier for a subscription refused by its message rate
/// limit.
pub(crate) const MESSAGE_RATE_LIMITED: &str = "message_rate_limited";

/// The reason the client records for a refusal that carried no `error` field.
/// It is not the venue's, so it is never reported as a code.
pub(crate) const NO_REASON: &str = "err";

/// The transport by the one socket table
/// (`polyoxide_venue::impl_ws_classification!`), whose only non-fault is an
/// upgrade refused with 451, a lost or silent connection `Network`,
/// a reply or frame that did not parse a `Decode`, and the client's own
/// refusals an `InvalidRequest`.
///
/// A refusal is `RateLimited` when every refused channel was refused for
/// `message_rate_limited`, the case a supervised subscribe retries (D10).
/// Otherwise it is a `VenueRefusal` whose code is the first identifier other
/// than `message_rate_limited`, the one that makes retrying pointless, and
/// `None` when every such refusal came without an identifier.
impl Classify for PerpsWsError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => ws_class(err),
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

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Connection(err) if !ws_is_fault(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InstrumentId;
    use polyoxide_ws::tungstenite::{self, http};

    fn refusal(channel: Channel, reason: &str) -> Refusal {
        Refusal {
            channel,
            reason: reason.to_owned(),
        }
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

    fn http_error(status: u16) -> PerpsWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> PerpsWsError {
        PerpsWsError::from(WsError::from(err))
    }

    #[test]
    fn only_a_451_refusal_is_not_a_fault() {
        assert!(!http_error(451).is_fault());
        assert_eq!(http_error(451).class(), Class::Restricted);
        assert!(http_error(418).is_fault());
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
            let err = transport(err);
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
