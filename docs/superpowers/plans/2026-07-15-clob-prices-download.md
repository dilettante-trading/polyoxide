# CLOB Prices Download Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `polyoxide clob prices download` CLI command that bulk-downloads Polymarket CLOB historical price data to per-market dataset files (CSV/JSONL/Parquet) for offline ML.

**Architecture:** Per-market concurrent fetch (approach A). Each market maps 1:1 to one `GET /prices-history` request, one output file, and one manifest row. A bounded `buffer_unordered` worker pool (`--concurrency`) is gated by a shared `governor` rate limiter under the `1000 req/10s` API limit; each fetch retries with exponential backoff. Resume = skip markets whose output file already exists. Writes are atomic (temp file + rename). Market selection is the deduped union of explicit IDs, an input file, and Gamma discovery filters.

**Tech Stack:** Rust, `clap` (derive), `tokio`, `futures-util` (`buffer_unordered`), `governor` (rate limiting), `serde_json`, `tempfile` (atomic writes + test dirs), `mockito` (mock HTTP tests), `arrow`/`parquet` (feature-gated). Builds on `polyoxide-clob` (`Clob::public()`, `markets().prices_history`) and `polyoxide-gamma` (`markets().list()`).

**Spec:** `docs/superpowers/specs/2026-07-15-clob-prices-download-design.md`

---

## File Structure

**Library (additive, semver-clean):**
- Modify `polyoxide-clob/src/api/markets.rs` — add `PricesHistoryQuery` + `prices_history_with`; refactor `prices_history` to delegate.
- Modify `polyoxide-clob/src/lib.rs` — re-export `PricesHistoryQuery`, `PricesHistoryResponse`, `PriceHistoryPoint`.

**CLI (new `clob` command group):**
- Modify `polyoxide-cli/Cargo.toml` — add `governor` dep, `parquet` feature, dev-deps.
- Modify `polyoxide-cli/src/main.rs` — wire `Clob` subcommand.
- Modify `polyoxide-cli/src/commands/mod.rs` — declare + re-export `clob`.
- Create `polyoxide-cli/src/commands/clob/mod.rs` — `ClobCommand` enum, client construction.
- Create `polyoxide-cli/src/commands/clob/prices/mod.rs` — `PricesCommand::Download`.
- Create `polyoxide-cli/src/commands/clob/prices/types.rs` — `OutputFormat`, `Target`, `ManifestRecord`.
- Create `polyoxide-cli/src/commands/clob/prices/select.rs` — id file parsing, dedup, discovery.
- Create `polyoxide-cli/src/commands/clob/prices/writer.rs` — `DatasetWriter` trait + Csv/Jsonl + `atomic_write`.
- Create `polyoxide-cli/src/commands/clob/prices/fetch.rs` — `fetch_one` (rate limit + retry).
- Create `polyoxide-cli/src/commands/clob/prices/manifest.rs` — `write_manifest`.
- Create `polyoxide-cli/src/commands/clob/prices/download.rs` — `DownloadArgs` + `run` orchestration.
- Create `polyoxide-cli/tests/prices_download.rs` — mockito end-to-end + resume tests.

Each `prices/*.rs` file has one responsibility; `download.rs` is the orchestrator that composes them.

---

## Task 1: Library — extend `prices_history` with query params

**Files:**
- Modify: `polyoxide-clob/src/api/markets.rs` (method at lines 88-97; add struct near `PricesHistoryResponse` ~line 493)
- Modify: `polyoxide-clob/src/lib.rs:84-91` (re-export block)
- Test: `polyoxide-clob/tests/mock_api.rs` (near existing `prices_history_renamed_fields` ~line 647)

- [ ] **Step 1: Write the failing test**

Add to `polyoxide-clob/tests/mock_api.rs`:

```rust
#[tokio::test]
async fn prices_history_with_query_params() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("market".into(), "0xtoken".into()),
            Matcher::UrlEncoded("interval".into(), "max".into()),
            Matcher::UrlEncoded("fidelity".into(), "1".into()),
            Matcher::UrlEncoded("startTs".into(), "1700000000".into()),
            Matcher::UrlEncoded("endTs".into(), "1700900000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let query = polyoxide_clob::PricesHistoryQuery {
        interval: Some("max".into()),
        fidelity: Some(1),
        start_ts: Some(1_700_000_000),
        end_ts: Some(1_700_900_000),
    };
    let resp = clob
        .markets()
        .prices_history_with("0xtoken", &query)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.history.len(), 1);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p polyoxide-clob --all-features -- prices_history_with_query_params`
Expected: FAIL — `PricesHistoryQuery` and `prices_history_with` do not exist (compile error).

- [ ] **Step 3: Add the struct and method**

In `polyoxide-clob/src/api/markets.rs`, add the struct immediately above `pub struct PricesHistoryResponse` (~line 493):

```rust
/// Optional query parameters for the `/prices-history` endpoint.
///
/// All fields are optional; only `Some` values are sent. See
/// `docs/specs/clob/markets.md` for the accepted `interval` values and the
/// `fidelity` (minutes) meaning.
#[derive(Debug, Clone, Default)]
pub struct PricesHistoryQuery {
    /// Aggregation window: `max`, `all`, `1m`, `1w`, `1d`, `6h`, or `1h`.
    pub interval: Option<String>,
    /// Resolution in minutes (upstream default is 1).
    pub fidelity: Option<i32>,
    /// Inclusive start of the window as a UNIX timestamp (seconds).
    pub start_ts: Option<i64>,
    /// Inclusive end of the window as a UNIX timestamp (seconds).
    pub end_ts: Option<i64>,
}
```

Then replace the existing `prices_history` method (lines 88-97) with:

```rust
    /// Get historical prices for a token (no extra filters).
    pub fn prices_history(&self, token_id: impl Into<String>) -> Request<PricesHistoryResponse> {
        self.prices_history_with(token_id, &PricesHistoryQuery::default())
    }

    /// Get historical prices for a token with optional interval/fidelity/time bounds.
    pub fn prices_history_with(
        &self,
        token_id: impl Into<String>,
        params: &PricesHistoryQuery,
    ) -> Request<PricesHistoryResponse> {
        Request::get(
            self.http_client.clone(),
            "/prices-history",
            AuthMode::None,
            self.chain_id,
        )
        .query("market", token_id.into())
        .query_opt("interval", params.interval.clone())
        .query_opt("fidelity", params.fidelity)
        .query_opt("startTs", params.start_ts)
        .query_opt("endTs", params.end_ts)
    }
```

(`query_opt` is a provided method on the `QueryBuilder` trait already imported at `markets.rs:3`.)

- [ ] **Step 4: Re-export the new + supporting types**

In `polyoxide-clob/src/lib.rs`, edit the `markets::{ ... }` re-export block (lines 84-90) to add `PriceHistoryPoint`, `PricesHistoryQuery`, and `PricesHistoryResponse`. The block becomes:

```rust
    markets::{
        BatchPricesHistoryRequest, BatchPricesHistoryResponse, BookParams, CalculatePriceResponse,
        ClobMarketDetails, ClobRewards, ClobToken, FeeDetails, LastTradePriceResponse,
        ListMarketsResponse, LiveActivityEvent, LiveActivityMarket, Market, MarketByTokenResponse,
        MarketPrice, MarketToken, MidpointResponse, OrderBook, OrderLevel, PriceHistoryPoint,
        PriceResponse, PricesHistoryQuery, PricesHistoryResponse, SpreadResponse,
    },
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p polyoxide-clob --all-features -- prices_history`
Expected: PASS (both `prices_history_renamed_fields` and `prices_history_with_query_params`).

