//! `polyoxide ws binance` over scripted event streams built from the captured
//! frames, so a flag that parses but never reaches the output fails here.

use std::io;

use clap::Parser;
use futures_util::stream;
use polyoxide_binance::usdm::ws::{
    fixtures, DisconnectReason, Event, StreamPath, Update, UsdmWsError,
};
use polyoxide_cli::commands::ws::binance::{run_with, BinanceArgs};
use serde_json::Value;

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    args: BinanceArgs,
}

type Item = Result<Event, UsdmWsError>;

fn update(frame: &str) -> Item {
    Ok(Event::Update(Box::new(Update::from_json(frame).unwrap())))
}

struct Run {
    out: String,
    err: String,
}

async fn run(argv: &[&str], events: Vec<Item>) -> Run {
    let mut full = vec!["binance", "--all-mark-prices"];
    full.extend_from_slice(argv);
    let cli = Cli::try_parse_from(full).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    run_with(cli.args, stream::iter(events), &mut out, &mut err)
        .await
        .unwrap();
    Run {
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

#[tokio::test]
async fn json_lines_are_the_frames_envelopes() {
    let events: Vec<Item> = fixtures::ALL
        .iter()
        .map(|(_, frame)| update(frame))
        .collect();
    let run = run(&["--format", "json"], events).await;
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), fixtures::ALL.len());
    for (line, (name, frame)) in lines.iter().zip(fixtures::ALL) {
        let printed: Value = serde_json::from_str(line).unwrap();
        let mut wire: Value = serde_json::from_str(frame).unwrap();
        // The kline's `B`, documented as "Ignore", is not modelled.
        if let Some(k) = wire.pointer_mut("/data/k").and_then(Value::as_object_mut) {
            k.remove("B");
        }
        assert_eq!(printed, wire, "{name}");
    }
    assert!(run.err.contains("The feed ended"), "{}", run.err);
}

#[tokio::test]
async fn count_stops_after_n_updates_and_markers_go_to_stderr() {
    let events = vec![
        update(fixtures::MARK_PRICE),
        Ok(Event::Disconnected {
            path: StreamPath::Market,
            reason: DisconnectReason::Stale,
        }),
        Ok(Event::Reconnected {
            path: StreamPath::Market,
        }),
        update(fixtures::MARK_PRICE),
        update(fixtures::MARK_PRICE),
    ];
    let run = run(&["--format", "json", "-n", "2"], events).await;
    assert_eq!(run.out.lines().count(), 2);
    assert!(
        !run.out.contains('#'),
        "markers leaked into stdout: {}",
        run.out
    );
    assert!(run.err.contains("# market disconnected"), "{}", run.err);
    assert!(run.err.contains("# market reconnected"), "{}", run.err);
    assert!(run.err.contains("Reached 2 update(s)"), "{}", run.err);
}

#[tokio::test]
async fn pretty_prints_a_line_per_update_and_per_array_row() {
    let run = run(
        &[],
        vec![
            update(fixtures::ALL_MARK_PRICES),
            update(fixtures::AGG_TRADE),
            update(fixtures::BOOK_TICKER),
        ],
    )
    .await;
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 4, "{}", run.out);
    assert!(
        lines[0].contains("mark") && lines[1].contains("mark"),
        "{}",
        run.out
    );
    assert!(
        lines[2].starts_with("BTCUSDT") && lines[2].contains("trade"),
        "{}",
        run.out
    );
    assert!(lines[3].contains("book"), "{}", run.out);
}

#[tokio::test]
async fn a_bad_frame_is_skipped_on_stderr_and_a_fatal_error_ends_the_run() {
    let bad =
        Update::from_json(r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade"}}"#).unwrap_err();
    let events = vec![Err(bad), update(fixtures::MARK_PRICE)];
    let run = run(&["--format", "json"], events).await;
    assert_eq!(run.out.lines().count(), 1);
    assert!(
        run.err
            .contains("# skipped a frame on \"btcusdt@aggTrade\""),
        "{}",
        run.err
    );

    let cli = Cli::try_parse_from(["binance", "--all-mark-prices"]).unwrap();
    let events: Vec<Item> = vec![Err(UsdmWsError::Refused {
        code: 2,
        msg: "Invalid request".into(),
    })];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let result = run_with(cli.args, stream::iter(events), &mut out, &mut err).await;
    assert!(result.unwrap_err().to_string().contains("Invalid request"));
}

#[tokio::test]
async fn a_closed_reader_ends_the_run_quietly() {
    struct Closed;
    impl io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let cli = Cli::try_parse_from(["binance", "--all-mark-prices"]).unwrap();
    let mut err = Vec::new();
    run_with(
        cli.args,
        stream::iter(vec![update(fixtures::MARK_PRICE)]),
        &mut Closed,
        &mut err,
    )
    .await
    .unwrap();
}
