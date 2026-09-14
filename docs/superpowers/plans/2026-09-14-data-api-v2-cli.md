# Data API v2 CLI (Phase 5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every `polyoxide data` command except `health` onto Data API v2. Cursor paging (`--cursor`, `--all` as JSONL, `--max-pages`) replaces `--offset`.

**Architecture:** `polyoxide-cli/src/commands/data/paging.rs` gives the five paged v2 builders one trait, `PagedRequest`, so a single `run_paged` implements the paging flags for all of them. One call prints the `{data, pagination}` envelope. `--all` streams rows as JSONL and, when a walk stops early, writes the cursor to resume from to stderr. `DataCommand::run_with(data, out, err)` takes the client and the output writers. That lets the mock tests run real arguments against `mockito`, serving the v2 responses `polyoxide-data` captured, and read what was printed. Commands move over one group per task. `health` stays on v1.

**Tech Stack:** clap 4.5 derive, `futures-util` streams, `mockito` 1.7, `polyoxide_data::v2`. No new dependencies: `futures-util` is already a CLI dependency, so `Cargo.toml` and `Cargo.lock` do not change.

**Design:** Component 6.2 and the Phasing table of [`2026-09-14-data-api-v2-design.md`](../specs/2026-09-14-data-api-v2-design.md). The phase's exit criterion is "CLI unit tests and live CLI tests green".

---

## What planning found

Every task's code was built and run before this plan was written. Each op below was replayed against a fresh copy of this branch: every gate was green, every mutation check was caught, and the final files equal the prototype's. The two live tests in Task 6 were also run once against the live host, and both passed.

**Every v1 list flag is broken.** On this branch, `polyoxide data open-interest --market <id>` panics before sending anything:

```text
Message:  Mismatch between definition and access of `market`. Could not downcast to alloc::string::String, need to downcast to alloc::vec::Vec<alloc::string::String>
```

The same failure hits every comma-separated `--market` and `--event-id` on `trades`, `activity`, `positions`, `holders` and `open-interest`. `parse_comma_separated` returns a `Vec<String>`, but clap's derive treats a `Vec<String>` field as many `String` values, so reading the field back fails. It compiles and parses, and it panics only when the field is accessed. The unit tests only parsed *without* these flags. The port uses `value_delimiter = ','` instead, and the new mock tests pass every list flag through a real run.

**`--taker-only false` was rejected.** A `bool` with `default_value = "true"` is a flag that can only set `true`. It becomes `ArgAction::Set`.

## Where this plan departs from 6.2

| 6.2 says | This plan | Why |
|----------|-----------|-----|
| `positions`: open and closed become `--status OPEN\|REDEEMABLE\|CLOSED` | `positions list --status open\|redeemable\|closed`; the uppercase spellings also parse. `positions closed` stays as a hidden subcommand that fails with `use positions list --status closed` | This CLI's value enums are lowercase. The hidden subcommand gives an old script a pointer instead of `unrecognized subcommand` |
| `traded` prints `trades` (the distinct-market count) | Prints the whole `/v2/user-stats` object (`proxy_wallet`, `trades`, `biggest_win`, `views`, `join_date`, `all_time_pnl`), and `null` when v2 returns `data: null` | Decided 2026-09-14 (decision 3 below). `null` keeps an unknown wallet distinguishable from a known one with zero counts |
| — | v1-only flags are removed: positions `--redeemable`, `--mergeable`, `--size-threshold`; activity `--sort-by`. holders `--min-balance` changes from a default of `1` to the API's `0`, and accepts fractions | v2 has none of these parameters, and sorts activity by timestamp only |
| "Not included: commands for routes that are new in v2" | New flags for v2 parameters on routes the CLI already covers: trades `--start`/`--end`; activity `--include-deposits-withdrawals`; positions `--filter-type`/`--filter-amount`, `--include-archived`, `--start`/`--end`; holders `--include-pnl`; builders volume `--limit` | These are not new routes. Without `--include-deposits-withdrawals`, two of v2's activity types are unreachable, because v2 hides them by default |
| holders "moved to the matching v2 route" | `--condition` is required | v2 answers 400 without it, where v1 accepted none |
| — | `DataCommand::run_with(data, out, err)` | So tests can run commands against a mock server and read the output |
| Exit criterion: "live CLI tests green" | Two new live tests in `polyoxide-cli/tests/live_api.rs` (Task 6) | There were no live tests for `data` commands |

## Decisions (confirmed 2026-09-14)

1. **The output break is accepted.** Every ported command prints v2's `{data, pagination}` envelope with snake_case fields: `proxy_wallet` for `proxyWallet`, and `{data: [...]}` for a bare array. Scripts parsing the old output break.
2. **`positions closed` stays hidden and fails with a pointer** to `positions list --status closed`, rather than aliasing it.
3. **`traded` prints the whole `/v2/user-stats` object** as pretty JSON, and `null` for a wallet v2 does not know. v1's `{user, traded}` shape is not kept. (The first version of this plan kept it; Task 5 was amended.)

## Conventions

- Run everything from the repository root.
- **Keep build output off `/tmp`.** On this machine it is a 16 GB RAM-backed tmpfs. Leave `CARGO_TARGET_DIR` unset, or point it at disk.
- Each mutation check changes one line with `sed`, runs the tests, and reverts in the same block. Before each commit, `git diff --stat` must show only the task's files.
- **Live traffic** comes only from Task 6 step 2 and Task 7's smoke run, about fifteen requests in all.
- Commit messages end with the attribution trailer shown in each commit step. Commits that change a command's flags or output are marked breaking (`feat(cli)!`), so git-cliff flags them in the changelog.

## File map

| File | Responsibility | Task |
|------|----------------|------|
| `polyoxide-cli/src/commands/data/paging.rs` | `PageArgs`, `PagedRequest`, `run_paged`, `print_pretty`, `walk_rows` | 1 |
| `polyoxide-cli/src/commands/data/mod.rs` | `run_with`, dispatch, `SortOrder: PartialEq`, parse tests | 1–5 |
| `polyoxide-cli/src/commands/common/parsing.rs` | `parse_list_entry` replaces `parse_comma_separated`; `parse_activity_types` on v2 | 2–4 |
| `polyoxide-cli/src/commands/data/trades.rs` | `trades list` | 2 |
| `polyoxide-cli/src/commands/data/activity.rs`, `positions.rs` | `activity`, `positions list/value/activity` | 3 |
| `polyoxide-cli/src/commands/data/holders.rs`, `open_interest.rs`, `live_volume.rs` | the three market commands | 4 |
| `polyoxide-cli/src/commands/data/traded.rs`, `builders.rs` | `traded`, `builders leaderboard/volume` | 5 |
| `polyoxide-cli/tests/data_v2.rs` | Mock-server tests: flag → query, output, paging, refusals | 2–5 |
| `polyoxide-cli/tests/live_api.rs` | Live tests for the ported commands | 6 |
| `polyoxide-cli/README.md` | Data API section | 7 |

Not touched: `Cargo.toml`, `Cargo.lock`, `main.rs`, any other crate, `CLAUDE.md`, `CHANGELOG.md`. See **After this plan** for the documentation text that belongs outside `polyoxide-cli/`.

---

## Tasks

### Task 1: Cursor paging and output helpers for `data` commands

**Files:**
- Create: `polyoxide-cli/src/commands/data/paging.rs`
- Modify: `polyoxide-cli/src/commands/data/mod.rs` (declare the module)

- [ ] **Step 1: Create the paging module**

Every paged v2 builder has the same `cursor`, `send` and `pages` methods but no shared trait, so `PagedRequest` gives them one, and `run_paged` implements the paging flags once for all five paged commands:

- **One call** prints the whole `{data, pagination}` envelope, pretty, so `next_cursor` is visible.
- **`--all`** streams rows as JSONL, flushing after each page. The bare trade feed covers a month of every trade, so collecting it into one array could exhaust memory, and rows already written survive a failure part-way.
- **An early stop** (`--max-pages`, or an error) writes `next_cursor: <cursor>` to stderr. On an error that is the cursor of the page that failed, so resuming retries it.
- **`--offset`** stays defined but hidden, with a value parser that always fails. clap then prints the replacement instead of `unexpected argument`.

Output goes to writers passed in, never straight to stdout: the mock tests in later tasks read what a command printed.

Create `polyoxide-cli/src/commands/data/paging.rs`:

```rust
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
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
mod open_interest;
mod positions;
```

with:

```rust
mod open_interest;
#[allow(dead_code)] // Used by the commands ported in the next tasks.
mod paging;
mod positions;
```

- [ ] **Step 2: Run the paging tests**

Run:

```bash
cargo test -p polyoxide-cli --all-features --lib paging
```

Expected: `test result: ok. 7 passed`.

- [ ] **Step 3: Prove the resume cursor is tested**

Make the walk forget each page's cursor, check that the cursor tests fail, then revert:

Run:

```bash
sed -i 's/resume = page.pagination.next_cursor;/resume = None;/' polyoxide-cli/src/commands/data/paging.rs
cargo test -p polyoxide-cli --all-features --lib paging 2>&1 | grep -E '^test .* FAILED'
sed -i 's/resume = None;/resume = page.pagination.next_cursor;/' polyoxide-cli/src/commands/data/paging.rs
```

Expected: `test commands::data::paging::tests::max_pages_stops_and_prints_the_cursor_to_resume_from ... FAILED` among the lines printed.

- [ ] **Step 4: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-cli --all-features --lib
```

Expected: `test result: ok. 228 passed`.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-cli/src/commands/data/mod.rs polyoxide-cli/src/commands/data/paging.rs
git commit -F - <<'EOF'
feat(cli): cursor paging and JSONL output helpers for data commands

Data API v2 pages by cursor only. A shared PagedRequest trait over the
five paged v2 builders lets one run_paged implement --cursor, --all
(JSONL, flushed per page) and --max-pages, printing the cursor to resume
from on stderr whenever a walk stops early. --offset stays as a hidden
flag whose parser always fails, so clap names --cursor instead. Unused
until the commands move over.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 2: Port `data trades` to v2, and route output through writers

**Files:**
- Modify: `polyoxide-cli/src/commands/data/trades.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/mod.rs`
- Modify: `polyoxide-cli/src/commands/common/parsing.rs`
- Create: `polyoxide-cli/tests/data_v2.rs`

- [ ] **Step 1: Background: the two v1 flag bugs this task fixes**

Every comma-separated flag in the v1 `data` commands panics the first time it is given. `parse_comma_separated` returns a `Vec<String>`, but clap's derive treats a `Vec<String>` field as *many* `String` values, so reading the field back fails to downcast. The unit tests only ever parsed without these flags, which is how it went unnoticed. On the branch before this plan:

```text
$ polyoxide data open-interest --market 0x…01
The application panicked (crashed).
Message:  Mismatch between definition and access of `market`. Could not downcast to alloc::string::String, need to downcast to alloc::vec::Vec<alloc::string::String>
```

The ported commands use `value_delimiter = ','` with a per-entry `parse_list_entry`, and the mock tests below pass every list flag through a real run. `--taker-only` had a second bug: a `bool` with `default_value = "true"` is a flag that can only set `true`, so `--taker-only false` was rejected. It becomes `ArgAction::Set`.

- [ ] **Step 2: Add the list-entry parser**

In `polyoxide-cli/src/commands/common/parsing.rs`, replace:

```rust
/// Parse comma-separated values into a Vec of trimmed strings.
```

with:

```rust
/// One entry of a comma-separated list flag, trimmed.
///
/// Pair it with `value_delimiter = ','` on a `Vec<String>` field: clap splits
/// the list and parses each entry. A value parser that returns the whole
/// `Vec` instead does not fit a `Vec` field, and clap panics at runtime the
/// first time the flag is given.
pub fn parse_list_entry(s: &str) -> Result<String, std::convert::Infallible> {
    Ok(s.trim().to_owned())
}

/// Parse comma-separated values into a Vec of trimmed strings.
```

In `polyoxide-cli/src/commands/common/parsing.rs`, replace:

```rust
    // --- parse_comma_separated tests ---
```

with:

```rust
    // --- parse_list_entry tests ---

    #[test]
    fn parse_list_entry_trims_whitespace() {
        assert_eq!(parse_list_entry(" 0xa ").unwrap(), "0xa");
    }

    // --- parse_comma_separated tests ---
```

- [ ] **Step 3: Replace `trades.rs`**

`--condition` replaces `--market`, which stays as a visible alias along with `-m`. `--start` and `--end` are new: v2 honours them on the `--user` shape only, and says so in the help. `TradeSideFilter` and `TradeFilterField` gain `PartialEq`, which `ActivityFilters` needs in Task 3. v1 `activity` and `positions` still map `TradeSideFilter` to v1's `TradeSide`, so that conversion stays for one task.

Replace the entire contents of `polyoxide-cli/src/commands/data/trades.rs`:

```rust
use std::io::Write;

