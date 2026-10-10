//! The perps suites' scripts, played on the shared scripted server
//! (`polyoxide_ws::test_server`) with perps' answer.
//!
//! A [`Script`] keeps the field names of perps' old server, so the suites'
//! bodies did not change when they moved onto the shared one. Each becomes a
//! shared `Script` for what the server sends, and the answer closure's state
//! for how it answers: refusals, whether pings are answered, the pong's status
//! and whether a membership change after the first is answered, per
//! connection, the last repeating.

use std::time::Duration;

use polyoxide_ws::{
    test_server::{Script as Shared, ScriptedServer},
    tungstenite::Message,
};
use serde_json::{json, Value};

/// How the server behaves on one connection.
#[derive(Debug, Clone)]
pub(crate) struct Script {
    /// Channels to refuse, with the identifier to answer; every other
    /// channel is answered `ok`.
    pub refuse: Vec<(String, String)>,
    /// Push frames to send after the first subscribe response, in order.
    pub pushes: Vec<String>,
    /// Raw WebSocket messages to send after `pushes`: the only way to drive
    /// the non-text arms of the client's `Stream` impl.
    pub raw_pushes: Vec<Message>,
    /// While the connection is held open, resend the first entry of `pushes`
    /// on this cadence: steady traffic.
    pub push_every: Option<Duration>,
    /// Whether to close the connection after the pushes (else hold it open).
    pub close_after: bool,
    /// Whether to answer pings. `false` models a dead socket.
    pub answer_pings: bool,
    /// The `status` of every pong; anything but `"ok"` also carries an
    /// `error`, modelling a control channel that answers but faults.
    pub pong_status: &'static str,
    /// Whether to answer `sub`/`unsub` requests after the first one. `false`
    /// models a socket that keeps a membership change hanging.
    pub answer_subscribes_after_first: bool,
    /// Accept the TCP connection and drop it without a handshake.
    pub reject_handshake: bool,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            refuse: Vec::new(),
            pushes: Vec::new(),
            raw_pushes: Vec::new(),
            push_every: None,
            close_after: false,
            answer_pings: true,
            pong_status: "ok",
            answer_subscribes_after_first: true,
            reject_handshake: false,
        }
    }
}

impl Script {
    /// What the shared server sends on the connection: the pushes once the
    /// client's first frame, its subscription, has been answered.
    fn shared(&self) -> Shared {
        if self.reject_handshake {
            return Shared::reject();
        }
        let mut send: Vec<Message> = self
            .pushes
            .iter()
            .map(|push| Message::Text(push.clone().into()))
            .collect();
        send.extend(self.raw_pushes.iter().cloned());
        let mut shared = Shared::messages(send).after_first_frame();
        if let Some(every) = self.push_every {
            shared = shared.push_every(every);
        }
        if self.close_after {
            shared = shared.then_close();
        }
        shared
    }
}

/// Start the shared server playing `scripts`, the n-th on the n-th
/// connection and the last repeating, each answered as perps' host answers.
pub(crate) async fn start(scripts: Vec<Script>) -> ScriptedServer {
    let shared = scripts.iter().map(Script::shared).collect();
    ScriptedServer::start_answering(shared, move |index| {
        let script = scripts
            .get(index)
            .or_else(|| scripts.last())
            .cloned()
            .unwrap_or_default();
        answer(script)
    })
    .await
}

/// One connection's answerer: a `sub`/`unsub` gets one status per channel in
/// request order (after the first, only if the script answers them); a
/// `post` whose `op.type` is `ping` gets a pong, unless the script says not
/// to answer pings; any other `post`, and anything else, gets no reply.
pub(crate) fn answer(script: Script) -> impl FnMut(&str) -> Vec<Message> + Send {
    let mut answered_first = false;
    move |request| {
        let first = !answered_first;
        answered_first = true;
        let Ok(request) = serde_json::from_str::<Value>(request) else {
            return Vec::new();
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let reply = match request.get("req").and_then(Value::as_str) {
            Some("sub" | "unsub") if first || script.answer_subscribes_after_first => {
                let statuses: Vec<Value> = request["chs"]
                    .as_array()
                    .map(|chs| {
                        chs.iter()
                            .map(|c| {
                                match script
                                    .refuse
                                    .iter()
                                    .find(|(name, _)| Some(name.as_str()) == c.as_str())
                                {
                                    Some((_, error)) => json!({"status": "err", "error": error}),
                                    None => json!({"status": "ok"}),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                json!({"id": id, "data": statuses})
            }
            Some("post")
                if script.answer_pings && request["op"]["type"].as_str() == Some("ping") =>
            {
                let data = if script.pong_status == "ok" {
                    json!({"status": "ok", "ts": 1, "sq": 1})
                } else {
                    json!({"status": script.pong_status, "error": "x"})
                };
                json!({"id": id, "ts": 1, "data": data})
            }
            _ => return Vec::new(),
        };
        vec![Message::Text(reply.to_string().into())]
    }
}
