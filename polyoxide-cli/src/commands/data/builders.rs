use std::io::Write;

use clap::{Args, Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use polyoxide_data::{v2::types::TimePeriod, DataApi};

use super::paging::{print_pretty, run_paged, PageArgs};

#[derive(Subcommand)]
pub enum BuildersCommand {
    /// Get the builder leaderboard (`/v2/builders/leaderboard`)
    Leaderboard(LeaderboardCommand),
    /// Get builder volume per time bucket (`/v2/builders/volume`)
    Volume(VolumeCommand),
}

impl BuildersCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        match self {
            Self::Leaderboard(cmd) => cmd.run(data, out, err).await,
            Self::Volume(cmd) => cmd.run(data, out).await,
        }
    }
}

/// Get the builder leaderboard
#[derive(Args)]
pub struct LeaderboardCommand {
    /// Time period for aggregation
    #[arg(short, long, default_value = "day")]
    pub time_period: CliTimePeriod,
    /// Page size (at most 1000)
    #[arg(short, long, default_value = "25")]
    pub limit: u32,
    #[command(flatten)]
    pub page: PageArgs,
}

impl LeaderboardCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
        let request = data
            .v2()
            .builders_leaderboard()
            .time_period(self.time_period.into())
            .limit(self.limit);
        run_paged(request, &self.page, out, err).await
    }
}

/// Get builder volume per time bucket
#[derive(Args)]
pub struct VolumeCommand {
    /// Bucket width
    #[arg(short, long, default_value = "day")]
    pub time_period: CliTimePeriod,
    /// Most recent buckets to return (at most 90; API default 30)
    #[arg(short, long)]
    pub limit: Option<u32>,
}

impl VolumeCommand {
    pub async fn run(self, data: &DataApi, out: &mut dyn Write) -> Result<()> {
        let mut request = data.v2().builder_volume().interval(self.time_period.into());
        if let Some(limit) = self.limit {
            request = request.limit(limit);
        }
        print_pretty(&request.send().await?, out)
    }
}

/// Time period for aggregation
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum CliTimePeriod {
    /// Daily aggregation
    #[default]
    Day,
    /// Weekly aggregation
    Week,
    /// Monthly aggregation
    Month,
    /// All time aggregation
    All,
}

impl From<CliTimePeriod> for TimePeriod {
    fn from(period: CliTimePeriod) -> Self {
        match period {
            CliTimePeriod::Day => TimePeriod::Day,
            CliTimePeriod::Week => TimePeriod::Week,
            CliTimePeriod::Month => TimePeriod::Month,
            CliTimePeriod::All => TimePeriod::All,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestLeaderboard {
        #[command(flatten)]
        cmd: LeaderboardCommand,
    }

    #[derive(Parser)]
    struct TestVolume {
        #[command(flatten)]
        cmd: VolumeCommand,
    }

    #[test]
    fn leaderboard_defaults() {
        let parsed = TestLeaderboard::try_parse_from(["test"]).unwrap();
        assert!(matches!(parsed.cmd.time_period, CliTimePeriod::Day));
        assert_eq!(parsed.cmd.limit, 25);
        assert_eq!(parsed.cmd.page, PageArgs::default());
    }

    #[test]
    fn leaderboard_time_periods_parse() {
        for (flag, expected) in [
            ("week", TimePeriod::Week),
            ("month", TimePeriod::Month),
            ("all", TimePeriod::All),
        ] {
            let parsed = TestLeaderboard::try_parse_from(["test", "--time-period", flag]).unwrap();
            assert_eq!(TimePeriod::from(parsed.cmd.time_period), expected);
        }
    }

    #[test]
    fn leaderboard_invalid_time_period_errors() {
        assert!(TestLeaderboard::try_parse_from(["test", "--time-period", "year"]).is_err());
    }

    #[test]
    fn leaderboard_offset_is_refused_with_a_pointer_to_cursor() {
        let message = TestLeaderboard::try_parse_from(["test", "-l", "10", "-o", "50"])
            .err()
            .expect("offset must be refused")
            .to_string();
        assert!(message.contains("--cursor"), "{message}");
    }

    #[test]
    fn volume_defaults() {
        let parsed = TestVolume::try_parse_from(["test"]).unwrap();
        assert!(matches!(parsed.cmd.time_period, CliTimePeriod::Day));
        assert!(parsed.cmd.limit.is_none());
    }

    #[test]
    fn time_period_maps_to_the_v2_period() {
        assert_eq!(TimePeriod::from(CliTimePeriod::Day), TimePeriod::Day);
        assert_eq!(TimePeriod::from(CliTimePeriod::Week), TimePeriod::Week);
        assert_eq!(TimePeriod::from(CliTimePeriod::Month), TimePeriod::Month);
        assert_eq!(TimePeriod::from(CliTimePeriod::All), TimePeriod::All);
    }
}
