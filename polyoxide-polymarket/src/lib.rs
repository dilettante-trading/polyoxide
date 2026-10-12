//! # polyoxide-polymarket
//!
//! Rust client library for Polymarket's APIs, one feature per module: `clob`,
//! `gamma`, `data`, `relay` and `perps`.
//!
//! The modules arrive in Stories 5.3 to 5.5. Today the crate holds what they
//! share, in [`shared`] (built by any module's feature): Polymarket's retry
//! policy, its window-quota tables, the CLOB's composed throttle and
//! per-signer layer, the HMAC signer and the session-signer types.

// Compile the crate README's `rust` code fences as doctests so broken examples
// fail CI. `doctest` keeps this out of normal builds and `cargo doc`, and its
// example needs Gamma's table.
#[cfg(all(doctest, feature = "gamma"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "relay",
    feature = "perps"
))]
pub mod shared;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
#[cfg(feature = "clob")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<shared::BurstCapacityExceeded>();
};
