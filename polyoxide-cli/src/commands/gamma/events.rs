use clap::{ArgAction, Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_gamma::Gamma;

use crate::commands::gamma::SortOrder;

/// Event status filter
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum EventStatus {
    /// Open events (not closed, not archived)
    #[default]
    Open,
    /// Closed events
    Closed,
    /// Archived events
    Archived,
}

#[derive(Subcommand)]
pub enum EventsCommand {
    /// List events
    List {
        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// Pagination offset
        #[arg(short, long, default_value = "0")]
        offset: u32,
        /// Filter by active status
        #[arg(long, default_value = "true")]
        active: bool,
        /// Filter by status (open, closed, archived)
        #[arg(short, long, value_enum, default_value = "open")]
        status: EventStatus,
        /// Show only featured events
        #[arg(long, action = ArgAction::SetTrue, conflicts_with = "not_featured")]
        featured: bool,
        /// Exclude featured events
        #[arg(long, action = ArgAction::SetTrue)]
        not_featured: bool,
        /// Minimum liquidity
        #[arg(long)]
        liquidity_min: Option<f64>,
        /// Maximum liquidity
        #[arg(long)]
        liquidity_max: Option<f64>,
        /// Minimum volume
        #[arg(long)]
        volume_min: Option<f64>,
        /// Maximum volume
        #[arg(long)]
        volume_max: Option<f64>,
        /// Sort order
        #[arg(long, value_enum, default_value = "desc")]
        sort: SortOrder,
        /// Order by field
        #[arg(long, default_value = "startDate")]
        order: String,
    },
    /// Get an event by ID
    Get {
        /// Event ID
        id: String,
    },
    /// Get an event by slug
    GetBySlug {
        /// Event slug
        slug: String,
    },
}

impl EventsCommand {
    #[allow(dead_code)]
    fn try_parse(args: &[&str]) -> Result<Self, clap::Error> {
        use clap::Parser;

        #[derive(Parser)]
        struct Wrapper {
            #[command(subcommand)]
            cmd: EventsCommand,
        }
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    pub async fn run(self, gamma: &Gamma) -> Result<()> {
        match self {
            Self::List {
                limit,
                offset,
                active,
                status,
                featured,
                not_featured,
                liquidity_min,
                liquidity_max,
                volume_min,
                volume_max,
                sort,
                order,
            } => {
                let mut request = gamma
                    .events()
                    .list()
                    .limit(limit)
                    .offset(offset)
                    .order(&order)
                    .active(active)
                    .ascending(matches!(sort, SortOrder::Asc));

                match status {
                    EventStatus::Open => {
                        request = request.closed(false).archived(false);
                    }
                    EventStatus::Closed => {
                        request = request.closed(true);
                    }
                    EventStatus::Archived => {
                        request = request.archived(true);
                    }
                }
                if featured {
                    request = request.featured(true);
                } else if not_featured {
                    request = request.featured(false);
                }
                if let Some(min) = liquidity_min {
                    request = request.liquidity_min(min);
                }
                if let Some(max) = liquidity_max {
                    request = request.liquidity_max(max);
                }
                if let Some(min) = volume_min {
                    request = request.volume_min(min);
                }
                if let Some(max) = volume_max {
                    request = request.volume_max(max);
                }

                let events = request.send().await?;
                println!("{}", serde_json::to_string_pretty(&events)?);
            }
            Self::Get { id } => {
                let event = gamma.events().get(&id).send().await?;
                println!("{}", serde_json::to_string_pretty(&event)?);
            }
            Self::GetBySlug { slug } => {
                let event = gamma.events().get_by_slug(&slug).send().await?;
                println!("{}", serde_json::to_string_pretty(&event)?);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> EventsCommand {
        EventsCommand::try_parse(args).unwrap()
    }

    fn assert_parse_err(args: &[&str]) {
        assert!(EventsCommand::try_parse(args).is_err());
    }

    #[test]
    fn list_defaults() {
        let cmd = parse(&["test", "list"]);
        match cmd {
            EventsCommand::List {
                limit,
                offset,
                active,
                status,
                featured,
                not_featured,
                sort,
                order,
                ..
            } => {
                assert_eq!(limit, 20);
                assert_eq!(offset, 0);
                assert!(active);
                assert!(matches!(status, EventStatus::Open));
                assert!(!featured);
                assert!(!not_featured);
                assert!(matches!(sort, SortOrder::Desc));
                assert_eq!(order, "startDate");
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_featured_flag() {
        let cmd = parse(&["test", "list", "--featured"]);
        match cmd {
            EventsCommand::List {
                featured,
                not_featured,
                ..
            } => {
                assert!(featured);
                assert!(!not_featured);
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_not_featured_flag() {
        let cmd = parse(&["test", "list", "--not-featured"]);
        match cmd {
            EventsCommand::List {
                featured,
                not_featured,
                ..
            } => {
                assert!(!featured);
                assert!(not_featured);
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_featured_and_not_featured_conflict() {
        // These are marked as conflicts_with, so both should error
        assert_parse_err(&["test", "list", "--featured", "--not-featured"]);
    }

    #[test]
    fn list_status_closed() {
        let cmd = parse(&["test", "list", "--status", "closed"]);
        match cmd {
            EventsCommand::List { status, .. } => {
                assert!(matches!(status, EventStatus::Closed));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_status_archived() {
        let cmd = parse(&["test", "list", "--status", "archived"]);
        match cmd {
            EventsCommand::List { status, .. } => {
                assert!(matches!(status, EventStatus::Archived));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_invalid_status_errors() {
        assert_parse_err(&["test", "list", "--status", "deleted"]);
    }

    #[test]
    fn list_with_filters() {
        let cmd = parse(&[
            "test",
            "list",
            "--liquidity-min",
            "100",
            "--liquidity-max",
            "5000",
            "--volume-min",
            "50",
            "--volume-max",
            "10000",
        ]);
        match cmd {
            EventsCommand::List {
                liquidity_min,
                liquidity_max,
                volume_min,
                volume_max,
                ..
            } => {
                assert_eq!(liquidity_min.unwrap(), 100.0);
                assert_eq!(liquidity_max.unwrap(), 5000.0);
                assert_eq!(volume_min.unwrap(), 50.0);
                assert_eq!(volume_max.unwrap(), 10000.0);
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn get_requires_id() {
        assert_parse_err(&["test", "get"]);
    }

    #[test]
    fn get_parses_id() {
        let cmd = parse(&["test", "get", "event-42"]);
        match cmd {
            EventsCommand::Get { id } => assert_eq!(id, "event-42"),
            _ => panic!("expected Get variant"),
        }
    }

    #[test]
    fn get_by_slug_parses() {
        let cmd = parse(&["test", "get-by-slug", "my-event"]);
        match cmd {
            EventsCommand::GetBySlug { slug } => assert_eq!(slug, "my-event"),
            _ => panic!("expected GetBySlug variant"),
        }
    }
}
