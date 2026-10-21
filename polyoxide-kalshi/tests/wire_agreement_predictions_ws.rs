//! Agreement between the socket's frame types and the frames captured from the demo
//! host. Provenance is in `tests/fixtures/predictions/ws/PROVENANCE.md`.
//!
//! The AsyncAPI mirror is the starting point for the types, not the oracle: the check is
//! against the wire. Every fixture decodes into its type, re-serialises with the same key
//! paths (a key the wire sent and the type does not model fails, and so does one the type
//! writes and the wire did not send), and every scalar compares equal.

use std::collections::BTreeSet;

use polyoxide_kalshi::predictions::ws::frames::{
    Delta, Rejected, Snapshot, Subscribed, TickerFrame, TradeFrame, Unsubscribed, Updated,
};
use polyoxide_test_support::{agreement, fixtures, Fixtures};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

const ASYNCAPI: &str = include_str!("../../docs/specs/kalshi/asyncapi.yaml");

/// `(schema, key)`: keys the mirror requires of a payload's `msg` that the capture does not
/// carry, each an `Option` of its type and recorded in `docs/specs/kalshi/OBSERVED.md`. Empty
/// until the capture lacks a key the mirror requires; a developer who meets one adds its row.
const ABSENT_ON_THE_WIRE: &[(&str, &str)] = &[];

fn asyncapi() -> Value {
    let options = serde_saphyr::options! { budget: None };
    serde_saphyr::from_str_with_options(ASYNCAPI, options).expect("the mirror is YAML")
}

/// The keys the type writes in a frame's `msg`.
fn written_msg_keys<T: DeserializeOwned + Serialize>(text: &str) -> BTreeSet<String> {
    let frame: T = serde_json::from_str(text).expect("the fixture decodes");
    let written = serde_json::to_value(frame).expect("the type serialises");
    written["msg"]
        .as_object()
        .map(|msg| msg.keys().cloned().collect())
        .unwrap_or_default()
}

fn ws() -> Fixtures {
    fixtures!("predictions/ws")
}

fn assert_agrees<T: DeserializeOwned + Serialize>(what: &str, text: &str) {
    let disagreement = agreement::compare::<T>(what, text);
    assert!(
        disagreement.unmodelled.is_empty(),
        "{what}: the wire sent keys the type does not model: {:?}",
        disagreement.unmodelled
    );
    assert!(
        disagreement.invented.is_empty(),
        "{what}: the type writes keys the wire did not send: {:?}",
        disagreement.invented
    );
}

#[test]
fn the_subscribed_fixture_agrees() {
    assert_agrees::<Subscribed>("subscribed", &ws().text("subscribed"));
}

#[test]
fn the_snapshot_fixture_agrees() {
    assert_agrees::<Snapshot>("snapshot", &ws().text("snapshot"));
}

#[test]
fn the_ok_fixture_agrees() {
    assert_agrees::<Updated>("ok", &ws().text("ok"));
}

#[test]
fn the_unsubscribed_fixture_agrees() {
    assert_agrees::<Unsubscribed>("unsubscribed", &ws().text("unsubscribed"));
}

#[test]
fn the_error_fixture_agrees() {
    assert_agrees::<Rejected>("error", &ws().text("error"));
}

#[test]
fn every_captured_delta_agrees() {
    let deltas = ws().json("deltas");
    let Some(frames) = deltas.as_array() else {
        panic!("deltas.json is not an array");
    };
    assert!(
        !frames.is_empty(),
        "a capture with no delta proves nothing about the delta type"
    );
    for (n, frame) in frames.iter().enumerate() {
        assert_agrees::<Delta>(&format!("delta {n}"), &frame.to_string());
    }
}

#[test]
fn the_captured_snapshot_is_not_crossed() {
    // `use_yes_price` puts both sides on one scale. A book crossed here would mean the
    // flag was not honoured and the asks are still in NO pricing.
    let snapshot: Snapshot = serde_json::from_str(&ws().text("snapshot")).unwrap();
    let bids: Vec<Decimal> = snapshot
        .msg
        .yes_dollars_fp
        .iter()
        .map(|level| level.price)
        .collect();
    let asks: Vec<Decimal> = snapshot
        .msg
        .no_dollars_fp
        .iter()
        .map(|level| level.price)
        .collect();
    assert!(
        !bids.is_empty() && !asks.is_empty(),
        "a one-sided snapshot cannot show whether the book is crossed"
    );
    let (best_bid, best_ask) = (bids.iter().max().unwrap(), asks.iter().min().unwrap());
    assert!(
        best_bid < best_ask,
        "the best bid {best_bid} is not below the best ask {best_ask}"
    );
}

#[test]
fn sequence_numbers_count_by_one_on_the_wire() {
    // The premise of the socket's gap check, pinned to the capture: after the snapshot,
    // each delta of the subscription is the previous number plus one.
    let snapshot: Snapshot = serde_json::from_str(&ws().text("snapshot")).unwrap();
    let deltas: Vec<Delta> = serde_json::from_str(&ws().text("deltas")).unwrap();
    let mut expected = snapshot.seq;
    for delta in &deltas {
        expected += 1;
        assert_eq!(
            (delta.sid, delta.seq),
            (snapshot.sid, expected),
            "{delta:?}"
        );
    }
}

