//! `polyoxide binance usdm stream` over scripted event streams built from the
//! captured frames, so a flag that parses but never reaches the output fails
//! here.

use std::{io, time::Duration};

use clap::Parser;
use futures_util::{stream, Stream, StreamExt};
use polyoxide_binance::usdm::ws::{
    fixtures, DisconnectReason, Event, StreamPath, Update, UsdmWsError,
};
use polyoxide_cli::commands::binance::usdm::{run_with, BinanceArgs};
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

/// How long a run may take before a test calls it hung. A feed that goes quiet
/// never ends on its own, so a regression must fail here instead of blocking.
const HANG: Duration = Duration::from_secs(2);

/// Run over any stream, with `--all-mark-prices` in front of `argv`.
async fn run_stream<S>(argv: &[&str], events: S) -> Run
where
    S: Stream<Item = Item> + Unpin,
{
    let mut full = vec!["binance", "--all-mark-prices"];
    full.extend_from_slice(argv);
    let cli = Cli::try_parse_from(full).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    tokio::time::timeout(HANG, run_with(cli.args, events, &mut out, &mut err))
        .await
        .expect("the run ended")
        .unwrap();
    Run {
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

async fn run(argv: &[&str], events: Vec<Item>) -> Run {
    run_stream(argv, stream::iter(events)).await
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
    // The feed stays open, so only the broken pipe can end this run.
    let events = stream::iter(vec![update(fixtures::MARK_PRICE)]).chain(stream::pending());
    let mut err = Vec::new();
    tokio::time::timeout(HANG, run_with(cli.args, events, &mut Closed, &mut err))
        .await
        .expect("the run ended when the reader went away")
        .expect("a closed reader is not an error");
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
}

#[tokio::test]
async fn the_timeout_ends_a_quiet_feed() {
    // One update, then silence: only the deadline can end this run.
    let events = stream::iter(vec![update(fixtures::MARK_PRICE)]).chain(stream::pending());
    let run = run_stream(&["--format", "json", "-t", "100ms"], events).await;
    assert_eq!(run.out.lines().count(), 1, "{}", run.out);
    assert!(run.err.contains("Timeout reached"), "{}", run.err);
}

#[tokio::test]
async fn a_timeout_too_large_to_add_means_no_deadline() {
    // Adding this to `Instant::now()` overflows; it must mean "no deadline",
    // not a panic.
    let events = stream::iter(vec![update(fixtures::MARK_PRICE)]);
    let run = run_stream(&["--format", "json", "-t", "18446744073709551615s"], events).await;
    assert_eq!(run.out.lines().count(), 1, "{}", run.out);
}

#[tokio::test]
async fn count_stops_without_waiting_for_another_frame() {
    // A live feed may go quiet after the last wanted update. Reaching `-n`
    // must end the run then, not on the next frame.
    let events = stream::iter(vec![update(fixtures::MARK_PRICE)]).chain(stream::pending());
    let run = run_stream(&["--format", "json", "-n", "1"], events).await;
    assert_eq!(run.out.lines().count(), 1, "{}", run.out);
    assert!(run.err.contains("Reached 1 update(s)"), "{}", run.err);
}

#[tokio::test]
async fn a_server_close_marker_names_its_code() {
    // The nightly classifier reads `closed by the server (1011 ...` in this
    // marker as a restart, so the text is pinned here.
    let run = run(
        &[],
        vec![Ok(Event::Disconnected {
            path: StreamPath::Market,
            reason: DisconnectReason::Closed {
                code: Some(1011),
                reason: "Internal error".into(),
            },
        })],
    )
    .await;
    assert!(
        run.err.contains(
            "# market disconnected: closed by the server (1011 Internal error). \
             Its streams are stale until it reconnects."
        ),
        "{}",
        run.err
    );
}

fn parse(argv: &[&str]) -> BinanceArgs {
    Cli::try_parse_from(argv).unwrap().args
}

#[test]
fn a_symbol_needs_a_kind_even_with_all_tickers() {
    // `--all-tickers` makes the stream list non-empty, so only the pairing
    // check can notice that the symbol would be silently dropped.
    let err = parse(&["binance", "--symbol", "BTCUSDT", "--all-tickers"])
        .streams()
        .unwrap_err()
        .to_string();
    assert!(err.contains("go together"), "{err}");
    let err = parse(&["binance", "--kind", "ticker", "--all-mark-prices"])
        .streams()
        .unwrap_err()
        .to_string();
    assert!(err.contains("go together"), "{err}");
}

#[test]
fn an_empty_list_entry_is_ignored() {
    let streams = parse(&[
        "binance",
        "--symbol",
        "BTCUSDT,,ETHUSDT",
        "--kind",
        "ticker",
    ])
    .streams()
    .unwrap();
    assert_eq!(streams.len(), 2, "{streams:?}");
    let streams = parse(&[
        "binance",
        "--symbol",
        "BTCUSDT",
        "--kind",
        "ticker,,agg-trade",
    ])
    .streams()
    .unwrap();
    assert_eq!(streams.len(), 2, "{streams:?}");
}

/// The first value of `pointer` in `value`, which must be a string.
fn str_at<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{pointer} is not a string in {value}"))
}

/// `run` appears in `line` as consecutive whitespace-separated words.
#[track_caller]
fn assert_run(line: &str, run: &[&str]) {
    let words: Vec<&str> = line.split_whitespace().collect();
    assert!(
        words.windows(run.len()).any(|window| window == run),
        "{run:?} missing from {line:?}"
    );
}

/// The pretty lines one frame prints.
async fn pretty(frame: &str) -> Vec<String> {
    run(&[], vec![update(frame)])
        .await
        .out
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A 24-hour ticker row, single or from the array stream.
#[track_caller]
fn assert_ticker(line: &str, row: &Value) {
    assert_run(
        line,
        &[
            str_at(row, "/s"),
            "ticker",
            "last",
            str_at(row, "/c"),
            "open",
            str_at(row, "/o"),
            "quote",
            "volume",
            str_at(row, "/q"),
        ],
    );
}

/// An aggregate trade. `m: true` means the buyer was the maker, so the taker
/// sold.
#[track_caller]
fn assert_trade(line: &str, trade: &Value) {
    let side = if trade["m"].as_bool().unwrap() {
        "sell"
    } else {
        "buy"
    };
    assert_run(
        line,
        &[
            str_at(trade, "/s"),
            "trade",
            str_at(trade, "/q"),
            "@",
            str_at(trade, "/p"),
            side,
        ],
    );
}

/// A mark-price row, single or from the array stream.
#[track_caller]
fn assert_mark(line: &str, row: &Value) {
    assert_run(
        line,
        &[
            str_at(row, "/s"),
            "mark",
            str_at(row, "/p"),
            "index",
            str_at(row, "/i"),
            "funding",
            str_at(row, "/r"),
        ],
    );
}

fn frame(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap()
}

#[tokio::test]
async fn pretty_prints_every_payload_kind() {
    // Every value is read from the fixture, so a re-capture does not break
    // this; the labels and the order of the fields are what is pinned.

    let wire = frame(fixtures::KLINE);
    let lines = pretty(fixtures::KLINE).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
    let k = &wire["data"]["k"];
    assert_run(
        &lines[0],
        &[
            str_at(&wire, "/data/s"),
            "kline",
            str_at(k, "/i"),
            "o",
            str_at(k, "/o"),
            "h",
            str_at(k, "/h"),
            "l",
            str_at(k, "/l"),
            "c",
            str_at(k, "/c"),
            "v",
            str_at(k, "/v"),
        ],
    );
    assert_eq!(
        lines[0].split_whitespace().last() == Some("closed"),
        k["x"].as_bool().unwrap(),
        "{}",
        lines[0]
    );

    let wire = frame(fixtures::TICKER);
    let lines = pretty(fixtures::TICKER).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_ticker(&lines[0], &wire["data"]);

    let wire = frame(fixtures::PARTIAL_DEPTH);
    let lines = pretty(fixtures::PARTIAL_DEPTH).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
    let book = &wire["data"];
    assert_run(
        &lines[0],
        &[
            str_at(book, "/s"),
            "depth",
            "bid",
            str_at(book, "/b/0/0"),
            "x",
            str_at(book, "/b/0/1"),
            "ask",
            str_at(book, "/a/0/0"),
            "x",
            str_at(book, "/a/0/1"),
        ],
    );
    let levels = book["b"].as_array().unwrap().len();
    assert!(
        lines[0].ends_with(&format!("({levels} levels)")),
        "{}",
        lines[0]
    );

    let wire = frame(fixtures::MARK_PRICE);
    let lines = pretty(fixtures::MARK_PRICE).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_mark(&lines[0], &wire["data"]);

    let wire = frame(fixtures::BOOK_TICKER);
    let lines = pretty(fixtures::BOOK_TICKER).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
    let book = &wire["data"];
    assert_run(
        &lines[0],
        &[
            str_at(book, "/s"),
            "book",
            str_at(book, "/b"),
            "x",
            str_at(book, "/B"),
            "/",
            str_at(book, "/a"),
            "x",
            str_at(book, "/A"),
        ],
    );

    let wire = frame(fixtures::AGG_TRADE);
    let lines = pretty(fixtures::AGG_TRADE).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_trade(&lines[0], &wire["data"]);

    // The array streams print a line per row, each as its single-symbol kind.
    let wire = frame(fixtures::ALL_TICKERS);
    let rows = wire["data"].as_array().unwrap();
    let lines = pretty(fixtures::ALL_TICKERS).await;
    assert_eq!(lines.len(), rows.len(), "{lines:?}");
    for (line, row) in lines.iter().zip(rows) {
        assert_ticker(line, row);
    }

    let wire = frame(fixtures::ALL_MARK_PRICES);
    let rows = wire["data"].as_array().unwrap();
    let lines = pretty(fixtures::ALL_MARK_PRICES).await;
    assert_eq!(lines.len(), rows.len(), "{lines:?}");
    for (line, row) in lines.iter().zip(rows) {
        assert_mark(line, row);
    }
}

#[tokio::test]
async fn a_trade_is_a_sell_when_the_buyer_was_the_maker() {
    // The fixture holds one side; the other is the same frame with `m` flipped.
    for buyer_is_maker in [false, true] {
        let mut wire = frame(fixtures::AGG_TRADE);
        wire["data"]["m"] = Value::Bool(buyer_is_maker);
        let lines = pretty(&wire.to_string()).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_trade(&lines[0], &wire["data"]);
        let sold = lines[0].split_whitespace().last() == Some("sell");
        assert_eq!(sold, buyer_is_maker, "{}", lines[0]);
    }
}

/// `polyoxide binance spot stream`, over the same scripted event streams built
/// from spot's captured frames.
mod spot_ws {
    use std::io;

    use clap::Parser;
    use futures_util::{stream, Stream, StreamExt};
    use polyoxide_binance::spot::ws::{
        fixtures, DisconnectReason, Event, SpotWsError, StreamPath, Update,
    };
    use polyoxide_cli::commands::binance::spot::{run_with, SpotArgs};
    use serde_json::Value;

    use super::{assert_run, frame, str_at, Run, HANG};

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: SpotArgs,
    }

    type Item = Result<Event, SpotWsError>;

    fn update(frame: &str) -> Item {
        Ok(Event::Update(Box::new(Update::from_json(frame).unwrap())))
    }

    /// Run over any stream, with `--all-mini-tickers` in front of `argv`.
    async fn run_stream<S>(argv: &[&str], events: S) -> Run
    where
        S: Stream<Item = Item> + Unpin,
    {
        let mut full = vec!["binance", "--all-mini-tickers"];
        full.extend_from_slice(argv);
        let cli = Cli::try_parse_from(full).unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        tokio::time::timeout(HANG, run_with(cli.args, events, &mut out, &mut err))
            .await
            .expect("the run ended")
            .unwrap();
        Run {
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    async fn run(argv: &[&str], events: Vec<Item>) -> Run {
        run_stream(argv, stream::iter(events)).await
    }

    /// The pretty lines one frame prints.
    async fn pretty(frame: &str) -> Vec<String> {
        run(&[], vec![update(frame)])
            .await
            .out
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn parse(argv: &[&str]) -> SpotArgs {
        Cli::try_parse_from(argv).unwrap().args
    }

    /// The wire frame as the client prints it: Binance documents `M` on trades
    /// and aggregate trades and `B` on the kline as "Ignore", and the client
    /// does not model them.
    fn modelled(raw: &str) -> Value {
        let mut wire = frame(raw);
        if let Some(data) = wire.get_mut("data").and_then(Value::as_object_mut) {
            data.remove("M");
        }
        if let Some(k) = wire.pointer_mut("/data/k").and_then(Value::as_object_mut) {
            k.remove("B");
        }
        wire
    }

    #[tokio::test]
    async fn json_lines_are_the_frames_envelopes() {
        // The eleven captured frames, so a loop over a shorter list cannot pass.
        assert_eq!(fixtures::ALL.len(), 11);
        let events: Vec<Item> = fixtures::ALL
            .iter()
            .map(|(_, frame)| update(frame))
            .collect();
        let run = run(&["--format", "json"], events).await;
        let lines: Vec<&str> = run.out.lines().collect();
        assert_eq!(lines.len(), fixtures::ALL.len());
        for (line, (name, frame)) in lines.iter().zip(fixtures::ALL) {
            let printed: Value = serde_json::from_str(line).unwrap();
            assert_eq!(printed, modelled(frame), "{name}");
        }
        assert!(run.err.contains("The feed ended"), "{}", run.err);
    }

    #[tokio::test]
    async fn count_stops_after_n_updates_and_markers_go_to_stderr() {
        let events = vec![
            update(fixtures::MINI_TICKER),
            Ok(Event::Disconnected {
                path: StreamPath::Stream,
                reason: DisconnectReason::Stale,
            }),
            Ok(Event::Reconnected {
                path: StreamPath::Stream,
            }),
            update(fixtures::MINI_TICKER),
            update(fixtures::MINI_TICKER),
        ];
        let run = run(&["--format", "json", "-n", "2"], events).await;
        assert_eq!(run.out.lines().count(), 2);
        assert!(
            !run.out.contains('#'),
            "markers leaked into stdout: {}",
            run.out
        );
        assert!(run.err.contains("# stream disconnected"), "{}", run.err);
        assert!(run.err.contains("# stream reconnected"), "{}", run.err);
        assert!(run.err.contains("Reached 2 update(s)"), "{}", run.err);
    }

    #[tokio::test]
    async fn pretty_prints_a_line_per_update_and_per_array_row() {
        let run = run(
            &[],
            vec![
                update(fixtures::ALL_MINI_TICKERS),
                update(fixtures::AGG_TRADE),
                update(fixtures::BOOK_TICKER),
            ],
        )
        .await;
        let lines: Vec<&str> = run.out.lines().collect();
        // The array fixture holds two rows.
        assert_eq!(lines.len(), 4, "{}", run.out);
        assert!(
            lines[0].contains("mini") && lines[1].contains("mini"),
            "{}",
            run.out
        );
        assert!(
            lines[2].starts_with("BTCUSDT") && lines[2].contains("aggtrade"),
            "{}",
            run.out
        );
        assert!(lines[3].contains("book"), "{}", run.out);
    }

    #[tokio::test]
    async fn a_bad_frame_is_skipped_on_stderr_and_a_fatal_error_ends_the_run() {
        let bad = Update::from_json(r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade"}}"#)
            .unwrap_err();
        let events = vec![Err(bad), update(fixtures::MINI_TICKER)];
        let run = run(&["--format", "json"], events).await;
        assert_eq!(run.out.lines().count(), 1);
        assert!(
            run.err
                .contains("# skipped a frame on \"btcusdt@aggTrade\""),
            "{}",
            run.err
        );

        let cli = Cli::try_parse_from(["binance", "--all-mini-tickers"]).unwrap();
        let events: Vec<Item> = vec![Err(SpotWsError::Refused {
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
        let cli = Cli::try_parse_from(["binance", "--all-mini-tickers"]).unwrap();
        // The feed stays open, so only the broken pipe can end this run.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]).chain(stream::pending());
        let mut err = Vec::new();
        tokio::time::timeout(HANG, run_with(cli.args, events, &mut Closed, &mut err))
            .await
            .expect("the run ended when the reader went away")
            .expect("a closed reader is not an error");
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    }

    #[tokio::test]
    async fn the_timeout_ends_a_quiet_feed() {
        // One update, then silence: only the deadline can end this run.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]).chain(stream::pending());
        let run = run_stream(&["--format", "json", "-t", "100ms"], events).await;
        assert_eq!(run.out.lines().count(), 1, "{}", run.out);
        assert!(run.err.contains("Timeout reached"), "{}", run.err);
    }

    #[tokio::test]
    async fn a_timeout_too_large_to_add_means_no_deadline() {
        // Adding this to `Instant::now()` overflows; it must mean "no
        // deadline", not a panic.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]);
        let run = run_stream(&["--format", "json", "-t", "18446744073709551615s"], events).await;
        assert_eq!(run.out.lines().count(), 1, "{}", run.out);
    }

    #[tokio::test]
    async fn count_stops_without_waiting_for_another_frame() {
        // A live feed may go quiet after the last wanted update. Reaching `-n`
        // must end the run then, not on the next frame.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]).chain(stream::pending());
        let run = run_stream(&["--format", "json", "-n", "1"], events).await;
        assert_eq!(run.out.lines().count(), 1, "{}", run.out);
        assert!(run.err.contains("Reached 1 update(s)"), "{}", run.err);
    }

    #[tokio::test]
    async fn a_server_close_marker_names_its_code() {
        // The nightly classifier reads `closed by the server (1011 ...` in this
        // marker as a restart, so the text is pinned here.
        let run = run(
            &[],
            vec![Ok(Event::Disconnected {
                path: StreamPath::Stream,
                reason: DisconnectReason::Closed {
                    code: Some(1011),
                    reason: "Internal error".into(),
                },
            })],
        )
        .await;
        assert!(
            run.err.contains(
                "# stream disconnected: closed by the server (1011 Internal error). \
                 Its streams are stale until it reconnects."
            ),
            "{}",
            run.err
        );
    }

    #[test]
    fn a_symbol_needs_a_kind_even_with_all_mini_tickers() {
        // `--all-mini-tickers` makes the stream list non-empty, so only the
        // pairing check can notice that the symbol would be silently dropped.
        let err = parse(&["binance", "--symbol", "BTCUSDT", "--all-mini-tickers"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("go together"), "{err}");
        let err = parse(&["binance", "--kind", "ticker", "--all-mini-tickers"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("go together"), "{err}");
    }

    #[test]
    fn an_empty_list_entry_is_ignored() {
        let streams = parse(&[
            "binance",
            "--symbol",
            "BTCUSDT,,ETHUSDT",
            "--kind",
            "ticker",
        ])
        .streams()
        .unwrap();
        assert_eq!(streams.len(), 2, "{streams:?}");
        let streams = parse(&[
            "binance",
            "--symbol",
            "BTCUSDT",
            "--kind",
            "ticker,,agg-trade",
        ])
        .streams()
        .unwrap();
        assert_eq!(streams.len(), 2, "{streams:?}");
    }

    /// A trade line. `m: true` means the buyer was the maker, so the taker sold.
    #[track_caller]
    fn assert_trade(line: &str, label: &str, trade: &Value) {
        let side = if trade["m"].as_bool().unwrap() {
            "sell"
        } else {
            "buy"
        };
        assert_run(
            line,
            &[
                str_at(trade, "/s"),
                label,
                str_at(trade, "/q"),
                "@",
                str_at(trade, "/p"),
                side,
            ],
        );
    }

    /// A mini-ticker or 24-hour ticker row, single or from the array stream:
    /// `label` and the word before the close price differ, the rest is shared.
    #[track_caller]
    fn assert_summary(line: &str, label: &str, close: &str, row: &Value) {
        assert_run(
            line,
            &[
                str_at(row, "/s"),
                label,
                close,
                str_at(row, "/c"),
                "open",
                str_at(row, "/o"),
                "quote",
                "volume",
                str_at(row, "/q"),
            ],
        );
    }

    /// A partial-depth line. The frame carries no symbol, so it is the
    /// envelope's, uppercased.
    #[track_caller]
    fn assert_depth(line: &str, wire: &Value) {
        let symbol = str_at(wire, "/stream")
            .split('@')
            .next()
            .unwrap()
            .to_ascii_uppercase();
        let book = &wire["data"];
        assert_run(
            line,
            &[
                &symbol,
                "depth",
                "bid",
                str_at(book, "/bids/0/0"),
                "x",
                str_at(book, "/bids/0/1"),
                "ask",
                str_at(book, "/asks/0/0"),
                "x",
                str_at(book, "/asks/0/1"),
            ],
        );
        let levels = book["bids"]
            .as_array()
            .unwrap()
            .len()
            .max(book["asks"].as_array().unwrap().len());
        assert!(line.ends_with(&format!("({levels} levels)")), "{line}");
    }

    #[tokio::test]
    async fn pretty_prints_every_payload_kind() {
        // Every value is read from the fixture, so a re-capture does not break
        // this; the labels and the order of the fields are what is pinned.

        let wire = frame(fixtures::AGG_TRADE);
        let lines = pretty(fixtures::AGG_TRADE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_trade(&lines[0], "aggtrade", &wire["data"]);

        let wire = frame(fixtures::TRADE);
        let lines = pretty(fixtures::TRADE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_trade(&lines[0], "trade", &wire["data"]);

        let wire = frame(fixtures::KLINE);
        let lines = pretty(fixtures::KLINE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let k = &wire["data"]["k"];
        assert_run(
            &lines[0],
            &[
                str_at(&wire, "/data/s"),
                "kline",
                str_at(k, "/i"),
                "o",
                str_at(k, "/o"),
                "h",
                str_at(k, "/h"),
                "l",
                str_at(k, "/l"),
                "c",
                str_at(k, "/c"),
                "v",
                str_at(k, "/v"),
            ],
        );
        assert_eq!(
            lines[0].split_whitespace().last() == Some("closed"),
            k["x"].as_bool().unwrap(),
            "{}",
            lines[0]
        );

        let wire = frame(fixtures::MINI_TICKER);
        let lines = pretty(fixtures::MINI_TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_summary(&lines[0], "mini", "close", &wire["data"]);

        let wire = frame(fixtures::TICKER);
        let lines = pretty(fixtures::TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_summary(&lines[0], "ticker", "last", &wire["data"]);

        let wire = frame(fixtures::BOOK_TICKER);
        let lines = pretty(fixtures::BOOK_TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let book = &wire["data"];
        assert_run(
            &lines[0],
            &[
                str_at(book, "/s"),
                "book",
                str_at(book, "/b"),
                "x",
                str_at(book, "/B"),
                "/",
                str_at(book, "/a"),
                "x",
                str_at(book, "/A"),
            ],
        );

        // Both spellings of a partial depth: the bare (1000 ms) one and `@100ms`.
        for raw in [fixtures::PARTIAL_DEPTH, fixtures::PARTIAL_DEPTH_100MS] {
            let wire = frame(raw);
            let lines = pretty(raw).await;
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert_depth(&lines[0], &wire);
        }

        let wire = frame(fixtures::AVG_PRICE);
        let lines = pretty(fixtures::AVG_PRICE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let avg = &wire["data"];
        assert_run(
            &lines[0],
            &[
                str_at(avg, "/s"),
                "avg",
                str_at(avg, "/w"),
                "over",
                str_at(avg, "/i"),
            ],
        );

        // A symbol with non-ASCII characters prints as the frame sent it.
        let wire = frame(fixtures::NON_ASCII_MINI_TICKER);
        let lines = pretty(fixtures::NON_ASCII_MINI_TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(!str_at(&wire, "/data/s").is_ascii(), "{wire}");
        assert_summary(&lines[0], "mini", "close", &wire["data"]);

        // The array stream prints a line per row, each as its single-symbol kind.
        let wire = frame(fixtures::ALL_MINI_TICKERS);
        let rows = wire["data"].as_array().unwrap();
        let lines = pretty(fixtures::ALL_MINI_TICKERS).await;
        assert!(rows.len() >= 2, "{wire}");
        assert_eq!(lines.len(), rows.len(), "{lines:?}");
        for (line, row) in lines.iter().zip(rows) {
            assert_summary(line, "mini", "close", row);
        }
    }

    #[tokio::test]
    async fn a_trade_is_a_sell_when_the_buyer_was_the_maker() {
        // The fixtures hold one side; the other is the same frame with `m`
        // flipped.
        for (raw, label) in [
            (fixtures::AGG_TRADE, "aggtrade"),
            (fixtures::TRADE, "trade"),
        ] {
            for buyer_is_maker in [false, true] {
                let mut wire = frame(raw);
                wire["data"]["m"] = Value::Bool(buyer_is_maker);
                let lines = pretty(&wire.to_string()).await;
                assert_eq!(lines.len(), 1, "{lines:?}");
                assert_trade(&lines[0], label, &wire["data"]);
                let sold = lines[0].split_whitespace().last() == Some("sell");
                assert_eq!(sold, buyer_is_maker, "{}", lines[0]);
            }
        }
    }

    #[tokio::test]
    async fn an_unknown_event_prints_its_type() {
        // A single-symbol object whose `e` is not its stream's is
        // `Payload::Unknown`, and prints as such rather than failing the run.
        let lines =
            pretty(r#"{"stream":"btcusdt@trade","data":{"e":"surprise","s":"BTCUSDT"}}"#).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("btcusdt@trade"), "{}", lines[0]);
        assert!(
            lines[0].contains("unknown event \"surprise\""),
            "{}",
            lines[0]
        );
    }

    #[tokio::test]
    async fn a_closed_kline_ends_its_line_with_closed() {
        // The fixture's bar is still open; the same frame with `x` set is the
        // bar's final update.
        let mut wire = frame(fixtures::KLINE);
        assert_eq!(wire["data"]["k"]["x"], Value::Bool(false), "{wire}");
        let open = pretty(&wire.to_string()).await;
        assert_eq!(open.len(), 1, "{open:?}");
        assert!(!open[0].ends_with("closed"), "{}", open[0]);

        wire["data"]["k"]["x"] = Value::Bool(true);
        let lines = pretty(&wire.to_string()).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].ends_with(" closed"), "{}", lines[0]);
        assert_eq!(
            lines[0].trim_end_matches(" closed"),
            open[0],
            "closing the bar changes only the suffix"
        );
    }

    #[tokio::test]
    async fn a_depth_line_counts_the_larger_side() {
        // Bids stay at the fixture's count and the asks are cut to one row, so
        // the larger side is the bids and the smaller is not what is counted.
        for raw in [fixtures::PARTIAL_DEPTH, fixtures::PARTIAL_DEPTH_100MS] {
            let mut wire = frame(raw);
            let bids = wire["data"]["bids"].as_array().unwrap().len();
            assert!(bids > 1, "{wire}");
            wire["data"]["asks"].as_array_mut().unwrap().truncate(1);
            let lines = pretty(&wire.to_string()).await;
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert_depth(&lines[0], &wire);
            assert!(
                lines[0].ends_with(&format!("({bids} levels)")),
                "{}",
                lines[0]
            );

            // And the other way round: bids cut to one, asks at their count.
            let mut wire = frame(raw);
            let asks = wire["data"]["asks"].as_array().unwrap().len();
            assert!(asks > 1, "{wire}");
            wire["data"]["bids"].as_array_mut().unwrap().truncate(1);
            let lines = pretty(&wire.to_string()).await;
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert!(
                lines[0].ends_with(&format!("({asks} levels)")),
                "{}",
                lines[0]
            );
        }
    }

    #[tokio::test]
    async fn an_empty_depth_side_prints_a_dash() {
        let wire = frame(fixtures::PARTIAL_DEPTH);
        let bids = wire["data"]["bids"].as_array().unwrap().len();
        let asks = wire["data"]["asks"].as_array().unwrap().len();
        let symbol = str_at(&wire, "/stream")
            .split('@')
            .next()
            .unwrap()
            .to_ascii_uppercase();

        let mut empty_asks = wire.clone();
        empty_asks["data"]["asks"] = Value::Array(Vec::new());
        let lines = pretty(&empty_asks.to_string()).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let best_bid = format!(
            "{} x {}",
            str_at(&wire, "/data/bids/0/0"),
            str_at(&wire, "/data/bids/0/1")
        );
        assert_eq!(
            lines[0],
            format!("{symbol:<16} depth   bid {best_bid} ask - ({bids} levels)")
        );

        let mut empty_bids = wire.clone();
        empty_bids["data"]["bids"] = Value::Array(Vec::new());
        let lines = pretty(&empty_bids.to_string()).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let best_ask = format!(
            "{} x {}",
            str_at(&wire, "/data/asks/0/0"),
            str_at(&wire, "/data/asks/0/1")
        );
        assert_eq!(
            lines[0],
            format!("{symbol:<16} depth   bid - ask {best_ask} ({asks} levels)")
        );
    }
}
