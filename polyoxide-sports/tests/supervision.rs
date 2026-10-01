//! The supervised tier against the scripted server.
//!
//! Staleness limits and backoff delays are tens to hundreds of milliseconds
//! so the suite runs fast. The windows are generous, to stay steady on a
//! loaded machine.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_sports::{
    fixtures,
    test_server::{Script, ScriptedServer},
    Event, SportsError, SportsWsBuilder, SupervisedSportsWs,
};
use tokio::time::{timeout, Instant};
use tokio_tungstenite::tungstenite::Message;

const STALE: Duration = Duration::from_millis(300);
const WINDOW: Duration = Duration::from_secs(3);

/// A builder pointed at `server`, with test-sized timings.
fn builder(server: &ScriptedServer) -> SportsWsBuilder {
    SportsWsBuilder::new()
        .url(server.url.clone())
        .stale_after(STALE)
        .backoff(Duration::from_millis(20), Duration::from_millis(200))
        .connect_timeout(Duration::from_secs(2))
}

/// The next item, which must arrive within the window.
async fn next_item(feed: &mut SupervisedSportsWs) -> Result<Event, SportsError> {
    timeout(WINDOW, feed.next())
        .await
        .expect("an event within the window")
        .expect("a supervised feed never ends")
}

/// Read the feed until the task is aborted, so the server sees each reconnect.
async fn drain(mut feed: SupervisedSportsWs) {
    while feed.next().await.is_some() {}
}

/// The time between consecutive accepted connections.
fn gaps(accepted: &[Instant]) -> Vec<Duration> {
    accepted.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

/// A short label per item, for asserting order.
fn label(item: Result<Event, SportsError>) -> String {
    match item {
        Ok(Event::Update(update)) => update.league_abbreviation.clone(),
        Ok(Event::Disconnected { .. }) => "disconnected".into(),
        Ok(Event::Reconnected) => "reconnected".into(),
        Ok(other) => format!("unexpected {other:?}"),
        Err(error) => format!("error: {error}"),
    }
}

fn text(frame: &str) -> Message {
    Message::Text(frame.into())
}

#[tokio::test]
async fn protocol_pings_alone_keep_a_quiet_connection_alive() {
    // In a quiet hour the server sends no data, only pings. If staleness
    // counted only data, a healthy connection would drop every stale period.
    let server = ScriptedServer::start(vec![Script::pings_only(Duration::from_millis(100))]).await;
    let mut feed = builder(&server).connect().await.unwrap();
    if let Ok(item) = timeout(STALE * 4, feed.next()).await {
        panic!("expected silence while pings flowed, got {item:?}");
    }
    assert_eq!(
        server.connection_count(),
        1,
        "the feed reconnected while pings were flowing"
    );
    assert!(!server.pongs().is_empty(), "the feed never answered a ping");
}

#[tokio::test]
async fn silence_is_stale_and_the_feed_reconnects() {
    let server = ScriptedServer::start(vec![
        Script::silent(),
        Script::pings_only(Duration::from_millis(100)),
    ])
    .await;
    let started = Instant::now();
    let mut feed = builder(&server).connect().await.unwrap();
    match next_item(&mut feed).await {
        Ok(Event::Disconnected {
            reason: SportsError::Stale { after },
        }) => assert_eq!(after, STALE),
        other => panic!("expected a stale disconnect, got {other:?}"),
    }
    assert!(started.elapsed() >= STALE, "declared stale early");
    assert!(matches!(next_item(&mut feed).await, Ok(Event::Reconnected)));
    assert_eq!(server.connection_count(), 2);
}

#[tokio::test]
async fn the_supervised_feed_answers_pings() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![Message::Ping(b"p1".to_vec().into())],
        ..Script::silent()
    }])
    .await;
    let feed = SportsWsBuilder::new()
        .url(server.url.clone())
        .connect()
        .await
        .unwrap();
    let reader = tokio::spawn(drain(feed));
    server
        .wait_for("a pong", |s| s.pongs() == [b"p1".to_vec()])
        .await;
    reader.abort();
}

#[tokio::test]
async fn a_closed_connection_yields_its_updates_then_one_marker_pair() {
    let server = ScriptedServer::start(vec![
        Script::frames(&[fixtures::SOCCER, fixtures::ESPORTS]).then_close(),
        Script::frames(&[fixtures::TENNIS_EVENT_STATE]),
    ])
    .await;
    let mut feed = builder(&server).connect().await.unwrap();
    let mut labels = Vec::new();
    for _ in 0..5 {
        labels.push(label(next_item(&mut feed).await));
    }
    assert_eq!(labels, ["kor", "lol", "disconnected", "reconnected", "atp"]);
    server
        .wait_for("the client's close reply", |s| s.close_reply_count() == 1)
        .await;
}

