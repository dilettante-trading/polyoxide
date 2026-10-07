//! Live tests against `fstream.binance.com`, `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --features ws --test live_ws -- --ignored
//! ```
//!
//! `live_frames_carry_no_unmodelled_keys` is the streams' drift detector.

mod common;

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::{Interval, Symbol},
    ws::{
        DepthLevels, DepthSpeed, Event, Payload, StreamName, StreamPath, Update, UsdmWs,
        UsdmWsBuilder,
    },
};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

fn every_kind() -> Vec<StreamName> {
    vec![
        StreamName::AllTickers,
        StreamName::AllMarkPrices,
        StreamName::AggTrade(btc()),
        StreamName::Kline(btc(), Interval::M1),
        StreamName::MarkPrice(btc()),
        StreamName::Ticker(btc()),
        StreamName::PartialDepth(btc(), DepthLevels::Twenty, DepthSpeed::Ms100),
        StreamName::BookTicker(btc()),
    ]
}

fn kind(payload: &Payload) -> &'static str {
    match payload {
        Payload::Tickers(_) => "tickers",
        Payload::MarkPrices(_) => "mark prices",
        Payload::AggTrade(_) => "aggTrade",
        Payload::Kline(_) => "kline",
        Payload::MarkPrice(_) => "mark price",
        Payload::Ticker(_) => "ticker",
        Payload::PartialDepth(_) => "partial depth",
        Payload::BookTicker(_) => "book ticker",
        _ => "unknown",
    }
}

#[tokio::test]
#[ignore]
async fn live_every_stream_kind_delivers_on_its_path() {
    let mut feed = UsdmWsBuilder::new()
        .streams(every_kind())
        .connect()
        .await
        .expect("connect");
    let mut seen = HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while seen.len() < 8 {
        let event = tokio::time::timeout_at(deadline, feed.next())
            .await
            .unwrap_or_else(|_| {
                panic!("saw only {seen:?} in 60 s; a quiet market can legitimately time out")
            })
            .expect("the stream is open")
            .expect("not an error");
        if let Event::Update(update) = event {
            assert!(
                !matches!(update.payload, Payload::Unknown { .. }),
                "{update:?}"
            );
            seen.insert(kind(&update.payload));
        }
    }
    feed.close().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn live_frames_carry_no_unmodelled_keys() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut unmodelled = Vec::new();
    for path in StreamPath::ALL {
        let names: Vec<String> = every_kind()
            .into_iter()
            .filter(|s| s.path() == *path)
            .map(|s| s.to_string())
            .collect();
        let url = format!(
            "wss://fstream.binance.com/{}/stream?streams={}",
            path.as_str(),
            names.join("/")
        );
        let (mut socket, _) = connect_async(url).await.expect("connect");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while let Ok(Some(message)) = tokio::time::timeout_at(deadline, socket.next()).await {
            let Message::Text(text) = message.expect("frame") else {
                continue;
            };
            let update = Update::from_json(&text).unwrap_or_else(|e| panic!("{e}"));
            assert!(!matches!(update.payload, Payload::Unknown { .. }), "{text}");
            let wire: Value = serde_json::from_str(&text).unwrap();
            let diff = common::compare_values(
                &update.stream.to_string(),
                &wire,
                &serde_json::to_value(&update).unwrap(),
            );
            for key in diff.unmodelled {
                // Binance documents the kline's `B` as "Ignore".
                if key != "/data/k/B" {
                    unmodelled.push(format!("{}: {key}", update.stream));
                }
            }
        }
    }
    unmodelled.sort();
    unmodelled.dedup();
    assert!(
        unmodelled.is_empty(),
        "the host sent keys the stream types do not model; add them and record them in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}

#[tokio::test]
#[ignore]
async fn live_a_quiet_supervised_connection_stays_up() {
    // A listed symbol with no trades: nothing arrives but pings and pongs.
    let quiet = StreamName::AggTrade(Symbol::new("ZZ0000USDT").unwrap());
    let mut feed = UsdmWsBuilder::new()
        .ping_interval(Duration::from_secs(2))
        .stale_after(Duration::from_secs(5))
        .streams([quiet])
        .connect()
        .await
        .expect("connect");
    let held = tokio::time::timeout(Duration::from_secs(12), feed.next()).await;
    assert!(held.is_err(), "expected silence, got {held:?}");
    feed.close().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn live_a_client_ping_is_answered() {
    let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::MarkPrice(btc())])
        .await
        .expect("connect");
    let rtt = ws.ping().await.expect("pong");
    assert!(rtt < Duration::from_secs(5), "{rtt:?}");
    assert_eq!(
        ws.list_subscriptions().await.expect("list"),
        ["btcusdt@markPrice@1s"]
    );
    ws.close().await.unwrap();
}
