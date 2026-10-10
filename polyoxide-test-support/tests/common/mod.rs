//! Runs one `#[ignore]`d test of this binary in a child process, as nextest runs
//! every test, and reads what it printed.
//!
//! A child test does its work only when [`CHILD`] is set, which the parent sets
//! on the child alone, so `cargo test -- --ignored` passes over the children
//! instead of failing them. Every environment variable a child reads is set on
//! its command too, never in the test process itself.

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

/// Set on a child's command, never in a test process.
pub const CHILD: &str = "POLYOXIDE_TEST_SUPPORT_CHILD";

/// What a child printed, and whether it passed.
pub struct Child {
    pub stderr: String,
    pub passed: bool,
}

impl Child {
    /// The lines of stderr.
    pub fn lines(&self) -> Vec<&str> {
        self.stderr.lines().collect()
    }

    /// The tag lines, in order.
    pub fn tags(&self) -> Vec<&str> {
        self.stderr
            .lines()
            .filter(|line| line.contains("polyoxide-class="))
            .collect()
    }

    /// The index of the first line containing `needle`.
    pub fn line_of(&self, needle: &str) -> usize {
        self.lines()
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no line contains {needle:?}:\n{}", self.stderr))
    }

    /// The first panic's message, the line after its `panicked at` line.
    pub fn first_message(&self) -> &str {
        let lines = self.lines();
        lines[self.line_of("panicked at") + 1]
    }
}

/// Whether this process is a child its parent test started.
pub fn is_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

/// Runs the ignored test `name` in a child process. `set` holds the variables
/// to set on it, and `unset` the ones to remove from what it inherits.
pub fn run_child(name: &str, set: &[(&str, &str)], unset: &[&str]) -> Child {
    run_child_in(None, name, set, unset)
}

/// [`run_child`], with the child's working directory `dir` when given.
pub fn run_child_in(dir: Option<&Path>, name: &str, set: &[(&str, &str)], unset: &[&str]) -> Child {
    let exe = std::env::current_exe().expect("the test binary's path");
    let mut command = Command::new(exe);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command
        .args(["--exact", name, "--ignored", "--nocapture"])
        .env(CHILD, "1")
        // A backtrace would only add lines between the ones these tests read.
        .env("RUST_BACKTRACE", "0");
    for name in unset {
        command.env_remove(name);
    }
    for (name, value) in set {
        command.env(name, value);
    }
    let output = command.output().expect("the child runs");
    Child {
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
        passed: output.status.success(),
    }
}
