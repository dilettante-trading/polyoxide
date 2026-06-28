//! # polyoxide-data
//!
//! Rust client library for Polymarket Data API.
//!
//! ## Features
//!
//! - User position data retrieval with filtering and pagination
//! - Type-safe API with idiomatic Rust patterns
//! - Request builder pattern for flexible, composable queries
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide_data::DataApi;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a new Data API client
//!     let data = DataApi::new()?;
//!
//!     // Get positions for a user with fluent builder pattern
//!     let positions = data.user("0x1234567890123456789012345678901234567890")
//!         .list_positions()
//!         .limit(10)
//!         .send()
//!         .await?;
//!
//!     for position in positions {
//!         println!("Position: {} - size: {}", position.title, position.size);
//!     }
//!
//!     Ok(())
//! }
//! ```

/// Compiles the crate README as doctests so its examples are checked by CI.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod api;
pub mod client;
pub mod error;
pub mod types;

pub use client::{DataApi, DataApiBuilder};
pub use error::DataApiError;
