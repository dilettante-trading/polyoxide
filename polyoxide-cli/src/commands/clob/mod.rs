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
