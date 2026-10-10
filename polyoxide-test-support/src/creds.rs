//! The credential loaders.
//!
//! Each source is all or nothing: a loader returns every credential it was
//! asked for, or names each one it could not find. A value that is present but
//! empty counts as absent, because the nightly run passes an unset repository
//! secret as `""`, and a half-configured set would otherwise reach a client
//! constructor and fail in words the classifier files as a fault.

use std::collections::BTreeMap;
use std::env::{self, VarError};
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::OnceLock;

use polyoxide_venue::Secret;

use crate::tag::{raise, Tag};

/// Credentials a loader found, by the environment variable name the test asked
/// for. `Debug` shows the names and never the values.
#[derive(Debug, Clone)]
pub struct Creds {
    values: BTreeMap<String, Secret<String>>,
}

impl Creds {
    /// The value loaded for `name`, which is never empty.
    ///
    /// # Panics
    ///
    /// When `name` was not passed to the loader, which is a mistake in the
    /// test.
    #[track_caller]
    pub fn get(&self, name: &str) -> &str {
        match self.values.get(name) {
            Some(value) => value.expose(),
            None => {
                panic!("{name} was not loaded; pass it to the loader that built these credentials")
            }
        }
    }
}

/// The credentials a loader could not find. It names them, and never holds a
/// value: the ones it found are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    source: Source,
    names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    Environment,
    Keychain(String),
}

impl Missing {
    /// What was absent or empty: environment variable names for [`load_env`],
    /// and keychain keys for [`keychain`].
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Fails the test as `auth-gated`, which the nightly run skips silently:
    /// prints `polyoxide-class=auth-gated`, then panics naming what was
    /// missing.
    #[track_caller]
    pub fn or_auth_gated(self) -> ! {
        raise(
            Tag::AuthGated,
            format_args!("credentials not configured: {self}"),
        )
    }
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} absent or empty in ", self.names.join(", "))?;
        match &self.source {
            Source::Environment => f.write_str("the environment"),
            Source::Keychain(service) => write!(f, "keychain service `{service}`"),
        }
    }
}

/// Reads every variable in `names` from the environment, or, for one the
/// environment does not set, from a `.env` file in the working directory or a
/// parent if there is one. A variable that is unset in both, empty or not UTF-8
/// is absent.
///
/// Pass the names as string literals. CI reads them from the call, and checks
/// that they equal the secrets the test target declares.
///
/// ```no_run
/// use polyoxide_test_support::load_env;
///
/// let creds = load_env(&["EXAMPLE_API_KEY", "EXAMPLE_API_SECRET"])
///     .unwrap_or_else(|missing| missing.or_auth_gated());
/// let key = creds.get("EXAMPLE_API_KEY");
/// ```
pub fn load_env(names: &[&str]) -> Result<Creds, Missing> {
    collect(
        Source::Environment,
        names.iter().map(|&name| (name, name)),
        env_var,
    )
}

/// One optional variable, for a test that runs either way: `None` when it is
/// unset, empty or not UTF-8. Falls back to a `.env` file, as [`load_env`]
/// does. Pass the name as a string literal.
pub fn optional_env(name: &str) -> Option<String> {
    present(env_var(name))
}

/// Reads credentials from the OS keychain. `entries` pairs the environment
/// variable name each value is filed under in the returned [`Creds`] with the
/// key it is stored under in `service`, so a test reads a value by the same
/// name whichever source supplied it.
///
/// An entry that does not exist, a keychain that cannot be reached, and an
/// empty value are all absent. Pass the variable names as string literals.
///
/// ```no_run
/// use polyoxide_test_support::{keychain, load_env};
///
/// let creds = load_env(&["EXAMPLE_API_KEY"])
///     .or_else(|_| keychain("example-service", &[("EXAMPLE_API_KEY", "api_key")]))
///     .unwrap_or_else(|missing| missing.or_auth_gated());
/// ```
pub fn keychain(service: &str, entries: &[(&str, &str)]) -> Result<Creds, Missing> {
    collect(
        Source::Keychain(service.to_owned()),
        entries.iter().copied(),
        // `NotFound` (no entry) and `Backend` (no keychain on this machine, as
        // on a CI runner) are both absent.
        |key| polyoxide_core::keychain::get(service, key).ok(),
    )
}

/// The credentials for `entries`, each a (name, key) pair whose value
/// `lookup(key)` reads, or every key it could not find.
fn collect<'a>(
    source: Source,
    entries: impl IntoIterator<Item = (&'a str, &'a str)>,
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Creds, Missing> {
    let mut values = BTreeMap::new();
    let mut names = Vec::new();
    for (name, key) in entries {
        match present(lookup(key)) {
            Some(value) => {
                values.insert(name.to_owned(), Secret::new(value));
            }
            None => names.push(key.to_owned()),
        }
    }
    if names.is_empty() {
        Ok(Creds { values })
    } else {
        Err(Missing { source, names })
    }
}

