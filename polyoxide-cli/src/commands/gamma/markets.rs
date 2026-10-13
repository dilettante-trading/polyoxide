use clap::{Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_polymarket::gamma::Gamma;

use crate::commands::gamma::SortOrder;

/// Market status filter
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum MarketStatus {
    /// Open markets (not closed, not archived)
    #[default]
    Open,
    /// Closed markets
    Closed,
    /// Archived markets
    Archived,
}

/// Preset filters for common market queries
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MarketPreset {
    /// Active markets with high volume (>$100k) sorted by 24h volume
    Trending,
    /// Active markets sorted by total volume (descending)
    TopVolume,
    /// Active markets with high liquidity (>$50k)
    HighLiquidity,
    /// New markets (recently created)
    New,
    /// Active competitive markets
    Competitive,
}

#[derive(Subcommand)]
pub enum MarketsCommand {
    /// List markets
    List {
        /// Use a preset filter for common queries
        #[arg(short, long, value_enum)]
        preset: Option<MarketPreset>,
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
        status: MarketStatus,
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
        #[arg(long)]
        order: Option<String>,
    },
    /// Get a market by ID
    Get {
        /// Market ID
        id: String,
    },
    /// Get a market by slug
    GetBySlug {
        /// Market slug
        slug: String,
    },
}

impl MarketsCommand {
    #[allow(dead_code)]
    fn try_parse(args: &[&str]) -> Result<Self, clap::Error> {
        use clap::Parser;

        // Wrapper so we can use try_parse_from on a subcommand enum
        #[derive(Parser)]
        struct Wrapper {
            #[command(subcommand)]
            cmd: MarketsCommand,
        }
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    pub async fn run(self, gamma: &Gamma) -> Result<()> {
        match self {
            Self::List {
                preset,
                limit,
                offset,
                active,
                status,
                liquidity_min,
                liquidity_max,
                volume_min,
                volume_max,
                sort,
                order,
            } => {
                let mut request = gamma.markets().list();

                // Apply preset filters first (can be overridden by explicit flags)
                request = match preset {
                    Some(MarketPreset::Trending) => request
                        .open(true)
                        .volume_num_min(100_000.0)
                        .order("volume24hr")
                        .ascending(false),
                    Some(MarketPreset::TopVolume) => {
                        request.open(true).order("volume").ascending(false)
                    }
                    Some(MarketPreset::HighLiquidity) => request
                        .open(true)
                        .liquidity_num_min(50_000.0)
                        .order("liquidity")
                        .ascending(false),
                    Some(MarketPreset::New) => {
                        request.open(true).order("startDate").ascending(false)
                    }
                    Some(MarketPreset::Competitive) => {
                        request.open(true).order("competitive").ascending(false)
                    }
                    None => request,
                };

                // Apply explicit overrides (these take precedence over presets)
                request = request.limit(limit).offset(offset).open(active);
                match status {
                    MarketStatus::Open => {
                        request = request.closed(false).archived(false);
                    }
                    MarketStatus::Closed => {
                        request = request.closed(true);
                    }
                    MarketStatus::Archived => {
                        request = request.archived(true);
                    }
                }
                if let Some(min) = liquidity_min {
                    request = request.liquidity_num_min(min);
                }
                if let Some(max) = liquidity_max {
                    request = request.liquidity_num_max(max);
                }
                if let Some(min) = volume_min {
                    request = request.volume_num_min(min);
                }
                if let Some(max) = volume_max {
                    request = request.volume_num_max(max);
                }
                request = request.ascending(matches!(sort, SortOrder::Asc));
                if let Some(ord) = order {
                    request = request.order(ord);
                }

                let markets = request.send().await?;
                println!("{}", serde_json::to_string_pretty(&markets)?);
            }
            Self::Get { id } => {
                let market = gamma.markets().get(&id).send().await?;
                println!("{}", serde_json::to_string_pretty(&market)?);
            }
            Self::GetBySlug { slug } => {
                let market = gamma.markets().get_by_slug(&slug).send().await?;
                println!("{}", serde_json::to_string_pretty(&market)?);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> MarketsCommand {
        MarketsCommand::try_parse(args).unwrap()
    }

    fn assert_parse_err(args: &[&str]) {
        assert!(MarketsCommand::try_parse(args).is_err());
    }

    #[test]
    fn list_defaults() {
        let cmd = parse(&["test", "list"]);
        match cmd {
            MarketsCommand::List {
                preset,
                limit,
                offset,
                active,
                status,
                liquidity_min,
                liquidity_max,
                volume_min,
                volume_max,
                sort,
                order,
            } => {
                assert!(preset.is_none());
                assert_eq!(limit, 20);
                assert_eq!(offset, 0);
                assert!(active);
                assert!(matches!(status, MarketStatus::Open));
                assert!(liquidity_min.is_none());
                assert!(liquidity_max.is_none());
                assert!(volume_min.is_none());
                assert!(volume_max.is_none());
                assert!(matches!(sort, SortOrder::Desc));
                assert!(order.is_none());
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_with_preset_trending() {
        let cmd = parse(&["test", "list", "--preset", "trending"]);
        match cmd {
            MarketsCommand::List { preset, .. } => {
                assert!(matches!(preset, Some(MarketPreset::Trending)));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_with_preset_top_volume() {
        let cmd = parse(&["test", "list", "--preset", "top-volume"]);
        match cmd {
            MarketsCommand::List { preset, .. } => {
                assert!(matches!(preset, Some(MarketPreset::TopVolume)));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_with_preset_high_liquidity() {
        let cmd = parse(&["test", "list", "--preset", "high-liquidity"]);
        match cmd {
            MarketsCommand::List { preset, .. } => {
                assert!(matches!(preset, Some(MarketPreset::HighLiquidity)));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_with_preset_new() {
        let cmd = parse(&["test", "list", "--preset", "new"]);
        match cmd {
            MarketsCommand::List { preset, .. } => {
                assert!(matches!(preset, Some(MarketPreset::New)));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_with_preset_competitive() {
        let cmd = parse(&["test", "list", "--preset", "competitive"]);
        match cmd {
            MarketsCommand::List { preset, .. } => {
                assert!(matches!(preset, Some(MarketPreset::Competitive)));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_invalid_preset_errors() {
        assert_parse_err(&["test", "list", "--preset", "nonexistent"]);
    }

    #[test]
    fn list_status_closed() {
        let cmd = parse(&["test", "list", "--status", "closed"]);
        match cmd {
            MarketsCommand::List { status, .. } => {
                assert!(matches!(status, MarketStatus::Closed));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_status_archived() {
        let cmd = parse(&["test", "list", "--status", "archived"]);
        match cmd {
            MarketsCommand::List { status, .. } => {
                assert!(matches!(status, MarketStatus::Archived));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_invalid_status_errors() {
        assert_parse_err(&["test", "list", "--status", "pending"]);
    }

    #[test]
    fn list_sort_asc() {
        let cmd = parse(&["test", "list", "--sort", "asc"]);
        match cmd {
            MarketsCommand::List { sort, .. } => {
                assert!(matches!(sort, SortOrder::Asc));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_custom_limit_and_offset() {
        let cmd = parse(&["test", "list", "-l", "50", "-o", "100"]);
        match cmd {
            MarketsCommand::List { limit, offset, .. } => {
                assert_eq!(limit, 50);
                assert_eq!(offset, 100);
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_volume_and_liquidity_filters() {
        let cmd = parse(&[
            "test",
            "list",
            "--volume-min",
            "1000.5",
            "--volume-max",
            "50000",
            "--liquidity-min",
            "500",
            "--liquidity-max",
            "25000",
        ]);
        match cmd {
            MarketsCommand::List {
                volume_min,
                volume_max,
                liquidity_min,
                liquidity_max,
                ..
            } => {
                assert_eq!(volume_min.unwrap(), 1000.5);
                assert_eq!(volume_max.unwrap(), 50000.0);
                assert_eq!(liquidity_min.unwrap(), 500.0);
                assert_eq!(liquidity_max.unwrap(), 25000.0);
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn list_order_field() {
        let cmd = parse(&["test", "list", "--order", "volume24hr"]);
        match cmd {
            MarketsCommand::List { order, .. } => {
                assert_eq!(order.unwrap(), "volume24hr");
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
        let cmd = parse(&["test", "get", "market-123"]);
        match cmd {
            MarketsCommand::Get { id } => assert_eq!(id, "market-123"),
            _ => panic!("expected Get variant"),
        }
    }

    #[test]
    fn get_by_slug_requires_slug() {
        assert_parse_err(&["test", "get-by-slug"]);
    }

    #[test]
    fn get_by_slug_parses_slug() {
        let cmd = parse(&["test", "get-by-slug", "my-market-slug"]);
        match cmd {
            MarketsCommand::GetBySlug { slug } => assert_eq!(slug, "my-market-slug"),
            _ => panic!("expected GetBySlug variant"),
        }
    }

    #[test]
    fn list_short_flags() {
        // -p for preset, -s for status (short flags)
        let cmd = parse(&["test", "list", "-p", "trending", "-s", "closed"]);
        match cmd {
            MarketsCommand::List { preset, status, .. } => {
                assert!(matches!(preset, Some(MarketPreset::Trending)));
                assert!(matches!(status, MarketStatus::Closed));
            }
            _ => panic!("expected List variant"),
        }
    }
}
