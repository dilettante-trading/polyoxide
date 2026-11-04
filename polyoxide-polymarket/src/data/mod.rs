//! # Data
//!
//! Client for Polymarket's Data API, the `/v2/*` routes on
//! `data-api.polymarket.com`, behind the `data` feature. Polymarket retired
//! Data API v1 on 2026-11-02, and its `user-pnl-api` and `lb-api` hosts with
//! it, so every route is v2's and a method of [`DataApi`].
//!
//! ## Features
//!
//! - Wallet data: positions across their lifecycle, combo positions, PnL,
//!   stats, volume, portfolio value and approvals
//! - The trade and activity feeds, and combo activity
//! - Market data: holders, live volume, open interest, price history,
//!   resolutions and tokens
//! - Ranked boards: the leaderboard, builders and biggest winners
//!
//! ## The contract
//!
//! - Every response is wrapped in `data`. Routes that do not paginate return
//!   the unwrapped value; paged routes return a [`Page`].
//! - Paging is cursor-only. [`Page::pagination`] carries `next_cursor`, and each
//!   paged builder has `.cursor()` to resume and `.pages()` to walk every page.
//! - Field names are snake_case, and a missing or `null` number means the value
//!   was unavailable, never zero.
//! - Errors carry a stable code, a `retryable` flag and a trace id; see
//!   [`VenueError`].
//!
//! Upstream's contract: `docs/specs/data-v2/openapi.json` in this repository,
//! mirrored from `https://data-api.polymarket.com/v2/openapi.json`.
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

#[macro_use]
mod envelope;
pub mod api;
pub mod client;
pub mod error;
pub mod types;

pub use client::{DataApi, DataApiBuilder};
pub use envelope::{Page, PageStream, Pagination};
pub use error::{DataError, ErrorCode, VenueError};
