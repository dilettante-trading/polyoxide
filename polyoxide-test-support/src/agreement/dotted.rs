//! The walker for suites that name key paths with dots from a root name,
//! `comment.reactions[0].profile.bio`, and assert as they walk.
//!
//! Unlike [`key_paths`](super::key_paths), arrays are walked by index and must
//! be the same length on both sides, and values are not compared. The two
//! allow-lists are `(path, reason)` pairs, matched against the full dotted
//! path with no wildcard.

use serde_json::Value;

use super::Excuse;

/// Walks a captured payload against what the type re-emits, asserting both
/// directions at every level of nesting:
///
/// 1. **Nothing invented.** A key the type emits but the wire lacks must be in
///    `expected_absent`. A key present on both sides is recursed into.
/// 2. **Nothing unmodelled.** A key the wire sent but the type does not emit
///    must be in `ignored`.
///
/// Arrays must have the same length on both sides, since comparing only the
/// shorter length would let a truncated collection pass.
#[track_caller]
pub fn check<E: Excuse>(
    wire: &Value,
    emitted: &Value,
    path: &str,
    ignored: &[E],
    expected_absent: &[E],
) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            // Direction 1: nothing invented. A key present on both sides
            // recurses so nested mismatches are caught too; a key the type
            // emits but the wire lacks must be declared in EXPECTED_ABSENT.
            for (key, value) in e {
                let full = format!("{path}.{key}");
                match w.get(key) {
                    Some(wire_value) => check(wire_value, value, &full, ignored, expected_absent),
                    None => assert!(
                        expected_absent.iter().any(|k| k.path() == full.as_str()),
                        "{full} is emitted by the type but absent from the captured \
                         payload, and not listed in EXPECTED_ABSENT with a reason — \
                         the field may be invented"
                    ),
                }
            }
            // Direction 2: nothing unmodelled. Keys present on both sides
            // were already recursed into above.
            for key in w.keys() {
                if e.contains_key(key) {
                    continue;
                }
                let full = format!("{path}.{key}");
                assert!(
                    ignored.iter().any(|k| k.path() == full.as_str()),
                    "{full} is sent by the server but not modelled, and not listed \
                     in IGNORED with a reason"
                );
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            assert_eq!(
                w.len(),
                e.len(),
                "{path} has {} elements on the wire but the type re-emits {} — a \
                 truncated collection would otherwise pass unnoticed by comparing \
                 only the shorter length",
                w.len(),
                e.len()
            );
            for (i, (wi, ei)) in w.iter().zip(e).enumerate() {
                check(wi, ei, &format!("{path}[{i}]"), ignored, expected_absent);
            }
        }
        _ => {}
    }
}

/// The top-level keys of the object `wire` that the object `emitted` lacks,
/// in the wire's order: direction 2 of [`check`] at the root alone, with no
/// allow-list and no recursion.
///
/// It is for a type that declares far more fields than any one payload
/// carries, where a field absent from a capture is the normal case and
/// direction 1 would mean nothing.
#[track_caller]
pub fn unmodelled_top_level(wire: &Value, emitted: &Value) -> Vec<String> {
    let emitted = emitted
        .as_object()
        .expect("the type serializes to an object");
    wire.as_object()
        .expect("the captured payload is an object")
        .keys()
        .filter(|key| !emitted.contains_key(*key))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;

    use serde_json::json;

    use super::*;

    const IGNORED: &[(&str, &str)] = &[("user.$schema", "response metadata")];
    const EXPECTED_ABSENT: &[(&str, &str)] = &[("user.bio", "not set by this subject")];

    fn message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let panic = catch_unwind(f).unwrap_err();
        panic.downcast_ref::<String>().unwrap().clone()
    }

    #[test]
    fn excused_differences_pass_in_both_directions() {
        let wire = json!({"name": "a", "$schema": "x", "tags": [{"id": 1}]});
        let emitted = json!({"name": "a", "bio": null, "tags": [{"id": 1}]});
        check(&wire, &emitted, "user", IGNORED, EXPECTED_ABSENT);
    }

    #[test]
    fn an_invented_nested_key_fails_with_its_dotted_path() {
        let wire = json!({"tags": [{"id": 1}]});
        let emitted = json!({"tags": [{"id": 1, "made_up": null}]});
        let message = message(|| check(&wire, &emitted, "user", IGNORED, EXPECTED_ABSENT));
        assert!(
            message.starts_with("user.tags[0].made_up is emitted by the type"),
            "{message}"
        );
    }

    #[test]
    fn an_unmodelled_key_fails_with_its_dotted_path() {
        let wire = json!({"name": "a", "extra": 1});
        let emitted = json!({"name": "a"});
        let message = message(|| check(&wire, &emitted, "user", IGNORED, EXPECTED_ABSENT));
        assert!(
            message.starts_with("user.extra is sent by the server but not modelled"),
            "{message}"
        );
    }

    #[test]
    fn a_truncated_array_fails() {
        let wire = json!({"tags": [1, 2]});
        let emitted = json!({"tags": [1]});
        let message = message(|| check(&wire, &emitted, "user", IGNORED, EXPECTED_ABSENT));
        assert!(
            message.contains("user.tags has 2 elements on the wire but the type re-emits 1"),
            "{message}"
        );
    }

    #[test]
    fn the_top_level_check_sees_only_unmodelled_root_keys() {
        let wire = json!({"id": 1, "gameId": 2, "teams": [{"new": 1}], "$schema": "x"});
        let emitted = json!({"id": 1, "teams": [], "volume": null});
        let mut unmodelled = unmodelled_top_level(&wire, &emitted);
        unmodelled.sort();
        assert_eq!(unmodelled, ["$schema", "gameId"]);
    }
}
