//! The `{data}` and `{data, pagination}` envelopes every v2 response uses, and
//! the cursor walk over paged routes.

use std::pin::Pin;

use futures_util::stream::{self, Stream};
use polyoxide_core::{QueryBuilder, Request};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::DataApiError;

/// `{ "data": T }` — the body of every route that does not paginate.
#[derive(Deserialize)]
pub(crate) struct Envelope<T> {
    pub(crate) data: T,
}

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

/// Adds `cursor`, `send` and `pages` to a builder whose paged request is held
/// in a field named `inner: Paged<$row>`.
macro_rules! paged_builder_methods {
    ($row:ty) => {
        /// Resume from a previous page's `next_cursor`. Upstream ignores `limit`
        /// when a cursor is sent, because the cursor carries its own page size.
        pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
            self.inner = self.inner.cursor(cursor.into());
            self
        }

        /// Fetch one page.
        pub async fn send(self) -> Result<$crate::v2::Page<$row>, $crate::DataApiError> {
            self.inner.send().await
        }

        /// Walk every page, re-sending this builder's filters on each one.
        pub fn pages(self) -> $crate::v2::PageStream<$row> {
            self.inner.pages()
        }
    };
}

/// A stream of pages that follows `next_cursor` until the last page.
pub type PageStream<T> = Pin<Box<dyn Stream<Item = Result<Page<T>, DataApiError>> + Send>>;

/// A paged request whose cursor is held apart from its filters.
///
/// Every page of a walk is cloned from the same `request`, so a walk cannot
/// change its filters between pages. On `/v2/trades` and `/v2/activity` a
/// changed filter re-anchors the walk silently, and on `/v2/positions` the
/// `title`, `condition` and window filters are not carried by the cursor at all.
pub(crate) struct Paged<T> {
    request: Request<Page<T>, DataApiError>,
    cursor: Option<String>,
}

impl<T> Paged<T> {
    pub(crate) fn new(request: Request<Page<T>, DataApiError>) -> Self {
        Self {
            request,
            cursor: None,
        }
    }

    pub(crate) fn query(mut self, key: &str, value: impl ToString) -> Self {
        self.request = self.request.query(key, value);
        self
    }

    pub(crate) fn cursor(mut self, cursor: String) -> Self {
        self.cursor = Some(cursor);
        self
    }
}

impl<T: DeserializeOwned + Send + 'static> Paged<T> {
    pub(crate) async fn send(self) -> Result<Page<T>, DataApiError> {
        with_cursor(self.request, self.cursor).send().await
    }

    pub(crate) fn pages(self) -> PageStream<T> {
        let start = Some((self.request, self.cursor));
        Box::pin(stream::try_unfold(start, |walk| async move {
            let Some((request, cursor)) = walk else {
                return Ok(None);
            };
            let page = with_cursor(request.clone(), cursor.clone()).send().await?;
            let next = match page.pagination.next_cursor.clone() {
                None => None,
                Some(next) if cursor.as_deref() == Some(next.as_str()) => {
                    return Err(DataApiError::Pagination(format!(
                        "server returned the cursor it was sent: {next}"
                    )));
                }
                Some(next) => Some((request, Some(next))),
            };
            Ok(Some((page, next)))
        }))
    }
}

fn with_cursor<T>(
    request: Request<T, DataApiError>,
    cursor: Option<String>,
) -> Request<T, DataApiError> {
    match cursor {
        Some(cursor) => request.query("cursor", cursor),
        None => request,
    }
}

/// Comma-joins a multi-value parameter; `None` when there are no values, so
/// the parameter is omitted rather than sent empty.
pub(crate) fn csv<I, S>(values: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    let joined = values
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join(",");
    (!joined.is_empty()).then_some(joined)
}
