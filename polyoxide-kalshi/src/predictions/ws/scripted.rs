//! The Kalshi suites' scripts, played on the shared scripted server
//! (`polyoxide_ws::test_server`) with Kalshi's answer.
//!
//! A `Script` says what the server sends besides its answers and how it ends a
//! connection, and injects faults; the answer closure keeps each connection's
//! subscriptions and sequence numbers, as Kalshi does: `sid`s count from 1 per
//! connection, and every sequenced frame of a subscription takes the next number from 1.
//! Its frames use the prices of the AsyncAPI mirror's own examples.
//!
//! One file serves both the unit tests (`#[cfg(test)]`) and the integration suite, which
//! includes it with `#[path]`, so it names nothing of this crate's own. The suite uses
//! part of it.
#![allow(dead_code)]

use std::{collections::HashMap, time::Duration};

use polyoxide_ws::{
    test_server::{Script as Shared, ScriptedServer as Server},
    tungstenite::Message,
};
use serde_json::{json, Value};
use tokio::time::Instant;

const SENDING_TS_MS: u64 = 1_669_149_841_234;
const MARKET_ID: &str = "9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1";

/// How the server behaves on one connection.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Frames to send, in order, once the client's first frame has been answered. Their
    /// sequence numbers are the test's own.
    pub pushes: Vec<String>,
    /// While the connection is held open, resend the first push on this cadence.
    pub push_every: Option<Duration>,
    /// Close the connection after the pushes.
    pub close_after: bool,
    /// The close code and reason to send when closing.
    pub close_code: Option<(u16, String)>,
    /// Close the connection this long after the handshake, whatever else is happening.
    pub drop_after: Option<Duration>,
    /// After the pushes, stop reading and writing for good: a dead socket.
    pub go_silent: bool,
    /// Send a protocol ping on this cadence while the connection is held open.
    pub ping_every: Option<Duration>,
    /// Drop the TCP connection without a close frame after the pushes.
    pub then_cut: bool,
    /// Drop the TCP connection without a close frame this long after the handshake.
    pub cut_after: Option<Duration>,
    /// Accept the TCP connection and drop it without a handshake.
    pub reject_handshake: bool,
    /// Answer the WebSocket upgrade with this HTTP status and these headers, such as
    /// `Retry-After`, instead of `101`.
    pub refuse_upgrade: Option<(u16, Vec<(String, String)>)>,
    /// After a `subscribe`'s snapshots, send a subscription-scoped error frame with this
    /// code and message (the suites use 9, authentication required, and 10 and 25, which
    /// Kalshi calls terminal). It carries `sid` and `seq`, and no `id`.
    pub error_after_subscribe: Option<(u32, String)>,
    /// A command naming this ticker is answered with an `error` of this code.
    pub refuse_ticker: Option<(String, u32)>,
    /// Leave every `update_subscription` unanswered.
    pub ignore_updates: bool,
    /// Skip one sequence number before the `ok` answering an `update_subscription`: an
    /// answer with a gap behind it.
    pub ok_skips_seq: bool,
    /// Send a delta ahead of that `ok`.
    pub delta_before_ok: bool,
}

