//! # Gamma
//!
//! Client for Polymarket's Gamma (market data) API, behind the `gamma`
//! feature.
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
//! use polyoxide_polymarket::gamma::Gamma;
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

pub mod api;
pub mod client;
pub mod error;
pub mod types;

pub use client::{Gamma, GammaBuilder};
pub use error::GammaError;