use clap::{ArgAction, Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_data::{
    v2::types::{FilterType, TradeSide},
    DataApi,
};

use super::paging::{run_paged, PageArgs};
use crate::commands::common::parsing::parse_list_entry;

#[derive(Subcommand)]
pub enum TradesCommand {
    /// List trades: a user's, a market's or event's, or the whole feed (`/v2/trades`)
    List {
        /// User address (0x-prefixed, 40 hex chars)
        #[arg(short, long)]
        user: Option<String>,
        /// Filter by market condition IDs (comma-separated, at most 20)
        #[arg(
            short = 'm',
            long = "condition",
            visible_alias = "market",
            value_delimiter = ',',
            value_parser = parse_list_entry
        )]
        condition: Option<Vec<String>>,
        /// Filter by event IDs (comma-separated)
        #[arg(short, long, value_delimiter = ',', value_parser = parse_list_entry)]
        event_id: Option<Vec<String>>,
        /// Filter by trade side
        #[arg(short, long, value_enum)]
        side: Option<TradeSideFilter>,
        /// Only taker trades; pass `--taker-only false` to include maker fills
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        taker_only: bool,
        /// Filter type (must be paired with --filter-amount)
        #[arg(long, value_enum)]
        filter_type: Option<TradeFilterField>,
        /// Filter amount (must be paired with --filter-type)
        #[arg(long)]
        filter_amount: Option<f64>,
        /// Window start, epoch seconds; honoured with --user only (1 for full history)
        #[arg(long)]
        start: Option<i64>,
        /// Window end, epoch seconds; honoured with --user only
        #[arg(long)]
        end: Option<i64>,
        /// Page size (at most 1000)
        #[arg(short, long, default_value = "100")]
        limit: u32,
        #[command(flatten)]
        page: PageArgs,
    },
}

impl TradesCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        match self {
            Self::List {
                user,
                condition,
                event_id,
                side,
                taker_only,
                filter_type,
                filter_amount,
                start,
                end,
                limit,
                page,
            } => {
                let mut request = data.v2().trades().taker_only(taker_only).limit(limit);
                if let Some(user) = user {
                    request = request.user(user);
                }
                if let Some(ids) = condition {
                    request = request.conditions(ids);
                }
                if let Some(ids) = event_id {
                    request = request.event_ids(ids);
                }
                if let Some(side) = side {
                    request = request.side(side.into());
                }
                if let Some(filter_type) = filter_type {
                    request = request.filter_type(filter_type.into());
                }
                if let Some(amount) = filter_amount {
                    request = request.filter_amount(amount);
                }
                if let Some(ts) = start {
                    request = request.start(ts);
                }
                if let Some(ts) = end {
                    request = request.end(ts);
                }
                run_paged(request, &page, out, err).await
            }
        }
    }
}

/// Trade side filter
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum TradeSideFilter {
    /// Buy trades
    Buy,
    /// Sell trades
    Sell,
}

impl From<TradeSideFilter> for TradeSide {
    fn from(side: TradeSideFilter) -> Self {
        match side {
            TradeSideFilter::Buy => Self::Buy,
            TradeSideFilter::Sell => Self::Sell,
        }
    }
}

/// The v1 side, for `activity` and `positions` until they move to v2 in the
/// next task.
impl From<TradeSideFilter> for polyoxide_data::types::TradeSide {
    fn from(side: TradeSideFilter) -> Self {
        match side {
            TradeSideFilter::Buy => Self::Buy,
            TradeSideFilter::Sell => Self::Sell,
        }
    }
}

/// Unit of a filter amount
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum TradeFilterField {
    /// Cash amount (USDC)
    Cash,
    /// Token amount (shares)
    Tokens,
}

