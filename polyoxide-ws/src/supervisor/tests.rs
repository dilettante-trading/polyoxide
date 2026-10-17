//! The Supervisor's rules (AD-11), each pinned against the shared scripted
//! server with a test protocol: JSON `{"id", "op", "topics"}` requests
//! answered `{"id", "ok"}`, `{"data": n}` and `{"burst": [..]}` pushes, and
//! protocol pings. It counts any frame as liveness, as one venue does, and
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
    /// Whether an empty membership stops being wanted, as a venue's paths do.
    empty_unwanted: bool,
}

#[derive(Debug)]
enum Change {
    Add(&'static str),
    Remove(&'static str),
}

/// How long a change to the topic `slow` takes before its request, as a
/// venue's retries of a rate-limited subscribe do.
const SLOW: Duration = Duration::from_millis(200);

/// The test protocol.
struct Feed {
    url: String,
    declaring: bool,
    /// Record a change made during an outage at once, as one venue does,
    /// rather than hold it, as another does.
    record: bool,
    queue: Queue,
    /// Requests on each connection no sooner than this apart, as one venue
    /// paces its own, set through `configure`.
    pacing: Option<Duration>,
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

    fn configure(&self, connection: Connection) -> Connection {
        match self.pacing {
            Some(interval) => connection.min_request_interval(interval),
            None => connection,
        }
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
                if topic == "slow" {
                    tokio::time::sleep(SLOW).await;
                }
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

    /// A refused upgrade's `Retry-After`, in seconds, as a venue's class
    /// reads it.
    fn retry_after(&self, err: &TestError) -> Option<Duration> {
        let TestError::Ws(err) = err else {
            return None;
        };
        match err.kind() {
            Kind::Refused {
                retry_after: Some(value),
                ..
            } => value.parse().ok().map(Duration::from_secs_f64),
            _ => None,
        }
    }

    fn disconnected(&mut self, outage: Outage<TestError>) -> Option<Ev> {
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
/// except one naming the topic `hang`, which is never answered. One naming
/// `slow` is answered after a push, `{"data": 7}`, which the request keeps.
fn answer(refuse: bool) -> impl FnMut(&str) -> Vec<Message> + Send {
    move |request| {
        let Ok(request) = serde_json::from_str::<Value>(request) else {
            return Vec::new();
        };
        let names = |name: &str| {
            request["topics"]
                .as_array()
                .is_some_and(|topics| topics.iter().any(|t| t == name))
        };
        if names("hang") {
            return Vec::new();
        }
        let mut reply = Vec::new();
        if names("slow") {
            reply.push(text(json!({"data": 7})));
        }
        reply.push(text(json!({"id": request["id"], "ok": !refuse})));
        reply
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
        pacing: None,
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

/// A refusal with a `Retry-After` of `seconds`.
fn refused_for(seconds: &str) -> Script {
    Script::refuse_upgrade(503).with_header("Retry-After", seconds)
}

/// The time between the `n`-th accepted connection and the one before it.
fn gap(server: &ScriptedServer, n: usize) -> Duration {
    let accepted = server.accepted_at();
    accepted[n] - accepted[n - 1]
}

/// Steps of 20 ms under a ceiling of `ceiling`.
fn ceiling(ceiling: Duration) -> Timing {
    Timing {
        initial_backoff: Duration::from_millis(20),
        max_backoff: ceiling,
        ..fast()
    }
}

#[tokio::test]
async fn a_reconnect_waits_out_a_refusals_retry_after_drift_r11() {
    // A 20 ms step and a 400 ms wait: the attempt after the refusal waits
    // the 400 ms, where it waited the step, inside the server's window.
    let server = server(vec![
        after_replay(data([1])).then_close(),
        refused_for("0.4"),
        after_replay(data([2])),
    ])
    .await;
    let timing = ceiling(Duration::from_secs(5));
    let mut ws = connect(feed(&server), timing, topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    let waited = gap(&server, 2);
    assert!(waited >= Duration::from_millis(400), "{waited:?}");
    assert!(waited < Duration::from_secs(4), "{waited:?}");
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_retry_after_past_the_ceiling_waits_the_ceiling_drift_r11() {
    let server = server(vec![
        after_replay(data([1])).then_close(),
        refused_for("600"),
        after_replay(data([2])),
    ])
    .await;
    let timing = ceiling(Duration::from_millis(300));
    let mut ws = connect(feed(&server), timing, topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    let waited = gap(&server, 2);
    assert!(waited >= Duration::from_millis(300), "{waited:?}");
    assert!(waited < Duration::from_secs(2), "{waited:?}");
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_start_in_an_outage_waits_out_the_first_refusals_retry_after_drift_r11() {
    let server = server(vec![refused_for("0.4"), after_replay(data([1]))]).await;
    let mut ws = Supervisor::new(feed(&server), ceiling(Duration::from_secs(5)))
        .start(topics(&["a"]))
        .await
        .expect("a retriable first failure starts in an outage");
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("error"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    let waited = gap(&server, 1);
    assert!(waited >= Duration::from_millis(400), "{waited:?}");
    ws.close().await.expect("close");
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

#[tokio::test]
async fn a_close_read_during_a_change_ends_the_connection_as_closed() {
    // The close arrives while the `hang` change awaits its answer, and a ping
    // falls due before the next read: the outage is the server's close, not
    // a failed send on the closed socket.
    let server = server(vec![
        Script::hold().drop_after(Duration::from_millis(150)),
        Script::hold(),
    ])
    .await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    let change = tokio::time::timeout(WINDOW, ws.membership().apply(Change::Add("hang")))
        .await
        .expect("the change returns");
    assert!(matches!(change, Err(TestError::Ws(_))), "{change:?}");
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn frames_a_change_kept_are_read_before_staleness_is_judged() {
    // Nothing is read while the `slow` change takes most of the window, so
    // the connection has been silent for longer than the window when it
    // returns; the push its request kept is that silence's end.
    let server = server(vec![Script::hold()]).await;
    let timing = Timing {
        ping_interval: None,
        ..fast()
    };
    let mut ws = connect(feed(&server), timing, topics(&["a"])).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    let members = ws
        .membership()
        .apply(Change::Add("slow"))
        .await
        .expect("applied");
    assert_eq!(members, 2);
    assert_eq!(next_event(&mut ws).await, Ev::Data(7));
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn dropping_the_stream_during_a_change_ends_the_task_promptly() {
    let server = server(vec![Script::hold()]).await;
    let timing = Timing {
        ping_interval: Some(Duration::from_secs(10)),
        stale_after: Duration::from_secs(30),
        ..fast()
    };
    let ws = connect(feed(&server), timing, topics(&["a"])).await;
    let change = tokio::spawn({
        let handle = ws.membership();
        async move { handle.apply(Change::Add("hang")).await }
    });
    server
        .wait_for("the change's request", |s| {
            s.client_frames().iter().any(|f| f.contains("hang"))
        })
        .await;
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
    let answer = change.await.expect("the change's task");
    assert!(matches!(answer, Err(TestError::Stopped)), "{answer:?}");
}

#[tokio::test]
async fn a_replay_the_server_never_answers_is_stalled_and_retried() {
    // The second connection never answers the replay.
    let server = ScriptedServer::start_answering(
        vec![
            after_replay(data([1])).then_close(),
            Script::hold(),
            after_replay(data([2])),
        ],
        |index| {
            let mut answer = answer(false);
            move |request: &str| {
                if index == 1 {
                    Vec::new()
                } else {
                    answer(request)
                }
            }
        },
    )
    .await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    assert_eq!(server.connection_count(), 3);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn connect_returns_stalled_when_the_first_replay_goes_unanswered() {
    let server = server(vec![Script::hold()]).await;
    let err = Supervisor::new(feed(&server), fast())
        .connect(topics(&["hang"]))
        .await
        .expect_err("the replay is never answered");
    assert!(matches!(err, TestError::Stalled(_)), "{err:?}");
}

#[tokio::test]
async fn configure_shapes_every_connection_before_its_replay() {
    // Paced requests, on the first connection and on the one after a
    // reconnect: each change waits out the pacing after the replay.
    let pacing = Duration::from_millis(200);
    let server = server(vec![
        after_replay(data([1])).drop_after(Duration::from_millis(600)),
        Script::hold(),
    ])
    .await;
    let feed = Feed {
        pacing: Some(pacing),
        ..feed(&server)
    };
    let mut ws = connect(feed, fast(), topics(&["a"])).await;
    ws.membership()
        .apply(Change::Add("b"))
        .await
        .expect("applied");
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    ws.membership()
        .apply(Change::Add("c"))
        .await
        .expect("applied");
    for connection in 0..2 {
        let at: Vec<Instant> = server
            .received()
            .into_iter()
            .filter(|r| r.connection == connection)
            .map(|r| r.at)
            .collect();
        assert_eq!(at.len(), 2, "connection {connection}: {at:?}");
        let gap = at[1] - at[0];
        assert!(
            gap >= pacing - Duration::from_millis(10),
            "connection {connection}: requests {gap:?} apart"
        );
    }
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
    assert!(Plain
        .disconnected(Outage::Error(TestError::Bad("x".into())))
        .is_none());
    assert!(matches!(Plain::stopped(), TestError::Ws(ws) if ws.kind() == Kind::AlreadyClosed));
    assert!(matches!(
        Plain::stalled(Duration::from_secs(2)),
        TestError::Ws(ws) if ws.kind() == Kind::NoAnswer { after: Duration::from_secs(2) }
    ));
}

#[test]
fn the_default_retry_after_is_none() {
    // A protocol that does not read its errors' waits keeps the backoff's
    // own schedule after a refusal.
    let refused = TestError::Ws(WsError::from(crate::tungstenite::Error::Http(
        crate::tungstenite::http::Response::builder()
            .status(429)
            .header("Retry-After", "30")
            .body(None)
            .unwrap(),
    )));
    assert_eq!(Plain.retry_after(&refused), None);
}

use std::{
    ops::Range,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

/// A protocol that dials for itself, as a venue that tries each address of
/// its host in turn does. Its `handshake` fails if anything calls it, so only
/// a supervisor that dials through `connect` gets a connection. Its `connect`
/// opens the scripted server, counts every dial, and fails the dials numbered
/// in `failing` (the first is 0) with `failure`. The rest is the file's
/// `Feed`.
struct Dialer {
    feed: Feed,
    dials: Arc<AtomicUsize>,
    failing: Range<usize>,
    failure: fn() -> TestError,
}

fn dialer(
    server: &ScriptedServer,
    dials: &Arc<AtomicUsize>,
    failing: Range<usize>,
    failure: fn() -> TestError,
) -> Dialer {
    Dialer {
        feed: feed(server),
        dials: Arc::clone(dials),
        failing,
        failure,
    }
}

impl Protocol for Dialer {
    type Event = Ev;
    type Error = TestError;
    type Membership = Topics;
    type Change = Change;
    type Ack = usize;

    fn handshake(&mut self) -> Result<Request, TestError> {
        Err(TestError::Refused(
            "the supervisor called handshake itself".into(),
        ))
    }

    fn connect(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<crate::Socket, TestError>> + Send {
        let dial = self.dials.fetch_add(1, Ordering::SeqCst);
        let failure = self.failing.contains(&dial).then_some(self.failure);
        let url = self.feed.url.clone();
        async move {
            if let Some(failure) = failure {
                return Err(failure());
            }
            let request = url.as_str().into_client_request().map_err(WsError::from)?;
            let socket = crate::connect(request, timeout).await?;
            Ok::<_, TestError>(socket)
        }
    }

    async fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &Topics,
    ) -> Result<(), TestError> {
        self.feed.replay(connection, membership).await
    }

    async fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut Topics,
        change: Change,
    ) -> Result<usize, TestError> {
        self.feed.apply(connection, membership, change).await
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Ev, TestError> {
        self.feed.decode(frame)
    }

    fn is_live(&self, frame: &Message, decoded: &Decoded<Ev, TestError>) -> bool {
        self.feed.is_live(frame, decoded)
    }

    fn delivered(&self, frame: &Message, decoded: &Decoded<Ev, TestError>) -> bool {
        self.feed.delivered(frame, decoded)
    }

    fn ping(&mut self, connection: &mut Connection) -> Option<Message> {
        self.feed.ping(connection)
    }

    fn is_retriable(err: &TestError) -> bool {
        <Feed as Protocol>::is_retriable(err)
    }

    fn disconnected(&mut self, outage: Outage<TestError>) -> Option<Ev> {
        self.feed.disconnected(outage)
    }

    fn reconnected(&mut self) -> Ev {
        self.feed.reconnected()
    }

    fn queue(&self) -> Queue {
        self.feed.queue()
    }
}

#[tokio::test]
async fn connect_dials_every_attempt_and_the_stream_reads_its_socket() {
    // Two connections: the first closes after one event, the second serves
    // another. `Dialer::handshake` fails if called, so every frame read here
    // came over a socket `connect` returned, for the first attempt and for
    // the reconnect.
    let server = server(vec![
        after_replay(data([1])).then_close(),
        after_replay(data([2])),
    ])
    .await;
    let dials = Arc::new(AtomicUsize::new(0));
    let mut ws = Supervisor::new(dialer(&server, &dials, 0..0, || TestError::Stopped), fast())
        .connect(topics(&["a"]))
        .await
        .expect("the first attempt dials through connect");
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    assert_eq!(dials.load(Ordering::SeqCst), 2);
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_error_from_connect_is_a_failed_attempt() {
    let server = server(vec![after_replay(data([1]))]).await;

    // `Supervisor::connect` returns the first attempt's failure and dials
    // nothing more.
    let dials = Arc::new(AtomicUsize::new(0));
    let err = Supervisor::new(
        dialer(&server, &dials, 0..1, || {
            TestError::Refused("no route".into())
        }),
        fast(),
    )
    .connect(topics(&["a"]))
    .await
    .expect_err("connect returns the dial's failure");
    assert!(
        matches!(&err, TestError::Refused(why) if why == "no route"),
        "{err:?}"
    );
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(server.connection_count(), 0);

    // `start` retries a retriable one, the second dial failing too: an
    // outage with its marker, then the attempt that connects.
    let dials = Arc::new(AtomicUsize::new(0));
    let mut ws = Supervisor::new(
        dialer(&server, &dials, 0..2, || {
            TestError::Stalled(Duration::from_millis(1))
        }),
        fast(),
    )
    .start(topics(&["a"]))
    .await
    .expect("a retriable dial failure starts in an outage");
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("error"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(dials.load(Ordering::SeqCst), 3);
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");

    // and `start` returns one recovery stops on.
    let dials = Arc::new(AtomicUsize::new(0));
    let err = Supervisor::new(
        dialer(&server, &dials, 0..1, || {
            TestError::Refused("no route".into())
        }),
        fast(),
    )
    .start(topics(&["a"]))
    .await
    .expect_err("a dial failure recovery stops on is returned");
    assert!(matches!(err, TestError::Refused(_)), "{err:?}");
    assert_eq!(dials.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_dial_failure_recovery_stops_on_ends_a_running_stream_after_it_is_yielded() {
    // The first dial connects; the reconnect's dial fails in a way recovery
    // stops on, so the stream yields it and ends.
    let server = server(vec![after_replay(data([1])).then_close()]).await;
    let dials = Arc::new(AtomicUsize::new(0));
    let mut ws = Supervisor::new(
        dialer(&server, &dials, 1..2, || {
            TestError::Refused("no route".into())
        }),
        fast(),
    )
    .connect(topics(&["a"]))
    .await
    .expect("the first dial connects");
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    let failed = next_item(&mut ws).await;
    assert!(
        matches!(&failed, Err(TestError::Refused(why)) if why == "no route"),
        "{failed:?}"
    );
    let end = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(end.is_none(), "{end:?}");
    assert_eq!(dials.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn the_default_connect_reports_a_handshake_that_cannot_be_built() {
    // `Feed` keeps the default `connect`: the handshake's failure is the
    // attempt's, and nothing is dialled.
    let server = server(vec![Script::hold()]).await;
    let unbuildable = Feed {
        url: "not a url".into(),
        ..feed(&server)
    };
    let err = Supervisor::new(unbuildable, fast())
        .connect(topics(&["a"]))
        .await
        .expect_err("the request cannot be built");
    assert!(matches!(err, TestError::Ws(_)), "{err:?}");
    assert_eq!(server.connection_count(), 0);
}
