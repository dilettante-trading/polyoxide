//! The supervised tier on the kit's `Supervisor` (Story 4.7): the behaviour
//! changes it brought (L2-D1, L2-D2, L2-D8) and the bundle's matrix rows
//! `supervision_usdm.rs` and `supervision_usdm_edges.rs` do not pin. Each test names
//! the rule it holds.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::Symbol,
    ws::{
        fixtures, DisconnectReason, Event, StreamName, StreamPath, SupervisedUsdmWs, UsdmWsBuilder,
        UsdmWsError,
    },
};
use polyoxide_venue::{Class, Classify};
use tokio::time::Instant;

// Binance's answer on the shared scripted server, as the unit tests use it.
#[path = "../src/usdm/ws/scripted.rs"]
mod scripted;

use scripted::{Script, ScriptedServer};

fn symbol(s: &str) -> Symbol {
    Symbol::new(s).unwrap()
}

fn agg(s: &str) -> StreamName {
    StreamName::AggTrade(symbol(s))
}

fn btc_mark() -> StreamName {
    StreamName::MarkPrice(symbol("BTCUSDT"))
}

fn book() -> StreamName {
    StreamName::BookTicker(symbol("BTCUSDT"))
}

/// `n` aggregate-trade streams of unlisted symbols, `ZZ0000USDT` on.
fn many(n: usize) -> Vec<StreamName> {
    (0..n).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect()
}

fn fast(server: &ScriptedServer) -> UsdmWsBuilder {
    UsdmWsBuilder::new()
        .base_url(&server.url)
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
}

/// Frames the server pushes once the subscription is answered, then every
/// 50 ms: steady traffic.
fn steady(frame: &str) -> Script {
    Script {
        pushes: vec![frame.into()],
        push_every: Some(Duration::from_millis(50)),
        ..Default::default()
    }
}

/// One push, then a close.
fn flap(frame: &str) -> Script {
    Script {
        pushes: vec![frame.into()],
        close_after: true,
        ..Default::default()
    }
}

fn reject() -> Script {
    Script {
        reject_handshake: true,
        ..Default::default()
    }
}

/// An upgrade refused with `status`, and a `Retry-After` when one is given.
fn refuse_upgrade(status: u16, retry_after: Option<&str>) -> Script {
    let headers = retry_after
        .map(|value| vec![("Retry-After".to_owned(), value.to_owned())])
        .unwrap_or_default();
    Script {
        refuse_upgrade: Some((status, headers)),
        ..Default::default()
    }
}

async fn next(ws: &mut SupervisedUsdmWs) -> Result<Event, UsdmWsError> {
    tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("an event within 3 s")
        .expect("the stream is open")
}

async fn next_event(ws: &mut SupervisedUsdmWs) -> Event {
    next(ws).await.expect("an event, not an error")
}

/// The next event that is not an update.
async fn next_marker(ws: &mut SupervisedUsdmWs) -> Event {
    loop {
        match next_event(ws).await {
            Event::Update(_) => {}
            marker => return marker,
        }
    }
}

