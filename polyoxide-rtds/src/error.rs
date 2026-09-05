//! Errors produced by the RTDS client.

use std::time::Duration;

use thiserror::Error;

use crate::topic::Topic;

/// An error from the RTDS stream.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RtdsError {
    /// The underlying WebSocket transport failed.
    #[error("RTDS connection error: {0}")]
    Connection(Box<tokio_tungstenite::tungstenite::Error>),

    /// A frame could not be parsed.
    #[error("RTDS JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The connection closed.
    #[error("RTDS connection closed")]
    ConnectionClosed,

    /// The venue rejected the subscription.
    ///
    /// Arrives in an envelope of its own — `{"body":{"message":…},
    /// "statusCode":N}` — that shares no fields with a data frame. The status
    /// is not trustworthy: an unrecognised topic reports `401` for what the
    /// message body describes as a not-found.
    ///
    /// Never recoverable. One rejected topic zeroes every topic in the same
    /// batch, so retrying replays the same silence.
    #[error("RTDS rejected the subscription (status {status_code}): {message}")]
    Server {
        /// The `statusCode` field, reported verbatim.
        status_code: u16,
        /// The `body.message` field.
        message: String,
    },

    /// A price could not be represented as a [`Decimal`](rust_decimal::Decimal).
    #[error("RTDS value {raw} on topic {topic:?} does not fit a Decimal")]
    Precision {
        /// The undecoded wire value.
        raw: String,
        /// The topic it arrived on.
        topic: Topic,
    },

    /// No frame arrived on any subscription within the configured window.
    ///
    /// RTDS never answers a ping, so a half-open socket is indistinguishable
    /// from a quiet market except by timing the gap between updates.
    #[error("RTDS stream stalled: no frames for {elapsed:?}")]
    Stalled {
        /// How long the stream was silent.
        elapsed: Duration,
    },

    /// The endpoint URL could not be parsed.
    #[error("RTDS URL parse error: {0}")]
    Url(#[from] url::ParseError),
}

impl From<tokio_tungstenite::tungstenite::Error> for RtdsError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connection(Box::new(err))
    }
}

impl RtdsError {
    /// Whether reconnecting could plausibly succeed.
    ///
    /// This gates the supervisor's retry loop. Without it, a rejected
    /// subscription becomes an infinite reconnect at full backoff with no
    /// output — the same failure the error was meant to surface.
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Connection(_) | Self::ConnectionClosed | Self::Stalled { .. } => true,
            Self::Server { .. } | Self::Precision { .. } | Self::Json(_) | Self::Url(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rejected_subscription_is_never_recoverable() {
        // Reconnecting into a subscription the venue rejects replays the same
        // rejection forever, at full backoff, silently.
        let err = RtdsError::Server {
            status_code: 401,
            message: "topic: nope and type: update not found".into(),
        };
        assert!(!err.is_recoverable());
    }

    #[test]
    fn transport_failures_are_recoverable() {
        assert!(RtdsError::ConnectionClosed.is_recoverable());
        assert!(RtdsError::Stalled {
            elapsed: std::time::Duration::from_secs(30)
        }
        .is_recoverable());
    }

    #[test]
    fn decode_failures_are_not_recoverable() {
        // A value we cannot represent will not become representable on a
        // reconnect, and a malformed frame means our model is wrong.
        assert!(!RtdsError::Precision {
            raw: "1".repeat(40),
            topic: crate::Topic::ChainlinkSpot,
        }
        .is_recoverable());
    }

    #[test]
    fn server_errors_render_both_status_and_message() {
        let err = RtdsError::Server {
            status_code: 401,
            message: "not found".into(),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("401"), "{rendered}");
        assert!(rendered.contains("not found"), "{rendered}");
    }
}
