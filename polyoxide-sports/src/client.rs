//! The bare tier, and the connect and classify steps both tiers share.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::CloseFrame, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{error::SportsError, update::MatchUpdate};

/// The production endpoint.
pub const SPORTS_WS_URL: &str = "wss://sports-api.polymarket.com/ws";

/// How long one handshake may take before it is abandoned.
pub(crate) const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// An open connection.
pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Make sure rustls has a default `CryptoProvider` before opening a connection.
///
/// A deliberate twin of the same function in `polyoxide-clob`,
/// `polyoxide-rtds` and `polyoxide-perps`. `tokio-tungstenite` builds its TLS
/// config from the process-wide default provider, and rustls installs one
/// automatically only when exactly one backend feature is enabled. With
/// `ring` and `aws-lc-rs` both in a consumer's graph it installs neither and
/// panics inside `connect_async`. `install_default` returns `Err` when a
/// provider is already set, so crates racing is a no-op.
fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Open one connection, bounded by `connect_timeout`.
///
/// Takes the URL by value so the supervised tier can box the future.
pub(crate) async fn open(url: String, connect_timeout: Duration) -> Result<Socket, SportsError> {
    ensure_crypto_provider();
    match tokio::time::timeout(connect_timeout, connect_async(url.as_str())).await {
        Ok(Ok((socket, _response))) => Ok(socket),
        Ok(Err(source)) => Err(SportsError::Connect {
            source: Box::new(source),
        }),
        Err(_) => Err(SportsError::ConnectTimeout {
            after: connect_timeout,
        }),
    }
}

/// What one inbound message means. Both tiers classify through this, so they
/// cannot disagree about a frame.
pub(crate) enum Inbound {
    /// A match update, boxed because it is several times larger than the
    /// other variants.
    Update(Box<MatchUpdate>),
    /// A text frame that did not parse.
    Undecodable(SportsError),
    /// A control or binary frame: proof of life with nothing to yield.
    Alive,
    /// The server closed the connection.
    Closed(SportsError),
}

/// Classify one inbound message.
pub(crate) fn classify(message: Message) -> Inbound {
    match message {
        Message::Text(text) => match MatchUpdate::from_json(&text) {
            Ok(update) => Inbound::Update(Box::new(update)),
            Err(source) => Inbound::Undecodable(SportsError::Decode {
                raw: text.to_string(),
                source,
            }),
        },
        Message::Close(frame) => Inbound::Closed(closed(frame)),
        Message::Binary(bytes) => {
            tracing::debug!(len = bytes.len(), "skipping a binary frame on the sports feed");
            Inbound::Alive
        }
        Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => Inbound::Alive,
    }
}

/// The error for a connection that closed, with or without a close frame.
pub(crate) fn closed(frame: Option<CloseFrame>) -> SportsError {
    match frame {
        Some(frame) => SportsError::Closed {
            code: Some(u16::from(frame.code)),
            reason: frame.reason.to_string(),
        },
        None => SportsError::Closed {
            code: None,
            reason: String::new(),
        },
    }
}

/// One connection to the sports feed. Ends when the connection does.
///
/// For a feed that reconnects on its own, use `SportsWsBuilder`.
///
/// The server sends a protocol ping every 15 seconds. The transport queues
/// the pong when it reads the ping and sends it at the start of the next
/// read, so pongs go out as long as the stream is being polled.
///
/// # Example
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_sports::SportsWs;
///
/// # async fn run() -> Result<(), polyoxide_sports::SportsError> {
/// let mut feed = SportsWs::connect().await?;
/// while let Some(update) = feed.next().await {
///     let update = update?;
///     println!("{} {} {}", update.league_abbreviation, update.score, update.period);
/// }
/// # Ok(())
/// # }
/// ```
pub struct SportsWs {
    socket: Socket,
    finished: bool,
}

impl SportsWs {
    /// Connect to the production feed.
    pub async fn connect() -> Result<Self, SportsError> {
        Self::connect_to(SPORTS_WS_URL).await
    }

    /// Connect to another endpoint, such as a local test server.
    pub async fn connect_to(url: &str) -> Result<Self, SportsError> {
        let socket = open(url.to_owned(), DEFAULT_CONNECT_TIMEOUT).await?;
        Ok(Self {
            socket,
            finished: false,
        })
    }
}

impl Stream for SportsWs {
    type Item = Result<MatchUpdate, SportsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        loop {
            let message = match self.socket.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                Poll::Ready(Some(Err(source))) => {
                    self.finished = true;
                    return Poll::Ready(Some(Err(SportsError::Transport {
                        source: Box::new(source),
                    })));
                }
                Poll::Ready(None) => {
                    self.finished = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            };
            match classify(message) {
                Inbound::Update(update) => return Poll::Ready(Some(Ok(*update))),
                Inbound::Undecodable(error) => return Poll::Ready(Some(Err(error))),
                // Reading again is what sends the pong for a ping just read.
                Inbound::Alive => continue,
                Inbound::Closed(reason) => {
                    tracing::debug!(%reason, "the sports feed closed");
                    self.finished = true;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};

    use super::*;
    use crate::fixtures;

    #[test]
    fn the_default_url_is_the_sports_host() {
        assert_eq!(SPORTS_WS_URL, "wss://sports-api.polymarket.com/ws");
    }

    #[test]
    fn a_text_frame_is_an_update() {
        let Inbound::Update(update) = classify(Message::Text(fixtures::SOCCER.into())) else {
            panic!("a captured frame was not classified as an update");
        };
        assert_eq!(update.league_abbreviation, "kor");
    }

    #[test]
    fn a_bad_text_frame_keeps_its_raw_text() {
        let Inbound::Undecodable(SportsError::Decode { raw, .. }) =
            classify(Message::Text("not json".into()))
        else {
            panic!("a bad frame was not reported as undecodable");
        };
        assert_eq!(raw, "not json");
    }

    #[test]
    fn control_and_binary_frames_are_proof_of_life() {
        for message in [
            Message::Ping(b"p".to_vec().into()),
            Message::Pong(b"p".to_vec().into()),
            Message::Binary(b"b".to_vec().into()),
        ] {
            assert!(matches!(classify(message), Inbound::Alive));
        }
    }

    #[test]
    fn a_close_frame_keeps_its_code_and_reason() {
        let frame = CloseFrame {
            code: CloseCode::Away,
            reason: "bye".into(),
        };
        let Inbound::Closed(SportsError::Closed { code, reason }) =
            classify(Message::Close(Some(frame)))
        else {
            panic!("a close frame was not classified as closed");
        };
        assert_eq!(code, Some(1001));
        assert_eq!(reason, "bye");
    }

    #[test]
    fn a_bare_close_has_no_code() {
        let Inbound::Closed(SportsError::Closed { code, reason }) =
            classify(Message::Close(None))
        else {
            panic!("a bare close was not classified as closed");
        };
        assert_eq!(code, None);
        assert!(reason.is_empty());
    }

    #[test]
    fn the_bare_stream_can_move_between_tasks() {
        fn assert_send_unpin<T: Send + Unpin>() {}
        assert_send_unpin::<SportsWs>();
    }
}
