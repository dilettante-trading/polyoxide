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
        // Orchestration is implemented in a later task. For now, construct the
        // client so the wiring compiles and the command is reachable.
        let _clob = Clob::public();
        println!(
            "clob prices download: {} explicit id(s)",
            self.token_ids.len()
        );
        Ok(())
    }
}
