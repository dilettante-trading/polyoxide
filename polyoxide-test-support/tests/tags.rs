//! The tag line, read from the stderr of a child process that fails the way a
//! live test would.

mod common;

use std::fmt;
use std::panic;

use common::{is_child, run_child};
use polyoxide_test_support::{environmental, fail, tag_for, transient, ResultExt, Tag};
use polyoxide_venue::{Class, Classify};

/// The class a child's error reports, set on the child's command.
const CLASS: &str = "POLYOXIDE_TEST_SUPPORT_CLASS";
/// Set to `false` on a child's command for an error that is not a fault.
const FAULT: &str = "POLYOXIDE_TEST_SUPPORT_FAULT";

/// Every class, by the name a child is given, with whether the error is a
/// fault and the tag it must print. Only `Restricted` reads the flag: a region
/// block is environmental, and a ban the client earned is real.
const CLASSES: [(&str, bool, &str); 11] = [
    ("network", true, "transient"),
    ("network", false, "transient"),
    ("unavailable", true, "transient"),
    ("rate-limited", true, "transient"),
    ("restricted", false, "environmental"),
    ("restricted", true, "real"),
    ("unauthorized", true, "real"),
    ("invalid-request", true, "real"),
    ("refusal", true, "real"),
    // A defined outcome, like a Fill-And-Kill order that matched nothing.
    ("refusal", false, "real"),
    ("decode", true, "real"),
];

fn class_named(name: &str) -> Class {
    match name {
        "network" => Class::Network,
        "unavailable" => Class::Unavailable { code: None },
        "rate-limited" => Class::RateLimited { retry_after: None },
        "restricted" => Class::Restricted,
        "unauthorized" => Class::Unauthorized,
        "invalid-request" => Class::InvalidRequest,
        "refusal" => Class::VenueRefusal { code: None },
        "decode" => Class::Decode,
        other => panic!("no class is named {other}"),
    }
}

/// An error of whatever class and fault flag it is given.
#[derive(Debug)]
struct Failed(Class, bool);

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed as {:?}", self.0)
    }
}

impl std::error::Error for Failed {}

impl Classify for Failed {
    fn class(&self) -> Class {
        self.0.clone()
    }

    fn is_fault(&self) -> bool {
        self.1
    }
}

/// The error the parent describes on the child's command.
fn failed() -> Result<(), Failed> {
    let class = std::env::var(CLASS).map_or(Class::Network, |name| class_named(&name));
    let fault = std::env::var(FAULT).map_or(true, |fault| fault != "false");
    Err(Failed(class, fault))
}

/// The variables that describe `class` and `fault` to a child.
fn described(class: &'static str, fault: bool) -> [(&'static str, &'static str); 2] {
    [
        (CLASS, class),
        (FAULT, if fault { "true" } else { "false" }),
    ]
}

// --- the children ------------------------------------------------------------