impl Script {
    /// What the shared server sends on the connection: the pushes after the client's first
    /// frame and the answer to it, and how it ends.
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
        if self.then_cut {
            shared = shared.then_cut();
        }
        if let Some(after) = self.cut_after {
            shared = shared.cut_after(after);
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

/// A running shared server, answering as Kalshi does.
pub struct KalshiServer {
    /// The base URL to connect to, `ws://127.0.0.1:<port>`; any path is accepted.
    pub url: String,
    inner: Server,
}

impl KalshiServer {
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

    /// The request headers of each handshake that completed, names in lowercase, in the
    /// order handshakes completed; a refused upgrade takes no slot, so an index is not a
    /// connection's index.
    pub fn handshake_headers(&self) -> Vec<Vec<(String, String)>> {
        self.inner.handshake_headers()
    }

    /// The first request of each connection, parsed.
    pub fn first_frames(&self) -> Vec<Value> {
        self.inner
            .first_frames()
            .iter()
            .map(|text| parse(text))
            .collect()
    }

    /// Every request received, in arrival order, across connections.
    pub fn received(&self) -> Vec<Received> {
        self.inner
            .received()
            .into_iter()
            .map(|r| Received {
                connection: r.connection,
                at: r.at,
                request: parse(&r.text),
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

    /// Connections that have ended, however they ended.
    pub fn ended_count(&self) -> usize {
        self.inner.ended_count()
    }

    /// Poll until `predicate` holds, or panic with `label` after five seconds.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        self.inner.wait_for(label, |_| predicate(self)).await;
    }
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text)
        .unwrap_or_else(|err| panic!("a request that is not JSON ({err}): {text}"))
}

/// A snapshot frame of `ticker`'s book, with the prices of the mirror's example: YES bids
/// at 0.08 and 0.22, and NO bids, in YES pricing, at 0.54 and 0.56.
pub fn snapshot(sid: u64, seq: u64, ticker: &str) -> Value {
    json!({
        "type": "orderbook_snapshot",
        "sending_ts_ms": SENDING_TS_MS,
        "sid": sid,
        "seq": seq,
        "msg": {
            "market_ticker": ticker,
            "market_id": MARKET_ID,
            "yes_dollars_fp": [["0.0800", "300.00"], ["0.2200", "333.00"]],
            "no_dollars_fp": [["0.5400", "20.00"], ["0.5600", "146.00"]],
        },
    })
}

/// A delta frame: a YES level of `ticker` losing 54 contracts at 0.96.
pub fn delta(sid: u64, seq: u64, ticker: &str) -> Value {
    json!({
        "type": "orderbook_delta",
        "sending_ts_ms": SENDING_TS_MS,
        "sid": sid,
        "seq": seq,
        "msg": {
            "market_ticker": ticker,
            "market_id": MARKET_ID,
            "price_dollars": "0.9600",
            "delta_fp": "-54.00",
            "side": "yes",
            "ts_ms": 1_669_149_841_000_u64,
        },
    })
}

fn error(id: &Value, code: u32, message: &str) -> Value {
    json!({
        "id": id,
        "type": "error",
        "sending_ts_ms": SENDING_TS_MS,
        "msg": {"code": code, "msg": message},
    })
}

/// What one connection has subscribed.
#[derive(Default)]
struct State {
    /// The last subscription id issued on this connection.
    issued: u64,
    subscriptions: HashMap<u64, Subscription>,
}

#[derive(Default)]
struct Subscription {
    /// The last sequence number issued.
    seq: u64,
    tickers: Vec<String>,
}

impl State {
    /// The next sequence number of subscription `sid`.
    fn next_seq(&mut self, sid: u64) -> u64 {
        let subscription = self.subscriptions.entry(sid).or_default();
        subscription.seq += 1;
        subscription.seq
    }
}

/// One connection's answerer: Kalshi's answer to `subscribe`, `update_subscription` and
/// `unsubscribe`, given the subscriptions the connection carries, with the script's
/// refusals and injected faults.
fn answer(script: Script) -> impl FnMut(&str) -> Vec<Message> + Send {
    let mut state = State::default();
    move |text| {
        let Ok(request) = serde_json::from_str::<Value>(text) else {
            return Vec::new();
        };
        reply(&mut state, &script, &request)
            .into_iter()
            .map(|frame| Message::Text(frame.to_string().into()))
            .collect()
    }
}

/// The frames Kalshi sends for one request, in order.
fn reply(state: &mut State, script: &Script, request: &Value) -> Vec<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = &request["params"];
    let tickers: Vec<String> = params["market_tickers"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if let Some((refused, code)) = &script.refuse_ticker {
        if tickers.contains(refused) {
            return vec![error(&id, *code, "Invalid parameter")];
        }
    }
    match request["cmd"].as_str() {
        Some("subscribe") => subscribe(state, script, &id, &tickers),
        Some("update_subscription") if script.ignore_updates => Vec::new(),
        Some("update_subscription") => update(state, script, &id, params, &tickers),
        Some("unsubscribe") => unsubscribe(state, &id, params),
        _ => vec![error(&id, 5, "Unknown command")],
    }
}

fn subscribe(state: &mut State, script: &Script, id: &Value, tickers: &[String]) -> Vec<Value> {
    state.issued += 1;
    let sid = state.issued;
    state.subscriptions.insert(
        sid,
        Subscription {
            seq: 0,
            tickers: tickers.to_vec(),
        },
    );
    let mut frames = vec![json!({
        "id": id,
        "type": "subscribed",
        "sending_ts_ms": SENDING_TS_MS,
        "msg": {"channel": "orderbook_delta", "sid": sid},
    })];
    for ticker in tickers {
        let seq = state.next_seq(sid);
        frames.push(snapshot(sid, seq, ticker));
    }
    if let Some((code, message)) = &script.error_after_subscribe {
        let seq = state.next_seq(sid);
        frames.push(json!({
            "type": "error",
            "sid": sid,
            "seq": seq,
            "sending_ts_ms": SENDING_TS_MS,
            "msg": {"code": code, "msg": message},
        }));
    }
    frames
}

fn update(
    state: &mut State,
    script: &Script,
    id: &Value,
    params: &Value,
    tickers: &[String],
) -> Vec<Value> {
    let Some(sid) = params["sid"]
        .as_u64()
        .filter(|sid| state.subscriptions.contains_key(sid))
    else {
        return vec![error(id, 7, "Unknown subscription ID")];
    };
    let adding = params["action"].as_str() == Some("add_markets");
    {
        let subscription = state.subscriptions.entry(sid).or_default();
        if adding {
            subscription.tickers.extend(tickers.iter().cloned());
        } else {
            subscription.tickers.retain(|held| !tickers.contains(held));
        }
    }
    let mut frames = Vec::new();
    if script.delta_before_ok {
        let ticker = state.subscriptions[&sid]
            .tickers
            .first()
            .cloned()
            .unwrap_or_default();
        let seq = state.next_seq(sid);
        frames.push(delta(sid, seq, &ticker));
    }
    if script.ok_skips_seq {
        // A number the client never sees: the answer that follows has a gap behind it.
        state.next_seq(sid);
    }
    let seq = state.next_seq(sid);
    let held = state.subscriptions[&sid].tickers.clone();
    frames.push(json!({
        "id": id,
        "sid": sid,
        "seq": seq,
        "type": "ok",
        "sending_ts_ms": SENDING_TS_MS,
        "msg": {"market_tickers": held},
    }));
    if adding {
        for ticker in tickers {
            let seq = state.next_seq(sid);
            frames.push(snapshot(sid, seq, ticker));
        }
    }
    frames
}

fn unsubscribe(state: &mut State, id: &Value, params: &Value) -> Vec<Value> {
    let sids: Vec<u64> = params["sids"]
        .as_array()
        .map(|sids| sids.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let mut frames = Vec::new();
    for sid in sids {
        if !state.subscriptions.contains_key(&sid) {
            frames.push(error(id, 7, "Unknown subscription ID"));
            continue;
        }
        let seq = state.next_seq(sid);
        state.subscriptions.remove(&sid);
        frames.push(json!({
            "id": id,
            "sid": sid,
            "seq": seq,
            "type": "unsubscribed",
            "sending_ts_ms": SENDING_TS_MS,
        }));
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(state: &mut State, script: &Script, request: Value) -> Vec<Value> {
        reply(state, script, &request)
    }

    fn subscribe_request(tickers: &[&str]) -> Value {
        json!({"id": 1, "cmd": "subscribe", "params": {
            "channels": ["orderbook_delta"], "market_tickers": tickers, "use_yes_price": true,
        }})
    }

    #[test]
    fn a_subscribe_gets_subscribed_and_a_snapshot_per_market_counting_from_one() {
        let mut state = State::default();
        let frames = ask(
            &mut state,
            &Script::default(),
            subscribe_request(&["A-1", "B-2"]),
        );
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0]["type"], "subscribed");
        assert_eq!(frames[0]["id"], 1);
        assert_eq!(frames[0]["msg"]["sid"], 1);
        assert_eq!(frames[1]["type"], "orderbook_snapshot");
        assert_eq!(
            (frames[1]["sid"].as_u64(), frames[1]["seq"].as_u64()),
            (Some(1), Some(1))
        );
        assert_eq!(frames[1]["msg"]["market_ticker"], "A-1");
        assert_eq!(frames[2]["msg"]["market_ticker"], "B-2");
        assert_eq!(frames[2]["seq"], 2);
    }

    #[test]
    fn an_update_is_answered_with_an_ok_on_the_next_seq_and_a_snapshot_per_added_market() {
        let mut state = State::default();
        ask(&mut state, &Script::default(), subscribe_request(&["A-1"]));
        let add = json!({"id": 2, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "add_markets", "market_tickers": ["B-2"],
        }});
        let frames = ask(&mut state, &Script::default(), add);
        assert_eq!(frames[0]["type"], "ok");
        assert_eq!(frames[0]["id"], 2);
        assert_eq!(frames[0]["seq"], 2);
        assert_eq!(frames[0]["msg"]["market_tickers"], json!(["A-1", "B-2"]));
        assert_eq!(frames[1]["type"], "orderbook_snapshot");
        assert_eq!(frames[1]["seq"], 3);
        let delete = json!({"id": 3, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "delete_markets", "market_tickers": ["A-1"],
        }});
        let frames = ask(&mut state, &Script::default(), delete);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["seq"], 4);
        assert_eq!(frames[0]["msg"]["market_tickers"], json!(["B-2"]));
    }