/// `value`, unless it is empty.
fn present(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// `name` from the environment, or from the `.env` file when the environment
/// does not set it at all. A variable set to `""` stays empty, as it would
/// have under `dotenvy::dotenv`, which never overrides one already set.
fn env_var(name: &str) -> Option<String> {
    match env::var(name) {
        Ok(value) => Some(value),
        Err(VarError::NotPresent) => dotenv().get(name).cloned(),
        Err(VarError::NotUnicode(_)) => None,
    }
}

/// The variables of the nearest `.env` file, read once.
///
/// They are kept here rather than written into the process environment:
/// `set_var` while another test thread reads the environment is a data race.
fn dotenv() -> &'static BTreeMap<String, String> {
    static VARS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    VARS.get_or_init(|| {
        let found = env::current_dir().ok().and_then(|dir| {
            dir.ancestors()
                .map(|d| d.join(".env"))
                .find(|p| p.is_file())
        });
        found.map_or_else(BTreeMap::new, |path| read_dotenv(&path))
    })
}

/// The variables in the `.env` file at `path`, the first of each name winning,
/// as with `dotenvy::dotenv`. A line that does not parse is skipped with a
/// warning on stderr naming the file and line, never the line's text, which
/// may hold a secret; the lines after it still load.
fn read_dotenv(path: &Path) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return vars,
        Err(err) => {
            eprintln!(
                "warning: {} cannot be read, so none of its variables load: {err}",
                path.display()
            );
            return vars;
        }
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    for item in dotenvy::from_read_iter(text.as_bytes()) {
        match item {
            Ok((name, value)) => {
                vars.entry(name).or_insert(value);
            }
            Err(dotenvy::Error::LineParse(line, _)) => eprintln!(
                "warning: {} does not parse, so the variable it sets is not loaded",
                line_of(path, text, &line)
            ),
            Err(err) => {
                eprintln!(
                    "warning: {} stopped loading at an error, so its later variables do not load: {err}",
                    path.display()
                );
                break;
            }
        }
    }
    vars
}

/// `path:N`, where line N of `text` starts `line`, or `path` when none does.
fn line_of(path: &Path, text: &str, line: &str) -> String {
    let start = line.lines().next().unwrap_or(line);
    match text.lines().position(|candidate| candidate == start) {
        Some(index) => format!("{}:{}", path.display(), index + 1),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET_VALUE: &str = "hunter2-do-not-print";

    fn store(entries: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let entries: BTreeMap<&str, &str> = entries.iter().copied().collect();
        move |key| entries.get(key).map(|value| value.to_string())
    }

    #[test]
    fn every_value_present_loads_them_all() {
        let creds = collect(
            Source::Environment,
            [("A", "A"), ("B", "B")],
            store(&[("A", "one"), ("B", "two")]),
        )
        .unwrap();
        assert_eq!(creds.get("A"), "one");
        assert_eq!(creds.get("B"), "two");
    }

    #[test]
    fn one_absent_or_empty_value_loads_nothing_and_names_only_the_gaps() {
        let missing = collect(
            Source::Environment,
            [("A", "A"), ("B", "B"), ("C", "C")],
            store(&[("A", SECRET_VALUE), ("B", "")]),
        )
        .unwrap_err();
        assert_eq!(missing.names(), ["B", "C"]);
        assert_eq!(
            missing.to_string(),
            "B, C absent or empty in the environment"
        );
    }

    #[test]
    fn a_keychain_gap_names_its_key_and_service() {
        let missing = collect(
            Source::Keychain("svc".into()),
            [("EXAMPLE_KEY", "api_key"), ("EXAMPLE_SECRET", "api_secret")],
            store(&[("api_key", "k")]),
        )
        .unwrap_err();
        assert_eq!(missing.names(), ["api_secret"]);
        assert_eq!(
            missing.to_string(),
            "api_secret absent or empty in keychain service `svc`"
        );
    }

    #[test]
    fn keychain_values_are_filed_under_their_variable_names() {
        let creds = collect(
            Source::Keychain("svc".into()),
            [("EXAMPLE_KEY", "api_key")],
            store(&[("api_key", "k")]),
        )
        .unwrap();
        assert_eq!(creds.get("EXAMPLE_KEY"), "k");
    }

    #[test]
    fn no_value_is_ever_printed() {
        let creds = collect(
            Source::Environment,
            [("A", "A")],
            store(&[("A", SECRET_VALUE)]),
        )
        .unwrap();
        let missing = collect(
            Source::Environment,
            [("A", "A"), ("B", "B")],
            store(&[("A", SECRET_VALUE)]),
        )
        .unwrap_err();
        let printed = format!("{creds:?} {creds:#?} {missing:?} {missing}");
        assert!(!printed.contains(SECRET_VALUE), "{printed}");
        assert!(
            printed.contains("\"A\""),
            "the names stay visible: {printed}"
        );
    }

    #[test]
    #[should_panic(expected = "Z was not loaded")]
    fn reading_a_name_never_loaded_is_a_mistake_in_the_test() {
        let creds = collect(Source::Environment, [("A", "A")], store(&[("A", "one")])).unwrap();
        creds.get("Z");
    }

    #[test]
    fn empty_is_absent() {
        assert_eq!(present(Some(String::new())), None);
        assert_eq!(present(None), None);
        assert_eq!(present(Some(" ".into())), Some(" ".into()));
    }
}
