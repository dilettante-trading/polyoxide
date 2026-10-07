//! Agreement between the stream payload types and the captured frames in
//! `tests/fixtures/ws/`, by the rules of `wire_agreement.rs`: nothing
//! unmodelled, nothing invented, nothing altered.

mod common;

use polyoxide_binance::usdm::ws::{fixtures, Payload, Update};
use serde_json::Value;

/// `(fixture, path, reason)` the types deliberately drop.
pub const IGNORED: &[(&str, &str, &str)] = &[(
    "stream_btcusdt_kline_1m",
    "/data/k/B",
    "Binance documents the kline's `B` as \"Ignore\"",
)];

#[test]
fn every_stream_fixture_agrees_with_its_type() {
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
            match IGNORED.iter().find(|(f, p, _)| f == fixture && *p == path) {
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
