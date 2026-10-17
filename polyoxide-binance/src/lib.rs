//! # polyoxide-binance
//!
//! Rust client library for Binance's APIs, one feature per module: `usdm`.
//!
//! `usdm` (feature `usdm`, on by default) reads USDⓈ-M futures public market
//! data (`fapi.binance.com`), and with `usdm-ws` its market streams
//! (`fstream.binance.com`). No credentials are needed. The module's own docs
//! carry its example.

// Compile the crate README's `rust` code fences as doctests so broken examples
// fail CI. `doctest` keeps this out of normal builds and `cargo doc`.
#[cfg(all(doctest, feature = "usdm"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// Each module's README is a doctest under its own feature, as the crate's is.
// `.github/scripts/tests/test_module_readmes.py` fails when a module README
// has no include here, or an include names no README.
#[cfg(all(doctest, feature = "usdm"))]
#[doc = include_str!("usdm/README.md")]
struct UsdmReadmeDoctests;

#[cfg(feature = "usdm")]
pub mod usdm;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
#[cfg(feature = "usdm")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<usdm::UsdmError>();
    is::<usdm::types::InvalidSymbol>();
    #[cfg(feature = "usdm-ws")]
    is::<usdm::ws::UsdmWsError>();
    #[cfg(feature = "usdm-ws")]
    is::<usdm::ws::InvalidStreamName>();
};
