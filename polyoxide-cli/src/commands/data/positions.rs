use std::io::Write;

use clap::{Args, Subcommand, ValueEnum};
use color_eyre::eyre::{bail, Result};
use polyoxide_data::{
    v2::types::{PositionAnchor, PositionSortBy, PositionStatus},
    DataApi,
};

use super::activity::ActivityFilters;
use super::paging::{print_pretty, run_paged, PageArgs};
use super::SortOrder;
use crate::commands::common::parsing::parse_list_entry;
use crate::commands::data::trades::TradeFilterField;

#[derive(Args)]
pub struct PositionsCommand {
    /// User address (0x-prefixed, 40 hex chars)
    #[arg(short, long)]
    pub user: String,

    #[command(subcommand)]
    pub command: PositionsSubcommand,
}

#[derive(Subcommand)]
pub enum PositionsSubcommand {
    /// List the user's positions (`/v2/positions`); --status selects open, redeemable or closed
    List {
        /// Filter by market condition IDs (comma-separated, at most 20)
        #[arg(
            short = 'm',
            long = "condition",
            visible_alias = "market",
            value_delimiter = ',',
            value_parser = parse_list_entry
        )]
        condition: Option<Vec<String>>,
        /// Filter by event IDs (comma-separated)
        #[arg(short, long, value_delimiter = ',', value_parser = parse_list_entry)]
        event_id: Option<Vec<String>>,
        /// Lifecycle state (open includes redeemable positions)
        #[arg(long, value_enum, ignore_case = true, default_value = "open")]
        status: PositionStatusFilter,
        /// Filter by market title (case-insensitive substring, at most 200 chars)
        #[arg(short, long)]
        title: Option<String>,
        /// Unit of --filter-amount (API default: tokens)
        #[arg(long, value_enum)]
        filter_type: Option<TradeFilterField>,
        /// Minimum current holding, in the unit of --filter-type
        #[arg(long)]
        filter_amount: Option<f64>,
        /// Include positions on archived markets (open and redeemable only)
        #[arg(long)]
        include_archived: bool,
        /// Sort field (API default depends on --status)
        #[arg(long, value_enum)]
        sort_by: Option<PositionSortField>,
        /// Sort direction
        #[arg(long, value_enum, default_value = "desc")]
        sort_direction: SortOrder,
        /// Only positions whose last event is at or after this epoch second
        #[arg(long)]
        start: Option<i64>,
        /// Only positions whose last event is at or before this epoch second
        #[arg(long)]
        end: Option<i64>,
        /// Page size (at most 1000)
        #[arg(short, long, default_value = "100")]
        limit: u32,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get the total value of the user's positions (`/v2/value`)
    Value {
        /// Value only these market condition IDs (comma-separated, at most 20)
        #[arg(
            short = 'm',
            long = "condition",
            visible_alias = "market",
            value_delimiter = ',',
            value_parser = parse_list_entry
        )]
        condition: Option<Vec<String>>,
    },
    /// Removed: use `positions list --status closed`
    #[command(hide = true)]
    Closed {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _ignored: Vec<String>,
    },
    /// List activity for the user (`/v2/activity`)
    Activity(ActivityFilters),
}

impl PositionsCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        match self.command {
            PositionsSubcommand::List {
                condition,
                event_id,
                status,
                title,
                filter_type,
                filter_amount,
                include_archived,
                sort_by,
                sort_direction,
                start,
                end,
                limit,
                page,
            } => {
                let anchor = match condition {
                    Some(conditions) => PositionAnchor::UserInConditions {
                        user: self.user,
                        conditions,
                    },
                    None => PositionAnchor::User(self.user),
                };
                let mut request = data
                    .v2()
                    .positions(anchor)
                    .status(status.into())
                    .sort_direction(sort_direction.into())
                    .limit(limit);
                if let Some(ids) = event_id {
                    request = request.event_ids(ids);
                }
                if let Some(title) = title {
                    request = request.title(title);
                }
                if let Some(filter_type) = filter_type {
                    request = request.filter_type(filter_type.into());
                }
                if let Some(amount) = filter_amount {
                    request = request.filter_amount(amount);
                }
                if include_archived {
                    request = request.include_archived(true);
                }
                if let Some(sort_by) = sort_by {
                    request = request.sort_by(sort_by.into());
                }
                if let Some(ts) = start {
                    request = request.start(ts);
                }
                if let Some(ts) = end {
                    request = request.end(ts);
                }
                run_paged(request, &page, out, err).await
            }
            PositionsSubcommand::Value { condition } => {
                let mut request = data.v2().value(self.user);
                if let Some(ids) = condition {
                    request = request.conditions(ids);
                }
                print_pretty(&request.send().await?, out)
            }
            PositionsSubcommand::Closed { .. } => {
                bail!("`positions closed` was removed: use `positions list --status closed`")
            }
            PositionsSubcommand::Activity(filters) => filters.run(data, &self.user, out, err).await,
        }
    }
}

/// Position lifecycle state
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq)]
pub enum PositionStatusFilter {
    /// Open positions, including settled-but-unredeemed winners
    #[default]
    Open,
    /// Only positions that can be redeemed now
    Redeemable,
    /// Exited positions
    Closed,
}

impl From<PositionStatusFilter> for PositionStatus {
    fn from(status: PositionStatusFilter) -> Self {
        match status {
            PositionStatusFilter::Open => Self::Open,
            PositionStatusFilter::Redeemable => Self::Redeemable,
            PositionStatusFilter::Closed => Self::Closed,
        }
    }
}

