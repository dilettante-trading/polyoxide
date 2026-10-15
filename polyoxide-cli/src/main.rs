use std::io::IsTerminal;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use tracing_subscriber::EnvFilter;

use polyoxide_cli::commands;

#[derive(Parser)]
#[command(name = "polyoxide")]
#[command(
    version,
    about = "CLI tool for Polymarket's APIs and streams, and Binance USDⓈ-M market data",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// One group per venue, then one per module: `polyoxide <venue> <module>
/// <verb…>`.
#[derive(Subcommand)]
enum Commands {
    /// Polymarket: market and user data, the CLOB, and its streams
    Polymarket {
        #[command(subcommand)]
        command: commands::polymarket::PolymarketCommand,
    },
    /// Binance: USDⓈ-M futures market data
    Binance {
        #[command(subcommand)]
        command: commands::binance::BinanceCommand,
    },
    /// Generate shell completions
    Completions(commands::completions::CompletionsCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    init_logging();

    let cli = Cli::parse();

    match cli.command {
        Commands::Polymarket { command } => command.run().await?,
        Commands::Binance { command } => command.run().await?,
        Commands::Completions(cmd) => cmd.run::<Cli>()?,
    }

    Ok(())
}

/// Send the libraries' log lines to stderr, at `WARN` unless `RUST_LOG` says
/// otherwise.
///
/// `WARN` is where the libraries report what they recover from on their own:
/// a WebSocket reconnect being retried, or a `429` being waited out. Without a
/// subscriber those lines are discarded, and a feed retrying a dead host
/// looks exactly like a quiet one.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .try_init();
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
    fn the_old_groups_are_refused() {
        // No shims: v0.39.0's transport-first groups are gone, not aliased.
        for group in ["ws", "gamma", "data", "clob", "credentials"] {
            let err = try_parse(&["polyoxide", group]).err().unwrap_or_else(|| {
                panic!("`polyoxide {group}` still parses");
            });
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::InvalidSubcommand,
                "{group}: {err}"
            );
        }
        let err = try_parse(&["polyoxide", "ws", "sports"]).err().unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn each_venue_requires_a_module() {
        for venue in ["polymarket", "binance"] {
            assert!(try_parse(&["polyoxide", venue]).is_err(), "{venue}");
        }
        assert!(try_parse(&["polyoxide", "binance", "usdm"]).is_err());
    }

    /// Every `Vec` argument in the tree, by its command path and id.
    fn list_arguments() -> Vec<(String, clap::Arg)> {
        fn walk(command: &clap::Command, path: &str, found: &mut Vec<(String, clap::Arg)>) {
            for arg in command.get_arguments() {
                if matches!(arg.get_action(), clap::ArgAction::Append) {
                    found.push((path.to_owned(), arg.clone()));
                }
            }
            for sub in command.get_subcommands() {
                walk(sub, &format!("{path} {}", sub.get_name()), found);
            }
        }
        let mut found = Vec::new();
        walk(
            &<Cli as clap::CommandFactory>::command(),
            "polyoxide",
            &mut found,
        );
        found
    }

    #[test]
    fn every_list_argument_splits_on_commas() {
        // A list flag without a delimiter takes `a,b` as one value, and a
        // value parser returning a `Vec` instead panics when the field is
        // read. The one exception is the hidden `positions closed`, which
        // swallows whatever follows the removed command to refuse it.
        let found = list_arguments();
        assert!(
            found.len() >= 20,
            "the walk found only {} list arguments: {found:?}",
            found.len()
        );
        for (path, arg) in &found {
            if path == "polyoxide polymarket data positions closed" && arg.get_id() == "_ignored" {
                assert!(arg.is_hide_set(), "{path} {}", arg.get_id());
                continue;
            }
            assert_eq!(
                arg.get_value_delimiter(),
                Some(','),
                "`{path}` argument `{}` does not split on commas",
                arg.get_id()
            );
        }
    }

    mod polymarket {
        use super::try_parse;
        use crate::commands::polymarket::{
            clob::{prices::PricesCommand, ClobCommand},
            PolymarketCommand,
        };

        #[test]
        fn gamma_subcommand_requires_nested_subcommand() {
            let result = try_parse(&["polyoxide", "polymarket", "gamma"]);
            assert!(result.is_err());
        }

        #[test]
        fn data_subcommand_requires_nested_subcommand() {
            let result = try_parse(&["polyoxide", "polymarket", "data"]);
            assert!(result.is_err());
        }

        #[test]
        fn clob_subcommand_requires_nested_subcommand() {
            let result = try_parse(&["polyoxide", "polymarket", "clob"]);
            assert!(result.is_err());
        }

