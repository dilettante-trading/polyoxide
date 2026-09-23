use std::io::Write;

use clap::Args;
use color_eyre::eyre::Result;
use polyoxide_data::DataApi;

use super::paging::{run_paged, PageArgs};
use super::SortOrder;
use crate::commands::common::parsing::{parse_activity_types, parse_list_entry};
use crate::commands::data::trades::TradeSideFilter;

/// Query a user's activity (`/v2/activity`)
#[derive(Args)]
pub struct UserActivityCommand {
    /// User address (0x-prefixed, 40 hex chars)
    #[arg(short, long)]
    pub user: String,
    #[command(flatten)]
    pub filters: ActivityFilters,
}

impl UserActivityCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        self.filters.run(data, &self.user, out, err).await
    }
}

/// Filters shared by `data activity` and `data positions activity`
#[derive(Args, Debug, Clone, PartialEq)]
pub struct ActivityFilters {
    /// Filter by market condition IDs (comma-separated, at most 20)
    #[arg(
        short = 'm',
        long = "condition",
        visible_alias = "market",
        value_delimiter = ',',
        value_parser = parse_list_entry
    )]
    pub condition: Option<Vec<String>>,
    /// Filter by event IDs (comma-separated, at most 20)
    #[arg(short, long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub event_id: Option<Vec<String>>,
    /// Filter by activity types (comma-separated, e.g. trade,split,tip)
    #[arg(short = 'T', long)]
    pub activity_type: Option<String>,
    /// Filter trade rows by side
    #[arg(short, long, value_enum)]
    pub side: Option<TradeSideFilter>,
    /// Window start, epoch seconds (default: three years back; 1 for full history)
    #[arg(long)]
    pub start: Option<i64>,
    /// Window end, epoch seconds
    #[arg(long)]
    pub end: Option<i64>,
    /// Include deposit and withdrawal rows, which the API hides by default
    #[arg(long)]
    pub include_deposits_withdrawals: bool,
    /// Page size (at most 1000)
    #[arg(short, long, default_value = "100")]
    pub limit: u32,
    /// Sort direction (rows are always sorted by timestamp)
    #[arg(long, value_enum, default_value = "desc")]
    pub sort_direction: SortOrder,
    #[command(flatten)]
    pub page: PageArgs,
}

impl ActivityFilters {
    pub async fn run(
        self,
        data: &DataApi,
        user: &str,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<()> {
        let mut request = data
            .v2()
            .activity(user)
            .limit(self.limit)
            .sort_direction(self.sort_direction.into());
        if let Some(ids) = self.condition {
            request = request.conditions(ids);
        }
        if let Some(ids) = self.event_id {
            request = request.event_ids(ids);
        }
        if let Some(types) = self.activity_type {
            request = request.types(parse_activity_types(&types)?);
        }
        if let Some(side) = self.side {
            request = request.side(side.into());
        }
        if let Some(ts) = self.start {
            request = request.start(ts);
        }
        if let Some(ts) = self.end {
            request = request.end(ts);
        }
        if self.include_deposits_withdrawals {
            request = request.exclude_deposits_withdrawals(false);
        }
        run_paged(request, &self.page, out, err).await
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        cmd: UserActivityCommand,
    }

    fn try_parse(args: &[&str]) -> Result<UserActivityCommand, clap::Error> {
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    #[test]
    fn activity_defaults() {
        let cmd = try_parse(&["test", "--user", "0xabc"]).unwrap();
        assert_eq!(cmd.user, "0xabc");
        assert_eq!(cmd.filters.limit, 100);
        assert!(matches!(cmd.filters.sort_direction, SortOrder::Desc));
        assert!(!cmd.filters.include_deposits_withdrawals);
        assert_eq!(cmd.filters.page, PageArgs::default());
    }

    #[test]
    fn sort_by_is_gone_because_only_timestamp_is_supported() {
        assert!(try_parse(&["test", "--user", "0xabc", "--sort-by", "tokens"]).is_err());
    }

    #[test]
    fn market_is_an_alias_of_condition() {
        let cmd = try_parse(&["test", "--user", "0xabc", "--market", "0xc"]).unwrap();
        assert_eq!(cmd.filters.condition.unwrap(), ["0xc"]);
    }
}
