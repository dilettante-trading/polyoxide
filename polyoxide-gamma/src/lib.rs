//! # polyoxide-gamma
//!
//! Rust client library for Polymarket Gamma (market data) API.
//!
//! ## Features
//!
//! - Market data retrieval with filtering and pagination
//! - Event and series (tournament/season) information
//! - Tags and sports metadata
//! - Comments on markets, events, and series
//! - Type-safe API with idiomatic Rust patterns
//! - Request builder pattern for flexible, composable queries
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide_gamma::Gamma;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a new Gamma client
//!     let gamma = Gamma::new()?;
//!
//!     // List active markets with fluent builder pattern
//!     let markets = gamma.markets()
//!         .list()
//!         .open(true)
//!         .limit(10)
//!         .send()
//!         .await?;
//!
//!     for market in markets {
//!         println!("Market: {}", market.question);
//!     }
//!
//!     // Get a specific market by its numeric market id
//!     let market = gamma.markets()
//!         .get("123456")
//!         .send()
//!         .await?;
//!
//!     Ok(())
//! }
//! ```

/// Compiles the crate README's ```rust code blocks as doctests so broken
/// examples fail CI. Only present during doctest builds; never affects
/// normal compilation or `cargo doc` output.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod api;
pub mod client;
pub mod error;
pub mod types;

pub use client::{Gamma, GammaBuilder};
pub use error::GammaError;
