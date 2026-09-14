//! Data API v2: the `/v2/*` routes on `data-api.polymarket.com`.
//!
//! Reach it with [`DataApi::v2`](crate::DataApi::v2). It shares that client's
//! connection pool, rate limiter, 429 cooldown and concurrency budget, since
//! v1 and v2 are served by the same host.
//!
//! v2 differs from v1 in contract, not only in paths:
//!
//! - Every response is wrapped in `data`. Routes that do not paginate return
//!   the unwrapped value; paged routes return a [`Page`].
//! - Paging is cursor-only. [`Page::pagination`] carries `next_cursor`, and each
//!   paged builder has `.cursor()` to resume and `.pages()` to walk every page.
//! - Field names are snake_case, and a missing or `null` number means the value
//!   was unavailable, never zero.
//! - Errors carry a stable code, a `retryable` flag and a trace id; see
//!   [`V2Error`].
//!
//! Upstream's contract: `docs/specs/data-v2/openapi.json` in this repository,
//! mirrored from `https://data-api.polymarket.com/v2/openapi.json`.

use polyoxide_core::HttpClient;

#[macro_use]
mod envelope;
pub mod api;
pub mod error;
pub mod types;

pub use envelope::{Page, PageStream, Pagination};
pub use error::{ErrorCode, V2Error};

/// Handle for the Data API v2 routes. Obtain one with
/// [`DataApi::v2`](crate::DataApi::v2); it is cheap to clone.
#[derive(Clone)]
pub struct DataV2 {
    pub(crate) http_client: HttpClient,
}