impl From<TradeFilterField> for FilterType {
    fn from(filter: TradeFilterField) -> Self {
        match filter {
            TradeFilterField::Cash => Self::Cash,
            TradeFilterField::Tokens => Self::Tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn try_parse(args: &[&str]) -> Result<TradesCommand, clap::Error> {
        #[derive(Parser)]
        struct Wrapper {
            #[command(subcommand)]
            cmd: TradesCommand,
        }
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    #[test]
    fn trade_side_filter_maps_to_the_v2_side() {
        assert_eq!(TradeSide::from(TradeSideFilter::Buy), TradeSide::Buy);
        assert_eq!(TradeSide::from(TradeSideFilter::Sell), TradeSide::Sell);
    }

    #[test]
    fn trade_filter_field_maps_to_the_v2_filter_type() {
        assert_eq!(FilterType::from(TradeFilterField::Cash), FilterType::Cash);
        assert_eq!(
            FilterType::from(TradeFilterField::Tokens),
            FilterType::Tokens
        );
    }

    #[test]
    fn list_defaults() {
        let TradesCommand::List {
            user,
            condition,
            event_id,
            side,
            taker_only,
            filter_type,
            filter_amount,
            start,
            end,
            limit,
            page,
        } = try_parse(&["test", "list"]).unwrap();
        assert!(user.is_none());
        assert!(condition.is_none());
        assert!(event_id.is_none());
        assert!(side.is_none());
        assert!(taker_only);
        assert!(filter_type.is_none());
        assert!(filter_amount.is_none());
        assert!(start.is_none() && end.is_none());
        assert_eq!(limit, 100);
        assert_eq!(page, PageArgs::default());
    }

    #[test]
    fn market_is_an_alias_of_condition() {
        for flag in ["--condition", "--market", "-m"] {
            let TradesCommand::List { condition, .. } =
                try_parse(&["test", "list", flag, "0xa,0xb"]).unwrap();
            assert_eq!(condition.unwrap(), ["0xa", "0xb"], "{flag}");
        }
    }

    #[test]
    fn list_flags_split_on_commas_and_trim_each_entry() {
        let TradesCommand::List {
            condition,
            event_id,
            ..
        } = try_parse(&[
            "test",
            "list",
            "--condition",
            " 0xa , 0xb ",
            "-e",
            "1",
            "-e",
            "2",
        ])
        .unwrap();
        assert_eq!(condition.unwrap(), ["0xa", "0xb"]);
        assert_eq!(event_id.unwrap(), ["1", "2"], "a repeated flag appends");
    }

    #[test]
    fn taker_only_can_be_turned_off() {
        let TradesCommand::List { taker_only, .. } =
            try_parse(&["test", "list", "--taker-only", "false"]).unwrap();
        assert!(!taker_only);
    }

    #[test]
    fn list_invalid_side_errors() {
        assert!(try_parse(&["test", "list", "--side", "short"]).is_err());
    }

    #[test]
    fn list_invalid_filter_type_errors() {
        assert!(try_parse(&["test", "list", "--filter-type", "volume"]).is_err());
    }

    #[test]
    fn offset_is_refused_with_a_pointer_to_cursor() {
        for args in [
            &["test", "list", "--offset", "100"][..],
            &["test", "list", "-o", "100"][..],
        ] {
            let message = try_parse(args)
                .err()
                .expect("offset must be refused")
                .to_string();
            assert!(message.contains("--cursor"), "{message}");
        }
    }

    #[test]
    fn cursor_all_and_max_pages_parse() {
        let TradesCommand::List { page, .. } = try_parse(&[
            "test",
            "list",
            "--cursor",
            "c1",
            "--all",
            "--max-pages",
            "3",
        ])
        .unwrap();
        assert_eq!(page.cursor.as_deref(), Some("c1"));
        assert!(page.all);
        assert_eq!(page.max_pages, Some(3));
    }

    #[test]
    fn max_pages_requires_all() {
        assert!(try_parse(&["test", "list", "--max-pages", "3"]).is_err());
    }
}
```

- [ ] **Step 4: Add `run_with` to `DataCommand`**

`run` keeps its signature for `main.rs`. `run_with` takes the client and the two writers, so tests can point a command at a mock server and read its output. `health` stays on v1 (`/v2/status` reports data freshness, not liveness) and prints through `out`, byte-identical to before. Commands not yet ported keep printing directly.

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
#[allow(dead_code)] // Used by the commands ported in the next tasks.
mod paging;
```

with:

```rust
mod paging;
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
use clap::{Subcommand, ValueEnum};
```

with:

```rust
use std::io::Write;

use clap::{Subcommand, ValueEnum};
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
impl DataCommand {
    pub async fn run(self) -> Result<()> {
        let data = DataApi::new()?;

        match self {
            Self::Health => {
                let health = data.health().check().await?;
                println!("{}", serde_json::to_string_pretty(&health)?);
                Ok(())
            }
            Self::Activity(cmd) => cmd.run(&data).await,
            Self::Builders { command } => command.run(&data).await,
            Self::Holders(cmd) => cmd.run(&data).await,
            Self::Trades { command } => command.run(&data).await,
            Self::Traded(cmd) => cmd.run(&data).await,
            Self::Positions(cmd) => cmd.run(&data).await,
            Self::OpenInterest(cmd) => cmd.run(&data).await,
            Self::LiveVolume(cmd) => cmd.run(&data).await,
        }
    }
}

```

with:

```rust
impl DataCommand {
    pub async fn run(self) -> Result<()> {
        self.run_with(
            &DataApi::new()?,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        )
        .await
    }

    /// Runs the command against `data`, writing results to `out` and resume
    /// cursors to `err`, so tests can use a mock server and read the output.
    pub async fn run_with(
        self,
        data: &DataApi,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<()> {
        match self {
            Self::Health => {
                let health = data.health().check().await?;
                paging::print_pretty(&health, out)
            }
            Self::Activity(cmd) => cmd.run(data).await,
            Self::Builders { command } => command.run(data).await,
            Self::Holders(cmd) => cmd.run(data).await,
            Self::Trades { command } => command.run(data, out, err).await,
            Self::Traded(cmd) => cmd.run(data).await,
            Self::Positions(cmd) => cmd.run(data).await,
            Self::OpenInterest(cmd) => cmd.run(data).await,
            Self::LiveVolume(cmd) => cmd.run(data).await,
        }
    }
}

```

- [ ] **Step 5: Run the unit tests**

Run:

```bash
cargo test -p polyoxide-cli --all-features --lib
```

Expected: `test result: ok. 226 passed`.

- [ ] **Step 6: Write the mock-server tests**

Each test parses real arguments, runs the command against `mockito` serving a response captured by `polyoxide-data` (`polyoxide-data/tests/fixtures/v2/`), and checks the decoded query and the output. Queries compare as sorted pairs, so the builders' parameter order does not matter. Later tasks append a section per command group.

Create `polyoxide-cli/tests/data_v2.rs`:

```rust
//! The `data` commands against a mock Data API v2 server.
//!
//! Each test parses real command-line arguments, runs the command against a
//! mock host serving a response captured from the live API, and checks the
//! query the command sent and what it wrote. Parsing tests alone cannot see a
//! flag that parses but never reaches the request, or one that parses and then
//! panics when clap hands it to the command.

use std::sync::{Arc, Mutex};

use clap::Parser;
use mockito::{Matcher, Server, ServerGuard};
use polyoxide_cli::commands::DataCommand;
use polyoxide_data::DataApi;
use serde_json::Value;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    data: DataCommand,
}

type Pairs = Vec<(String, String)>;

/// A response captured from the live API by `polyoxide-data`.
fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../polyoxide-data/tests/fixtures/v2/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// What a command returned and wrote.
struct Run {
    result: color_eyre::Result<()>,
    out: String,
    err: String,
}

impl Run {
    fn expect_ok(&self) {
        if let Err(error) = &self.result {
            panic!("command failed: {error:?}\nstderr: {}", self.err);
        }
    }

    /// Each stdout line, parsed as JSON.
    fn rows(&self) -> Vec<Value> {
        self.out
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

async fn run(server: &ServerGuard, args: &[&str]) -> Run {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let result = execute(server, args, &mut out, &mut err).await;
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

async fn execute(
    server: &ServerGuard,
    args: &[&str],
    out: &mut Vec<u8>,
    err: &mut Vec<u8>,
) -> color_eyre::Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("data").chain(args.iter().copied()))?;
    let data = DataApi::builder().base_url(server.url()).build()?;
    cli.data.run_with(&data, out, err).await
}

/// Decodes a query string into sorted pairs, so tests do not depend on the
/// order the builder appends parameters in.
fn query_pairs(path_and_query: &str) -> Pairs {
    let Some((_, query)) = path_and_query.split_once('?') else {
        return Vec::new();
    };
    let mut pairs: Pairs = query
        .split('&')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect();
    pairs.sort();
    pairs
}

fn decode(component: &str) -> String {
    let bytes = component.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                out.push(u8::from_str_radix(&component[i + 1..i + 3], 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

fn pairs(expected: &[(&str, &str)]) -> Pairs {
    let mut pairs: Pairs = expected
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    pairs.sort();
    pairs
}

/// Serves `path`, answering each request with `respond(query)` and recording
/// every query in the order received.
async fn serve<F>(server: &mut ServerGuard, path: &str, respond: F) -> Arc<Mutex<Vec<Pairs>>>
where
    F: Fn(&Pairs) -> String + Send + Sync + 'static,
{
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .match_request(move |request| {
            sink.lock()
                .unwrap()
                .push(query_pairs(request.path_and_query()));
            true
        })
        .with_status(200)
        .with_body_from_request(move |request| {
            respond(&query_pairs(request.path_and_query())).into_bytes()
        })
        .create_async()
        .await;
    seen
}

/// Runs `args` against a host serving `body` on `path`, requires success, and
/// returns the query of the single request sent along with the run.
async fn sent(path: &str, body: &str, args: &[&str]) -> (Pairs, Run) {
    let mut server = Server::new_async().await;
    let body = body.to_owned();
    let seen = serve(&mut server, path, move |_| body.clone()).await;
    let run = run(&server, args).await;
    run.expect_ok();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "{args:?} sent {seen:?}");
    (seen[0].clone(), run)
}

/// The captured two-row trades page, with its cursor replaced so a walk can be
/// steered.
fn trades_page(next_cursor: Option<&str>) -> String {
    let mut page: Value = serde_json::from_str(&fixture("trades")).unwrap();
    page["pagination"]["has_more"] = Value::Bool(next_cursor.is_some());
    page["pagination"]["next_cursor"] = next_cursor.map_or(Value::Null, Value::from);
    page.to_string()
}

fn cursor(query: &Pairs) -> Option<String> {
    query
        .iter()
        .find(|(key, _)| key == "cursor")
        .map(|(_, value)| value.clone())
}

fn cursors(seen: &Mutex<Vec<Pairs>>) -> Vec<Option<String>> {
    seen.lock().unwrap().iter().map(cursor).collect()
}

fn some(cursor: &str) -> Option<String> {
    Some(cursor.to_owned())
}

fn without_cursor(query: &Pairs) -> Pairs {
    query
        .iter()
        .filter(|(key, _)| key != "cursor")
        .cloned()
        .collect()
}

/// Serves a three-page trade feed: no cursor, then `c1`, then `c2`.
async fn three_pages(server: &mut ServerGuard) -> Arc<Mutex<Vec<Pairs>>> {
    serve(server, "/v2/trades", |query| {
        match cursor(query).as_deref() {
            None => trades_page(Some("c1")),
            Some("c1") => trades_page(Some("c2")),
            Some("c2") => trades_page(None),
            Some(other) => panic!("unexpected cursor {other}"),
        }
    })
    .await
}

// ── trades ───────────────────────────────────────────────────────────

#[tokio::test]
async fn trades_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/trades",
        &fixture("trades"),
        &[
            "trades",
            "list",
            "--user",
            "0xu",
            "--market",
            "0xa,0xb",
            "--event-id",
            "7,8",
            "--side",
            "sell",
            "--taker-only",
            "false",
            "--filter-type",
            "cash",
            "--filter-amount",
            "5",
            "--start",
            "100",
            "--end",
            "200",
            "--limit",
            "3",
        ],
    )
    .await;

    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("condition", "0xa,0xb"),
            ("event_id", "7,8"),
            ("side", "SELL"),
            ("taker_only", "false"),
            ("filter_type", "CASH"),
            ("filter_amount", "5"),
            ("start", "100"),
            ("end", "200"),
            ("limit", "3"),
        ])
    );
}

#[tokio::test]
async fn trades_defaults_send_no_offset() {
    let (query, _) = sent("/v2/trades", &fixture("trades"), &["trades", "list"]).await;
    assert_eq!(query, pairs(&[("taker_only", "true"), ("limit", "100")]));
}

#[tokio::test]
async fn a_single_page_prints_the_envelope_as_received() {
    let (_, run) = sent("/v2/trades", &fixture("trades"), &["trades", "list"]).await;

    let printed: Value = serde_json::from_str(&run.out).unwrap();
    let received: Value = serde_json::from_str(&fixture("trades")).unwrap();
    assert_eq!(printed["pagination"], received["pagination"]);
    assert_eq!(
        printed["data"][0]["proxy_wallet"], received["data"][0]["proxy_wallet"],
        "rows keep v2's snake_case field names"
    );
    assert_eq!(run.err, "");
}

// ── paging ───────────────────────────────────────────────────────────

#[tokio::test]
async fn cursor_resumes_a_single_page() {
    let (query, _) = sent(
        "/v2/trades",
        &fixture("trades"),
        &["trades", "list", "--cursor", "c7"],
    )
    .await;
    assert_eq!(cursor(&query), some("c7"));
}

#[tokio::test]
async fn all_writes_every_row_of_every_page_as_jsonl() {
    let mut server = Server::new_async().await;
    let seen = three_pages(&mut server).await;

    let run = run(
        &server,
        &["trades", "list", "--user", "0xu", "--limit", "2", "--all"],
    )
    .await;
    run.expect_ok();

    let rows = run.rows();
    assert_eq!(rows.len(), 6, "three pages of two rows");
    assert!(rows.iter().all(|row| row["proxy_wallet"].is_string()));
    assert_eq!(
        run.err, "",
        "a walk that reaches the end has nothing to resume"
    );

    assert_eq!(cursors(&seen), [None, some("c1"), some("c2")]);
    let seen = seen.lock().unwrap();
    let first = without_cursor(&seen[0]);
    assert_eq!(
        first,
        pairs(&[("user", "0xu"), ("taker_only", "true"), ("limit", "2")])
    );
    for (i, query) in seen.iter().enumerate() {
        assert_eq!(
            without_cursor(query),
            first,
            "page {} changed filters",
            i + 1
        );
    }
}

#[tokio::test]
async fn max_pages_stops_the_walk_and_prints_the_cursor_to_resume_from() {
    let mut server = Server::new_async().await;
    let seen = three_pages(&mut server).await;

    let run = run(&server, &["trades", "list", "--all", "--max-pages", "2"]).await;
    run.expect_ok();

    assert_eq!(run.rows().len(), 4);
    assert_eq!(run.err, "next_cursor: c2\n");
    assert_eq!(
        cursors(&seen),
        [None, some("c1")],
        "the third page is never fetched"
    );
}

#[tokio::test]
async fn all_starts_from_the_given_cursor() {
    let mut server = Server::new_async().await;
    let seen = three_pages(&mut server).await;

    let run = run(&server, &["trades", "list", "--cursor", "c1", "--all"]).await;
    run.expect_ok();

    assert_eq!(run.rows().len(), 4);
    assert_eq!(cursors(&seen), [some("c1"), some("c2")]);
}

#[tokio::test]
async fn a_failed_page_fails_the_walk_and_prints_the_cursor_to_retry() {
    let mut server = Server::new_async().await;
    let fails = |request: &mockito::Request| {
        cursor(&query_pairs(request.path_and_query())).as_deref() == Some("c1")
    };
    server
        .mock("GET", "/v2/trades")
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |request| if fails(request) { 500 } else { 200 })
        .with_body_from_request(move |request| {
            if fails(request) {
                br#"{"error":"boom","code":"internal","retryable":false,"trace_id":"t-500"}"#
                    .to_vec()
            } else {
                trades_page(Some("c1")).into_bytes()
            }
        })
        .expect(2)
        .create_async()
        .await;

    let run = run(&server, &["trades", "list", "--all"]).await;

    assert!(
        run.result.is_err(),
        "a walk that loses a page must not exit 0"
    );
    assert_eq!(run.rows().len(), 2, "rows before the failure are kept");
    assert_eq!(run.err, "next_cursor: c1\n");
}

#[tokio::test]
async fn offset_is_refused_with_a_pointer_to_cursor() {
    let server = Server::new_async().await;
    for args in [
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
    ] {
        let run = run(&server, args).await;
        let message = run.result.unwrap_err().to_string();
        assert!(message.contains("--cursor"), "{args:?}: {message}");
    }
}

#[tokio::test]
async fn max_pages_without_all_is_refused() {
    let server = Server::new_async().await;
    let run = run(&server, &["trades", "list", "--max-pages", "2"]).await;
    assert!(run.result.is_err());
}
```

Run:

```bash
cargo test -p polyoxide-cli --all-features --test data_v2
```

Expected: `test result: ok. 10 passed`.

- [ ] **Step 7: Prove the mock tests catch a dropped cursor and the old `--taker-only`**

Run:

```bash
sed -i 's/Some(cursor) => request.with_cursor(cursor),/Some(_) => request,/' polyoxide-cli/src/commands/data/paging.rs
cargo test -p polyoxide-cli --all-features --test data_v2 2>&1 | grep -E '^test .* FAILED'
sed -i 's/Some(_) => request,/Some(cursor) => request.with_cursor(cursor),/' polyoxide-cli/src/commands/data/paging.rs
```

Expected: `test cursor_resumes_a_single_page ... FAILED` among the lines printed.

Run:

```bash
sed -i 's/#\[arg(long, default_value_t = true, action = ArgAction::Set)\]/#[arg(long, default_value = "true")]/' polyoxide-cli/src/commands/data/trades.rs
cargo test -p polyoxide-cli --all-features --test data_v2 2>&1 | grep -E '^test .* FAILED'
sed -i 's/#\[arg(long, default_value = "true")\]/#[arg(long, default_value_t = true, action = ArgAction::Set)]/' polyoxide-cli/src/commands/data/trades.rs
```

Expected: `test trades_flags_reach_their_v2_parameters ... FAILED` among the lines printed.

- [ ] **Step 8: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-cli --all-features
```

Expected: every suite passes; `data_v2` reports `10 passed`.

- [ ] **Step 9: Commit**

```bash
git add polyoxide-cli/src/commands/common/parsing.rs polyoxide-cli/src/commands/data/mod.rs polyoxide-cli/src/commands/data/trades.rs polyoxide-cli/tests/data_v2.rs
git commit -F - <<'EOF'
feat(cli)!: data trades on Data API v2 with cursor paging

trades list reads /v2/trades. It prints the v2 envelope with snake_case
fields, pages with --cursor/--all/--max-pages, and refuses --offset.
--condition replaces --market, which stays as an alias. --start and --end
are new.

Fixes two v1 flag bugs: every comma-separated flag panicked when given,
because a Vec-returning value parser does not fit a Vec field, and
--taker-only could not be turned off.

DataCommand::run_with takes the client and output writers, so mock
tests run real arguments and read what was printed.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 3: Port `data activity` and `data positions` to v2

**Files:**
- Modify: `polyoxide-cli/src/commands/data/activity.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/positions.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/trades.rs` (drop the v1 side conversion)
- Modify: `polyoxide-cli/src/commands/data/mod.rs`
- Modify: `polyoxide-cli/src/commands/common/parsing.rs`
- Modify: `polyoxide-cli/tests/data_v2.rs`

- [ ] **Step 1: Accept every v2 activity type**

The parser moves to v2's `ActivityType` and matches against `ActivityType::ALL`, so `tip`, `maker_rebate`, `deposit` and the rest are accepted, and the error lists whatever the SDK knows rather than a hand-kept list. It moves in the same task as both commands that call it.

In `polyoxide-cli/src/commands/common/parsing.rs`, replace:

```rust
use polyoxide_data::types::ActivityType;
```

with:

```rust
use polyoxide_data::v2::types::ActivityType;
```

In `polyoxide-cli/src/commands/common/parsing.rs`, replace:

```rust
pub fn parse_activity_types(input: &str) -> Result<Vec<ActivityType>> {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for s in input.split(',') {
        let trimmed = s.trim();
        match trimmed.to_uppercase().as_str() {
            "TRADE" => valid.push(ActivityType::Trade),
            "SPLIT" => valid.push(ActivityType::Split),
            "MERGE" => valid.push(ActivityType::Merge),
            "REDEEM" => valid.push(ActivityType::Redeem),
            "REWARD" => valid.push(ActivityType::Reward),
            "CONVERSION" => valid.push(ActivityType::Conversion),
            _ => invalid.push(trimmed.to_string()),
        }
    }

    if !invalid.is_empty() {
        bail!(
            "Invalid activity type(s): {}. Valid types: trade, split, merge, redeem, reward, conversion",
            invalid.join(", ")
        );
    }

    Ok(valid)
}

```

with:

```rust
/// Parses comma-separated activity types, case-insensitively, against every
/// type the SDK knows (`ActivityType::ALL`).
pub fn parse_activity_types(input: &str) -> Result<Vec<ActivityType>> {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for s in input.split(',') {
        let trimmed = s.trim();
        let wire = trimmed.to_uppercase();
        match ActivityType::ALL.iter().find(|t| t.as_str() == wire) {
            Some(activity_type) => valid.push(activity_type.clone()),
            None => invalid.push(trimmed.to_string()),
        }
    }

    if !invalid.is_empty() {
        let names: Vec<String> = ActivityType::ALL
            .iter()
            .map(|t| t.as_str().to_lowercase())
            .collect();
        bail!(
            "Invalid activity type(s): {}. Valid types: {}",
            invalid.join(", "),
            names.join(", ")
        );
    }

    Ok(valid)
}

```

In `polyoxide-cli/src/commands/common/parsing.rs`, replace:

```rust
    #[test]
    fn parse_activity_types_all_variants() {
        let result = parse_activity_types("trade,split,merge,redeem,reward,conversion").unwrap();
        assert_eq!(result.len(), 6);
    }

```

with:

```rust
    #[test]
    fn parse_activity_types_accepts_every_v2_type() {
        let every = ActivityType::ALL
            .iter()
            .map(|t| t.as_str().to_lowercase())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse_activity_types(&every).unwrap(), ActivityType::ALL);
    }

    #[test]
    fn parse_activity_types_accepts_v2_only_types() {
        assert_eq!(
            parse_activity_types("tip,maker_rebate").unwrap(),
            vec![ActivityType::Tip, ActivityType::MakerRebate]
        );
    }

    #[test]
    fn parse_activity_types_error_lists_the_valid_types() {
        let msg = parse_activity_types("nope").unwrap_err().to_string();
        assert!(msg.contains("tip") && msg.contains("deposit"), "{msg}");
    }

```

- [ ] **Step 2: Replace `activity.rs`**

The filters become `ActivityFilters`, shared with `positions activity`. `--sort-by` is removed: v2 supports only `TIMESTAMP`. `--include-deposits-withdrawals` is new, because v2 hides those rows by default and they are otherwise unreachable.

Replace the entire contents of `polyoxide-cli/src/commands/data/activity.rs`:

```rust
use std::io::Write;

use clap::Args;
use color_eyre::eyre::Result;
use polyoxide_data::DataApi;

use super::paging::{run_paged, PageArgs};
use super::SortOrder;
use crate::commands::common::parsing::{parse_activity_types, parse_list_entry};
use crate::commands::data::trades::TradeSideFilter;

/// Query a user's activity (`/v2/activity`)
#[derive(Args)]
pub struct UserActivityCommand {
    /// User address (0x-prefixed, 40 hex chars)
    #[arg(short, long)]
    pub user: String,
    #[command(flatten)]
    pub filters: ActivityFilters,
}

impl UserActivityCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        self.filters.run(data, &self.user, out, err).await
    }
}

/// Filters shared by `data activity` and `data positions activity`
#[derive(Args, Debug, Clone, PartialEq)]
pub struct ActivityFilters {
    /// Filter by market condition IDs (comma-separated, at most 20)
    #[arg(
        short = 'm',
        long = "condition",
        visible_alias = "market",
        value_delimiter = ',',
        value_parser = parse_list_entry
    )]
    pub condition: Option<Vec<String>>,
    /// Filter by event IDs (comma-separated)
    #[arg(short, long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub event_id: Option<Vec<String>>,
    /// Filter by activity types (comma-separated, e.g. trade,split,tip)
    #[arg(short = 'T', long)]
    pub activity_type: Option<String>,
    /// Filter trade rows by side
    #[arg(short, long, value_enum)]
    pub side: Option<TradeSideFilter>,
    /// Window start, epoch seconds (default: three years back; 1 for full history)
    #[arg(long)]
    pub start: Option<i64>,
    /// Window end, epoch seconds
    #[arg(long)]
    pub end: Option<i64>,
    /// Include deposit and withdrawal rows, which the API hides by default
    #[arg(long)]
    pub include_deposits_withdrawals: bool,
    /// Page size (at most 1000)
    #[arg(short, long, default_value = "100")]
    pub limit: u32,
    /// Sort direction (rows are always sorted by timestamp)
    #[arg(long, value_enum, default_value = "desc")]
    pub sort_direction: SortOrder,
    #[command(flatten)]
    pub page: PageArgs,
}

impl ActivityFilters {
    pub async fn run(
        self,
        data: &DataApi,
        user: &str,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<()> {
        let mut request = data
            .v2()
            .activity(user)
            .limit(self.limit)
            .sort_direction(self.sort_direction.into());
        if let Some(ids) = self.condition {
            request = request.conditions(ids);
        }
        if let Some(ids) = self.event_id {
            request = request.event_ids(ids);
        }
        if let Some(types) = self.activity_type {
            request = request.types(parse_activity_types(&types)?);
        }
        if let Some(side) = self.side {
            request = request.side(side.into());
        }
        if let Some(ts) = self.start {
            request = request.start(ts);
        }
        if let Some(ts) = self.end {
            request = request.end(ts);
        }
        if self.include_deposits_withdrawals {
            request = request.exclude_deposits_withdrawals(false);
        }
        run_paged(request, &self.page, out, err).await
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        cmd: UserActivityCommand,
    }

    fn try_parse(args: &[&str]) -> Result<UserActivityCommand, clap::Error> {
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    #[test]
    fn activity_defaults() {
        let cmd = try_parse(&["test", "--user", "0xabc"]).unwrap();
        assert_eq!(cmd.user, "0xabc");
        assert_eq!(cmd.filters.limit, 100);
        assert!(matches!(cmd.filters.sort_direction, SortOrder::Desc));
        assert!(!cmd.filters.include_deposits_withdrawals);
        assert_eq!(cmd.filters.page, PageArgs::default());
    }

    #[test]
    fn sort_by_is_gone_because_only_timestamp_is_supported() {
        assert!(try_parse(&["test", "--user", "0xabc", "--sort-by", "tokens"]).is_err());
    }

    #[test]
    fn market_is_an_alias_of_condition() {
        let cmd = try_parse(&["test", "--user", "0xabc", "--market", "0xc"]).unwrap();
        assert_eq!(cmd.filters.condition.unwrap(), ["0xc"]);
    }
}
```

- [ ] **Step 3: Replace `positions.rs`**

`list --status open|redeemable|closed` replaces `list` and `closed` (`OPEN`, `REDEEMABLE` and `CLOSED` also parse). With `--condition` the anchor is `UserInConditions`, otherwise `User`. `--redeemable`, `--mergeable` and `--size-threshold` are gone; `--filter-type`/`--filter-amount`, `--include-archived`, `--start` and `--end` are new. `closed` stays as a hidden subcommand that fails with its replacement, so an old script gets a pointer rather than `unrecognized subcommand`.

Replace the entire contents of `polyoxide-cli/src/commands/data/positions.rs`:

```rust
use std::io::Write;

use clap::{Args, Subcommand, ValueEnum};
use color_eyre::eyre::{bail, Result};
use polyoxide_data::{
    v2::types::{PositionAnchor, PositionSortBy, PositionStatus},
    DataApi,
};

use super::activity::ActivityFilters;
use super::paging::{print_pretty, run_paged, PageArgs};
use super::SortOrder;
use crate::commands::common::parsing::parse_list_entry;
use crate::commands::data::trades::TradeFilterField;

#[derive(Args)]
pub struct PositionsCommand {
    /// User address (0x-prefixed, 40 hex chars)
    #[arg(short, long)]
    pub user: String,

    #[command(subcommand)]
    pub command: PositionsSubcommand,
}

#[derive(Subcommand)]
pub enum PositionsSubcommand {
    /// List the user's positions (`/v2/positions`); --status selects open, redeemable or closed
    List {
        /// Filter by market condition IDs (comma-separated, at most 20)
        #[arg(
            short = 'm',
            long = "condition",
            visible_alias = "market",
            value_delimiter = ',',
            value_parser = parse_list_entry
        )]
        condition: Option<Vec<String>>,
        /// Filter by event IDs (comma-separated)
        #[arg(short, long, value_delimiter = ',', value_parser = parse_list_entry)]
        event_id: Option<Vec<String>>,
        /// Lifecycle state (open includes redeemable positions)
        #[arg(long, value_enum, ignore_case = true, default_value = "open")]
        status: PositionStatusFilter,
        /// Filter by market title (case-insensitive substring, at most 200 chars)
        #[arg(short, long)]
        title: Option<String>,
        /// Unit of --filter-amount (API default: tokens)
        #[arg(long, value_enum)]
        filter_type: Option<TradeFilterField>,
        /// Minimum current holding, in the unit of --filter-type
        #[arg(long)]
        filter_amount: Option<f64>,
        /// Include positions on archived markets (open and redeemable only)
        #[arg(long)]
        include_archived: bool,
        /// Sort field (API default depends on --status)
        #[arg(long, value_enum)]
        sort_by: Option<PositionSortField>,
        /// Sort direction
        #[arg(long, value_enum, default_value = "desc")]
        sort_direction: SortOrder,
        /// Only positions whose last event is at or after this epoch second
        #[arg(long)]
        start: Option<i64>,
        /// Only positions whose last event is at or before this epoch second
        #[arg(long)]
        end: Option<i64>,
        /// Page size (at most 1000)
        #[arg(short, long, default_value = "100")]
        limit: u32,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get the total value of the user's positions (`/v2/value`)
    Value {
        /// Value only these market condition IDs (comma-separated, at most 20)
        #[arg(
            short = 'm',
            long = "condition",
            visible_alias = "market",
            value_delimiter = ',',
            value_parser = parse_list_entry
        )]
        condition: Option<Vec<String>>,
    },
    /// Removed: use `positions list --status closed`
    #[command(hide = true)]
    Closed {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _ignored: Vec<String>,
    },
    /// List activity for the user (`/v2/activity`)
    Activity(ActivityFilters),
}

impl PositionsCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        match self.command {
            PositionsSubcommand::List {
                condition,
                event_id,
                status,
                title,
                filter_type,
                filter_amount,
                include_archived,
                sort_by,
                sort_direction,
                start,
                end,
                limit,
                page,
            } => {
                let anchor = match condition {
                    Some(conditions) => PositionAnchor::UserInConditions {
                        user: self.user,
                        conditions,
                    },
                    None => PositionAnchor::User(self.user),
                };
                let mut request = data
                    .v2()
                    .positions(anchor)
                    .status(status.into())
                    .sort_direction(sort_direction.into())
                    .limit(limit);
                if let Some(ids) = event_id {
                    request = request.event_ids(ids);
                }
                if let Some(title) = title {
                    request = request.title(title);
                }
                if let Some(filter_type) = filter_type {
                    request = request.filter_type(filter_type.into());
                }
                if let Some(amount) = filter_amount {
                    request = request.filter_amount(amount);
                }
                if include_archived {
                    request = request.include_archived(true);
                }
                if let Some(sort_by) = sort_by {
                    request = request.sort_by(sort_by.into());
                }
                if let Some(ts) = start {
                    request = request.start(ts);
                }
                if let Some(ts) = end {
                    request = request.end(ts);
                }
                run_paged(request, &page, out, err).await
            }
            PositionsSubcommand::Value { condition } => {
                let mut request = data.v2().value(self.user);
                if let Some(ids) = condition {
                    request = request.conditions(ids);
                }
                print_pretty(&request.send().await?, out)
            }
            PositionsSubcommand::Closed { .. } => {
                bail!("`positions closed` was removed: use `positions list --status closed`")
            }
            PositionsSubcommand::Activity(filters) => filters.run(data, &self.user, out, err).await,
        }
    }
}

/// Position lifecycle state
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq)]
pub enum PositionStatusFilter {
    /// Open positions, including settled-but-unredeemed winners
    #[default]
    Open,
    /// Only positions that can be redeemed now
    Redeemable,
    /// Exited positions
    Closed,
}

impl From<PositionStatusFilter> for PositionStatus {
    fn from(status: PositionStatusFilter) -> Self {
        match status {
            PositionStatusFilter::Open => Self::Open,
            PositionStatusFilter::Redeemable => Self::Redeemable,
            PositionStatusFilter::Closed => Self::Closed,
        }
    }
}

/// Sort field for positions
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum PositionSortField {
    /// Mark-to-market value
    CurrentValue,
    /// Token count
    Tokens,
    /// Unrealized P&L
    UnrealizedPnl,
    /// Realized P&L
    RealizedPnl,
    /// Total P&L
    TotalPnl,
    /// Time of the position's last event
    Timestamp,
}

impl From<PositionSortField> for PositionSortBy {
    fn from(field: PositionSortField) -> Self {
        match field {
            PositionSortField::CurrentValue => Self::CurrentValue,
            PositionSortField::Tokens => Self::Tokens,
            PositionSortField::UnrealizedPnl => Self::UnrealizedPnl,
            PositionSortField::RealizedPnl => Self::RealizedPnl,
            PositionSortField::TotalPnl => Self::TotalPnl,
            PositionSortField::Timestamp => Self::Timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::commands::data::trades::TradeSideFilter;

    #[derive(Parser)]
    struct TestWrapper {
        #[command(flatten)]
        cmd: PositionsCommand,
    }

    fn try_parse(args: &[&str]) -> Result<TestWrapper, clap::Error> {
        TestWrapper::try_parse_from(args)
    }

    #[test]
    fn status_filter_maps_to_the_v2_status() {
        assert_eq!(
            PositionStatus::from(PositionStatusFilter::Open),
            PositionStatus::Open
        );
        assert_eq!(
            PositionStatus::from(PositionStatusFilter::Redeemable),
            PositionStatus::Redeemable
        );
        assert_eq!(
            PositionStatus::from(PositionStatusFilter::Closed),
            PositionStatus::Closed
        );
    }

    #[test]
    fn sort_field_maps_to_the_v2_sort() {
        let pairs = [
            (
                PositionSortField::CurrentValue,
                PositionSortBy::CurrentValue,
            ),
            (PositionSortField::Tokens, PositionSortBy::Tokens),
            (
                PositionSortField::UnrealizedPnl,
                PositionSortBy::UnrealizedPnl,
            ),
            (PositionSortField::RealizedPnl, PositionSortBy::RealizedPnl),
            (PositionSortField::TotalPnl, PositionSortBy::TotalPnl),
            (PositionSortField::Timestamp, PositionSortBy::Timestamp),
        ];
        for (field, expected) in pairs {
            assert_eq!(PositionSortBy::from(field), expected);
        }
    }

    #[test]
    fn positions_requires_user_flag() {
        assert!(try_parse(&["test", "list"]).is_err());
    }

    #[test]
    fn positions_list_defaults() {
        let w = try_parse(&["test", "--user", "0xabc", "list"]).unwrap();
        assert_eq!(w.cmd.user, "0xabc");
        match w.cmd.command {
            PositionsSubcommand::List {
                status,
                sort_by,
                sort_direction,
                limit,
                include_archived,
                page,
                ..
            } => {
                assert_eq!(status, PositionStatusFilter::Open);
                assert!(sort_by.is_none(), "the API picks a default by status");
                assert!(matches!(sort_direction, SortOrder::Desc));
                assert_eq!(limit, 100);
                assert!(!include_archived);
                assert_eq!(page, PageArgs::default());
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn positions_list_status_and_sort_parse() {
        let w = try_parse(&[
            "test",
            "--user",
            "0xabc",
            "list",
            "--status",
            "closed",
            "--sort-by",
            "realized-pnl",
        ])
        .unwrap();
        match w.cmd.command {
            PositionsSubcommand::List {
                status, sort_by, ..
            } => {
                assert_eq!(status, PositionStatusFilter::Closed);
                assert_eq!(sort_by, Some(PositionSortField::RealizedPnl));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn status_accepts_the_upstream_spelling() {
        let w = try_parse(&["test", "--user", "0xabc", "list", "--status", "REDEEMABLE"]).unwrap();
        match w.cmd.command {
            PositionsSubcommand::List { status, .. } => {
                assert_eq!(status, PositionStatusFilter::Redeemable)
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn v1_only_list_flags_are_gone() {
        for flag in ["--redeemable", "--mergeable"] {
            assert!(
                try_parse(&["test", "--user", "0xabc", "list", flag]).is_err(),
                "{flag}"
            );
        }
        assert!(try_parse(&["test", "--user", "0xabc", "list", "--size-threshold", "1"]).is_err());
    }

    #[test]
    fn positions_value_parses_with_market_alias() {
        let w = try_parse(&["test", "--user", "0xabc", "value", "--market", "0xc"]).unwrap();
        match w.cmd.command {
            PositionsSubcommand::Value { condition } => assert_eq!(condition.unwrap(), ["0xc"]),
            _ => panic!("expected Value"),
        }
    }

    #[test]
    fn closed_still_parses_so_it_can_explain_its_replacement() {
        let w = try_parse(&["test", "--user", "0xabc", "closed", "--limit", "20"]).unwrap();
        assert!(matches!(w.cmd.command, PositionsSubcommand::Closed { .. }));
    }

    #[test]
    fn positions_activity_parses() {
        let w = try_parse(&["test", "--user", "0xabc", "activity", "--side", "buy"]).unwrap();
        match w.cmd.command {
            PositionsSubcommand::Activity(filters) => {
                assert!(matches!(filters.side, Some(TradeSideFilter::Buy)));
            }
            _ => panic!("expected Activity"),
        }
    }

    #[test]
    fn positions_requires_subcommand() {
        assert!(try_parse(&["test", "--user", "0xabc"]).is_err());
    }
}
```

- [ ] **Step 4: Drop the v1 side conversion**

Nothing maps to v1's `TradeSide` any more.

In `polyoxide-cli/src/commands/data/trades.rs`, delete:

```rust
/// The v1 side, for `activity` and `positions` until they move to v2 in the
/// next task.
impl From<TradeSideFilter> for polyoxide_data::types::TradeSide {
    fn from(side: TradeSideFilter) -> Self {
        match side {
            TradeSideFilter::Buy => Self::Buy,
            TradeSideFilter::Sell => Self::Sell,
        }
    }
}
```

- [ ] **Step 5: Dispatch both through the writers**

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
    /// Query user positions (open, closed, value) and activity
```

with:

```rust
    /// Query user positions (open, redeemable, closed, value) and activity
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
            Self::Activity(cmd) => cmd.run(data).await,
```

with:

```rust
            Self::Activity(cmd) => cmd.run(data, out, err).await,
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
            Self::Positions(cmd) => cmd.run(data).await,
```

with:

```rust
            Self::Positions(cmd) => cmd.run(data, out, err).await,
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum SortOrder {
```

with:

```rust
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq)]
pub enum SortOrder {
```

- [ ] **Step 6: Run the unit tests**

Run:

```bash
cargo test -p polyoxide-cli --all-features --lib
```

Expected: `test result: ok. 218 passed`.

- [ ] **Step 7: Add the mock tests**

In `polyoxide-cli/tests/data_v2.rs`, replace:

```rust
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
```

with:

```rust
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
        &["activity", "--user", "0xu", "--offset", "100"][..],
        &["positions", "--user", "0xu", "list", "--offset", "100"][..],
```

Append to the end of `polyoxide-cli/tests/data_v2.rs`:

```rust

// ── positions and activity ───────────────────────────────────────────

#[tokio::test]
async fn positions_list_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/positions",
        &fixture("positions_closed"),
        &[
            "positions",
            "--user",
            "0xu",
            "list",
            "--condition",
            "0xa,0xb",
            "--event-id",
            "7",
            "--status",
            "closed",
            "--title",
            "rain",
            "--filter-type",
            "tokens",
            "--filter-amount",
            "2.5",
            "--include-archived",
            "--sort-by",
            "realized-pnl",
            "--sort-direction",
            "asc",
            "--start",
            "100",
            "--end",
            "200",
            "--limit",
            "4",
        ],
    )
    .await;

    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("condition", "0xa,0xb"),
            ("event_id", "7"),
            ("status", "CLOSED"),
            ("title", "rain"),
            ("filter_type", "TOKENS"),
            ("filter_amount", "2.5"),
            ("include_archived", "true"),
            ("sort_by", "REALIZED_PNL"),
            ("sort_direction", "ASC"),
            ("start", "100"),
            ("end", "200"),
            ("limit", "4"),
        ])
    );
}