    #[test]
    fn an_unsubscribe_ends_the_subscription_and_an_unknown_sid_is_an_error() {
        let mut state = State::default();
        ask(&mut state, &Script::default(), subscribe_request(&["A-1"]));
        let end = json!({"id": 2, "cmd": "unsubscribe", "params": {"sids": [1]}});
        let frames = ask(&mut state, &Script::default(), end.clone());
        assert_eq!(frames[0]["type"], "unsubscribed");
        assert_eq!(
            (frames[0]["sid"].as_u64(), frames[0]["seq"].as_u64()),
            (Some(1), Some(2))
        );
        let again = ask(&mut state, &Script::default(), end);
        assert_eq!(again[0]["type"], "error");
        assert_eq!(again[0]["msg"]["code"], 7);
    }

    #[test]
    fn a_second_subscription_gets_the_next_sid_and_its_own_count() {
        let mut state = State::default();
        ask(&mut state, &Script::default(), subscribe_request(&["A-1"]));
        let frames = ask(&mut state, &Script::default(), subscribe_request(&["B-2"]));
        assert_eq!(frames[0]["msg"]["sid"], 2);
        assert_eq!(
            (frames[1]["sid"].as_u64(), frames[1]["seq"].as_u64()),
            (Some(2), Some(1))
        );
    }

