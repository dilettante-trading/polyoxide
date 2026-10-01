//! `polyoxide ws sports`: stream live match updates from the sports feed.

use std::{
    collections::HashMap,
    io::{self, Write},
    time::Duration,
};

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
    /// Leagues to keep, comma-separated, e.g. atp,wta. Matching ignores
    /// case. Omit to keep every league.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub league: Vec<String>,

    /// Games to keep, comma-separated. Takes numeric ids and cricket's id…
    /// ids alike. Omit to keep every game.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub game: Vec<String>,

    /// Skip a frame identical to the last one printed for its game. The
    /// server re-sends unchanged state on a timer, so many frames are
    /// repeats.
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
///
/// Unlike `ws prices`, no Ctrl+C handler is installed. Every printed line is
/// already flushed, and the socket closes when the process exits, so the
/// default signal exit loses nothing.
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
    if let Some(summary) = filter.describe() {
        writeln!(err, "# keeping {summary}")?;
    }
    let mut printed: u64 = 0;
    loop {
        if let Some(n) = args.count {
            if printed >= n {
                writeln!(err, "Reached {n} update(s)")?;
                break;
            }
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
                    match print_update(&update, args.format, out) {
                        Ok(()) => printed += 1,
                        // The reader went away, as with `| head -1`. That
                        // ends the run; it is not an error.
                        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => break,
                        Err(error) => return Err(error.into()),
                    }
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
            // The supervised feed's only `Err`: one frame it could not read.
            Some(Err(SportsError::Decode { raw, source })) => writeln!(
                err,
                "# skipped a frame that did not parse ({source}): {}",
                excerpt(&raw)
            )?,
            Some(Err(error)) => return Err(error.into()),
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
            // An empty entry, as in `--league atp,` or `--league ""`, would
            // match nothing and silently hide every update.
            leagues: args
                .league
                .iter()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_lowercase())
                .collect(),
            games: args
                .game
                .iter()
                .filter(|g| !g.is_empty())
                .cloned()
                .collect(),
            changes_only: args.changes_only,
            last: HashMap::new(),
        }
    }

    /// The active filters, for one line on stderr, so a mistyped league is
    /// not mistaken for a quiet feed.
    fn describe(&self) -> Option<String> {
        let mut parts = Vec::new();
        if !self.leagues.is_empty() {
            parts.push(format!("leagues {}", self.leagues.join(", ")));
        }
        if !self.games.is_empty() {
            parts.push(format!("games {}", self.games.join(", ")));
        }
        if self.changes_only {
            parts.push("changes only".to_owned());
        }
        (!parts.is_empty()).then(|| parts.join("; "))
    }

    /// Whether to print `update`. With `--changes-only` this keeps the last
    /// frame of every game seen, which grows by a few hundred bytes per game
    /// over a session.
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

/// Print one update and flush it, so a reader sees it at once whatever kind of
/// writer `out` is.
fn print_update(update: &MatchUpdate, format: OutputFormat, out: &mut dyn Write) -> io::Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(update)?)?,
        OutputFormat::Pretty => {
            let id = update
                .key()
                .map(|key| key.to_string())
                .unwrap_or_else(|| "-".into());
            let teams = match (&update.home_team, &update.away_team) {
                (Some(home), Some(away)) => fit(&format!("{home} v {away}"), TEAMS_WIDTH),
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
                "{:<16} {:>18}  {:<TEAMS_WIDTH$} {:>18} {:>6}  {}",
                update.league_abbreviation, id, teams, update.score, update.period, state
            )?;
        }
    }
    out.flush()
}

/// The first 200 characters of a frame from the server, escaped, so an
/// oversized frame or one carrying newlines or terminal escapes stays one
/// readable stderr line.
fn excerpt(raw: &str) -> String {
    const LIMIT: usize = 200;
    let escaped: String = raw
        .chars()
        .take(LIMIT)
        .flat_map(char::escape_debug)
        .collect();
    if raw.chars().count() > LIMIT {
        escaped + "…"
    } else {
        escaped
    }
}

/// Width of the teams column in pretty output.
const TEAMS_WIDTH: usize = 44;

/// `text` cut to `width` characters, ending in an ellipsis when cut, so a long
/// pairing does not push the score and period out of their columns.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_owned()
    } else {
        let mut cut: String = text.chars().take(width - 1).collect();
        cut.push('…');
        cut
    }
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

    #[test]
    fn fit_keeps_text_that_fits_and_cuts_on_a_character_boundary() {
        let exact = "x".repeat(TEAMS_WIDTH);
        assert_eq!(fit(&exact, TEAMS_WIDTH), exact);
        let over = "x".repeat(TEAMS_WIDTH + 1);
        assert_eq!(
            fit(&over, TEAMS_WIDTH),
            format!("{}…", "x".repeat(TEAMS_WIDTH - 1))
        );
        // Multibyte characters are counted and cut whole.
        assert_eq!(fit("Ñandú v Ñandú", 6), "Ñandú…");
    }

    #[test]
    fn an_excerpt_is_one_escaped_line_of_bounded_length() {
        assert_eq!(excerpt("a\nb\u{1b}[31m"), "a\\nb\\u{1b}[31m");
        let long = "y".repeat(300);
        let cut = excerpt(&long);
        assert_eq!(cut.chars().count(), 201);
        assert!(cut.ends_with('…'));
    }
}