#[tokio::test]
async fn positions_list_defaults_to_open_positions_for_the_user() {
    let (query, _) = sent(
        "/v2/positions",
        &fixture("positions"),
        &["positions", "--user", "0xu", "list"],
    )
    .await;
    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("status", "OPEN"),
            ("sort_direction", "DESC"),
            ("limit", "100"),
        ])
    );
}

#[tokio::test]
async fn positions_value_scopes_to_conditions() {
    let (query, _) = sent(
        "/v2/value",
        &fixture("value"),
        &["positions", "--user", "0xu", "value", "--market", "0xa,0xb"],
    )
    .await;
    assert_eq!(query, pairs(&[("user", "0xu"), ("condition", "0xa,0xb")]));
}

#[tokio::test]
async fn positions_closed_names_its_replacement() {
    let server = Server::new_async().await;
    let run = run(&server, &["positions", "--user", "0xu", "closed"]).await;
    let message = run.result.unwrap_err().to_string();
    assert!(message.contains("--status closed"), "{message}");
}

#[tokio::test]
async fn activity_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/activity",
        &fixture("activity"),
        &[
            "activity",
            "--user",
            "0xu",
            "--condition",
            "0xa",
            "--event-id",
            "7",
            "--activity-type",
            "trade,tip",
            "--side",
            "buy",
            "--start",
            "100",
            "--end",
            "200",
            "--include-deposits-withdrawals",
            "--sort-direction",
            "asc",
            "--limit",
            "5",
        ],
    )
    .await;

    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("condition", "0xa"),
            ("event_id", "7"),
            ("type", "TRADE,TIP"),
            ("side", "BUY"),
            ("start", "100"),
            ("end", "200"),
            ("exclude_deposits_withdrawals", "false"),
            ("sort_direction", "ASC"),
            ("limit", "5"),
        ])
    );
}