/// Sort field for positions
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum PositionSortField {
    /// Mark-to-market value
    CurrentValue,
    /// Token count
    Tokens,
    /// Unrealized P&L
    UnrealizedPnl,
    /// Realized P&L
    RealizedPnl,
    /// Total P&L
    TotalPnl,
    /// Time of the position's last event
    Timestamp,
}

impl From<PositionSortField> for PositionSortBy {
    fn from(field: PositionSortField) -> Self {
        match field {
            PositionSortField::CurrentValue => Self::CurrentValue,
            PositionSortField::Tokens => Self::Tokens,
            PositionSortField::UnrealizedPnl => Self::UnrealizedPnl,
            PositionSortField::RealizedPnl => Self::RealizedPnl,
            PositionSortField::TotalPnl => Self::TotalPnl,
            PositionSortField::Timestamp => Self::Timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::commands::data::trades::TradeSideFilter;

    #[derive(Parser)]
    struct TestWrapper {
        #[command(flatten)]
        cmd: PositionsCommand,
    }

    fn try_parse(args: &[&str]) -> Result<TestWrapper, clap::Error> {
        TestWrapper::try_parse_from(args)
    }

    #[test]
    fn status_filter_maps_to_the_v2_status() {
        assert_eq!(
            PositionStatus::from(PositionStatusFilter::Open),
            PositionStatus::Open
        );
        assert_eq!(
            PositionStatus::from(PositionStatusFilter::Redeemable),
            PositionStatus::Redeemable
        );
        assert_eq!(
            PositionStatus::from(PositionStatusFilter::Closed),
            PositionStatus::Closed
        );
    }

    #[test]
    fn sort_field_maps_to_the_v2_sort() {
        let pairs = [
            (
                PositionSortField::CurrentValue,
                PositionSortBy::CurrentValue,
            ),
            (PositionSortField::Tokens, PositionSortBy::Tokens),
            (
                PositionSortField::UnrealizedPnl,
                PositionSortBy::UnrealizedPnl,
            ),
            (PositionSortField::RealizedPnl, PositionSortBy::RealizedPnl),
            (PositionSortField::TotalPnl, PositionSortBy::TotalPnl),
            (PositionSortField::Timestamp, PositionSortBy::Timestamp),
        ];
        for (field, expected) in pairs {
            assert_eq!(PositionSortBy::from(field), expected);
        }
    }

    #[test]
    fn positions_requires_user_flag() {
        assert!(try_parse(&["test", "list"]).is_err());
    }

    #[test]
    fn positions_list_defaults() {
        let w = try_parse(&["test", "--user", "0xabc", "list"]).unwrap();
        assert_eq!(w.cmd.user, "0xabc");
        match w.cmd.command {
            PositionsSubcommand::List {
                status,
                sort_by,
                sort_direction,
                limit,
                include_archived,
                page,
                ..
            } => {
                assert_eq!(status, PositionStatusFilter::Open);
                assert!(sort_by.is_none(), "the API picks a default by status");
                assert!(matches!(sort_direction, SortOrder::Desc));
                assert_eq!(limit, 100);
                assert!(!include_archived);
                assert_eq!(page, PageArgs::default());
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn positions_list_status_and_sort_parse() {
        let w = try_parse(&[
            "test",
            "--user",
            "0xabc",
            "list",
            "--status",
            "closed",
            "--sort-by",
            "realized-pnl",
        ])
        .unwrap();
        match w.cmd.command {
            PositionsSubcommand::List {
                status, sort_by, ..
            } => {
                assert_eq!(status, PositionStatusFilter::Closed);
                assert_eq!(sort_by, Some(PositionSortField::RealizedPnl));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn status_accepts_the_upstream_spelling() {
        let w = try_parse(&["test", "--user", "0xabc", "list", "--status", "REDEEMABLE"]).unwrap();
        match w.cmd.command {
            PositionsSubcommand::List { status, .. } => {
                assert_eq!(status, PositionStatusFilter::Redeemable)
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn v1_only_list_flags_are_gone() {
        for flag in ["--redeemable", "--mergeable"] {
            assert!(
                try_parse(&["test", "--user", "0xabc", "list", flag]).is_err(),
                "{flag}"
            );
        }
        assert!(try_parse(&["test", "--user", "0xabc", "list", "--size-threshold", "1"]).is_err());
    }

    #[test]
    fn positions_value_parses_with_market_alias() {
        let w = try_parse(&["test", "--user", "0xabc", "value", "--market", "0xc"]).unwrap();
        match w.cmd.command {
            PositionsSubcommand::Value { condition } => assert_eq!(condition.unwrap(), ["0xc"]),
            _ => panic!("expected Value"),
        }
    }

    #[test]
    fn closed_still_parses_so_it_can_explain_its_replacement() {
        let w = try_parse(&["test", "--user", "0xabc", "closed", "--limit", "20"]).unwrap();
        assert!(matches!(w.cmd.command, PositionsSubcommand::Closed { .. }));
    }

    #[test]
    fn positions_activity_parses() {
        let w = try_parse(&["test", "--user", "0xabc", "activity", "--side", "buy"]).unwrap();
        match w.cmd.command {
            PositionsSubcommand::Activity(filters) => {
                assert!(matches!(filters.side, Some(TradeSideFilter::Buy)));
            }
            _ => panic!("expected Activity"),
        }
    }

    #[test]
    fn positions_requires_subcommand() {
        assert!(try_parse(&["test", "--user", "0xabc"]).is_err());
    }
}
