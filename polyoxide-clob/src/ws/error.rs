use thiserror::Error;

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
}

impl From<tokio_tungstenite::tungstenite::Error> for WebSocketError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        WebSocketError::Connection(Box::new(err))
    }
}
