//! The `{data, pagination}` envelope that paged v2 routes return.

use std::pin::Pin;

use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};

use crate::DataApiError;

/// One page of a cursor-paginated route.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Page<T> {
    /// The page's rows.
    pub data: Vec<T>,
    /// Paging state; follow `next_cursor` until it is `None`.
    pub pagination: Pagination,
}

/// Paging state returned with every page.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Pagination {
    /// Page size this page was served with.
    pub limit: u32,
    /// Running item offset, for display only. The cursor drives the seek, and
    /// there is no total.
    pub offset: u32,
    /// `true` iff another page exists. Exact, not inferred from page fullness.
    pub has_more: bool,
    /// Opaque, signed cursor for the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// A stream of pages that follows `next_cursor` until the last page.
pub type PageStream<T> = Pin<Box<dyn Stream<Item = Result<Page<T>, DataApiError>> + Send>>;
