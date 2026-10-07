//! Wire-agreement machinery shared by the REST and stream agreement tests and
//! the live suites.

// Each test file that includes this module uses a different part of it.
#![allow(dead_code)]

use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// Every key path in a JSON value: `/symbols[]/filters[]/tickSize`. Arrays of
/// scalars contribute no path, so a positional row is compared by value only.
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

/// Asserts every scalar present on both sides is equal, recursing objects by
/// key and arrays by index, so a value that rounds or overflows on decode fails
/// here rather than passing on its key alone.
pub fn assert_values_agree(what: &str, path: &str, wire: &Value, emitted: &Value) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            for (key, wv) in w {
                if let Some(ev) = e.get(key) {
                    assert_values_agree(what, &format!("{path}/{key}"), wv, ev);
                }
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            for (i, (wv, ev)) in w.iter().zip(e).enumerate() {
                assert_values_agree(what, &format!("{path}[{i}]"), wv, ev);
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
pub struct Disagreement {
    pub unmodelled: Vec<String>,
    pub invented: Vec<String>,
}

/// Decodes `text` as `T`, re-encodes it, and compares the two.
pub fn compare<T: DeserializeOwned + Serialize>(what: &str, text: &str) -> Disagreement {
    let wire: Value =
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{what}: not JSON: {e}"));
    let parsed: T = serde_json::from_str(text).unwrap_or_else(|e| panic!("{what}: {e}"));
    compare_values(what, &wire, &serde_json::to_value(&parsed).unwrap())
}

/// Compares what the wire sent with what a type emitted after decoding it.
pub fn compare_values(what: &str, wire: &Value, emitted: &Value) -> Disagreement {
    assert_values_agree(what, "", wire, emitted);
    let mut sent = BTreeSet::new();
    key_paths(wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(emitted, "", &mut modelled);
    Disagreement {
        unmodelled: sent.difference(&modelled).cloned().collect(),
        invented: modelled.difference(&sent).cloned().collect(),
    }
}
