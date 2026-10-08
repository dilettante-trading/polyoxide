//! Rust client for the Polymarket Perps HTTP API (`api.perpetuals.polymarket.com`).
//!
//! This crate covers the public `/v1/info/*` routes, which need no credentials,
//! and, with the `ws` feature, the six public WebSocket channels (`bbo`,
//! `book`, `trades`, `klines`, `tickers`, `statistics`) through the `ws`
//! module.
//!
//! ```no_run
//! use polyoxide_perps::Perps;
//!
//! # async fn example() -> Result<(), polyoxide_perps::PerpsError> {
//! let perps = Perps::new()?;
//! let latency = perps.health().ping().await?;
//! println!("perps host answered in {latency:?}");
//! # Ok(())
//! # }
//! ```

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod api;
pub mod client;
pub mod error;
pub mod types;
#[cfg(feature = "ws")]
pub mod ws;

pub use client::{Perps, PerpsBuilder, DEFAULT_BASE_URL, DEFAULT_MAX_CONCURRENT};
pub use error::{PerpsError, VenueError};

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<PerpsError>();
    is::<VenueError>();
    is::<types::UnknownVariant>();
    #[cfg(feature = "ws")]
    is::<ws::PerpsWsError>();
};
