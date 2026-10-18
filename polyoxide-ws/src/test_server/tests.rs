//! The server's own tests.
//!
//! Infrastructure with a bug is worse than none: a harness that silently
//! stops accepting connections, mis-scripts a connection or drops a frame it
//! claims to record makes the suites built on it fail in a way that looks
//! like a library bug. These pin its behaviour against real sockets. The
//! first eight are one socket crate's former server's, and one venue's five
//! and another venue's two answers are written here as the closures their
//! suites will pass, to show the answer hook fits them.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::{frame::coding::CloseCode, CloseFrame},
    MaybeTlsStream, WebSocketStream,
};

use super::*;
use crate::{connect, retrying_can_fix, Connection, Kind, Mode, DEFAULT_CONNECT_TIMEOUT};

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Connects to `url` and immediately sends `frame` as the subscription
/// message, mirroring what a real client does on connect.
async fn connect_and_subscribe(url: &str, frame: &str) -> Client {
    let (mut ws, _) = connect_async(url).await.expect("connect");
    ws.send(Message::Text(frame.to_string().into()))
        .await
        .expect("send subscription frame");
    ws
}

async fn next_message(ws: &mut Client) -> Message {
    timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("a frame promptly")
        .expect("stream item")
        .expect("ok message")
}

fn text(value: Value) -> Message {
    Message::Text(value.to_string().into())
}

fn parse(message: &Message) -> Value {
    match message {
        Message::Text(text) => serde_json::from_str(text).expect("JSON"),
        other => panic!("expected a text frame, got {other:?}"),
    }
}

mod as_rtds_used_it {
    use super::*;

    #[tokio::test]
    async fn send_then_close_delivers_its_frames_in_order_then_closes() {
        let server = ScriptedServer::start(vec![Script::frames(&["frame-a", "frame-b"])
            .after_first_frame()
            .then_close()])
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
            ScriptedServer::start(vec![Script::frames(&["frame-c"]).after_first_frame()]).await;
        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        match next_message(&mut ws).await {
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
            Script::frames(&["frame-a"])
                .after_first_frame()
                .then_close(),
            Script::frames(&["frame-c"]).after_first_frame(),
        ])
        .await;

        // Connections 1 and 2 consume the two scripted entries; connection 3
        // has nothing scripted for it and must repeat the last one, which is
        // how it differs from connection 1's close.
        for expected in ["frame-a", "frame-c", "frame-c"] {
            let mut ws = connect_and_subscribe(&server.url, "sub").await;
            match next_message(&mut ws).await {
                Message::Text(text) => assert_eq!(text.to_string(), expected),
                other => panic!("unexpected message: {other:?}"),
            }
        }

        assert_eq!(server.connection_count(), 3);
    }

    #[tokio::test]
    async fn received_subscriptions_records_one_frame_per_connection_in_order() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;

        // Keep each socket alive; dropping it right after sending would race
        // the server's read against the client's teardown.
        let mut sockets = Vec::new();
        for frame in ["sub-1", "sub-2", "sub-3"] {
            sockets.push(connect_and_subscribe(&server.url, frame).await);
        }

        server
            .wait_for("all three subscription frames", |s| {
                s.first_frames().len() == 3
            })
            .await;

        assert_eq!(
            server.first_frames(),
            vec![
                "sub-1".to_string(),
                "sub-2".to_string(),
                "sub-3".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn client_frames_records_what_a_client_sends_after_subscribing() {
        // `first_frames` keeps only the first frame per connection, so a test
        // for a ping or a second subscribe would have nothing to assert
        // against without this second list.
        let server = ScriptedServer::start(vec![Script::hold()]).await;
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
            server.first_frames(),
            vec!["sub-1".to_string()],
            "only the first frame of a connection is its subscription"
        );
    }

    #[tokio::test]
    async fn close_count_observes_a_client_initiated_close() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        assert_eq!(server.close_count(), 0);
        ws.close(None).await.expect("close");

        server
            .wait_for("a close frame", |s| s.close_count() == 1)
            .await;
    }

    #[tokio::test]
    async fn reject_handshake_fails_the_connection_attempt_recoverably() {
        // A reconnect loop's retry arm runs only for a failure retrying can
        // fix, so a rejection that read as unfixable would make a supervisor
        // give up instead of retrying, and a test using it would pass for the
        // wrong reason.
        let server = ScriptedServer::start(vec![Script::reject()]).await;

        let err = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect_err("a dropped handshake must not succeed");

        assert!(retrying_can_fix(&err, |_| false), "{err:?}");
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn send_raw_then_idle_delivers_non_text_messages() {
        let server = ScriptedServer::start(vec![Script::messages(vec![
            Message::Ping(vec![7u8].into()),
            Message::Binary(vec![1u8, 2, 3].into()),
            Message::Text("frame-d".into()),
        ])
        .after_first_frame()])
        .await;

        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;

        // tokio-tungstenite answers the Ping itself but still surfaces it, so
        // all three arrive. The client under test must skip the first two.
        let mut kinds = Vec::new();
        for _ in 0..3 {
            kinds.push(match next_message(&mut ws).await {
                Message::Ping(_) => "ping",
                Message::Binary(_) => "binary",
                Message::Text(_) => "text",
                other => panic!("unexpected message: {other:?}"),
            });
        }
        assert_eq!(kinds, vec!["ping", "binary", "text"]);
    }
}

