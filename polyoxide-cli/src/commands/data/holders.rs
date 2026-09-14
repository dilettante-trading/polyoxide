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
