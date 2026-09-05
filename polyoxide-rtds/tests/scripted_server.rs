//! A local WebSocket server for testing supervision behaviour.
//!
//! Not a test file in its own right — `supervision.rs` includes it with
//! `#[path]`. It exists because the behaviours tier 2 adds (reconnect,
//! staleness detection, resubscribe) cannot be triggered on demand against the
//! real host.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// What the server should do on a given connection.
#[derive(Debug, Clone)]
pub enum Script {
    /// Send these frames, then hold the connection open and silent.
    SendThenIdle(Vec<String>),
    /// Send these frames, then close the connection.
    SendThenClose(Vec<String>),
}

/// A running local server.
pub struct ScriptedServer {
    /// The `ws://` URL clients should connect to.
    pub url: String,
    /// How many connections have been accepted so far.
    connections: Arc<AtomicUsize>,
    /// The subscription frame received on each connection, in order.
    received: Arc<Mutex<Vec<String>>>,
}

impl ScriptedServer {
    /// Start a server that applies `scripts[n]` to the n-th connection,
    /// repeating the last script for any further connections.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let connections = Arc::new(AtomicUsize::new(0));
        let received = Arc::new(Mutex::new(Vec::new()));

        let task_connections = Arc::clone(&connections);
        let task_received = Arc::clone(&received);
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let index = task_connections.fetch_add(1, Ordering::SeqCst);
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or(Script::SendThenIdle(Vec::new()));
                let received = Arc::clone(&task_received);
                tokio::spawn(async move {
                    let _ = serve(stream, script, received).await;
                });
            }
        });

        Self {
            url: format!("ws://{addr}"),
            connections,
            received,
        }
    }

    /// How many connections the server has accepted.
    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// The subscription frames received, one per connection, in order.
    ///
    /// **Not synchronised with [`connection_count`](Self::connection_count).**
    /// That counter increments in the accept loop, before the per-connection
    /// task is spawned; a frame lands here later, inside that task. So
    /// `connection_count() == n` does not imply `received_subscriptions()`
    /// has `n` entries yet.
    ///
    /// Establish happens-before some other way first — either poll this until
    /// it reaches the length you expect, or observe a frame the server sent,
    /// which works because [`serve`] records the subscription *before* it
    /// sends anything.
    ///
    /// The `expect` cannot fire in practice: poisoning would require a panic
    /// inside a `Vec<String>` push or clone.
    pub fn received_subscriptions(&self) -> Vec<String> {
        self.received
            .lock()
            .expect("received mutex poisoned")
            .clone()
    }
}

async fn serve(
    stream: TcpStream,
    script: Script,
    received: Arc<Mutex<Vec<String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ws = accept_async(stream).await?;

    // The client sends its subscription frame immediately on connect.
    //
    // Recording it BEFORE sending any scripted frames is load-bearing, not
    // incidental. It is what lets a test treat "the client saw a frame from
    // us" as proof that the subscription was already recorded — which is how
    // the supervision tests avoid racing `received_subscriptions()` against
    // `connection_count()`. Do not move this below the send loop.
    if let Some(Ok(Message::Text(frame))) = ws.next().await {
        received
            .lock()
            .expect("received mutex poisoned")
            .push(frame.to_string());
    }

    let (frames, close_after) = match script {
        Script::SendThenIdle(frames) => (frames, false),
        Script::SendThenClose(frames) => (frames, true),
    };

    for frame in frames {
        ws.send(Message::Text(frame.into())).await?;
    }

    if close_after {
        ws.close(None).await?;
        return Ok(());
    }

    // Hold open and silent, draining pings so the socket stays healthy. This
    // is what a stalled-but-not-closed feed looks like.
    while let Some(Ok(message)) = ws.next().await {
        if matches!(message, Message::Close(_)) {
            break;
        }
    }
    Ok(())
}

