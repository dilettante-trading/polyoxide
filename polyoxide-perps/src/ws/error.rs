//! Errors of the WebSocket tiers and how each one recovers.

use std::time::Duration;

use thiserror::Error;

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
    /// Each entry is the channel name and the server's identifier.
    #[error("subscription refused: {}", refused.iter().map(|(c, e)| format!("{c} ({e})")).collect::<Vec<_>>().join(", "))]
    Refused {
        /// The refused channels with the server's error identifier.
        refused: Vec<(String, String)>,
    },

    /// The server's response to a control request was not the documented
    /// shape.
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
                Ws::Url(_)
                | Ws::Tls(_)
                | Ws::Http(_)
                | Ws::HttpFormat(_)
                | Ws::AlreadyClosed
                | Ws::AttackAttempt => Recovery::Fatal,
                // `tungstenite::Error` is non_exhaustive; an unknown transport
                // error costs one retry cycle if it turns out permanent.
                _ => Recovery::Reconnect,
            },
            Self::ConnectionClosed | Self::Stalled { .. } => Recovery::Reconnect,
            Self::Refused { refused } => {
                if refused.iter().all(|(_, e)| e == "message_rate_limited") {
                    Recovery::Retry
                } else {
                    Recovery::Fatal
                }
            }
            Self::Frame { .. } | Self::Unrecognised { .. } => Recovery::SkipFrame,
            Self::Response { .. } | Self::EmptySubscription | Self::Stopped => Recovery::Fatal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_subscription_lists_every_refusal() {
        let err = PerpsWsError::Refused {
            refused: vec![("nonsense::1".to_owned(), "invalid channel".to_owned())],
        };
        assert_eq!(err.recovery(), Recovery::Fatal);
        assert!(err.to_string().contains("nonsense::1"));
    }

    #[test]
    fn a_rate_limited_subscription_is_retriable_not_fatal() {
        let err = PerpsWsError::Refused {
            refused: vec![("bbo::1".to_owned(), "message_rate_limited".to_owned())],
        };
        assert_eq!(err.recovery(), Recovery::Retry);
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
}
