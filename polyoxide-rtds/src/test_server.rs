//! A local WebSocket server for testing client and supervision behaviour.
//!
//! Lives in `src/` rather than `tests/` for the same reason
//! [`fixtures`](crate::fixtures) does: the behaviours worth testing here reach
//! `pub(crate)` items such as [`Rtds::connect_to`](crate::client::Rtds), and
//! `tests/` is a separate crate that cannot see them. Gating it on
//! `test-fixtures` lets the integration tests under `tests/` use the same
//! single copy.
//!
//! It exists because the behaviours the client and supervisor add — reconnect,
//! staleness detection, resubscribe, keep-alive, skipping a bad frame — cannot
//! be triggered on demand against the real host.

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
    /// Send these text frames, then hold the connection open and silent.
    SendThenIdle(Vec<String>),
    /// Send these text frames, then close the connection.
    SendThenClose(Vec<String>),
    /// Accept the TCP connection and drop it without completing the WebSocket
    /// handshake.
    ///
    /// Models a host that is reachable but unwell — the case the reconnect
    /// loop's own retry arm exists for. A refused *connection attempt* is a
    /// different path from a connection that opened and then failed.
    RejectHandshake,
    /// Send these raw WebSocket messages, then hold open and silent.
    ///
    /// The only way to drive the non-text arms of the client's `Stream` impl:
    /// a server-initiated Ping or a Binary frame must be skipped, not treated
    /// as the end of the stream.
    SendRawThenIdle(Vec<Message>),
}

/// What the server observed, shared with the accept loop's tasks.
#[derive(Default)]
struct Recorder {
    /// The first text frame of each connection — the subscription.
    subscriptions: Mutex<Vec<String>>,
    /// Every text frame received on any connection, in arrival order.
    all_frames: Mutex<Vec<String>>,
    /// How many connections the client closed with a Close frame.
    closes: AtomicUsize,
}

impl Recorder {
    /// Records into both lists: the subscription is also a client frame.
    ///
    /// The `expect`s below cannot fire in practice — poisoning would require a
    /// panic inside a `Vec<String>` push or clone.
    fn record_subscription(&self, frame: String) {
        self.subscriptions
            .lock()
            .expect("subscriptions mutex poisoned")
            .push(frame.clone());
        self.record_frame(frame);
    }

    fn record_frame(&self, frame: String) {
        self.all_frames
            .lock()
            .expect("all_frames mutex poisoned")
            .push(frame);
    }
}

/// A running local server.
pub struct ScriptedServer {
    /// The `ws://` URL clients should connect to.
    pub url: String,
    /// How many connections have been accepted so far.
    connections: Arc<AtomicUsize>,
    recorder: Arc<Recorder>,
}

impl ScriptedServer {
    /// Start a server that applies `scripts[n]` to the n-th connection,
    /// repeating the last script for any further connections.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let connections = Arc::new(AtomicUsize::new(0));
        let recorder = Arc::new(Recorder::default());

        let task_connections = Arc::clone(&connections);
        let task_recorder = Arc::clone(&recorder);
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
                let recorder = Arc::clone(&task_recorder);
                tokio::spawn(async move {
                    let _ = serve(stream, script, recorder).await;
                });
            }
        });

        Self {
            url: format!("ws://{addr}"),
            connections,
            recorder,
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
    /// which works because `serve` records the subscription *before* it
    /// sends anything.
    pub fn received_subscriptions(&self) -> Vec<String> {
        self.recorder
            .subscriptions
            .lock()
            .expect("subscriptions mutex poisoned")
            .clone()
    }

    /// Every text frame the server received, in arrival order.
    ///
    /// Includes each connection's subscription frame, so this is a superset of
    /// [`received_subscriptions`](Self::received_subscriptions). Anything a
    /// client sends *after* subscribing — a `PING`, a second subscribe frame —
    /// only appears here. Carries the same synchronisation caveat.
    pub fn client_frames(&self) -> Vec<String> {
        self.recorder
            .all_frames
            .lock()
            .expect("all_frames mutex poisoned")
            .clone()
    }

    /// How many connections the client closed with a Close frame.
    pub fn close_count(&self) -> usize {
        self.recorder.closes.load(Ordering::SeqCst)
    }

    /// Wait until `predicate` holds, polling rather than sleeping a fixed
    /// amount.
    ///
    /// Panics with `label` if it has not held within two seconds. Every
    /// observation this harness offers lands asynchronously in a per-connection
    /// task, so a bare assert after an action races that task; sleeping a fixed
    /// amount instead just trades a race for a slow test that still flakes.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while tokio::time::Instant::now() < deadline {
            if predicate(self) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("timed out waiting for {label}");
    }
}