/// One venue's answers, as its suites will pass them: a `sub`/`unsub` gets one
/// status per channel in request order, a `post` ping gets a pong unless the
/// socket is dead, and anything else gets no reply.
mod as_perps_will_use_it {
    use super::*;

    #[derive(Clone)]
    pub(super) struct Perps {
        pub(super) refuse: Vec<(&'static str, &'static str)>,
        pub(super) answer_pings: bool,
    }

    impl Default for Perps {
        fn default() -> Self {
            Self {
                refuse: Vec::new(),
                answer_pings: true,
            }
        }
    }

    pub(super) fn answer(perps: Perps) -> impl FnMut(&str) -> Vec<Message> + Send {
        move |request| {
            let Ok(request) = serde_json::from_str::<Value>(request) else {
                return Vec::new();
            };
            let id = request.get("id").cloned().unwrap_or(Value::Null);
            match request.get("req").and_then(Value::as_str) {
                Some("sub" | "unsub") => {
                    let statuses: Vec<Value> = request["chs"]
                        .as_array()
                        .map(|chs| {
                            chs.iter()
                                .map(|c| {
                                    match perps
                                        .refuse
                                        .iter()
                                        .find(|(name, _)| Some(*name) == c.as_str())
                                    {
                                        Some((_, error)) => {
                                            json!({"status": "err", "error": error})
                                        }
                                        None => json!({"status": "ok"}),
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    vec![text(json!({"id": id, "data": statuses}))]
                }
                Some("post")
                    if perps.answer_pings && request["op"]["type"].as_str() == Some("ping") =>
                {
                    vec![text(
                        json!({"id": id, "ts": 1, "data": {"status": "ok", "ts": 1, "sq": 1}}),
                    )]
                }
                _ => Vec::new(),
            }
        }
    }

    fn reply(perps: Perps, request: &str) -> Option<Value> {
        answer(perps)(request).first().map(parse)
    }

    #[test]
    fn a_subscribe_gets_one_ok_per_channel_in_order() {
        let got = reply(
            Perps::default(),
            r#"{"id":7,"req":"sub","chs":["bbo::1","book::1::50","trades::2"]}"#,
        )
        .unwrap();
        assert_eq!(got["id"], 7);
        assert_eq!(got["data"].as_array().unwrap().len(), 3);
        assert!(got["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s == &json!({"status": "ok"})));
    }

    #[test]
    fn a_scripted_refusal_lands_at_its_channels_index() {
        let perps = Perps {
            refuse: vec![("nonsense::1", "invalid channel")],
            ..Perps::default()
        };
        let got = reply(
            perps,
            r#"{"id":1,"req":"sub","chs":["bbo::1","nonsense::1","trades::1"]}"#,
        )
        .unwrap();
        let data = got["data"].as_array().unwrap();
        assert_eq!(data[0], json!({"status": "ok"}));
        assert_eq!(
            data[1],
            json!({"status": "err", "error": "invalid channel"})
        );
        assert_eq!(data[2], json!({"status": "ok"}));
    }

    #[test]
    fn a_ping_gets_a_pong_with_a_sequence() {
        let got = reply(
            Perps::default(),
            r#"{"id":2,"req":"post","op":{"type":"ping"}}"#,
        )
        .unwrap();
        assert_eq!(got["id"], 2);
        assert_eq!(got["data"]["status"], "ok");
        assert!(got["data"]["sq"].is_u64());
    }

    #[test]
    fn a_post_that_is_not_a_ping_gets_no_reply() {
        assert!(reply(
            Perps::default(),
            r#"{"id":3,"req":"post","op":{"type":"order"}}"#
        )
        .is_none());
        assert!(reply(Perps::default(), r#"{"id":4,"req":"post"}"#).is_none());
    }

    #[test]
    fn a_dead_socket_does_not_answer_pings() {
        let perps = Perps {
            answer_pings: false,
            ..Perps::default()
        };
        assert!(reply(
            perps.clone(),
            r#"{"id":2,"req":"post","op":{"type":"ping"}}"#
        )
        .is_none());
        // Subscriptions are still answered; only the keep-alive is dead.
        assert!(reply(perps, r#"{"id":1,"req":"sub","chs":["bbo::1"]}"#).is_some());
    }

    #[tokio::test]
    async fn the_hook_answers_on_the_wire() {
        let server =
            ScriptedServer::start_answering(vec![Script::hold()], |_| answer(Perps::default()))
                .await;
        let mut ws =
            connect_and_subscribe(&server.url, r#"{"id":1,"req":"sub","chs":["bbo::1"]}"#).await;
        assert_eq!(
            parse(&next_message(&mut ws).await),
            json!({"id": 1, "data": [{"status": "ok"}]})
        );
    }
}

/// Another venue's answers, as its suites will pass them, with the streams
/// each connection carries as the answerer's own state.
mod as_binance_will_use_it {
    use super::*;

    pub(super) fn answer(
        refuse: Vec<(&'static str, i64, &'static str)>,
    ) -> impl FnMut(&str) -> Vec<Message> + Send {
        let mut streams: Vec<String> = Vec::new();
        move |request| {
            let Ok(request) = serde_json::from_str::<Value>(request) else {
                return Vec::new();
            };
            vec![text(reply(&refuse, &request, &mut streams))]
        }
    }

    fn reply(
        refuse: &[(&'static str, i64, &'static str)],
        request: &Value,
        streams: &mut Vec<String>,
    ) -> Value {
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
        if let Some((_, code, msg)) = refuse
            .iter()
            .find(|(name, _, _)| params.iter().any(|p| p == name))
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

    fn ask(answer: &mut impl FnMut(&str) -> Vec<Message>, request: Value) -> Value {
        parse(&answer(&request.to_string())[0])
    }

    #[test]
    fn subscribe_list_and_unsubscribe_answer_as_binance_does() {
        let mut answer = answer(Vec::new());
        let sub = json!({"method": "SUBSCRIBE", "params": ["btcusdt@aggTrade", "ethusdt@aggTrade"], "id": 1});
        assert_eq!(ask(&mut answer, sub), json!({"result": null, "id": 1}));
        let unsub = json!({"method": "UNSUBSCRIBE", "params": ["btcusdt@aggTrade"], "id": 2});
        ask(&mut answer, unsub);
        let list = json!({"method": "LIST_SUBSCRIPTIONS", "id": 3});
        assert_eq!(
            ask(&mut answer, list),
            json!({"result": ["ethusdt@aggTrade"], "id": 3})
        );
    }

    #[test]
    fn a_scripted_refusal_is_an_error_answer() {
        let mut answer = answer(vec![("ethusdt@aggTrade", 2, "Invalid request")]);
        let sub = json!({"method": "SUBSCRIBE", "params": ["ethusdt@aggTrade"], "id": 9});
        assert_eq!(
            ask(&mut answer, sub),
            json!({"error": {"code": 2, "msg": "Invalid request"}, "id": 9})
        );
    }

    #[tokio::test]
    async fn each_connection_gets_its_own_answerer() {
        let server =
            ScriptedServer::start_answering(vec![Script::hold()], |_| answer(Vec::new())).await;
        for _ in 0..2 {
            let mut ws = connect_and_subscribe(
                &server.url,
                r#"{"method":"SUBSCRIBE","params":["btcusdt@aggTrade"],"id":1}"#,
            )
            .await;
            next_message(&mut ws).await;
            ws.send(Message::Text(
                r#"{"method":"LIST_SUBSCRIPTIONS","id":2}"#.into(),
            ))
            .await
            .unwrap();
            assert_eq!(
                parse(&next_message(&mut ws).await)["result"],
                json!(["btcusdt@aggTrade"]),
                "a second connection must not see the first one's streams"
            );
        }
    }
}

mod actions_and_recorders {
    use super::*;

    #[tokio::test]
    async fn a_refused_upgrade_carries_its_status_and_headers() {
        let server = ScriptedServer::start(vec![
            Script::refuse_upgrade(429).with_header("Retry-After", "7")
        ])
        .await;
        let err = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect_err("refused");
        assert_eq!(
            err.kind(),
            Kind::Refused {
                status: 429,
                retry_after: Some("7")
            }
        );
        assert_eq!(server.handshake_count(), 0);
    }

    #[tokio::test]
    async fn a_stalled_handshake_never_completes() {
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let after = Duration::from_millis(200);
        let err = connect(server.url.as_str(), after)
            .await
            .expect_err("stalled");
        assert_eq!(err.kind(), Kind::ConnectTimeout { after });
        assert_eq!(server.connection_count(), 1);
        assert_eq!(server.handshake_count(), 0);
    }

    #[tokio::test]
    async fn pings_carry_their_sequence_and_their_pongs_are_recorded() {
        let server =
            ScriptedServer::start(vec![Script::pings_only(Duration::from_millis(20))]).await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .unwrap();
        let mut connection = Connection::new(socket, Mode::All);
        for sequence in 1u32..=2 {
            assert_eq!(
                next_event(&mut connection).await,
                Message::Ping(sequence.to_be_bytes().to_vec().into())
            );
        }
        server.wait_for("two pongs", |s| s.pongs().len() >= 2).await;
        assert_eq!(server.pongs()[..2], [vec![0, 0, 0, 1], vec![0, 0, 0, 2]]);
    }

    #[tokio::test]
    async fn push_every_resends_the_first_frame() {
        let server = ScriptedServer::start(vec![
            Script::frames(&["tick", "once"]).push_every(Duration::from_millis(10))
        ])
        .await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .unwrap();
        let mut connection = Connection::new(socket, Mode::Data);
        let mut seen = Vec::new();
        for _ in 0..5 {
            seen.push(next_event(&mut connection).await);
        }
        assert_eq!(seen[..2], [Message::from("tick"), Message::from("once")]);
        assert!(
            seen[2..].iter().all(|m| m == &Message::from("tick")),
            "{seen:?}"
        );
    }

    #[tokio::test]
    async fn drop_after_closes_with_the_scripted_code_and_counts_the_reply() {
        let server = ScriptedServer::start(vec![Script::hold()
            .drop_after(Duration::from_millis(50))
            .close_with(1008, "Too many requests")])
        .await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .unwrap();
        let mut connection = Connection::new(socket, Mode::All);
        assert_eq!(
            next_event(&mut connection).await,
            Message::Close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "Too many requests".into()
            }))
        );
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
        server.wait_for("the end", |s| s.ended_count() == 1).await;
    }

    #[tokio::test]
    async fn go_silent_answers_the_first_request_and_nothing_after() {
        let server = ScriptedServer::start_answering(
            vec![Script::frames(&["push"]).after_first_frame().go_silent()],
            |_| |request: &str| vec![Message::from(format!("answer to {request}"))],
        )
        .await;
        let mut ws = connect_and_subscribe(&server.url, "one").await;
        assert_eq!(next_message(&mut ws).await, Message::from("answer to one"));
        assert_eq!(next_message(&mut ws).await, Message::from("push"));
        ws.send(Message::from("two")).await.unwrap();
        ws.send(Message::Ping(Vec::new().into())).await.unwrap();
        let silence = timeout(Duration::from_millis(300), ws.next()).await;
        assert!(silence.is_err(), "a silent socket answered: {silence:?}");
        assert_eq!(
            server.client_frames(),
            ["one"],
            "nothing is read once silent"
        );
    }

    #[tokio::test]
    async fn a_cut_connection_ends_without_a_close_frame() {
        let server = ScriptedServer::start(vec![Script::hold().then_cut()]).await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .unwrap();
        let mut connection = Connection::new(socket, Mode::All);
        let err = next_item(&mut connection).await.unwrap_err();
        assert!(matches!(err.kind(), Kind::Protocol | Kind::Io), "{err:?}");
    }

    #[tokio::test]
    async fn a_connection_is_cut_on_schedule_without_a_close_frame() {
        let server = ScriptedServer::start(vec![
            Script::frames(&["push"]).cut_after(Duration::from_millis(100))
        ])
        .await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .unwrap();
        let mut connection = Connection::new(socket, Mode::All);
        assert_eq!(next_event(&mut connection).await, Message::from("push"));
        let err = next_item(&mut connection).await.unwrap_err();
        assert!(matches!(err.kind(), Kind::Protocol | Kind::Io), "{err:?}");
        assert_eq!(server.close_count(), 0);
    }

    #[tokio::test]
    async fn a_silent_connection_is_still_dropped_on_schedule() {
        let server = ScriptedServer::start(vec![Script::frames(&["push"])
            .go_silent()
            .drop_after(Duration::from_millis(100))
            .close_with(1001, "bye")])
        .await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .unwrap();
        let mut connection = Connection::new(socket, Mode::All);
        assert_eq!(next_event(&mut connection).await, Message::from("push"));
        connection
            .send(Message::Ping(Vec::new().into()))
            .await
            .unwrap();
        assert_eq!(
            next_event(&mut connection).await,
            Message::Close(Some(CloseFrame {
                code: CloseCode::Away,
                reason: "bye".into()
            })),
            "silence answers no ping, but drop_after still closes"
        );
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
    }

    #[tokio::test]
    async fn a_frame_in_flight_does_not_hide_the_close_reply() {
        // The server closes on the first frame; the second is already on its
        // way, and arrives ahead of the client's reply.
        let server =
            ScriptedServer::start(vec![Script::hold().after_first_frame().then_close()]).await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .unwrap();
        let mut connection = Connection::new(socket, Mode::Data);
        connection.send(Message::from("one")).await.unwrap();
        connection.send(Message::from("two")).await.unwrap();
        assert!(next_item_or_end(&mut connection).await.is_none());
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
        assert_eq!(server.client_frames(), ["one", "two"]);
    }

    #[tokio::test]
    async fn a_client_close_is_answered() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut ws = connect_and_subscribe(&server.url, "sub-1").await;
        ws.close(None).await.expect("close");
        assert!(
            matches!(next_message(&mut ws).await, Message::Close(_)),
            "the server must reply to the client's close"
        );
    }

    /// The next item, or the end, within the window.
    async fn next_item_or_end(
        connection: &mut Connection,
    ) -> Option<Result<Message, crate::WsError>> {
        timeout(WINDOW, connection.next())
            .await
            .expect("an item or the end within the window")
    }

    #[tokio::test]
    async fn every_connection_is_counted_and_timed_and_its_path_kept() {
        let server = ScriptedServer::start(vec![Script::reject(), Script::hold()]).await;
        let _ = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT).await;
        let socket = connect(
            format!("{}/market/stream", server.url),
            DEFAULT_CONNECT_TIMEOUT,
        )
        .await
        .unwrap();
        let mut connection = Connection::new(socket, Mode::Data);
        connection
            .send(Message::Ping(Vec::new().into()))
            .await
            .unwrap();
        server.wait_for("a ping", |s| s.ping_count() == 1).await;
        assert_eq!(server.connection_count(), 2);
        assert_eq!(server.handshake_count(), 1);
        assert_eq!(server.paths(), ["/market/stream"]);
        let accepted = server.accepted_at();
        assert!(accepted[0] <= accepted[1]);

        connection.close().await.unwrap();
        server
            .wait_for("the client's end", |s| s.client_ended_count() == 1)
            .await;
        server.wait_for("both ends", |s| s.ended_count() == 2).await;
        assert_eq!(server.close_count(), 1);
    }

    #[tokio::test]
    async fn received_frames_name_their_connection_and_count_by_predicate() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut first = connect_and_subscribe(&server.url, r#"{"type":"ping"}"#).await;
        let _second = connect_and_subscribe(&server.url, "sub").await;
        first
            .send(Message::from(r#"{"type":"ping"}"#))
            .await
            .unwrap();
        server
            .wait_for("three frames", |s| s.received().len() == 3)
            .await;
        assert_eq!(server.count_frames(|f| f.contains(r#""type":"ping""#)), 2);
        let connections: Vec<usize> = server.received().iter().map(|r| r.connection).collect();
        assert_eq!(connections.iter().filter(|&&c| c == 0).count(), 2);
        assert_eq!(connections.iter().filter(|&&c| c == 1).count(), 1);
    }
}

mod helpers_as_the_suites_use_them {
    use futures_util::stream;

    use super::*;

    #[test]
    fn fast_is_test_sized() {
        let timing = fast();
        assert_eq!(timing.ping_interval, Some(Duration::from_millis(50)));
        assert_eq!(timing.stale_after, Duration::from_millis(300));
        assert_eq!(timing.initial_backoff, Duration::from_millis(10));
        assert_eq!(timing.max_backoff, Duration::from_millis(20));
        assert_eq!(timing.connect_timeout, DEFAULT_CONNECT_TIMEOUT);
    }

    #[tokio::test]
    async fn next_event_takes_the_next_ok() {
        let mut events = stream::iter([Ok::<_, ()>(1), Ok(2)]);
        assert_eq!(next_event(&mut events).await, 1);
        assert_eq!(next_item(&mut events).await, Ok(2));
    }

    #[tokio::test]
    #[should_panic(expected = "an event, not an error")]
    async fn next_event_refuses_an_error() {
        next_event(&mut stream::iter([Err::<(), _>("boom")])).await;
    }

    #[tokio::test(start_paused = true)]
    #[should_panic(expected = "no item within")]
    async fn next_item_gives_up_after_the_window() {
        next_item(&mut stream::pending::<()>()).await;
    }

    #[tokio::test]
    async fn drain_reads_to_the_end() {
        let mut drained = Vec::new();
        drain(stream::iter([1, 2, 3]).inspect(|item| drained.push(*item))).await;
        assert_eq!(drained, [1, 2, 3]);
    }
}

mod handshake_headers_are_recorded {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    use super::*;

    /// Connects to `url` with one extra request header, `x-sample-stamp: value`.
    // The error type is tungstenite's; a test helper does not box it.
    #[allow(clippy::result_large_err)]
    async fn connect_stamped(
        url: &str,
        value: &'static str,
    ) -> Result<Client, tokio_tungstenite::tungstenite::Error> {
        let mut request = url.into_client_request().expect("a client request");
        request
            .headers_mut()
            .insert("x-sample-stamp", HeaderValue::from_static(value));
        connect_async(request).await.map(|(ws, _)| ws)
    }

    /// The value of the `x-sample-stamp` header among a handshake's headers.
    fn stamp(headers: &[(String, String)]) -> Option<&str> {
        headers
            .iter()
            .find(|(name, _)| name.as_str() == "x-sample-stamp")
            .map(|(_, value)| value.as_str())
    }

    #[tokio::test]
    async fn each_handshake_is_recorded_with_its_headers_in_accept_order() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        assert!(server.handshake_headers().is_empty());

        let _first = connect_stamped(&server.url, "alpha").await.unwrap();
        server
            .wait_for("one handshake", |s| s.handshake_headers().len() == 1)
            .await;
        let _second = connect_stamped(&server.url, "beta").await.unwrap();
        server
            .wait_for("two handshakes", |s| s.handshake_headers().len() == 2)
            .await;

        let recorded = server.handshake_headers();
        assert_eq!(recorded.len(), 2);
        assert_eq!(stamp(&recorded[0]), Some("alpha"));
        assert_eq!(stamp(&recorded[1]), Some("beta"));
        // Every name is lowercase (this pins the http crate's guarantee, which
        // the recorder relies on), and the client's own upgrade headers are
        // among them, so the record is the request and not only what a test
        // added to it.
        assert!(recorded
            .iter()
            .flatten()
            .all(|(name, _)| *name == name.to_lowercase()));
        for headers in &recorded {
            assert!(headers
                .iter()
                .any(|(name, _)| name.as_str() == "sec-websocket-key"));
        }
    }

    #[tokio::test]
    async fn a_refused_upgrade_records_nothing() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(429), Script::hold()]).await;
        // The first connection is refused; the second is accepted.
        assert!(connect_stamped(&server.url, "refused").await.is_err());
        let _accepted = connect_stamped(&server.url, "accepted").await.unwrap();
        server
            .wait_for("the accepted handshake", |s| {
                !s.handshake_headers().is_empty()
            })
            .await;

        let recorded = server.handshake_headers();
        assert_eq!(recorded.len(), 1);
        assert_eq!(stamp(&recorded[0]), Some("accepted"));
        assert_eq!(server.paths().len(), 1);
    }
}
