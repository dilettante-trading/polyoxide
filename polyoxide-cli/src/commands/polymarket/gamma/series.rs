use clap::{Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_polymarket::gamma::Gamma;

use crate::commands::polymarket::gamma::SortOrder;

/// Series status filter
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum SeriesStatus {
    /// Open series
    #[default]
    Open,
    /// Closed series
    Closed,
}

#[derive(Subcommand)]
pub enum SeriesCommand {
    /// List series
    List {
        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// Pagination offset
        #[arg(short, long, default_value = "0")]
        offset: u32,
        /// Sort order
        #[arg(long, value_enum, default_value = "desc")]
        sort: SortOrder,
        /// Filter by status (open, closed)
        #[arg(long, value_enum, default_value = "open")]
        status: SeriesStatus,
    },
    /// Get a series by ID
    Get {
        /// Series ID
        id: String,
    },
}

impl SeriesCommand {
    pub async fn run(self, gamma: &Gamma) -> Result<()> {
        match self {
            Self::List {
                limit,
                offset,
                sort,
                status,
            } => {
                let request = gamma
                    .series()
                    .list()
                    .limit(limit)
                    .offset(offset)
                    .ascending(matches!(sort, SortOrder::Asc))
                    .closed(matches!(status, SeriesStatus::Closed));

                let series = request.send().await?;
                println!("{}", serde_json::to_string_pretty(&series)?);
            }
            Self::Get { id } => {
                let series = gamma.series().get(&id).send().await?;
                println!("{}", serde_json::to_string_pretty(&series)?);
            }
        }
        Ok(())
    }
}
