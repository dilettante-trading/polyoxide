use std::io::Write;

use clap::{ArgAction, Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_data::{
    v2::types::{FilterType, TradeSide},
    DataApi,
};

use super::paging::{run_paged, PageArgs};
use crate::commands::common::parsing::parse_list_entry;

#[derive(Subcommand)]
pub enum TradesCommand {
    /// List trades: a user's, a market's or event's, or the whole feed (`/v2/trades`)
    List {
        /// User address (0x-prefixed, 40 hex chars)
        #[arg(short, long)]
        user: Option<String>,
        /// Filter by market condition IDs (comma-separated, at most 20)
        #[arg(
            short = 'm',
            long = "condition",
            visible_alias = "market",
            value_delimiter = ',',
            value_parser = parse_list_entry
        )]
        condition: Option<Vec<String>>,
        /// Filter by event IDs (comma-separated, at most 20)
        #[arg(short, long, value_delimiter = ',', value_parser = parse_list_entry)]
        event_id: Option<Vec<String>>,
        /// Filter by trade side
        #[arg(short, long, value_enum)]
        side: Option<TradeSideFilter>,
        /// Only taker trades; pass `--taker-only false` to include maker fills
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        taker_only: bool,
        /// Filter type (must be paired with --filter-amount)
        #[arg(long, value_enum)]
        filter_type: Option<TradeFilterField>,
        /// Filter amount (must be paired with --filter-type)
        #[arg(long)]
        filter_amount: Option<f64>,
        /// Window start, epoch seconds; honoured with --user only (1 for full history)
        #[arg(long)]
        start: Option<i64>,
        /// Window end, epoch seconds; honoured with --user only
        #[arg(long)]
        end: Option<i64>,
        /// Page size (at most 1000)
        #[arg(short, long, default_value = "100")]
        limit: u32,
        #[command(flatten)]
        page: PageArgs,
    },
}

impl TradesCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        match self {
            Self::List {
                user,
                condition,
                event_id,
                side,
                taker_only,
                filter_type,
                filter_amount,
                start,
                end,
                limit,
                page,
            } => {
                let mut request = data.v2().trades().taker_only(taker_only).limit(limit);
                if let Some(user) = user {
                    request = request.user(user);
                }
                if let Some(ids) = condition {
                    request = request.conditions(ids);
                }
                if let Some(ids) = event_id {
                    request = request.event_ids(ids);
                }
                if let Some(side) = side {
                    request = request.side(side.into());
                }
                if let Some(filter_type) = filter_type {
                    request = request.filter_type(filter_type.into());
                }
                if let Some(amount) = filter_amount {
                    request = request.filter_amount(amount);
                }
                if let Some(ts) = start {
                    request = request.start(ts);
                }
                if let Some(ts) = end {
                    request = request.end(ts);
                }
                run_paged(request, &page, out, err).await
            }
        }
    }
}

/// Trade side filter
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum TradeSideFilter {
    /// Buy trades
    Buy,
    /// Sell trades
    Sell,
}

impl From<TradeSideFilter> for TradeSide {
    fn from(side: TradeSideFilter) -> Self {
        match side {
            TradeSideFilter::Buy => Self::Buy,
            TradeSideFilter::Sell => Self::Sell,
        }
    }
}

/// Unit of a filter amount
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum TradeFilterField {
    /// Cash amount (USDC)
    Cash,
    /// Token amount (shares)
    Tokens,
}

impl From<TradeFilterField> for FilterType {
    fn from(filter: TradeFilterField) -> Self {
        match filter {
            TradeFilterField::Cash => Self::Cash,
            TradeFilterField::Tokens => Self::Tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn try_parse(args: &[&str]) -> Result<TradesCommand, clap::Error> {
        #[derive(Parser)]
        struct Wrapper {
            #[command(subcommand)]
            cmd: TradesCommand,
        }
        Wrapper::try_parse_from(args).map(|w| w.cmd)
    }

    #[test]
    fn trade_side_filter_maps_to_the_v2_side() {
        assert_eq!(TradeSide::from(TradeSideFilter::Buy), TradeSide::Buy);
        assert_eq!(TradeSide::from(TradeSideFilter::Sell), TradeSide::Sell);
    }

    #[test]
    fn trade_filter_field_maps_to_the_v2_filter_type() {
        assert_eq!(FilterType::from(TradeFilterField::Cash), FilterType::Cash);
        assert_eq!(
            FilterType::from(TradeFilterField::Tokens),
            FilterType::Tokens
        );
    }

    #[test]
    fn list_defaults() {
        let TradesCommand::List {
            user,
            condition,
            event_id,
            side,
            taker_only,
            filter_type,
            filter_amount,
            start,
            end,
            limit,
            page,
        } = try_parse(&["test", "list"]).unwrap();
        assert!(user.is_none());
        assert!(condition.is_none());
        assert!(event_id.is_none());
        assert!(side.is_none());
        assert!(taker_only);
        assert!(filter_type.is_none());
        assert!(filter_amount.is_none());
        assert!(start.is_none() && end.is_none());
        assert_eq!(limit, 100);
        assert_eq!(page, PageArgs::default());
    }

    #[test]
    fn market_is_an_alias_of_condition() {
        for flag in ["--condition", "--market", "-m"] {
            let TradesCommand::List { condition, .. } =
                try_parse(&["test", "list", flag, "0xa,0xb"]).unwrap();
            assert_eq!(condition.unwrap(), ["0xa", "0xb"], "{flag}");
        }
    }

    #[test]
    fn list_flags_split_on_commas_and_trim_each_entry() {
        let TradesCommand::List {
            condition,
            event_id,
            ..
        } = try_parse(&[
            "test",
            "list",
            "--condition",
            " 0xa , 0xb ",
            "-e",
            "1",
            "-e",
            "2",
        ])
        .unwrap();
        assert_eq!(condition.unwrap(), ["0xa", "0xb"]);
        assert_eq!(event_id.unwrap(), ["1", "2"], "a repeated flag appends");
    }

    #[test]
    fn taker_only_can_be_turned_off() {
        let TradesCommand::List { taker_only, .. } =
            try_parse(&["test", "list", "--taker-only", "false"]).unwrap();
        assert!(!taker_only);
    }

    #[test]
    fn list_invalid_side_errors() {
        assert!(try_parse(&["test", "list", "--side", "short"]).is_err());
    }

    #[test]
    fn list_invalid_filter_type_errors() {
        assert!(try_parse(&["test", "list", "--filter-type", "volume"]).is_err());
    }

    #[test]
    fn offset_is_refused_with_a_pointer_to_cursor() {
        for args in [
            &["test", "list", "--offset", "100"][..],
            &["test", "list", "-o", "100"][..],
        ] {
            let message = try_parse(args)
                .err()
                .expect("offset must be refused")
                .to_string();
            assert!(message.contains("--cursor"), "{message}");
        }
    }

    #[test]
    fn cursor_all_and_max_pages_parse() {
        let TradesCommand::List { page, .. } = try_parse(&[
            "test",
            "list",
            "--cursor",
            "c1",
            "--all",
            "--max-pages",
            "3",
        ])
        .unwrap();
        assert_eq!(page.cursor.as_deref(), Some("c1"));
        assert!(page.all);
        assert_eq!(page.max_pages, Some(3));
    }

    #[test]
    fn max_pages_requires_all() {
        assert!(try_parse(&["test", "list", "--max-pages", "3"]).is_err());
    }
}
