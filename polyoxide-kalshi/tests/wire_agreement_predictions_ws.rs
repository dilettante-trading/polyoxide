//! Agreement between the socket's frame types and the frames captured from the demo
//! host. Provenance is in `tests/fixtures/predictions/ws/PROVENANCE.md`.
//!
//! The AsyncAPI mirror is the starting point for the types, not the oracle: the check is
//! against the wire. Every fixture decodes into its type, re-serialises with the same key
//! paths (a key the wire sent and the type does not model fails, and so does one the type
//! writes and the wire did not send), and every scalar compares equal.

use polyoxide_kalshi::predictions::ws::frames::{
    Delta, Rejected, Snapshot, Subscribed, Unsubscribed, Updated,
};
use polyoxide_test_support::{agreement, fixtures, Fixtures};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};

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
