//! # polyoxide-kalshi
//!
//! Rust client library for Kalshi's APIs, one feature per module: `predictions`.
//!
//! The crate is a walking skeleton and it is not published. It is the workspace's third venue,
//! built by the architecture guide's recipe to show that a venue lands by editing its own files
//! and the few places the recipe names.
//!
//! `predictions` (feature `predictions`, on by default) reads Kalshi's Predictions exchange
//! (`external-api.kalshi.com`, and the demo host `external-api.demo.kalshi.co`). Public reads need
//! no credentials. The module's own docs carry its example.

// The crate README is compiled as a doctest, so a code fence that is ever added
// to it cannot rot. `doctest` keeps this out of normal builds and `cargo doc`.
#[cfg(all(doctest, feature = "predictions"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// Each module's README is a doctest under its own feature, as the crate's is.
// `.github/scripts/tests/test_module_readmes.py` fails when a module README
// has no include here, or an include names no README.
#[cfg(all(doctest, feature = "predictions"))]
#[doc = include_str!("predictions/README.md")]
struct PredictionsReadmeDoctests;

#[cfg(feature = "predictions")]
pub mod predictions;
