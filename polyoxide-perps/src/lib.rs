//! Rust client for the Polymarket Perps HTTP API (`api.perpetuals.polymarket.com`).
//!
//! This crate covers the public `/v1/info/*` routes, which need no credentials.
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

pub use client::{Perps, PerpsBuilder, DEFAULT_BASE_URL};
pub use error::{PerpsError, VenueError};
