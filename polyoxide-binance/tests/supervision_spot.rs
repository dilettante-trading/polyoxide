//! The supervised tier against the scripted server, with limits in hundreds of
//! milliseconds. Each test names the bug it exists to catch.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_binance::spot::{
    types::Symbol,
    ws::{
        fixtures, DepthLevels, DepthSpeed, DisconnectReason, Event, SpotWsBuilder, SpotWsError,
        StreamName, StreamPath, SupervisedSpotWs,
    },
};
use polyoxide_venue::{Class, Classify};

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

fn btc_mini() -> StreamName {
    StreamName::MiniTicker(symbol("BTCUSDT"))
}

/// `n` aggregate-trade streams of unlisted symbols, `ZZ0000USDT` on.
fn many(n: usize) -> Vec<StreamName> {
    (0..n).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect()
}

fn fast(server: &ScriptedServer) -> SpotWsBuilder {
    SpotWsBuilder::new()
        .base_url(&server.url)
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
}

/// One push, then a close.
fn flap(frame: &str) -> Script {
    Script {
        pushes: vec![frame.into()],
        close_after: true,
        ..Default::default()
    }
}

/// One push, then steady traffic.
fn steady(frame: &str) -> Script {
    Script {
        pushes: vec![frame.into()],
        push_every: Some(Duration::from_millis(50)),
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

async fn next(ws: &mut SupervisedSpotWs) -> Result<Event, SpotWsError> {
    tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("an event within 3 s")
        .expect("the stream is open")
}

async fn next_event(ws: &mut SupervisedSpotWs) -> Event {
    next(ws).await.expect("an event, not an error")
}

#[tokio::test]
async fn every_kind_shares_the_one_connection_on_the_one_path() {
    // Bug: routing streams by kind as USDⓈ-M does, which opens a second
    // connection spot does not serve.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let depth = StreamName::PartialDepth(symbol("BTCUSDT"), DepthLevels::Twenty, DepthSpeed::Ms100);
    let ws = fast(&server)
        .streams([
            agg("BTCUSDT"),
            depth,
            StreamName::BookTicker(symbol("BTCUSDT")),
            StreamName::AllMiniTickers,
        ])
        .connect()
        .await
        .unwrap();
    assert_eq!(server.paths(), ["/stream"]);
    assert_eq!(server.connection_count(), 1);
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "SUBSCRIBE");
    assert_eq!(
        requests[0]["params"],
        serde_json::json!([
            "btcusdt@aggTrade",
            "btcusdt@depth20@100ms",
            "btcusdt@bookTicker",
            "!miniTicker@arr"
        ])
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_request_carries_all_1024_names() {
    // Bug: batching at USDⓈ-M's 200 names, six requests at 500 ms apiece where
    // one request of 1024 names was measured to hold.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(many(1024))
        .connect()
        .await
        .unwrap();
    let sizes: Vec<usize> = server
        .requests()
        .iter()
        .map(|r| r["params"].as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, [1024]);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn the_1025th_stream_is_refused_and_nothing_reaches_the_server() {
    // Bug: sending it, which makes the server close the connection and lose
    // all 1024.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(many(1024))
        .connect()
        .await
        .unwrap();
    let sent = server.received().len();
    let err = ws
        .membership()
        .subscribe([agg("BTCUSDT")])
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            SpotWsError::TooManyStreams {
                path: StreamPath::Stream,
                limit: 1024
            }
        ),
        "{err:?}"
    );
    assert_eq!(server.received().len(), sent);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn connecting_with_1025_names_is_an_error_and_sends_nothing() {
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let err = fast(&server)
        .streams(many(1025))
        .connect()
        .await
        .unwrap_err();
    assert!(
        matches!(err, SpotWsError::TooManyStreams { limit: 1024, .. }),
        "{err:?}"
    );
    assert!(server.received().is_empty());
}

#[tokio::test]
async fn a_membership_change_waits_500_ms_for_the_one_before() {
    // Bug: a pace of 250 ms, which the host closed with 1008 "Too many
    // requests" once pings and pongs were counted (OBSERVED.md, `### Pace at
    // 250 ms`). Gaps are read off the server's clock against the measured
    // 500 ms, not the constant.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .stale_after(Duration::from_secs(5))
        .streams([agg("AAAUSDT")])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    membership.subscribe([agg("BBBUSDT")]).await.unwrap();
    membership.unsubscribe([agg("AAAUSDT")]).await.unwrap();
    let received = server.received();
    assert_eq!(received.len(), 3);
    for pair in received.windows(2) {
        let gap = pair[1].at - pair[0].at;
        assert!(gap >= Duration::from_millis(490), "requests {gap:?} apart");
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_replay_is_one_request_of_every_name() {
    // Bug: replaying in batches of 200, which is five requests and two
    // seconds at spot's pace where one request carries them all.
    let server = ScriptedServer::start(vec![
        Script {
            drop_after: Some(Duration::from_millis(400)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(many(1000))
        .connect()
        .await
        .unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    let received = server.received();
    for connection in 0..2 {
        let sizes: Vec<usize> = received
            .iter()
            .filter(|r| r.connection == connection)
            .map(|r| r.request["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [1000], "connection {connection}");
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_closed_connection_is_reported_replaced_and_resubscribed() {
    // Bug: missing resubscribe after a reconnect.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MINI_TICKER.into()],
            close_after: true,
            close_code: Some((1008, "Too many requests".into())),
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MINI_TICKER.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mini()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let Event::Disconnected { path, reason } = next_event(&mut ws).await else {
        panic!("expected Disconnected");
    };
    assert_eq!(path, StreamPath::Stream);
    assert!(
        matches!(&reason, DisconnectReason::Closed { code: Some(1008), reason } if reason == "Too many requests"),
        "{reason:?}"
    );
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Stream
        }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let replayed = server
        .received()
        .into_iter()
        .find(|r| r.connection == 1)
        .unwrap();
    assert_eq!(replayed.request["params"][0], "btcusdt@miniTicker");
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_old_connection_is_rotated_without_backoff() {
    // Bug: running into Binance's 24-hour cutoff unannounced.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .backoff(Duration::from_secs(10), Duration::from_secs(10))
        .max_connection_age(Duration::from_millis(200))
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    let started = tokio::time::Instant::now();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Stream,
            reason: DisconnectReason::Rotation
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "rotation waited out the backoff: {:?}",
        started.elapsed()
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_refused_replay_stops_the_client_and_its_cause_is_kept() {
    // Bug: a stop that forgets why it stopped, so `close()` and the handle
    // answer a bare `Stopped` and a refusal reads as an invalid request.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MINI_TICKER),
        Script {
            refuse: vec![("btcusdt@miniTicker".into(), 2, "Invalid request".into())],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mini()]).connect().await.unwrap();
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Stream,
            ..
        }
    ));
    let refusal = next(&mut ws).await.expect_err("the refusal");
    assert!(
        matches!(refusal, SpotWsError::Refused { code: 2, .. }),
        "{refusal:?}"
    );
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");

    let stopped = ws.close().await.expect_err("a stop on an error");
    let SpotWsError::Stopped { cause: Some(cause) } = &stopped else {
        panic!("expected a stop with its cause, got {stopped:?}");
    };
    assert_eq!(cause.message, refusal.to_string());
    assert_eq!(
        cause.class,
        Class::VenueRefusal {
            code: Some("2".into())
        }
    );
    assert!(cause.is_fault);
    assert_eq!(stopped.class(), cause.class);

    let later = membership
        .subscribe([agg("ETHUSDT")])
        .await
        .expect_err("the client has stopped");
    assert!(
        matches!(&later, SpotWsError::Stopped { cause: Some(_) }),
        "{later:?}"
    );
    assert_eq!(later.class(), cause.class);
}

#[tokio::test]
async fn a_region_block_on_reconnect_stops_the_client_restricted_with_its_wait() {
    // A reconnect refused with 451 and a `Retry-After` stops the client.
    // `Stopped` carries the refusal's class, fault and wait, so a region block
    // is `Restricted`, not a fault, and tags environmental rather than real.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MINI_TICKER),
        refuse_upgrade(451, Some("7")),
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mini()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    let refusal = next(&mut ws).await.expect_err("the refusal");
    assert!(matches!(refusal, SpotWsError::Connect(_)), "{refusal:?}");
    assert_eq!(refusal.retry_after(), Some(Duration::from_secs(7)));
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");

    let stopped = ws.close().await.expect_err("a stop on an error");
    let SpotWsError::Stopped { cause: Some(cause) } = &stopped else {
        panic!("expected a stop with its cause, got {stopped:?}");
    };
    assert_eq!(cause.class, Class::Restricted);
    assert!(!cause.is_fault);
    assert_eq!(cause.retry_after, Some(Duration::from_secs(7)));
    assert_eq!(stopped.class(), Class::Restricted);
    assert!(!stopped.is_fault(), "{stopped:?}");
}

#[tokio::test]
async fn a_refused_handshake_s_retry_after_is_waited_out() {
    // A 10 ms step and a 400 ms wait: the attempt after the refusal waits the
    // 400 ms, where it retried inside the server's window.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MINI_TICKER),
        refuse_upgrade(503, Some("0.4")),
        steady(fixtures::MINI_TICKER),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(10), Duration::from_secs(5))
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Stream,
            ..
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Stream
        }
    ));
    let accepted = server.accepted_at();
    let waited = accepted[2] - accepted[1];
    assert!(waited >= Duration::from_millis(400), "{waited:?}");
    ws.close().await.unwrap();
}

/// The requests the server received whose method is `method`, in order.
fn requests_of(server: &ScriptedServer, method: &str) -> Vec<serde_json::Value> {
    server
        .requests()
        .into_iter()
        .filter(|request| request["method"] == method)
        .collect()
}

#[tokio::test]
async fn the_builders_socket_transport_dials_the_connection() {
    // `browser_like()` reports no client pings, so a connection it dialled
    // keeps alive with `LIST_SUBSCRIPTIONS` and sends no protocol ping (the
    // default transport's pings: `supervision_usdm_kit`'s `..._keeps_the_protocol_ping_d9`).
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_millis(1500))
        .socket_transport(polyoxide_ws::test_server::browser_like())
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    let quiet = tokio::time::timeout(Duration::from_millis(1200), ws.next()).await;
    assert!(quiet.is_err(), "the path went stale or ended: {quiet:?}");
    assert_eq!(server.connection_count(), 1);
    assert_eq!(server.ping_count(), 0, "a protocol ping reached the server");
    assert!(
        !requests_of(&server, "LIST_SUBSCRIPTIONS").is_empty(),
        "the injected transport did not dial the connection"
    );
    ws.close().await.unwrap();
}
