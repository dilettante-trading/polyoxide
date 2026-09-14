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
