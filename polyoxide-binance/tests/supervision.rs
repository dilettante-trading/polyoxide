//! The supervised tier against the scripted server, with limits in hundreds of
//! milliseconds. Each test names the bug it exists to catch.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::Symbol,
    ws::{
        fixtures,
        test_server::{Script, ScriptedServer},
        DepthLevels, DepthSpeed, DisconnectReason, Event, StreamName, StreamPath, SupervisedUsdmWs,
        UsdmWsBuilder, UsdmWsError, MIN_REQUEST_INTERVAL,
    },
};

fn symbol(s: &str) -> Symbol {
    Symbol::new(s).unwrap()
}

fn agg(s: &str) -> StreamName {
    StreamName::AggTrade(symbol(s))
}

fn btc_mark() -> StreamName {
    StreamName::MarkPrice(symbol("BTCUSDT"))
}

fn fast(server: &ScriptedServer) -> UsdmWsBuilder {
    UsdmWsBuilder::new()
        .base_url(&server.url)
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
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

/// Asserts nothing arrives for `quiet`.
async fn assert_quiet(ws: &mut SupervisedUsdmWs, quiet: Duration) {
    if let Ok(item) = tokio::time::timeout(quiet, ws.next()).await {
        panic!("expected silence, got {item:?}");
    }
}

#[tokio::test]
async fn each_path_gets_its_own_connection() {
    // Bug: one connection for both paths, which drops one path's data silently.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let depth = StreamName::PartialDepth(symbol("BTCUSDT"), DepthLevels::Twenty, DepthSpeed::Ms100);
    let ws = fast(&server)
        .streams([agg("BTCUSDT"), depth.clone()])
        .connect()
        .await
        .unwrap();
    let mut paths = server.paths();
    paths.sort();
    assert_eq!(paths, ["/market/stream", "/public/stream"]);
    for received in server.received() {
        let names = received.request["params"].as_array().unwrap().clone();
        let path = &server.paths()[received.connection];
        let expected = if path == "/market/stream" {
            "btcusdt@aggTrade".to_owned()
        } else {
            depth.to_string()
        };
        assert_eq!(names, [serde_json::Value::from(expected)], "{path}");
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_1000_stream_replay_is_paced_in_batches_of_200() {
    // Bugs: bursting a resubscribe past Binance's message limit, which closes
    // the connection; an oversized request.
    let server = ScriptedServer::start(vec![
        Script {
            drop_after: Some(Duration::from_millis(1500)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let streams: Vec<StreamName> = (0..1000).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
    let mut ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(streams)
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
        let requests: Vec<_> = received
            .iter()
            .filter(|r| r.connection == connection)
            .collect();
        let sizes: Vec<usize> = requests
            .iter()
            .map(|r| r.request["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [200; 5], "connection {connection}");
        for pair in requests.windows(2) {
            let gap = pair[1].at - pair[0].at;
            assert!(
                gap >= MIN_REQUEST_INTERVAL - Duration::from_millis(10),
                "connection {connection}: requests {gap:?} apart"
            );
        }
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn the_1025th_stream_is_refused_and_nothing_reaches_the_server() {
    // Bug: sending it, which makes the server close the connection and lose
    // all 1024.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let streams: Vec<StreamName> = (0..1024).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
    let ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(streams)
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
            UsdmWsError::TooManyStreams {
                path: StreamPath::Market,
                limit: 1024
            }
        ),
        "{err:?}"
    );
    assert_eq!(server.received().len(), sent);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_silent_server_is_stale_then_replaced() {
    // Bug: staleness counting only data, or not enforced at all.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            go_silent: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
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
    assert_eq!(server.connection_count(), 2);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn server_pings_alone_keep_a_quiet_connection() {
    // Bug: dropping a healthy connection whose streams are quiet. The client
    // never pings here, so only the server's pings show it is alive.
    let server = ScriptedServer::start(vec![Script {
        ping_every: Some(Duration::from_millis(50)),
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server)
        .ping_interval(Duration::MAX)
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert_quiet(&mut ws, Duration::from_millis(900)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn answered_client_pings_keep_a_quiet_connection() {
    // The same, kept alive by pongs to the client's own pings.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert_quiet(&mut ws, Duration::from_millis(900)).await;
    assert_eq!(server.connection_count(), 1);
    assert!(server.ping_count() >= 10, "pings: {}", server.ping_count());
    ws.close().await.unwrap();
}

#[tokio::test]
async fn pings_keep_going_under_steady_traffic() {
    // Bug: a ping sent only when the connection is quiet, which a busy
    // connection never is.
    let server = ScriptedServer::start(vec![Script {
        pushes: vec![fixtures::MARK_PRICE.into()],
        push_every: Some(Duration::from_millis(10)),
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    let mut updates = 0;
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, ws.next()).await {
        assert!(matches!(event.unwrap(), Event::Update(_)));
        updates += 1;
    }
    assert!(updates >= 20, "traffic stopped: {updates} updates");
    assert!(
        server.ping_count() >= 5,
        "pings under traffic: {}",
        server.ping_count()
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_closed_connection_is_reported_replaced_and_resubscribed() {
    // Bug: missing resubscribe after a reconnect.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            close_code: Some((1008, "Too many requests".into())),
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let Event::Disconnected { path, reason } = next_event(&mut ws).await else {
        panic!("expected Disconnected");
    };
    assert_eq!(path, StreamPath::Market);
    assert!(
        matches!(&reason, DisconnectReason::Closed { code: Some(1008), reason } if reason == "Too many requests"),
        "{reason:?}"
    );
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let replayed = server
        .received()
        .into_iter()
        .find(|r| r.connection == 1)
        .unwrap();
    assert_eq!(replayed.request["params"][0], "btcusdt@markPrice@1s");
    ws.close().await.unwrap();
}

#[tokio::test]
async fn prader_s_scenario_update_disconnected_reconnected_update() {
    // The consumer's own test, as it will run against this server.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = UsdmWsBuilder::new()
        .base_url(&server.url)
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
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
    assert_eq!(server.connection_count(), 2);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn failed_reconnects_make_one_outage_not_one_per_attempt() {
    // Bug: a Disconnected per attempt, which a consumer would count as many
    // outages.
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
        Script {
            reject_handshake: true,
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            ..Default::default()
        },
    ])
    .await;
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
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert_eq!(server.connection_count(), 4);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_old_connection_is_rotated_without_backoff() {
    // Bug: running into Binance's 24-hour cutoff unannounced.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .backoff(Duration::from_secs(10), Duration::from_secs(10))
        .max_connection_age(Duration::from_millis(200))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let started = tokio::time::Instant::now();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            reason: DisconnectReason::Rotation,
            ..
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
async fn an_error_answer_fails_only_the_call_that_sent_it() {
    // Bug: one refused request tearing down the path.
    let server = ScriptedServer::start(vec![Script {
        refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server)
        .streams([agg("BTCUSDT")])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    let err = membership.subscribe([agg("ETHUSDT")]).await.unwrap_err();
    assert!(
        matches!(err, UsdmWsError::Refused { code: 2, .. }),
        "{err:?}"
    );
    membership.subscribe([agg("SOLUSDT")]).await.unwrap();
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_bad_frame_is_an_error_and_the_next_update_still_arrives() {
    // Bug: one undecodable frame ending the stream.
    let server = ScriptedServer::start(vec![Script {
        pushes: vec![
            r#"{"stream":"btcusdt@markPrice@1s","data":{"e":"markPriceUpdate"}}"#.into(),
            fixtures::MARK_PRICE.into(),
        ],
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(
        next(&mut ws).await,
        Err(UsdmWsError::Frame { .. })
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_path_closes_when_its_last_stream_leaves_and_says_nothing() {
    // Bug: a leaked connection; and a path that closes while up must yield
    // neither marker.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let book = StreamName::BookTicker(symbol("BTCUSDT"));
    let mut ws = fast(&server)
        .streams([btc_mark(), book.clone()])
        .connect()
        .await
        .unwrap();
    ws.membership().unsubscribe([book]).await.unwrap();
    server
        .wait_for("the public connection to end", |s| s.ended_count() == 1)
        .await;
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 2);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_path_wanted_again_reopens() {
    let server = ScriptedServer::start(vec![Script {
        pushes: vec![fixtures::BOOK_TICKER.into()],
        ..Default::default()
    }])
    .await;
    let book = StreamName::BookTicker(symbol("BTCUSDT"));
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    ws.membership().subscribe([book]).await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let mut paths = server.paths();
    paths.sort();
    assert_eq!(paths, ["/market/stream", "/public/stream"]);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_outage_ends_with_reconnected_even_when_its_last_stream_leaves() {
    // Bug: a Disconnected with no Reconnected, which leaves a consumer that
    // folds outages into one state stale for good.
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
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(100), Duration::from_millis(100))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    ws.membership().unsubscribe([btc_mark()]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Market
        }
    ));
    let attempts = server.connection_count();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        server.connection_count(),
        attempts,
        "kept reconnecting a path nobody wants"
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_change_during_an_outage_is_answered_at_once_and_replayed() {
    // Bug: a membership call failing, or hanging, while its path is down.
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
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    let started = tokio::time::Instant::now();
    ws.membership().subscribe([agg("ETHUSDT")]).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    let replayed: Vec<String> = server
        .received()
        .into_iter()
        .filter(|r| r.connection == 2)
        .flat_map(|r| r.request["params"].as_array().unwrap().clone())
        .map(|name| name.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(replayed, ["btcusdt@markPrice@1s", "ethusdt@aggTrade"]);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_refused_replay_is_an_error_and_ends_the_stream() {
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            refuse: vec![("btcusdt@markPrice@1s".into(), 2, "Invalid request".into())],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next(&mut ws).await,
        Err(UsdmWsError::Refused { code: 2, .. })
    ));
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");
}

#[tokio::test]
async fn dropping_the_stream_closes_every_socket() {
    // Bug: a leaked task holding connections open.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .streams([btc_mark(), StreamName::BookTicker(symbol("BTCUSDT"))])
        .connect()
        .await
        .unwrap();
    drop(ws);
    server
        .wait_for("both connections to end", |s| s.ended_count() == 2)
        .await;
}

#[tokio::test]
async fn a_refused_first_connect_is_an_error_from_connect() {
    // Bug: retrying silently when the setup is wrong.
    let server = ScriptedServer::start(vec![Script {
        reject_handshake: true,
        ..Default::default()
    }])
    .await;
    let err = fast(&server)
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap_err();
    assert!(matches!(err, UsdmWsError::Connect(_)), "{err:?}");
}

#[tokio::test]
async fn close_ends_the_stream_and_the_handle_promptly_even_mid_backoff() {
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
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_secs(5), Duration::from_secs(5))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    let started = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(2), ws.close())
        .await
        .expect("close returns")
        .unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(
        membership.subscribe([agg("ETHUSDT")]).await,
        Err(UsdmWsError::Stopped)
    ));
}
