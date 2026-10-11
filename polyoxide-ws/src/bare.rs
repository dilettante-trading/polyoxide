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
/// The server's close is answered: the RFC 6455 close reply is flushed before
/// the stream yields its last item, so it goes out even when the caller never
/// reads again (DRIFT R3). Once the stream has ended on the close,
/// [`closed`](Self::closed) says with what code and reason, as a
/// [`Kind::Closed`](crate::Kind::Closed) error.
///
/// The basis is `polyoxide-sports`' `SportsWs::poll_next` (`src/client.rs`)
/// and its frame classifier, with the queue `polyoxide-perps`' and
/// `polyoxide-binance`'s former bare tiers drained first and Binance's former
/// close capture (`record_close` and `closed_error`); perps, Binance and rtds
/// read through this type since Stories 4.6 to 4.8. Two divergence rows stay with each
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
    /// Whether tungstenite holds a close reply not yet flushed.
    replying: bool,
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
            replying: false,
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
    /// [`Kind::Closed`](crate::Kind::Closed) with the close frame's code and
    /// reason, or with no code and an empty reason when the stream ended
    /// without a close frame. A close frame without a code reads the same.
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

    /// Close the connection: send a close frame and flush it. On a
    /// connection the server already closed, flush the reply if it has not
    /// gone out yet; there is nothing else to send. A connection whose read
    /// failed is already gone, so closing it does nothing.
    pub async fn close(&mut self) -> Result<(), WsError> {
        if self.close.is_some() {
            self.flush_reply().await;
            return Ok(());
        }
        if self.finished {
            return Ok(());
        }
        // Sends the close frame and flushes it.
        self.socket.close(None).await?;
        Ok(())
    }

    /// Note the server's close frame, or the end of the stream without one.
    /// tungstenite queues the reply to a close frame as it reads it, and sends
    /// it on the next read or flush.
    pub(crate) fn record_close(&mut self, frame: Option<&Message>) {
        self.replying = matches!(frame, Some(Message::Close(_)));
        self.close = Some(match frame {
            Some(Message::Close(Some(frame))) => {
                (Some(u16::from(frame.code)), frame.reason.to_string())
            }
            _ => (None, String::new()),
        });
        self.finished = true;
        tracing::debug!(close = ?self.close, "the server closed the WebSocket connection");
    }

    /// Send the queued close reply, if there is one.
    pub(crate) async fn flush_reply(&mut self) {
        if self.replying {
            if let Err(err) = self.socket.flush().await {
                tracing::debug!(%err, "the close reply could not be sent");
            }
            self.replying = false;
        }
    }

    /// Keep a frame for the stream to yield later, if its mode yields it.
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

    /// Whether frames a request kept are waiting to be yielded.
    pub(crate) fn has_kept(&self) -> bool {
        !self.buffered.is_empty()
    }
}