        #[test]
        fn gamma_markets_list_parses_defaults() {
            let cli = try_parse(&["polyoxide", "polymarket", "gamma", "markets", "list"]).unwrap();
            assert!(matches!(
                cli.command,
                super::super::Commands::Polymarket {
                    command: PolymarketCommand::Gamma { .. }
                }
            ));
        }

        #[test]
        fn data_health_parses() {
            let cli = try_parse(&["polyoxide", "polymarket", "data", "health"]).unwrap();
            assert!(matches!(
                cli.command,
                super::super::Commands::Polymarket {
                    command: PolymarketCommand::Data { .. }
                }
            ));
        }

        #[test]
        fn clob_stream_requires_a_channel() {
            let result = try_parse(&["polyoxide", "polymarket", "clob", "stream"]);
            assert!(result.is_err());
        }

        #[test]
        fn clob_stream_market_requires_asset_ids() {
            let result = try_parse(&["polyoxide", "polymarket", "clob", "stream", "market"]);
            assert!(result.is_err());
        }

        #[test]
        fn clob_stream_market_parses_with_asset_id() {
            let cli = try_parse(&[
                "polyoxide",
                "polymarket",
                "clob",
                "stream",
                "market",
                "some-asset-id",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                super::super::Commands::Polymarket {
                    command: PolymarketCommand::Clob {
                        command: ClobCommand::Stream { .. }
                    }
                }
            ));
        }

        #[test]
        fn clob_stream_user_requires_market_ids() {
            let result = try_parse(&["polyoxide", "polymarket", "clob", "stream", "user"]);
            assert!(result.is_err());
        }

        #[test]
        fn rtds_stream_parses_with_no_arguments() {
            let cli = try_parse(&["polyoxide", "polymarket", "rtds", "stream"]).unwrap();
            assert!(matches!(
                cli.command,
                super::super::Commands::Polymarket {
                    command: PolymarketCommand::Rtds { .. }
                }
            ));
        }

        #[test]
        fn sports_stream_parses_with_no_arguments() {
            let cli = try_parse(&["polyoxide", "polymarket", "sports", "stream"]).unwrap();
            assert!(matches!(
                cli.command,
                super::super::Commands::Polymarket {
                    command: PolymarketCommand::Sports { .. }
                }
            ));
        }

        #[test]
        fn clob_prices_download_parses_with_token_id() {
            let cli = try_parse(&[
                "polyoxide",
                "polymarket",
                "clob",
                "prices",
                "download",
                "--token-id",
                "0xabc",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                super::super::Commands::Polymarket {
                    command: PolymarketCommand::Clob { .. }
                }
            ));
        }

        #[test]
        fn clob_prices_download_token_ids_split_on_commas() {
            let cli = try_parse(&[
                "polyoxide",
                "polymarket",
                "clob",
                "prices",
                "download",
                "--token-id",
                "0xabc, 0xdef",
                "--token-id",
                "0x123",
            ])
            .unwrap();
            let super::super::Commands::Polymarket {
                command:
                    PolymarketCommand::Clob {
                        command: ClobCommand::Prices { command },
                    },
            } = cli.command
            else {
                panic!("expected polymarket clob prices");
            };
            let PricesCommand::Download(args) = command;
            assert_eq!(args.token_ids, ["0xabc", "0xdef", "0x123"]);
        }

        #[test]
        fn clob_prices_download_defaults_interval_max() {
            let cli = try_parse(&[
                "polyoxide",
                "polymarket",
                "clob",
                "prices",
                "download",
                "--token-id",
                "0xabc",
            ])
            .unwrap();
            let super::super::Commands::Polymarket {
                command:
                    PolymarketCommand::Clob {
                        command: ClobCommand::Prices { command },
                    },
            } = cli.command
            else {
                panic!("expected polymarket clob prices");
            };
            let PricesCommand::Download(args) = command;
            assert_eq!(args.interval, "max");
            assert_eq!(args.fidelity, 1);
            assert_eq!(args.concurrency, 4);
        }
    }

    mod binance {
        use super::try_parse;
        use crate::commands::binance::{usdm::UsdmCommand, BinanceCommand};

        #[test]
        fn usdm_stream_parses_with_a_symbol_and_kind() {
            let cli = try_parse(&[
                "polyoxide",
                "binance",
                "usdm",
                "stream",
                "--symbol",
                "BTCUSDT",
                "--kind",
                "ticker",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                super::super::Commands::Binance {
                    command: BinanceCommand::Usdm {
                        command: UsdmCommand::Stream { .. }
                    }
                }
            ));
        }
    }
}
