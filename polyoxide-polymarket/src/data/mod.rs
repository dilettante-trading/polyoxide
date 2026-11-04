//! # Data
//!
//! Client for Polymarket's Data API, behind the `data` feature. Polymarket
//! retired Data API v1 on 2026-11-02, so every route is v2's, through
//! [`DataApi::v2`].
//!
//! ## Features
//!
//! - Wallet data: positions across their lifecycle, combo positions, PnL,
//!   stats, volume, portfolio value and approvals
//! - The trade and activity feeds, and combo activity
//! - Market data: holders, live volume, open interest, price history,
//!   resolutions and tokens
//! - Ranked boards: the leaderboard, builders and biggest winners
//! - Cursor paging and structured errors
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide_polymarket::data::DataApi;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let data = DataApi::new()?;
//!
//!     let page = data
//!         .v2()
//!         .positions("0x1234567890123456789012345678901234567890")
//!         .limit(10)
//!         .send()
//!         .await?;
//!
//!     for position in page.data {
//!         println!("Position: {} - size: {}", position.title, position.current_size);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod v2;

pub use client::{DataApi, DataApiBuilder};
pub use error::DataError;
