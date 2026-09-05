//! A local WebSocket server for testing supervision behaviour.
//!
//! Not a test file in its own right — `supervision.rs` includes it with
//! `#[path]`. It exists because the behaviours tier 2 adds (reconnect,
//! staleness detection, resubscribe) cannot be triggered on demand against the
//! real host.

#![allow(dead_code)]

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
    pub async fn received_subscriptions(&self) -> Vec<String> {
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