#[tokio::test]
async fn positions_activity_uses_the_positions_user() {
    let (query, _) = sent(
        "/v2/activity",
        &fixture("activity"),
        &["positions", "--user", "0xu", "activity"],
    )
    .await;
    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("sort_direction", "DESC"),
            ("limit", "100")
        ]),
        "deposits stay at the API's default of hidden unless asked for"
    );
}
```

Run:

```bash
cargo test -p polyoxide-cli --all-features --test data_v2
```

Expected: `test result: ok. 16 passed`.

- [ ] **Step 8: Prove the status and deposit mappings are tested**

Run:

```bash
sed -i 's/PositionStatusFilter::Closed => Self::Closed,/PositionStatusFilter::Closed => Self::Open,/' polyoxide-cli/src/commands/data/positions.rs
cargo test -p polyoxide-cli --all-features --test data_v2 2>&1 | grep -E '^test .* FAILED'
sed -i 's/PositionStatusFilter::Closed => Self::Open,/PositionStatusFilter::Closed => Self::Closed,/' polyoxide-cli/src/commands/data/positions.rs
```

Expected: `test positions_list_flags_reach_their_v2_parameters ... FAILED` among the lines printed.

Run:

```bash
sed -i 's/exclude_deposits_withdrawals(false)/exclude_deposits_withdrawals(true)/' polyoxide-cli/src/commands/data/activity.rs
cargo test -p polyoxide-cli --all-features --test data_v2 2>&1 | grep -E '^test .* FAILED'
sed -i 's/exclude_deposits_withdrawals(true)/exclude_deposits_withdrawals(false)/' polyoxide-cli/src/commands/data/activity.rs
```

Expected: `test activity_flags_reach_their_v2_parameters ... FAILED` among the lines printed.

- [ ] **Step 9: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-cli --all-features
```

Expected: every suite passes; `data_v2` reports `16 passed`.

- [ ] **Step 10: Commit**

```bash
git add polyoxide-cli/src/commands/common/parsing.rs polyoxide-cli/src/commands/data/activity.rs polyoxide-cli/src/commands/data/mod.rs polyoxide-cli/src/commands/data/positions.rs polyoxide-cli/src/commands/data/trades.rs polyoxide-cli/tests/data_v2.rs
git commit -F - <<'EOF'
feat(cli)!: data activity and positions on Data API v2

activity reads /v2/activity and accepts every v2 activity type, and
--include-deposits-withdrawals reaches the rows v2 hides by default.
--sort-by is removed: v2 sorts by timestamp only.

positions list reads /v2/positions with --status open|redeemable|closed,
which replaces positions closed; the old subcommand now fails with its
replacement. --redeemable, --mergeable and --size-threshold are removed.
positions value reads /v2/value.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 4: Port `data holders`, `open-interest` and `live-volume` to v2

**Files:**
- Modify: `polyoxide-cli/src/commands/data/holders.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/open_interest.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/live_volume.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/mod.rs`
- Modify: `polyoxide-cli/src/commands/common/parsing.rs`
- Modify: `polyoxide-cli/tests/data_v2.rs`

- [ ] **Step 1: Replace the three commands**

- **holders:** `--condition` is required (v2 answers 400 without it). `--min-balance` becomes optional and fractional; v1 defaulted it to `1`, v2's default is `0`. `--include-pnl` is new.
- **open-interest:** with no `--condition`, v2 returns one `GLOBAL` row.
- **live-volume:** `--event-id` takes a comma-separated list, since v2 spans events in one call.

Replace the entire contents of `polyoxide-cli/src/commands/data/holders.rs`:

```rust
use std::io::Write;

