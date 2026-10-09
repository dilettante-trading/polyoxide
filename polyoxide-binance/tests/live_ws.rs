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
//! Each failure prints the tag the nightly classifier reads, taken from its
//! error's class: a connect timeout, a 5xx or a reset is transient, a 451
//! environmental, and a 418, a 403 or a malformed frame real. A stream that
//! ends without saying why is transient. A kind that delivered nothing is
//! real, since eight BTCUSDT streams are never all quiet for a minute, and so
//! is an unanswered ping, which is what `live_a_client_ping_is_answered` tests.

use polyoxide_test_support::agreement as common;

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::{Interval, Symbol},
    ws::{
        client::CONNECT_TIMEOUT, DepthLevels, DepthSpeed, Event, Payload, StreamName, StreamPath,
        SymbolType, Update, UsdmWs, UsdmWsBuilder, UsdmWsError, USDM_WS_BASE,
    },
};
use polyoxide_test_support::{fail, transient, ResultExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").or_fail("BTCUSDT")
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
        .or_fail("connect");
    let mut seen = HashSet::new();
    let mut last_outage = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while seen.len() < KINDS.len() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            // Eight BTCUSDT streams are never all quiet for a minute: a kind
            // missing here stopped delivering, or its path never came back.
            let missing: Vec<&str> = KINDS.into_iter().filter(|k| !seen.contains(k)).collect();
            let finding = format!(
                "in 60 s these kinds delivered nothing: {missing:?}; last outage: {last_outage:?}"
            );
            panic!("{finding}"); // live-unwraps: a kind that delivered nothing is the fault under test
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the connection"))
            .or_fail("the supervised feed");
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
    feed.close().await.or_fail("close");
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
        let connect = format!("{path}: connect");
        let Ok(connected) = tokio::time::timeout(CONNECT_TIMEOUT, connect_async(url)).await else {
            fail(&connect, &UsdmWsError::ConnectTimeout(CONNECT_TIMEOUT));
        };
        let (mut socket, _) = connected.map_err(UsdmWsError::from).or_fail(&connect);
        let mut checked = HashSet::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while let Ok(next) = tokio::time::timeout_at(deadline, socket.next()).await {
            let Some(message) = next else {
                transient(&format!("{path}: the server ended the connection"));
            };
            let message = message
                .map_err(UsdmWsError::from)
                .or_fail(&format!("{path}: read"));
            let Message::Text(text) = message else {
                continue;
            };
            let update = Update::from_json(&text).or_fail(&text);
            assert!(!matches!(update.payload, Payload::Unknown { .. }), "{text}");
            let stream = update.stream.to_string();
            checked.insert(stream.clone());
            let wire: Value = serde_json::from_str(&text).expect("a JSON frame"); // live-unwraps: the frame decoded above
            let emitted = serde_json::to_value(&update).expect("an update serialises"); // live-unwraps: serialising test data
            let diff = common::compare_values(&stream, &wire, &emitted);
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
                    let levels = wire["data"][side].as_array().expect("a book side"); // live-unwraps: an assertion on the frame
                    for level in levels {
                        let n = level.as_array().expect("a book level").len(); // live-unwraps: an assertion on the frame
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
    let quiet = StreamName::AggTrade(Symbol::new("ZZ0000USDT").or_fail("ZZ0000USDT"));
    let mut feed = UsdmWsBuilder::new()
        .ping_interval(Duration::from_secs(2))
        .stale_after(Duration::from_secs(8))
        .streams([quiet])
        .connect()
        .await
        .or_fail("connect");
    let held = tokio::time::timeout(Duration::from_secs(12), feed.next()).await;
    assert!(held.is_err(), "expected silence, got {held:?}");
    feed.close().await.or_fail("close");
}

#[tokio::test]
#[ignore]
async fn live_a_client_ping_is_answered() {
    let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::MarkPrice(btc())])
        .await
        .or_fail("connect");
    // An unanswered ping is the fault under test, so it files whatever its
    // class; any other error, a server restart among them, fails by its class.
    let rtt = match ws.ping().await {
        Ok(rtt) => rtt,
        Err(err @ UsdmWsError::NoAnswer { .. }) => panic!("pong: {err:?}"), // live-unwraps: the property under test
        Err(err) => fail("ping", &err),
    };
    assert!(rtt < Duration::from_secs(5), "{rtt:?}");
    assert_eq!(
        ws.list_subscriptions().await.or_fail("list"),
        ["btcusdt@markPrice@1s"]
    );
    ws.close().await.or_fail("close");
}
