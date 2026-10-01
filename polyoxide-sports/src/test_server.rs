//! A local WebSocket server that plays a script per connection, for the
//! offline tests here and in `polyoxide-cli`. Behind `test-server`; not for
//! consumers.

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    time::Instant,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// How the server behaves on one connection.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Accept the TCP connection, then drop it without a handshake.
    pub reject_handshake: bool,
    /// Accept the TCP connection and never answer the handshake.
    pub stall_handshake: bool,
    /// Messages to send right after the handshake, in order.
    pub send: Vec<Message>,
    /// Send a close frame after `send`. Otherwise hold the connection open.
    pub close_after: bool,
    /// While holding the connection open, send a protocol ping this often.
    pub ping_every: Option<Duration>,
}

impl Script {
    /// Hold the connection open and send nothing at all.
    pub fn silent() -> Self {
        Self::default()
    }

    /// Send these text frames, then hold the connection open in silence.
    pub fn frames(frames: &[&str]) -> Self {
        Self {
            send: frames
                .iter()
                .map(|frame| Message::Text((*frame).into()))
                .collect(),
            ..Self::default()
        }
    }

    /// Hold the connection open, sending only protocol pings.
    pub fn pings_only(every: Duration) -> Self {
        Self {
            ping_every: Some(every),
            ..Self::default()
        }
    }

    /// Complete the handshake, then close at once.
    pub fn close_at_once() -> Self {
        Self {
            close_after: true,
            ..Self::default()
        }
    }

    /// Drop the TCP connection without a handshake.
    pub fn reject() -> Self {
        Self {
            reject_handshake: true,
            ..Self::default()
        }
    }

    /// Accept the TCP connection and never answer the handshake.
    pub fn stall() -> Self {
        Self {
            stall_handshake: true,
            ..Self::default()
        }
    }

    /// Close after sending, instead of holding the connection open.
    pub fn then_close(mut self) -> Self {
        self.close_after = true;
        self
    }
}

#[derive(Default)]
struct Recorder {
    accepted_at: Mutex<Vec<Instant>>,
    handshakes: AtomicUsize,
    pongs: Mutex<Vec<Vec<u8>>>,
    client_ended: AtomicUsize,
    close_replies: AtomicUsize,
}

/// A running local server.
pub struct ScriptedServer {
    /// The `ws://` URL to connect to.
    pub url: String,
    recorder: Arc<Recorder>,
}

impl ScriptedServer {
    /// Start a server that applies `scripts[n]` to the n-th connection,
    /// repeating the last script for every connection after it.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("ws://{}", listener.local_addr().expect("local address"));
        let recorder = Arc::new(Recorder::default());
        let shared = Arc::clone(&recorder);
        tokio::spawn(async move {
            let mut index = 0;
            while let Ok((stream, _)) = listener.accept().await {
                shared.accepted_at.lock().unwrap().push(Instant::now());
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or_default();
                index += 1;
                let recorder = Arc::clone(&shared);
                tokio::spawn(async move {
                    let _ = serve(stream, script, recorder).await;
                });
            }
        });
        Self { url, recorder }
    }

    /// Connections accepted, counted at TCP accept, before any handshake.
    pub fn connection_count(&self) -> usize {
        self.recorder.accepted_at.lock().unwrap().len()
    }

    /// When each connection was accepted, in order.
    pub fn accepted_at(&self) -> Vec<Instant> {
        self.recorder.accepted_at.lock().unwrap().clone()
    }

    /// Handshakes completed.
    pub fn handshake_count(&self) -> usize {
        self.recorder.handshakes.load(Ordering::SeqCst)
    }

    /// The payload of every pong received, in order, across connections.
    pub fn pongs(&self) -> Vec<Vec<u8>> {
        self.recorder.pongs.lock().unwrap().clone()
    }

    /// Held connections the client ended, by a close frame, end of stream,
    /// or a read error.
    pub fn client_ended_count(&self) -> usize {
        self.recorder.client_ended.load(Ordering::SeqCst)
    }

    /// Close frames the client sent in reply to the server's own.
    pub fn close_reply_count(&self) -> usize {
        self.recorder.close_replies.load(Ordering::SeqCst)
    }

    /// Poll until `predicate` holds, or panic naming `label` after five
    /// seconds.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if predicate(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("timed out waiting for {label}");
    }
}

async fn serve(
    stream: TcpStream,
    script: Script,
    recorder: Arc<Recorder>,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    if script.reject_handshake {
        drop(stream);
        return Ok(());
    }
    if script.stall_handshake {
        // Hold the socket without reading it until the test ends.
        let _held = stream;
        std::future::pending::<()>().await;
        return Ok(());
    }
    let mut ws = accept_async(stream).await?;
    recorder.handshakes.fetch_add(1, Ordering::SeqCst);
    for message in script.send {
        ws.send(message).await?;
    }
    if script.close_after {
        let _ = ws.close(None).await;
        // RFC 6455 requires the client to answer with its own close frame.
        if let Ok(Some(Ok(Message::Close(_)))) =
            tokio::time::timeout(Duration::from_secs(1), ws.next()).await
        {
            recorder.close_replies.fetch_add(1, Ordering::SeqCst);
        }
        return Ok(());
    }
    let mut pings = script
        .ping_every
        .map(|every| tokio::time::interval_at(Instant::now() + every, every));
    let mut sequence: u32 = 0;
    loop {
        tokio::select! {
            _ = async {
                match pings.as_mut() {
                    Some(interval) => {
                        interval.tick().await;
                    }
                    None => std::future::pending::<()>().await,
                }
            } => {
                sequence += 1;
                ws.send(Message::Ping(sequence.to_be_bytes().to_vec().into())).await?;
            }
            message = ws.next() => match message {
                Some(Ok(Message::Pong(payload))) => {
                    recorder.pongs.lock().unwrap().push(payload.to_vec());
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                    recorder.client_ended.fetch_add(1, Ordering::SeqCst);
                    return Ok(());
                }
                Some(Ok(_)) => {}
            },
        }
    }
}
