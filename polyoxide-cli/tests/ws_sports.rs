//! `polyoxide ws sports` over scripted event streams.
//!
//! Each test parses real arguments and runs the command over events built
//! from captured frames, so a flag that parses but never reaches the filter
//! fails here.

use std::time::Duration;

use clap::Parser;
use futures_util::stream;
use polyoxide_cli::commands::ws::sports::{run_with, SportsArgs};
use polyoxide_sports::{fixtures, Event, MatchUpdate, SportsError};
use serde_json::Value;

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    args: SportsArgs,
}

type Item = Result<Event, SportsError>;

fn update(frame: &str) -> Item {
    Ok(Event::Update(Box::new(
        MatchUpdate::from_json(frame).unwrap(),
    )))
}

fn with_score(frame: &str, score: &str) -> Item {
    let mut value: Value = serde_json::from_str(frame).unwrap();
    value["score"] = Value::from(score);
    Ok(Event::Update(Box::new(
        serde_json::from_value(value).unwrap(),
    )))
}

/// What one run wrote.
struct Run {
    out: String,
    err: String,
}

impl Run {
    /// Each stdout line, parsed as JSON.
    fn rows(&self) -> Vec<Value> {
        self.out
            .lines()
            .map(|line| {
                serde_json::from_str(line)
                    .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line}"))
            })
            .collect()
    }

    /// One string field from every row.
    fn field(&self, name: &str) -> Vec<String> {
        self.rows()
            .iter()
            .map(|row| row[name].as_str().unwrap().to_owned())
            .collect()
    }
}

async fn run_argv(argv: &[&str], events: Vec<Item>) -> Run {
    let cli = Cli::try_parse_from(argv).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    run_with(cli.args, stream::iter(events), &mut out, &mut err)
        .await
        .unwrap();
    Run {
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// Run with JSON output plus `flags`.
async fn run(flags: &[&str], events: Vec<Item>) -> Run {
    let mut argv = vec!["sports", "--format", "json"];
    argv.extend_from_slice(flags);
    run_argv(&argv, events).await
}

#[tokio::test]
async fn every_update_is_printed_without_filters() {
    let run = run(
        &[],
        vec![update(fixtures::SOCCER), update(fixtures::CRICKET)],
    )
    .await;
    assert_eq!(run.field("leagueAbbreviation"), ["kor", "cricket"]);
}

#[tokio::test]
async fn the_league_filter_splits_on_commas_and_ignores_case() {
    let run = run(
        &["--league", "ATP,Wta Challenger"],
        vec![
            update(fixtures::TENNIS_EVENT_STATE),
            update(fixtures::LEAGUE_WITH_SPACE),
            update(fixtures::SOCCER),
        ],
    )
    .await;
    assert_eq!(run.field("leagueAbbreviation"), ["atp", "wta challenger"]);
}

#[tokio::test]
async fn the_game_filter_matches_both_kinds_of_id() {
    let run = run(
        &["--game", "90106111,id2703680373085574"],
        vec![
            update(fixtures::SOCCER),
            update(fixtures::ESPORTS),
            update(fixtures::CRICKET),
        ],
    )
    .await;
    assert_eq!(run.field("leagueAbbreviation"), ["kor", "cricket"]);
}

#[tokio::test]
async fn changes_only_drops_repeats_and_keeps_changes() {
    let events = || {
        vec![
            update(fixtures::SOCCER),
            update(fixtures::SOCCER),
            with_score(fixtures::SOCCER, "3-1"),
            with_score(fixtures::SOCCER, "3-1"),
            update(fixtures::ESPORTS),
        ]
    };
    let changes = run(&["--changes-only"], events()).await;
    assert_eq!(changes.field("score"), ["2-1", "3-1", "000-000|0-0|Bo1"]);
    let everything = run(&[], events()).await;
    assert_eq!(
        everything.rows().len(),
        5,
        "without --changes-only every frame is printed"
    );
}

#[tokio::test]
async fn count_stops_after_n_printed_updates_not_n_received() {
    let run = run(
        &["-n", "2", "--league", "cricket"],
        vec![
            update(fixtures::SOCCER),
            update(fixtures::CRICKET),
            update(fixtures::ESPORTS),
            update(fixtures::CRICKET_FINISHED),
            update(fixtures::CRICKET),
        ],
    )
    .await;
    assert_eq!(run.field("score"), ["21-178", "116-38"]);
}

#[tokio::test]
async fn markers_go_to_stderr_and_stdout_stays_jsonl() {
    let run = run(
        &[],
        vec![
            update(fixtures::SOCCER),
            Ok(Event::Disconnected {
                reason: SportsError::Stale {
                    after: Duration::from_secs(45),
                },
            }),
            Ok(Event::Reconnected),
            update(fixtures::ESPORTS),
        ],
    )
    .await;
    assert_eq!(run.rows().len(), 2);
    assert!(
        run.err.contains("disconnected") && run.err.contains("pings included"),
        "{}",
        run.err
    );
    assert!(run.err.contains("reconnected"), "{}", run.err);
}

#[tokio::test]
async fn a_bad_frame_is_reported_and_streaming_continues() {
    let source = serde_json::from_str::<Value>("not json").unwrap_err();
    let run = run(
        &[],
        vec![
            Err(SportsError::Decode {
                raw: "not json".into(),
                source,
            }),
            update(fixtures::SOCCER),
        ],
    )
    .await;
    assert_eq!(run.field("leagueAbbreviation"), ["kor"]);
    assert!(run.err.contains("skipped a frame"), "{}", run.err);
}

#[tokio::test]
async fn the_timeout_ends_a_quiet_feed() {
    let cli = Cli::try_parse_from(["sports", "--timeout", "100ms"]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    run_with(cli.args, stream::pending::<Item>(), &mut out, &mut err)
        .await
        .unwrap();
    assert!(out.is_empty());
    assert!(String::from_utf8(err).unwrap().contains("Timeout reached"));
}

#[tokio::test]
async fn pretty_output_shows_teams_score_and_state() {
    let run = run_argv(
        &["sports"],
        vec![update(fixtures::SOCCER), update(fixtures::CRICKET_FINISHED)],
    )
    .await;
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 2, "{}", run.out);
    for expected in [
        "kor",
        "90106111",
        "Gimcheon Sangmu FC v Daejeon Hana Citizen FC",
        "2-1",
        "2H",
        "live",
    ] {
        assert!(
            lines[0].contains(expected),
            "{expected} missing from {:?}",
            lines[0]
        );
    }
    for expected in ["cricket", "id2703438269077680", "116-38", "FT", "ended"] {
        assert!(
            lines[1].contains(expected),
            "{expected} missing from {:?}",
            lines[1]
        );
    }
}
