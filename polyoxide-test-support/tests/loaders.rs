//! The environment loaders, run in child processes whose environment the
//! parent sets, so no test changes its own.

mod common;

use std::path::PathBuf;

use common::{is_child, run_child, run_child_in};
use polyoxide_test_support::{load_env, optional_env};

const A: &str = "POLYOXIDE_TEST_SUPPORT_A";
const B: &str = "POLYOXIDE_TEST_SUPPORT_B";
const VALUE_A: &str = "value-of-a-never-printed";
const VALUE_B: &str = "value-of-b-never-printed";

// --- the children ------------------------------------------------------------

#[test]
#[ignore = "a child process of the tests below"]
fn child_load_env() {
    if is_child() {
        let creds = load_env(&[A, B]).unwrap_or_else(|missing| missing.or_auth_gated());
        assert_eq!(creds.get(A), VALUE_A);
        assert_eq!(creds.get(B), VALUE_B);
        eprintln!("loaded {creds:?}");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_optional_env() {
    if is_child() {
        eprintln!("optional: {:?}", optional_env(A));
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_dotenv_stays_out_of_the_environment() {
    if is_child() {
        load_env(&[A, B]).unwrap_or_else(|missing| missing.or_auth_gated());
        assert_eq!(optional_env(B).as_deref(), Some(VALUE_B));
        assert!(std::env::var_os(A).is_none(), "{A} reached the environment");
        assert!(std::env::var_os(B).is_none(), "{B} reached the environment");
    }
}

/// A fresh directory under cargo's per-target scratch space holding a `.env`
/// file of `lines`, for a child to run in.
fn with_dotenv(test: &str, lines: &[&str]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("dotenv-{test}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    std::fs::write(dir.join(".env"), lines.join("\n") + "\n").expect("a .env file");
    dir
}

// --- the tests ---------------------------------------------------------------

#[test]
fn every_variable_set_loads_without_a_tag() {
    let child = run_child("child_load_env", &[(A, VALUE_A), (B, VALUE_B)], &[]);
    assert!(child.passed, "{}", child.stderr);
    assert!(child.tags().is_empty(), "{}", child.stderr);
    let loaded = child.lines()[child.line_of("loaded")];
    assert!(loaded.contains(A) && loaded.contains(B), "{loaded}");
    assert!(
        !child.stderr.contains(VALUE_A),
        "Debug printed a value: {loaded}"
    );
}

#[test]
fn an_empty_variable_is_absent_and_auth_gated() {
    let child = run_child("child_load_env", &[(A, VALUE_A), (B, "")], &[]);
    assert!(!child.passed);
    assert_eq!(
        child.tags(),
        ["polyoxide-class=auth-gated"],
        "{}",
        child.stderr
    );
    assert_eq!(
        child.first_message(),
        format!("credentials not configured: {B} absent or empty in the environment")
    );
}

#[test]
fn one_unset_variable_loads_nothing_and_names_only_itself() {
    let child = run_child("child_load_env", &[(B, VALUE_B)], &[A]);
    assert!(!child.passed);
    assert_eq!(
        child.tags(),
        ["polyoxide-class=auth-gated"],
        "{}",
        child.stderr
    );
    let message = child.first_message();
    assert!(message.contains(A), "{message}");
    assert!(!message.contains(B), "{B} was set: {message}");
    assert!(
        !child.stderr.contains(VALUE_B),
        "a value was printed:\n{}",
        child.stderr
    );
}

#[test]
fn every_variable_unset_names_them_all() {
    let child = run_child("child_load_env", &[], &[A, B]);
    assert_eq!(
        child.tags(),
        ["polyoxide-class=auth-gated"],
        "{}",
        child.stderr
    );
    assert_eq!(
        child.first_message(),
        format!("credentials not configured: {A}, {B} absent or empty in the environment")
    );
}

#[test]
fn optional_env_counts_empty_as_unset() {
    let cases = [
        (Some("x"), "optional: Some(\"x\")"),
        (Some(""), "optional: None"),
        (None, "optional: None"),
    ];
    for (value, printed) in cases {
        let child = match value {
            Some(value) => run_child("child_optional_env", &[(A, value)], &[]),
            None => run_child("child_optional_env", &[], &[A]),
        };
        assert!(child.passed, "{}", child.stderr);
        assert!(child.tags().is_empty(), "{}", child.stderr);
        assert!(
            child.stderr.contains(printed),
            "{value:?}:\n{}",
            child.stderr
        );
    }
}

#[test]
fn a_dotenv_file_fills_what_the_environment_lacks_and_a_bad_line_skips_only_itself() {
    let dir = with_dotenv(
        "bad-line",
        &[
            "# a comment",
            &format!("{A}={VALUE_A}"),
            "THIS LINE DOES NOT PARSE",
            &format!("{B}={VALUE_B}"),
        ],
    );
    let child = run_child_in(Some(&dir), "child_load_env", &[], &[A, B]);
    assert!(child.passed, "{}", child.stderr);
    let warning = format!(
        "warning: {}:3 does not parse, so the variable it sets is not loaded",
        dir.join(".env").display()
    );
    assert_eq!(
        child
            .lines()
            .iter()
            .filter(|line| line.starts_with("warning:"))
            .collect::<Vec<_>>(),
        [&warning.as_str()],
        "{}",
        child.stderr
    );
    for text in ["THIS LINE", VALUE_A, VALUE_B] {
        assert!(
            !child.stderr.contains(text),
            "printed {text:?}:\n{}",
            child.stderr
        );
    }
}

#[test]
fn the_environment_wins_over_a_dotenv_file() {
    let dir = with_dotenv(
        "environment-wins",
        &[&format!("{A}=from-the-file"), &format!("{B}={VALUE_B}")],
    );
    let child = run_child_in(Some(&dir), "child_load_env", &[(A, VALUE_A)], &[B]);
    assert!(child.passed, "{}", child.stderr);
}

#[test]
fn an_empty_variable_is_not_filled_from_a_dotenv_file() {
    let dir = with_dotenv(
        "empty-wins",
        &[&format!("{A}={VALUE_A}"), &format!("{B}={VALUE_B}")],
    );
    let child = run_child_in(Some(&dir), "child_load_env", &[(B, "")], &[A]);
    assert_eq!(
        child.tags(),
        ["polyoxide-class=auth-gated"],
        "{}",
        child.stderr
    );
}

#[test]
fn a_dotenv_file_never_reaches_the_process_environment() {
    let dir = with_dotenv(
        "no-set-var",
        &[&format!("{A}={VALUE_A}"), &format!("{B}={VALUE_B}")],
    );
    let child = run_child_in(
        Some(&dir),
        "child_dotenv_stays_out_of_the_environment",
        &[],
        &[A, B],
    );
    assert!(child.passed, "{}", child.stderr);
    assert!(child.tags().is_empty(), "{}", child.stderr);
}
