//! Contract behaviours of the supervised tier that `supervision.rs` does not
//! pin. Each test here was written by Task 4's review to catch one change to
//! `supervised.rs` that passed every other test.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::Symbol,
    ws::{
        fixtures,
        test_server::{Script, ScriptedServer},
        DepthLevels, DepthSpeed, Event, StreamName, StreamPath, SupervisedUsdmWs, UsdmWsBuilder,
        UsdmWsError,
    },
};
use tokio::time::Instant;

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
fn fast(server: &ScriptedServer) -> UsdmWsBuilder {
    UsdmWsBuilder::new()
        .base_url(&server.url)
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
}
async fn next_event(ws: &mut SupervisedUsdmWs) -> Event {
    tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("an event within 3 s")
        .expect("open")
        .expect("not an error")
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_path_closed_for_want_of_streams_reopens_when_wanted_again() {
    let server = ScriptedServer::start(vec![
        Script::default(),
        Script::default(),
        Script {
            pushes: vec![fixtures::BOOK_TICKER.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server)
        .streams([btc_mark(), book()])
        .connect()
        .await
        .unwrap();
    ws.membership().unsubscribe([book()]).await.unwrap();
    server
        .wait_for("public to end", |s| s.ended_count() == 1)
        .await;
    ws.membership().subscribe([book()]).await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_path_whose_first_connect_drops_is_answered_ok_and_is_an_outage() {
    let server = ScriptedServer::start(vec![
        Script::default(),
        Script {
            reject_handshake: true,
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::BOOK_TICKER.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    ws.membership().subscribe([book()]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Public,
            ..
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Public
        }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_partial_unsubscribe_during_an_outage_is_not_replayed() {
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            reject_handshake: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(300), Duration::from_millis(300))
        .streams([btc_mark(), agg("ETHUSDT")])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    ws.membership().unsubscribe([agg("ETHUSDT")]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert_eq!(names_on(&server, 2), ["btcusdt@markPrice@1s"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_subscribe_the_connection_died_under_is_replayed() {
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
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert_eq!(
        names_on(&server, 1),
        ["btcusdt@markPrice@1s", "ethusdt@aggTrade"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unsubscribe_the_connection_died_under_is_not_replayed() {
    let server = ScriptedServer::start(vec![
        Script {
            ignore_later_requests: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .streams([btc_mark(), agg("ETHUSDT")])
        .connect()
        .await
        .unwrap();
    ws.membership().unsubscribe([agg("ETHUSDT")]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert_eq!(names_on(&server, 1), ["btcusdt@markPrice@1s"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_path_whose_first_connect_was_refused_can_be_wanted_again() {
    let server = ScriptedServer::start(vec![
        Script::default(),
        Script {
            refuse: vec![("btcusdt@bookTicker".into(), 2, "Invalid request".into())],
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    let err = ws.membership().subscribe([book()]).await.unwrap_err();
    assert!(matches!(err, UsdmWsError::Refused { .. }), "{err:?}");
    let depth = StreamName::PartialDepth(symbol("BTCUSDT"), DepthLevels::Five, DepthSpeed::Ms100);
    ws.membership().subscribe([depth]).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backoff_resets_after_a_connection_that_delivered() {
    let flap = Script {
        pushes: vec![fixtures::MARK_PRICE.into()],
        close_after: true,
        ..Default::default()
    };
    let reject = Script {
        reject_handshake: true,
        ..Default::default()
    };
    let server = ScriptedServer::start(vec![
        flap.clone(),
        reject.clone(),
        reject.clone(),
        reject,
        flap,
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(50), Duration::from_millis(800))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    // Update, Disconnected, Reconnected after three refused handshakes, then again.
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
        Event::Disconnected { .. }
    ));
    let down = Instant::now();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(
        down.elapsed() < Duration::from_millis(400),
        "backoff not reset: {:?}",
        down.elapsed()
    );
}