async fn serve(
    stream: TcpStream,
    script: Script,
    recorder: Arc<Recorder>,
) -> Result<(), Box<dyn std::error::Error>> {
    if matches!(script, Script::RejectHandshake) {
        drop(stream);
        return Ok(());
    }

    let mut ws = accept_async(stream).await?;

    // The client sends its subscription frame immediately on connect.
    //
    // Recording it BEFORE sending any scripted frames is load-bearing, not
    // incidental. It is what lets a test treat "the client saw a frame from
    // us" as proof that the subscription was already recorded — which is how
    // the supervision tests avoid racing `received_subscriptions()` against
    // `connection_count()`. Do not move this below the send loop.
    if let Some(Ok(Message::Text(frame))) = ws.next().await {
        recorder.record_subscription(frame.to_string());
    }

    let (messages, close_after) = match script {
        Script::SendThenIdle(frames) => (text_messages(frames), false),
        Script::SendThenClose(frames) => (text_messages(frames), true),
        Script::SendRawThenIdle(messages) => (messages, false),
        // Handled above, before the handshake.
        Script::RejectHandshake => unreachable!(),
    };

    for message in messages {
        ws.send(message).await?;
    }

    if close_after {
        ws.close(None).await?;
        return Ok(());
    }

    // Hold open and silent, recording whatever the client sends. This is what
    // a stalled-but-not-closed feed looks like, and it is where a `PING` or a
    // second subscribe frame is observed.
    while let Some(Ok(message)) = ws.next().await {
        match message {
            Message::Close(_) => {
                recorder.closes.fetch_add(1, Ordering::SeqCst);
                break;
            }
            Message::Text(text) => recorder.record_frame(text.to_string()),
            _ => {}
        }
    }
    Ok(())
}

fn text_messages(frames: Vec<String>) -> Vec<Message> {
    frames
        .into_iter()
        .map(|f| Message::Text(f.into()))
        .collect()
}

// Infrastructure with a bug is worse than none, because a harness that
// silently stops accepting connections, mis-scripts a connection or drops a
// frame it claims to record would make the client and supervision tests fail
// in a way that looks like a library bug rather than a harness bug. These
// tests pin the harness's own behaviour directly against real sockets so that
// failure mode is ruled out.
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;
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

        server
            .wait_for("all three subscription frames", |s| {
                s.received_subscriptions().len() == 3
            })
            .await;

        assert_eq!(
            server.received_subscriptions(),
            vec![
                "sub-1".to_string(),
                "sub-2".to_string(),
                "sub-3".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn client_frames_records_what_a_client_sends_after_subscribing() {
        // `received_subscriptions` deliberately keeps only the first frame per
        // connection, so a test for `ping` or `subscribe_more` would have
        // nothing to assert against without this second list.
        let server = ScriptedServer::start(vec![Script::SendThenIdle(Vec::new())]).await;
        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        for frame in ["PING", "sub-2"] {
            ws.send(Message::Text(frame.to_string().into()))
                .await
                .expect("send");
        }

        server
            .wait_for("three client frames", |s| s.client_frames().len() == 3)
            .await;

        assert_eq!(
            server.client_frames(),
            vec!["sub-1".to_string(), "PING".to_string(), "sub-2".to_string()],
            "client_frames must include the subscription and everything after it"
        );
        assert_eq!(
            server.received_subscriptions(),
            vec!["sub-1".to_string()],
            "only the first frame of a connection is its subscription"
        );
    }

    #[tokio::test]
    async fn close_count_observes_a_client_initiated_close() {
        let server = ScriptedServer::start(vec![Script::SendThenIdle(Vec::new())]).await;
        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        assert_eq!(server.close_count(), 0);
        ws.close(None).await.expect("close");

        server
            .wait_for("a close frame", |s| s.close_count() == 1)
            .await;
    }

    #[tokio::test]
    async fn reject_handshake_fails_the_connection_attempt_recoverably() {
        // The retry arm it exists for only runs for `Recovery::Reconnect`, so
        // a rejection that classified as Fatal would make the supervisor
        // return instead of retrying — and the test using it would pass for
        // the wrong reason.
        let server = ScriptedServer::start(vec![Script::RejectHandshake]).await;

        // `Rtds` is not `Debug`, so `expect_err` is unavailable here.
        let Err(err) = crate::client::Rtds::connect_to(
            &server.url,
            crate::subscription::Subscription::for_topic(crate::topic::Topic::BinanceSpot)
                .symbols(["btcusdt"]),
        )
        .await
        else {
            panic!("a dropped handshake must not succeed");
        };

        assert_eq!(err.recovery(), crate::error::Recovery::Reconnect, "{err:?}");
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn send_raw_then_idle_delivers_non_text_messages() {
        let server = ScriptedServer::start(vec![Script::SendRawThenIdle(vec![
            Message::Ping(vec![7u8].into()),
            Message::Binary(vec![1u8, 2, 3].into()),
            Message::Text("frame-d".into()),
        ])])
        .await;

        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        // tokio-tungstenite answers the Ping itself but still surfaces it, so
        // all three arrive. The client under test must skip the first two.
        let mut kinds = Vec::new();
        for _ in 0..3 {
            let message = timeout(Duration::from_secs(2), ws.next())
                .await
                .expect("frame should arrive promptly")
                .expect("stream item")
                .expect("ok message");
            kinds.push(match message {
                Message::Ping(_) => "ping",
                Message::Binary(_) => "binary",
                Message::Text(_) => "text",
                other => panic!("unexpected message: {other:?}"),
            });
        }
        assert_eq!(kinds, vec!["ping", "binary", "text"]);
    }
}
