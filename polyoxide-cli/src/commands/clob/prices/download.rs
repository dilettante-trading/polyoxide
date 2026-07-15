use std::num::NonZeroU32;
use std::sync::Arc;

use clap::Args;
use color_eyre::eyre::Result;
use futures_util::{stream, StreamExt};
use governor::{Quota, RateLimiter};
use polyoxide_clob::{Clob, PricesHistoryQuery};
use polyoxide_gamma::Gamma;

use crate::commands::clob::prices::fetch::fetch_one;
use crate::commands::clob::prices::manifest::write_manifest;
use crate::commands::clob::prices::select::{
    dedupe_targets, discover_targets, read_ids_file, DiscoverFilters,
};
use crate::commands::clob::prices::types::{ManifestRecord, OutputFormat, Target};
use crate::commands::clob::prices::writer::{atomic_write, writer_for};

/// Stay under the documented `1000 requests / 10s` limit for `/prices-history`
/// (`docs/specs/clob/rate-limits.md`) with a safety margin.
const MAX_REQUESTS_PER_SEC: u32 = 90;

/// Retry attempts per market before recording it as failed.
const MAX_RETRIES: u32 = 3;

/// Build a `failed` manifest record for a market.
fn failed_record(token_id: &str, path: String, err: impl std::fmt::Display) -> ManifestRecord {
    ManifestRecord {
        token_id: token_id.to_string(),
        path,
        points: 0,
        first_ts: None,
        last_ts: None,
        status: "failed".into(),
        error: Some(err.to_string()),
    }
}

/// Per-run outcome tallies.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub ok: usize,
    pub empty: usize,
    pub failed: usize,
    pub skipped: usize,
}

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

    /// Number of markets to fetch concurrently (must be >= 1).
    #[arg(long, default_value_t = 4)]
    pub concurrency: usize,

    /// Re-fetch and overwrite even if the output file already exists.
    #[arg(long)]
    pub overwrite: bool,

    /// Return a non-zero exit if any market failed (the full batch still runs to completion).
    #[arg(long = "fail-fast")]
    pub fail_fast: bool,

    /// Resolve and print the target list without fetching.
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

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
            return Err(color_eyre::eyre::eyre!(
                "{} market(s) failed",
                summary.failed
            ));
        }
        Ok(())
    }

    /// Core orchestration, taking injected clients so tests can point them at a
    /// mock server.
    pub async fn run_with_clients(&self, clob: &Clob, gamma: Option<&Gamma>) -> Result<Summary> {
        // `buffer_unordered(0)` never polls, which would hang forever. This fn is
        // also called directly by tests, so guard here rather than relying on clap.
        color_eyre::eyre::ensure!(self.concurrency > 0, "--concurrency must be >= 1");

        // 1. Resolve the target list: explicit ids ∪ file ids ∪ discovery.
        let mut raw_ids: Vec<String> = self.token_ids.clone();
        if let Some(ref path) = self.input {
            raw_ids.extend(read_ids_file(path)?);
        }
        if self.discover {
            let gamma = gamma.ok_or_else(|| {
                color_eyre::eyre::eyre!("discovery requested but no Gamma client")
            })?;
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

        Ok(summary)
    }

    /// Fetch one market and write it, producing its manifest record. Errors are
    /// captured into a `failed` record so one market's failure doesn't abort the
    /// others; `--fail-fast` is enforced by the caller after the batch.
    async fn fetch_and_write(
        &self,
        clob: &Clob,
        target: &Target,
        query: &PricesHistoryQuery,
        limiter: &governor::DefaultDirectRateLimiter,
    ) -> ManifestRecord {
        let path = target.output_path(&self.out, self.format);
        let path_str = path.display().to_string();
        match fetch_one(clob, &target.token_id, query, limiter, MAX_RETRIES).await {
            Ok(points) => {
                let first_ts = points.iter().map(|p| p.timestamp).min();
                let last_ts = points.iter().map(|p| p.timestamp).max();
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
                    Err(e) => failed_record(&target.token_id, path_str, e),
                }
            }
            Err(e) => failed_record(&target.token_id, path_str, e),
        }
    }
}