use clap::Args;
use color_eyre::eyre::Result;
use polyoxide_data::DataApi;

use super::paging::{run_paged, PageArgs};
use crate::commands::common::parsing::parse_list_entry;

/// Get top holders for markets (`/v2/holders`)
#[derive(Args)]
pub struct HoldersCommand {
    /// Market condition IDs (comma-separated, at most 20; exactly one with --include-pnl)
    #[arg(
        short = 'm',
        long = "condition",
        visible_alias = "market",
        value_delimiter = ',',
        value_parser = parse_list_entry,
        required = true
    )]
    condition: Vec<String>,
    /// Rows per outcome token (at most 1000, or 100 with --include-pnl)
    #[arg(short, long, default_value = "100")]
    limit: u32,
    /// Minimum balance in shares (API default: 0)
    #[arg(long)]
    min_balance: Option<f64>,
    /// Add each holder's entry cost and P&L, and switch to per-side balances
    #[arg(long)]
    include_pnl: bool,
    #[command(flatten)]
    page: PageArgs,
}

impl HoldersCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        let mut request = data.v2().holders(self.condition).limit(self.limit);
        if let Some(min_balance) = self.min_balance {
            request = request.min_balance(min_balance);
        }
        if self.include_pnl {
            request = request.include_pnl(true);
        }
        run_paged(request, &self.page, out, err).await
    }
}
```

Replace the entire contents of `polyoxide-cli/src/commands/data/open_interest.rs`:

```rust
use std::io::Write;

use clap::Args;
use color_eyre::eyre::Result;
use polyoxide_data::DataApi;

use super::paging::print_pretty;
use crate::commands::common::parsing::parse_list_entry;

/// Get open interest (`/v2/oi`); with no markets, the single global figure
#[derive(Args)]
pub struct OpenInterestCommand {
    /// Filter by market condition IDs (comma-separated, at most 20)
    #[arg(
        short = 'm',
        long = "condition",
        visible_alias = "market",
        value_delimiter = ',',
        value_parser = parse_list_entry
    )]
    pub condition: Option<Vec<String>>,
}

impl OpenInterestCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write) -> Result<()> {
        let mut request = data.v2().open_interest();
        if let Some(ids) = self.condition {
            request = request.conditions(ids);
        }
        print_pretty(&request.send().await?, out)
    }
}
```

Replace the entire contents of `polyoxide-cli/src/commands/data/live_volume.rs`:

```rust
use std::io::Write;

use clap::Args;
use color_eyre::eyre::Result;
use polyoxide_data::DataApi;

use super::paging::print_pretty;
use crate::commands::common::parsing::parse_list_entry;

/// Get live taker volume for events (`/v2/live-volume`)
#[derive(Args)]
pub struct LiveVolumeCommand {
    /// Event IDs (comma-separated)
    #[arg(
        short,
        long,
        value_delimiter = ',',
        value_parser = parse_list_entry,
        required = true
    )]
    pub event_id: Vec<String>,
}

impl LiveVolumeCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write) -> Result<()> {
        let volume = data.v2().live_volume(self.event_id).send().await?;
        print_pretty(&volume, out)
    }
}
```

- [ ] **Step 2: Dispatch them through the writers and update the parse tests**

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
    /// Get live volume for an event
```

with:

```rust
    /// Get live volume for events
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
            Self::Holders(cmd) => cmd.run(data).await,
```

with:

```rust
            Self::Holders(cmd) => cmd.run(data, out, err).await,
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
            Self::OpenInterest(cmd) => cmd.run(data).await,
```

with:

```rust
            Self::OpenInterest(cmd) => cmd.run(data, out).await,
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
            Self::LiveVolume(cmd) => cmd.run(data).await,
```

with:

```rust
            Self::LiveVolume(cmd) => cmd.run(data, out).await,
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
    #[test]
    fn holders_parses_without_market() {
        // market defaults to empty Vec, so it parses without --market
        let cmd = try_parse(&["test", "holders"]).unwrap();
        assert!(matches!(cmd, DataCommand::Holders(_)));
    }

```

with:

```rust
    #[test]
    fn live_volume_parses_several_event_ids() {
        let cmd = try_parse(&["test", "live-volume", "--event-id", "42,43"]).unwrap();
        match cmd {
            DataCommand::LiveVolume(cmd) => assert_eq!(cmd.event_id, ["42", "43"]),
            _ => panic!("expected LiveVolume"),
        }
    }

    #[test]
    fn holders_requires_a_condition() {
        // v2 has no marketless holders listing: it 400s without `condition`
        assert!(try_parse(&["test", "holders"]).is_err());
    }

    #[test]
    fn holders_parses_with_market_alias() {
        let cmd = try_parse(&["test", "holders", "--market", "0xc"]).unwrap();
        assert!(matches!(cmd, DataCommand::Holders(_)));
    }

```

- [ ] **Step 3: Remove the broken comma-separated parser**

Nothing uses `parse_comma_separated` now.

In `polyoxide-cli/src/commands/common/parsing.rs`, delete:

```rust
/// Parse comma-separated values into a Vec of trimmed strings.
/// Used as a clap value_parser for arguments that accept multiple IDs.
pub fn parse_comma_separated(s: &str) -> Result<Vec<String>, std::convert::Infallible> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    let strings = s.split(',').map(|s| s.trim().to_string()).collect();
    Ok(strings)
}
```

In `polyoxide-cli/src/commands/common/parsing.rs`, delete:

```rust
    // --- parse_comma_separated tests ---

    #[test]
    fn parse_comma_separated_single_value() {
        let result = parse_comma_separated("abc").unwrap();
        assert_eq!(result, vec!["abc"]);
    }

    #[test]
    fn parse_comma_separated_multiple_values() {
        let result = parse_comma_separated("a,b,c").unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_comma_separated_trims_whitespace() {
        let result = parse_comma_separated(" a , b , c ").unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_comma_separated_empty_string() {
        let result = parse_comma_separated("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_comma_separated_trailing_comma() {
        let result = parse_comma_separated("a,b,").unwrap();
        assert_eq!(result, vec!["a", "b", ""]);
    }
```

- [ ] **Step 4: Run the unit tests**

Run:

```bash
cargo test -p polyoxide-cli --all-features --lib
```

Expected: `test result: ok. 215 passed`.

- [ ] **Step 5: Add the mock tests**

In `polyoxide-cli/tests/data_v2.rs`, replace:

```rust
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
        &["activity", "--user", "0xu", "--offset", "100"][..],
        &["positions", "--user", "0xu", "list", "--offset", "100"][..],
```

with:

```rust
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
        &["activity", "--user", "0xu", "--offset", "100"][..],
        &["positions", "--user", "0xu", "list", "--offset", "100"][..],
        &["holders", "--condition", "0xc", "--offset", "100"][..],
```

Append to the end of `polyoxide-cli/tests/data_v2.rs`:

```rust

// ── holders, open interest and live volume ───────────────────────────

#[tokio::test]
async fn holders_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/holders",
        &fixture("holders_pnl"),
        &[
            "holders",
            "--market",
            "0xc",
            "--min-balance",
            "2.5",
            "--include-pnl",
            "--limit",
            "10",
        ],
    )
    .await;
    assert_eq!(
        query,
        pairs(&[
            ("condition", "0xc"),
            ("min_balance", "2.5"),
            ("include_pnl", "true"),
            ("limit", "10"),
        ])
    );
}

#[tokio::test]
async fn open_interest_is_global_without_markets() {
    let (query, _) = sent(
        "/v2/oi",
        &fixture("open_interest_global"),
        &["open-interest"],
    )
    .await;
    assert_eq!(query, pairs(&[]));
}

#[tokio::test]
async fn open_interest_scopes_to_markets() {
    let (query, _) = sent(
        "/v2/oi",
        &fixture("open_interest"),
        &["open-interest", "--market", "0xa,0xb"],
    )
    .await;
    assert_eq!(query, pairs(&[("condition", "0xa,0xb")]));
}

#[tokio::test]
async fn live_volume_sends_every_event_id() {
    let (query, _) = sent(
        "/v2/live-volume",
        &fixture("live_volume"),
        &["live-volume", "--event-id", "1,2"],
    )
    .await;
    assert_eq!(query, pairs(&[("event_id", "1,2")]));
}
```

Run:

```bash
cargo test -p polyoxide-cli --all-features --test data_v2
```

Expected: `test result: ok. 20 passed`.

- [ ] **Step 6: Prove `--include-pnl` is tested**

Run:

```bash
sed -i 's/request = request.include_pnl(true);/request = request.limit(self.limit);/' polyoxide-cli/src/commands/data/holders.rs
cargo test -p polyoxide-cli --all-features --test data_v2 2>&1 | grep -E '^test .* FAILED'
sed -i 's/request = request.limit(self.limit);/request = request.include_pnl(true);/' polyoxide-cli/src/commands/data/holders.rs
```

Expected: `test holders_flags_reach_their_v2_parameters ... FAILED` among the lines printed.

- [ ] **Step 7: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-cli --all-features
```

Expected: every suite passes; `data_v2` reports `20 passed`.

- [ ] **Step 8: Commit**

```bash
git add polyoxide-cli/src/commands/common/parsing.rs polyoxide-cli/src/commands/data/holders.rs polyoxide-cli/src/commands/data/live_volume.rs polyoxide-cli/src/commands/data/mod.rs polyoxide-cli/src/commands/data/open_interest.rs polyoxide-cli/tests/data_v2.rs
git commit -F - <<'EOF'
feat(cli)!: data holders, open-interest and live-volume on Data API v2

holders reads /v2/holders and now requires --condition, which v2
enforces. --min-balance defaults to the API's 0 rather than 1, and
--include-pnl is new. open-interest reads /v2/oi. live-volume reads
/v2/live-volume and takes several --event-id values.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 5: Port `data traded` and `data builders` to v2

**Files:**
- Modify: `polyoxide-cli/src/commands/data/traded.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/builders.rs` (full replacement)
- Modify: `polyoxide-cli/src/commands/data/mod.rs`
- Modify: `polyoxide-cli/tests/data_v2.rs`

- [ ] **Step 1: Write the failing mock tests**

`traded` still calls v1's `/traded`, which the mock does not serve, and `builders` still accepts `--offset`, so these fail first.

In `polyoxide-cli/tests/data_v2.rs`, replace:

```rust
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
        &["activity", "--user", "0xu", "--offset", "100"][..],
        &["positions", "--user", "0xu", "list", "--offset", "100"][..],
        &["holders", "--condition", "0xc", "--offset", "100"][..],
```

with:

```rust
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
        &["activity", "--user", "0xu", "--offset", "100"][..],
        &["positions", "--user", "0xu", "list", "--offset", "100"][..],
        &["holders", "--condition", "0xc", "--offset", "100"][..],
        &["builders", "leaderboard", "--offset", "25"][..],
```

Append to the end of `polyoxide-cli/tests/data_v2.rs`:

```rust

// ── traded and builders ──────────────────────────────────────────────

#[tokio::test]
async fn traded_prints_the_user_stats_object() {
    let (query, run) = sent(
        "/v2/user-stats",
        &fixture("user_stats"),
        &["traded", "--user", "0xu"],
    )
    .await;
    assert_eq!(query, pairs(&[("user", "0xu")]));

    let printed: Value = serde_json::from_str(&run.out).unwrap();
    let received: Value = serde_json::from_str(&fixture("user_stats")).unwrap();
    assert_eq!(
        printed, received["data"],
        "the whole stats object, out of its envelope"
    );
    assert_eq!(printed["trades"], 2746, "`trades` counts distinct markets");
}

#[tokio::test]
async fn traded_prints_null_for_a_wallet_the_api_does_not_know() {
    let (_, run) = sent(
        "/v2/user-stats",
        &fixture("user_stats_unknown"),
        &["traded", "--user", "0xu"],
    )
    .await;
    assert_eq!(run.out, "null\n");
}

#[tokio::test]
async fn builders_leaderboard_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/builders/leaderboard",
        &fixture("builders_leaderboard"),
        &[
            "builders",
            "leaderboard",
            "--time-period",
            "week",
            "--limit",
            "5",
        ],
    )
    .await;
    assert_eq!(query, pairs(&[("time_period", "week"), ("limit", "5")]));
}

#[tokio::test]
async fn builders_volume_sends_the_period_as_its_interval() {
    let (query, _) = sent(
        "/v2/builders/volume",
        &fixture("builder_volume"),
        &[
            "builders",
            "volume",
            "--time-period",
            "month",
            "--limit",
            "12",
        ],
    )
    .await;
    assert_eq!(query, pairs(&[("interval", "month"), ("limit", "12")]));
}
```

Run:

