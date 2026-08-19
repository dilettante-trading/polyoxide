use clap::{Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_gamma::Gamma;

use crate::commands::gamma::SortOrder;

/// Parent entity type for comments.
///
/// Mirrors [`polyoxide_gamma::types::ParentEntityType`] minus its `Unknown`
/// forward-compat variant, which is not a filter a user can meaningfully ask
/// for. `market` is deliberately absent: the server rejects it.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ParentEntityType {
    /// Event comments
    Event,
    /// Series comments
    Series,
    /// Perpetual futures asset comments
    PerpsAsset,
}

impl From<ParentEntityType> for polyoxide_gamma::types::ParentEntityType {
    fn from(value: ParentEntityType) -> Self {
        match value {
            ParentEntityType::Event => Self::Event,
            ParentEntityType::Series => Self::Series,
            ParentEntityType::PerpsAsset => Self::PerpsAsset,
        }
    }
}

#[derive(Subcommand)]
pub enum CommentsCommand {
    /// List comments
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
        /// Order by field
        #[arg(long)]
        order: Option<String>,
        /// Filter by parent entity type
        #[arg(long, value_enum)]
        parent_entity_type: Option<ParentEntityType>,
        /// Filter by parent entity ID
        #[arg(long)]
        parent_entity_id: Option<i64>,
        /// Include position data
        #[arg(long)]
        get_positions: Option<bool>,
        /// Filter to position holders only
        #[arg(long)]
        holders_only: Option<bool>,
    },
}

impl CommentsCommand {
    #[allow(dead_code)]
    fn try_parse(args: &[&str]) -> Result<Self, clap::Error> {
        use clap::Parser;

        #[derive(Parser)]
        struct Wrapper {
            #[command(subcommand)]
            cmd: CommentsCommand,
        }
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    pub async fn run(self, gamma: &Gamma) -> Result<()> {
        match self {
            Self::List {
                limit,
                offset,
                sort,
                order,
                parent_entity_type,
                parent_entity_id,
                get_positions,
                holders_only,
            } => {
                let mut request = gamma
                    .comments()
                    .list()
                    .limit(limit)
                    .offset(offset)
                    .ascending(matches!(sort, SortOrder::Asc));

                if let Some(ord) = order {
                    request = request.order(ord);
                }
                if let Some(pet) = parent_entity_type {
                    request = request.parent_entity_type(pet.into());
                }
                if let Some(pei) = parent_entity_id {
                    request = request.parent_entity_id(pei);
                }
                if let Some(gp) = get_positions {
                    request = request.get_positions(gp);
                }
                if let Some(ho) = holders_only {
                    request = request.holders_only(ho);
                }

                let comments = request.send().await?;
                println!("{}", serde_json::to_string_pretty(&comments)?);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> CommentsCommand {
        CommentsCommand::try_parse(args).unwrap()
    }

    fn assert_parse_err(args: &[&str]) {
        assert!(CommentsCommand::try_parse(args).is_err());
    }

    #[test]
    fn list_defaults() {
        let cmd = parse(&["test", "list"]);
        match cmd {
            CommentsCommand::List {
                limit,
                offset,
                parent_entity_type,
                parent_entity_id,
                get_positions,
                holders_only,
                ..
            } => {
                assert_eq!(limit, 20);
                assert_eq!(offset, 0);
                assert!(parent_entity_type.is_none());
                assert!(parent_entity_id.is_none());
                assert!(get_positions.is_none());
                assert!(holders_only.is_none());
            }
        }
    }

    #[test]
    fn parent_entity_type_event() {
        let cmd = parse(&["test", "list", "--parent-entity-type", "event"]);
        match cmd {
            CommentsCommand::List {
                parent_entity_type, ..
            } => {
                let pet = parent_entity_type.unwrap();
                assert!(matches!(pet, ParentEntityType::Event));
                assert_eq!(
                    polyoxide_gamma::types::ParentEntityType::from(pet).to_string(),
                    "Event"
                );
            }
        }
    }

    #[test]
    fn parent_entity_type_series() {
        let cmd = parse(&["test", "list", "--parent-entity-type", "series"]);
        match cmd {
            CommentsCommand::List {
                parent_entity_type, ..
            } => {
                let pet = parent_entity_type.unwrap();
                assert!(matches!(pet, ParentEntityType::Series));
                assert_eq!(
                    polyoxide_gamma::types::ParentEntityType::from(pet).to_string(),
                    "Series"
                );
            }
        }
    }

    #[test]
    fn parent_entity_type_perps_asset() {
        let cmd = parse(&["test", "list", "--parent-entity-type", "perps-asset"]);
        match cmd {
            CommentsCommand::List {
                parent_entity_type, ..
            } => {
                let pet = parent_entity_type.unwrap();
                assert!(matches!(pet, ParentEntityType::PerpsAsset));
                assert_eq!(
                    polyoxide_gamma::types::ParentEntityType::from(pet).to_string(),
                    "PerpsAsset"
                );
            }
        }
    }

    #[test]
    fn market_is_no_longer_accepted() {
        // Probed live 2026-08-19: the server rejects both `market` and
        // `Market` with a 422. It must not be reachable from the CLI.
        assert_parse_err(&["test", "list", "--parent-entity-type", "market"]);
    }

    #[test]
    fn invalid_parent_entity_type_errors() {
        assert_parse_err(&["test", "list", "--parent-entity-type", "comment"]);
    }

    #[test]
    fn list_with_parent_entity_id() {
        let cmd = parse(&["test", "list", "--parent-entity-id", "42"]);
        match cmd {
            CommentsCommand::List {
                parent_entity_id, ..
            } => {
                assert_eq!(parent_entity_id.unwrap(), 42);
            }
        }
    }

    #[test]
    fn list_with_boolean_flags() {
        let cmd = parse(&[
            "test",
            "list",
            "--get-positions",
            "true",
            "--holders-only",
            "false",
        ]);
        match cmd {
            CommentsCommand::List {
                get_positions,
                holders_only,
                ..
            } => {
                assert!(get_positions.unwrap());
                assert!(!holders_only.unwrap());
            }
        }
    }

    #[test]
    fn list_requires_subcommand() {
        // No subcommand at all should error
        assert_parse_err(&["test"]);
    }
}
