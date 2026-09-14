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
