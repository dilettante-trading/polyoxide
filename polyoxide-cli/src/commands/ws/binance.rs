//! `polyoxide ws binance`: stream Binance USDⓈ-M futures market data.

use std::{
    io::{self, Write},
    time::Duration,
};

use clap::Args;
use color_eyre::eyre::{bail, Result};
use futures_util::{Stream, StreamExt};
use polyoxide_binance::usdm::{
    types::{Interval, Symbol},
    ws::{
        AggTradeEvent, BookTickerEvent, DepthLevels, DepthSpeed, Event, KlineEvent, MarkPriceEvent,
        PartialDepthEvent, Payload, Recovery, StreamName, TickerEvent, Update, UsdmWsBuilder,
        UsdmWsError,
    },
};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};

/// How each update is printed.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable: one line per update, and one per row of an array stream.
    #[default]
    Pretty,
    /// The frame's `{"stream", "data"}` envelope as compact JSON, one per line.
    Json,
}

/// The kinds a symbol can be streamed as.
const KINDS: &str = "agg-trade, book-ticker, depth5, depth10, depth20, kline-<interval>, \
                     mark-price, ticker";

#[derive(Args, Debug)]
pub struct BinanceArgs {
    /// Symbols, comma-separated, e.g. BTCUSDT,ETHUSDT. Each `--kind` is
    /// streamed for each symbol. Letters are matched case-blind.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub symbol: Vec<String>,

    /// Kinds to stream for each symbol, comma-separated: agg-trade,
    /// book-ticker, depth5, depth10 and depth20 (each every 100 ms),
    /// `kline-<interval>` (kline-1m, kline-1h, kline-1d, ...; `1M` is a month,
    /// `1m` a minute), mark-price, ticker. Kinds are case-sensitive.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub kind: Vec<String>,

    /// Stream the 24-hour ticker of every symbol that changed (`!ticker@arr`).
    #[arg(long)]
    pub all_tickers: bool,

    /// Stream every symbol's mark price and funding each second
    /// (`!markPrice@arr@1s`).
    #[arg(long)]
    pub all_mark_prices: bool,

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

impl BinanceArgs {
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
        if self.all_tickers {
            streams.push(StreamName::AllTickers);
        }
        if self.all_mark_prices {
            streams.push(StreamName::AllMarkPrices);
        }
        for symbol in &symbols {
            for kind in &kinds {
                streams.push(stream(symbol.clone(), kind)?);
            }
        }
        if streams.is_empty() {
            bail!(
                "nothing to stream: pass --symbol with --kind, --all-tickers or --all-mark-prices"
            );
        }
        Ok(streams)
    }
}

/// One `--kind` for one symbol.
fn stream(symbol: Symbol, kind: &str) -> Result<StreamName> {
    Ok(match kind {
        "agg-trade" => StreamName::AggTrade(symbol),
        "book-ticker" => StreamName::BookTicker(symbol),
        "depth5" => StreamName::PartialDepth(symbol, DepthLevels::Five, DepthSpeed::Ms100),
        "depth10" => StreamName::PartialDepth(symbol, DepthLevels::Ten, DepthSpeed::Ms100),
        "depth20" => StreamName::PartialDepth(symbol, DepthLevels::Twenty, DepthSpeed::Ms100),
        "mark-price" => StreamName::MarkPrice(symbol),
        "ticker" => StreamName::Ticker(symbol),
        _ => match kind
            .strip_prefix("kline-")
            .and_then(|i| i.parse::<Interval>().ok())
        {
            Some(interval) => StreamName::Kline(symbol, interval),
            None => bail!(
                "unknown kind {kind:?}; expected one of {KINDS}, where <interval> is one of {}",
                Interval::ALL
                    .iter()
                    .map(|i| i.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        },
    })
}

/// Connect to the production host and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(args: BinanceArgs) -> Result<()> {
    let streams = args.streams()?;
    let names: Vec<String> = streams.iter().map(ToString::to_string).collect();
    let listed = if names.len() <= 10 {
        names.join(", ")
    } else {
        format!("{} streams", names.len())
    };
    eprintln!("Connecting to Binance USDⓈ-M streams: {listed}");
    let mut feed = UsdmWsBuilder::new().streams(streams).connect().await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
    // As in `ws sports`, there is no Ctrl+C handler: the process ends with the
    // signal. On `-n`, `-t` or the end of the feed the connections are closed
    // with a handshake.
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
    args: BinanceArgs,
    mut events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<Event, UsdmWsError>> + Unpin,
{
    let deadline = args
        .timeout
        .and_then(|t| tokio::time::Instant::now().checked_add(t));
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
            Some(Ok(Event::Update(update))) => match print_update(&update, args.format, out) {
                Ok(()) => printed += 1,
                // The reader went away, as with `| head -1`; not an error.
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => break,
                Err(error) => return Err(error.into()),
            },
            Some(Ok(Event::Disconnected { path, reason })) => writeln!(
                err,
                "# {path} disconnected: {reason}. Its streams are stale until it reconnects."
            )?,
            Some(Ok(Event::Reconnected { path })) => writeln!(
                err,
                "# {path} reconnected. Rebuild anything built from its streams."
            )?,
            // `Event` is #[non_exhaustive]; a future variant is not a fault.
            Some(Ok(_)) => {}
            Some(Err(UsdmWsError::Frame {
                stream,
                raw,
                reason,
            })) => writeln!(
                err,
                "# skipped a frame on {stream:?} that did not decode ({reason}): {}",
                excerpt(&raw)
            )?,
            // Any other error the library says to skip; `UsdmWsError` is
            // #[non_exhaustive].
            Some(Err(error)) if error.recovery() == Recovery::SkipFrame => {
                writeln!(err, "# skipped a frame: {error}")?
            }
            Some(Err(error)) => return Err(error.into()),
            None => {
                writeln!(err, "The feed ended")?;
                break;
            }
        }
    }
    Ok(())
}

