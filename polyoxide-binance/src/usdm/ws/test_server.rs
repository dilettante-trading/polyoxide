//! A local server speaking Binance's combined-stream protocol from a script,
//! for the offline tests here and downstream. Behind `test-server`.
//!
//! It accepts any path, answers `SUBSCRIBE`, `UNSUBSCRIBE` and
//! `LIST_SUBSCRIPTIONS` as Binance does, and answers protocol pings while it
//! reads.

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    net::{TcpListener, TcpStream},
    time::Instant,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{Request, Response},
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
};

/// How the server behaves on one connection.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Frames to send, in order, after answering the first request.
    pub pushes: Vec<String>,
    /// While the connection is held open, resend the first push on this
    /// cadence: steady traffic.
    pub push_every: Option<Duration>,
    /// Close the connection after the pushes.
    pub close_after: bool,
    /// The close code and reason to send when closing.
    pub close_code: Option<(u16, String)>,
    /// Close the connection this long after the handshake, whatever else is
    /// happening.
    pub drop_after: Option<Duration>,
    /// After the pushes, stop reading and writing for good: a dead socket that
    /// answers neither requests nor pings.
    pub go_silent: bool,
    /// Send a protocol ping on this cadence while the connection is held open.
    pub ping_every: Option<Duration>,
    /// A request naming one of these streams is answered with
    /// `{"error": {"code", "msg"}}`.
    pub refuse: Vec<(String, i64, String)>,
    /// Leave every request after the first unanswered.
    pub ignore_later_requests: bool,
    /// Leave the request with this id unanswered until the next request
    /// arrives; then send this frame, the held answer and that request's
    /// answer, in that order: an answer that arrives after its caller gave up.
    pub hold_answer: Option<(u64, String)>,
    /// Accept the TCP connection and drop it without a handshake.
    pub reject_handshake: bool,
}

/// One text frame the client sent.
#[derive(Debug, Clone)]
pub struct Received {
    /// Which connection, counted from zero.
    pub connection: usize,
    /// When it arrived.
    pub at: Instant,
    /// The frame, parsed.
    pub request: Value,
}

#[derive(Default)]
struct Recorder {
    paths: Mutex<Vec<String>>,
    received: Mutex<Vec<Received>>,
    pings: AtomicUsize,
    closes: AtomicUsize,
    ended: AtomicUsize,
}

/// A running local server.
pub struct ScriptedServer {
    /// The base URL to connect to, `ws://127.0.0.1:<port>`.
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
        let (count, rec) = (Arc::clone(&connections), Arc::clone(&recorder));
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let index = count.fetch_add(1, Ordering::SeqCst);
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or_default();
                let recorder = Arc::clone(&rec);
                tokio::spawn(async move {
                    let _ = serve(stream, index, script, &recorder).await;
                    recorder.ended.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        Self {
            url: format!("ws://{addr}"),
            connections,
            recorder,
        }
    }

    /// Connections accepted so far, counted before the handshake.
    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// The request path of each connection that completed a handshake.
    pub fn paths(&self) -> Vec<String> {
        self.recorder.paths.lock().unwrap().clone()
    }

    /// Every request received, in arrival order, across connections.
    pub fn received(&self) -> Vec<Received> {
        self.recorder.received.lock().unwrap().clone()
    }

    /// Every request received, parsed.
    pub fn requests(&self) -> Vec<Value> {
        self.received().into_iter().map(|r| r.request).collect()
    }

    /// Protocol pings received from the client.
    pub fn ping_count(&self) -> usize {
        self.recorder.pings.load(Ordering::SeqCst)
    }

    /// Connections the client closed with a Close frame.
    pub fn close_count(&self) -> usize {
        self.recorder.closes.load(Ordering::SeqCst)
    }

    /// Connections that have ended, however they ended.
    pub fn ended_count(&self) -> usize {
        self.recorder.ended.load(Ordering::SeqCst)
    }

    /// Poll until `predicate` holds, or panic with `label` after two seconds.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if predicate(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("timed out waiting for {label}");
    }
}

