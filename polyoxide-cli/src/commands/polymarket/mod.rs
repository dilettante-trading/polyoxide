//! `polyoxide polymarket`: Polymarket's APIs and streams, one module each.

pub mod clob;
pub mod data;
pub mod gamma;
pub mod rtds;
pub mod sports;

#[cfg(feature = "keychain")]
pub mod credentials;

use clap::Subcommand;
use color_eyre::eyre::Result;

#[derive(Subcommand)]
pub enum PolymarketCommand {
    /// Query Gamma API (market data)
    Gamma {
        #[command(subcommand)]
        command: gamma::GammaCommand,
    },
    /// Query Data API (positions, trades, builders, holders, activity, and more)
    Data {
        #[command(subcommand)]
        command: data::DataCommand,
    },
    /// The CLOB: historical prices, and the market and user streams
    Clob {
        #[command(subcommand)]
        command: clob::ClobCommand,
    },
    /// RTDS reference prices: Binance, Chainlink spot and Chainlink TWAP
    Rtds {
        #[command(subcommand)]
        command: rtds::RtdsCommand,
    },
    /// Live match scores from the sports feed
    Sports {
        #[command(subcommand)]
        command: sports::SportsCommand,
    },
    /// Store, show or delete Polymarket credentials in the OS keychain
    #[cfg(feature = "keychain")]
    Credentials {
        #[command(subcommand)]
        command: crate::commands::credentials::Credentials<credentials::Polymarket>,
    },
}

impl PolymarketCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Gamma { command } => command.run().await,
            Self::Data { command } => command.run().await,
            Self::Clob { command } => command.run().await,
            Self::Rtds { command } => command.run().await,
            Self::Sports { command } => command.run().await,
            #[cfg(feature = "keychain")]
            Self::Credentials { command } => command.run(),
        }
    }
}