/// Print one update and flush it.
fn print_update(update: &Update, format: OutputFormat, out: &mut dyn Write) -> io::Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(update)?)?,
        OutputFormat::Pretty => match &update.payload {
            Payload::Tickers(rows) => {
                for row in rows {
                    writeln!(out, "{}", ticker(row))?;
                }
            }
            Payload::MarkPrices(rows) => {
                for row in rows {
                    writeln!(out, "{}", mark_price(row))?;
                }
            }
            Payload::AggTrade(event) => writeln!(out, "{}", agg_trade(event))?,
            Payload::Kline(event) => writeln!(out, "{}", kline(event))?,
            Payload::MarkPrice(event) => writeln!(out, "{}", mark_price(event))?,
            Payload::Ticker(event) => writeln!(out, "{}", ticker(event))?,
            Payload::PartialDepth(event) => writeln!(out, "{}", depth(event))?,
            Payload::BookTicker(event) => writeln!(out, "{}", book_ticker(event))?,
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

fn mark_price(m: &MarkPriceEvent) -> String {
    format!(
        "{:<16} mark    {} index {} funding {}",
        m.symbol, m.mark_price, m.index_price, m.funding_rate
    )
}

fn ticker(t: &TickerEvent) -> String {
    format!(
        "{:<16} ticker  last {} open {} quote volume {}",
        t.symbol, t.last_price, t.open_price, t.quote_volume
    )
}

fn depth(d: &PartialDepthEvent) -> String {
    let level = |side: &[polyoxide_binance::usdm::types::Level]| match side.first() {
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

fn book_ticker(b: &BookTickerEvent) -> String {
    format!(
        "{:<16} book    {} x {} / {} x {}",
        b.symbol, b.bid_price, b.bid_quantity, b.ask_price, b.ask_quantity
    )
}

/// The first 200 characters of a frame, escaped, so it stays one stderr line.
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: BinanceArgs,
    }

    fn parse(argv: &[&str]) -> BinanceArgs {
        Wrapper::try_parse_from(argv).unwrap().args
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
        let names: Vec<String> = args
            .streams()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            names,
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
            "agg-trade,book-ticker,depth5,depth10,depth20,kline-1M,mark-price,ticker",
            "--all-tickers",
            "--all-mark-prices",
        ]);
        let names: Vec<String> = args
            .streams()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            names,
            [
                "!ticker@arr",
                "!markPrice@arr@1s",
                "btcusdt@aggTrade",
                "btcusdt@bookTicker",
                "btcusdt@depth5@100ms",
                "btcusdt@depth10@100ms",
                "btcusdt@depth20@100ms",
                "btcusdt@kline_1M",
                "btcusdt@markPrice@1s",
                "btcusdt@ticker",
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
            err.contains("\"trades\"") && err.contains("agg-trade") && err.contains("1M"),
            "{err}"
        );
        let err = parse(&["test", "--symbol", "BTCUSDT", "--kind", "kline-1s"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("kline-1s"), "{err}");
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
            "--all-mark-prices",
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
