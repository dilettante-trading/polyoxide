//! `polyoxide binance spot stream`: stream Binance spot market data.

use std::{
    io::{self, Write},
    time::Duration,
};

use clap::{Args, Subcommand};
use color_eyre::eyre::{bail, Result};
use futures_util::Stream;
use polyoxide_binance::spot::{
    types::{Interval, Level, Symbol},
    ws::{
        AggTradeEvent, AvgPriceEvent, BookTickerEvent, DepthLevels, DepthSpeed, Event, KlineEvent,
        MiniTickerEvent, PartialDepthEvent, Payload, SpotWsBuilder, SpotWsError, StreamName,
        TickerEvent, TradeEvent, Update,
    },
};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};
use crate::commands::stream::{self, excerpt, Feed, Handled, Limits, OutputFormat};

#[derive(Subcommand)]
pub enum SpotCommand {
    /// Stream spot market data: trades, klines, tickers, depth, book tickers
    /// and average prices
    Stream {
        #[command(flatten)]
        args: SpotArgs,
    },
}

impl SpotCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Stream { args } => run(args).await,
        }
    }
}

/// The kinds a symbol can be streamed as.
const KINDS: &str = "agg-trade, avg-price, book-ticker, depth5, depth10, depth20, \
                     kline-<interval>, mini-ticker, ticker, trade";

#[derive(Args, Debug)]
pub struct SpotArgs {
    /// Symbols, comma-separated, e.g. BTCUSDT,ETHUSDT. Each `--kind` is
    /// streamed for each symbol. Letters are matched case-blind.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub symbol: Vec<String>,

    /// Kinds to stream for each symbol, comma-separated: agg-trade,
    /// avg-price, book-ticker, depth5, depth10 and depth20 (each every
    /// 100 ms), `kline-<interval>` (kline-1s, kline-1m, kline-1h, kline-1d,
    /// ...; `1M` is a month, `1m` a minute), mini-ticker, ticker, trade.
    /// Kinds are case-sensitive.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub kind: Vec<String>,

    /// Stream the mini ticker of every symbol that changed
    /// (`!miniTicker@arr`).
    #[arg(long)]
    pub all_mini_tickers: bool,

    /// Output format: `pretty` prints one line per update, and one per row of
    /// an array stream; `json` prints each frame's `{"stream", "data"}`
    /// envelope
    #[arg(short, long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,

    /// Exit after printing N updates
    #[arg(short = 'n', long)]
    pub count: Option<u64>,

    /// Exit after the given duration (e.g. "30s", "5m")
    #[arg(short, long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
}

impl SpotArgs {
    /// The streams the arguments name.
    pub fn streams(&self) -> Result<Vec<StreamName>> {
        let symbols = self
            .symbol
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| Symbol::new(s.as_str()).map_err(|e| color_eyre::eyre::eyre!("{e}")))
            .collect::<Result<Vec<_>>>()?;
        let kinds: Vec<&str> = self
            .kind
            .iter()
            .map(String::as_str)
            .filter(|k| !k.is_empty())
            .collect();
        if symbols.is_empty() != kinds.is_empty() {
            bail!("--symbol and --kind go together: each kind is streamed for each symbol");
        }
        let mut streams = Vec::new();
        if self.all_mini_tickers {
            streams.push(StreamName::AllMiniTickers);
        }
        for symbol in &symbols {
            for kind in &kinds {
                streams.push(stream(symbol.clone(), kind)?);
            }
        }
        if streams.is_empty() {
            bail!("nothing to stream: pass --symbol with --kind, or --all-mini-tickers");
        }
        Ok(streams)
    }
}

