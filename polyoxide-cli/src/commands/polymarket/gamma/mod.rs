mod comments;
mod events;
mod markets;
mod series;
mod sports;
mod tags;

use clap::{Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_polymarket::gamma::Gamma;

#[derive(Subcommand)]
pub enum GammaCommand {
    /// Query markets
    Markets {
        #[command(subcommand)]
        command: markets::MarketsCommand,
    },
    /// Query events
    Events {
        #[command(subcommand)]
        command: events::EventsCommand,
    },
    /// Query tags
    Tags {
        #[command(subcommand)]
        command: tags::TagsCommand,
    },
    /// Query series
    Series {
        #[command(subcommand)]
        command: series::SeriesCommand,
    },
    /// Query sports
    Sports {
        #[command(subcommand)]
        command: sports::SportsCommand,
    },
    /// Query comments
    Comments {
        #[command(subcommand)]
        command: comments::CommentsCommand,
    },
}

impl GammaCommand {
    pub async fn run(self) -> Result<()> {
        let gamma = Gamma::new()?;

        match self {
            Self::Markets { command } => command.run(&gamma).await,
            Self::Events { command } => command.run(&gamma).await,
            Self::Tags { command } => command.run(&gamma).await,
            Self::Series { command } => command.run(&gamma).await,
            Self::Sports { command } => command.run(&gamma).await,
            Self::Comments { command } => command.run(&gamma).await,
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
