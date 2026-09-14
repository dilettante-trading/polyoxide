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