- [ ] **Step 6: Lint**

Run: `cargo clippy -p polyoxide-clob --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add polyoxide-clob/src/api/markets.rs polyoxide-clob/src/lib.rs polyoxide-clob/tests/mock_api.rs
git commit -m "feat(clob): add PricesHistoryQuery + prices_history_with for time-bounded history"
```

---

## Task 2: CLI — dependencies + `clob` command scaffold

**Files:**
- Modify: `polyoxide-cli/Cargo.toml`
- Modify: `polyoxide-cli/src/commands/mod.rs`
- Modify: `polyoxide-cli/src/main.rs`
- Create: `polyoxide-cli/src/commands/clob/mod.rs`
- Create: `polyoxide-cli/src/commands/clob/prices/mod.rs`
- Create: `polyoxide-cli/src/commands/clob/prices/download.rs`
- Test: `polyoxide-cli/src/main.rs` (existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Add dependencies and feature to `polyoxide-cli/Cargo.toml`**

Under `[features]`, add a `parquet` feature (leave `default` unchanged):

```toml
[features]
default = []
keychain = ["polyoxide-clob/keychain", "dep:polyoxide-core", "dep:polyoxide-relay"]
parquet = ["dep:arrow", "dep:parquet"]
```

Under `[dependencies]`, add (after `ctrlc = "3.4"`):

```toml
governor = { workspace = true }
tempfile = "3"
arrow = { version = "54", optional = true }
parquet = { version = "54", optional = true }
```

Add a new `[dev-dependencies]` section at the end of the file:

```toml
[dev-dependencies]
mockito = { workspace = true }
tempfile = "3"
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time"] }
```

- [ ] **Step 2: Write the failing parse tests**

Add to the `#[cfg(test)] mod tests` block in `polyoxide-cli/src/main.rs`:

```rust
    #[test]
    fn clob_subcommand_requires_nested_subcommand() {
        let result = try_parse(&["polyoxide", "clob"]);
        assert!(result.is_err());
    }

    #[test]
    fn clob_prices_download_parses_with_token_id() {
        let cli = try_parse(&[
            "polyoxide", "clob", "prices", "download", "--token-id", "0xabc",
        ])
        .unwrap();
        assert!(matches!(cli.command, super::Commands::Clob { .. }));
    }

    #[test]
    fn clob_prices_download_defaults_interval_max() {
        use crate::commands::clob::{ClobCommand, prices::PricesCommand};
        let cli = try_parse(&[
            "polyoxide", "clob", "prices", "download", "--token-id", "0xabc",
        ])
        .unwrap();
        let super::Commands::Clob { command: ClobCommand::Prices { command } } = cli.command else {
            panic!("expected clob prices");
        };
        let PricesCommand::Download(args) = command;
        assert_eq!(args.interval, "max");
        assert_eq!(args.fidelity, 1);
        assert_eq!(args.concurrency, 4);
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p polyoxide-cli -- clob_`
Expected: FAIL — `clob` module / `Commands::Clob` do not exist (compile error).

- [ ] **Step 4: Create `polyoxide-cli/src/commands/clob/prices/download.rs`**

```rust
use clap::Args;
use color_eyre::eyre::Result;
use polyoxide_clob::Clob;

use crate::commands::clob::prices::types::OutputFormat;

/// Bulk-download CLOB historical price data to per-market dataset files.
#[derive(Args, Debug)]
pub struct DownloadArgs {
    /// Explicit token IDs to download (repeatable).
    #[arg(long = "token-id")]
    pub token_ids: Vec<String>,

    /// File of newline-delimited token IDs (`#` comments and blank lines ignored).
    #[arg(short, long)]
    pub input: Option<std::path::PathBuf>,

    /// Enable Gamma-based market discovery using the filters below.
    #[arg(long)]
    pub discover: bool,

    /// Discovery: only closed (`true`) or only open (`false`) markets.
    #[arg(long)]
    pub closed: Option<bool>,

    /// Discovery: only open markets (`true`).
    #[arg(long)]
    pub open: Option<bool>,

    /// Discovery: minimum cumulative volume.
    #[arg(long = "min-volume")]
    pub min_volume: Option<f64>,

    /// Discovery: minimum liquidity.
    #[arg(long = "min-liquidity")]
    pub min_liquidity: Option<f64>,

    /// Discovery: filter by numeric Gamma tag id.
    #[arg(long = "tag-id")]
    pub tag_id: Option<i64>,

    /// Discovery: max markets to pull from Gamma (upstream caps at 1000/page).
    #[arg(long = "discover-limit")]
    pub discover_limit: Option<u32>,

    /// Aggregation interval: max, all, 1m, 1w, 1d, 6h, 1h.
    #[arg(long, default_value = "max")]
    pub interval: String,

    /// Resolution in minutes.
    #[arg(long, default_value_t = 1)]
    pub fidelity: i32,

    /// Inclusive window start (UNIX seconds).
    #[arg(long = "start-ts")]
    pub start_ts: Option<i64>,

    /// Inclusive window end (UNIX seconds).
    #[arg(long = "end-ts")]
    pub end_ts: Option<i64>,

    /// Output directory.
    #[arg(short, long, default_value = "./polymarket-prices")]
    pub out: std::path::PathBuf,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Csv)]
    pub format: OutputFormat,

    /// Number of markets to fetch concurrently.
    #[arg(long, default_value_t = 4)]
    pub concurrency: usize,

    /// Re-fetch and overwrite even if the output file already exists.
    #[arg(long)]
    pub overwrite: bool,

    /// Abort the whole run on the first market failure.
    #[arg(long = "fail-fast")]
    pub fail_fast: bool,

    /// Resolve and print the target list without fetching.
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

impl DownloadArgs {
    pub async fn run(self) -> Result<()> {
        // Orchestration is implemented in Task 9. For now, construct the client
        // so the wiring compiles and the command is reachable.
        let _clob = Clob::public();
        println!("clob prices download: {} explicit id(s)", self.token_ids.len());
        Ok(())
    }
}
```

- [ ] **Step 5: Create `polyoxide-cli/src/commands/clob/prices/mod.rs`**

```rust
pub mod download;
pub mod types;

use clap::Subcommand;
use color_eyre::eyre::Result;

pub use download::DownloadArgs;

#[derive(Subcommand)]
pub enum PricesCommand {
    /// Bulk-download historical price data to dataset files.
    Download(DownloadArgs),
}

impl PricesCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Download(args) => args.run().await,
        }
    }
}
```

- [ ] **Step 6: Create a minimal `polyoxide-cli/src/commands/clob/prices/types.rs`**

(Full contents land in Task 3; this minimal version unblocks Task 2's compile.)

```rust
use clap::ValueEnum;

/// Output format for downloaded datasets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    /// Comma-separated values with a header row.
    #[default]
    Csv,
    /// Newline-delimited JSON objects.
    Jsonl,
    /// Apache Parquet (requires building with the `parquet` feature).
    Parquet,
}
```

- [ ] **Step 7: Create `polyoxide-cli/src/commands/clob/mod.rs`**

```rust
pub mod prices;