```bash
cargo test -p polyoxide-cli --all-features --test data_v2
```

Expected: the run fails, with `test traded_prints_the_user_stats_object ... FAILED` and `test offset_is_refused_with_a_pointer_to_cursor ... FAILED` among the failures.

- [ ] **Step 2: Replace the two commands**

- **traded** reads `/v2/user-stats` and prints the stats object as pretty JSON: `proxy_wallet`, `trades` (the number of distinct markets traded), `biggest_win`, `views`, `join_date` and `all_time_pnl`. For a wallet v2 does not know (`data: null`) it prints `null`, which a script can tell apart from a known wallet whose counts are zero. v1's `{"user", "traded"}` shape is gone.
- **builders leaderboard** pages by cursor. **builders volume** sends `--time-period` as v2's `interval` (the bucket width) and gains `--limit`, the number of most recent buckets.

Replace the entire contents of `polyoxide-cli/src/commands/data/traded.rs`:

```rust
use std::io::Write;

use clap::Args;
use color_eyre::eyre::Result;
use polyoxide_data::DataApi;

use super::paging::print_pretty;

/// Get a user's profile stats (`/v2/user-stats`); `trades` counts distinct markets
#[derive(Args)]
pub struct TradedCommand {
    /// User address (0x-prefixed, 40 hex chars)
    #[arg(short, long)]
    user: String,
}

impl TradedCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write) -> Result<()> {
        let stats = data.v2().user_stats(&self.user).send().await?;
        // A wallet the API does not know has no stats: `data: null` prints as
        // `null`, which a script can tell apart from a known wallet's zeros.
        print_pretty(&stats, out)
    }
}
```

Replace the entire contents of `polyoxide-cli/src/commands/data/builders.rs`:

```rust
use std::io::Write;

use clap::{Args, Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_data::{v2::types::TimePeriod, DataApi};

use super::paging::{print_pretty, run_paged, PageArgs};

#[derive(Subcommand)]
pub enum BuildersCommand {
    /// Get the builder leaderboard (`/v2/builders/leaderboard`)
    Leaderboard(LeaderboardCommand),
    /// Get builder volume per time bucket (`/v2/builders/volume`)
    Volume(VolumeCommand),
}

impl BuildersCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        match self {
            Self::Leaderboard(cmd) => cmd.run(data, out, err).await,
            Self::Volume(cmd) => cmd.run(data, out).await,
        }
    }
}

/// Get the builder leaderboard
#[derive(Args)]
pub struct LeaderboardCommand {
    /// Time period for aggregation
    #[arg(short, long, default_value = "day")]
    pub time_period: CliTimePeriod,
    /// Page size (at most 1000)
    #[arg(short, long, default_value = "25")]
    pub limit: u32,
    #[command(flatten)]
    pub page: PageArgs,
}

impl LeaderboardCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        let request = data
            .v2()
            .builders_leaderboard()
            .time_period(self.time_period.into())
            .limit(self.limit);
        run_paged(request, &self.page, out, err).await
    }
}

/// Get builder volume per time bucket
#[derive(Args)]
pub struct VolumeCommand {
    /// Bucket width
    #[arg(short, long, default_value = "day")]
    pub time_period: CliTimePeriod,
    /// Most recent buckets to return (at most 90; API default 30)
    #[arg(short, long)]
    pub limit: Option<u32>,
}

impl VolumeCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write) -> Result<()> {
        let mut request = data.v2().builder_volume().interval(self.time_period.into());
        if let Some(limit) = self.limit {
            request = request.limit(limit);
        }
        print_pretty(&request.send().await?, out)
    }
}

/// Time period for aggregation
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum CliTimePeriod {
    /// Daily aggregation
    #[default]
    Day,
    /// Weekly aggregation
    Week,
    /// Monthly aggregation
    Month,
    /// All time aggregation
    All,
}

impl From<CliTimePeriod> for TimePeriod {
    fn from(period: CliTimePeriod) -> Self {
        match period {
            CliTimePeriod::Day => TimePeriod::Day,
            CliTimePeriod::Week => TimePeriod::Week,
            CliTimePeriod::Month => TimePeriod::Month,
            CliTimePeriod::All => TimePeriod::All,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestLeaderboard {
        #[command(flatten)]
        cmd: LeaderboardCommand,
    }

    #[derive(Parser)]
    struct TestVolume {
        #[command(flatten)]
        cmd: VolumeCommand,
    }

    #[test]
    fn leaderboard_defaults() {
        let parsed = TestLeaderboard::try_parse_from(["test"]).unwrap();
        assert!(matches!(parsed.cmd.time_period, CliTimePeriod::Day));
        assert_eq!(parsed.cmd.limit, 25);
        assert_eq!(parsed.cmd.page, PageArgs::default());
    }

    #[test]
    fn leaderboard_time_periods_parse() {
        for (flag, expected) in [
            ("week", TimePeriod::Week),
            ("month", TimePeriod::Month),
            ("all", TimePeriod::All),
        ] {
            let parsed = TestLeaderboard::try_parse_from(["test", "--time-period", flag]).unwrap();
            assert_eq!(TimePeriod::from(parsed.cmd.time_period), expected);
        }
    }

    #[test]
    fn leaderboard_invalid_time_period_errors() {
        assert!(TestLeaderboard::try_parse_from(["test", "--time-period", "year"]).is_err());
    }

    #[test]
    fn leaderboard_offset_is_refused_with_a_pointer_to_cursor() {
        let message = TestLeaderboard::try_parse_from(["test", "-l", "10", "-o", "50"])
            .err()
            .expect("offset must be refused")
            .to_string();
        assert!(message.contains("--cursor"), "{message}");
    }

    #[test]
    fn volume_defaults() {
        let parsed = TestVolume::try_parse_from(["test"]).unwrap();
        assert!(matches!(parsed.cmd.time_period, CliTimePeriod::Day));
        assert!(parsed.cmd.limit.is_none());
    }

    #[test]
    fn time_period_maps_to_the_v2_period() {
        assert_eq!(TimePeriod::from(CliTimePeriod::Day), TimePeriod::Day);
        assert_eq!(TimePeriod::from(CliTimePeriod::Week), TimePeriod::Week);
        assert_eq!(TimePeriod::from(CliTimePeriod::Month), TimePeriod::Month);
        assert_eq!(TimePeriod::from(CliTimePeriod::All), TimePeriod::All);
    }
}
```

- [ ] **Step 3: Dispatch them through the writers**

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
    /// Get traded markets by user
```

with:

```rust
    /// Get a user's profile stats, including the number of markets traded
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
            Self::Builders { command } => command.run(data).await,
```

with:

```rust
            Self::Builders { command } => command.run(data, out, err).await,
```

In `polyoxide-cli/src/commands/data/mod.rs`, replace:

```rust
            Self::Traded(cmd) => cmd.run(data).await,
```

with:

```rust
            Self::Traded(cmd) => cmd.run(data, out).await,
```

- [ ] **Step 4: Run the tests**

Run:

```bash
cargo test -p polyoxide-cli --all-features --lib
```

Expected: `test result: ok. 213 passed`.

Run:

```bash
cargo test -p polyoxide-cli --all-features --test data_v2
```

Expected: `test result: ok. 24 passed`.

- [ ] **Step 5: Prove the output shape is tested**

Print v1's `{user, traded}` shape, then print `{}` instead of `null` for an unknown wallet. Each must fail a named test. The backup restores the file, and `touch` matters: `mv` brings back the old modification time, so without it cargo keeps the mutated build and the next test run fails for no visible reason.

Run:

```bash
sed -i.bak 's/print_pretty(&stats, out)/print_pretty(\&serde_json::json!({"user": self.user, "traded": stats.map_or(0, |s| s.trades)}), out)/' polyoxide-cli/src/commands/data/traded.rs
cargo test -p polyoxide-cli --all-features --test data_v2 2>&1 | grep -E '^test .* FAILED'
mv polyoxide-cli/src/commands/data/traded.rs.bak polyoxide-cli/src/commands/data/traded.rs && touch polyoxide-cli/src/commands/data/traded.rs
```

Expected: `test traded_prints_the_user_stats_object ... FAILED` among the lines printed.

Run:

```bash
sed -i.bak 's/print_pretty(&stats, out)/print_pretty(\&stats.map_or(serde_json::json!({}), |s| serde_json::json!(s)), out)/' polyoxide-cli/src/commands/data/traded.rs
cargo test -p polyoxide-cli --all-features --test data_v2 2>&1 | grep -E '^test .* FAILED'
mv polyoxide-cli/src/commands/data/traded.rs.bak polyoxide-cli/src/commands/data/traded.rs && touch polyoxide-cli/src/commands/data/traded.rs
```

Expected: `test traded_prints_null_for_a_wallet_the_api_does_not_know ... FAILED` among the lines printed.

- [ ] **Step 6: Check that no data command still reaches v1**

`health` is the only v1 call left, by design:

Run:

```bash
grep -rn 'data\.\(user\|trades\|holders\|builders\|live_volume\|open_interest\|traded\|health\)()' polyoxide-cli/src/commands/data/ || true
```

Expected: exactly one line, `mod.rs: … data.health().check().await?;`.

- [ ] **Step 7: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-cli --all-features
```

Expected: every suite passes; `data_v2` reports `24 passed`.

- [ ] **Step 8: Commit**

```bash
git add polyoxide-cli/src/commands/data/builders.rs polyoxide-cli/src/commands/data/mod.rs polyoxide-cli/src/commands/data/traded.rs polyoxide-cli/tests/data_v2.rs
git commit -F - <<'EOF'
feat(cli)!: data traded and builders on Data API v2

traded reads /v2/user-stats and prints the stats object, whose trades
field is the distinct-market count, or null for a wallet v2 does not
know. The v1 {user, traded} output is gone. builders leaderboard reads
/v2/builders/leaderboard with cursor paging; builders volume reads
/v2/builders/volume, sending --time-period as its interval, and gains
--limit. Every data command but health now uses v2.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 6: Live tests for the v2 `data` commands

**Files:**
- Modify: `polyoxide-cli/tests/live_api.rs`

- [ ] **Step 1: Add the live tests**

Two tests, about a dozen requests in all. Inputs come from the live trade feed rather than hardcoded wallets. The nightly behavioral workflow already runs `polyoxide-cli --test live_api -- --ignored`, so no workflow change is needed.

Append to the end of `polyoxide-cli/tests/live_api.rs`:

```rust

// ── data (Data API v2) ───────────────────────────────────────────────
//
// Inputs are chosen live, never hardcoded: a wallet and market from the bare
// trade feed. Every listing asks for two rows, to keep the load light.

