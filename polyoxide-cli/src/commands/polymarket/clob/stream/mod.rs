//! `polyoxide polymarket clob stream`: the CLOB's two WebSocket channels.

pub mod market;
pub mod user;

use clap::Subcommand;
use color_eyre::eyre::Result;

#[derive(Subcommand)]
pub enum StreamCommand {
    /// Stream the market channel (order book, price changes)
    Market {
        #[command(flatten)]
        args: market::MarketArgs,
    },
    /// Stream the user channel (orders, trades) - requires authentication
    User {
        #[command(flatten)]
        args: user::UserArgs,
    },
}

impl StreamCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Market { args } => market::run(args).await,
            Self::User { args } => user::run(args).await,
        }
    }
}