use clap::Subcommand;
use color_eyre::eyre::Result;

use prices::PricesCommand;

#[derive(Subcommand)]
pub enum ClobCommand {
    /// Historical price data.
    Prices {
        #[command(subcommand)]
        command: PricesCommand,
    },
}

impl ClobCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Prices { command } => command.run().await,
        }
    }
}
```

- [ ] **Step 8: Declare + re-export the module in `polyoxide-cli/src/commands/mod.rs`**

Add after the `pub mod gamma;` line:

```rust
pub mod clob;
```

Add after the `pub use gamma::GammaCommand;` line:

```rust
pub use clob::ClobCommand;
```

- [ ] **Step 9: Wire the subcommand in `polyoxide-cli/src/main.rs`**

In `enum Commands`, add after the `Data { ... }` variant:

```rust
    /// Query CLOB API (order book, historical prices)
    Clob {
        #[command(subcommand)]
        command: commands::ClobCommand,
    },
```

In `main`'s `match cli.command`, add after the `Commands::Data` arm:

```rust
        Commands::Clob { command } => command.run().await?,
```

- [ ] **Step 10: Run tests to verify they pass**

Run: `cargo test -p polyoxide-cli -- clob_`
Expected: PASS (3 new tests).

- [ ] **Step 11: Build + lint**

Run: `cargo clippy -p polyoxide-cli --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 12: Commit**

```bash
git add polyoxide-cli/Cargo.toml polyoxide-cli/src/main.rs polyoxide-cli/src/commands/mod.rs polyoxide-cli/src/commands/clob
git commit -m "feat(cli): scaffold clob prices download command group"
```

---

## Task 3: `types.rs` — OutputFormat, Target, ManifestRecord

**Files:**
- Modify: `polyoxide-cli/src/commands/clob/prices/types.rs`
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-cli/src/commands/clob/prices/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn extension_matches_format() {
        assert_eq!(OutputFormat::Csv.extension(), "csv");
        assert_eq!(OutputFormat::Jsonl.extension(), "jsonl");
        assert_eq!(OutputFormat::Parquet.extension(), "parquet");
    }

    #[test]
    fn output_path_joins_dir_id_and_extension() {
        let t = Target { token_id: "0xabc".into() };
        let p = t.output_path(Path::new("/data/out"), OutputFormat::Csv);
        assert_eq!(p, Path::new("/data/out/0xabc.csv"));
    }

    #[test]
    fn manifest_record_serializes_status_and_nulls() {
        let rec = ManifestRecord {
            token_id: "0xabc".into(),
            path: "out/0xabc.csv".into(),
            points: 0,
            first_ts: None,
            last_ts: None,
            status: "empty".into(),
            error: None,
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"status\":\"empty\""));
        assert!(json.contains("\"error\":null"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p polyoxide-cli -- clob::prices::types`
Expected: FAIL — `extension`, `Target`, `ManifestRecord` do not exist.

- [ ] **Step 3: Implement the types**

Replace the whole body of `polyoxide-cli/src/commands/clob/prices/types.rs` above the test module with:

```rust
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::Serialize;

/// Output format for downloaded datasets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    /// Comma-separated values with a header row.
    #[default]
    Csv,
    /// Newline-delimited JSON objects.
    Jsonl,
    /// Apache Parquet (requires building with the `parquet` feature).
    Parquet,
}

impl OutputFormat {
    /// File extension (without a dot) for this format.
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Csv => "csv",
            OutputFormat::Jsonl => "jsonl",
            OutputFormat::Parquet => "parquet",
        }
    }
}

/// A single market to download, identified by its CLOB token id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub token_id: String,
}

impl Target {
    /// Output file path for this target: `<out_dir>/<token_id>.<ext>`.
    ///
    /// Token ids are decimal integer strings, so they are filesystem-safe as-is.
    pub fn output_path(&self, out_dir: &Path, format: OutputFormat) -> PathBuf {
        out_dir.join(format!("{}.{}", self.token_id, format.extension()))
    }
}

/// One manifest row per considered market. `status` is `ok` | `empty` |
/// `failed` | `skipped`.
#[derive(Debug, Clone, Serialize)]
pub struct ManifestRecord {
    pub token_id: String,
    pub path: String,
    pub points: usize,
    pub first_ts: Option<i64>,
    pub last_ts: Option<i64>,
    pub status: String,
    pub error: Option<String>,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p polyoxide-cli -- clob::prices::types`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-cli/src/commands/clob/prices/types.rs
git commit -m "feat(cli): prices download domain types (format, target, manifest)"
```

---

## Task 4: `select.rs` — id file parsing, dedup, token-id parsing

**Files:**
- Create: `polyoxide-cli/src/commands/clob/prices/select.rs`
- Modify: `polyoxide-cli/src/commands/clob/prices/mod.rs` (add `pub mod select;`)
- Test: `select.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Declare the module**

In `polyoxide-cli/src/commands/clob/prices/mod.rs`, add after `pub mod download;`:

```rust
pub mod select;
```

- [ ] **Step 2: Write the failing tests**

Create `polyoxide-cli/src/commands/clob/prices/select.rs` with only the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn dedupe_preserves_first_seen_order() {
        let targets = dedupe_targets(
            ["b", "a", "b", "c", "a"].into_iter().map(String::from),
        );
        let ids: Vec<&str> = targets.iter().map(|t| t.token_id.as_str()).collect();
        assert_eq!(ids, ["b", "a", "c"]);
    }

    #[test]
    fn read_ids_file_skips_blanks_and_comments() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "0xabc").unwrap();
        writeln!(f, "  # a comment").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "  0xdef  ").unwrap();
        let ids = read_ids_file(f.path()).unwrap();
        assert_eq!(ids, ["0xabc", "0xdef"]);
    }

    #[test]
    fn parse_clob_token_ids_parses_json_array() {
        let ids = parse_clob_token_ids(r#"["111","222"]"#).unwrap();
        assert_eq!(ids, ["111", "222"]);
    }

    #[test]
    fn parse_clob_token_ids_rejects_malformed() {
        assert!(parse_clob_token_ids("not json").is_err());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p polyoxide-cli -- clob::prices::select`
Expected: FAIL — functions not defined.

- [ ] **Step 4: Implement the functions**

Prepend to `select.rs` (above the test module):

```rust
use std::path::Path;

use color_eyre::eyre::{Context, Result};

use crate::commands::clob::prices::types::Target;

/// Deduplicate token ids into `Target`s, preserving first-seen order.
pub fn dedupe_targets(ids: impl IntoIterator<Item = String>) -> Vec<Target> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        if seen.insert(id.clone()) {
            out.push(Target { token_id: id });
        }
    }
    out
}

/// Read a newline-delimited token-id file. Blank lines and lines whose first
/// non-whitespace character is `#` are ignored; other lines are trimmed.
pub fn read_ids_file(path: &Path) -> Result<Vec<String>> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("reading token id file {}", path.display()))?;
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect())
}