/// One `--kind` for one symbol.
fn stream(symbol: Symbol, kind: &str) -> Result<StreamName> {
    Ok(match kind {
        "agg-trade" => StreamName::AggTrade(symbol),
        "avg-price" => StreamName::AvgPrice(symbol),
        "book-ticker" => StreamName::BookTicker(symbol),
        "depth5" => StreamName::PartialDepth(symbol, DepthLevels::Five, DepthSpeed::Ms100),
        "depth10" => StreamName::PartialDepth(symbol, DepthLevels::Ten, DepthSpeed::Ms100),
        "depth20" => StreamName::PartialDepth(symbol, DepthLevels::Twenty, DepthSpeed::Ms100),
        "mini-ticker" => StreamName::MiniTicker(symbol),
        "ticker" => StreamName::Ticker(symbol),
        "trade" => StreamName::Trade(symbol),
        _ => match kind
            .strip_prefix("kline-")
            .and_then(|i| i.parse::<Interval>().ok())
        {
            Some(interval) => StreamName::Kline(symbol, interval),
            // In a block, as a statement: Rust 1.99 rejects `bail!`, which
            // expands to `return Err(..);`, as a match arm's value.
            None => {
                bail!(
                    "unknown kind {kind:?}; expected one of {KINDS}, where <interval> is one of {}",
                    Interval::ALL
                        .iter()
                        .map(|i| i.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
        },
    })
}

/// Connect to the production host and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(args: SpotArgs) -> Result<()> {
    let streams = args.streams()?;
    let names: Vec<String> = streams.iter().map(ToString::to_string).collect();
    let listed = if names.len() <= 10 {
        names.join(", ")
    } else {
        format!("{} streams", names.len())
    };
    eprintln!("Connecting to Binance spot streams: {listed}");
    let mut feed = SpotWsBuilder::new().streams(streams).connect().await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
    // As in `binance usdm stream`, there is no Ctrl+C handler: the process
    // ends with the signal. On `-n`, `-t` or the end of the feed the
    // connection is closed with a handshake.
    let result = run_with(
        args,
        &mut feed,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await;
    let _ = feed.close().await;
    result
}

/// Print events from any stream until `-n` or `-t` is reached or the stream
/// ends.
///
/// Takes the stream rather than connecting, so tests can drive every flag with
/// a scripted list of events. Updates go to `out`; outage markers and skipped
/// frames go to `err`, so JSON output stays clean JSONL.
pub async fn run_with<S>(
    args: SpotArgs,
    events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<Event, SpotWsError>> + Unpin,
{
    let limits = Limits {
        count: args.count,
        timeout: args.timeout,
    };
    stream::run(limits, events, &mut Spot(args.format), out, err).await
}

/// The streams' half of a run: what each event means, printed in a format.
struct Spot(OutputFormat);

impl Feed for Spot {
    type Event = Event;
    type Error = SpotWsError;

    fn event(&mut self, event: Event, out: &mut dyn Write, err: &mut dyn Write) -> Result<Handled> {
        match event {
            Event::Update(update) => {
                print_update(&update, self.0, out)?;
                return Ok(Handled::Printed);
            }
            Event::Disconnected { path, reason } => writeln!(
                err,
                "# {path} disconnected: {reason}. Its streams are stale until it reconnects."
            )?,
            Event::Reconnected { path } => writeln!(
                err,
                "# {path} reconnected. Rebuild anything built from its streams."
            )?,
            // `Event` is #[non_exhaustive]; a future variant is not a fault.
            _ => {}
        }
        Ok(Handled::Skipped)
    }

    fn error(&mut self, error: SpotWsError, err: &mut dyn Write) -> Result<()> {
        match error {
            SpotWsError::Frame {
                stream,
                raw,
                reason,
            } => {
                writeln!(
                    err,
                    "# skipped a frame on {stream:?} that did not decode ({reason}): {}",
                    excerpt(&raw)
                )?;
                Ok(())
            }
            error => Err(error.into()),
        }
    }
}

/// Print one update and flush it.
fn print_update(update: &Update, format: OutputFormat, out: &mut dyn Write) -> io::Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(update)?)?,
        OutputFormat::Pretty => match &update.payload {
            Payload::MiniTickers(rows) => {
                for row in rows {
                    writeln!(out, "{}", mini_ticker(row))?;
                }
            }
            Payload::AggTrade(event) => writeln!(out, "{}", agg_trade(event))?,
            Payload::Trade(event) => writeln!(out, "{}", trade(event))?,
            Payload::Kline(event) => writeln!(out, "{}", kline(event))?,
            Payload::MiniTicker(event) => writeln!(out, "{}", mini_ticker(event))?,
            Payload::Ticker(event) => writeln!(out, "{}", ticker(event))?,
            Payload::BookTicker(event) => writeln!(out, "{}", book_ticker(event))?,
            Payload::PartialDepth(event) => writeln!(out, "{}", depth(event))?,
            Payload::AvgPrice(event) => writeln!(out, "{}", avg_price(event))?,
            Payload::Unknown { event_type, .. } => writeln!(
                out,
                "{:<16} unknown event {event_type:?}",
                update.stream.to_string()
            )?,
            _ => writeln!(out, "{}", update.stream)?,
        },
    }
    out.flush()
}

fn agg_trade(t: &AggTradeEvent) -> String {
    let side = if t.is_buyer_maker { "sell" } else { "buy" };
    format!(
        "{:<16} aggtrade {} @ {} {side}",
        t.symbol, t.quantity, t.price
    )
}

fn trade(t: &TradeEvent) -> String {
    let side = if t.is_buyer_maker { "sell" } else { "buy" };
    format!(
        "{:<16} trade   {} @ {} {side}",
        t.symbol, t.quantity, t.price
    )
}

fn kline(k: &KlineEvent) -> String {
    let bar = &k.kline;
    let closed = if bar.is_closed { " closed" } else { "" };
    format!(
        "{:<16} kline   {} o {} h {} l {} c {} v {}{closed}",
        k.symbol, bar.interval, bar.open, bar.high, bar.low, bar.close, bar.volume
    )
}

fn mini_ticker(t: &MiniTickerEvent) -> String {
    format!(
        "{:<16} mini    close {} open {} quote volume {}",
        t.symbol, t.close_price, t.open_price, t.quote_volume
    )
}

fn ticker(t: &TickerEvent) -> String {
    format!(
        "{:<16} ticker  last {} open {} quote volume {}",
        t.symbol, t.last_price, t.open_price, t.quote_volume
    )
}

fn book_ticker(b: &BookTickerEvent) -> String {
    format!(
        "{:<16} book    {} x {} / {} x {}",
        b.symbol, b.bid_price, b.bid_quantity, b.ask_price, b.ask_quantity
    )
}

fn depth(d: &PartialDepthEvent) -> String {
    let level = |side: &[Level]| match side.first() {
        Some(l) => format!("{} x {}", l.price, l.quantity),
        None => "-".to_owned(),
    };
    format!(
        "{:<16} depth   bid {} ask {} ({} levels)",
        d.symbol,
        level(&d.bids),
        level(&d.asks),
        d.bids.len().max(d.asks.len())
    )
}

fn avg_price(a: &AvgPriceEvent) -> String {
    format!("{:<16} avg     {} over {}", a.symbol, a.price, a.interval)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: SpotArgs,
    }

    fn parse(argv: &[&str]) -> SpotArgs {
        Wrapper::try_parse_from(argv).unwrap().args
    }

    fn names(args: &SpotArgs) -> Vec<String> {
        args.streams()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[derive(Parser)]
    struct Tree {
        #[command(subcommand)]
        command: SpotCommand,
    }

    #[test]
    fn stream_is_spots_one_verb() {
        let tree =
            Tree::try_parse_from(["test", "stream", "--all-mini-tickers", "-n", "1"]).unwrap();
        let SpotCommand::Stream { args } = tree.command;
        assert!(args.all_mini_tickers);
        assert_eq!(args.count, Some(1));
        assert!(Tree::try_parse_from(["test", "trades"]).is_err());
    }

    #[test]
    fn symbols_and_kinds_split_on_commas_and_multiply() {
        let args = parse(&[
            "test",
            "--symbol",
            "btcusdt, 币安人生USDT",
            "--kind",
            "agg-trade,kline-1h",
        ]);
        assert_eq!(
            names(&args),
            [
                "btcusdt@aggTrade",
                "btcusdt@kline_1h",
                "币安人生usdt@aggTrade",
                "币安人生usdt@kline_1h"
            ]
        );
    }

    #[test]
    fn every_kind_parses_and_depth_streams_at_100ms() {
        let args = parse(&[
            "test",
            "--symbol",
            "BTCUSDT",
            "--kind",
            "agg-trade,avg-price,book-ticker,depth5,depth10,depth20,kline-1s,kline-1M,\
             mini-ticker,ticker,trade",
            "--all-mini-tickers",
        ]);
        assert_eq!(
            names(&args),
            [
                "!miniTicker@arr",
                "btcusdt@aggTrade",
                "btcusdt@avgPrice",
                "btcusdt@bookTicker",
                "btcusdt@depth5@100ms",
                "btcusdt@depth10@100ms",
                "btcusdt@depth20@100ms",
                "btcusdt@kline_1s",
                "btcusdt@kline_1M",
                "btcusdt@miniTicker",
                "btcusdt@ticker",
                "btcusdt@trade",
            ]
        );
    }

    #[test]
    fn a_bad_kind_names_the_valid_ones() {
        let err = parse(&["test", "--symbol", "BTCUSDT", "--kind", "trades"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("\"trades\"") && err.contains("avg-price") && err.contains("1M"),
            "{err}"
        );
        // Spot's interval list is its own: `kline-2m` is no interval of it.
        let err = parse(&["test", "--symbol", "BTCUSDT", "--kind", "kline-2m"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("kline-2m"), "{err}");
        // The USDⓈ-M kinds spot has no stream for.
        for kind in ["mark-price", "funding"] {
            assert!(
                parse(&["test", "--symbol", "BTCUSDT", "--kind", kind])
                    .streams()
                    .is_err(),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_symbol_needs_a_kind_and_something_must_be_streamed() {
        assert!(parse(&["test", "--symbol", "BTCUSDT"]).streams().is_err());
        assert!(parse(&["test", "--kind", "ticker"]).streams().is_err());
        assert!(parse(&["test"])
            .streams()
            .unwrap_err()
            .to_string()
            .contains("nothing to stream"));
        assert!(parse(&["test", "--symbol", "BTC USDT", "--kind", "ticker"])
            .streams()
            .is_err());
    }

    #[test]
    fn count_timeout_and_format_parse() {
        let args = parse(&[
            "test",
            "--all-mini-tickers",
            "-n",
            "3",
            "-t",
            "5m",
            "--format",
            "json",
        ]);
        assert_eq!(args.count, Some(3));
        assert_eq!(args.timeout, Some(Duration::from_secs(300)));
        assert_eq!(args.format, OutputFormat::Json);
    }
}
