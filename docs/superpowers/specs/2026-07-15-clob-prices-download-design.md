# Design: `polyoxide clob prices download`

**Date:** 2026-07-15
**Branch:** `aidanb/ml-data`
**Status:** Approved design, pending implementation plan

## Purpose

Add a CLI command that downloads Polymarket CLOB historical price data in bulk
and writes it to disk as per-market dataset files, for offline machine-learning
use. A successful run produces a directory of price-history files (one per
market) plus a manifest recording what was fetched.

This adds the **first `clob` command group** to `polyoxide-cli`, which currently
exposes only `data`, `gamma`, `ws`, `credentials`, and `completions`. The
command sets the pattern for future CLOB CLI surfaces.

## Scope

**In scope (v1):**
- Bulk download of `/prices-history` for many markets to per-market files.
- Market selection by explicit token IDs, an input file, and/or Gamma-based
  discovery filters — the union of all three, deduped.
- Output as CSV, JSONL, or Parquet (Parquet feature-gated).
- Full time-granularity controls (`interval`, `fidelity`, `start-ts`, `end-ts`).
- Resumable, rate-limited, concurrent fetching with a per-market manifest.

**Out of scope (explicit YAGNI cuts):**
- Ad-hoc single-market interactive query (bulk-only for v1).
- The batch endpoint (`POST /batch-prices-history`) — deferred; the per-market
  model is simpler and correct. May be added later as a `--batch` optimization
  if request volume becomes a real bottleneck.
- Combined single-file output — per-file only for v1.

## Decisions (locked)

| Dimension | Decision |
|-----------|----------|
| Core goal | Bulk dataset files for ML |
| Selection | Explicit IDs + input file **and** Gamma discovery mode |
| Formats | CSV + JSONL + Parquet (Parquet behind a cargo feature) |
| Layout | One file per market + a manifest |
| Granularity | Full flags; default `interval=max`, `fidelity=1` (finest, max history) |
| Robustness | Resume (skip existing) + rate-limit + concurrency |
| Fetch strategy | Per-market concurrent fetch (approach A) |

### Why per-market (approach A) over the batch endpoint (B)

The 1:1 mapping (market → file → request → manifest row) is what makes every
hard requirement fall out naturally:

- **Resume** = "does the output file already exist?" — trivial and correct.
- **Error isolation** — one failing market cannot corrupt a batch of others.
- **Atomic writes** — write to a temp file, then rename into place, so a killed
  run never leaves a half-written file that resume would wrongly skip.

The batch endpoint's fewer-requests advantage is offset by resume granularity
becoming the batch (partial failures are messy) and awkward manifest
bookkeeping. With concurrency + rate-limiting, per-market is fast enough.

## Command Surface

```
polyoxide clob prices download [OPTIONS]
```

### Selection (union of all sources, deduped)
- `--token-id <ID>` — repeatable; explicit token IDs.
- `-i, --input <FILE>` — newline-delimited token IDs; `#` comments and blank
  lines ignored.
- `--discover` — enable Gamma-based discovery. Discovery filters (only meaningful
  with `--discover`), mapped to `gamma.markets().list()`:
  - `--closed <bool>` → `.closed(bool)` (omit for the API default)
  - `--open <bool>` → `.open(bool)`
  - `--min-volume <f64>` → `.volume_num_min(f64)`
  - `--min-liquidity <f64>` → `.liquidity_num_min(f64)`
  - `--tag-id <i64>` → `.tag_id(i64)` (Gamma filters by numeric tag id, not slug)
  - `--discover-limit <u32>` → `.limit(u32)` (upstream caps at 1000/page)

Discovery extracts token IDs from each `Market.clob_token_ids`, which is a
**JSON-encoded string** (`Option<String>`, e.g. `"[\"123\",\"456\"]"`) and must
be parsed with `serde_json::from_str::<Vec<String>>`. Markets with a missing or
unparseable `clob_token_ids` are skipped with a warning.

### Granularity
- `--interval <max|all|1m|1w|1d|6h|1h>` — default `max`.
- `--fidelity <minutes>` — default `1` (finest).
- `--start-ts <unix>` — optional.
- `--end-ts <unix>` — optional.

### Output
- `-o, --out <DIR>` — default `./polymarket-prices`.
- `--format <csv|jsonl|parquet>` — default `csv`. `parquet` errors out with a
  clear message if the binary was built without the `parquet` feature.

### Operational
- `--concurrency <N>` — default `4`.
- `--overwrite` — re-fetch and rewrite even if the output file exists (disables
  resume).
- `--fail-fast` — abort the whole run on the first market failure (default:
  isolate failures and continue).
- `--dry-run` — resolve and print the target list (and what would be
  skipped/fetched) without making price-history requests.

## Module Layout

```
polyoxide-cli/src/commands/clob/
  mod.rs          — ClobCommand enum; builds Clob::public() (+ Gamma when --discover)
  prices/
    mod.rs        — PricesCommand::Download
    download.rs   — DownloadArgs (clap) + run() orchestration
    select.rs     — resolve explicit + file + discovery → deduped Vec<Target>
    fetch.rs      — bounded concurrent fetch (semaphore + governor rate limiter + retry)
    writer.rs     — DatasetWriter trait: Csv, Jsonl, Parquet(cfg-gated)
    manifest.rs   — ManifestRecord + atomic append
```

`ClobCommand` is wired into `Commands` in `polyoxide-cli/src/main.rs` alongside
`Data`, `Gamma`, `Ws`, following the existing `commands/data/mod.rs` pattern
(enum + `async fn run`).

