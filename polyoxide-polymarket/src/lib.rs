//! # polyoxide-polymarket
//!
//! Rust client library for Polymarket's APIs, one feature per module: `clob`,
//! `gamma`, `data`, `relay` and `perps`.
//!
//! Two modules are here: `gamma` (market data, feature `gamma`) and `data`
//! (user data, v1 and v2, feature `data`). The rest arrive in Stories 5.4 and
//! 5.5. What the modules share is in [`shared`] (built by any module's
//! feature): Polymarket's retry policy, its window-quota tables, the CLOB's
//! composed throttle and per-signer layer, the HMAC signer and the
//! session-signer types.

// Compile the crate README's `rust` code fences as doctests so broken examples
// fail CI. `doctest` keeps this out of normal builds and `cargo doc`, and its
// example needs Gamma's table.
#[cfg(all(doctest, feature = "gamma"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// Each module's README is a doctest under its own feature, as the crate's is.
// `.github/scripts/tests/test_module_readmes.py` fails when a module README
// has no include here, or an include names no README.
#[cfg(all(doctest, feature = "gamma"))]
#[doc = include_str!("gamma/README.md")]
struct GammaReadmeDoctests;

#[cfg(all(doctest, feature = "data"))]
#[doc = include_str!("data/README.md")]
struct DataReadmeDoctests;

#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "relay",
    feature = "perps"
))]
pub mod shared;

#[cfg(feature = "data")]
pub mod data;
#[cfg(feature = "gamma")]
pub mod gamma;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
#[cfg(feature = "clob")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<shared::BurstCapacityExceeded>();
};

#[cfg(feature = "gamma")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<gamma::GammaError>();
};

#[cfg(feature = "data")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<data::DataError>();
    is::<data::v2::V2Error>();
};
