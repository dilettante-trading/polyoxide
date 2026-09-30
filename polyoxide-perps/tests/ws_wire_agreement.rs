//! Agreement between the WebSocket types and frames captured from the live
//! host. Provenance is in `tests/fixtures/ws/PROVENANCE.md`.
//!
//! The AsyncAPI mirror is not the oracle here: it types `tickers` and
//! `statistics` data as arrays and `trades` as an object, and the wire does
//! the opposite (`docs/specs/perps/OBSERVED.md`). So the check is against
//! the wire alone: every fixture decodes into its type, re-serialises with
//! the same key paths, and every scalar compares equal.

use std::collections::BTreeSet;

use polyoxide_perps::{
    types::InstrumentId,
    ws::{Channel, Frame, Payload, StreamDepth},
};
use serde_json::Value;

/// The instrument the fixtures were captured for: the busiest by 24-hour
/// volume on capture day, chosen because `trades` on instrument 1 stayed
/// quiet for two 20 s windows. `PROVENANCE.md` records it.
const INSTRUMENT: InstrumentId = InstrumentId(6);

fn fixture(name: &str) -> (String, Value) {
    let path = format!(
        "{}/tests/fixtures/ws/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let value = serde_json::from_str(&text).unwrap();
    (text, value)
}

fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let p = format!("{prefix}/{k}");
                out.insert(p.clone());
                key_paths(v, &p, out);
            }
        }
        Value::Array(items) => items
            .iter()
            .for_each(|i| key_paths(i, &format!("{prefix}[]"), out)),
        _ => {}
    }
}

fn assert_values_agree(wire: &Value, emitted: &Value, path: &str) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            for (k, wv) in w {
                if let Some(ev) = e.get(k) {
                    assert_values_agree(wv, ev, &format!("{path}/{k}"));
                }
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            assert_eq!(w.len(), e.len(), "{path}: array length");
            for (i, (wv, ev)) in w.iter().zip(e).enumerate() {
                assert_values_agree(wv, ev, &format!("{path}[{i}]"));
            }
        }
        (w, e) => assert_eq!(w, e, "{path}: decoded as {e} but the wire sent {w}"),
    }
}

/// Decode a push fixture through the crate's parser and hand back the
/// `data` it re-serialises, so the comparison covers exactly the payload.
fn decode(name: &str) -> (Value, Frame) {
    let (text, value) = fixture(name);
    let frame = polyoxide_perps::ws::frame_from_text_for_tests(&text).expect("decodes");
    (value, frame)
}

#[test]
fn every_push_fixture_decodes_to_its_channel_and_round_trips() {
    let expected = [
        ("bbo", Channel::Bbo(INSTRUMENT)),
        ("book", Channel::Book(INSTRUMENT, StreamDepth::Twenty)),
        ("book_50", Channel::Book(INSTRUMENT, StreamDepth::Fifty)),
        ("trades", Channel::Trades(INSTRUMENT)),
        (
            "klines",
            Channel::Klines(INSTRUMENT, polyoxide_perps::types::Interval::M1),
        ),
        ("tickers", Channel::Tickers(Some(INSTRUMENT))),
        ("statistics", Channel::Statistics(Some(INSTRUMENT))),
    ];
    for (name, channel) in expected {
        let (wire, frame) = decode(name);
        let Frame::Update(update) = frame else {
            panic!("{name}: not an update")
        };
        assert_eq!(update.channel, channel, "{name}");
        assert!(
            update.ets.is_some() || wire["ets"] == 0,
            "{name}: ets present on every frame"
        );
        let emitted = match &update.payload {
            Payload::Bbo(d) => serde_json::to_value(d).unwrap(),
            Payload::Book(d) => serde_json::to_value(d).unwrap(),
            Payload::Trades(d) => serde_json::to_value(d).unwrap(),
            Payload::Klines(d) => serde_json::to_value(d).unwrap(),
            Payload::Ticker(d) => serde_json::to_value(d).unwrap(),
            Payload::Statistics(d) => serde_json::to_value(d).unwrap(),
            other => panic!("{name}: unexpected payload {other:?}"),
        };
        let mut sent = BTreeSet::new();
        key_paths(&wire["data"], "", &mut sent);
        let mut modelled = BTreeSet::new();
        key_paths(&emitted, "", &mut modelled);
        assert_eq!(sent, modelled, "{name}: data keys differ");
        assert_values_agree(&wire["data"], &emitted, "/data");
    }
}

#[test]
fn the_responses_have_the_documented_shapes() {
    let (_, sub) = fixture("response_subscribe");
    assert_eq!(sub["id"], 1);
    assert!(sub["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["status"] == "ok"));
    let (_, refused) = fixture("response_refused");
    let statuses = refused["data"].as_array().unwrap();
    assert_eq!(
        statuses[0]["status"], "ok",
        "an unknown instrument is accepted"
    );
    assert_eq!(statuses[1]["error"], "invalid channel");
    assert_eq!(statuses[2]["error"], "invalid channel");
    let (_, pong) = fixture("response_ping");
    assert_eq!(pong["id"], 2);
    assert_eq!(pong["data"]["status"], "ok");
    assert!(pong["data"]["sq"].is_u64());
}
