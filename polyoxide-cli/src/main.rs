use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

mod commands;

#[derive(Parser)]
#[command(name = "polyoxide")]
#[command(version, about = "CLI tool for querying Polymarket APIs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Query Data API (positions, trades, builders, holders, activity, and more)
    Data {
        #[command(subcommand)]
        command: commands::DataCommand,
    },
    /// Query CLOB API (order book, historical prices)
    Clob {
        #[command(subcommand)]
        command: commands::ClobCommand,
    },
    /// Query Gamma API (market data)
    Gamma {
        #[command(subcommand)]
        command: commands::GammaCommand,
    },
    /// Subscribe to WebSocket channels (real-time updates)
    Ws {
        #[command(subcommand)]
        command: commands::WsCommand,
    },
    /// Generate shell completions
    Completions(commands::CompletionsCommand),
    /// Manage OS keychain credentials
    #[cfg(feature = "keychain")]
    Credentials(commands::CredentialsCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Data { command } => command.run().await?,
        Commands::Clob { command } => command.run().await?,
        Commands::Gamma { command } => command.run().await?,
        Commands::Ws { command } => command.run().await?,
        Commands::Completions(cmd) => cmd.run::<Cli>()?,
        #[cfg(feature = "keychain")]
        Commands::Credentials(cmd) => cmd.run()?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Cli;

    /// Helper to try parsing CLI args, returning a Result
    fn try_parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn no_subcommand_shows_error() {
        let result = try_parse(&["polyoxide"]);
        assert!(result.is_err());
    }

    #[test]
    fn gamma_subcommand_requires_nested_subcommand() {
        let result = try_parse(&["polyoxide", "gamma"]);
        assert!(result.is_err());
    }

    #[test]
    fn data_subcommand_requires_nested_subcommand() {
        let result = try_parse(&["polyoxide", "data"]);
        assert!(result.is_err());
    }

    #[test]
    fn ws_subcommand_requires_nested_subcommand() {
        let result = try_parse(&["polyoxide", "ws"]);
        assert!(result.is_err());
    }

    #[test]
    fn completions_requires_shell_arg() {
        let result = try_parse(&["polyoxide", "completions"]);
        assert!(result.is_err());
    }

    #[test]
    fn completions_accepts_bash() {
        let cli = try_parse(&["polyoxide", "completions", "bash"]).unwrap();
        assert!(matches!(cli.command, super::Commands::Completions(_)));
    }

    #[test]
    fn completions_accepts_zsh() {
        let cli = try_parse(&["polyoxide", "completions", "zsh"]).unwrap();
        assert!(matches!(cli.command, super::Commands::Completions(_)));
    }

    #[test]
    fn completions_accepts_fish() {
        let cli = try_parse(&["polyoxide", "completions", "fish"]).unwrap();
        assert!(matches!(cli.command, super::Commands::Completions(_)));
    }

    #[test]
    fn completions_rejects_invalid_shell() {
        let result = try_parse(&["polyoxide", "completions", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_subcommand_errors() {
        let result = try_parse(&["polyoxide", "nonexistent"]);
        assert!(result.is_err());
    }

    #[test]
    fn gamma_markets_list_parses_defaults() {
        let cli = try_parse(&["polyoxide", "gamma", "markets", "list"]).unwrap();
        assert!(matches!(cli.command, super::Commands::Gamma { .. }));
    }

    #[test]
    fn data_health_parses() {
        let cli = try_parse(&["polyoxide", "data", "health"]).unwrap();
        assert!(matches!(cli.command, super::Commands::Data { .. }));
    }

    #[test]
    fn ws_market_requires_asset_ids() {
        let result = try_parse(&["polyoxide", "ws", "market"]);
        assert!(result.is_err());
    }

    #[test]
    fn ws_market_parses_with_asset_id() {
        let cli = try_parse(&["polyoxide", "ws", "market", "some-asset-id"]).unwrap();
        assert!(matches!(cli.command, super::Commands::Ws { .. }));
    }

    #[test]
    fn ws_user_requires_market_ids() {
        let result = try_parse(&["polyoxide", "ws", "user"]);
        assert!(result.is_err());
    }

    #[test]
    fn clob_subcommand_requires_nested_subcommand() {
        let result = try_parse(&["polyoxide", "clob"]);
        assert!(result.is_err());
    }

    #[test]
    fn clob_prices_download_parses_with_token_id() {
        let cli = try_parse(&[
            "polyoxide",
            "clob",
            "prices",
            "download",
            "--token-id",
            "0xabc",
        ])
        .unwrap();
        assert!(matches!(cli.command, super::Commands::Clob { .. }));
    }

    #[test]
    fn clob_prices_download_defaults_interval_max() {
        use crate::commands::clob::{prices::PricesCommand, ClobCommand};
        let cli = try_parse(&[
            "polyoxide",
            "clob",
            "prices",
            "download",
            "--token-id",
            "0xabc",
        ])
        .unwrap();
        let super::Commands::Clob {
            command: ClobCommand::Prices { command },
        } = cli.command
        else {
            panic!("expected clob prices");
        };
        let PricesCommand::Download(args) = command;
        assert_eq!(args.interval, "max");
        assert_eq!(args.fidelity, 1);
        assert_eq!(args.concurrency, 4);
    }
}
