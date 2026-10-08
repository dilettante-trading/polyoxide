//! The tag line, and the panic hook that prints it.
//!
//! A panic hook is handed only the panic's payload and location, never the
//! error that caused it. It cannot work out a tag for itself, and reading one
//! back out of the panic text would be the guesswork the tag replaces. So every
//! entry point here stores its tag in a thread-local, then panics. The hook
//! runs on the panicking thread, finds the tag that thread stored, prints it,
//! clears it so a later panic on the thread cannot reuse it, and calls the hook
//! that was installed before it, which prints the usual `panicked at` report.
//!
//! The first entry point a process reaches installs the hook, through a
//! [`Once`]. cargo nextest runs every test in a process of its own, so a hook
//! installed once per test binary, by whichever test ran first, would be
//! missing from every other test's process. The hook wraps the one set before
//! it rather than replacing it. A hook set after it replaces it in turn, unless
//! that hook calls the one it took.

use std::cell::Cell;
use std::fmt;
use std::panic;
use std::sync::Once;

use polyoxide_venue::{Class, Classify};

/// The prefix of the tag line, which is this followed by the tag and nothing
/// else.
const PREFIX: &str = "polyoxide-class=";

/// What a failing live test tells the nightly run about its failure. Each tag
/// has one nightly action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    /// The test's credentials are absent or empty. Skipped silently.
    AuthGated,
    /// The world, not the code, is why the test could not run. Logged and
    /// skipped.
    Environmental,
    /// Retrying could succeed. Retried twice, and filed as `real` if it still
    /// fails.
    Transient,
    /// A fault. Filed as an issue.
    Real,
}

impl Tag {
    /// The tag as the nightly classifier reads it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthGated => "auth-gated",
            Self::Environmental => "environmental",
            Self::Transient => "transient",
            Self::Real => "real",
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The tag an error fails a test with, read from its class and whether it is a
/// fault ([`Classify::is_fault`]).
///
/// [`Network`](Class::Network), [`Unavailable`](Class::Unavailable) and
/// [`RateLimited`](Class::RateLimited) are transient. A
/// [`Restricted`](Class::Restricted) error is environmental when it is not a
/// fault, such as a venue that does not serve the runner's region, and real
/// when it is, such as an IP ban the client earned by overspending its budget:
/// skipping that would lose the one alarm for the bug. Every other class, and
/// any class added later, is real, fault or not.
///
/// ```
/// use polyoxide_test_support::{tag_for, Tag};
/// use polyoxide_venue::Class;
///
/// assert_eq!(tag_for(&Class::RateLimited { retry_after: None }, true), Tag::Transient);
/// assert_eq!(tag_for(&Class::Restricted, false), Tag::Environmental);
/// assert_eq!(tag_for(&Class::Restricted, true), Tag::Real);
/// assert_eq!(tag_for(&Class::Decode, true), Tag::Real);
/// ```
pub fn tag_for(class: &Class, is_fault: bool) -> Tag {
    match class {
        Class::Network | Class::Unavailable { .. } | Class::RateLimited { .. } => Tag::Transient,
        Class::Restricted if !is_fault => Tag::Environmental,
        _ => Tag::Real,
    }
}

/// Unwraps a `Result` in a live test, tagging the failure by its error.
pub trait ResultExt<T> {
    /// The `Ok` value. On an `Err`, fails the test as [`fail`] does: prints
    /// `polyoxide-class=<tag>` to stderr, with the tag [`tag_for`] gives the
    /// error, then panics with `"{ctx}: {err:?}"`, the message `expect(ctx)`
    /// gives. The panic is reported at the caller's line.
    fn or_fail(self, ctx: &str) -> T;
}

impl<T, E: Classify> ResultExt<T> for Result<T, E> {
    #[track_caller]
    fn or_fail(self, ctx: &str) -> T {
        match self {
            Ok(value) => value,
            Err(err) => fail(ctx, &err),
        }
    }
}

/// Fails the test with `err`, for an error a test holds rather than a `Result`
/// to unwrap: a match arm, a borrowed error, or one taken from an error's
/// source chain. Prints `polyoxide-class=<tag>` to stderr, with the tag
/// [`tag_for`] gives the error's class and fault flag, then panics with
/// `"{ctx}: {err:?}"`, as [`ResultExt::or_fail`] does. The panic is reported at
/// the caller's line.
///
/// `E` may be unsized, so a `&dyn Classify` will do. Its `Debug` comes with
/// [`Classify`], whose supertrait is `Error`.
#[track_caller]
pub fn fail<E: Classify + fmt::Debug + ?Sized>(ctx: &str, err: &E) -> ! {
    raise(
        tag_for(&err.class(), err.is_fault()),
        format_args!("{ctx}: {err:?}"),
    )
}

/// Fails the test because the world, not the code, is why it cannot run, such
/// as a venue that does not serve the runner's region or a feed with nothing
/// live. Prints `polyoxide-class=environmental`, then panics with `reason`.
#[track_caller]
pub fn environmental(reason: &str) -> ! {
    raise(Tag::Environmental, format_args!("{reason}"))
}

/// Fails the test for a reason a retry may not share, such as a stream that
/// ended without a close code. Prints `polyoxide-class=transient`, then panics
/// with `reason`.
#[track_caller]
pub fn transient(reason: &str) -> ! {
    raise(Tag::Transient, format_args!("{reason}"))
}

thread_local! {
    /// The tag of the panic this thread is about to raise.
    static PENDING: Cell<Option<Tag>> = const { Cell::new(None) };
}

static HOOK: Once = Once::new();

/// Stores `tag` for the hook, then panics with `message` at the caller's line.
#[track_caller]
pub(crate) fn raise(tag: Tag, message: fmt::Arguments<'_>) -> ! {
    install_hook();
    PENDING.set(Some(tag));
    panic!("{message}");
}

fn install_hook() {
    HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            // `try_with`, since a hook that panicked would abort the process.
            if let Some(tag) = PENDING.try_with(Cell::take).ok().flatten() {
                // `eprintln!` rather than a raw stderr write, so the line joins
                // the test's captured output, ahead of the report below.
                eprintln!("{PREFIX}{tag}");
            }
            previous(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tag_reads_as_the_classifier_spells_it() {
        let tags = [
            (Tag::AuthGated, "auth-gated"),
            (Tag::Environmental, "environmental"),
            (Tag::Transient, "transient"),
            (Tag::Real, "real"),
        ];
        for (tag, spelled) in tags {
            assert_eq!(tag.as_str(), spelled);
            assert_eq!(tag.to_string(), spelled);
        }
    }
}