/// Parse a Gamma `clobTokenIds` field, which is a JSON-encoded string array
/// such as `"[\"111\",\"222\"]"`.
pub fn parse_clob_token_ids(raw: &str) -> Result<Vec<String>> {
    serde_json::from_str::<Vec<String>>(raw)
        .with_context(|| format!("parsing clob_token_ids: {raw}"))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p polyoxide-cli -- clob::prices::select`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add polyoxide-cli/src/commands/clob/prices/select.rs polyoxide-cli/src/commands/clob/prices/mod.rs
git commit -m "feat(cli): prices download selection helpers (file parse, dedupe, token ids)"
```

---

## Task 5: `select.rs` — Gamma discovery

**Files:**
- Modify: `polyoxide-cli/src/commands/clob/prices/select.rs`
- Test: `polyoxide-cli/tests/prices_discovery.rs` (mockito)

- [ ] **Step 1: Write the failing mock test**

Create `polyoxide-cli/tests/prices_discovery.rs`:

```rust
use mockito::{Matcher, Server};
use polyoxide_cli::commands::clob::prices::select::{discover_targets, DiscoverFilters};

#[tokio::test]
async fn discover_extracts_token_ids_from_markets() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/markets")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("closed".into(), "false".into()),
            Matcher::UrlEncoded("volume_num_min".into(), "1000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"id":"1","clobTokenIds":"[\"111\",\"222\"]"},
                {"id":"2","clobTokenIds":"[\"333\"]"},
                {"id":"3"}
            ]"#,
        )
        .create_async()
        .await;

    let gamma = polyoxide_gamma::Gamma::builder()
        .base_url(server.url())
        .build()
        .unwrap();

    let filters = DiscoverFilters {
        open: Some(true),
        min_volume: Some(1000.0),
        ..Default::default()
    };
    let ids = discover_targets(&gamma, &filters).await.unwrap();

    // Market 3 has no clobTokenIds and is skipped.
    assert_eq!(ids, ["111", "222", "333"]);
    mock.assert_async().await;
}
```

This test uses the crate as a library, which requires a `lib.rs`. If `polyoxide-cli` has no `src/lib.rs`, add one exposing the modules:

Create `polyoxide-cli/src/lib.rs`:

```rust
//! Library surface for integration tests. The binary entry point is `main.rs`.
pub mod commands;
```

And confirm `polyoxide-cli/Cargo.toml` `[[bin]]` still points at `src/main.rs` (it does). In `main.rs`, replace `mod commands;` with `use polyoxide_cli::commands;` so the binary and lib share one module tree. Add the package `[lib]` name if needed (defaults to `polyoxide_cli`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p polyoxide-cli --test prices_discovery`
Expected: FAIL — `discover_targets` / `DiscoverFilters` not defined (compile error).

- [ ] **Step 3: Implement discovery**

Append to `polyoxide-cli/src/commands/clob/prices/select.rs` (above the test module):

```rust
use polyoxide_gamma::Gamma;

/// Gamma market-discovery filters. All fields optional.
#[derive(Debug, Clone, Default)]
pub struct DiscoverFilters {
    pub closed: Option<bool>,
    pub open: Option<bool>,
    pub min_volume: Option<f64>,
    pub min_liquidity: Option<f64>,
    pub tag_id: Option<i64>,
    pub limit: Option<u32>,
}

/// Discover token ids via `gamma.markets().list()`, applying the given filters.
///
/// Each returned market's `clob_token_ids` (a JSON-encoded string array) is
/// parsed and flattened. Markets with a missing or unparseable field are
/// skipped with a warning to stderr.
pub async fn discover_targets(gamma: &Gamma, filters: &DiscoverFilters) -> Result<Vec<String>> {
    let mut req = gamma.markets().list();
    if let Some(closed) = filters.closed {
        req = req.closed(closed);
    }
    if let Some(open) = filters.open {
        req = req.open(open);
    }
    if let Some(v) = filters.min_volume {
        req = req.volume_num_min(v);
    }
    if let Some(l) = filters.min_liquidity {
        req = req.liquidity_num_min(l);
    }
    if let Some(tag) = filters.tag_id {
        req = req.tag_id(tag);
    }
    if let Some(limit) = filters.limit {
        req = req.limit(limit);
    }

    let markets = req.send().await.context("gamma market discovery")?;

    let mut ids = Vec::new();
    for market in markets {
        match market.clob_token_ids.as_deref() {
            Some(raw) => match parse_clob_token_ids(raw) {
                Ok(parsed) => ids.extend(parsed),
                Err(e) => eprintln!("warning: skipping market {}: {e}", market.id),
            },
            None => eprintln!(
                "warning: skipping market {} (no clob_token_ids)",
                market.id
            ),
        }
    }
    Ok(ids)
}
```

(`market.id` is `pub id: String` on `polyoxide_gamma::types::Market` — confirmed — so the warning string compiles as written.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p polyoxide-cli --test prices_discovery`
Expected: PASS.

- [ ] **Step 5: Lint**

Run: `cargo clippy -p polyoxide-cli --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-cli/src/lib.rs polyoxide-cli/src/main.rs polyoxide-cli/src/commands/clob/prices/select.rs polyoxide-cli/tests/prices_discovery.rs
git commit -m "feat(cli): Gamma-based market discovery for prices download"
```

---

## Task 6: `writer.rs` — DatasetWriter trait, CSV/JSONL, atomic write

**Files:**
- Create: `polyoxide-cli/src/commands/clob/prices/writer.rs`
- Modify: `polyoxide-cli/src/commands/clob/prices/mod.rs` (add `pub mod writer;`)
- Test: `writer.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Declare the module**

In `mod.rs`, add `pub mod writer;`.

- [ ] **Step 2: Write the failing tests**

Create `polyoxide-cli/src/commands/clob/prices/writer.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_clob::PriceHistoryPoint;

    fn points() -> Vec<PriceHistoryPoint> {
        vec![
            PriceHistoryPoint { timestamp: 1700000000, price: 0.55 },
            PriceHistoryPoint { timestamp: 1700001000, price: 0.60 },
        ]
    }

    #[test]
    fn csv_serializes_header_and_rows() {
        let mut buf = Vec::new();
        CsvWriter.serialize(&mut buf, "0xabc", &points()).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(
            text,
            "token_id,timestamp,price\n0xabc,1700000000,0.55\n0xabc,1700001000,0.6\n"
        );
    }

    #[test]
    fn jsonl_serializes_one_object_per_line() {
        let mut buf = Vec::new();
        JsonlWriter.serialize(&mut buf, "0xabc", &points()).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"token_id":"0xabc","t":1700000000,"p":0.55}"#);
    }

    #[test]
    fn atomic_write_creates_file_with_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0xabc.csv");
        atomic_write(&CsvWriter, &path, "0xabc", &points()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("token_id,timestamp,price\n"));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p polyoxide-cli -- clob::prices::writer`
Expected: FAIL — `CsvWriter`, `JsonlWriter`, `atomic_write` not defined.

- [ ] **Step 4: Implement the writer**

Prepend to `writer.rs`:

```rust
use std::io::{self, Write};
use std::path::Path;

use color_eyre::eyre::{Context, Result};
use polyoxide_clob::PriceHistoryPoint;

use crate::commands::clob::prices::types::OutputFormat;

/// Serializes a market's price points into a byte sink. Separating
/// serialization from the filesystem keeps the format logic unit-testable.
pub trait DatasetWriter {
    fn serialize(
        &self,
        out: &mut dyn Write,
        token_id: &str,
        points: &[PriceHistoryPoint],
    ) -> io::Result<()>;
}

/// CSV with a `token_id,timestamp,price` header.
pub struct CsvWriter;

impl DatasetWriter for CsvWriter {
    fn serialize(
        &self,
        out: &mut dyn Write,
        token_id: &str,
        points: &[PriceHistoryPoint],
    ) -> io::Result<()> {
        writeln!(out, "token_id,timestamp,price")?;
        for p in points {
            writeln!(out, "{token_id},{},{}", p.timestamp, p.price)?;
        }
        Ok(())
    }
}

/// Newline-delimited JSON: one `{"token_id","t","p"}` object per line.
pub struct JsonlWriter;

impl DatasetWriter for JsonlWriter {
    fn serialize(
        &self,
        out: &mut dyn Write,
        token_id: &str,
        points: &[PriceHistoryPoint],
    ) -> io::Result<()> {
        for p in points {
            writeln!(
                out,
                r#"{{"token_id":"{token_id}","t":{},"p":{}}}"#,
                p.timestamp, p.price
            )?;
        }
        Ok(())
    }
}

/// Return the writer for a format. Parquet is handled separately in Task 10.
pub fn writer_for(format: OutputFormat) -> Box<dyn DatasetWriter> {
    match format {
        OutputFormat::Csv => Box::new(CsvWriter),
        OutputFormat::Jsonl => Box::new(JsonlWriter),
        OutputFormat::Parquet => Box::new(CsvWriter), // replaced in Task 10
    }
}

/// Atomically write a dataset: serialize into a temp file in the destination
/// directory, then rename it into place. A killed run never leaves a partial
/// file that resume would wrongly skip.
pub fn atomic_write(
    writer: &dyn DatasetWriter,
    path: &Path,
    token_id: &str,
    points: &[PriceHistoryPoint],
) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating output dir {}", dir.display()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("creating temp file in {}", dir.display()))?;
    writer.serialize(tmp.as_file_mut(), token_id, points)?;
    tmp.as_file_mut().flush()?;
    tmp.persist(path)
        .with_context(|| format!("persisting dataset to {}", path.display()))?;
    Ok(())
}
```

Note: the `PriceHistoryPoint` field names used above (`timestamp`, `price`) are the Rust field names from `polyoxide-clob/src/api/markets.rs:483` (the `#[serde(rename)]` maps them to `t`/`p` on the wire only).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p polyoxide-cli -- clob::prices::writer`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add polyoxide-cli/src/commands/clob/prices/writer.rs polyoxide-cli/src/commands/clob/prices/mod.rs
git commit -m "feat(cli): dataset writers (csv/jsonl) with atomic temp-then-rename"
```

---

## Task 7: `fetch.rs` — rate-limited, retrying single-market fetch

**Files:**
- Create: `polyoxide-cli/src/commands/clob/prices/fetch.rs`
- Modify: `polyoxide-cli/src/commands/clob/prices/mod.rs` (add `pub mod fetch;`)
- Test: `polyoxide-cli/tests/prices_fetch.rs` (mockito)

- [ ] **Step 1: Declare the module**

In `mod.rs`, add `pub mod fetch;`.

- [ ] **Step 2: Write the failing mock test**

Create `polyoxide-cli/tests/prices_fetch.rs`. Two deterministic tests — one exercises the retry-exhaustion path (single always-failing mock asserted to be hit `max_retries + 1` times), one the happy path. This avoids relying on mockito's ordering when two mocks match the same path:

```rust
use std::num::NonZeroU32;
use std::sync::Arc;

