//! Cursor paging and output for the Data API v2 commands.
//!
//! A single call prints the whole `{data, pagination}` envelope, so
//! `pagination.next_cursor` is visible and a walk can be resumed with
//! `--cursor`. `--all` walks every page and writes one JSON row per line,
//! flushing after each page: the bare trade feed spans a month of every trade,
//! so collecting it into one array could exhaust memory, and a walk that fails
//! part-way has already written the rows before the failure. When a walk stops
//! before the end, the cursor to resume from goes to stderr.

use std::{future::Future, io::Write};

use clap::Args;
use color_eyre::eyre::Result;
use futures_util::{Stream, StreamExt};
use polyoxide_data::{
    v2::{
        api::{
            boards::ListBuildersLeaderboard,
            feeds::{ListActivity, ListTrades},
            markets::ListHolders,
            wallet::ListPositions,
        },
        types::{Activity, BuilderStanding, MetaHolder, Position, Trade},
        Page, PageStream,
    },
    DataApiError,
};
use serde::Serialize;

/// Paging flags shared by every paged `data` command.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PageArgs {
    /// Resume from a previous page's `pagination.next_cursor`
    #[arg(long)]
    pub cursor: Option<String>,
    /// Walk every page, writing one JSON row per line (JSONL)
    #[arg(long)]
    pub all: bool,
    /// With --all, stop after this many pages and print the cursor to resume from
    #[arg(long, requires = "all")]
    pub max_pages: Option<u64>,
    /// Removed: Data API v2 pages by cursor
    #[arg(short = 'o', long, hide = true, value_parser = removed_offset)]
    pub offset: Option<String>,
}

/// Refuses `--offset` with a pointer to its replacement. Data API v2 rejects
/// an `offset` parameter with a 400.
pub fn removed_offset(_: &str) -> Result<String, String> {
    Err("--offset was removed: the Data API v2 pages by cursor. \
         Pass --cursor <next_cursor> from a previous page, or --all"
        .to_owned())
}

/// A v2 request builder that can be resumed, sent once, or walked.
pub trait PagedRequest: Sized {
    type Row: Serialize + Send + 'static;

    fn with_cursor(self, cursor: String) -> Self;
    fn send_page(self) -> impl Future<Output = Result<Page<Self::Row>, DataApiError>> + Send;
    fn into_pages(self) -> PageStream<Self::Row>;
}

macro_rules! paged_request {
    ($builder:ty, $row:ty) => {
        impl PagedRequest for $builder {
            type Row = $row;

            fn with_cursor(self, cursor: String) -> Self {
                self.cursor(cursor)
            }

            fn send_page(self) -> impl Future<Output = Result<Page<$row>, DataApiError>> + Send {
                self.send()
            }

            fn into_pages(self) -> PageStream<$row> {
                self.pages()
            }
        }
    };
}

paged_request!(ListTrades, Trade);
paged_request!(ListActivity, Activity);
paged_request!(ListPositions, Position);
paged_request!(ListHolders, MetaHolder);
paged_request!(ListBuildersLeaderboard, BuilderStanding);

/// Runs a paged request as the flags ask: one page printed whole, or every
/// page streamed as JSONL.
pub async fn run_paged<R: PagedRequest>(
    request: R,
    page: &PageArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()> {
    let request = match page.cursor.clone() {
        Some(cursor) => request.with_cursor(cursor),
        None => request,
    };
    if page.all {
        walk_rows(
            request.into_pages(),
            page.max_pages,
            page.cursor.clone(),
            out,
            err,
        )
        .await
    } else {
        print_pretty(&request.send_page().await?, out)
    }
}

/// Prints a value as pretty JSON followed by a newline.
pub fn print_pretty<T: Serialize>(value: &T, out: &mut dyn Write) -> Result<()> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)?;
    Ok(())
}

