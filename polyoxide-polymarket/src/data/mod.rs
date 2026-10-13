//! # Data
//!
//! Client for Polymarket's Data API, v1 and v2, behind the `data` feature.
//!
//! ## Features
//!
//! - User data: open/closed positions, position value, trades, activity, and markets-traded counts
//! - Market positions (`/v1/market-positions`) and token holders
//! - Combinatorial (multi-market) positions and their lifecycle activity
//! - Trade history across users
//! - Leaderboards, builder stats, open interest, and live volume
//! - Accounting snapshots (returned as ZIP bytes)
//! - Data API v2 (`/v2/*`) via [`DataApi::v2`], with cursor paging and structured errors
//! - Type-safe API with idiomatic Rust patterns
//! - Request builder pattern for flexible, composable queries
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide_polymarket::data::DataApi;
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

pub mod api;
pub mod client;
pub mod error;
pub mod types;
pub mod v2;

pub use client::{DataApi, DataApiBuilder};
pub use error::DataError;