### Library-layer change (additive, semver-clean)

The published `polyoxide-clob` method
`markets().prices_history(token_id)` currently sets only the `market` query
param. It will be **extended additively**:

- Add `struct PricesHistoryQuery { interval: Option<String>, fidelity:
  Option<i32>, start_ts: Option<i64>, end_ts: Option<i64> }`.
- Add a method that applies these as query params (`startTs`, `endTs`,
  `interval`, `fidelity` per `docs/specs/clob/markets.md:284`) via the existing
  `Request::query` chaining, applying each `Some` value.
- Keep the existing `prices_history(token_id)` signature untouched for
  backward compatibility.

## Data Flow

1. Build `Clob::public()` (prices-history is `AuthMode::None`). If `--discover`,
   also build `Gamma::builder().build()`.
2. `select` resolves the target list: explicit IDs ∪ file IDs ∪ discovery
   results, deduped into `Vec<Target { token_id }>`.
3. **Resume filter:** for each target, output path = `<out>/<token_id>.<ext>`;
   if it exists and `--overwrite` is not set, mark as skipped.
4. `fetch`: a bounded worker pool of size `--concurrency` (tokio `Semaphore`).
   Each task acquires a **`governor` rate-limit permit** (a shared quota tuned
   under the `1000 req / 10s` limit from `docs/specs/clob/rate-limits.md`), then
   calls the extended `prices_history` with the granularity params, with
   **retry + exponential backoff** on HTTP 429 (respecting `Retry-After`) and
   transient network errors.
5. `writer` serializes the returned `Vec<PriceHistoryPoint>` to a **temp file**,
   then atomically renames it into the final path.
6. `manifest.jsonl` (in `<out>/`) gets one appended row per market.
7. Progress and a final summary print to stderr: `N ok / M skipped / K failed`.

`PriceHistoryPoint` is `{ t: i64 unix-seconds, p: f64 }`
(`polyoxide-clob/src/api/markets.rs:483`) — already tabular and ML-friendly.

## Output Schema

All three formats share the same columns:

| Column | Type | Source |
|--------|------|--------|
| `token_id` | string | the market token id |
| `timestamp` | i64 (unix seconds) | `PriceHistoryPoint.t` |
| `price` | f64 | `PriceHistoryPoint.p` |

Including `token_id` per-row is mildly redundant with the filename but makes
downstream concatenation into one dataframe trivial — a deliberate
ML-friendliness choice.

- **CSV:** header row `token_id,timestamp,price`, then one row per point.
- **JSONL:** one object per line `{"token_id":..., "t":..., "p":...}`.
- **Parquet:** same three-column schema, columnar + compressed.

Parquet lives behind a **`parquet` cargo feature** on `polyoxide-cli` (pulls the
`arrow` + `parquet` crates). Off by default so normal builds stay lean; selecting
`--format parquet` without the feature is a clean runtime error.

### Manifest schema (`manifest.jsonl`)

One JSON object per market, appended as each market completes:

```json
{"token_id": "...", "path": "polymarket-prices/<id>.csv", "points": 1234,
 "first_ts": 1700000000, "last_ts": 1700900000, "status": "ok", "error": null}
```

`status` ∈ `ok | empty | failed`. `empty` markets still write a header-only
file. `failed` records carry the error string in `error`.

## Error Handling

- **Per-market isolation (default):** a market failure is caught, recorded in the
  manifest with `status=failed`, and the run continues. `--fail-fast` overrides
  to abort on the first failure.
- **Retries:** exponential backoff on HTTP 429 (respect `Retry-After` when
  present) and transient network errors; after retries are exhausted, mark the
  market failed.
- **Empty history:** write a header-only file, record `status=empty`.
- **Exit code:** non-zero if any market ended `failed`; the summary is always
  printed regardless.
- **Discovery robustness:** markets whose `clob_token_ids` is missing or
  unparseable are skipped with a warning, not fatal.

## Testing Strategy

- **Unit:**
  - selection dedup (explicit ∪ file ∪ discovery),
  - `clob_token_ids` JSON-string parsing (valid, empty, malformed),
  - target → output-path mapping,
  - resume-skip logic (exists vs `--overwrite`),
  - `ManifestRecord` construction,
  - CSV/JSONL golden serialization from a fixed `Vec<PriceHistoryPoint>`.
- **Mock (`mockito` + `tempfile` tempdir):** point the client at a mocked
  `/prices-history`; assert the correct per-market files and manifest rows are
  written; assert a second run skips existing files (resume).
- **Library:** assert the new `PricesHistoryQuery` produces the correct query
  string on `/prices-history` (extend existing `markets` mock tests).
- **Live (`#[ignore]`):** end-to-end download of one real market's history,
  gated so CI skips it (per repo testing conventions).

## Dependencies

- **Concurrency/rate-limiting:** reuse `futures-util` (already a CLI dep) and
  `governor 0.8` (already a workspace dep) — no new core dependencies.
- **Atomic writes / tempdir:** `tempfile` for tests; temp-then-rename in the
  writer uses std + the target directory.
- **Parquet (feature-gated):** `arrow` + `parquet`, added under the optional
  `parquet` feature on `polyoxide-cli` only.

### To confirm during implementation
- Exact `governor` quota shape for the `1000 req/10s` bucket and how it composes
  with the `--concurrency` semaphore.
- Progress output: reuse an existing dependency vs plain stderr counters (lean
  toward stderr counters to avoid a new dep unless one is already present).
```
