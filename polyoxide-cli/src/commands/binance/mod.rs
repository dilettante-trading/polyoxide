//! `polyoxide binance`: Binance's APIs, one module per product.

pub mod spot;
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
    /// Spot market data
    Spot {
        #[command(subcommand)]
        command: spot::SpotCommand,
    },
}

impl BinanceCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Usdm { command } => command.run().await,
            Self::Spot { command } => command.run().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(subcommand)]
        command: BinanceCommand,
    }

    #[test]
    fn each_product_has_its_stream_verb() {
        let cli = Wrapper::try_parse_from([
            "test", "spot", "stream", "--symbol", "BTCUSDT", "--kind", "ticker",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            BinanceCommand::Spot {
                command: spot::SpotCommand::Stream { .. }
            }
        ));
        let cli = Wrapper::try_parse_from(["test", "usdm", "stream", "--all-tickers"]).unwrap();
        assert!(matches!(
            cli.command,
            BinanceCommand::Usdm {
                command: usdm::UsdmCommand::Stream { .. }
            }
        ));
    }
}