use governor::{Quota, RateLimiter};
use mockito::{Matcher, Server};
use polyoxide_cli::commands::clob::prices::fetch::fetch_one;
use polyoxide_clob::{ClobBuilder, PricesHistoryQuery};

fn limiter() -> Arc<governor::DefaultDirectRateLimiter> {
    Arc::new(RateLimiter::direct(Quota::per_second(
        NonZeroU32::new(1000).unwrap(),
    )))
}

#[tokio::test]
async fn fetch_one_retries_until_exhausted_then_errors() {
    let mut server = Server::new_async().await;
    // Always fails (empty body → deserialize error). With max_retries = 2,
    // fetch_one makes 3 attempts total.
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(500)
        .expect(3)
        .create_async()
        .await;

    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let result =
        fetch_one(&clob, "0xtoken", &PricesHistoryQuery::default(), &limiter(), 2).await;

    assert!(result.is_err());
    mock.assert_async().await;
}

#[tokio::test]
async fn fetch_one_succeeds_first_try() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .expect(1)
        .create_async()
        .await;

    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let points = fetch_one(&clob, "0xtoken", &PricesHistoryQuery::default(), &limiter(), 2)
        .await
        .unwrap();

    assert_eq!(points.len(), 1);
    mock.assert_async().await;
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p polyoxide-cli --test prices_fetch`
Expected: FAIL — `fetch_one` not defined.

- [ ] **Step 4: Implement `fetch_one`**

Create `polyoxide-cli/src/commands/clob/prices/fetch.rs`:

```rust
use std::time::Duration;

use governor::DefaultDirectRateLimiter;
use polyoxide_clob::{Clob, ClobError, PriceHistoryPoint, PricesHistoryQuery};

