//! `polyoxide ws sports`: stream live match updates from the sports feed.

use std::{collections::HashMap, io::Write, time::Duration};

use clap::Args;
use color_eyre::eyre::Result;
use futures_util::{Stream, StreamExt};
use polyoxide_sports::{Event, GameKey, MatchUpdate, SportsError, SportsWsBuilder};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};

/// How each update is printed.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable, one line per update.
    #[default]
    Pretty,
    /// The update as compact JSON, one object per line.
    Json,
}

#[derive(Args, Debug)]
pub struct SportsArgs {
    /// Leagues to keep, comma-separated, e.g. `atp,wta`. Matching ignores
    /// case. Omit to keep every league.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub league: Vec<String>,

    /// Games to keep, comma-separated. Takes numeric ids and cricket's `id…`
    /// ids alike. Omit to keep every game.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub game: Vec<String>,

    /// Skip a frame identical to the last one printed for its game. The
    /// server re-sends unchanged state on a timer, so about half of all
    /// frames are repeats.
    #[arg(long)]
    pub changes_only: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,

    /// Exit after printing N updates
    #[arg(short = 'n', long)]
    pub count: Option<u64>,

    /// Exit after the given duration (e.g. "30s", "5m")
    #[arg(short, long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
}

/// Connect to the production feed and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(args: SportsArgs) -> Result<()> {
    eprintln!("Connecting to the sports feed...");
    let feed = SportsWsBuilder::new().connect().await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
    run_with(args, feed, &mut std::io::stdout(), &mut std::io::stderr()).await
}

/// Filter and print events from any stream until `-n` or `-t` is reached or
/// the stream ends.
///
/// Takes the stream rather than connecting, so tests can drive every flag
/// with a scripted list of events. Updates go to `out`; connection markers
/// and skipped frames go to `err`, so JSON output stays clean JSONL.
pub async fn run_with<S>(
    args: SportsArgs,
    mut events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<Event, SportsError>> + Unpin,
{
    let deadline = args.timeout.map(|t| tokio::time::Instant::now() + t);
    let mut filter = Filter::new(&args);
    let mut printed: u64 = 0;
    loop {
        if args.count.is_some_and(|n| printed >= n) {
            break;
        }
        let next = match deadline {
            Some(deadline) => match tokio::time::timeout_at(deadline, events.next()).await {
                Ok(next) => next,
                Err(_) => {
                    writeln!(err, "Timeout reached")?;
                    break;
                }
            },
            None => events.next().await,
        };
        match next {
            Some(Ok(Event::Update(update))) => {
                if filter.admits(&update) {
                    print_update(&update, args.format, out)?;
                    printed += 1;
                }
            }
            Some(Ok(Event::Disconnected { reason })) => writeln!(
                err,
                "# disconnected: {reason}. Scores are stale until reconnected."
            )?,
            Some(Ok(Event::Reconnected)) => writeln!(
                err,
                "# reconnected. Games that ended during the gap were not re-sent."
            )?,
            // `Event` is #[non_exhaustive]; a future variant is not a fault.
            Some(Ok(_)) => {}
            Some(Err(error)) => writeln!(err, "# skipped a frame: {error}")?,
            None => {
                writeln!(err, "The feed ended")?;
                break;
            }
        }
    }
    Ok(())
}

/// `--league`, `--game` and `--changes-only` as one decision per update.
struct Filter {
    leagues: Vec<String>,
    games: Vec<String>,
    changes_only: bool,
    last: HashMap<GameKey, MatchUpdate>,
}

impl Filter {
    fn new(args: &SportsArgs) -> Self {
        Self {
            leagues: args.league.iter().map(|l| l.to_lowercase()).collect(),
            games: args.game.clone(),
            changes_only: args.changes_only,
            last: HashMap::new(),
        }
    }

    fn admits(&mut self, update: &MatchUpdate) -> bool {
        if !self.leagues.is_empty()
            && !self
                .leagues
                .contains(&update.league_abbreviation.to_lowercase())
        {
            return false;
        }
        let key = update.key();
        if !self.games.is_empty() {
            match &key {
                Some(key) if self.games.contains(&key.to_string()) => {}
                _ => return false,
            }
        }
        if self.changes_only {
            if let Some(key) = key {
                if self.last.get(&key) == Some(update) {
                    return false;
                }
                self.last.insert(key, update.clone());
            }
        }
        true
    }
}

fn print_update(update: &MatchUpdate, format: OutputFormat, out: &mut dyn Write) -> Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(update)?)?,
        OutputFormat::Pretty => {
            let id = update
                .key()
                .map(|key| key.to_string())
                .unwrap_or_else(|| "-".into());
            let teams = match (&update.home_team, &update.away_team) {
                (Some(home), Some(away)) => format!("{home} v {away}"),
                _ => "-".into(),
            };
            let state = if update.ended {
                "ended"
            } else if update.live {
                "live"
            } else {
                "not started"
            };
            writeln!(
                out,
                "{:<16} {:>20}  {:<44} {:>18} {:>6}  {}",
                update.league_abbreviation, id, teams, update.score, update.period, state
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: SportsArgs,
    }

    fn parse(argv: &[&str]) -> SportsArgs {
        Wrapper::try_parse_from(argv).unwrap().args
    }

    #[test]
    fn league_and_game_split_on_commas_and_are_trimmed() {
        let args = parse(&[
            "test",
            "--league",
            "atp, wta challenger",
            "--game",
            "1712005,id2704098174740616",
        ]);
        assert_eq!(args.league, ["atp", "wta challenger"]);
        assert_eq!(args.game, ["1712005", "id2704098174740616"]);
    }

    #[test]
    fn repeated_flags_accumulate() {
        let args = parse(&["test", "--league", "atp", "--league", "wta"]);
        assert_eq!(args.league, ["atp", "wta"]);
    }

    #[test]
    fn defaults_keep_everything_forever() {
        let args = parse(&["test"]);
        assert!(args.league.is_empty() && args.game.is_empty());
        assert!(!args.changes_only);
        assert_eq!(args.format, OutputFormat::Pretty);
        assert_eq!(args.count, None);
        assert_eq!(args.timeout, None);
    }

    #[test]
    fn count_and_timeout_parse() {
        let args = parse(&["test", "-n", "3", "-t", "5m", "--format", "json"]);
        assert_eq!(args.count, Some(3));
        assert_eq!(args.timeout, Some(Duration::from_secs(300)));
        assert_eq!(args.format, OutputFormat::Json);
    }
}
