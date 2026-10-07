//! Errors of the socket tiers and how each one recovers.

use std::time::Duration;

use thiserror::Error;

use crate::usdm::ws::{stream::StreamName, StreamPath};

/// What to do about an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Recovery {
    /// Replace the connection and replay its streams.
    Reconnect,
    /// Skip this frame and keep reading.
    SkipFrame,
    /// Give up: retrying replays the same failure.
    Fatal,
}

/// Error type for the socket tiers.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum UsdmWsError {
    /// The connection could not be opened, or failed mid-stream.
    #[error("WebSocket transport error: {0}")]
    Connect(#[from] Box<tokio_tungstenite::tungstenite::Error>),

    /// The connection did not open within the connect timeout.
    #[error("no connection within {0:?}")]
    ConnectTimeout(Duration),

    /// The server closed the connection, with its close code and reason when
    /// it sent them: `1008 "Too many requests"` after a burst of requests,
    /// `1008 "Invalid request"` after a refused 1025th stream.
    #[error("the server closed the connection ({code:?}: {reason})")]
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },

    /// The server answered a request with `{"error": {"code", "msg"}}`.
    #[error("the server refused the request: {code} {msg}")]
    Refused {
        /// Binance's code.
        code: i64,
        /// Binance's message.
        msg: String,
    },

    /// No answer to a request, or no pong to a ping (id 0), within the answer
    /// timeout.
    #[error("no answer to request {id} within {timeout:?}")]
    NoAnswer {
        /// The request id.
        id: u64,
        /// How long the client waited.
        timeout: Duration,
    },

    /// An answer that was not the documented shape.
    #[error("malformed answer to request {id}: {raw}")]
    Response {
        /// The request id.
        id: u64,
        /// The answer as sent.
        raw: String,
    },

    /// A subscribe that would take a connection past Binance's cap. Nothing
    /// was sent: the server answers the 1025th stream with an error and then
    /// closes the connection, losing all 1024.
    #[error("a {path} connection carries at most {limit} streams")]
    TooManyStreams {
        /// The path whose connection is full.
        path: StreamPath,
        /// The cap.
        limit: usize,
    },

    /// A stream for the other path, on the bare tier. Nothing was sent.
    #[error("{stream} rides the {} path, not {path}", stream.path())]
    WrongPath {
        /// The stream.
        stream: StreamName,
        /// The connection's path.
        path: StreamPath,
    },

    /// A text frame that did not decode as an update.
    #[error("a frame on {stream:?} did not decode: {reason}")]
    Frame {
        /// The envelope's stream name, or empty when there was none.
        stream: String,
        /// The frame as sent.
        raw: String,
        /// Why it did not decode.
        reason: String,
    },

    /// The supervised tier has stopped; its handles can no longer be used.
    #[error("the supervised connection has stopped")]
    Stopped,
}

impl From<tokio_tungstenite::tungstenite::Error> for UsdmWsError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connect(Box::new(err))
    }
}

impl UsdmWsError {
    /// How the supervised tier treats this error.
    pub fn recovery(&self) -> Recovery {
        use tokio_tungstenite::tungstenite::Error as Ws;
        match self {
            Self::Connect(err) => match &**err {
                // A handshake refused with 5xx or 429 is the host being unwell
                // or throttling; any other status is our request.
                Ws::Http(response) => {
                    let status = response.status();
                    if status.is_server_error() || status.as_u16() == 429 {
                        Recovery::Reconnect
                    } else {
                        Recovery::Fatal
                    }
                }
                Ws::Url(_) | Ws::Tls(_) | Ws::HttpFormat(_) | Ws::AttackAttempt => Recovery::Fatal,
                // `tungstenite::Error` is non_exhaustive; an unknown transport
                // error costs one retry cycle if it turns out permanent.
                _ => Recovery::Reconnect,
            },
            Self::ConnectTimeout(_)
            | Self::Closed { .. }
            | Self::NoAnswer { .. }
            | Self::Response { .. } => Recovery::Reconnect,
            Self::Frame { .. } => Recovery::SkipFrame,
            Self::Refused { .. }
            | Self::TooManyStreams { .. }
            | Self::WrongPath { .. }
            | Self::Stopped => Recovery::Fatal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::tungstenite::{self, http};

    fn http_error(status: u16) -> UsdmWsError {
        UsdmWsError::from(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_and_one_refused_for_us_is_fatal() {
        assert_eq!(http_error(503).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(429).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(451).recovery(), Recovery::Fatal);
        assert_eq!(http_error(404).recovery(), Recovery::Fatal);
    }

    #[test]
    fn losing_the_connection_reconnects_and_a_bad_frame_is_skipped() {
        for err in [
            UsdmWsError::from(tungstenite::Error::ConnectionClosed),
            UsdmWsError::ConnectTimeout(Duration::from_secs(10)),
            UsdmWsError::Closed {
                code: Some(1008),
                reason: "Too many requests".into(),
            },
            UsdmWsError::NoAnswer {
                id: 3,
                timeout: Duration::from_secs(10),
            },
        ] {
            assert_eq!(err.recovery(), Recovery::Reconnect, "{err}");
        }
        let frame = UsdmWsError::Frame {
            stream: "btcusdt@aggTrade".into(),
            raw: "{}".into(),
            reason: "missing field".into(),
        };
        assert_eq!(frame.recovery(), Recovery::SkipFrame);
    }

    #[test]
    fn a_refusal_and_a_request_the_client_refused_to_send_are_fatal() {
        let refused = UsdmWsError::Refused {
            code: 2,
            msg: "Invalid request".into(),
        };
        assert_eq!(refused.recovery(), Recovery::Fatal);
        let full = UsdmWsError::TooManyStreams {
            path: StreamPath::Market,
            limit: 1024,
        };
        assert_eq!(full.recovery(), Recovery::Fatal);
        assert_eq!(
            full.to_string(),
            "a market connection carries at most 1024 streams"
        );
        assert_eq!(UsdmWsError::Stopped.recovery(), Recovery::Fatal);
    }

    #[test]
    fn usdm_ws_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<UsdmWsError>();
    }
}
