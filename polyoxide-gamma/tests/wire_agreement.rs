//! Agreement between the comment types and payloads captured from the live
//! Gamma host. Provenance is in `tests/fixtures/README.md`.
//!
//! Two directions, both of which must fail if a type drifts from the wire:
//!
//! 1. **No invented fields.** Every non-null, non-empty value the type emits
//!    corresponds to a key the server actually sent.
//! 2. **No unmodelled fields.** Every key the server sent is either modelled
//!    or listed in `IGNORED` with a written reason.
//!
//! The oracle is the captured payload, not `docs/specs/gamma/openapi.yaml`.
//! The published spec is known to be wrong about this API — see
//! `docs/specs/gamma/OBSERVED.md`.
//!
//! # Known limitation: an invented `Option` field is invisible here
//!
//! Direction 1 must exempt `null`, because a legitimately-optional field the
//! server omitted also serializes to `null`. That exemption cannot distinguish
//! "optional and absent this time" from "does not exist at all", so a wholly
//! invented `Option<T>` field passes. Verified 2026-08-19 by adding
//! `totally_invented_field: Option<String>` to `Comment`: all three tests
//! still passed.
//!
//! This guard caught issue #28 only because that type's invented fields
//! included *required* ones (`user`, `like_count`), which failed
//! deserialization before these assertions ran. Had they all been `Option`,
//! it would have gone green.
//!
//! What this guard does reliably catch: any wire key that stops being
//! modelled (direction 2), any invented non-`Option` field, and any change in
//! the `ID`-suffixed key spellings. Closing the remaining gap needs a second
//! source for "does this field name exist anywhere" — see the follow-up in
//! `docs/plans/2026-08-19-gamma-type-parity-worklist.md`.

use polyoxide_gamma::types::Comment;
use serde_json::Value;

const FULL: &str = include_str!("fixtures/comment_full.json");
const SPARSE: &str = include_str!("fixtures/comment_sparse.json");

/// Wire keys deliberately left unmodelled, each with a reason.
///
/// Adding an entry is a written decision that shows up in a reviewed diff.
/// There is no wildcard. Paths are dotted from the root, e.g.
/// `comment.profile.someKey`.
const IGNORED: &[(&str, &str)] = &[];

/// Walk a captured payload against what the type re-emits, asserting both
/// directions at every level of nesting.
fn check(wire: &Value, emitted: &Value, path: &str) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            // Direction 1: nothing invented.
            for (key, value) in e {
                // `Option::None` serializes to null, and `#[serde(default)]`
                // on a Vec serializes to []. Neither claims the server sent
                // anything, so neither can be an invented field.
                if value.is_null() || matches!(value, Value::Array(a) if a.is_empty()) {
                    continue;
                }
                assert!(
                    w.contains_key(key),
                    "{path}.{key} is emitted by the type but absent from the captured \
                     payload — the field is invented"
                );
            }
            // Direction 2: nothing unmodelled.
            for (key, value) in w {
                match e.get(key) {
                    Some(emitted_value) => check(value, emitted_value, &format!("{path}.{key}")),
                    None => {
                        let full = format!("{path}.{key}");
                        assert!(
                            IGNORED.iter().any(|(k, _)| *k == full.as_str()),
                            "{full} is sent by the server but not modelled, and not listed \
                             in IGNORED with a reason"
                        );
                    }
                }
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            for (i, (wi, ei)) in w.iter().zip(e).enumerate() {
                check(wi, ei, &format!("{path}[{i}]"));
            }
        }
        _ => {}
    }
}

fn round_trip(fixture: &str, path: &str) {
    let wire: Value = serde_json::from_str(fixture).expect("fixture is valid JSON");
    let typed: Comment = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into Comment: {e}"));
    let emitted = serde_json::to_value(&typed).expect("Comment must serialize");
    check(&wire, &emitted, path);
}

#[test]
fn full_comment_agrees_with_captured_payload() {
    round_trip(FULL, "comment");
}

#[test]
fn sparse_comment_agrees_with_captured_payload() {
    round_trip(SPARSE, "comment");
}

#[test]
fn id_suffixed_keys_keep_their_wire_casing() {
    let wire: Value = serde_json::from_str(FULL).expect("fixture is valid JSON");
    let typed: Comment = serde_json::from_value(wire).expect("payload deserializes");
    let emitted = serde_json::to_value(&typed).expect("Comment serializes");

    // `rename_all = "camelCase"` would produce `parentEntityId` here, which the
    // server neither sends nor accepts.
    assert!(
        emitted.get("parentEntityID").is_some(),
        "parentEntityID must keep its capitalised suffix"
    );
    assert!(
        emitted.get("parentEntityId").is_none(),
        "rename_all must not be allowed to win over the explicit rename"
    );
    assert!(
        emitted.get("parentCommentID").is_some(),
        "parentCommentID must keep its capitalised suffix"
    );
    assert!(
        emitted["reactions"][0].get("commentID").is_some(),
        "commentID must keep its capitalised suffix"
    );
}
