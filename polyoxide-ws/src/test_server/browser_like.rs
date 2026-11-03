//! A socket transport that behaves as a browser's does, for the suites of the
//! sockets that adapt to it.
//!
//! It dials as the native transport does, then takes away what a browser
//! takes away: the server's ping frames never reach the reader, and a send of
//! a ping, a pong or a raw frame is refused with the error the browser
//! transport gives (`Kind::Unsupported`, the same text), so a protocol that
//! sends one anyway fails the test that drives it. Its capabilities are all
//! `false`, which the browser transport reports.

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Sink, Stream, StreamExt};
use polyoxide_rt::{BoxFuture, Runtime};

use crate::{
    browser::plain::outgoing,
    tungstenite::{self, http, Message},
    BoxSocket, Capabilities, SocketTransport, TungsteniteTransport, WsError,
};

/// A socket as a browser shows it: the server's pings are read and dropped,
/// and a frame the browser cannot send is refused before it is sent.
struct BrowserSocket(BoxSocket);

impl Stream for BrowserSocket {
    type Item = Result<Message, tungstenite::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.0.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(Message::Ping(_)))) => continue,
                other => return other,
            }
        }
    }
}

impl Sink<Message> for BrowserSocket {
    type Error = tungstenite::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut *self.0).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        // The browser transport's own rule, so the two refuse alike.
        outgoing(&item)?;
        Pin::new(&mut *self.0).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut *self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut *self.0).poll_close(cx)
    }
}

/// The native dial with a browser's socket and capabilities.
#[derive(Debug, Default)]
pub struct BrowserLike(TungsteniteTransport);

impl BrowserLike {
    /// A browser-like transport over the native one's plain dial.
    pub fn new() -> Self {
        Self(TungsteniteTransport::new())
    }
}

impl SocketTransport for BrowserLike {
    fn connect(
        &self,
        request: http::Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        let dial = self.0.connect(request, timeout, runtime);
        Box::pin(async move {
            let socket: BoxSocket = Box::new(BrowserSocket(dial.await?));
            Ok(socket)
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(false, false, false)
    }
}

/// A [`BrowserLike`] ready to hand to a builder's `socket_transport`.
pub fn browser_like() -> Arc<dyn SocketTransport> {
    Arc::new(BrowserLike::new())
}

#[cfg(test)]
mod tests {
    use futures_util::SinkExt;
    use tokio::time::timeout;

    use super::*;
    use crate::{
        test_server::{Script, ScriptedServer},
        tungstenite::client::IntoClientRequest,
        Kind,
    };

    async fn dial(server: &ScriptedServer) -> BoxSocket {
        browser_like()
            .connect(
                server.url.as_str().into_client_request().unwrap(),
                Duration::from_secs(2),
                polyoxide_rt::default_runtime(),
            )
            .await
            .expect("a dial to the scripted server")
    }

    #[test]
    fn it_reports_what_the_browser_transport_reports() {
        let caps = BrowserLike::new().capabilities();
        assert!(!caps.client_pings && !caps.sees_server_pings && !caps.handshake_headers);
    }

    #[tokio::test]
    async fn a_ping_pong_or_raw_frame_is_refused_as_the_browser_transport_refuses_it() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut socket = dial(&server).await;
        let raw = Message::Frame(tungstenite::protocol::frame::Frame::ping(vec![]));
        for (message, text) in [
            (
                Message::Ping(vec![].into()),
                "a ping frame is not supported by the browser transport",
            ),
            (
                Message::Pong(vec![].into()),
                "a pong frame is not supported by the browser transport",
            ),
            (raw, "a raw frame is not supported by the browser transport"),
        ] {
            let err = socket.send(message).await.unwrap_err();
            let err = WsError::from(err);
            assert_eq!(err.kind(), Kind::Unsupported, "{err:?}");
            assert!(err.to_string().contains(text), "{err}");
        }
        // The refusals sent nothing and left the socket usable.
        socket
            .send(Message::Text("still open".into()))
            .await
            .unwrap();
        server
            .wait_for("the text frame", |s| !s.first_frames().is_empty())
            .await;
        assert_eq!(server.ping_count(), 0);
    }

    #[tokio::test]
    async fn the_servers_pings_never_reach_the_reader() {
        let server =
            ScriptedServer::start(vec![Script::pings_only(Duration::from_millis(20))]).await;
        let mut socket = dial(&server).await;
        let read = timeout(Duration::from_millis(300), socket.next()).await;
        assert!(read.is_err(), "a ping reached the reader: {read:?}");
    }
}
