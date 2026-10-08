//! # polyoxide-test-support
//!
//! What every live test shares: a failure that tells the nightly run what kind
//! of failure it is, and credentials that load the same way in every suite.
//!
//! A failing test prints one line to stderr just before it panics, which
//! `.github/scripts/classify_failures.py` reads before anything else:
//!
//! ```text
//! polyoxide-class=transient
//! ```
//!
//! - [`ResultExt::or_fail`] unwraps a `Result` whose error implements
//!   [`Classify`](polyoxide_venue::Classify). On an error it prints the tag
//!   [`tag_for`] gives the error's class and fault flag, then panics as
//!   `expect` does. [`fail`] does the same for an error already in hand, such
//!   as one in a match arm or a source chain.
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

#![warn(missing_docs)]

mod creds;
mod tag;

pub use creds::{keychain, load_env, optional_env, Creds, Missing};
pub use tag::{environmental, fail, tag_for, transient, ResultExt, Tag};
