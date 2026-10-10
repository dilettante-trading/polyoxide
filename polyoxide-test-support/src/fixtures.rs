//! Captured payloads, read from the `tests/fixtures/` of the crate that asks.
//!
//! [`fixtures!`](crate::fixtures!) has to be a macro: `env!("CARGO_MANIFEST_DIR")`
//! written in this crate would name this crate's directory, so the macro
//! expands it in the calling crate instead.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// A directory of `<name>.json` fixture files.
///
/// Build one with [`fixtures!`](crate::fixtures!), or with [`Fixtures::at`]
/// for a directory outside the calling crate's `tests/fixtures/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixtures {
    dir: PathBuf,
}

impl Fixtures {
    /// The fixtures in `dir`.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The directory the fixtures are read from.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The path of the fixture `name`: `<dir>/<name>.json`.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    /// The fixture `name`, as the bytes on disk read as text. Panics with the
    /// path and the I/O error when it cannot be read.
    #[track_caller]
    pub fn text(&self, name: &str) -> String {
        // A `match`, not a closure: `#[track_caller]` does not reach into one.
        let path = self.path(name);
        match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => panic!("{}: {e}", path.display()),
        }
    }

    /// The fixture `name`, parsed as JSON. Panics with the path when it cannot
    /// be read or is not JSON.
    #[track_caller]
    pub fn json(&self, name: &str) -> Value {
        let text = self.text(name);
        match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => panic!("{}: not JSON: {e}", self.path(name).display()),
        }
    }
}

/// The calling crate's fixtures: `tests/fixtures/` under its manifest
/// directory, or the subdirectory named.
///
/// ```
/// use polyoxide_test_support::fixtures;
///
/// let all = fixtures!();
/// let captured = fixtures!("ws");
/// assert_eq!(captured.dir(), all.dir().join("ws"));
/// assert!(all.dir().ends_with("polyoxide-test-support/tests/fixtures"));
/// ```
#[macro_export]
macro_rules! fixtures {
    () => {
        $crate::Fixtures::at(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
    };
    ($sub:literal) => {
        $crate::Fixtures::at(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/",
            $sub
        ))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str, files: &[(&str, &str)]) -> Fixtures {
        let dir = std::env::temp_dir().join(format!(
            "polyoxide-test-support-fixtures-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for (file, body) in files {
            std::fs::write(dir.join(file), body).unwrap();
        }
        Fixtures::at(dir)
    }

    #[test]
    fn a_fixture_is_read_by_name_as_text_and_as_json() {
        let fixtures = scratch("read", &[("time.json", "{\"serverTime\": 1}\n")]);
        assert_eq!(fixtures.text("time"), "{\"serverTime\": 1}\n");
        assert_eq!(fixtures.json("time"), serde_json::json!({"serverTime": 1}));
        assert_eq!(fixtures.path("time"), fixtures.dir().join("time.json"));
    }

    #[test]
    fn a_missing_fixture_names_its_path() {
        let fixtures = scratch("missing", &[]);
        let path = fixtures.path("absent").display().to_string();
        let panic = std::panic::catch_unwind(|| fixtures.text("absent")).unwrap_err();
        let message = panic.downcast_ref::<String>().unwrap();
        assert!(message.starts_with(&format!("{path}: ")), "{message}");
    }

    #[test]
    fn a_fixture_that_is_not_json_names_its_path() {
        let fixtures = scratch("not-json", &[("page.json", "<html>")]);
        let path = fixtures.path("page").display().to_string();
        let panic = std::panic::catch_unwind(|| fixtures.json("page")).unwrap_err();
        let message = panic.downcast_ref::<String>().unwrap();
        assert!(
            message.starts_with(&format!("{path}: not JSON: ")),
            "{message}"
        );
    }

    #[test]
    fn the_macro_resolves_in_the_crate_that_expands_it() {
        // Expanded here, so the manifest directory is this crate's.
        let root = crate::fixtures!();
        assert_eq!(
            root.dir(),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
        );
        assert_eq!(crate::fixtures!("rest").dir(), root.dir().join("rest"));
    }
}
