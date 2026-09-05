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
    Connection(#[source] Box<tokio_tungstenite::tungstenite::Error>),

    /// A frame could not be parsed.
    ///
    /// Retains the text that failed. When the failure came from parsing the
    /// whole frame, `serde_json` also reports a line and column into bytes you
    /// would otherwise no longer have. When it came from interpreting an
    /// already-parsed payload, there is no byte stream to point into — the
    /// position reads `0:0` and `raw` is a re-serialisation of the payload,
    /// not the original wire bytes.
    #[error("RTDS could not parse a frame: {source}; frame was: {raw}")]
    Json {
        /// The frame text as received.
        raw: String,
        /// The underlying parse failure.
        #[source]
        source: serde_json::Error,
    },

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

    /// A price could not be decoded.
    ///
    /// Three different failures share this variant: a value that is not a
    /// valid E18 integer, one whose magnitude exceeds what a
    /// [`Decimal`](rust_decimal::Decimal) can hold, and a plain-decimal string
    /// that will not parse. The first is the interesting one — it usually
    /// means a payload arrived on a topic whose scale this crate got wrong.
    #[error("RTDS could not decode value {raw} on topic {topic:?}")]
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

/// What a caller should do about an [`RtdsError`].
///
/// A single boolean cannot express this. "Reconnect the transport" and "give
/// up entirely" are different from "this one frame was bad, the connection is
/// fine" — and collapsing the third into the second lets one unparseable
/// frame kill a feed that is otherwise healthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Recovery {
    /// Reconnect and resubscribe. The connection is gone but the request is
    /// still valid.
    Reconnect,
    /// Skip this frame and keep the connection. Something about one message
    /// was wrong; the transport is healthy.
    SkipFrame,
    /// Give up. Retrying replays the same failure.
    Fatal,
}

impl RtdsError {
    /// Build a [`RtdsError::Json`] retaining the frame that failed to parse.
    pub fn json(raw: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Json {
            raw: raw.into(),
            source,
        }
    }

    /// What a caller should do about this error.
    ///
    /// This gates the supervisor's retry loop. Written without a wildcard arm
    /// so a new variant fails to compile until it is classified — a default
    /// would be a guess, and each of the three outcomes is wrong for some
    /// variant.
    pub fn recovery(&self) -> Recovery {
        match self {
            // Not every transport failure is transient. A bad URL, a failed
            // TLS handshake, a rejected upgrade, a protocol-violation
            // detection and a use-after-close are all permanent, and
            // reconnecting into them spins at full backoff forever.
            Self::Connection(err) => match &**err {
                tokio_tungstenite::tungstenite::Error::Url(_)
                | tokio_tungstenite::tungstenite::Error::Tls(_)
                | tokio_tungstenite::tungstenite::Error::Http(_)
                // Raised when building the handshake request from an invalid
                // header name or value — deterministic given the same inputs,
                // so it belongs with `Http` rather than in the retry bucket.
                | tokio_tungstenite::tungstenite::Error::HttpFormat(_)
                | tokio_tungstenite::tungstenite::Error::AlreadyClosed
                | tokio_tungstenite::tungstenite::Error::AttackAttempt => Recovery::Fatal,
                // `tungstenite::Error` is `#[non_exhaustive]`, so this match
                // can never be exhaustive without a default. An unknown
                // transport error is treated as transient rather than fatal:
                // reconnecting is the recoverable guess, and a variant that
                // turns out to be permanent just costs one extra retry cycle
                // instead of wedging the feed shut.
                _ => Recovery::Reconnect,
            },
            Self::ConnectionClosed | Self::Stalled { .. } => Recovery::Reconnect,
            // One rejected topic zeroes every topic in the batch, so retrying
            // replays the same silence.
            Self::Server { .. } | Self::Url(_) => Recovery::Fatal,
            // Frame-level: the socket is fine, this one message was not.
            Self::Json { .. } | Self::Precision { .. } => Recovery::SkipFrame,
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
        assert_eq!(err.recovery(), Recovery::Fatal);
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

    #[test]
    fn a_permanent_transport_failure_is_fatal_not_a_retry_loop() {
        // Reconnecting after a TLS or bad-URL failure spins forever at full
        // backoff. Retrying is not "safe by default" here.
        let closed = RtdsError::from(tokio_tungstenite::tungstenite::Error::AlreadyClosed);
        assert_eq!(closed.recovery(), Recovery::Fatal);
    }

    #[test]
    fn a_malformed_handshake_request_is_fatal() {
        // HttpFormat comes from an invalid header name or value while
        // building the upgrade request. Same inputs produce the same failure,
        // so retrying it is a spin, not a recovery.
        let bad_header =
            tokio_tungstenite::tungstenite::http::header::HeaderName::from_bytes(b"in valid")
                .unwrap_err();
        let err = RtdsError::from(tokio_tungstenite::tungstenite::Error::HttpFormat(
            bad_header.into(),
        ));
        assert_eq!(err.recovery(), Recovery::Fatal);
    }

    #[test]
    fn a_transient_transport_failure_reconnects() {
        let err = RtdsError::from(tokio_tungstenite::tungstenite::Error::ConnectionClosed);
        assert_eq!(err.recovery(), Recovery::Reconnect);
    }

    #[test]
    fn a_bad_frame_skips_without_killing_the_connection() {
        // One unparseable message must not end a 24/7 feed.
        let source = serde_json::from_str::<serde_json::Value>("{not json").unwrap_err();
        let err = RtdsError::json("{not json", source);
        assert_eq!(err.recovery(), Recovery::SkipFrame);

        assert_eq!(
            RtdsError::Precision {
                raw: "1".repeat(40),
                topic: Topic::ChainlinkSpot,
            }
            .recovery(),
            Recovery::SkipFrame
        );
    }

    #[test]
    fn a_url_failure_is_fatal() {
        let err = RtdsError::Url(url::ParseError::EmptyHost);
        assert_eq!(err.recovery(), Recovery::Fatal);
    }

    #[test]
    fn json_errors_retain_the_raw_frame_in_display() {
        let source = serde_json::from_str::<serde_json::Value>("{not json").unwrap_err();
        let err = RtdsError::json("{not json", source);
        let rendered = err.to_string();
        assert!(rendered.contains("{not json"), "{rendered}");
    }

    #[test]
    fn connection_errors_keep_the_transport_cause_in_the_chain() {
        use std::error::Error as _;

        let err = RtdsError::from(tokio_tungstenite::tungstenite::Error::AlreadyClosed);
        assert!(err.source().is_some());
    }
}
