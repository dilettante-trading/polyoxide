//! A helper's failure points at the test that called it, not into this crate.
//!
//! `#[track_caller]` does not reach into a closure, so a helper that panics
//! inside `unwrap_or_else(|e| panic!(..))` reports its own line. Each case
//! here records where the panic was reported and requires this file.

use std::{
    cell::RefCell,
    panic::{self, AssertUnwindSafe},
    sync::Once,
};

use polyoxide_test_support::{
    agreement::{self, dotted, Arrays},
    openapi, Fixtures,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

thread_local! {
    static AT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The file `f`'s panic was reported in.
fn panic_file(f: impl FnOnce()) -> String {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if let Some(at) = info.location() {
                AT.with(|cell| *cell.borrow_mut() = Some(at.file().to_owned()));
            }
            previous(info);
        }));
    });
    assert!(
        panic::catch_unwind(AssertUnwindSafe(f)).is_err(),
        "the helper did not panic"
    );
    AT.with(|cell| cell.borrow_mut().take())
        .expect("a panic location")
}

#[track_caller]
fn assert_reported_here(f: impl FnOnce()) {
    let file = panic_file(f);
    assert!(file.ends_with("tests/locations.rs"), "reported at {file}");
}

fn scratch() -> Fixtures {
    let dir = std::env::temp_dir().join(format!(
        "polyoxide-test-support-locations-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("page.json"), "<html>").unwrap();
    Fixtures::at(dir)
}

#[test]
fn a_fixture_failure_points_at_the_caller() {
    let fixtures = scratch();
    assert_reported_here(|| {
        fixtures.text("absent");
    });
    assert_reported_here(|| {
        fixtures.json("page");
    });
}

#[derive(Debug, Deserialize, Serialize)]
struct Row {
    a: u32,
    b: Option<u8>,
}

#[test]
fn an_agreement_failure_points_at_the_caller() {
    assert_reported_here(|| {
        agreement::compare::<Row>("row", "<html>");
    });
    assert_reported_here(|| {
        agreement::compare::<Row>("row", r#"{"b": 1}"#);
    });
    assert_reported_here(|| {
        agreement::assert_values_agree("row", "", &json!([1]), &json!([2]), Arrays::Zip)
    });
    assert_reported_here(|| {
        dotted::check(
            &json!({"x": 1}),
            &json!({}),
            "row",
            &[] as &[(&str, &str)],
            &[],
        )
    });
}

#[test]
fn a_schema_failure_points_at_the_caller() {
    let schemas = openapi::schemas(&json!({"components": {"schemas": {
        "Row": {"type": "object", "required": ["a"], "properties": {
            "a": {"type": "integer"},
            "b": {"type": "string"}
        }}
    }}}));
    // The full object sends `b` as a string, which `Row` cannot decode.
    assert_reported_here(|| openapi::check::<Row>(&schemas, "Row", &[]));
    assert_reported_here(|| {
        openapi::synth(&schemas, &json!({"type": "array"}), true);
    });
    assert_reported_here(|| {
        openapi::synth(&schemas, &json!({"type": "tuple"}), true);
    });
}

#[cfg(feature = "query")]
#[test]
fn a_nameless_documented_parameter_points_at_the_caller() {
    let spec = json!({"paths": {"/x": {"get": {"parameters": [{}]}}}});
    assert_reported_here(|| {
        polyoxide_test_support::query::documented_parameters(&spec, "/x");
    });
}
