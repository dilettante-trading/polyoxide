//! # polyoxide-polymarket
//!
//! Rust client library for Polymarket's APIs, one feature per module: `clob`,
//! `gamma`, `data`, `relay`, `perps`, `rtds` and `sports`.
//!
//! Five modules speak HTTP: `clob` (the order book, feature `clob`, with its
//! WebSocket channels under `clob-ws`), `gamma` (market data, feature
//! `gamma`), `data` (user data, v1 and v2, feature `data`), `relay` (gasless
//! transactions, feature `relay`) and `perps` (perpetual futures market data,
//! feature `perps`, with its WebSocket channels under `perps-ws`). Two are
//! sockets alone: `rtds` (crypto price streams, feature `rtds`) and `sports`
//! (live sports scores, feature `sports`), which build neither core's HTTP
//! client nor the signing stack. `clob`, `gamma` and `data` are on by default.
//! Only `clob` and `relay` build the signing stack, `alloy`. What the HTTP
//! modules share is in `shared` (built by any of their features):
//! Polymarket's retry policy, its window-quota tables, the CLOB's composed
//! throttle and per-signer layer, the HMAC signer and the session-signer types.

// Compile the crate README's `rust` code fences as doctests so broken examples
// fail CI. `doctest` keeps this out of normal builds and `cargo doc`, and its
// example needs Gamma's table.
#[cfg(all(doctest, feature = "gamma"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// Each module's README is a doctest under its own feature, as the crate's is.
// `.github/scripts/tests/test_module_readmes.py` fails when a module README
// has no include here, or an include names no README.
#[cfg(all(doctest, feature = "clob"))]
#[doc = include_str!("clob/README.md")]
struct ClobReadmeDoctests;

#[cfg(all(doctest, feature = "gamma"))]
#[doc = include_str!("gamma/README.md")]
struct GammaReadmeDoctests;

#[cfg(all(doctest, feature = "data"))]
#[doc = include_str!("data/README.md")]
struct DataReadmeDoctests;

#[cfg(all(doctest, feature = "relay"))]
#[doc = include_str!("relay/README.md")]
struct RelayReadmeDoctests;

#[cfg(all(doctest, feature = "perps"))]
#[doc = include_str!("perps/README.md")]
struct PerpsReadmeDoctests;

#[cfg(all(doctest, feature = "sports"))]
#[doc = include_str!("sports/README.md")]
struct SportsReadmeDoctests;

#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "relay",
    feature = "perps"
))]
pub mod shared;

#[cfg(feature = "clob")]
pub mod clob;
#[cfg(feature = "data")]
pub mod data;
#[cfg(feature = "gamma")]
pub mod gamma;
#[cfg(feature = "perps")]
pub mod perps;
#[cfg(feature = "relay")]
pub mod relay;
#[cfg(feature = "rtds")]
pub mod rtds;
#[cfg(feature = "sports")]
pub mod sports;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
#[cfg(feature = "clob")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<shared::BurstCapacityExceeded>();
    is::<clob::ClobError>();
    is::<clob::ParseTickSizeError>();
    #[cfg(feature = "clob-ws")]
    is::<clob::ws::ClobWsError>();
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

#[cfg(feature = "relay")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<relay::RelayError>();
};

#[cfg(feature = "perps")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<perps::PerpsError>();
    is::<perps::VenueError>();
    #[cfg(feature = "perps-ws")]
    is::<perps::ws::PerpsWsError>();
};

#[cfg(feature = "rtds")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<rtds::RtdsError>();
};

#[cfg(feature = "sports")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<sports::SportsError>();
};
