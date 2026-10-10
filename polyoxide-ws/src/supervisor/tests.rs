//! The Supervisor's rules (AD-11), each pinned against the shared scripted
//! server with a test protocol: JSON `{"id", "op", "topics"}` requests
//! answered `{"id", "ok"}`, `{"data": n}` and `{"burst": [..]}` pushes, and
//! protocol pings. It counts any frame as liveness, as Binance does, and
//! skips a frame that is not JSON.

use std::time::Duration;

use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::time::Instant;

use super::*;
use crate::{
    retrying_can_fix,
    test_server::{fast, next_event, next_item, Script, ScriptedServer, WINDOW},
    tungstenite::client::IntoClientRequest,
    Correlated, Kind,
};

#[derive(Debug, PartialEq, Eq)]
enum Ev {
    Data(u64),
    Disconnected(&'static str),
    Reconnected,
}

#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error(transparent)]
    Ws(#[from] WsError),
    #[error("bad frame: {0}")]
    Bad(String),
    #[error("refused: {0}")]
    Refused(String),
    #[error("stalled after {0:?}")]
    Stalled(Duration),
    #[error("stopped")]
    Stopped,
}

#[derive(Debug, Clone)]
struct Topics {
    topics: Vec<String>,
    /// Whether an empty membership stops being wanted, as Binance's paths do.
    empty_unwanted: bool,
}

#[derive(Debug)]
enum Change {
    Add(&'static str),
    Remove(&'static str),
}

/// The test protocol.
struct Feed {
    url: String,
    declaring: bool,
    /// Record a change made during an outage at once, as Binance does, rather
    /// than hold it, as perps does.
    record: bool,
    queue: Queue,
}

fn text(value: Value) -> Message {
    Message::Text(value.to_string().into())
}

fn parse(frame: &Message) -> Option<Value> {
    match frame {
        Message::Text(text) => serde_json::from_str(text).ok(),
        _ => None,
    }
}

/// One request and its answer.
async fn ask(connection: &mut Connection, op: &str, topics: Vec<String>) -> Result<(), TestError> {
    let names = topics.join(",");
    let op = op.to_owned();
    let accepted = connection
        .request(
            move |id| text(json!({"id": id, "op": op, "topics": topics})),
            |id, frame| match parse(frame) {
                Some(answer) if answer["id"] == id => {
                    Correlated::Answer(answer["ok"].as_bool() == Some(true))
                }
                _ => Correlated::Push,
            },
        )
        .await?;
    if accepted {
        Ok(())
    } else {
        Err(TestError::Refused(names))
    }
}

impl Protocol for Feed {
    type Event = Ev;
    type Error = TestError;
    type Membership = Topics;
    type Change = Change;
    type Ack = usize;

    fn handshake(&mut self) -> Result<Request, TestError> {
        Ok(self
            .url
            .as_str()
            .into_client_request()
            .map_err(WsError::from)?)
    }

    async fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &Topics,
    ) -> Result<(), TestError> {
        if membership.topics.is_empty() {
            return Ok(());
        }
        ask(connection, "sub", membership.topics.clone()).await
    }

    async fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut Topics,
        change: Change,
    ) -> Result<usize, TestError> {
        match change {
            Change::Add(topic) => {
                ask(connection, "sub", vec![topic.to_owned()]).await?;
                membership.topics.push(topic.to_owned());
            }
            Change::Remove(topic) => {
                ask(connection, "unsub", vec![topic.to_owned()]).await?;
                membership.topics.retain(|t| t != topic);
            }
        }
        Ok(membership.topics.len())
    }

