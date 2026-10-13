use clap::Subcommand;
use color_eyre::eyre::Result;
use polyoxide_polymarket::gamma::Gamma;

use crate::commands::gamma::SortOrder;

#[derive(Subcommand)]
pub enum SportsCommand {
    /// List sports metadata
    List,
    /// List teams
    Teams {
        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// Pagination offset
        #[arg(short, long, default_value = "0")]
        offset: u32,
        /// Sort order
        #[arg(long, value_enum, default_value = "desc")]
        sort: SortOrder,
        /// Order by field
        #[arg(long)]
        order: Option<String>,
        /// Filter by league
        #[arg(long)]
        league: Option<String>,
    },
}

impl SportsCommand {
    pub async fn run(self, gamma: &Gamma) -> Result<()> {
        match self {
            Self::List => {
                let sports = gamma.sports().list().send().await?;
                println!("{}", serde_json::to_string_pretty(&sports)?);
            }
            Self::Teams {
                limit,
                offset,
                sort,
                order,
                league,
            } => {
                let mut request = gamma
                    .sports()
                    .list_teams()
                    .limit(limit)
                    .offset(offset)
                    .ascending(matches!(sort, SortOrder::Asc));

                if let Some(ord) = order {
                    request = request.order(ord);
                }
                if let Some(l) = league {
                    request = request.league([l]);
                }

                let teams = request.send().await?;
                println!("{}", serde_json::to_string_pretty(&teams)?);
            }
        }
        Ok(())
    }
}
