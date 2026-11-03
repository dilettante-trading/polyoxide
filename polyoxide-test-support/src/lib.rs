//! # polyoxide-test-support
//!
//! What every live test shares: a failure that tells the nightly run what kind
//! of failure it is, and credentials that load the same way in every suite.
//!
//! A failing test prints one line to stderr just before it panics:
//!
//! ```text
//! polyoxide-class=transient
//! ```
//!
//! That line is all `.github/scripts/classify_failures.py` reads, and a failure
//! that prints none is filed as a fault.
//!
//! - [`ResultExt::or_fail`] unwraps a `Result` whose error implements
//!   [`Classify`](polyoxide_venue::Classify). On an error it prints the tag
//!   [`tag_for`] gives the error's class and fault flag, then panics as
//!   `expect` does. [`fail`] does the same for an error already in hand, such
//!   as one in a match arm or a source chain.
//! - [`ClassifiedResultExt::or_fail`] and [`fail_classified`] do the same for
//!   the [`ClassifiedError`](polyoxide_venue::ClassifiedError) a venue trait
//!   returns, which does not implement `Classify`.
//! - [`environmental`] and [`transient`] fail a test for a reason no error value
//!   carries: the world has nothing to test today, or a stream ended without
//!   saying why.
//! - [`load_env`], [`keychain`] and [`optional_env`] read credentials, and count
//!   an empty value as absent, since an unset repository secret reaches the
//!   nightly run as `""`. [`Missing::or_auth_gated`] fails a test whose
//!   credentials are absent with the `auth-gated` tag.
//!
//! | Tag | Printed for | Nightly action |
//! | --- | --- | --- |
//! | `auth-gated` | [`Missing::or_auth_gated`] | skipped silently |
//! | `environmental` | `Class::Restricted` that is not a fault, [`environmental`] | logged and skipped |
//! | `transient` | `Class::Network`, `Unavailable` and `RateLimited`, [`transient`] | retried twice, then filed as `real` if it still fails |
//! | `real` | `Class::Restricted` that is a fault, and every other class | filed as an issue |
//!
//! ```
//! use polyoxide_test_support::ResultExt;
//! use polyoxide_venue::{Class, Classify};
//!
//! #[derive(Debug)]
//! struct Offline;
//!
//! impl std::fmt::Display for Offline {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         f.write_str("offline")
//!     }
//! }
//!
//! impl std::error::Error for Offline {}
//!
//! impl Classify for Offline {
//!     fn class(&self) -> Class {
//!         Class::Network
//!     }
//! }
//!
//! let price: Result<u32, Offline> = Ok(7);
//! assert_eq!(price.or_fail("read the price"), 7);
//! ```
//!
//! The same call on `Err(Offline)` prints `polyoxide-class=transient`, then
//! panics with `read the price: Offline`, the message `expect` would give.
//!
//! The crate names no venue and depends on no venue crate. A test passes the
//! environment variable names its target declares as `secrets`, and builds its
//! own client from the strings the loaders return.
//!
//! # The agreement, fixture and soak helpers
//!
//! The offline suites that hold a type to its wire and its schema share their
//! machinery here too, each module the superset of the copies it replaced. A
//! suite keeps a thin local function with its old name, which states its own
//! allow-lists and venue facts and calls in:
//!
//! - [`fixtures!`] and [`Fixtures`] read captured payloads from the calling
//!   crate's `tests/fixtures/`.
//! - [`agreement`] compares a decoded and re-encoded payload with the wire:
//!   key paths both ways, scalar values, the allow-lists and their stale
//!   entries ([`agreement::Ledger`]), and the dotted-path walker.
//! - [`openapi`] synthesises values from a vendored OpenAPI schema and holds a
//!   type to it.
//! - `query` (feature `query`) reads the query keys a builder sends off a mock
//!   server, for comparison with the documented parameters.
//! - [`minute`] keeps a test or a probe clear of a UTC minute boundary.
//! - [`registration!`] and [`Registration`] read the ids a crate declares in
//!   its `[package.metadata.polyoxide]`, for the test that holds the crate's
//!   id constants to them.
//! - `soak` (feature `soak`) is the rate-limit harnesses' pacer, arguments,
//!   throttle observer and verdict rulebooks.
//!
//! Their failures panic with no tag, so the nightly classifier files them as
//! `real`: a wire or schema disagreement is a fault.
//!
//! # In a browser
//!
//! On `wasm32-unknown-unknown` only the tags build: [`fail`], [`environmental`],
//! [`transient`], [`tag_for`] and the extension traits. The loaders, the fixture,
//! agreement and soak helpers read files, the keychain and native clocks, and are
//! absent. The tag line goes to `console.error`, since std's stderr prints
//! nothing there and wasm-bindgen-test captures the console into the failing
//! test's output, where `scripts/wasm_live.sh` reads it.

#![warn(missing_docs)]

#[cfg(not(target_arch = "wasm32"))]
pub mod agreement;
#[cfg(not(target_arch = "wasm32"))]
mod creds;
#[cfg(not(target_arch = "wasm32"))]
mod fixtures;
#[cfg(not(target_arch = "wasm32"))]
pub mod minute;
#[cfg(not(target_arch = "wasm32"))]
pub mod openapi;
#[cfg(all(feature = "query", not(target_arch = "wasm32")))]
pub mod query;
#[cfg(not(target_arch = "wasm32"))]
mod registration;
#[cfg(all(feature = "soak", not(target_arch = "wasm32")))]
pub mod soak;
mod tag;

#[cfg(not(target_arch = "wasm32"))]
pub use creds::{keychain, load_env, optional_env, Creds, Missing};
#[cfg(not(target_arch = "wasm32"))]
pub use fixtures::Fixtures;
#[cfg(not(target_arch = "wasm32"))]
pub use registration::Registration;
pub use tag::{
    environmental, fail, fail_classified, tag_for, transient, ClassifiedResultExt, ResultExt, Tag,
};