    fn record(
        &mut self,
        membership: &mut Topics,
        change: Change,
    ) -> Offline<Change, usize, TestError> {
        if !self.record {
            return Offline::Held(change);
        }
        match change {
            Change::Add(topic) => membership.topics.push(topic.to_owned()),
            Change::Remove(topic) => membership.topics.retain(|t| t != topic),
        }
        Offline::Recorded(Ok(membership.topics.len()))
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Ev, TestError> {
        let Message::Text(raw) = frame else {
            return Decoded::Nothing;
        };
        let Some(value) = parse(frame) else {
            return Decoded::Error(TestError::Bad(raw.to_string()));
        };
        if let Some(n) = value["data"].as_u64() {
            Decoded::Event(Ev::Data(n))
        } else if let Some(burst) = value["burst"].as_array() {
            Decoded::Events(
                burst
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(Ev::Data)
                    .collect(),
            )
        } else {
            Decoded::Nothing
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<Ev, TestError>) -> bool {
        true
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<Ev, TestError>) -> bool {
        matches!(decoded, Decoded::Event(_) | Decoded::Events(_))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Ping(Vec::new().into()))
    }

    fn wanted(membership: &Topics) -> bool {
        !(membership.empty_unwanted && membership.topics.is_empty())
    }

    fn is_retriable(err: &TestError) -> bool {
        match err {
            TestError::Ws(err) => {
                retrying_can_fix(err, |status| matches!(status, 408 | 425 | 429 | 500..=599))
            }
            TestError::Stalled(_) => true,
            TestError::Bad(_) | TestError::Refused(_) | TestError::Stopped => false,
        }
    }

    fn recover(&self, err: &TestError) -> Recovery {
        match err {
            TestError::Bad(_) => Recovery::Skip,
            err if Self::is_retriable(err) => Recovery::Reconnect,
            _ => Recovery::Stop,
        }
    }

    fn disconnected(&mut self, outage: Outage<'_, TestError>) -> Option<Ev> {
        let why = match outage {
            Outage::Closed { .. } => "closed",
            Outage::Error(_) => "error",
            Outage::Stale { .. } => "stale",
            Outage::Rotation => "rotation",
        };
        self.declaring.then_some(Ev::Disconnected(why))
    }

    fn reconnected(&mut self) -> Ev {
        Ev::Reconnected
    }

    fn queue(&self) -> Queue {
        self.queue
    }

    fn stalled(silent: Duration) -> TestError {
        TestError::Stalled(silent)
    }

    fn stopped() -> TestError {
        TestError::Stopped
    }
}

/// One connection's answer: every request accepted, or every one refused,
/// except one naming the topic `hang`, which is never answered.
fn answer(refuse: bool) -> impl FnMut(&str) -> Vec<Message> + Send {
    move |request| {
        let Ok(request) = serde_json::from_str::<Value>(request) else {
            return Vec::new();
        };
        let hang = request["topics"]
            .as_array()
            .is_some_and(|topics| topics.iter().any(|t| t == "hang"));
        if hang {
            return Vec::new();
        }
        vec![text(json!({"id": request["id"], "ok": !refuse}))]
    }
}

/// A server that accepts every request.
async fn server(scripts: Vec<Script>) -> ScriptedServer {
    server_refusing_from(scripts, usize::MAX).await
}

/// A server that refuses every request from its `refuse_from`-th connection on.
async fn server_refusing_from(scripts: Vec<Script>, refuse_from: usize) -> ScriptedServer {
    ScriptedServer::start_answering(scripts, move |index| answer(index >= refuse_from)).await
}

fn data(numbers: impl IntoIterator<Item = u64>) -> Vec<Message> {
    numbers
        .into_iter()
        .map(|n| text(json!({"data": n})))
        .collect()
}

/// Send these frames once the client's replay has arrived, then hold.
fn after_replay(frames: Vec<Message>) -> Script {
    Script::messages(frames).after_first_frame()
}

fn feed(server: &ScriptedServer) -> Feed {
    Feed {
        url: server.url.clone(),
        declaring: true,
        record: false,
        queue: Queue::Unbounded,
    }
}

fn topics(names: &[&str]) -> Topics {
    Topics {
        topics: names.iter().map(|n| (*n).to_owned()).collect(),
        empty_unwanted: false,
    }
}

fn backoff(timing: Timing, delay: Duration) -> Timing {
    Timing {
        initial_backoff: delay,
        max_backoff: delay,
        ..timing
    }
}

async fn connect(feed: Feed, timing: Timing, membership: Topics) -> Supervised<Feed> {
    Supervisor::new(feed, timing)
        .connect(membership)
        .await
        .expect("connect")
}

/// Nothing more within `within`.
async fn quiet(ws: &mut Supervised<Feed>, within: Duration) {
    let next = tokio::time::timeout(within, ws.next()).await;
    assert!(next.is_err(), "expected nothing, got {next:?}");
}

async fn the_end(ws: &mut Supervised<Feed>) {
    let end = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(end.is_none(), "{end:?}");
}

#[tokio::test]
async fn a_slow_consumer_keeps_the_pings_going_and_loses_nothing() {
    // Ten events, two of them one frame's burst, into a queue of two, with
    // nobody reading for two staleness windows.
    let mut frames = data(1..=8);
    frames.push(text(json!({"burst": [9, 10]})));
    let server = server(vec![after_replay(frames)]).await;
    let mut ws = connect(
        Feed {
            queue: Queue::Bounded(2),
            ..feed(&server)
        },
        fast(),
        topics(&["a"]),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(
        server.ping_count() >= 6,
        "pings stopped behind the full queue: {}",
        server.ping_count()
    );
    for n in 1..=10 {
        assert_eq!(next_event(&mut ws).await, Ev::Data(n));
    }
    // Had the blocked time counted as silence, the connection would be
    // replaced now, with a marker.
    quiet(&mut ws, Duration::from_millis(150)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_outage_on_a_declaring_protocol_yields_one_disconnected_then_one_reconnected() {
    let server = server(vec![
        after_replay(data([1])).then_close(),
        Script::reject(),
        Script::reject(),
        after_replay(data([2])),
    ])
    .await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    quiet(&mut ws, Duration::from_millis(200)).await;
    assert_eq!(server.connection_count(), 4);
}

#[tokio::test]
async fn an_outage_on_a_protocol_that_does_not_declare_yields_only_reconnected() {
    let server = server(vec![
        after_replay(data([1])).then_close(),
        Script::reject(),
        after_replay(data([2])),
    ])
    .await;
    let feed = Feed {
        declaring: false,
        ..feed(&server)
    };
    let mut ws = connect(feed, fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
}

#[tokio::test]
async fn membership_emptied_mid_outage_yields_reconnected_once_and_connects_no_more() {
    let server = server(vec![after_replay(data([1])).then_close(), Script::reject()]).await;
    let feed = Feed {
        record: true,
        ..feed(&server)
    };
    let membership = Topics {
        empty_unwanted: true,
        ..topics(&["a"])
    };
    let mut ws = connect(
        feed,
        backoff(fast(), Duration::from_millis(300)),
        membership,
    )
    .await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    let left = ws
        .membership()
        .apply(Change::Remove("a"))
        .await
        .expect("recorded during the outage");
    assert_eq!(left, 0);
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    the_end(&mut ws).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1, "connected after the end");
}

#[tokio::test]
async fn membership_emptied_on_a_healthy_connection_closes_it_politely_and_ends() {
    let server = server(vec![after_replay(data([1]))]).await;
    let membership = Topics {
        empty_unwanted: true,
        ..topics(&["a"])
    };
    let mut ws = connect(feed(&server), fast(), membership).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    let left = ws
        .membership()
        .apply(Change::Remove("a"))
        .await
        .expect("unsubscribed");
    assert_eq!(left, 0);
    // No marker: the stream just ends.
    the_end(&mut ws).await;
    server
        .wait_for("a close frame", |s| s.close_count() == 1)
        .await;
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn a_connection_is_rotated_at_its_maximum_age_and_its_membership_replayed() {
    let server = server(vec![Script::hold()]).await;
    let timing = Timing {
        max_connection_age: Some(Duration::from_millis(200)),
        ..fast()
    };
    let mut ws = connect(feed(&server), timing, topics(&["a", "b"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("rotation"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    server
        .wait_for("the rotated connection closed politely", |s| {
            s.close_count() >= 1
        })
        .await;
    let replays = server.first_frames();
    assert!(replays.len() >= 2, "{replays:?}");
    assert!(
        replays[1].contains(r#""topics":["a","b"]"#),
        "the replay after rotation: {}",
        replays[1]
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_change_held_during_an_outage_is_applied_after_the_reconnect() {
    let server = server(vec![after_replay(data([1])).then_close(), Script::hold()]).await;
    let mut ws = connect(
        feed(&server),
        backoff(fast(), Duration::from_millis(200)),
        topics(&["a"]),
    )
    .await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    let change = tokio::spawn({
        let handle = ws.membership();
        async move { handle.apply(Change::Add("b")).await }
    });
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    let held = change.await.expect("the change's task").expect("its ack");
    assert_eq!(held, 2);
    let on_the_new_connection: Vec<String> = server
        .received()
        .into_iter()
        .filter(|r| r.connection == 1)
        .map(|r| r.text)
        .collect();
    assert_eq!(on_the_new_connection.len(), 2, "{on_the_new_connection:?}");
    assert!(on_the_new_connection[0].contains(r#""topics":["a"]"#));
    assert!(on_the_new_connection[1].contains(r#""topics":["b"]"#));
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_change_recorded_during_an_outage_is_answered_at_once_and_replayed() {
    let server = server(vec![after_replay(data([1])).then_close(), Script::hold()]).await;
    let feed = Feed {
        record: true,
        ..feed(&server)
    };
    let mut ws = connect(
        feed,
        backoff(fast(), Duration::from_millis(300)),
        topics(&["a"]),
    )
    .await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    let recorded = tokio::time::timeout(
        Duration::from_millis(200),
        ws.membership().apply(Change::Add("b")),
    )
    .await
    .expect("answered during the backoff")
    .expect("recorded");
    assert_eq!(recorded, 2);
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert!(
        server.first_frames()[1].contains(r#""topics":["a","b"]"#),
        "{:?}",
        server.first_frames()
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn dropping_the_stream_ends_the_task_promptly() {
    // No ping or staleness deadline falls due for seconds, so only the
    // consumer's departure can end the task this soon.
    let server = server(vec![after_replay(data([1]))]).await;
    let timing = Timing {
        ping_interval: Some(Duration::from_secs(10)),
        stale_after: Duration::from_secs(30),
        ..fast()
    };
    let mut ws = connect(feed(&server), timing, topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    let handle = ws.membership();
    let dropped = Instant::now();
    drop(ws);
    server
        .wait_for("the client to go", |s| s.client_ended_count() == 1)
        .await;
    assert!(
        dropped.elapsed() < Duration::from_secs(1),
        "the task outlived its consumer by {:?}",
        dropped.elapsed()
    );
    assert_eq!(server.close_count(), 1, "it closed politely");
    assert!(matches!(
        handle.apply(Change::Add("b")).await,
        Err(TestError::Stopped)
    ));
}

#[tokio::test]
async fn close_does_not_wait_on_a_consumer_that_never_read() {
    let server = server(vec![after_replay(data(1..=100))]).await;
    let feed = Feed {
        queue: Queue::Bounded(4),
        ..feed(&server)
    };
    let ws = connect(feed, fast(), topics(&["a"])).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let closing = Instant::now();
    let closed = tokio::time::timeout(Duration::from_secs(2), ws.close()).await;
    assert!(matches!(closed, Ok(Ok(()))), "close: {closed:?}");
    assert!(closing.elapsed() < Duration::from_millis(1500));
}

#[tokio::test]
async fn a_skippable_error_is_yielded_and_the_connection_kept() {
    let mut frames = vec![Message::Text("not json".into())];
    frames.extend(data([1]));
    let server = server(vec![after_replay(frames)]).await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    assert!(matches!(next_item(&mut ws).await, Err(TestError::Bad(_))));
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_error_recovery_stops_on_is_yielded_and_ends_the_stream() {
    // The second connection refuses the replay, which no retry fixes.
    let server = server_refusing_from(
        vec![after_replay(data([1])).then_close(), Script::hold()],
        1,
    )
    .await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert!(matches!(
        next_item(&mut ws).await,
        Err(TestError::Refused(_))
    ));
    the_end(&mut ws).await;
    assert!(matches!(ws.close().await, Err(TestError::Stopped)));
}

#[tokio::test]
async fn silence_is_stale_and_the_connection_replaced() {
    let server = server(vec![
        after_replay(data([1])).go_silent(),
        after_replay(data([2])),
    ])
    .await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("stale"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
}

#[tokio::test]
async fn a_change_the_server_never_answers_is_stalled_and_the_connection_replaced() {
    let server = server(vec![Script::hold()]).await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    let handle = ws.membership();
    let result = tokio::time::timeout(WINDOW, handle.apply(Change::Add("hang")))
        .await
        .expect("the change returns");
    assert!(matches!(result, Err(TestError::Stalled(_))), "{result:?}");
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("stale"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_refused_or_hung_handshake_on_reconnect_is_retried() {
    let server = server(vec![
        after_replay(data([1])).then_close(),
        Script::refuse_upgrade(503),
        Script::stall(),
        Script::refuse_upgrade(408),
        after_replay(data([2])),
    ])
    .await;
    let timing = Timing {
        connect_timeout: Duration::from_millis(100),
        ..fast()
    };
    let mut ws = connect(feed(&server), timing, topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    assert_eq!(server.connection_count(), 5);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn connect_returns_a_first_failure_and_start_begins_in_an_outage() {
    let refusing = server(vec![Script::refuse_upgrade(503)]).await;
    let err = Supervisor::new(feed(&refusing), fast())
        .connect(topics(&["a"]))
        .await
        .expect_err("connect reports the first failure");
    assert!(
        matches!(&err, TestError::Ws(ws) if matches!(ws.kind(), Kind::Refused { status: 503, .. })),
        "{err:?}"
    );

    let recovering = server(vec![Script::refuse_upgrade(503), after_replay(data([1]))]).await;
    let mut ws = Supervisor::new(feed(&recovering), fast())
        .start(topics(&["a"]))
        .await
        .expect("a retriable first failure starts in an outage");
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("error"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    ws.close().await.expect("close");

    let missing = server(vec![Script::refuse_upgrade(404)]).await;
    let err = Supervisor::new(feed(&missing), fast())
        .start(topics(&["a"]))
        .await
        .expect_err("a failure recovery stops on is returned");
    assert!(matches!(err, TestError::Ws(_)), "{err:?}");
    assert_eq!(missing.connection_count(), 1);
}

#[tokio::test]
async fn pings_go_out_on_the_wall_clock_under_steady_traffic() {
    let server = server(vec![
        after_replay(data([1])).push_every(Duration::from_millis(10))
    ])
    .await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut updates = 0;
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, ws.next()).await {
        assert_eq!(event.expect("an event"), Ev::Data(1));
        updates += 1;
    }
    assert!(updates >= 20, "traffic stopped flowing: {updates} updates");
    assert!(
        server.ping_count() >= 5,
        "pings under traffic: {}",
        server.ping_count()
    );
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

/// A protocol that keeps every default, for the default recovery.
struct Plain;

impl Protocol for Plain {
    type Event = ();
    type Error = TestError;
    type Membership = ();
    type Change = ();
    type Ack = ();

    fn handshake(&mut self) -> Result<Request, TestError> {
        Ok("ws://localhost"
            .into_client_request()
            .map_err(WsError::from)?)
    }

    async fn replay(&mut self, _: &mut Connection, _: &()) -> Result<(), TestError> {
        Ok(())
    }

    async fn apply(&mut self, _: &mut Connection, _: &mut (), _: ()) -> Result<(), TestError> {
        Ok(())
    }

    fn decode(&mut self, _: &Message) -> Decoded<(), TestError> {
        Decoded::Nothing
    }

    fn is_live(&self, _: &Message, _: &Decoded<(), TestError>) -> bool {
        true
    }

    fn delivered(&self, _: &Message, _: &Decoded<(), TestError>) -> bool {
        false
    }

    fn ping(&mut self, _: &mut Connection) -> Option<Message> {
        None
    }

    fn is_retriable(err: &TestError) -> bool {
        <Feed as Protocol>::is_retriable(err)
    }

    fn reconnected(&mut self) {}

    fn queue(&self) -> Queue {
        Queue::Unbounded
    }
}

#[test]
fn the_default_recovery_reconnects_exactly_when_is_retriable() {
    let refused = |status: u16| {
        TestError::Ws(WsError::from(crate::tungstenite::Error::Http(
            crate::tungstenite::http::Response::builder()
                .status(status)
                .body(None)
                .unwrap(),
        )))
    };
    for err in [
        refused(503),
        refused(408),
        TestError::Stalled(Duration::from_secs(1)),
        TestError::Ws(WsError::closed(Some(1011), "")),
    ] {
        assert!(Plain::is_retriable(&err), "{err:?}");
        assert_eq!(Plain.recover(&err), Recovery::Reconnect, "{err:?}");
    }
    for err in [
        refused(404),
        TestError::Bad("x".into()),
        TestError::Refused("a".into()),
        TestError::Ws(WsError::from(crate::tungstenite::Error::AttackAttempt)),
    ] {
        assert!(!Plain::is_retriable(&err), "{err:?}");
        assert_eq!(Plain.recover(&err), Recovery::Stop, "{err:?}");
    }
    // The other defaults: held offline, always wanted, and no marker.
    assert!(matches!(Plain.record(&mut (), ()), Offline::Held(())));
    assert!(Plain::wanted(&()));
    assert!(Plain.disconnected(Outage::Rotation).is_none());
    assert!(matches!(Plain::stopped(), TestError::Ws(ws) if ws.kind() == Kind::AlreadyClosed));
    assert!(matches!(
        Plain::stalled(Duration::from_secs(2)),
        TestError::Ws(ws) if ws.kind() == Kind::NoAnswer { after: Duration::from_secs(2) }
    ));
}
