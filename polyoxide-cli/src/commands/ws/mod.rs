mod market;
mod prices;
pub mod sports;
mod user;

use clap::Subcommand;
use color_eyre::eyre::Result;

#[derive(Subcommand)]
pub enum WsCommand {
    /// Subscribe to market channel (order book, price changes)
    Market {
        #[command(flatten)]
        args: market::MarketArgs,
    },
    /// Subscribe to user channel (orders, trades) - requires authentication
    User {
        #[command(flatten)]
        args: user::UserArgs,
    },
    /// Stream RTDS reference prices (Binance, Chainlink spot, Chainlink TWAP)
    Prices {
        #[command(flatten)]
        args: prices::PricesArgs,
    },
    /// Stream live match scores from the sports feed
    Sports {
        #[command(flatten)]
        args: sports::SportsArgs,
    },
}

impl WsCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Market { args } => market::run(args).await,
            Self::User { args } => user::run(args).await,
            Self::Prices { args } => prices::run(args).await,
            Self::Sports { args } => sports::run(args).await,
        }
    }
}
