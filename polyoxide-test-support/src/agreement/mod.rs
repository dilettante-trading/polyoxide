//! Agreement between a type and the payloads captured from its host.
//!
//! A wire-agreement test decodes a captured payload into its type, encodes it
//! back, and compares the two:
//!
//! 1. **Nothing unmodelled.** Every key path the server sent is emitted by the
//!    type, unless an allow-list excuses it.
//! 2. **Nothing invented.** Every key path the type emits was sent by the
//!    server, unless an allow-list excuses it. An `Option` encodes as `null`,
//!    so a field modelled but absent from the wire shows up here instead of
//!    hiding.
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a
//!    value that rounds or overflows on decode fails rather than passing on its
//!    key alone ([`assert_values_agree`]).
//!
//! Key paths are slash-separated from the root, with `[]` for an array's
//! elements ([`key_paths`]). [`excuse`] holds the allow-lists and finds the
//! entries no fixture needs any more, and [`dotted`] is the walker for suites
//! that name paths with dots and assert as they go.

use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

pub mod dotted;
pub mod excuse;

pub use excuse::{Excuse, Ledger};

/// Adds every key path in `value` to `out`, each prefixed by `prefix`:
/// `/symbols[]/filters[]/tickSize`. An array contributes `[]` to its
/// elements' paths and no path of its own, so an array of scalars, such as a
/// positional row, adds nothing and is compared by value only.
pub fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let path = format!("{prefix}/{key}");
                out.insert(path.clone());
                key_paths(child, &path, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                key_paths(item, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

/// How [`assert_values_agree`] compares two arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrays {
    /// By index, as far as the shorter side goes, for a type that drops a
    /// trailing element on purpose.
    Zip,
    /// The lengths must be equal, then by index.
    SameLength,
}

/// Asserts every scalar present on both sides is equal, recursing objects by
/// key and arrays by index, as `arrays` says. A key on one side only is the
/// key-path checks' business, not this one's; an object or array facing a
/// scalar, or each other, fails. `what` names the payload in the message.
#[track_caller]
pub fn assert_values_agree(what: &str, path: &str, wire: &Value, emitted: &Value, arrays: Arrays) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            for (key, wv) in w {
                if let Some(ev) = e.get(key) {
                    assert_values_agree(what, &format!("{path}/{key}"), wv, ev, arrays);
                }
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            if arrays == Arrays::SameLength {
                assert_eq!(w.len(), e.len(), "{what}: {path}: array length");
            }
            for (i, (wv, ev)) in w.iter().zip(e).enumerate() {
                assert_values_agree(what, &format!("{path}[{i}]"), wv, ev, arrays);
            }
        }
        (Value::Object(_) | Value::Array(_), _) | (_, Value::Object(_) | Value::Array(_)) => {
            panic!("{what}: {path} decoded as {emitted} but the wire sent {wire}")
        }
        _ => assert_eq!(
            wire, emitted,
            "{what}: {path} decoded as {emitted} but the wire sent {wire}"
        ),
    }
}

/// Paths the server sent that the type does not emit, and paths the type
/// emits that the server did not send, after checking every shared value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    /// Sent by the server, not emitted by the type.
    pub unmodelled: Vec<String>,
    /// Emitted by the type, not sent by the server.
    pub invented: Vec<String>,
}

/// Decodes `text` as `T`, encodes it back, and compares the two as
/// [`compare_values`] does. Panics, naming `what`, when `text` is not JSON or
/// does not decode as `T`.
#[track_caller]
pub fn compare<T: DeserializeOwned + Serialize>(what: &str, text: &str) -> Disagreement {
    // `match`es, not closures: `#[track_caller]` does not reach into one.
    let wire: Value = match serde_json::from_str(text) {
        Ok(wire) => wire,
        Err(e) => panic!("{what}: not JSON: {e}"),
    };
    let parsed: T = match serde_json::from_str(text) {
        Ok(parsed) => parsed,
        Err(e) => panic!("{what}: {e}"),
    };
    let emitted = serde_json::to_value(&parsed).expect("a value serialises");
    compare_values(what, &wire, &emitted)
}

