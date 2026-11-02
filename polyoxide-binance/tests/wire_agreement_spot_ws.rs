//! Agreement between the spot stream payload types and the captured frames in
//! `tests/fixtures/spot/ws/`, by the rules of `wire_agreement_usdm_ws.rs`:
//! nothing unmodelled, nothing invented, nothing altered.

use polyoxide_test_support::agreement as common;

use polyoxide_binance::spot::ws::{fixtures, Payload, StreamName, Update};
use polyoxide_test_support::agreement::Ledger;
use serde_json::Value;

/// `(fixture, path, reason)` the types deliberately drop.
pub const IGNORED: &[(&str, &str, &str)] = &[
    (
        "stream_btcusdt_trade",
        "/data/M",
        "Binance documents the trade's `M` as \"Ignore\"",
    ),
    (
        "stream_btcusdt_aggTrade",
        "/data/M",
        "Binance documents the aggregate trade's `M` as \"Ignore\"",
    ),
    (
        "stream_btcusdt_kline_1m",
        "/data/k/B",
        "Binance documents the kline's `B` as \"Ignore\"",
    ),
];

#[test]
fn every_stream_fixture_agrees_with_its_type() {
    let mut ledger = Ledger::new(&[IGNORED]);
    let mut used = Vec::new();
    for (fixture, frame) in fixtures::ALL {
        let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        assert!(
            !matches!(update.payload, Payload::Unknown { .. }),
            "{fixture}: decoded as Unknown"
        );
        let wire: Value = serde_json::from_str(frame).unwrap();
        let emitted = serde_json::to_value(&update).unwrap();
        let diff = common::compare_values(fixture, &wire, &emitted);
        for path in diff.unmodelled {
            match ledger.excuse(IGNORED, fixture, &path) {
                Some(entry) => used.push(entry),
                None => panic!("{fixture}: the server sent {path}, which the type does not model"),
            }
        }
        assert!(
            diff.invented.is_empty(),
            "{fixture}: the type emits {:?}, which the server did not send",
            diff.invented
        );
    }
    assert_eq!(
        used.len(),
        IGNORED.len(),
        "an IGNORED entry no fixture needs"
    );
}

#[test]
fn the_fixture_set_covers_every_stream_kind() {
    // A set that lost a kind would leave its payload type untested.
    let mut kinds = Vec::new();
    for (fixture, frame) in fixtures::ALL {
        let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        let kind = match (&update.stream, &update.payload) {
            (StreamName::AggTrade(_), Payload::AggTrade(_)) => "aggTrade",
            (StreamName::Trade(_), Payload::Trade(_)) => "trade",
            (StreamName::Kline(..), Payload::Kline(_)) => "kline",
            (StreamName::MiniTicker(_), Payload::MiniTicker(_)) => "miniTicker",
            (StreamName::AllMiniTickers, Payload::MiniTickers(_)) => "miniTicker@arr",
            (StreamName::Ticker(_), Payload::Ticker(_)) => "ticker",
            (StreamName::BookTicker(_), Payload::BookTicker(_)) => "bookTicker",
            (StreamName::PartialDepth(..), Payload::PartialDepth(_)) => "depth",
            (StreamName::AvgPrice(_), Payload::AvgPrice(_)) => "avgPrice",
            other => panic!("{fixture}: {other:?}"),
        };
        kinds.push(kind);
    }
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(
        kinds,
        [
            "aggTrade",
            "avgPrice",
            "bookTicker",
            "depth",
            "kline",
            "miniTicker",
            "miniTicker@arr",
            "ticker",
            "trade",
        ]
    );
}

#[test]
fn the_array_fixture_keeps_two_rows_and_the_depth_fixtures_their_levels() {
    let Payload::MiniTickers(rows) = Update::from_json(fixtures::ALL_MINI_TICKERS)
        .unwrap()
        .payload
    else {
        panic!("not MiniTickers");
    };
    assert_eq!(rows.len(), 2);

    // The bare name's frame is untrimmed (five levels a side) and the 100 ms
    // frame is trimmed to three.
    for (frame, levels) in [
        (fixtures::PARTIAL_DEPTH, 5),
        (fixtures::PARTIAL_DEPTH_100MS, 3),
    ] {
        let Payload::PartialDepth(e) = Update::from_json(frame).unwrap().payload else {
            panic!("not PartialDepth");
        };
        assert_eq!((e.bids.len(), e.asks.len()), (levels, levels));
    }
}

#[test]
fn the_non_ascii_fixture_is_a_non_ascii_symbol() {
    // The capture picks the first trading symbol with a non-ASCII character;
    // a fixture that lost it would leave the lowercasing rule untested.
    let update = Update::from_json(fixtures::NON_ASCII_MINI_TICKER).unwrap();
    let StreamName::MiniTicker(symbol) = update.stream else {
        panic!("not MiniTicker");
    };
    assert!(!symbol.as_str().is_ascii(), "{symbol}");
}
