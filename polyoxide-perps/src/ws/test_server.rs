//! A local server speaking the perps WebSocket protocol from a script, for
//! the offline tests. Behind `test-server` so integration tests can use it.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// How the server behaves on one connection.
#[derive(Debug, Clone)]
pub struct Script {
    /// Channels to refuse, with the identifier to answer; every other
    /// channel is answered `ok`.
    pub refuse: Vec<(String, String)>,
    /// Push frames to send after the first subscribe response, in order.
    pub pushes: Vec<String>,
    /// Whether to close the connection after the pushes (else hold it open).
    pub close_after: bool,
    /// Whether to answer pings. `false` models a dead socket.
    pub answer_pings: bool,
    /// Accept the TCP connection and drop it without a handshake.
    pub reject_handshake: bool,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            refuse: Vec::new(),
            pushes: Vec::new(),
            close_after: false,
            answer_pings: true,
            reject_handshake: false,
        }
    }
}

#[derive(Default)]
struct Recorder {
    /// Every text frame received, in arrival order, across connections.
    frames: Mutex<Vec<String>>,
    /// The first frame of each connection: its subscription.
    subscriptions: Mutex<Vec<String>>,
    closes: AtomicUsize,
}

/// A running local server.
pub struct ScriptedServer {
    /// The `ws://` URL to connect to.
    pub url: String,
    connections: Arc<AtomicUsize>,
    recorder: Arc<Recorder>,
}

impl ScriptedServer {
    /// Apply `scripts[n]` to the n-th connection, repeating the last.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let connections = Arc::new(AtomicUsize::new(0));
        let recorder = Arc::new(Recorder::default());
        let (c, r) = (Arc::clone(&connections), Arc::clone(&recorder));
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let index = c.fetch_add(1, Ordering::SeqCst);
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or_default();
                let recorder = Arc::clone(&r);
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

    /// Connections accepted so far.
    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// Every text frame received, in order.
    pub fn client_frames(&self) -> Vec<String> {
        self.recorder.frames.lock().unwrap().clone()
    }

    /// The first frame of each connection.
    pub fn subscriptions(&self) -> Vec<String> {
        self.recorder.subscriptions.lock().unwrap().clone()
    }

    /// Connections the client closed with a Close frame.
    pub fn close_count(&self) -> usize {
        self.recorder.closes.load(Ordering::SeqCst)
    }

    /// Poll until `predicate` holds, or panic with `label` after two seconds.
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

/// Answer one request the way the host does.
fn answer(script: &Script, request: &serde_json::Value) -> Option<String> {
    let id = request
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    match request.get("req").and_then(|r| r.as_str()) {
        Some("sub") | Some("unsub") => {
            let statuses: Vec<serde_json::Value> = request["chs"]
                .as_array()
                .map(|chs| {
                    chs.iter()
                        .map(|c| {
                            match script
                                .refuse
                                .iter()
                                .find(|(name, _)| Some(name.as_str()) == c.as_str())
                            {
                                Some((_, error)) => {
                                    serde_json::json!({"status": "err", "error": error})
                                }
                                None => serde_json::json!({"status": "ok"}),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(serde_json::json!({"id": id, "data": statuses}).to_string())
        }
        Some("post") if script.answer_pings => Some(
            serde_json::json!({"id": id, "ts": 1, "data": {"status": "ok", "ts": 1, "sq": 1}})
                .to_string(),
        ),
        _ => None,
    }
}

async fn serve(
    stream: TcpStream,
    script: Script,
    recorder: Arc<Recorder>,
) -> Result<(), Box<dyn std::error::Error>> {
    if script.reject_handshake {
        drop(stream);
        return Ok(());
    }
    let mut ws = accept_async(stream).await?;

    // The client subscribes immediately. Record before answering so a test
    // that has seen the response knows the subscription is recorded.
    let first = loop {
        match ws.next().await {
            Some(Ok(Message::Text(text))) => break text.to_string(),
            Some(Ok(_)) => continue,
            _ => return Ok(()),
        }
    };
    recorder.subscriptions.lock().unwrap().push(first.clone());
    recorder.frames.lock().unwrap().push(first.clone());
    if let Some(reply) = answer(&script, &serde_json::from_str(&first)?) {
        ws.send(Message::Text(reply.into())).await?;
    }
    for push in &script.pushes {
        ws.send(Message::Text(push.clone().into())).await?;
    }
    if script.close_after {
        let _ = ws.close(None).await;
        return Ok(());
    }
    // Hold open: answer later control frames, count closes.
    while let Some(message) = ws.next().await {
        match message {
            Ok(Message::Text(text)) => {
                let text = text.to_string();
                recorder.frames.lock().unwrap().push(text.clone());
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(reply) = answer(&script, &value) {
                        ws.send(Message::Text(reply.into())).await?;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                recorder.closes.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
            Ok(_) => continue,
            Err(_) => return Ok(()),
        }
    }
    Ok(())
}
