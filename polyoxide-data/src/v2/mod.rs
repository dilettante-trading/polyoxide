//! Data API v2: the `/v2/*` routes on `data-api.polymarket.com`.

mod envelope;
pub mod error;
pub mod types;

pub use envelope::{Page, PageStream, Pagination};
pub use error::{ErrorCode, V2Error};
