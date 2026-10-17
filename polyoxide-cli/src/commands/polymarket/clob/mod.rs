pub mod prices;
pub mod stream;

use clap::Subcommand;
use color_eyre::eyre::Result;

use prices::PricesCommand;
use stream::StreamCommand;

#[derive(Subcommand)]
pub enum ClobCommand {
    /// Historical price data.
    Prices {
        #[command(subcommand)]
        command: PricesCommand,
    },
    /// Stream the market or user channel.
    Stream {
        #[command(subcommand)]
        command: StreamCommand,
    },
}

impl ClobCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Prices { command } => command.run().await,
            Self::Stream { command } => command.run().await,
        }
    }
}