#[test]
fn the_answers_carry_the_ids_and_sequences_the_protocol_reads() {
    // The correlation matches an answer to its command by `id`, and an `ok` or an
    // `unsubscribed` counts in its subscription's sequence by `sid` and `seq`.
    let subscribed: Subscribed = serde_json::from_str(&ws().text("subscribed")).unwrap();
    assert!(subscribed.id.is_some(), "{subscribed:?}");
    let ok: Updated = serde_json::from_str(&ws().text("ok")).unwrap();
    assert!(
        ok.id.is_some() && ok.sid.is_some() && ok.seq.is_some(),
        "{ok:?}"
    );
    let unsubscribed: Unsubscribed = serde_json::from_str(&ws().text("unsubscribed")).unwrap();
    assert!(unsubscribed.id.is_some(), "{unsubscribed:?}");
    let rejected: Rejected = serde_json::from_str(&ws().text("error")).unwrap();
    assert!(rejected.id.is_some(), "{rejected:?}");
}

#[test]
fn the_ticker_fixture_agrees() {
    assert_agrees::<TickerFrame>("ticker", &ws().text("ticker"));
}

#[test]
fn the_trade_fixture_agrees() {
    assert_agrees::<TradeFrame>("trade", &ws().text("trade"));
}

#[test]
fn the_ticker_channel_carries_no_seq_and_the_trade_channel_does() {
    // The premise of the protocol's sequence check: a `seq` on a ticker frame would make
    // every one after the first a candidate gap, and none on a trade would leave its
    // subscription unchecked.
    let ticker = ws().json("ticker");
    assert!(ticker["sid"].is_u64(), "{ticker}");
    assert!(ticker.get("seq").is_none(), "{ticker}");
    let trade = ws().json("trade");
    assert!(trade["sid"].is_u64() && trade["seq"].is_u64(), "{trade}");
}

#[test]
fn the_subscribed_answers_of_the_other_channels_agree_and_name_their_channel() {
    // One `subscribe` naming three channels is answered by one `subscribed` for each; the
    // protocol keeps the `sid` of each by the channel the answer names.
    for (name, channel) in [
        ("subscribed_ticker", "ticker"),
        ("subscribed_trade", "trade"),
    ] {
        assert_agrees::<Subscribed>(name, &ws().text(name));
        let answer: Subscribed = serde_json::from_str(&ws().text(name)).unwrap();
        assert_eq!(answer.msg.channel, channel, "{name}");
        assert!(
            answer.id.is_some(),
            "{name}: the answer carries the command's id"
        );
    }
}

#[test]
fn the_requested_snapshot_agrees_and_carries_the_commands_id_and_the_subscriptions_numbering() {
    let text = ws().text("snapshot_answer");
    assert_agrees::<Snapshot>("snapshot_answer", &text);
    let answer: Snapshot = serde_json::from_str(&text).unwrap();
    // The re-snapshot recognises its answers by the id, and re-anchors on the `sid` and
    // `seq` they carry.
    assert!(answer.id.is_some(), "{answer:?}");
    // And it is on a subscription's numbering: `deltas_after_a_requested_snapshot_continue_its_numbering`
    // pins that the frames after it continue from this `seq`.
    assert!(answer.seq >= 1, "{answer:?}");
}

#[test]
fn deltas_after_a_requested_snapshot_continue_its_numbering() {
    // The premise of re-anchoring: after the snapshot that answered `get_snapshot`, the
    // subscription's next frame is the answer's `seq` plus one. If the capture shows
    // otherwise, the design (D9) does not hold and the story stops and asks the owner.
    let answer: Snapshot = serde_json::from_str(&ws().text("snapshot_answer")).unwrap();
    let deltas: Vec<Delta> = serde_json::from_str(&ws().text("deltas_after_snapshot")).unwrap();
    assert!(
        !deltas.is_empty(),
        "a capture with no delta after the snapshot proves nothing about the numbering"
    );
    let mut expected = answer.seq;
    for delta in &deltas {
        expected += 1;
        assert_eq!((delta.sid, delta.seq), (answer.sid, expected), "{delta:?}");
    }
    let frames = ws().json("deltas_after_snapshot");
    for (n, frame) in frames.as_array().expect("an array").iter().enumerate() {
        assert_agrees::<Delta>(&format!("delta after the snapshot {n}"), &frame.to_string());
    }
}

#[test]
fn a_snapshot_for_a_ticker_outside_the_subscription_is_the_snapshot_frame_with_the_commands_id() {
    // Kalshi answers `get_snapshot` for a ticker the subscription does not hold with an
    // ordinary `orderbook_snapshot` on the order-book `sid`, carrying the command's `id`
    // (the demo host, 2026-10-21), and not with an error. The protocol reads it as it reads
    // any other answer, so `snapshot()` refuses such a ticker before anything is sent.
    let text = ws().text("snapshot_outside");
    assert_agrees::<Snapshot>("snapshot_outside", &text);
    let answer: Snapshot = serde_json::from_str(&text).unwrap();
    assert!(answer.id.is_some(), "{answer:?}");
    assert!(answer.sid >= 1 && answer.seq >= 1, "{answer:?}");
}

