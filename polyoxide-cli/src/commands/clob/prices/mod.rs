pub mod download;
pub mod select;
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