fn names_on(server: &ScriptedServer, connection: usize) -> Vec<String> {
    server
        .received()
        .into_iter()
        .filter(|r| r.connection == connection)
        .flat_map(|r| r.request["params"].as_array().cloned().unwrap_or_default())
        .map(|n| n.as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_change_the_server_leaves_unanswered_reads_as_a_stale_outage_l2_d1() {
    // The server never answers the subscribe, so the change outlives the
    // staleness window: the caller gets `Ok`, as the change is recorded, and
    // the outage is `Stale`, where the old loop's 10 s answer timeout made it
    // `Error(NoAnswer)`.
    let server = ScriptedServer::start(vec![
        Script {
            ignore_later_requests: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    ws.membership().subscribe([agg("ETHUSDT")]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Market,
            reason: DisconnectReason::Stale
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Market
        }
    ));
    assert_eq!(
        names_on(&server, 1),
        ["btcusdt@markPrice@1s", "ethusdt@aggTrade"]
    );
    ws.close().await.unwrap();
}

/// A connection that answers its replay, ignores every later request, and is
/// closed 500 ms after its handshake: a change sent after the 200 ms pacing
/// is in flight when the close arrives.
fn closes_under_a_change() -> Script {
    Script {
        ignore_later_requests: true,
        drop_after: Some(Duration::from_millis(500)),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_subscribe_the_connection_closes_under_is_answered_ok_and_replayed() {
    // The connection fails while the request is in flight, well inside the
    // staleness window: the change is recorded, its caller gets `Ok`, the
    // outage is the server's close, and the replay carries the change.
    let server = ScriptedServer::start(vec![closes_under_a_change(), Script::default()]).await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    ws.membership().subscribe([agg("ETHUSDT")]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Market,
            reason: DisconnectReason::Closed { .. }
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Market
        }
    ));
    assert_eq!(
        names_on(&server, 1),
        ["btcusdt@markPrice@1s", "ethusdt@aggTrade"]
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_unsubscribe_the_connection_closes_under_is_answered_ok_and_not_replayed() {
    let server = ScriptedServer::start(vec![closes_under_a_change(), Script::default()]).await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams([btc_mark(), agg("ETHUSDT")])
        .connect()
        .await
        .unwrap();
    ws.membership().unsubscribe([agg("ETHUSDT")]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Market,
            reason: DisconnectReason::Closed { .. }
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Market
        }
    ));
    assert_eq!(names_on(&server, 1), ["btcusdt@markPrice@1s"]);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_failed_first_connect_of_a_new_path_is_an_error_outage() {
    // The reason carries the connect's error, which the old loop reported
    // too: the outage of a path a membership change opened.
    let server = ScriptedServer::start(vec![
        Script::default(),
        reject(),
        Script {
            pushes: vec![fixtures::BOOK_TICKER.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    ws.membership().subscribe([book()]).await.unwrap();
    let Event::Disconnected { path, reason } = next_event(&mut ws).await else {
        panic!("expected Disconnected");
    };
    assert_eq!(path, StreamPath::Public);
    assert!(
        matches!(reason, DisconnectReason::Error(UsdmWsError::Connect(_))),
        "{reason:?}"
    );
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Public
        }
    ));
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_path_reopened_while_its_old_connection_drains_takes_changes() {
    // The public path's last stream leaves and returns at once, so a new
    // supervisor opens it while the old one closes. The old one's end must
    // not make the router forget the new one: a change to the reopened path
    // is still sent to it. Both changes are queued together, so the router
    // takes the subscribe before the old path's end reaches it, which it
    // then reads only after it has opened the new path.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .streams([btc_mark(), book()])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    let (left, back) = tokio::join!(
        membership.unsubscribe([book()]),
        membership.subscribe([book()])
    );
    left.unwrap();
    back.unwrap();
    server
        .wait_for("the old public connection's polite close", |s| {
            s.close_count() == 1
        })
        .await;
    // Room for the old path's end to reach the router.
    tokio::time::sleep(Duration::from_millis(200)).await;
    membership.unsubscribe([book()]).await.unwrap();
    server
        .wait_for("the reopened public connection's polite close", |s| {
            s.close_count() == 2
        })
        .await;
    assert_eq!(server.connection_count(), 3);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_refused_change_keeps_the_connection_delivering() {
    let server = ScriptedServer::start(vec![Script {
        refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
        ..steady(fixtures::MARK_PRICE)
    }])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let err = ws
        .membership()
        .subscribe([agg("ETHUSDT")])
        .await
        .unwrap_err();
    assert!(
        matches!(err, UsdmWsError::Refused { code: 2, .. }),
        "{err:?}"
    );
    for _ in 0..3 {
        assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    }
    assert_eq!(server.connection_count(), 1);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn emptying_a_healthy_path_sends_nothing_and_closes_it_politely() {
    // The market path is connection 0, the public one connection 1.
    let server = ScriptedServer::start(vec![steady(fixtures::MARK_PRICE), Script::default()]).await;
    let mut ws = fast(&server)
        .streams([btc_mark(), book()])
        .connect()
        .await
        .unwrap();
    let sent = server.received().len();
    ws.membership().unsubscribe([book()]).await.unwrap();
    server
        .wait_for("the public path's polite close", |s| s.close_count() == 1)
        .await;
    assert_eq!(server.received().len(), sent, "an UNSUBSCRIBE went out");
    // No marker, and the market path carries on.
    for _ in 0..3 {
        assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    }
    assert_eq!(server.connection_count(), 2);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_path_emptied_mid_outage_reconnects_once_and_the_other_path_continues() {
    let server = ScriptedServer::start(vec![
        steady(fixtures::MARK_PRICE),
        flap(fixtures::BOOK_TICKER),
        reject(),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(200), Duration::from_millis(200))
        .streams([btc_mark(), book()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(
        next_marker(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Public,
            ..
        }
    ));
    ws.membership().unsubscribe([book()]).await.unwrap();
    assert!(matches!(
        next_marker(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Public
        }
    ));
    // The merged stream goes on with the market path's updates, and nothing
    // else.
    let until = Instant::now() + Duration::from_millis(500);
    let mut updates = 0;
    while let Ok(event) = tokio::time::timeout_at(until, ws.next()).await {
        let event = event.expect("the stream is open").expect("an event");
        assert!(matches!(event, Event::Update(_)), "{event:?}");
        updates += 1;
    }
    assert!(updates >= 3, "{updates} updates");
    assert_eq!(server.connection_count(), 2, "the emptied path reconnected");
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_stop_on_one_path_ends_the_whole_stream_and_the_handle() {
    // The public path's replay is refused after a drop; the market path is
    // healthy throughout.
    let server = ScriptedServer::start(vec![
        Script::default(),
        flap(fixtures::BOOK_TICKER),
        Script {
            refuse: vec![("btcusdt@bookTicker".into(), 2, "Invalid request".into())],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server)
        .streams([btc_mark(), book()])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Public,
            ..
        }
    ));
    assert!(matches!(
        next(&mut ws).await,
        Err(UsdmWsError::Refused { code: 2, .. })
    ));
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");
    assert!(matches!(
        membership.subscribe([agg("ETHUSDT")]).await,
        Err(UsdmWsError::Stopped { .. })
    ));
    server
        .wait_for("the market path's polite close", |s| s.close_count() == 1)
        .await;
}

#[tokio::test]
async fn a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11() {
    // A 10 ms step and a 400 ms wait: the attempt after the refusal waits the
    // 400 ms, where it retried inside the server's window.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MARK_PRICE),
        refuse_upgrade(503, Some("0.4")),
        steady(fixtures::MARK_PRICE),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(10), Duration::from_secs(5))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Market,
            ..
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Market
        }
    ));
    let accepted = server.accepted_at();
    let waited = accepted[2] - accepted[1];
    assert!(waited >= Duration::from_millis(400), "{waited:?}");
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_path_stopped_by_a_451_keeps_its_cause_m_d8() {
    // A reconnect refused with 451 stops the client. `Stopped` carries the
    // refusal's class and fault, so a stream stopped by a region block is
    // `Restricted`, not a fault, and tags environmental rather than real.
    let server =
        ScriptedServer::start(vec![flap(fixtures::MARK_PRICE), refuse_upgrade(451, None)]).await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Market,
            ..
        }
    ));
    let refusal = next(&mut ws).await.expect_err("the refusal");
    assert!(matches!(refusal, UsdmWsError::Connect(_)), "{refusal:?}");
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");

    let stopped = ws.close().await.expect_err("a stop on an error");
    let UsdmWsError::Stopped { cause: Some(cause) } = &stopped else {
        panic!("expected a stop with its cause, got {stopped:?}");
    };
    assert_eq!(cause.message, refusal.to_string());
    assert_eq!(stopped.class(), Class::Restricted);
    assert!(!stopped.is_fault(), "{stopped:?}");

    let later = membership
        .subscribe([agg("ETHUSDT")])
        .await
        .expect_err("the client has stopped");
    assert_eq!(later.class(), Class::Restricted);
    assert!(!later.is_fault(), "{later:?}");
}

#[tokio::test]
async fn a_slow_consumer_keeps_the_pings_going_and_loses_nothing_l2_d2() {
    // More updates than the 1024-event queue holds, and nobody reading for
    // two staleness windows: the path stops reading but keeps pinging, and
    // the wait is not silence, so nothing is replaced and nothing lost. The
    // old loop stopped pinging behind a full queue.
    let server = ScriptedServer::start(vec![Script {
        pushes: vec![fixtures::MARK_PRICE.into(); 1100],
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    server
        .wait_for("the first ping", |s| s.ping_count() >= 1)
        .await;
    let before = server.ping_count();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let during = server.ping_count() - before;
    assert!(during >= 6, "pings behind the full queue: {during}");
    for n in 0..1100 {
        assert!(
            matches!(next_event(&mut ws).await, Event::Update(_)),
            "update {n}"
        );
    }
    let after = tokio::time::timeout(Duration::from_millis(150), ws.next()).await;
    assert!(after.is_err(), "expected nothing more, got {after:?}");
    assert_eq!(server.connection_count(), 1);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_rotation_resets_the_backoff_after_a_connection_that_delivered() {
    // Three failed attempts take the schedule to 400 ms. The connection that
    // follows delivers and then rotates; its replacement is refused once, and
    // the wait after that is the reset 50 ms, where the old loop waited the
    // 400 ms it had reached.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MARK_PRICE),
        reject(),
        reject(),
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            ..Default::default()
        },
        reject(),
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(50), Duration::from_millis(800))
        .max_connection_age(Duration::from_millis(400))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            reason: DisconnectReason::Rotation,
            ..
        }
    ));
    let rotated = Instant::now();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(
        rotated.elapsed() < Duration::from_millis(300),
        "the backoff was not reset: {:?}",
        rotated.elapsed()
    );
    assert_eq!(server.connection_count(), 6);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_replay_is_bounded_as_a_whole_by_stale_after() {
    // 600 streams are three requests 200 ms apart, each answered at once:
    // past a 300 ms window as a whole. The old loop bounded each request
    // alone, so it connected.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let err = fast(&server)
        .streams(many(600))
        .connect()
        .await
        .unwrap_err();
    assert!(matches!(err, UsdmWsError::NoAnswer { .. }), "{err:?}");
    assert!(server.received().len() >= 2);
}

#[tokio::test]
async fn a_partly_refused_subscribe_keeps_the_batches_answered() {
    // 250 streams are two requests; the second names a stream the server
    // refuses. The first 200 stay subscribed, and the replay after a drop
    // carries them, but not the refused batch. The change's pacing takes
    // most of the default test window, so the window is wider here.
    let server = ScriptedServer::start(vec![
        Script {
            refuse: vec![("zz0210usdt@aggTrade".into(), 2, "Invalid request".into())],
            drop_after: Some(Duration::from_millis(1200)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let err = ws.membership().subscribe(many(250)).await.unwrap_err();
    assert!(
        matches!(err, UsdmWsError::Refused { code: 2, .. }),
        "{err:?}"
    );
    // An answered stream is known, so subscribing it again sends nothing.
    let sent = server.received().len();
    ws.membership().subscribe(many(1)).await.unwrap();
    assert_eq!(server.received().len(), sent);
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    let mut expected = vec!["btcusdt@markPrice@1s".to_owned()];
    expected.extend(many(200).iter().map(ToString::to_string));
    assert_eq!(names_on(&server, 1), expected);
    ws.close().await.unwrap();
}

/// Records the target and level of every event, for the log-target test.
#[derive(Clone, Default)]
struct Targets(Arc<Mutex<Vec<(String, tracing::Level)>>>);

impl tracing::Subscriber for Targets {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let metadata = event.metadata();
        self.0
            .lock()
            .unwrap()
            .push((metadata.target().to_owned(), *metadata.level()));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

#[tokio::test]
async fn supervision_logs_under_the_polyoxide_ws_target() {
    // The current-thread runtime runs every path's task on this thread, so
    // the scoped subscriber sees their events. The old loop warned under
    // `polyoxide_binance`.
    let targets = Targets::default();
    let _scope = tracing::subscriber::set_default(targets.clone());
    // With one dispatcher registered, tracing-core works out the interest of
    // a callsite another test's thread hits first from that thread's
    // default, which has none, and caches it as never for every thread. With
    // a second one registered it asks each registered dispatcher instead.
    let _second = tracing::Dispatch::new(Targets::default());
    let server = ScriptedServer::start(vec![flap(fixtures::MARK_PRICE), Script::default()]).await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    let warnings: Vec<String> = targets
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, level)| *level == tracing::Level::WARN)
        .map(|(target, _)| target.clone())
        .collect();
    assert!(
        !warnings.is_empty() && warnings.iter().all(|t| t.starts_with("polyoxide_ws")),
        "the reconnect's warnings: {warnings:?}"
    );
    ws.close().await.unwrap();
}