mod data_v2 {
    use clap::Parser;
    use polyoxide_cli::commands::DataCommand;
    use polyoxide_data::DataApi;
    use serde_json::Value;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        data: DataCommand,
    }

    /// Runs a `data` command against the live host and returns its stdout
    /// and stderr.
    async fn data(args: &[&str]) -> (String, String) {
        let cli = Cli::try_parse_from(std::iter::once("data").chain(args.iter().copied()))
            .unwrap_or_else(|e| panic!("{args:?}: {e}"));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        cli.data
            .run_with(&DataApi::new().unwrap(), &mut out, &mut err)
            .await
            .unwrap_or_else(|e| panic!("{args:?}: {e:?}"));
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn json(args: &[&str], out: &str) -> Value {
        serde_json::from_str(out).unwrap_or_else(|e| panic!("{args:?} printed non-JSON: {e}"))
    }

    #[tokio::test]
    #[ignore = "hits the real Polymarket API"]
    async fn live_data_commands_read_v2() {
        let args = ["trades", "list", "--limit", "1"];
        let feed = json(&args, &data(&args).await.0);
        let trade = &feed["data"][0];
        let wallet = trade["proxy_wallet"].as_str().expect("snake_case rows");
        let condition = trade["condition_id"].as_str().expect("snake_case rows");

        for args in [
            &["activity", "--user", wallet, "--limit", "2"][..],
            &["positions", "--user", wallet, "list", "--limit", "2"][..],
            &[
                "positions",
                "--user",
                wallet,
                "list",
                "--status",
                "closed",
                "--limit",
                "2",
            ][..],
            &["positions", "--user", wallet, "value"][..],
            &["holders", "--condition", condition, "--limit", "2"][..],
            &["open-interest", "--condition", condition][..],
            &["builders", "leaderboard", "--limit", "2"][..],
            &["builders", "volume", "--limit", "2"][..],
        ] {
            json(args, &data(args).await.0);
        }

        let args = ["traded", "--user", wallet];
        let stats = json(&args, &data(&args).await.0);
        assert_eq!(
            stats["proxy_wallet"].as_str().map(str::to_lowercase),
            Some(wallet.to_lowercase()),
            "a wallet that just traded has stats: {stats}"
        );
        assert!(stats["trades"].is_u64(), "{stats}");
    }

    #[tokio::test]
    #[ignore = "hits the real Polymarket API"]
    async fn live_data_walk_stops_at_max_pages_and_resumes_from_its_cursor() {
        let args = [
            "trades",
            "list",
            "--limit",
            "2",
            "--all",
            "--max-pages",
            "2",
        ];
        let (rows, err) = data(&args).await;
        assert_eq!(rows.lines().count(), 4, "two pages of two rows");
        for row in rows.lines() {
            json(&args, row);
        }
        let cursor = err
            .strip_prefix("next_cursor: ")
            .unwrap_or_else(|| panic!("no resume cursor on stderr: {err:?}"))
            .trim_end();

        let args = ["trades", "list", "--limit", "2", "--cursor", cursor];
        let resumed = json(&args, &data(&args).await.0);
        assert_eq!(resumed["data"].as_array().map(Vec::len), Some(2));
    }
}
```

Run:

```bash
cargo test -p polyoxide-cli --all-features --test live_api
```

Expected: `test result: ok. 0 passed; 0 failed; 3 ignored`.

- [ ] **Step 2: Run them against the live host, once**

Run:

```bash
cargo test -p polyoxide-cli --all-features --test live_api data_v2 -- --ignored --test-threads 1
```

Expected: `live_data_commands_read_v2 ... ok`, `live_data_walk_stops_at_max_pages_and_resumes_from_its_cursor ... ok`, `2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out`.

- [ ] **Step 3: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-cli/tests/live_api.rs
git commit -F - <<'EOF'
test(cli): live tests for the Data API v2 data commands

One test runs every ported command against a wallet and market taken from
the live trade feed and checks each prints JSON; the other walks two
pages with --max-pages, then resumes from the cursor it printed.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 7: Document the v2 `data` commands

**Files:**
- Modify: `polyoxide-cli/README.md`

- [ ] **Step 1: Rewrite the Data API section of the CLI README**

The README is not a doctest in this crate, so nothing compiles it; the examples below were run against the branch.

In `polyoxide-cli/README.md`, replace:

````markdown
### Data API

User positions, trades, and aggregate data. No authentication required.

#### `data health`

```bash
polyoxide data health
```

#### `data activity`

```bash
# List user activity (--user required)
polyoxide data activity --user 0xADDRESS
polyoxide data activity --user 0xADDRESS --activity-type trade,split
polyoxide data activity --user 0xADDRESS --side buy --sort-by tokens --sort-direction asc
```

#### `data positions`

```bash
# List open positions (--user required, then a subcommand)
polyoxide data positions --user 0xADDRESS list
polyoxide data positions --user 0xADDRESS list --redeemable --sort-by cash-pnl
polyoxide data positions --user 0xADDRESS list --market <CONDITION_ID> --title "search term"

# Total value of positions
polyoxide data positions --user 0xADDRESS value

# Closed positions
polyoxide data positions --user 0xADDRESS closed
polyoxide data positions --user 0xADDRESS closed --sort-by realized-pnl --limit 20

# User activity (same as top-level activity, scoped to user)
polyoxide data positions --user 0xADDRESS activity
```

#### `data trades`

```bash
# List global trades
polyoxide data trades list

# List trades for a specific user
polyoxide data trades list --user 0xADDRESS

# Filter by market, side, or amounts
polyoxide data trades list --market <CONDITION_ID> --side buy
polyoxide data trades list --filter-type cash --filter-amount 100
```

#### `data traded`

```bash
# Get markets traded by a user
polyoxide data traded --user 0xADDRESS
```

#### `data holders`

```bash
# Top holders for markets (comma-separated condition IDs)
polyoxide data holders --market <CONDITION_ID>
polyoxide data holders --market "id1,id2" --limit 50 --min-balance 10
```

#### `data builders`

```bash
# Builder leaderboard (time-period: day, week, month, all)
polyoxide data builders leaderboard
polyoxide data builders leaderboard --time-period week --limit 10

# Builder volume time series
polyoxide data builders volume
polyoxide data builders volume --time-period month
```

#### `data open-interest`

```bash
polyoxide data open-interest
polyoxide data open-interest --market <CONDITION_ID>
```

#### `data live-volume`

```bash
polyoxide data live-volume --event-id 42
```

````

with:

````markdown
### Data API

User positions, trades, and aggregate data, from Data API v2. No authentication
required.

**Output.** A single call prints the response envelope as pretty JSON:
`{"data": ..., "pagination": {..., "next_cursor": ...}}`. Field names are
snake_case (`proxy_wallet`, `condition_id`), as v2 returns them.

**Paging.** v2 pages by cursor only, so `--offset` is gone. Every listing
command takes:

- `--cursor <next_cursor>` to fetch the page after a previous one;
- `--all` to walk every page, writing one JSON row per line (JSONL);
- `--max-pages N`, with `--all`, to stop after N pages.

When a walk stops before the last page, whether at `--max-pages` or on an error,
the cursor to resume from is printed to stderr as `next_cursor: <cursor>`.

```bash
polyoxide data trades list --user 0xADDRESS --all > trades.jsonl
polyoxide data trades list --all --max-pages 5 2> resume.txt
polyoxide data trades list --all --cursor "$(sed -n 's/^next_cursor: //p' resume.txt)"
```

`--condition` takes comma-separated condition IDs; `--market` and `-m` are
aliases for it.

#### `data health`

```bash
polyoxide data health
```

#### `data activity`

```bash
# List user activity (--user required), newest first
polyoxide data activity --user 0xADDRESS
polyoxide data activity --user 0xADDRESS --activity-type trade,split,tip
polyoxide data activity --user 0xADDRESS --side buy --sort-direction asc

# Deposits and withdrawals are hidden unless asked for
polyoxide data activity --user 0xADDRESS --include-deposits-withdrawals

# Full history (the API's default window starts three years back)
polyoxide data activity --user 0xADDRESS --start 1 --all
```

#### `data positions`

```bash
# List open positions (--user required, then a subcommand)
polyoxide data positions --user 0xADDRESS list
polyoxide data positions --user 0xADDRESS list --condition <CONDITION_ID> --title "search term"

# Redeemable or closed positions
polyoxide data positions --user 0xADDRESS list --status redeemable
polyoxide data positions --user 0xADDRESS list --status closed --sort-by realized-pnl --limit 20

# Total value of positions
polyoxide data positions --user 0xADDRESS value

# User activity (same as top-level activity, scoped to user)
polyoxide data positions --user 0xADDRESS activity
```

#### `data trades`

```bash
# List global trades
polyoxide data trades list

# List trades for a specific user
polyoxide data trades list --user 0xADDRESS

# Filter by market, side, or amounts
polyoxide data trades list --condition <CONDITION_ID> --side buy
polyoxide data trades list --filter-type cash --filter-amount 100
```

#### `data traded`

```bash
# A user's profile stats; `trades` is the number of distinct markets traded.
# Prints `null` for a wallet the API does not know.
polyoxide data traded --user 0xADDRESS
```

#### `data holders`

```bash
# Top holders per outcome token (--condition required, comma-separated)
polyoxide data holders --condition <CONDITION_ID>
polyoxide data holders --condition "id1,id2" --limit 50 --min-balance 10

# Add each holder's entry cost and P&L (one condition only)
polyoxide data holders --condition <CONDITION_ID> --include-pnl
```

#### `data builders`

```bash
# Builder leaderboard (time-period: day, week, month, all)
polyoxide data builders leaderboard
polyoxide data builders leaderboard --time-period week --limit 10

# Builder volume per bucket (time-period is the bucket width)
polyoxide data builders volume
polyoxide data builders volume --time-period month --limit 12
```

#### `data open-interest`

```bash
polyoxide data open-interest
polyoxide data open-interest --condition <CONDITION_ID>
```

#### `data live-volume`

```bash
polyoxide data live-volume --event-id 42
polyoxide data live-volume --event-id 42,43
```

````

- [ ] **Step 2: Final verification**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-cli --all-features
```

Expected: every suite passes: `213` lib, `18` main, `24` `data_v2`, `3 ignored` live.

Run:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features -p polyoxide-cli
```

Expected: finishes with no warnings.

The refusals, from the real binary (no network needed):

Run:

```bash
cargo run -q -p polyoxide-cli -- data trades list --offset 100; echo "exit $?"
```

Expected: `error: invalid value '100' for '--offset <OFFSET>': --offset was removed: the Data API v2 pages by cursor. Pass --cursor <next_cursor> from a previous page, or --all`, then `exit 2`.

Run:

```bash
cargo run -q -p polyoxide-cli -- data positions --user 0x0 closed; echo "exit $?"
```

Expected: an error reading `` `positions closed` was removed: use `positions list --status closed` ``, then `exit 1`.

A light live smoke run, one page each:

Run:

```bash
cargo run -q -p polyoxide-cli -- data trades list --limit 2
cargo run -q -p polyoxide-cli -- data trades list --limit 2 --all --max-pages 2 > /dev/null
```

Expected: a pretty `{"data": [two trades], "pagination": {…, "next_cursor": "…"}}`; then only `next_cursor: …` on the terminal (stderr), since the rows went to `/dev/null`.

Then the phase exit gates across the workspace, as in the design's Phasing section. Only `polyoxide-cli` changed, so these confirm nothing else moved:

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

Expected: all three finish clean. If rustc is killed with signal 15 or exit 254, that is earlyoom on this machine, not a failure; re-run with `-j4`.

- [ ] **Step 3: Commit**

```bash
git add polyoxide-cli/README.md
git commit -F - <<'EOF'
docs(cli): document the Data API v2 data commands

The README's Data API section covers the v2 envelope and snake_case
output, cursor paging with --all and --max-pages, the resume cursor on
stderr, and the renamed and new flags.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```

---

## After this plan

This plan edits only `polyoxide-cli/`. These documentation changes belong outside it and go in with the phase's PR.

**`CLAUDE.md`**, after the paragraph on the CLI's `clob` command group:

> The CLI's `data` command group reads Data API v2, except `data health`, which stays on v1's `/` because `/v2/status` reports data freshness, not liveness. Listing commands print the v2 `{data, pagination}` envelope and page with `--cursor`, `--all` (JSONL, flushed per page) and `--max-pages`. The cursor to resume from goes to stderr when a walk stops early. `--offset` is refused with a pointer to `--cursor`. `DataCommand::run_with` takes the client and the output writers, and `polyoxide-cli/tests/data_v2.rs` uses it to run real arguments against mock servers serving `polyoxide-data`'s v2 fixtures.
>
> **A clap `Vec<String>` field needs `value_delimiter`, not a value parser that returns `Vec`.** The latter compiles and parses, then panics when the field is read. Every v1 `data` list flag shipped that way, and no test caught it because the parse tests never passed those flags.

**`CHANGELOG.md`**, in the release intro. git-cliff lists the `feat(cli)!` commits as breaking on its own, but the intro should say what breaks for scripts:

> Breaking for scripts: `polyoxide data` commands now read Data API v2. Output is the v2 `{data, pagination}` envelope with snake_case fields (`proxy_wallet`, not `proxyWallet`). `--offset` is replaced by `--cursor`, `--all` and `--max-pages`, `positions closed` by `positions list --status closed`, and `holders` requires `--condition`. `data traded` prints the `/v2/user-stats` object (its `trades` field is the distinct-market count), or `null` for a wallet the API does not know, instead of `{user, traded}`. `data health` is unchanged. Comma-separated `--market` and `--event-id` values, which panicked in earlier releases, now work.

## Spec coverage

| 6.2 requirement | Task |
|-----------------|------|
| `activity`, `builders`, `holders`, `trades`, `open-interest`, `live-volume` on their v2 routes | 2, 3, 4, 5 |
| `positions`: `--status` on `/v2/positions`; `value` on `/v2/value` | 3 |
| `traded` on `/v2/user-stats`, printing the distinct-market count (as `trades`, in the full stats object, per decision 3) | 5 |
| `health` stays on v1 | 2 (dispatch), 5 (grep check) |
| `--offset` removed, with a clap error pointing to `--cursor` | 1 (parser), 2–5 (mock test per paged command), 7 (binary) |
| `--condition` replaces `--market`, which stays as an alias | 2, 3, 4 |
| Single call prints the envelope as pretty JSON | 1, 2 (`a_single_page_prints_the_envelope_as_received`) |
| `--all` writes JSONL, flushed per page | 1, 2, 6 |
| Last `next_cursor` to stderr on `--max-pages` or an error | 1, 2, 6 |
| No commands for routes new in v2 | none added |
| Changelog marks the snake_case output change as breaking | `feat(cli)!` commits; intro text above |
| Exit: CLI unit tests and live CLI tests green | 2–5 gates, 6, 7 |