// This file is test infrastructure for Task 11's supervision tests, not a
// test in its own right -- but infrastructure with a bug is worse than none,
// because a harness that silently stops accepting connections or mis-scripts
// a connection would make Task 11's supervision tests fail in a way that
// looks like a supervisor bug rather than a harness bug. These tests pin the
// harness's own behaviour directly against real sockets so that failure mode
// is ruled out.
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::{sleep, timeout};
    use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

    use super::*;

    /// Connects to `url` and immediately sends `frame` as the subscription
    /// message, mirroring what a real client does on connect.
    async fn connect_and_subscribe(
        url: &str,
        frame: &str,
    ) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        let (mut ws, _) = connect_async(url).await.expect("connect");
        ws.send(Message::Text(frame.to_string().into()))
            .await
            .expect("send subscription frame");
        ws
    }

    #[tokio::test]
    async fn send_then_close_delivers_its_frames_in_order_then_closes() {
        let server = ScriptedServer::start(vec![Script::SendThenClose(vec![
            "frame-a".to_string(),
            "frame-b".to_string(),
        ])])
        .await;

        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        let mut frames = Vec::new();
        while let Some(Ok(message)) = ws.next().await {
            match message {
                Message::Text(text) => frames.push(text.to_string()),
                Message::Close(_) => break,
                other => panic!("unexpected message: {other:?}"),
            }
        }

        assert_eq!(frames, vec!["frame-a".to_string(), "frame-b".to_string()]);
    }

    #[tokio::test]
    async fn send_then_idle_delivers_its_frames_then_falls_silent() {
        let server =
            ScriptedServer::start(vec![Script::SendThenIdle(vec!["frame-c".to_string()])]).await;
        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        let first = timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("should receive the scripted frame promptly")
            .expect("stream item")
            .expect("ok message");
        match first {
            Message::Text(text) => assert_eq!(text.to_string(), "frame-c"),
            other => panic!("unexpected message: {other:?}"),
        }

        let silence = timeout(Duration::from_millis(300), ws.next()).await;
        assert!(
            silence.is_err(),
            "expected the connection to stay open and silent, got {silence:?}"
        );
    }

    #[tokio::test]
    async fn connections_beyond_the_script_list_repeat_the_last_script() {
        let server = ScriptedServer::start(vec![
            Script::SendThenClose(vec!["frame-a".to_string()]),
            Script::SendThenIdle(vec!["frame-c".to_string()]),
        ])
        .await;

        // Connections 1 and 2 consume the two scripted entries; connection 3
        // has nothing scripted for it and must repeat the last one
        // (SendThenIdle), which is how it differs from connection 1's close.
        for expected in ["frame-a", "frame-c", "frame-c"] {
            let mut ws = connect_and_subscribe(&server.url, "sub").await;
            let message = timeout(Duration::from_secs(2), ws.next())
                .await
                .expect("should receive a frame promptly")
                .expect("stream item")
                .expect("ok message");
            match message {
                Message::Text(text) => assert_eq!(text.to_string(), expected),
                other => panic!("unexpected message: {other:?}"),
            }
        }

        assert_eq!(server.connection_count(), 3);
    }

    #[tokio::test]
    async fn received_subscriptions_records_one_frame_per_connection_in_order() {
        let server = ScriptedServer::start(vec![Script::SendThenIdle(Vec::new())]).await;

        // Keep each socket alive; dropping it right after sending would race
        // the server's read against the client's teardown.
        let mut sockets = Vec::new();
        for frame in ["sub-1", "sub-2", "sub-3"] {
            sockets.push(connect_and_subscribe(&server.url, frame).await);
        }

        // The server records each frame on a task spawned per connection, so
        // give it a moment to catch up rather than asserting immediately.
        timeout(Duration::from_secs(2), async {
            while server.received_subscriptions().len() < 3 {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("server should record all three subscription frames promptly");

        assert_eq!(
            server.received_subscriptions(),
            vec![
                "sub-1".to_string(),
                "sub-2".to_string(),
                "sub-3".to_string()
            ]
        );
    }
}
