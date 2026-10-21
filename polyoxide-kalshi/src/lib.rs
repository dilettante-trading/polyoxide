//! # polyoxide-kalshi
//!
//! Rust client library for Kalshi's APIs, one feature per module: `predictions` (event-contract
//! market data, with its streams under `predictions-ws`).
//!
//! It is the workspace's third venue, built by the architecture guide's recipe, and the umbrella
//! crate `polyoxide` re-exports it as `polyoxide::kalshi`, under its `kalshi`,
//! `kalshi-predictions` and `kalshi-predictions-ws` features.
//!
//! `predictions` (feature `predictions`, on by default) reads Kalshi's Predictions exchange
//! (`external-api.kalshi.com`, and the demo host `external-api.demo.kalshi.co`). Public REST reads
//! need no credentials. It implements `polyoxide-venue`'s `MarketData`, `Trades`, `Candles` and
//! `EventGroups`, its candles at one minute, one hour and one day. The module's own docs carry its
//! example.
//!
//! `predictions-ws` (feature `predictions-ws`, off by default, and it turns on `predictions`) adds
//! the module's sockets, `predictions::ws`, on the socket kit: the order-book channel, a snapshot
//! of each subscribed market's book and then its changes in YES pricing, and the `ticker` and
//! `trade` channels, chosen with `PredictionsWsBuilder::channels`. A local book, `Books`, folds the
//! order-book events into each market's ladders. The stream reconnects after a drop or a stall,
//! and repairs a gap in the order-book subscription's sequence in place with a fresh snapshot,
//! falling back to a reconnect. Kalshi refuses a socket handshake that carries no API-key
//! headers, public data included, so the socket needs `Credentials` where the REST reads of
//! public data need none.

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

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
#[cfg(feature = "predictions")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<predictions::PredictionsError>();
    is::<predictions::VenueError>();
    is::<predictions::key::InvalidTicker>();
    is::<predictions::CredentialsError>();
    #[cfg(feature = "predictions-ws")]
    is::<predictions::ws::PredictionsWsError>();
};
