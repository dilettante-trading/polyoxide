//! Agreement between the stream payload types and the captured frames in
//! `tests/fixtures/ws/`, by the rules of `wire_agreement.rs`: nothing
//! unmodelled, nothing invented, nothing altered.

use polyoxide_test_support::agreement as common;

use polyoxide_binance::usdm::ws::{fixtures, Payload, SymbolType, Update};
use polyoxide_test_support::agreement::Ledger;
use serde_json::Value;

/// `(fixture, path, reason)` the types deliberately drop.
pub const IGNORED: &[(&str, &str, &str)] = &[(
    "stream_btcusdt_kline_1m",
    "/data/k/B",
    "Binance documents the kline's `B` as \"Ignore\"",
)];

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
fn the_array_fixtures_carry_a_coin_m_row() {
    // The capture reads an array stream until a frame carries a COIN-M row, so
    // both arrays keep one after a USDⓈ-M row; a re-capture that lost it would
    // leave the COIN-M row's shape untested.
    for (fixture, frame) in [
        ("stream_all_ticker_arr", fixtures::ALL_TICKERS),
        ("stream_all_markPrice_arr_1s", fixtures::ALL_MARK_PRICES),
    ] {
        let update = Update::from_json(frame).unwrap();
        let types: Vec<SymbolType> = match update.payload {
            Payload::Tickers(rows) => rows.iter().map(|r| r.symbol_type).collect(),
            Payload::MarkPrices(rows) => rows.iter().map(|r| r.symbol_type).collect(),
            other => panic!("{fixture}: {other:?}"),
        };
        assert_eq!(types, [SymbolType::Um, SymbolType::Cm], "{fixture}");
    }
}
