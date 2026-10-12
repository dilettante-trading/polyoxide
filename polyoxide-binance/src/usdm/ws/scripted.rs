//! The Binance suites' scripts, played on the shared scripted server
//! (`polyoxide_ws::test_server`) with Binance's answer.
//!
//! A [`Script`] keeps the field names of Binance's old server, so the suites'
//! bodies did not change when they moved onto the shared one. Each becomes a
//! shared `Script` for what the server sends, and the answer closure's state
//! for how it answers: the streams the connection carries, refusals, whether
//! requests after the first are answered, and an answer held back, per
//! connection, the last repeating.
//!
//! One file serves both the unit tests (`#[cfg(test)]`) and the integration
//! suites, which include it with `#[path]`, so it names nothing of this
//! crate's own. Each suite uses part of it.
#![allow(dead_code)]

use std::time::Duration;

use polyoxide_ws::{
    test_server::{Script as Shared, ScriptedServer as Server},
    tungstenite::Message,
};
use serde_json::{json, Value};
use tokio::time::Instant;

/// How the server behaves on one connection.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Frames to send, in order, once the client's first frame has arrived
    /// and been answered as the script answers it, which may be not at all.
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
    /// Answer the WebSocket upgrade with this HTTP status and these headers,
    /// such as `Retry-After`, instead of `101`.
    pub refuse_upgrade: Option<(u16, Vec<(String, String)>)>,
}

impl Script {
    /// What the shared server sends on the connection: the pushes after the
    /// client's first frame and the answer the script gives it, if any.
    fn shared(&self) -> Shared {
        if self.reject_handshake {
            return Shared::reject();
        }
        if let Some((status, headers)) = &self.refuse_upgrade {
            return headers
                .iter()
                .fold(Shared::refuse_upgrade(*status), |shared, (name, value)| {
                    shared.with_header(name, value)
                });
        }
        let pushes: Vec<&str> = self.pushes.iter().map(String::as_str).collect();
        let mut shared = Shared::frames(&pushes).after_first_frame();
        if let Some(every) = self.push_every {
            shared = shared.push_every(every);
        }
        if self.close_after {
            shared = shared.then_close();
        }
        if let Some((code, reason)) = &self.close_code {
            shared = shared.close_with(*code, reason);
        }
        if let Some(after) = self.drop_after {
            shared = shared.drop_after(after);
        }
        if self.go_silent {
            shared = shared.go_silent();
        }
        if let Some(every) = self.ping_every {
            shared = shared.ping_every(every);
        }
        shared
    }
}

/// One request the client sent.
#[derive(Debug, Clone)]
pub struct Received {
    /// Which connection, counted from zero.
    pub connection: usize,
    /// When it arrived.
    pub at: Instant,
    /// The frame, parsed.
    pub request: Value,
}

/// A running shared server, answering as Binance does.
pub struct ScriptedServer {
    /// The base URL to connect to, `ws://127.0.0.1:<port>`.
    pub url: String,
    inner: Server,
}

impl ScriptedServer {
    /// Apply `scripts[n]` to the n-th connection, repeating the last.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let shared = scripts.iter().map(Script::shared).collect();
        let inner = Server::start_answering(shared, move |index| {
            let script = scripts
                .get(index)
                .or_else(|| scripts.last())
                .cloned()
                .unwrap_or_default();
            answer(script)
        })
        .await;
        Self {
            url: inner.url.clone(),
            inner,
        }
    }

    /// Connections accepted so far, counted before the handshake.
    pub fn connection_count(&self) -> usize {
        self.inner.connection_count()
    }

    /// When each connection was accepted, in order.
    pub fn accepted_at(&self) -> Vec<Instant> {
        self.inner.accepted_at()
    }

    /// The request path of each connection that completed a handshake.
    pub fn paths(&self) -> Vec<String> {
        self.inner.paths()
    }

    /// Every request received, in arrival order, across connections.
    pub fn received(&self) -> Vec<Received> {
        self.inner
            .received()
            .into_iter()
            .map(|r| Received {
                connection: r.connection,
                at: r.at,
                request: serde_json::from_str(&r.text)
                    .unwrap_or_else(|err| panic!("a request that is not JSON ({err}): {}", r.text)),
            })
            .collect()
    }

    /// Every request received, parsed.
    pub fn requests(&self) -> Vec<Value> {
        self.received().into_iter().map(|r| r.request).collect()
    }

    /// Protocol pings received from the client.
    pub fn ping_count(&self) -> usize {
        self.inner.ping_count()
    }

    /// Connections the client closed with a Close frame.
    pub fn close_count(&self) -> usize {
        self.inner.close_count()
    }

    /// Close frames the client sent in reply to the server's own.
    pub fn close_reply_count(&self) -> usize {
        self.inner.close_reply_count()
    }

    /// Connections that have ended, however they ended.
    pub fn ended_count(&self) -> usize {
        self.inner.ended_count()
    }

    /// Poll until `predicate` holds, or panic with `label` after five seconds.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        self.inner.wait_for(label, |_| predicate(self)).await;
    }
}

/// One connection's answerer: Binance's answer to `SUBSCRIBE`, `UNSUBSCRIBE`
/// and `LIST_SUBSCRIPTIONS`, given the streams the connection carries, with
/// the script's refusals, its silence after the first request, and its held
/// answer.
fn answer(script: Script) -> impl FnMut(&str) -> Vec<Message> + Send {
    let mut streams: Vec<String> = Vec::new();
    let mut answered = 0usize;
    let mut held: Option<Value> = None;
    move |text| {
        let Ok(request) = serde_json::from_str::<Value>(text) else {
            return Vec::new();
        };
        if answered > 0 && script.ignore_later_requests {
            return Vec::new();
        }
        let reply = reply(&script, &request, &mut streams);
        let mut frames = Vec::new();
        if let Some((held_id, before)) = &script.hold_answer {
            if request.get("id").and_then(Value::as_u64) == Some(*held_id) {
                held = Some(reply);
                return Vec::new();
            }
            if let Some(late) = held.take() {
                frames.push(Message::Text(before.clone().into()));
                frames.push(Message::Text(late.to_string().into()));
            }
        }
        frames.push(Message::Text(reply.to_string().into()));
        answered += 1;
        frames
    }
}

/// The answer Binance gives to one request, given the connection's streams.
fn reply(script: &Script, request: &Value, streams: &mut Vec<String>) -> Value {
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
