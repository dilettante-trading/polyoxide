//! Live tests against `stream.binance.com`, `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --features spot-ws --test live_spot_ws -- --ignored
//! ```
//!
//! `live_frames_carry_no_unmodelled_keys` is the streams' drift detector. It
//! sees keys and the length of a book level this version does not model, and
//! fails when a stream sends nothing to check. It does not see an array row
//! whose `e` changed, nor a new value of any other enum, which decodes as
//! `Other`. Binance documents `M` on trades and aggregate trades and `B` on the
//! kline as "Ignore", and the types do not model them.
//!
//! Each failure prints the tag the nightly classifier reads, taken from its
//! error's class: a connect timeout, a 5xx or a reset is transient, a 451
//! environmental, and a 418, a 403 or a malformed frame real. A stream that
//! ends without saying why is transient. A kind that delivered nothing is
//! real, since ten BTCUSDT streams are never all quiet for a minute, and so
//! is an unanswered ping, which is what `live_a_client_ping_is_answered` tests.
//!
//! The address is shared with other sessions and the host closes a connection
//! that sends more than 5 messages a second, so nothing here sends faster than
//! the client paces: one `SUBSCRIBE` for the supervised feed, none for the
//! drift check, and requests at least 500 ms apart for the bare tier.

use polyoxide_test_support::agreement as common;

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_binance::spot::{
    types::{Interval, Symbol},
    ws::{
        client::CONNECT_TIMEOUT, DepthLevels, DepthSpeed, Event, Payload, SpotWs, SpotWsBuilder,
        SpotWsError, StreamName, StreamPath, Update, SPOT_WS_BASE,
    },
};
use polyoxide_test_support::{fail, transient, ResultExt};
use polyoxide_ws::{tungstenite::Message, WsError};
use serde_json::Value;

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").or_fail("BTCUSDT")
}

fn every_kind() -> Vec<StreamName> {
    vec![
        StreamName::AggTrade(btc()),
        StreamName::Trade(btc()),
        StreamName::Kline(btc(), Interval::Minute1),
        StreamName::MiniTicker(btc()),
        StreamName::AllMiniTickers,
        StreamName::Ticker(btc()),
        StreamName::BookTicker(btc()),
        StreamName::PartialDepth(btc(), DepthLevels::Twenty, DepthSpeed::Ms100),
        StreamName::PartialDepth(btc(), DepthLevels::Five, DepthSpeed::Ms1000),
        StreamName::AvgPrice(btc()),
    ]
}

/// A book level is `[price, quantity]`.
const LEVEL_VALUES: usize = 2;

const KINDS: [&str; 10] = [
    "aggTrade",
    "trade",
    "kline",
    "miniTicker",
    "all miniTickers",
    "ticker",
    "book ticker",
    "partial depth",
    "partial depth, bare",
    "avgPrice",
];

fn kind(update: &Update) -> &'static str {
    match (
        &update.payload,
        update.stream.to_string().ends_with("@100ms"),
    ) {
        (Payload::AggTrade(_), _) => "aggTrade",
        (Payload::Trade(_), _) => "trade",
        (Payload::Kline(_), _) => "kline",
        (Payload::MiniTicker(_), _) => "miniTicker",
        (Payload::MiniTickers(_), _) => "all miniTickers",
        (Payload::Ticker(_), _) => "ticker",
        (Payload::BookTicker(_), _) => "book ticker",
        (Payload::PartialDepth(_), true) => "partial depth",
        (Payload::PartialDepth(_), false) => "partial depth, bare",
        (Payload::AvgPrice(_), _) => "avgPrice",
        _ => "unknown",
    }
}

#[tokio::test]
#[ignore]
async fn live_every_stream_kind_delivers() {
    // One SUBSCRIBE of ten names on one connection.
    let mut feed = SpotWsBuilder::new()
        .streams(every_kind())
        .connect()
        .await
        .or_fail("connect");
    let mut seen = HashSet::new();
    let mut last_outage = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while seen.len() < KINDS.len() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            // Ten BTCUSDT streams are never all quiet for a minute: a kind
            // missing here stopped delivering, or the connection never came
            // back.
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
                seen.insert(kind(&update));
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
    let names: Vec<String> = every_kind().into_iter().map(|s| s.to_string()).collect();
    // The names ride the URL, so this connection sends no request at all.
    let url = format!(
        "{SPOT_WS_BASE}/{}?streams={}",
        StreamPath::Stream.as_str(),
        names.join("/")
    );
    let mut unmodelled = Vec::new();
    // A raw socket through the kit's connect, which installs the TLS provider
    // (DRIFT R9) and fails a timeout as `ConnectTimeout`.
    let mut socket = polyoxide_ws::connect(url, CONNECT_TIMEOUT)
        .await
        .map_err(SpotWsError::from)
        .or_fail("connect");
    let mut checked = HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while let Ok(next) = tokio::time::timeout_at(deadline, socket.next()).await {
        let Some(message) = next else {
            transient("the server ended the connection");
        };
        let message = message
            .map_err(|err| SpotWsError::from(WsError::from(err)))
            .or_fail("read");
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
            // Binance documents these as "Ignore": the kline's `B`, and the
            // `M` of a trade and an aggregate trade.
            let ignored = key == "/data/k/B"
                || (key == "/data/M"
                    && matches!(update.payload, Payload::AggTrade(_) | Payload::Trade(_)));
            if !ignored {
                unmodelled.push(format!("{stream}: {key}"));
            }
        }
        // A book level has no keys, so a value appended to one is seen only
        // by its length.
        if matches!(update.payload, Payload::PartialDepth(_)) {
            for side in ["bids", "asks"] {
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
    }
    let missing: Vec<&String> = names.iter().filter(|n| !checked.contains(*n)).collect();
    assert!(
        missing.is_empty(),
        "in 10 s these streams sent no frame to check: {missing:?}"
    );
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
async fn live_a_client_ping_is_answered() {
    // A SUBSCRIBE, a ping, then a LIST_SUBSCRIPTIONS 500 ms after the
    // SUBSCRIBE: three messages in a second at most.
    let mut ws = SpotWs::connect([StreamName::MiniTicker(btc())])
        .await
        .or_fail("connect");
    // An unanswered ping is the fault under test, so it files whatever its
    // class; any other error, a server restart among them, fails by its class.
    let rtt = match ws.ping().await {
        Ok(rtt) => rtt,
        Err(err @ SpotWsError::NoAnswer { .. }) => panic!("pong: {err:?}"), // live-unwraps: the property under test
        Err(err) => fail("ping", &err),
    };
    assert!(rtt < Duration::from_secs(5), "{rtt:?}");
    assert_eq!(
        ws.list_subscriptions().await.or_fail("list"),
        ["btcusdt@miniTicker"]
    );
    ws.close().await.or_fail("close");
}
