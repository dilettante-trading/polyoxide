//! The bare tier: one connection as a `Stream` of frames (W11), with the
//! server's close captured (W9).

use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{SinkExt, Stream, StreamExt};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

use crate::{request::Requests, Socket, WsError};

/// Which frames a [`Connection`] yields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Text and Binary frames only. Pings, pongs and raw frames are read and
    /// skipped, the server's pings still answered, and a server close ends
    /// the stream.
    Data,
    /// Every frame, pings, pongs and the server's close included, for a
    /// caller whose liveness rule counts them. A close is the last item.
    All,
}

impl Mode {
    /// Whether a frame other than a close is yielded in this mode.
    fn yields(self, message: &Message) -> bool {
        match self {
            Self::Data => matches!(message, Message::Text(_) | Message::Binary(_)),
            Self::All => true,
        }
    }
}

/// One open connection, as a `Stream` of its frames.
///
/// Yields `Ok` frames as [`Mode`] selects, and ends after the server's close,
/// after the end of the stream, or after the one `Err` a failed read yields.
/// Frames kept while a [request](Self::request) awaited its answer come out
/// first, in the order they arrived. The server's pings are answered as the
/// stream is read.
///
/// Once it has ended on the server's close, [`closed`](Self::closed) says
/// with what code and reason, as a [`Kind::Closed`](crate::Kind::Closed)
/// error.
///
/// The basis is `polyoxide-sports`' `SportsWs::poll_next` (`src/client.rs`)
/// and its frame classifier, with the queue `polyoxide-perps` and
/// `polyoxide-binance` drain first and Binance's close capture
/// (`record_close` and `closed_error`). Two divergence rows stay with each
/// venue: what counts as liveness (D5), which [`Mode::All`] and
/// [`last_inbound`](Self::last_inbound) let a venue decide, and what a
/// Binary frame means (D12): clob decodes it as text, and the others skip it.
pub struct Connection {
    socket: Socket,
    mode: Mode,
    buffered: VecDeque<Message>,
    last_inbound: Instant,
    /// The server's close code and reason, once the stream ended on them.
    close: Option<(Option<u16>, String)>,
    finished: bool,
    pub(crate) requests: Requests,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("mode", &self.mode)
            .field("buffered", &self.buffered.len())
            .field("close", &self.close)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl Connection {
    /// Read `socket` in `mode`.
    pub fn new(socket: Socket, mode: Mode) -> Self {
        Self {
            socket,
            mode,
            buffered: VecDeque::new(),
            last_inbound: Instant::now(),
            close: None,
            finished: false,
            requests: Requests::default(),
        }
    }

    /// Which frames this connection yields.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// When the last frame of any kind arrived, pings, pongs and frames read
    /// while awaiting an answer included; when the connection was made, if
    /// none has.
    pub fn last_inbound(&self) -> Instant {
        self.last_inbound
    }

    /// The server's close, once the stream has ended on it: a
    /// [`Kind::Closed`](crate::Kind::Closed) with the frame's code and
    /// reason, or with neither when the stream ended without a close frame.
    /// `None` while the connection is open, and after a failed read, whose
    /// error the stream yielded.
    pub fn closed(&self) -> Option<WsError> {
        self.close
            .as_ref()
            .map(|(code, reason)| WsError::closed(*code, reason.as_str()))
    }

    /// Send one frame and flush it.
    pub async fn send(&mut self, message: Message) -> Result<(), WsError> {
        self.socket.send(message).await?;
        Ok(())
    }

    /// Close the connection: send a close frame and flush it. A connection
    /// the server already closed has nothing to send.
    pub async fn close(&mut self) -> Result<(), WsError> {
        if self.close.is_some() {
            return Ok(());
        }
        self.socket.close(None).await?;
        Ok(())
    }

    /// Note the server's close frame, or the end of the stream without one.
    pub(crate) fn record_close(&mut self, frame: Option<&Message>) {
        self.close = Some(match frame {
            Some(Message::Close(Some(frame))) => {
                (Some(u16::from(frame.code)), frame.reason.to_string())
            }
            _ => (None, String::new()),
        });
        self.finished = true;
        tracing::debug!(close = ?self.close, "the server closed the WebSocket connection");
    }

    /// Note a frame read while a request awaited its answer: a close ends
    /// the connection, and anything else the stream would yield is kept for
    /// it.
    pub(crate) fn keep(&mut self, message: Message) {
        if self.mode.yields(&message) {
            self.buffered.push_back(message);
        }
    }

    /// The underlying socket, for the request module's reads.
    pub(crate) fn socket(&mut self) -> &mut Socket {
        &mut self.socket
    }

    /// Note that a frame arrived now.
    pub(crate) fn touch(&mut self) {
        self.last_inbound = Instant::now();
    }

    /// End the stream after a failed read.
    pub(crate) fn fail(&mut self) {
        self.finished = true;
    }

    /// Whether the stream has ended.
    pub(crate) fn is_finished(&self) -> bool {
        self.finished
    }
}

impl Stream for Connection {
    type Item = Result<Message, WsError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if let Some(kept) = this.buffered.pop_front() {
            return Poll::Ready(Some(Ok(kept)));
        }
        if this.finished {
            return Poll::Ready(None);
        }
        loop {
            let message = match this.socket.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                Poll::Ready(Some(Err(err))) => {
                    this.fail();
                    return Poll::Ready(Some(Err(err.into())));
                }
                Poll::Ready(None) => {
                    this.record_close(None);
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            };
            this.touch();
            match message {
                Message::Close(_) => {
                    this.record_close(Some(&message));
                    return Poll::Ready(match this.mode {
                        Mode::All => Some(Ok(message)),
                        Mode::Data => None,
                    });
                }
                Message::Ping(_) if this.mode == Mode::All => {
                    // tungstenite has queued the pong, and sends it on the
                    // next read or flush; the caller may not read again soon.
                    let _ = this.socket.poll_flush_unpin(cx);
                    return Poll::Ready(Some(Ok(message)));
                }
                message if this.mode.yields(&message) => return Poll::Ready(Some(Ok(message))),
                // Reading again is what sends the pong for a ping just read.
                _ => continue,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};

    use super::*;
    use crate::{connect, loopback, Kind, DEFAULT_CONNECT_TIMEOUT};

    const WINDOW: Duration = Duration::from_secs(3);

    async fn open(url: &str, mode: Mode) -> Connection {
        let socket = connect(url, DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect("connect");
        Connection::new(socket, mode)
    }

    async fn next(connection: &mut Connection) -> Option<Result<Message, WsError>> {
        tokio::time::timeout(WINDOW, connection.next())
            .await
            .expect("an item within the window")
    }

    fn frames() -> Vec<Message> {
        vec![
            Message::Ping(b"p".to_vec().into()),
            Message::Pong(b"q".to_vec().into()),
            Message::Binary(vec![1, 2, 3].into()),
            Message::Text("data".into()),
        ]
    }

    #[tokio::test]
    async fn data_mode_yields_text_and_binary_and_skips_control_frames() {
        let url = loopback::push(frames()).await;
        let mut connection = open(&url, Mode::Data).await;
        assert_eq!(
            next(&mut connection).await.unwrap().unwrap(),
            Message::Binary(vec![1, 2, 3].into())
        );
        assert_eq!(
            next(&mut connection).await.unwrap().unwrap(),
            Message::Text("data".into())
        );
    }

    #[tokio::test]
    async fn all_mode_yields_every_frame() {
        let url = loopback::push(frames()).await;
        let mut connection = open(&url, Mode::All).await;
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(next(&mut connection).await.unwrap().unwrap());
        }
        assert_eq!(seen, frames());
    }

    #[tokio::test]
    async fn a_server_ping_is_answered_in_either_mode() {
        // Data mode answers by reading on; all mode yields the ping and still
        // sends the pong, though the caller never reads again.
        for mode in [Mode::Data, Mode::All] {
            let (url, pongs) = loopback::ping(b"p1").await;
            let mut connection = open(&url, mode).await;
            let reader = async {
                if mode == Mode::All {
                    assert_eq!(
                        next(&mut connection).await.unwrap().unwrap(),
                        Message::Ping(b"p1".to_vec().into())
                    );
                    std::future::pending::<()>().await;
                } else {
                    while connection.next().await.is_some() {}
                }
            };
            tokio::select! {
                () = reader => panic!("the stream ended"),
                pong = tokio::time::timeout(WINDOW, pongs) => assert_eq!(
                    pong.expect("a pong within the window").expect("a pong"),
                    b"p1".to_vec(),
                    "{mode:?}"
                ),
            }
        }
    }

    #[tokio::test]
    async fn a_server_close_ends_the_stream_with_its_code_and_reason() {
        for (code, reason) in [(1011, "overloaded"), (1008, "policy")] {
            let url = loopback::close_with(code, reason).await;
            let mut connection = open(&url, Mode::Data).await;
            assert!(connection.closed().is_none(), "open, so not closed yet");
            assert!(next(&mut connection).await.is_none());
            assert_eq!(
                connection.closed().expect("closed").kind(),
                Kind::Closed {
                    code: Some(code),
                    reason
                }
            );
            assert!(next(&mut connection).await.is_none(), "it stays ended");
        }
    }

    #[tokio::test]
    async fn all_mode_yields_the_close_frame_last() {
        let url = loopback::close_with(1001, "bye").await;
        let mut connection = open(&url, Mode::All).await;
        assert_eq!(
            next(&mut connection).await.unwrap().unwrap(),
            Message::Close(Some(CloseFrame {
                code: CloseCode::Away,
                reason: "bye".into()
            }))
        );
        assert!(next(&mut connection).await.is_none());
        assert_eq!(
            connection.closed().unwrap().kind(),
            Kind::Closed {
                code: Some(1001),
                reason: "bye"
            }
        );
    }

    #[tokio::test]
    async fn a_failed_read_is_yielded_once_and_ends_the_stream() {
        // The server drops the TCP connection without a closing handshake.
        let url = loopback::drop_after_handshake().await;
        let mut connection = open(&url, Mode::Data).await;
        let err = next(&mut connection).await.unwrap().unwrap_err();
        assert!(matches!(err.kind(), Kind::Protocol | Kind::Io), "{err:?}");
        assert!(next(&mut connection).await.is_none());
        assert!(connection.closed().is_none(), "an error is not a close");
    }

    #[tokio::test]
    async fn close_sends_a_close_frame_the_server_receives() {
        let (url, closed) = loopback::await_close().await;
        let mut connection = open(&url, Mode::Data).await;
        connection.close().await.expect("close");
        tokio::time::timeout(WINDOW, closed)
            .await
            .expect("the server saw a close frame")
            .expect("the server task ran");
    }

    #[tokio::test]
    async fn every_frame_refreshes_last_inbound() {
        let url = loopback::push(vec![Message::Pong(Vec::new().into())]).await;
        let mut connection = open(&url, Mode::Data).await;
        let opened = connection.last_inbound();
        // The pong is skipped, so the read waits; it must still count.
        let _ = tokio::time::timeout(Duration::from_millis(200), connection.next()).await;
        assert!(connection.last_inbound() > opened);
    }

    #[test]
    fn a_connection_can_move_between_tasks() {
        fn assert_send_unpin<T: Send + Unpin>() {}
        assert_send_unpin::<Connection>();
    }
}