    #[test]
    fn injected_faults_skip_a_number_refuse_a_ticker_and_report_on_a_subscription() {
        let mut state = State::default();
        let script = Script {
            ok_skips_seq: true,
            delta_before_ok: true,
            refuse_ticker: Some(("BAD-1".to_owned(), 11)),
            error_after_subscribe: Some((25, "Subscription buffer overflow".to_owned())),
            ..Script::default()
        };
        let frames = ask(&mut state, &script, subscribe_request(&["A-1"]));
        assert_eq!(frames[2]["type"], "error");
        assert_eq!(
            (frames[2]["sid"].as_u64(), frames[2]["seq"].as_u64()),
            (Some(1), Some(2))
        );
        assert_eq!(frames[2]["msg"]["code"], 25);
        assert!(frames[2].get("id").is_none());
        let add = json!({"id": 2, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "add_markets", "market_tickers": ["B-2"],
        }});
        let frames = ask(&mut state, &script, add);
        // A delta (3), the number the ok skips (4), then the ok (5).
        assert_eq!(frames[0]["type"], "orderbook_delta");
        assert_eq!(frames[0]["seq"], 3);
        assert_eq!(frames[1]["type"], "ok");
        assert_eq!(frames[1]["seq"], 5);
        let refused = json!({"id": 3, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "add_markets", "market_tickers": ["BAD-1"],
        }});
        let frames = ask(&mut state, &script, refused);
        assert_eq!(frames[0]["type"], "error");
        assert_eq!(frames[0]["id"], 3);
        assert_eq!(frames[0]["msg"]["code"], 11);
    }

    #[test]
    fn a_subscription_error_carries_the_code_it_is_given() {
        // Kalshi's terminal codes: 9 (authentication required), 10 (a channel error) and
        // 25 (the subscription's buffer overflowed).
        for (code, message) in [
            (9, "Authentication required"),
            (10, "Channel error"),
            (25, "Subscription buffer overflow"),
        ] {
            let mut state = State::default();
            let script = Script {
                error_after_subscribe: Some((code, message.to_owned())),
                ..Script::default()
            };
            let frames = ask(&mut state, &script, subscribe_request(&["A-1"]));
            assert_eq!(frames.len(), 3, "{code}");
            assert_eq!(frames[2]["type"], "error", "{code}");
            assert_eq!(frames[2]["msg"]["code"], code, "{code}");
            assert_eq!(frames[2]["msg"]["msg"], message, "{code}");
            assert_eq!(
                (frames[2]["sid"].as_u64(), frames[2]["seq"].as_u64()),
                (Some(1), Some(2)),
                "{code}"
            );
            assert!(frames[2].get("id").is_none(), "{code}");
        }
    }
}
