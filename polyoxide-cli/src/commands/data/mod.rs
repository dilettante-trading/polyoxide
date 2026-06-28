mod activity;
mod builders;
mod holders;
mod live_volume;
mod open_interest;
mod positions;
mod traded;
mod trades;

use clap::{Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_data::DataApi;

use crate::commands::data::{
    activity::UserActivityCommand, holders::HoldersCommand, live_volume::LiveVolumeCommand,
    open_interest::OpenInterestCommand, positions::PositionsCommand, traded::TradedCommand,
};

#[derive(Subcommand)]
pub enum DataCommand {
    /// Check API health status
    Health,
    /// Query user activity
    Activity(UserActivityCommand),
    /// Query builder leaderboard and volume
    Builders {
        #[command(subcommand)]
        command: builders::BuildersCommand,
    },
    /// Query top holders for markets
    Holders(HoldersCommand),
    /// Query trades
    Trades {
        #[command(subcommand)]
        command: trades::TradesCommand,
    },
    /// Get traded markets by user
    Traded(TradedCommand),
    /// Query user positions (open, closed, value) and activity
    Positions(PositionsCommand),
    /// Get open interest for markets
    OpenInterest(OpenInterestCommand),
    /// Get live volume for an event
    LiveVolume(LiveVolumeCommand),
}

impl DataCommand {
    pub async fn run(self) -> Result<()> {
        let data = DataApi::new()?;

        match self {
            Self::Health => {
                let health = data.health().check().await?;
                println!("{}", serde_json::to_string_pretty(&health)?);
                Ok(())
            }
            Self::Activity(cmd) => cmd.run(&data).await,
            Self::Builders { command } => command.run(&data).await,
            Self::Holders(cmd) => cmd.run(&data).await,
            Self::Trades { command } => command.run(&data).await,
            Self::Traded(cmd) => cmd.run(&data).await,
            Self::Positions(cmd) => cmd.run(&data).await,
            Self::OpenInterest(cmd) => cmd.run(&data).await,
            Self::LiveVolume(cmd) => cmd.run(&data).await,
        }
    }
}

/// Sort order
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum SortOrder {
    /// Ascending order
    Asc,
    /// Descending order
    #[default]
    Desc,
}

impl From<SortOrder> for polyoxide_data::types::SortDirection {
    fn from(order: SortOrder) -> Self {
        match order {
            SortOrder::Asc => Self::Asc,
            SortOrder::Desc => Self::Desc,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use polyoxide_data::types::SortDirection;

    use super::*;

    fn try_parse(args: &[&str]) -> Result<DataCommand, clap::Error> {
        #[derive(Parser)]
        struct Wrapper {
            #[command(subcommand)]
            cmd: DataCommand,
        }
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    #[test]
    fn sort_order_from_asc() {
        let sd: SortDirection = SortOrder::Asc.into();
        assert!(matches!(sd, SortDirection::Asc));
    }

    #[test]
    fn sort_order_from_desc() {
        let sd: SortDirection = SortOrder::Desc.into();
        assert!(matches!(sd, SortDirection::Desc));
    }

    #[test]
    fn health_subcommand_parses() {
        let cmd = try_parse(&["test", "health"]).unwrap();
        assert!(matches!(cmd, DataCommand::Health));
    }

    #[test]
    fn activity_requires_user() {
        let result = try_parse(&["test", "activity"]);
        assert!(result.is_err());
    }

    #[test]
    fn activity_parses_with_user() {
        let cmd = try_parse(&["test", "activity", "--user", "0xabc123"]).unwrap();
        assert!(matches!(cmd, DataCommand::Activity(_)));
    }

    #[test]
    fn positions_requires_user() {
        let result = try_parse(&["test", "positions", "--user", "0xabc", "list"]);
        // Should parse since user is provided
        assert!(result.is_ok());
    }

    #[test]
    fn positions_without_user_errors() {
        let result = try_parse(&["test", "positions", "list"]);
        assert!(result.is_err());
    }

    #[test]
    fn traded_requires_user() {
        let result = try_parse(&["test", "traded"]);
        assert!(result.is_err());
    }

    #[test]
    fn traded_parses_with_user() {
        let cmd = try_parse(&["test", "traded", "--user", "0xdef456"]).unwrap();
        assert!(matches!(cmd, DataCommand::Traded(_)));
    }

    #[test]
    fn live_volume_requires_event_id() {
        let result = try_parse(&["test", "live-volume"]);
        assert!(result.is_err());
    }

    #[test]
    fn live_volume_parses_with_event_id() {
        let cmd = try_parse(&["test", "live-volume", "--event-id", "42"]).unwrap();
        assert!(matches!(cmd, DataCommand::LiveVolume(_)));
    }

    #[test]
    fn holders_parses_without_market() {
        // market defaults to empty Vec, so it parses without --market
        let cmd = try_parse(&["test", "holders"]).unwrap();
        assert!(matches!(cmd, DataCommand::Holders(_)));
    }

    #[test]
    fn open_interest_parses_without_market() {
        let cmd = try_parse(&["test", "open-interest"]).unwrap();
        assert!(matches!(cmd, DataCommand::OpenInterest(_)));
    }

    #[test]
    fn invalid_subcommand_errors() {
        let result = try_parse(&["test", "nonexistent"]);
        assert!(result.is_err());
    }

    #[test]
    fn builders_requires_subcommand() {
        let result = try_parse(&["test", "builders"]);
        assert!(result.is_err());
    }

    #[test]
    fn builders_leaderboard_parses() {
        let cmd = try_parse(&["test", "builders", "leaderboard"]).unwrap();
        assert!(matches!(cmd, DataCommand::Builders { .. }));
    }

    #[test]
    fn builders_volume_parses() {
        let cmd = try_parse(&["test", "builders", "volume"]).unwrap();
        assert!(matches!(cmd, DataCommand::Builders { .. }));
    }

    #[test]
    fn trades_requires_subcommand() {
        let result = try_parse(&["test", "trades"]);
        assert!(result.is_err());
    }

    #[test]
    fn trades_list_parses() {
        let cmd = try_parse(&["test", "trades", "list"]).unwrap();
        assert!(matches!(cmd, DataCommand::Trades { .. }));
    }
}