/// The answer Binance gives to one request, given the connection's streams.
fn answer(script: &Script, request: &Value, streams: &mut Vec<String>) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params: Vec<String> = request["params"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(|n| n.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if let Some((_, code, msg)) = script
        .refuse
        .iter()
        .find(|(name, _, _)| params.contains(name))
    {
        return json!({ "error": { "code": code, "msg": msg }, "id": id });
    }
    match request["method"].as_str() {
        Some("SUBSCRIBE") => {
            for name in params {
                if !streams.contains(&name) {
                    streams.push(name);
                }
            }
            json!({ "result": null, "id": id })
        }
        Some("UNSUBSCRIBE") => {
            streams.retain(|name| !params.contains(name));
            json!({ "result": null, "id": id })
        }
        Some("LIST_SUBSCRIPTIONS") => json!({ "result": streams, "id": id }),
        _ => json!({ "error": { "code": 2, "msg": "Invalid request" }, "id": id }),
    }
}

// The handshake callback's error type is tungstenite's, fixed by its signature.
#[allow(clippy::result_large_err)]
async fn serve(
    stream: TcpStream,
    index: usize,
    script: Script,
    recorder: &Recorder,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if script.reject_handshake {
        drop(stream);
        return Ok(());
    }
    let path = Arc::new(Mutex::new(String::new()));
    let seen = Arc::clone(&path);
    let mut ws = accept_hdr_async(stream, move |request: &Request, response: Response| {
        *seen.lock().unwrap() = request.uri().path().to_owned();
        Ok(response)
    })
    .await?;
    recorder
        .paths
        .lock()
        .unwrap()
        .push(path.lock().unwrap().clone());

    let close_frame = script.close_code.clone().map(|(code, reason)| CloseFrame {
        code: CloseCode::from(code),
        reason: reason.into(),
    });
    let drop_at = script.drop_after.map(|after| Instant::now() + after);
    let mut streams: Vec<String> = Vec::new();
    let mut answered = 0usize;
    let mut pushed = false;
    let mut push_every = script.push_every.map(tokio::time::interval);
    let mut ping_every = script.ping_every.map(tokio::time::interval);
    let mut held: Option<Value> = None;

    loop {
        tokio::select! {
            () = async {
                match drop_at {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending().await,
                }
            } => {
                let _ = ws.close(close_frame.clone()).await;
                return Ok(());
            }
            _ = async {
                match ping_every.as_mut() {
                    Some(interval) => { interval.tick().await; }
                    None => std::future::pending::<()>().await,
                }
            } => {
                ws.send(Message::Ping(Vec::new().into())).await?;
            }
            _ = async {
                match push_every.as_mut() {
                    Some(interval) if pushed => { interval.tick().await; }
                    _ => std::future::pending::<()>().await,
                }
            } => {
                if let Some(first) = script.pushes.first() {
                    ws.send(Message::Text(first.clone().into())).await?;
                }
            }
            message = ws.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    let Ok(request) = serde_json::from_str::<Value>(&text) else { continue };
                    recorder.received.lock().unwrap().push(Received {
                        connection: index,
                        at: Instant::now(),
                        request: request.clone(),
                    });
                    if answered > 0 && script.ignore_later_requests {
                        continue;
                    }
                    let reply = answer(&script, &request, &mut streams);
                    if let Some((held_id, before)) = &script.hold_answer {
                        if request.get("id").and_then(Value::as_u64) == Some(*held_id) {
                            held = Some(reply);
                            continue;
                        }
                        if let Some(late) = held.take() {
                            ws.send(Message::Text(before.clone().into())).await?;
                            ws.send(Message::Text(late.to_string().into())).await?;
                        }
                    }
                    ws.send(Message::Text(reply.to_string().into())).await?;
                    answered += 1;
                    if answered == 1 {
                        for push in &script.pushes {
                            ws.send(Message::Text(push.clone().into())).await?;
                        }
                        pushed = true;
                        if script.close_after {
                            let _ = ws.close(close_frame.clone()).await;
                            return Ok(());
                        }
                        if script.go_silent {
                            // Hold the socket without reading it: nothing is
                            // answered, pings included.
                            std::future::pending::<()>().await;
                        }
                    }
                }
                Some(Ok(Message::Ping(_))) => {
                    recorder.pings.fetch_add(1, Ordering::SeqCst);
                }
                Some(Ok(Message::Close(_))) => {
                    recorder.closes.fetch_add(1, Ordering::SeqCst);
                    return Ok(());
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return Ok(()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_list_and_unsubscribe_answer_as_binance_does() {
        let script = Script::default();
        let mut streams = Vec::new();
        let sub = json!({"method": "SUBSCRIBE", "params": ["btcusdt@aggTrade", "ethusdt@aggTrade"], "id": 1});
        assert_eq!(
            answer(&script, &sub, &mut streams),
            json!({"result": null, "id": 1})
        );
        let unsub = json!({"method": "UNSUBSCRIBE", "params": ["btcusdt@aggTrade"], "id": 2});
        answer(&script, &unsub, &mut streams);
        let list = json!({"method": "LIST_SUBSCRIPTIONS", "id": 3});
        assert_eq!(
            answer(&script, &list, &mut streams),
            json!({"result": ["ethusdt@aggTrade"], "id": 3})
        );
    }

    #[test]
    fn a_scripted_refusal_is_an_error_answer() {
        let script = Script {
            refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
            ..Script::default()
        };
        let sub = json!({"method": "SUBSCRIBE", "params": ["ethusdt@aggTrade"], "id": 9});
        assert_eq!(
            answer(&script, &sub, &mut Vec::new()),
            json!({"error": {"code": 2, "msg": "Invalid request"}, "id": 9})
        );
    }
}