/// Fetch a single market's price history, waiting for a rate-limit permit
/// before each attempt and retrying transient failures with exponential
/// backoff (200ms, 400ms, 800ms, ...).
///
/// The typed client abstracts away HTTP status codes and the `Retry-After`
/// header, so every error is treated as retryable up to `max_retries`. This is
/// a deliberate simplification over per-status handling (see spec).
pub async fn fetch_one(
    clob: &Clob,
    token_id: &str,
    query: &PricesHistoryQuery,
    limiter: &DefaultDirectRateLimiter,
    max_retries: u32,
) -> Result<Vec<PriceHistoryPoint>, ClobError> {
    let mut attempt = 0;
    loop {
        limiter.until_ready().await;
        match clob.markets().prices_history_with(token_id, query).send().await {
            Ok(resp) => return Ok(resp.history),
            Err(e) => {
                if attempt >= max_retries {
                    return Err(e);
                }
                let backoff = Duration::from_millis(200 * (1 << attempt));
                tokio::time::sleep(backoff).await;
                attempt += 1;
            }
        }
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p polyoxide-cli --test prices_fetch`
Expected: PASS.

- [ ] **Step 6: Lint**

Run: `cargo clippy -p polyoxide-cli --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add polyoxide-cli/src/commands/clob/prices/fetch.rs polyoxide-cli/src/commands/clob/prices/mod.rs polyoxide-cli/tests/prices_fetch.rs
git commit -m "feat(cli): rate-limited, retrying single-market price fetch"
```

---

## Task 8: `manifest.rs` — write the run manifest

**Files:**
- Create: `polyoxide-cli/src/commands/clob/prices/manifest.rs`
- Modify: `polyoxide-cli/src/commands/clob/prices/mod.rs` (add `pub mod manifest;`)
- Test: `manifest.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Declare the module**

In `mod.rs`, add `pub mod manifest;`.

- [ ] **Step 2: Write the failing test**

Create `polyoxide-cli/src/commands/clob/prices/manifest.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::clob::prices::types::ManifestRecord;

    #[test]
    fn write_manifest_emits_one_json_object_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.jsonl");
        let records = vec![
            ManifestRecord {
                token_id: "111".into(),
                path: "111.csv".into(),
                points: 2,
                first_ts: Some(1700000000),
                last_ts: Some(1700001000),
                status: "ok".into(),
                error: None,
            },
            ManifestRecord {
                token_id: "222".into(),
                path: "222.csv".into(),
                points: 0,
                first_ts: None,
                last_ts: None,
                status: "failed".into(),
                error: Some("boom".into()),
            },
        ];
        write_manifest(&path, &records).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"token_id\":\"111\""));
        assert!(lines[1].contains("\"status\":\"failed\""));
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p polyoxide-cli -- clob::prices::manifest`
Expected: FAIL — `write_manifest` not defined.

- [ ] **Step 4: Implement `write_manifest`**

Prepend to `manifest.rs`:

```rust
use std::io::Write;
use std::path::Path;

use color_eyre::eyre::{Context, Result};

use crate::commands::clob::prices::types::ManifestRecord;

/// Write the run manifest as JSONL (one record per line) to `path`.
///
/// Written once at end-of-run; resume relies on dataset-file existence, not the
/// manifest, so a crash mid-run simply omits the manifest without breaking a
/// subsequent resume.
pub fn write_manifest(path: &Path, records: &[ManifestRecord]) -> Result<()> {
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating manifest dir {}", dir.display()))?;
    }
    let mut file = std::fs::File::create(path)
        .with_context(|| format!("creating manifest {}", path.display()))?;
    for rec in records {
        let line = serde_json::to_string(rec).context("serializing manifest record")?;
        writeln!(file, "{line}")?;
    }
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p polyoxide-cli -- clob::prices::manifest`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-cli/src/commands/clob/prices/manifest.rs polyoxide-cli/src/commands/clob/prices/mod.rs
git commit -m "feat(cli): write prices download run manifest as jsonl"
```

---

## Task 9: `download.rs` — orchestration (select → resume → fetch → write → manifest)

**Files:**
- Modify: `polyoxide-cli/src/commands/clob/prices/download.rs`
- Modify: `polyoxide-cli/src/commands/clob/mod.rs` (build clients, pass to `run`)
- Test: `polyoxide-cli/tests/prices_download.rs` (mockito end-to-end + resume)

- [ ] **Step 1: Write the failing end-to-end + resume tests**

Create `polyoxide-cli/tests/prices_download.rs`:

```rust
use mockito::{Matcher, Server};
use polyoxide_cli::commands::clob::prices::download::DownloadArgs;
use polyoxide_clob::ClobBuilder;

fn base_args(out: &std::path::Path) -> DownloadArgs {
    DownloadArgs {
        token_ids: vec!["111".into(), "222".into()],
        input: None,
        discover: false,
        closed: None,
        open: None,
        min_volume: None,
        min_liquidity: None,
        tag_id: None,
        discover_limit: None,
        interval: "max".into(),
        fidelity: 1,
        start_ts: None,
        end_ts: None,
        out: out.to_path_buf(),
        format: polyoxide_cli::commands::clob::prices::types::OutputFormat::Csv,
        concurrency: 2,
        overwrite: false,
        fail_fast: false,
        dry_run: false,
    }
}

#[tokio::test]
async fn downloads_two_markets_and_writes_manifest() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .expect(2)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let args = base_args(dir.path());

    let summary = args.run_with_clients(&clob, None).await.unwrap();

    assert_eq!(summary.ok, 2);
    assert_eq!(summary.failed, 0);
    assert!(dir.path().join("111.csv").exists());
    assert!(dir.path().join("222.csv").exists());
    assert!(dir.path().join("manifest.jsonl").exists());
    mock.assert_async().await;
}

#[tokio::test]
async fn resume_skips_existing_files() {
    let mut server = Server::new_async().await;
    // Only ONE market should be fetched; the other file already exists.
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .expect(1)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("111.csv"), "token_id,timestamp,price\n").unwrap();

    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let summary = base_args(dir.path())
        .run_with_clients(&clob, None)
        .await
        .unwrap();

    assert_eq!(summary.ok, 1);
    assert_eq!(summary.skipped, 1);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p polyoxide-cli --test prices_download`
Expected: FAIL — `run_with_clients` and `Summary` not defined.

- [ ] **Step 3: Implement orchestration in `download.rs`**

Add these imports at the top of `download.rs` (keep the existing `use` lines):

```rust
use std::num::NonZeroU32;
use std::sync::Arc;

use futures_util::{stream, StreamExt};
use governor::{Quota, RateLimiter};
use polyoxide_clob::PricesHistoryQuery;
use polyoxide_gamma::Gamma;

use crate::commands::clob::prices::fetch::fetch_one;
use crate::commands::clob::prices::manifest::write_manifest;
use crate::commands::clob::prices::select::{
    dedupe_targets, discover_targets, read_ids_file, DiscoverFilters,
};
use crate::commands::clob::prices::types::{ManifestRecord, Target};
use crate::commands::clob::prices::writer::{atomic_write, writer_for};
```

Add a rate constant and a `Summary` type near the top of the file (below imports):

```rust
/// Stay under the documented `1000 requests / 10s` limit for `/prices-history`
/// (`docs/specs/clob/rate-limits.md`) with a safety margin.
const MAX_REQUESTS_PER_SEC: u32 = 90;

/// Per-run outcome tallies.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub ok: usize,
    pub empty: usize,
    pub failed: usize,
    pub skipped: usize,
}
```

Replace the placeholder `impl DownloadArgs { pub async fn run ... }` from Task 2 with:

```rust
impl DownloadArgs {
    /// Build the real clients and run. Discovery lazily builds a Gamma client.
    pub async fn run(self) -> Result<()> {
        let clob = Clob::public();
        let gamma = if self.discover {
            Some(Gamma::builder().build()?)
        } else {
            None
        };
        let summary = self.run_with_clients(&clob, gamma.as_ref()).await?;
        eprintln!(
            "done: {} ok, {} empty, {} skipped, {} failed",
            summary.ok, summary.empty, summary.skipped, summary.failed
        );
        if summary.failed > 0 {
            std::process::exit(1);
        }
        Ok(())
    }

