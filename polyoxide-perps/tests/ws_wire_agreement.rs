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
    ws::{incoming_from_text_for_tests, Channel, Frame, IncomingForTests, Payload, StreamDepth},
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

fn statuses(name: &str) -> Vec<(bool, Option<String>)> {
    match incoming_from_text_for_tests(&fixture(name).0).expect(name) {
        IncomingForTests::Statuses(statuses) => statuses,
        other => panic!("{name}: expected statuses, got {other:?}"),
    }
}

#[test]
fn every_response_fixture_decodes_through_the_control_path() {
    let subscribed = statuses("response_subscribe");
    assert_eq!(subscribed.len(), 9, "one status per requested channel");
    assert!(subscribed.iter().all(|(ok, error)| *ok && error.is_none()));

    let refused = statuses("response_refused");
    let invalid = || Some("invalid channel".to_owned());
    assert_eq!(
        refused,
        vec![(true, None), (false, invalid()), (false, invalid())],
        "an unknown instrument is accepted; only malformed names are refused"
    );

    assert_eq!(statuses("response_unsubscribe"), vec![(true, None)]);

    match incoming_from_text_for_tests(&fixture("response_ping").0).expect("pong") {
        IncomingForTests::Pong { ok, sq } => {
            assert!(ok);
            assert!(sq.is_some(), "a pong carries the server's sequence stamp");
        }
        other => panic!("expected a pong, got {other:?}"),
    }
}

#[test]
fn every_asyncapi_channel_example_parses_and_renders_back() {
    // The published patterns are the only statement of the channel grammar.
    // Their own examples must parse through `Channel` and render back
    // unchanged, except that `book::1` renders at the default depth.
    let path = format!(
        "{}/../docs/specs/perps/asyncapi.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let spec: Value = serde_json::from_str(&text).unwrap();
    let mut seen = 0;
    for name in ["bbo", "book", "trades", "klines", "tickers", "statistics"] {
        let chs =
            &spec["channels"][name]["messages"]["SubscribeRequest"]["payload"]["properties"]["chs"];
        let examples = chs["example"]
            .as_array()
            .unwrap_or_else(|| panic!("{name}: no chs example in the AsyncAPI"));
        for example in examples {
            let text = example.as_str().unwrap();
            let channel: Channel = text
                .parse()
                .unwrap_or_else(|e| panic!("{name}: example {text:?} did not parse: {e:?}"));
            let rendered = channel.to_string();
            if text == "book::1" {
                assert_eq!(
                    rendered, "book::1::20",
                    "the bare book name renders at the default depth"
                );
            } else {
                assert_eq!(rendered, text, "{name}: example renders back unchanged");
            }
            seen += 1;
        }
    }
    assert_eq!(
        seen, 7,
        "every example of the six public channels was checked"
    );
}