/// Writes every row of every page as one JSON line, flushing after each page.
///
/// Stops after `max_pages` pages if given. Whenever the walk ends before the
/// last page, whether at `max_pages` or on an error, the cursor that resumes
/// it is written to `err` as `next_cursor: <cursor>`. `start` is the cursor the
/// walk began from, so a failure on the very first page can be resumed too.
pub async fn walk_rows<T, S>(
    mut pages: S,
    max_pages: Option<u64>,
    start: Option<String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    T: Serialize,
    S: Stream<Item = Result<Page<T>, DataApiError>> + Unpin,
{
    let mut resume = start;
    let mut walked = 0u64;
    while let Some(page) = pages.next().await {
        let page = match page {
            Ok(page) => page,
            Err(error) => {
                if let Some(cursor) = &resume {
                    writeln!(err, "next_cursor: {cursor}")?;
                }
                return Err(error.into());
            }
        };
        for row in &page.data {
            serde_json::to_writer(&mut *out, row)?;
            writeln!(out)?;
        }
        out.flush()?;
        walked += 1;
        resume = page.pagination.next_cursor;
        if max_pages.is_some_and(|max| walked >= max) {
            if let Some(cursor) = &resume {
                writeln!(err, "next_cursor: {cursor}")?;
            }
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use futures_util::stream;
    use serde_json::{json, Value};

    use super::*;

    fn page(rows: &[i64], next: Option<&str>) -> Result<Page<Value>, DataApiError> {
        Ok(serde_json::from_value(json!({
            "data": rows.iter().map(|n| json!({ "n": n })).collect::<Vec<_>>(),
            "pagination": { "limit": 2, "offset": 0, "has_more": next.is_some(), "next_cursor": next },
        }))
        .unwrap())
    }

    fn failure() -> Result<Page<Value>, DataApiError> {
        Err(DataApiError::Pagination("boom".into()))
    }

    async fn walk(
        pages: Vec<Result<Page<Value>, DataApiError>>,
        max_pages: Option<u64>,
        start: Option<&str>,
    ) -> (Result<()>, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let result = walk_rows(
            stream::iter(pages),
            max_pages,
            start.map(str::to_owned),
            &mut out,
            &mut err,
        )
        .await;
        (
            result,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[tokio::test]
    async fn a_walk_writes_one_json_row_per_line_across_pages() {
        let (result, out, err) = walk(
            vec![
                page(&[1, 2], Some("c1")),
                page(&[], Some("c2")),
                page(&[3], None),
            ],
            None,
            None,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(out, "{\"n\":1}\n{\"n\":2}\n{\"n\":3}\n");
        assert_eq!(
            err, "",
            "a walk that reaches the last page has nothing to resume"
        );
    }

    #[tokio::test]
    async fn max_pages_stops_and_prints_the_cursor_to_resume_from() {
        let (result, out, err) = walk(
            vec![
                page(&[1], Some("c1")),
                page(&[2], Some("c2")),
                page(&[3], None),
            ],
            Some(2),
            None,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(out, "{\"n\":1}\n{\"n\":2}\n");
        assert_eq!(err, "next_cursor: c2\n");
    }

    #[tokio::test]
    async fn max_pages_on_the_last_page_prints_no_cursor() {
        let (_, _, err) = walk(vec![page(&[1], None)], Some(1), None).await;
        assert_eq!(err, "");
    }

    #[tokio::test]
    async fn an_error_mid_walk_keeps_the_rows_and_prints_the_failed_pages_cursor() {
        let (result, out, err) = walk(vec![page(&[1], Some("c1")), failure()], None, None).await;

        assert!(result.is_err());
        assert_eq!(
            out, "{\"n\":1}\n",
            "rows before the failure are already written"
        );
        assert_eq!(
            err, "next_cursor: c1\n",
            "resuming from c1 retries the page that failed"
        );
    }

    #[tokio::test]
    async fn an_error_on_the_first_page_resumes_from_the_starting_cursor() {
        let (result, _, err) = walk(vec![failure()], None, Some("c0")).await;
        assert!(result.is_err());
        assert_eq!(err, "next_cursor: c0\n");
    }

    #[test]
    fn offset_is_refused_with_a_pointer_to_cursor() {
        let message = removed_offset("100").unwrap_err();
        assert!(message.contains("--cursor"), "{message}");
    }

    #[test]
    fn a_single_page_prints_the_whole_envelope() {
        let mut out = Vec::new();
        print_pretty(&page(&[1], Some("c1")).unwrap(), &mut out).unwrap();
        let printed: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(printed["pagination"]["next_cursor"], "c1");
        assert_eq!(printed["data"][0]["n"], 1);
    }
}