    /// Core orchestration, taking injected clients so tests can point them at a
    /// mock server.
    pub async fn run_with_clients(&self, clob: &Clob, gamma: Option<&Gamma>) -> Result<Summary> {
        // 1. Resolve the target list: explicit ids ∪ file ids ∪ discovery.
        let mut raw_ids: Vec<String> = self.token_ids.clone();
        if let Some(ref path) = self.input {
            raw_ids.extend(read_ids_file(path)?);
        }
        if self.discover {
            let gamma = gamma
                .ok_or_else(|| color_eyre::eyre::eyre!("discovery requested but no Gamma client"))?;
            let filters = DiscoverFilters {
                closed: self.closed,
                open: self.open,
                min_volume: self.min_volume,
                min_liquidity: self.min_liquidity,
                tag_id: self.tag_id,
                limit: self.discover_limit,
            };
            raw_ids.extend(discover_targets(gamma, &filters).await?);
        }
        let targets = dedupe_targets(raw_ids);

        if targets.is_empty() {
            return Err(color_eyre::eyre::eyre!(
                "no markets selected: pass --token-id, --input, or --discover"
            ));
        }

        // 2. Dry run: print the plan and stop.
        if self.dry_run {
            for t in &targets {
                let path = t.output_path(&self.out, self.format);
                let exists = path.exists() && !self.overwrite;
                println!(
                    "{} -> {} {}",
                    t.token_id,
                    path.display(),
                    if exists { "(skip, exists)" } else { "(fetch)" }
                );
            }
            return Ok(Summary::default());
        }

        // 3. Resume filter.
        let mut to_fetch = Vec::new();
        let mut skipped_records = Vec::new();
        for t in targets {
            let path = t.output_path(&self.out, self.format);
            if path.exists() && !self.overwrite {
                skipped_records.push(ManifestRecord {
                    token_id: t.token_id.clone(),
                    path: path.display().to_string(),
                    points: 0,
                    first_ts: None,
                    last_ts: None,
                    status: "skipped".into(),
                    error: None,
                });
            } else {
                to_fetch.push(t);
            }
        }

        // 4. Concurrent, rate-limited fetch → write → per-market record.
        let query = PricesHistoryQuery {
            interval: Some(self.interval.clone()),
            fidelity: Some(self.fidelity),
            start_ts: self.start_ts,
            end_ts: self.end_ts,
        };
        let limiter = Arc::new(RateLimiter::direct(Quota::per_second(
            NonZeroU32::new(MAX_REQUESTS_PER_SEC).expect("nonzero rate"),
        )));

        let records: Vec<ManifestRecord> = stream::iter(to_fetch)
            .map(|t| {
                let limiter = Arc::clone(&limiter);
                let query = &query;
                async move { self.fetch_and_write(clob, &t, query, &limiter).await }
            })
            .buffer_unordered(self.concurrency)
            .collect()
            .await;

        // 5. Tally + write manifest (skipped + fetched).
        let mut summary = Summary {
            skipped: skipped_records.len(),
            ..Summary::default()
        };
        for r in &records {
            match r.status.as_str() {
                "ok" => summary.ok += 1,
                "empty" => summary.empty += 1,
                _ => summary.failed += 1,
            }
        }
        let mut all = skipped_records;
        all.extend(records);
        write_manifest(&self.out.join("manifest.jsonl"), &all)?;

        Ok(summary)
    }

    /// Fetch one market and write it, producing its manifest record. Errors are
    /// captured into a `failed` record (isolation) unless `--fail-fast`, which
    /// is enforced by the caller inspecting the returned status is not enough —
    /// so fail-fast short-circuits here by returning early via panic-free flag.
    async fn fetch_and_write(
        &self,
        clob: &Clob,
        target: &Target,
        query: &PricesHistoryQuery,
        limiter: &governor::DefaultDirectRateLimiter,
    ) -> ManifestRecord {
        let path = target.output_path(&self.out, self.format);
        let path_str = path.display().to_string();
        match fetch_one(clob, &target.token_id, query, limiter, 3).await {
            Ok(points) => {
                let (first_ts, last_ts) = (
                    points.first().map(|p| p.timestamp),
                    points.last().map(|p| p.timestamp),
                );
                let status = if points.is_empty() { "empty" } else { "ok" };
                match atomic_write(&*writer_for(self.format), &path, &target.token_id, &points) {
                    Ok(()) => ManifestRecord {
                        token_id: target.token_id.clone(),
                        path: path_str,
                        points: points.len(),
                        first_ts,
                        last_ts,
                        status: status.into(),
                        error: None,
                    },
                    Err(e) => ManifestRecord {
                        token_id: target.token_id.clone(),
                        path: path_str,
                        points: 0,
                        first_ts: None,
                        last_ts: None,
                        status: "failed".into(),
                        error: Some(e.to_string()),
                    },
                }
            }
            Err(e) => ManifestRecord {
                token_id: target.token_id.clone(),
                path: path_str,
                points: 0,
                first_ts: None,
                last_ts: None,
                status: "failed".into(),
                error: Some(e.to_string()),
            },
        }
    }
}
```

Note on `--fail-fast`: with `buffer_unordered`, cleanly aborting mid-stream adds real complexity. For v1, implement fail-fast as a **post-fetch check**: after the `collect`, if `self.fail_fast` and any record is `failed`, return an `Err` summarizing the first failure instead of `Ok(summary)`. Add this block immediately before `Ok(summary)`:

```rust
        if self.fail_fast && summary.failed > 0 {
            let first = all.iter().find(|r| r.status == "failed");
            if let Some(rec) = first {
                return Err(color_eyre::eyre::eyre!(
                    "fail-fast: market {} failed: {}",
                    rec.token_id,
                    rec.error.as_deref().unwrap_or("unknown")
                ));
            }
        }
```

(The `--fail-fast` flag thus still fetches the concurrent batch but exits non-zero with context; true early-abort is a deferred refinement. Document this in the commit body.)

- [ ] **Step 4: Simplify `ClobCommand` (no client threading needed)**

`download.rs::run` now builds its own clients, so `polyoxide-cli/src/commands/clob/mod.rs` from Task 2 already works unchanged. No edit required in this step — verify it still compiles.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p polyoxide-cli --test prices_download`
Expected: PASS (both tests).

- [ ] **Step 6: Lint**

Run: `cargo clippy -p polyoxide-cli --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add polyoxide-cli/src/commands/clob/prices/download.rs polyoxide-cli/tests/prices_download.rs
git commit -m "feat(cli): orchestrate concurrent resumable prices download + manifest"
```

---

## Task 10: Parquet writer behind the `parquet` feature

**Files:**
- Modify: `polyoxide-cli/src/commands/clob/prices/writer.rs`
- Test: `writer.rs` (cfg-gated test)

- [ ] **Step 1: Write the failing cfg-gated test**

Add to the `#[cfg(test)] mod tests` in `writer.rs`:

```rust
    #[cfg(feature = "parquet")]
    #[test]
    fn parquet_writes_readable_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0xabc.parquet");
        atomic_write(&ParquetWriter, &path, "0xabc", &points()).unwrap();
        // File exists and has the Parquet magic header "PAR1".
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"PAR1");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p polyoxide-cli --features parquet -- clob::prices::writer::tests::parquet`
Expected: FAIL — `ParquetWriter` not defined.

- [ ] **Step 3: Implement the Parquet writer**

Add to `writer.rs`, gated by the feature:

```rust
/// Apache Parquet writer (`token_id: Utf8`, `timestamp: Int64`, `price: Float64`).
#[cfg(feature = "parquet")]
pub struct ParquetWriter;

#[cfg(feature = "parquet")]
impl DatasetWriter for ParquetWriter {
    fn serialize(
        &self,
        out: &mut dyn Write,
        token_id: &str,
        points: &[PriceHistoryPoint],
    ) -> io::Result<()> {
        use std::sync::Arc;

        use arrow::array::{Float64Array, Int64Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;

        let schema = Arc::new(Schema::new(vec![
            Field::new("token_id", DataType::Utf8, false),
            Field::new("timestamp", DataType::Int64, false),
            Field::new("price", DataType::Float64, false),
        ]));
        let ids = StringArray::from(vec![token_id; points.len()]);
        let ts = Int64Array::from(points.iter().map(|p| p.timestamp).collect::<Vec<_>>());
        let px = Float64Array::from(points.iter().map(|p| p.price).collect::<Vec<_>>());
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(ids), Arc::new(ts), Arc::new(px)],
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut writer = ArrowWriter::try_new(out, schema, None)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        writer
            .write(&batch)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        writer
            .close()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }
}
```