#[test]
fn the_errors_for_a_snapshot_on_the_other_channels_agree_and_carry_what_the_protocol_reads() {
    // On the ticker channel's `sid` the error answers the command (its `id`, no `seq`); on the
    // trade channel's it carries no `id` and takes the next `seq` of its `sid`, so it is the
    // stream's, and counts in the trade subscription's numbering.
    let text = ws().text("error_snapshot_ticker_sid");
    assert_agrees::<Rejected>("error_snapshot_ticker_sid", &text);
    let rejected: Rejected = serde_json::from_str(&text).unwrap();
    assert!(
        rejected.id.is_some() && rejected.sid.is_some(),
        "{rejected:?}"
    );
    assert!(rejected.seq.is_none(), "{rejected:?}");
    let text = ws().text("error_snapshot_trade_sid");
    assert_agrees::<Rejected>("error_snapshot_trade_sid", &text);
    let rejected: Rejected = serde_json::from_str(&text).unwrap();
    assert!(rejected.id.is_none(), "{rejected:?}");
    assert!(
        rejected.sid.is_some() && rejected.seq.is_some(),
        "{rejected:?}"
    );
}

#[test]
fn the_answer_to_an_update_naming_two_subscriptions_agrees_whatever_it_is() {
    // Story 1 records what Kalshi does with several `sids` in one command: one `ok`, one for
    // each, or an error. Whichever it is, the frames decode into the types the protocol
    // reads them as.
    let wire = ws().json("multi_sid_answer");
    let frames = match wire {
        Value::Array(frames) => frames,
        single => vec![single],
    };
    assert!(!frames.is_empty());
    for (n, frame) in frames.iter().enumerate() {
        let text = frame.to_string();
        match frame["type"].as_str() {
            Some("ok") => assert_agrees::<Updated>(&format!("multi_sid_answer {n}"), &text),
            Some("error") => assert_agrees::<Rejected>(&format!("multi_sid_answer {n}"), &text),
            other => {
                panic!("multi_sid_answer {n}: a frame type the protocol does not read: {other:?}")
            }
        }
    }
}

#[test]
fn the_ticker_and_trade_types_write_every_key_the_asyncapi_requires_of_them() {
    // The spec-agreement coverage of `tickerPayload` and `tradePayload`: every key the mirror
    // requires of a `msg` is one the type writes for the capture, so the mirror and the types
    // cannot part without a test saying so. A key the capture lacks is an `Option` of its type,
    // excused in `ABSENT_ON_THE_WIRE` and recorded in `OBSERVED.md`.
    let spec = asyncapi();
    for (schema, written) in [
        (
            "tickerPayload",
            written_msg_keys::<TickerFrame>(&ws().text("ticker")),
        ),
        (
            "tradePayload",
            written_msg_keys::<TradeFrame>(&ws().text("trade")),
        ),
    ] {
        let required = spec["components"]["schemas"][schema]["properties"]["msg"]["required"]
            .as_array()
            .unwrap_or_else(|| panic!("{schema} lists no required keys in the mirror"));
        for key in required.iter().filter_map(Value::as_str) {
            if ABSENT_ON_THE_WIRE.contains(&(schema, key)) {
                continue;
            }
            assert!(
                written.contains(key),
                "{schema}: the mirror requires {key}, which the type does not write for the capture"
            );
        }
    }
}

#[test]
fn a_one_sid_update_is_answered_ok_on_its_own_sid_and_only_the_numbered_channels_carry_a_seq() {
    // The facts the protocol's one-command-per-`sid` change rests on (the demo host,
    // 2026-10-21): every `sid` answers with an `ok` carrying the command's `id`, its own `sid`
    // and the whole membership; the order-book and trade answers take the next `seq` of their
    // `sid`, the ticker's carries none.
    for name in ["update_add_answers", "update_delete_answers"] {
        let answers = ws().json(name);
        let answers = answers.as_array().expect("an array of answers");
        assert!(!answers.is_empty(), "{name}");
        for (n, frame) in answers.iter().enumerate() {
            let what = format!("{name} {n}");
            assert_agrees::<Updated>(&what, &frame.to_string());
            let ok: Updated = serde_json::from_value(frame.clone()).unwrap();
            assert!(ok.id.is_some() && ok.sid.is_some(), "{what}: {ok:?}");
        }
        let seq = |n: usize| answers[n].get("seq").and_then(Value::as_u64);
        // The additions answer on all three `sid`s; the deletions on the ticker and trade.
        let (ticker, trade) = if name == "update_add_answers" {
            assert!(seq(0).is_some(), "the order book's answer is numbered");
            (1, 2)
        } else {
            (0, 1)
        };
        assert!(
            seq(ticker).is_none(),
            "{name}: the ticker's answer is not numbered"
        );
        assert!(
            seq(trade).is_some(),
            "{name}: the trade's answer is numbered"
        );
    }
}
