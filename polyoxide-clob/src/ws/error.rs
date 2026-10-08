use polyoxide_venue::{class_for_handshake_status, Class, Classify};
use thiserror::Error;
use tokio_tungstenite::tungstenite;

/// WebSocket-specific errors.
#[derive(Debug, Error)]
pub enum WebSocketError {
    /// WebSocket connection error
    #[error("WebSocket connection error: {0}")]
    Connection(Box<tokio_tungstenite::tungstenite::Error>),

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

impl From<tokio_tungstenite::tungstenite::Error> for WebSocketError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        WebSocketError::Connection(Box::new(err))
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

/// The transport by the socket table, a frame that did not parse a `Decode`,
/// a refused login `Unauthorized`, and the client's own refusals an
/// `InvalidRequest`.
impl Classify for WebSocketError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => transport_class(err),
            Self::Json(_) => Class::Decode,
            Self::ConnectionClosed | Self::ConnectTimeout { .. } => Class::Network,
            Self::Authentication(_) => Class::Unauthorized,
            Self::InvalidMessage(_) | Self::Url(_) | Self::MembershipClosed => {
                Class::InvalidRequest
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use tungstenite::{error, http, Message};

    fn handshake(status: u16) -> tungstenite::Error {
        tungstenite::Error::Http(http::Response::builder().status(status).body(None).unwrap())
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
            let err = WebSocketError::from(err);
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
        }
    }

    #[test]
    fn every_variant_classifies() {
        let json = serde_json::from_str::<u8>("x").unwrap_err();
        let rows = [
            (
                WebSocketError::from(handshake(503)),
                Class::Unavailable { code: None },
            ),
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
