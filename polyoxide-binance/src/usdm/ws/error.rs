//! Errors of the socket tiers and how each one recovers.

use std::time::Duration;

use polyoxide_venue::{class_for_close_code, class_for_handshake_status, Class, Classify};
use polyoxide_ws::{retrying_can_fix, Kind, WsError};
use thiserror::Error;

use crate::usdm::ws::{stream::StreamName, StreamPath};

polyoxide_venue::impl_ws_classification!();

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
    /// The connection could not be opened, or failed mid-stream: the kit's
    /// one socket error, read through its [`Kind`]. A
    /// server close and a connect timeout have variants of their own.
    #[error("WebSocket transport error: {0}")]
    Connect(#[source] WsError),

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

/// A server close is [`UsdmWsError::Closed`], a connect timeout
/// [`UsdmWsError::ConnectTimeout`], and every other socket failure
/// [`UsdmWsError::Connect`].
impl From<WsError> for UsdmWsError {
    fn from(err: WsError) -> Self {
        match err.kind() {
            Kind::Closed { code, reason } => Self::Closed {
                code,
                reason: reason.to_owned(),
            },
            Kind::ConnectTimeout { after } => Self::ConnectTimeout(after),
            _ => Self::Connect(err),
        }
    }
}

impl UsdmWsError {
    /// How the supervised tier treats this error.
    pub fn recovery(&self) -> Recovery {
        match self {
            Self::Connect(err) => match err.kind() {
                // Binance's own rules: a connection used after it closed
                // (D19) and a frame or header past tungstenite's limits
                // reconnect.
                Kind::AlreadyClosed | Kind::Capacity => Recovery::Reconnect,
                // A handshake refused with 408, 425, 429 or 5xx is the host
                // being unwell or throttling, by polyoxide-venue's status
                // rule; any other status, a bad URL or TLS name, or an attack
                // detection is our request. Everything else, a lost
                // connection among them, is retried.
                _ if retrying_can_fix(err, |status| {
                    class_for_handshake_status(status).is_retriable()
                }) =>
                {
                    Recovery::Reconnect
                }
                _ => Recovery::Fatal,
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

/// The transport by the one socket table
/// (`polyoxide_venue::impl_ws_classification!`), a server close by its close
/// code, a refused request a `VenueRefusal` with Binance's code in decimal,
/// an answer or frame that did not parse a `Decode`, and the client's own
/// refusals an `InvalidRequest`.
///
/// One row is Binance's own, ahead of the table (D14): a handshake refused
/// with `403` is `Restricted`, as on REST. Binance's firewall answers it, and
/// it is not a credential failure, since the streams need none.
///
/// Every error is a fault except a handshake refused with `451`: Binance does
/// not serve the caller's region, a defined outcome, as
/// [`BinanceError::RegionBlocked`](crate::BinanceError::RegionBlocked) is on
/// REST. A `418` ban and a `403` from the firewall stay faults.
impl Classify for UsdmWsError {
    fn class(&self) -> Class {
        match self {
            Self::Connect(err) => match err.kind() {
                Kind::Refused { status: 403, .. } => Class::Restricted,
                _ => ws_class(err),
            },
            Self::ConnectTimeout(_) | Self::NoAnswer { .. } => Class::Network,
            Self::Closed { code, .. } => class_for_close_code(*code),
            Self::Refused { code, .. } => Class::VenueRefusal {
                code: Some(code.to_string().into()),
            },
            Self::Response { .. } | Self::Frame { .. } => Class::Decode,
            Self::TooManyStreams { .. } | Self::WrongPath { .. } | Self::Stopped => {
                Class::InvalidRequest
            }
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Connect(err) if !ws_is_fault(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_ws::tungstenite::{self, http};

    fn http_error(status: u16) -> UsdmWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> UsdmWsError {
        UsdmWsError::from(WsError::from(err))
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_and_one_refused_for_us_is_fatal() {
        assert_eq!(http_error(503).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(429).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(408).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(425).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(451).recovery(), Recovery::Fatal);
        assert_eq!(http_error(404).recovery(), Recovery::Fatal);
        assert_eq!(http_error(418).recovery(), Recovery::Fatal);
    }

    #[test]
    fn losing_the_connection_reconnects_and_a_bad_frame_is_skipped() {
        for err in [
            transport(tungstenite::Error::ConnectionClosed),
            UsdmWsError::ConnectTimeout(Duration::from_secs(10)),
            UsdmWsError::Closed {
                code: Some(1008),
                reason: "Too many requests".into(),
            },
            UsdmWsError::NoAnswer {
                id: 3,
                timeout: Duration::from_secs(10),
            },
            UsdmWsError::Response {
                id: 4,
                raw: "{\"id\":4}".into(),
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
        let wrong = UsdmWsError::WrongPath {
            stream: StreamName::BookTicker(crate::usdm::types::Symbol::new("BTCUSDT").unwrap()),
            path: StreamPath::Market,
        };
        assert_eq!(wrong.recovery(), Recovery::Fatal);
        assert_eq!(
            wrong.to_string(),
            "btcusdt@bookTicker rides the public path, not market"
        );
        let url = transport(tungstenite::Error::Url(
            tungstenite::error::UrlError::NoHostName,
        ));
        assert_eq!(url.recovery(), Recovery::Fatal);
        assert_eq!(UsdmWsError::Stopped.recovery(), Recovery::Fatal);
    }

    #[test]
    fn a_server_close_and_a_connect_timeout_keep_their_own_variants() {
        // L2-D4: the kit's socket error maps onto Binance's variants, so a
        // close's code and a timeout read as they did before the kit.
        let closed = UsdmWsError::from(WsError::closed(Some(1008), "Too many requests"));
        assert!(
            matches!(&closed, UsdmWsError::Closed { code: Some(1008), reason } if reason == "Too many requests"),
            "{closed:?}"
        );
        let after = Duration::from_secs(10);
        assert!(matches!(
            UsdmWsError::from(WsError::connect_timeout(after)),
            UsdmWsError::ConnectTimeout(timeout) if timeout == after
        ));
        // The bare tier maps an unanswered request itself, with its id.
        assert!(matches!(
            UsdmWsError::from(WsError::no_answer(after)),
            UsdmWsError::Connect(_)
        ));
        assert!(matches!(
            transport(tungstenite::Error::ConnectionClosed),
            UsdmWsError::Connect(_)
        ));
    }

    #[test]
    fn a_refused_429_surfaces_its_retry_after() {
        // L2-D8: the socket table reads a refusal's `Retry-After`, which the
        // old transport class dropped.
        let err = transport(tungstenite::Error::Http(
            http::Response::builder()
                .status(429)
                .header("Retry-After", "7")
                .body(None)
                .unwrap(),
        ));
        assert_eq!(
            err.class(),
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(7))
            }
        );
        assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
        assert!(err.is_retriable());
    }

    #[test]
    fn usdm_ws_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<UsdmWsError>();
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
        // (status, class, is_fault)
        for (status, class, fault) in [
            (401, Class::Unauthorized, true),
            // Binance's firewall, as on REST.
            (403, Class::Restricted, true),
            (404, Class::VenueRefusal { code: None }, true),
            // A ban the client earned by overspending its weight.
            (418, Class::Restricted, true),
            (429, Class::RateLimited { retry_after: None }, true),
            // A region block, as `BinanceError::RegionBlocked` is on REST.
            (451, Class::Restricted, false),
            (503, Class::Unavailable { code: None }, true),
            (200, Class::Decode, true),
        ] {
            assert_eq!(http_error(status).class(), class, "{status}");
            assert_eq!(http_error(status).is_fault(), fault, "{status}");
        }
    }

    #[test]
    fn every_variant_classifies() {
        let symbol = crate::usdm::types::Symbol::new("BTCUSDT").unwrap();
        let rows = [
            (http_error(503), Class::Unavailable { code: None }),
            (
                UsdmWsError::ConnectTimeout(Duration::from_secs(10)),
                Class::Network,
            ),
            (
                UsdmWsError::Closed {
                    code: Some(1008),
                    reason: "Too many requests".into(),
                },
                Class::VenueRefusal {
                    code: Some("1008".into()),
                },
            ),
            (
                UsdmWsError::Closed {
                    code: Some(1001),
                    reason: String::new(),
                },
                Class::Network,
            ),
            (
                UsdmWsError::Closed {
                    code: None,
                    reason: String::new(),
                },
                Class::Network,
            ),
            (
                UsdmWsError::Refused {
                    code: 2,
                    msg: "Invalid request".into(),
                },
                Class::VenueRefusal {
                    code: Some("2".into()),
                },
            ),
            (
                UsdmWsError::NoAnswer {
                    id: 3,
                    timeout: Duration::from_secs(10),
                },
                Class::Network,
            ),
            (
                UsdmWsError::Response {
                    id: 4,
                    raw: "{}".into(),
                },
                Class::Decode,
            ),
            (
                UsdmWsError::TooManyStreams {
                    path: StreamPath::Market,
                    limit: 1024,
                },
                Class::InvalidRequest,
            ),
            (
                UsdmWsError::WrongPath {
                    stream: StreamName::BookTicker(symbol),
                    path: StreamPath::Market,
                },
                Class::InvalidRequest,
            ),
            (
                UsdmWsError::Frame {
                    stream: "btcusdt@aggTrade".into(),
                    raw: "{}".into(),
                    reason: "missing field".into(),
                },
                Class::Decode,
            ),
            (UsdmWsError::Stopped, Class::InvalidRequest),
        ];
        for (err, class) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