/// Compares what the wire sent with what a type emitted after decoding it:
/// asserts every shared value agrees, arrays zipped ([`Arrays::Zip`]), and
/// returns the key paths on one side only.
#[track_caller]
pub fn compare_values(what: &str, wire: &Value, emitted: &Value) -> Disagreement {
    assert_values_agree(what, "", wire, emitted, Arrays::Zip);
    let mut sent = BTreeSet::new();
    key_paths(wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(emitted, "", &mut modelled);
    Disagreement {
        unmodelled: sent.difference(&modelled).cloned().collect(),
        invented: modelled.difference(&sent).cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;

    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    fn paths(value: &Value) -> Vec<String> {
        let mut out = BTreeSet::new();
        key_paths(value, "", &mut out);
        out.into_iter().collect()
    }

    fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let panic = catch_unwind(f).unwrap_err();
        match panic.downcast_ref::<String>() {
            Some(message) => message.clone(),
            None => panic.downcast_ref::<&str>().unwrap().to_string(),
        }
    }

    #[test]
    fn key_paths_recurse_objects_and_arrays_and_skip_scalar_rows() {
        let value = json!({"a": {"b": 1}, "rows": [{"c": 1}, {"d": 2}], "kline": [[1, "2"]]});
        assert_eq!(
            paths(&value),
            ["/a", "/a/b", "/kline", "/rows", "/rows[]/c", "/rows[]/d"]
        );
    }

    #[test]
    fn values_agree_by_key_and_index_ignoring_one_sided_keys() {
        let wire = json!({"price": "1.50", "only_wire": 1, "rows": [1, 2]});
        let emitted = json!({"price": "1.50", "only_type": null, "rows": [1, 2]});
        assert_values_agree("fixture", "", &wire, &emitted, Arrays::SameLength);
    }

    #[test]
    fn an_altered_scalar_fails_with_its_path() {
        let message = panic_message(|| {
            assert_values_agree(
                "fixture",
                "",
                &json!({"rows": [{"p": "0.1000000000000000000000000001"}]}),
                &json!({"rows": [{"p": "0.1"}]}),
                Arrays::Zip,
            )
        });
        assert!(
            message.contains("fixture: /rows[0]/p decoded as \"0.1\""),
            "{message}"
        );
    }

    #[test]
    fn a_structure_facing_a_scalar_fails() {
        let message = panic_message(|| {
            assert_values_agree("f", "", &json!({"a": [1]}), &json!({"a": 1}), Arrays::Zip)
        });
        assert_eq!(message, "f: /a decoded as 1 but the wire sent [1]");
    }

    #[test]
    fn zip_compares_as_far_as_the_shorter_array_and_same_length_does_not() {
        let wire = json!([1, 2, 3]);
        let emitted = json!([1, 2]);
        assert_values_agree("kline", "", &wire, &emitted, Arrays::Zip);
        let message = panic_message(|| {
            assert_values_agree("kline", "/data", &wire, &emitted, Arrays::SameLength)
        });
        assert!(message.contains("kline: /data: array length"), "{message}");
    }

    #[derive(Deserialize, Serialize)]
    struct Row {
        a: u32,
        b: Option<u32>,
    }

    #[test]
    fn compare_reports_both_directions() {
        let diff = compare::<Row>("row", r#"{"a": 1, "c": 3}"#);
        assert_eq!(diff.unmodelled, ["/c"]);
        assert_eq!(diff.invented, ["/b"]);
    }

    #[test]
    fn compare_names_the_payload_that_does_not_decode() {
        let message = panic_message(|| {
            compare::<Row>("row", r#"{"b": 1}"#);
        });
        assert!(message.starts_with("row: missing field `a`"), "{message}");
        let message = panic_message(|| {
            compare::<Row>("row", "<html>");
        });
        assert!(message.starts_with("row: not JSON: "), "{message}");
    }
}
