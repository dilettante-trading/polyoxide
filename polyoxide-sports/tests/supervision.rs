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
