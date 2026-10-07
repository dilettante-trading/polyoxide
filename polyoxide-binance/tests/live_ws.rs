//! Live tests against `fstream.binance.com`, `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --features ws --test live_ws -- --ignored
//! ```
//!
//! `live_frames_carry_no_unmodelled_keys` is the streams' drift detector. It
//! sees keys, the length of a book level and a symbol type this version does
//! not know, and fails when a stream sends nothing to check. It does not see an
//! array row whose `e` changed, nor a new value of any other enum, which
//! decodes as `Other`.
//!
//! A failure's panic text decides how the nightly files it, so each prints its
//! error's Display and Debug: a connect timeout, a 5xx or a reset reads as
//! transient, a 451 as environmental, and a kind that delivered nothing as
//! real, since eight BTCUSDT streams are never all quiet for a minute.

mod common;

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::{Interval, Symbol},
    ws::{
        client::CONNECT_TIMEOUT, DepthLevels, DepthSpeed, Event, Payload, StreamName, StreamPath,
        SymbolType, Update, UsdmWs, UsdmWsBuilder, USDM_WS_BASE,
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

/// A book level is `[price, quantity]`.
const LEVEL_VALUES: usize = 2;

const KINDS: [&str; 8] = [
    "tickers",
    "mark prices",
    "aggTrade",
    "kline",
    "mark price",
    "ticker",
    "partial depth",
    "book ticker",
];

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

fn symbol_types(payload: &Payload) -> Vec<SymbolType> {
    match payload {
        Payload::Tickers(rows) => rows.iter().map(|row| row.symbol_type).collect(),
        Payload::MarkPrices(rows) => rows.iter().map(|row| row.symbol_type).collect(),
        Payload::AggTrade(event) => vec![event.symbol_type],
        Payload::MarkPrice(event) => vec![event.symbol_type],
        Payload::Ticker(event) => vec![event.symbol_type],
        Payload::PartialDepth(event) => vec![event.symbol_type],
        Payload::BookTicker(event) => vec![event.symbol_type],
        _ => Vec::new(),
    }
}

#[tokio::test]
#[ignore]
async fn live_every_stream_kind_delivers_on_its_path() {
    let mut feed = UsdmWsBuilder::new()
        .streams(every_kind())
        .connect()
        .await
        .unwrap_or_else(|e| panic!("connect: {e} ({e:?})"));
    let mut seen = HashSet::new();
    let mut last_outage = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while seen.len() < KINDS.len() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            // Eight BTCUSDT streams are never all quiet for a minute: a kind
            // missing here stopped delivering, or its path never came back.
            let missing: Vec<&str> = KINDS.into_iter().filter(|k| !seen.contains(k)).collect();
            panic!(
                "in 60 s these kinds delivered nothing: {missing:?}; last outage: {last_outage:?}"
            );
        };
        let event = next
            .expect("the server ended the connection")
            .unwrap_or_else(|e| panic!("{e} ({e:?})"));
        match event {
            Event::Update(update) => {
                assert!(
                    !matches!(update.payload, Payload::Unknown { .. }),
                    "{update:?}"
                );
                seen.insert(kind(&update.payload));
            }
            Event::Disconnected { path, reason } => {
                last_outage = Some(format!("{path}: {reason:?}"));
            }
            _ => {}
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
            "{USDM_WS_BASE}/{}/stream?streams={}",
            path.as_str(),
            names.join("/")
        );
        let Ok(connected) = tokio::time::timeout(CONNECT_TIMEOUT, connect_async(url)).await else {
            panic!("{path}: no connection within {CONNECT_TIMEOUT:?}");
        };
        let (mut socket, _) = connected.unwrap_or_else(|e| panic!("{path}: connect: {e} ({e:?})"));
        let mut checked = HashSet::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while let Ok(next) = tokio::time::timeout_at(deadline, socket.next()).await {
            let Some(message) = next else {
                panic!("{path}: the server ended the connection");
            };
            let message = message.unwrap_or_else(|e| panic!("{path}: {e} ({e:?})"));
            let Message::Text(text) = message else {
                continue;
            };
            let update = Update::from_json(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
            assert!(!matches!(update.payload, Payload::Unknown { .. }), "{text}");
            let stream = update.stream.to_string();
            checked.insert(stream.clone());
            let wire: Value = serde_json::from_str(&text).unwrap();
            let diff =
                common::compare_values(&stream, &wire, &serde_json::to_value(&update).unwrap());
            for key in diff.unmodelled {
                // Binance documents the kline's `B` as "Ignore".
                if key != "/data/k/B" {
                    unmodelled.push(format!("{stream}: {key}"));
                }
            }
            // A book level has no keys, so a value appended to one is seen
            // only by its length.
            if matches!(update.payload, Payload::PartialDepth(_)) {
                for side in ["b", "a"] {
                    for level in wire["data"][side].as_array().expect("a book side") {
                        let n = level.as_array().expect("a book level").len();
                        if n != LEVEL_VALUES {
                            unmodelled.push(format!(
                                "{stream}: a {side} level of {n} values, not {LEVEL_VALUES}"
                            ));
                        }
                    }
                }
            }
            // A symbol type this version does not know decodes and
            // re-serialises unchanged, so the key comparison cannot see it.
            for st in symbol_types(&update.payload) {
                if matches!(st, SymbolType::Other(_)) {
                    unmodelled.push(format!("{stream}: symbol type {st:?}"));
                }
            }
        }
        let missing: Vec<&String> = names.iter().filter(|n| !checked.contains(*n)).collect();
        assert!(
            missing.is_empty(),
            "{path}: in 10 s these streams sent no frame to check: {missing:?}"
        );
    }
    unmodelled.sort();
    unmodelled.dedup();
    assert!(
        unmodelled.is_empty(),
        "the host sent what the stream types do not model; model it and record it in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}

#[tokio::test]
#[ignore]
async fn live_a_quiet_supervised_connection_stays_up() {
    // An unlisted symbol: Binance acknowledges the subscription, and nothing
    // arrives but pings and pongs.
    let quiet = StreamName::AggTrade(Symbol::new("ZZ0000USDT").unwrap());
    let mut feed = UsdmWsBuilder::new()
        .ping_interval(Duration::from_secs(2))
        .stale_after(Duration::from_secs(8))
        .streams([quiet])
        .connect()
        .await
        .unwrap_or_else(|e| panic!("connect: {e} ({e:?})"));
    let held = tokio::time::timeout(Duration::from_secs(12), feed.next()).await;
    assert!(held.is_err(), "expected silence, got {held:?}");
    feed.close().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn live_a_client_ping_is_answered() {
    let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::MarkPrice(btc())])
        .await
        .unwrap_or_else(|e| panic!("connect: {e} ({e:?})"));
    let rtt = ws.ping().await.expect("pong");
    assert!(rtt < Duration::from_secs(5), "{rtt:?}");
    assert_eq!(
        ws.list_subscriptions().await.expect("list"),
        ["btcusdt@markPrice@1s"]
    );
    ws.close().await.unwrap();
}