#[tokio::test]
async fn an_outage_yields_one_marker_pair_however_many_attempts_fail() {
    let server = ScriptedServer::start(vec![
        Script::close_at_once(),
        Script::reject(),
        Script::reject(),
        Script::reject(),
        Script::frames(&[fixtures::SOCCER]),
    ])
    .await;
    let mut feed = builder(&server)
        .stale_after(Duration::from_secs(10))
        .connect()
        .await
        .unwrap();
    let mut labels = Vec::new();
    for _ in 0..3 {
        labels.push(label(next_item(&mut feed).await));
    }
    assert_eq!(labels, ["disconnected", "reconnected", "kor"]);
    assert_eq!(
        server.connection_count(),
        5,
        "expected three refused attempts between the two good connections"
    );
}

#[tokio::test]
async fn backoff_grows_while_connections_receive_nothing() {
    // Every connection is accepted and closed at once. Nothing is received,
    // so the delay must keep doubling instead of hammering the server.
    let server = ScriptedServer::start(vec![Script::close_at_once()]).await;
    let feed = SportsWsBuilder::new()
        .url(server.url.clone())
        .backoff(Duration::from_millis(40), Duration::from_secs(5))
        .connect()
        .await
        .unwrap();
    let reader = tokio::spawn(drain(feed));
    server
        .wait_for("six connections", |s| s.connection_count() >= 6)
        .await;
    reader.abort();
    let gaps = gaps(&server.accepted_at()[..6]);
    for pair in gaps.windows(2) {
        assert!(pair[1] >= pair[0] * 3 / 2, "backoff did not grow: {gaps:?}");
    }
}

#[tokio::test]
async fn backoff_resets_after_a_connection_that_received_something() {
    let server =
        ScriptedServer::start(vec![Script::frames(&[fixtures::SOCCER]).then_close()]).await;
    let feed = SportsWsBuilder::new()
        .url(server.url.clone())
        .backoff(Duration::from_millis(40), Duration::from_secs(5))
        .connect()
        .await
        .unwrap();
    let reader = tokio::spawn(drain(feed));
    server
        .wait_for("six connections", |s| s.connection_count() >= 6)
        .await;
    reader.abort();
    let gaps = gaps(&server.accepted_at()[..6]);
    assert!(
        gaps.iter().all(|gap| *gap < Duration::from_millis(120)),
        "backoff grew although every connection delivered a frame: {gaps:?}"
    );
}

#[tokio::test]
async fn a_bad_frame_is_reported_and_the_connection_survives() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![text("not json"), text(fixtures::SOCCER)],
        ..Script::silent()
    }])
    .await;
    let mut feed = builder(&server)
        .stale_after(Duration::from_secs(10))
        .connect()
        .await
        .unwrap();
    match next_item(&mut feed).await {
        Err(SportsError::Decode { raw, .. }) => assert_eq!(raw, "not json"),
        other => panic!("expected a decode error, got {other:?}"),
    }
    assert_eq!(label(next_item(&mut feed).await), "kor");
    assert_eq!(
        server.connection_count(),
        1,
        "a bad frame cost the connection"
    );
}

#[tokio::test]
async fn dropping_the_feed_closes_the_socket() {
    let server = ScriptedServer::start(vec![Script::silent()]).await;
    let feed = builder(&server)
        .stale_after(Duration::from_secs(10))
        .connect()
        .await
        .unwrap();
    server
        .wait_for("the handshake", |s| s.handshake_count() == 1)
        .await;
    drop(feed);
    server
        .wait_for("the client to end the connection", |s| {
            s.client_ended_count() == 1
        })
        .await;
}

#[tokio::test]
async fn the_first_connection_failure_is_returned() {
    let server = ScriptedServer::start(vec![Script::reject()]).await;
    match builder(&server).connect().await {
        Err(SportsError::Connect { .. }) => {}
        Err(other) => panic!("expected a connect error, got {other}"),
        Ok(_) => panic!("connected to a server that drops every handshake"),
    }
}

#[tokio::test]
async fn a_handshake_that_never_finishes_times_out() {
    let server = ScriptedServer::start(vec![Script::stall()]).await;
    let limit = Duration::from_millis(200);
    match builder(&server).connect_timeout(limit).connect().await {
        Err(SportsError::ConnectTimeout { after }) => assert_eq!(after, limit),
        Err(other) => panic!("expected a timeout, got {other}"),
        Ok(_) => panic!("connected to a server that never answers"),
    }
}
