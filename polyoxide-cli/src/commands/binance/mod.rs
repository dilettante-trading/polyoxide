//! `polyoxide binance`: Binance's APIs, one module per product.

pub mod usdm;

use clap::Subcommand;
use color_eyre::eyre::Result;

#[derive(Subcommand)]
pub enum BinanceCommand {
    /// USDⓈ-M futures market data
    Usdm {
        #[command(subcommand)]
        command: usdm::UsdmCommand,
    },
}

impl BinanceCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Usdm { command } => command.run().await,
        }
    }
}