`ArrowWriter` accepts any `Write`, so it composes with `atomic_write` (which passes the temp file as `&mut dyn Write`).

- [ ] **Step 4: Replace the Parquet arm in `writer_for`**

Change the `OutputFormat::Parquet` arm of `writer_for`:

```rust
        #[cfg(feature = "parquet")]
        OutputFormat::Parquet => Box::new(ParquetWriter),
        #[cfg(not(feature = "parquet"))]
        OutputFormat::Parquet => Box::new(CsvWriter), // unreachable: guarded in download.rs
```

- [ ] **Step 5: Guard `--format parquet` without the feature**

In `download.rs::run_with_clients`, add this check right after the empty-target guard:

```rust
        #[cfg(not(feature = "parquet"))]
        if self.format == crate::commands::clob::prices::types::OutputFormat::Parquet {
            return Err(color_eyre::eyre::eyre!(
                "--format parquet requires building with the `parquet` feature"
            ));
        }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p polyoxide-cli --features parquet -- clob::prices::writer`
Then confirm the default build still passes: `cargo test -p polyoxide-cli -- clob::prices::writer`
Expected: PASS in both.

- [ ] **Step 7: Lint both feature configurations**

Run: `cargo clippy -p polyoxide-cli --all-targets -- -D warnings`
Run: `cargo clippy -p polyoxide-cli --all-targets --features parquet -- -D warnings`
Expected: no warnings.

- [ ] **Step 8: Commit**

```bash
git add polyoxide-cli/src/commands/clob/prices/writer.rs polyoxide-cli/src/commands/clob/prices/download.rs
git commit -m "feat(cli): feature-gated Parquet dataset writer"
```

---

## Task 11: Live integration test + docs

**Files:**
- Create: `polyoxide-cli/tests/live_api.rs` (ignored)
- Modify: `CLAUDE.md` (CLI command note)

- [ ] **Step 1: Add an ignored live test**

Create `polyoxide-cli/tests/live_api.rs`:

```rust
//! Live integration tests. Hit the real Polymarket API; skipped in CI.
//! Run with: `cargo test -p polyoxide-cli --test live_api -- --ignored`

use polyoxide_cli::commands::clob::prices::download::DownloadArgs;
use polyoxide_cli::commands::clob::prices::types::OutputFormat;
use polyoxide_clob::Clob;

#[tokio::test]
#[ignore = "hits the real Polymarket API"]
async fn live_download_one_market() {
    // A known liquid token id; update if it resolves empty.
    let token_id = "71321045679252212594626385532706912750332728571942532289631379312455583992563";
    let dir = tempfile::tempdir().unwrap();

    let args = DownloadArgs {
        token_ids: vec![token_id.into()],
        input: None,
        discover: false,
        closed: None,
        open: None,
        min_volume: None,
        min_liquidity: None,
        tag_id: None,
        discover_limit: None,
        interval: "1d".into(),
        fidelity: 60,
        start_ts: None,
        end_ts: None,
        out: dir.path().to_path_buf(),
        format: OutputFormat::Csv,
        concurrency: 1,
        overwrite: false,
        fail_fast: false,
        dry_run: false,
    };

    let clob = Clob::public();
    let summary = args.run_with_clients(&clob, None).await.unwrap();
    assert_eq!(summary.failed, 0);
    assert!(dir.path().join(format!("{token_id}.csv")).exists());
}
```

- [ ] **Step 2: Verify it compiles (do not run live in CI)**

Run: `cargo test -p polyoxide-cli --test live_api -- --list`
Expected: lists `live_download_one_market` as ignored; no compile errors.

- [ ] **Step 3: Document the command in `CLAUDE.md`**

In `CLAUDE.md`, under the `polyoxide-cli` description in the Workspace Architecture section, add a sentence noting the new group. Add after the paragraph that begins "Note: `polyoxide-cli` does **not** depend on the unified `polyoxide` crate.":

```markdown
The CLI's `clob` command group currently exposes `clob prices download` — a bulk,
resumable, rate-limited downloader for CLOB historical price data
(`GET /prices-history`) that writes per-market CSV/JSONL/Parquet dataset files
plus a `manifest.jsonl`. Parquet output requires building the CLI with the
`parquet` feature.
```

- [ ] **Step 4: Full workspace verification**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --all-targets --all-features --workspace -- -D warnings`
Run: `cargo test --all-features --workspace`
Expected: all pass (live tests remain ignored).

- [ ] **Step 5: Commit**

```bash
git add polyoxide-cli/tests/live_api.rs CLAUDE.md
git commit -m "test(cli): live prices download test + document clob command group"
```

---

## Self-Review Notes

**Spec coverage check (each spec section → task):**
- Command surface / flags → Task 2 (`DownloadArgs`), Task 3 (`OutputFormat`).
- Selection (explicit ∪ file ∪ discovery, deduped) → Task 4 + Task 5 + Task 9 (union in `run_with_clients`).
- Granularity flags (`interval`/`fidelity`/`start-ts`/`end-ts`) → Task 1 (library), Task 2 (flags), Task 9 (`PricesHistoryQuery` build).
- Per-file layout + manifest → Task 3 (`output_path`), Task 8 (`write_manifest`), Task 9 (wiring).
- Formats CSV/JSONL/Parquet (Parquet feature-gated) → Task 6 + Task 10.
- Resume (skip existing) → Task 9 resume filter + Task 9 resume test.
- Rate-limit + concurrency → Task 7 (`governor` limiter) + Task 9 (`buffer_unordered`).
- Retry/backoff → Task 7.
- Error isolation + `--fail-fast` + exit code → Task 9 (`fetch_and_write` captures errors; post-collect fail-fast; `run` exits 1 on failures).
- Empty history → header-only file + `status=empty` → Task 9 (`fetch_and_write`).
- Testing (unit/mock/library/live) → distributed across Tasks 1,3,4,5,6,7,8,9,11.
- Dependencies (`governor`, `futures-util`, `tempfile`, `arrow`/`parquet` gated) → Task 2 + Task 10.

**Deliberate simplifications (documented in-plan, not gaps):**
- Retry treats all errors as retryable (typed client hides HTTP status / `Retry-After`) — Task 7.
- `--fail-fast` exits non-zero after the concurrent batch rather than aborting mid-stream — Task 9.
- Manifest is written once at end-of-run (resume depends on data files, not the manifest) — Task 8.

**Type consistency check:** `PricesHistoryQuery` fields (`interval`/`fidelity`/`start_ts`/`end_ts`), `PriceHistoryPoint` fields (`timestamp`/`price`), `Target.token_id`, `ManifestRecord` fields, `OutputFormat` variants, and `Summary` fields are used identically across Tasks 1, 3, 6, 7, 8, 9, 10, 11.

**Verified against the codebase while writing this plan:** `Market.id: String` (Task 5 warning), `governor::DefaultDirectRateLimiter` alias (Task 7), `QueryBuilder::query_opt` (Task 1), `PriceHistoryPoint { timestamp, price }` field names (Tasks 6/9), and the `ClobBuilder::new().base_url(...)` mock pattern (Tasks 7/9) all exist as used.
```