impl Stream for Connection {
    type Item = Result<Message, WsError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            // RFC 6455 requires the close reply, and this stream may never be
            // read again once it has yielded its last item.
            if this.replying {
                match this.socket.poll_flush_unpin(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(err)) => {
                        tracing::debug!(%err, "the close reply could not be sent");
                    }
                    Poll::Ready(Ok(())) => {}
                }
                this.replying = false;
            }
            if let Some(kept) = this.buffered.pop_front() {
                return Poll::Ready(Some(Ok(kept)));
            }
            if this.finished {
                return Poll::Ready(None);
            }
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
                    this.keep(message);
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
    use crate::{
        connect,
        test_server::{Script, ScriptedServer},
        Kind, DEFAULT_CONNECT_TIMEOUT,
    };

    const WINDOW: Duration = Duration::from_secs(3);

    async fn open(server: &ScriptedServer, mode: Mode) -> Connection {
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
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
        let server = ScriptedServer::start(vec![Script::messages(frames())]).await;
        let mut connection = open(&server, Mode::Data).await;
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
        let server = ScriptedServer::start(vec![Script::messages(frames())]).await;
        let mut connection = open(&server, Mode::All).await;
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
            let server = ScriptedServer::start(vec![Script::messages(vec![Message::Ping(
                b"p1".to_vec().into(),
            )])])
            .await;
            let mut connection = open(&server, mode).await;
            let reader = match mode {
                Mode::Data => {
                    tokio::spawn(async move { while connection.next().await.is_some() {} })
                }
                Mode::All => {
                    assert_eq!(
                        next(&mut connection).await.unwrap().unwrap(),
                        Message::Ping(b"p1".to_vec().into())
                    );
                    // Held, never read again.
                    tokio::spawn(async move {
                        let _held = connection;
                        std::future::pending::<()>().await;
                    })
                }
            };
            server
                .wait_for("a pong", |s| s.pongs() == [b"p1".to_vec()])
                .await;
            reader.abort();
        }
    }

    #[tokio::test]
    async fn a_server_close_ends_the_stream_with_its_code_and_reason() {
        for (code, reason) in [(1011, "overloaded"), (1008, "policy")] {
            let server =
                ScriptedServer::start(vec![Script::close_at_once().close_with(code, reason)]).await;
            let mut connection = open(&server, Mode::Data).await;
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
        let server =
            ScriptedServer::start(vec![Script::close_at_once().close_with(1001, "bye")]).await;
        let mut connection = open(&server, Mode::All).await;
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
    async fn a_server_close_is_answered_though_the_stream_is_never_read_again() {
        // RFC 6455's close reply (DRIFT R3). tungstenite queues it as it reads
        // the close and sends it only on a later read or flush, which a
        // stream that has ended never makes.
        for mode in [Mode::Data, Mode::All] {
            let server = ScriptedServer::start(vec![
                Script::close_at_once().close_with(1011, "pong_timeout")
            ])
            .await;
            let mut connection = open(&server, mode).await;
            let last = next(&mut connection).await;
            match mode {
                Mode::Data => assert!(last.is_none(), "{last:?}"),
                Mode::All => assert!(matches!(last, Some(Ok(Message::Close(_)))), "{last:?}"),
            }
            assert_eq!(
                connection.closed().unwrap().kind(),
                Kind::Closed {
                    code: Some(1011),
                    reason: "pong_timeout"
                }
            );
            // Held open and never read again, as a caller that saw the end
            // would leave it.
            server
                .wait_for("the close reply", |s| s.close_reply_count() == 1)
                .await;
            drop(connection);
        }
    }

    #[tokio::test]
    async fn close_after_the_server_closed_succeeds() {
        let server = ScriptedServer::start(vec![Script::close_at_once()]).await;
        let mut connection = open(&server, Mode::Data).await;
        assert!(next(&mut connection).await.is_none());
        connection.close().await.expect("close");
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
    }

    #[tokio::test]
    async fn a_failed_read_is_yielded_once_and_ends_the_stream() {
        // The server drops the TCP connection without a closing handshake.
        let server = ScriptedServer::start(vec![Script::hold().then_cut()]).await;
        let mut connection = open(&server, Mode::Data).await;
        let err = next(&mut connection).await.unwrap().unwrap_err();
        assert!(matches!(err.kind(), Kind::Protocol | Kind::Io), "{err:?}");
        assert!(next(&mut connection).await.is_none());
        assert!(connection.closed().is_none(), "an error is not a close");
    }

    #[tokio::test]
    async fn close_after_a_failed_read_does_nothing() {
        let server = ScriptedServer::start(vec![Script::hold().then_cut()]).await;
        let mut connection = open(&server, Mode::Data).await;
        assert!(next(&mut connection).await.unwrap().is_err());
        connection
            .close()
            .await
            .expect("closing a connection that is already gone succeeds");
    }

    #[tokio::test]
    async fn close_sends_a_close_frame_the_server_receives() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut connection = open(&server, Mode::Data).await;
        connection.close().await.expect("close");
        server
            .wait_for("a close frame", |s| s.close_count() == 1)
            .await;
    }

    #[tokio::test]
    async fn every_frame_refreshes_last_inbound() {
        let server = ScriptedServer::start(vec![Script::messages(vec![Message::Pong(
            Vec::new().into(),
        )])])
        .await;
        let mut connection = open(&server, Mode::Data).await;
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