#[test]
#[ignore = "a child process of the tests below"]
fn child_or_fail() {
    if is_child() {
        failed().or_fail("read the price");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_fail() {
    if is_child() {
        // Borrowed, and unsized behind `dyn`, as an error taken from a source
        // chain would be.
        let err: Box<dyn Classify + Send + Sync> = Box::new(failed().unwrap_err());
        fail("read the price", &*err);
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_expect() {
    if is_child() {
        failed().expect("read the price");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_environmental() {
    if is_child() {
        environmental("nothing is live anywhere right now");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_transient() {
    if is_child() {
        transient("the server ended the stream without a close code");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_after_a_hook_of_its_own() {
    if is_child() {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            eprintln!("the earlier hook ran");
            default(info);
        }));
        transient("the stream ended");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_two_entry_points() {
    if is_child() {
        // The first entry point installs the hook.
        let caught = panic::catch_unwind(|| {
            environmental("first");
        });
        assert!(caught.is_err());
        // A hook set now runs ahead of the installed one. Were the second entry
        // point to install again, its hook would wrap this one and print the
        // tag before this line.
        let installed = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            eprintln!("set between the entry points");
            installed(info);
        }));
        failed().or_fail("second");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_caught_then_untagged() {
    if is_child() {
        let caught = panic::catch_unwind(|| {
            transient("first");
        });
        assert!(caught.is_err());
        panic!("an untagged panic");
    }
}

// --- the tests ---------------------------------------------------------------

#[test]
fn every_class_maps_to_its_tag() {
    for (name, fault, tag) in CLASSES {
        assert_eq!(
            tag_for(&class_named(name), fault).as_str(),
            tag,
            "{name}, fault {fault}"
        );
    }
    assert_eq!(Tag::AuthGated.as_str(), "auth-gated");
}

#[test]
fn or_fail_prints_every_class_as_its_tag() {
    for (name, fault, tag) in CLASSES {
        let child = run_child("child_or_fail", &described(name, fault), &[]);
        assert!(
            !child.passed,
            "{name}, fault {fault}: the child did not fail"
        );
        assert_eq!(
            child.tags(),
            [format!("polyoxide-class={tag}")],
            "{name}, fault {fault}:\n{}",
            child.stderr
        );
    }
}

#[test]
fn fail_prints_every_class_as_its_tag() {
    for (name, fault, tag) in CLASSES {
        let child = run_child("child_fail", &described(name, fault), &[]);
        assert!(
            !child.passed,
            "{name}, fault {fault}: the child did not fail"
        );
        assert_eq!(
            child.tags(),
            [format!("polyoxide-class={tag}")],
            "{name}, fault {fault}:\n{}",
            child.stderr
        );
    }
}

#[test]
fn fail_panics_as_or_fail_does_at_the_callers_line() {
    let held = run_child("child_fail", &described("restricted", false), &[]);
    let unwrapped = run_child("child_or_fail", &described("restricted", false), &[]);
    assert_eq!(
        held.first_message(),
        "read the price: Failed(Restricted, false)"
    );
    assert_eq!(held.first_message(), unwrapped.first_message());
    let tag = held.line_of("polyoxide-class=");
    assert!(tag < held.line_of("panicked at"), "{}", held.stderr);
    let report = held.lines()[held.line_of("panicked at")];
    assert!(report.contains("tests/tags.rs:"), "{report}");
}

#[test]
fn exactly_one_tag_line_comes_before_the_panic_report() {
    let child = run_child("child_or_fail", &described("unavailable", true), &[]);
    let lines = child.lines();
    let tag = child.line_of("polyoxide-class=");
    assert_eq!(lines[tag], "polyoxide-class=transient", "alone on its line");
    assert_eq!(child.tags().len(), 1, "{}", child.stderr);
    assert!(tag < child.line_of("panicked at"), "{}", child.stderr);
}

#[test]
fn or_fail_panics_with_expects_message_at_the_callers_line() {
    let tagged = run_child("child_or_fail", &described("decode", true), &[]);
    let plain = run_child("child_expect", &described("decode", true), &[]);
    assert_eq!(
        tagged.first_message(),
        "read the price: Failed(Decode, true)"
    );
    assert_eq!(tagged.first_message(), plain.first_message());
    assert!(
        plain.tags().is_empty(),
        "expect prints no tag:\n{}",
        plain.stderr
    );
    let report = tagged.lines()[tagged.line_of("panicked at")];
    assert!(report.contains("tests/tags.rs:"), "{report}");
}

#[test]
fn environmental_and_transient_print_their_own_tags() {
    let child = run_child("child_environmental", &[], &[]);
    assert_eq!(
        child.tags(),
        ["polyoxide-class=environmental"],
        "{}",
        child.stderr
    );
    assert_eq!(child.first_message(), "nothing is live anywhere right now");

    let child = run_child("child_transient", &[], &[]);
    assert_eq!(
        child.tags(),
        ["polyoxide-class=transient"],
        "{}",
        child.stderr
    );
    assert_eq!(
        child.first_message(),
        "the server ended the stream without a close code"
    );
    let report = child.lines()[child.line_of("panicked at")];
    assert!(report.contains("tests/tags.rs:"), "{report}");
}

#[test]
fn a_hook_set_beforehand_still_runs_after_the_tag() {
    let child = run_child("child_after_a_hook_of_its_own", &[], &[]);
    let tag = child.line_of("polyoxide-class=transient");
    let earlier = child.line_of("the earlier hook ran");
    assert!(tag < earlier, "{}", child.stderr);
    assert!(earlier < child.line_of("panicked at"), "{}", child.stderr);
}

#[test]
fn two_entry_points_install_one_hook() {
    let child = run_child("child_two_entry_points", &[], &[]);
    assert_eq!(
        child.tags(),
        ["polyoxide-class=environmental", "polyoxide-class=transient"],
        "{}",
        child.stderr
    );
    let between = child.line_of("set between the entry points");
    let second = child.line_of("polyoxide-class=transient");
    assert!(
        between < second,
        "the second entry point installed a hook of its own:\n{}",
        child.stderr
    );
}

#[test]
fn a_caught_tag_is_never_reused() {
    let child = run_child("child_caught_then_untagged", &[], &[]);
    // One tag, ahead of the first report, so the second panic printed none.
    assert_eq!(
        child.tags(),
        ["polyoxide-class=transient"],
        "{}",
        child.stderr
    );
    assert!(child.line_of("polyoxide-class=") < child.line_of("panicked at"));
    assert!(
        child.stderr.contains("an untagged panic"),
        "{}",
        child.stderr
    );
}

#[test]
fn a_child_does_nothing_unless_its_parent_started_it() {
    // What `cargo test -- --ignored` sees: every child passes, untagged.
    let exe = std::env::current_exe().expect("the test binary's path");
    let output = std::process::Command::new(exe)
        .args(["child_", "--ignored", "--nocapture"])
        .env_remove(common::CHILD)
        .output()
        .expect("the children run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(!stderr.contains("polyoxide-class="), "{stderr}");
}
