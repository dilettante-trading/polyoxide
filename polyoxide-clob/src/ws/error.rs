use polyoxide_venue::{Class, Classify};
use polyoxide_ws::WsError;
use thiserror::Error;

polyoxide_venue::impl_ws_classification!();

/// WebSocket-specific errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WebSocketError {
    /// The connection could not be opened, or failed mid-stream: the kit's
    /// one socket error, read through its [`Kind`](polyoxide_ws::Kind). A
    /// connect that timed out is [`ConnectTimeout`](Self::ConnectTimeout).
    #[error("WebSocket connection error: {0}")]
    Connection(#[from] WsError),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Connection was closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Authentication error
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Invalid message received
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// URL parse error
    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    /// Connecting (TCP, TLS, or WebSocket handshake) exceeded the configured
    /// per-address timeout, and no later address succeeded either.
    #[error("WebSocket connect to {url} timed out after {timeout:?} per address")]
    ConnectTimeout {
        /// The URL that was being connected to.
        url: String,
        /// The per-address timeout that elapsed.
        timeout: std::time::Duration,
    },

    /// A [`MembershipHandle`](crate::ws::MembershipHandle) was used after the
    /// connection's `run` loop had exited, so there is no socket left to send
    /// the frame on. Reconnect and take a fresh handle.
    #[error("subscription update refused: the connection's run loop has exited")]
    MembershipClosed,
}

/// The transport by the one socket table
/// (`polyoxide_venue::impl_ws_classification!`), whose only non-fault is an
/// upgrade refused with 451, a timeout `Network`, a frame that did not parse
/// a `Decode`, a refused login `Unauthorized`, and the client's own refusals
/// an `InvalidRequest`.
impl Classify for WebSocketError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => ws_class(err),
            Self::Json(_) => Class::Decode,
            Self::ConnectionClosed | Self::ConnectTimeout { .. } => Class::Network,
            Self::Authentication(_) => Class::Unauthorized,
            Self::InvalidMessage(_) | Self::Url(_) | Self::MembershipClosed => {
                Class::InvalidRequest
            }
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Connection(err) if !ws_is_fault(err))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use polyoxide_ws::tungstenite::{self, error, http, Message};

    fn handshake(status: u16) -> tungstenite::Error {
        tungstenite::Error::Http(http::Response::builder().status(status).body(None).unwrap())
    }

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> WebSocketError {
        WebSocketError::from(WsError::from(err))
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
                tungstenite::Error::Protocol(error::ProtocolError::ResetWithoutClosingHandshake),
                Class::Network,
            ),
            (
                tungstenite::Error::WriteBufferFull(Message::Close(None)),
                Class::Network,
            ),
            (tungstenite::Error::Utf8, Class::Network),
            (tungstenite::Error::AlreadyClosed, Class::InvalidRequest),
            (
                tungstenite::Error::Url(error::UrlError::NoHostName),
                Class::InvalidRequest,
            ),
            (
                tungstenite::Error::HttpFormat(bad_header.into()),
                Class::InvalidRequest,
            ),
            (
                tungstenite::Error::Tls(error::TlsError::InvalidDnsName),
                Class::InvalidRequest,
            ),
            (tungstenite::Error::AttackAttempt, Class::InvalidRequest),
            (
                tungstenite::Error::Capacity(error::CapacityError::TooManyHeaders),
                Class::InvalidRequest,
            ),
            (handshake(401), Class::Unauthorized),
            (handshake(404), Class::VenueRefusal { code: None }),
            (handshake(429), Class::RateLimited { retry_after: None }),
            (handshake(503), Class::Unavailable { code: None }),
            (handshake(200), Class::Decode),
        ];
        for (err, class) in rows {
            let err = transport(err);
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
        }
    }

    /// A refused upgrade with these headers, as the connection reports it.
    fn refused(status: u16, headers: &[(&str, &str)]) -> WebSocketError {
        let mut response = http::Response::builder().status(status);
        for (name, value) in headers {
            response = response.header(*name, *value);
        }
        transport(tungstenite::Error::Http(response.body(None).unwrap()))
    }

    #[test]
    fn a_451_refusal_is_restricted_and_not_a_fault() {
        // A region the venue does not serve answers as designed, as a 451 is
        // on every HTTP client in the workspace; a 418 ban stays a fault.
        let err = refused(451, &[]);
        assert_eq!(err.class(), Class::Restricted);
        assert!(!err.is_fault(), "{err:?}");
        assert!(refused(418, &[]).is_fault());
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
        assert!(err.is_retriable());
    }

    #[test]
    fn every_variant_classifies() {
        let json = serde_json::from_str::<u8>("x").unwrap_err();
        let rows = [
            (transport(handshake(503)), Class::Unavailable { code: None }),
            (WebSocketError::Json(json), Class::Decode),
            (WebSocketError::ConnectionClosed, Class::Network),
            (
                WebSocketError::ConnectTimeout {
                    url: "wss://example.invalid".into(),
                    timeout: Duration::from_secs(10),
                },
                Class::Network,
            ),
            (
                WebSocketError::Authentication("refused".into()),
                Class::Unauthorized,
            ),
            (
                WebSocketError::InvalidMessage("URL has no host".into()),
                Class::InvalidRequest,
            ),
            (
                WebSocketError::Url(url::ParseError::EmptyHost),
                Class::InvalidRequest,
            ),
            (WebSocketError::MembershipClosed, Class::InvalidRequest),
        ];
        for (err, class) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
