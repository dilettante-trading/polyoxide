//! `polyoxide polymarket rtds stream`: stream RTDS reference prices.

use std::{io::Write, time::Duration};

use clap::{Args, Subcommand};
use color_eyre::eyre::Result;
use futures_util::Stream;
use polyoxide_polymarket::rtds::{
    PriceEvent, PriceUpdate, Rtds, RtdsError, Subscription, Topic, TwapWindow,
};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};
use crate::commands::stream::{self, excerpt, Feed, Handled, Limits, OutputFormat};

#[derive(Subcommand)]
pub enum RtdsCommand {
    /// Stream reference prices (Binance, Chainlink spot, Chainlink TWAP)
    Stream {
        #[command(flatten)]
        args: PricesArgs,
    },
}

impl RtdsCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Stream { args } => run(args).await,
        }
    }
}

/// Which RTDS price topic to stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PriceTopic {
    /// Binance spot prices. Symbols look like `btcusdt`.
    Binance,
    /// Chainlink spot prices. Symbols look like `btc/usd`.
    Chainlink,
    /// Chainlink time-weighted average prices. Needs `--window`.
    Twap,
}

#[derive(Args)]
pub struct PricesArgs {
    /// Which price topic to stream
    #[arg(long, value_enum, default_value = "twap")]
    topic: PriceTopic,

    /// TWAP lookback window in seconds. Only meaningful with `--topic twap`;
    /// passing it with any other topic is ignored with a warning.
    #[arg(long, value_parser = ["30", "60"])]
    window: Option<String>,

    /// Symbols to filter to, comma-separated. Omit to receive every symbol on
    /// the topic.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    symbol: Vec<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Exit after receiving N updates
    #[arg(short = 'n', long)]
    count: Option<u64>,

    /// Exit after the given duration (e.g. "30s", "5m")
    #[arg(short, long, value_parser = parse_duration)]
    timeout: Option<Duration>,
}

/// `--window` only affects the TWAP topic. Warn rather than error on a
/// non-TWAP topic, matching `polymarket clob stream market`'s handling of a
/// gated-event filter without `--custom-features`: the flag is harmless noise
/// here, not a contradiction worth refusing to run over.
fn warn_if_window_ignored(window: Option<&str>, topic_name: &str) {
    if window.is_some() {
        eprintln!(
            "warning: --window has no effect with --topic {topic_name}; it only applies to --topic twap"
        );
    }
}

impl PricesArgs {
    /// What to subscribe to on `topic`: one subscription per symbol, or one
    /// unfiltered subscription for every symbol.
    fn subscriptions(&self, topic: Topic) -> Vec<Subscription> {
        // An empty --symbol list means "every symbol", which is a single
        // unfiltered subscription — not an empty fan-out. `Subscription::symbols`
        // returns an empty vec for an empty input, and `Rtds::connect` refuses an
        // empty subscription set outright, so this has to be special-cased here.
        // An empty entry, as in `--symbol btc/usd,`, is dropped: a filter that
        // matches nothing would deliver nothing, and say nothing about it.
        let symbols: Vec<String> = self
            .symbol
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect();
        if symbols.is_empty() {
            vec![Subscription::for_topic(topic)]
        } else {
            Subscription::for_topic(topic).symbols(symbols)
        }
    }
}

/// Connect to RTDS and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(args: PricesArgs) -> Result<()> {
    let topic = match args.topic {
        PriceTopic::Binance => {
            warn_if_window_ignored(args.window.as_deref(), "binance");
            Topic::BinanceSpot
        }
        PriceTopic::Chainlink => {
            warn_if_window_ignored(args.window.as_deref(), "chainlink");
            Topic::ChainlinkSpot
        }
        PriceTopic::Twap => {
            let seconds: u32 = args.window.as_deref().unwrap_or("30").parse()?;
            let window = TwapWindow::from_seconds(seconds)
                .ok_or_else(|| color_eyre::eyre::eyre!("window must be 30 or 60"))?;
            Topic::ChainlinkTwap(window)
        }
    };

    let subscriptions = args.subscriptions(topic);

    eprintln!("Connecting to RTDS...");
    let mut feed = Rtds::connect(subscriptions).await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
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
/// a scripted list of events. Updates go to `out`; backfill markers and
/// skipped frames go to `err`, so JSON output stays clean JSONL.
pub async fn run_with<S>(
    args: PricesArgs,
    events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<PriceEvent, RtdsError>> + Unpin,
{
    let limits = Limits {
        count: args.count,
        timeout: args.timeout,
    };
    stream::run(limits, events, &mut Prices(args.format), out, err).await
}

/// RTDS's half of a run: what each event means, printed in a format.
struct Prices(OutputFormat);

impl Feed for Prices {
    type Event = PriceEvent;
    type Error = RtdsError;

    fn event(
        &mut self,
        event: PriceEvent,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<Handled> {
        match event {
            PriceEvent::Snapshot(snapshot) => {
                writeln!(
                    err,
                    "# backfill: {} point(s) for {}",
                    snapshot.points.len(),
                    snapshot.symbol
                )?;
                Ok(Handled::Skipped)
            }
            PriceEvent::Update(update) => {
                print_update(&update, self.0, out)?;
                Ok(Handled::Printed)
            }
            // `PriceEvent` is #[non_exhaustive]; a future variant is skipped
            // rather than treated as a fault.
            _ => Ok(Handled::Skipped),
        }
    }

    fn error(&mut self, error: RtdsError, err: &mut dyn Write) -> Result<()> {
        match error {
            // One frame, or one value, the connection could not read; it
            // carries on.
            RtdsError::Json { raw, source } => {
                writeln!(
                    err,
                    "# skipped a frame that did not parse ({source}): {}",
                    excerpt(&raw)
                )?;
                Ok(())
            }
            RtdsError::Precision { raw, topic } => {
                writeln!(
                    err,
                    "# skipped a value on {topic:?} that did not decode: {}",
                    excerpt(&raw)
                )?;
                Ok(())
            }
            error => Err(error.into()),
        }
    }
}

/// Print one update and flush it, so a reader sees it at once whatever kind of
/// writer `out` is.
fn print_update(update: &PriceUpdate, format: OutputFormat, out: &mut dyn Write) -> Result<()> {
    match format {
        OutputFormat::Pretty => writeln!(
            out,
            "{:>10} {:>20} {}",
            update.symbol(),
            update.value(),
            update.observed_at()
        )?,
        // `value` is serialized as a string so the exact decimal survives —
        // a JSON number would round-trip through a float and degrade it.
        OutputFormat::Json => {
            let line = serde_json::json!({
                "symbol": update.symbol(),
                "value": update.value().to_string(),
                "observed_at": update.observed_at(),
                "window_s": update.window().map(TwapWindow::seconds),
            });
            writeln!(out, "{}", serde_json::to_string(&line)?)?;
        }
    }
    out.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestWrapper {
        #[command(flatten)]
        args: PricesArgs,
    }

    fn try_parse(args: &[&str]) -> Result<TestWrapper, clap::Error> {
        TestWrapper::try_parse_from(args)
    }

    #[test]
    fn default_topic_is_twap() {
        let w = try_parse(&["test"]).unwrap();
        assert!(matches!(w.args.topic, PriceTopic::Twap));
    }

    #[test]
    fn parses_binance_topic() {
        let w = try_parse(&["test", "--topic", "binance"]).unwrap();
        assert!(matches!(w.args.topic, PriceTopic::Binance));
    }

    #[test]
    fn parses_chainlink_topic() {
        let w = try_parse(&["test", "--topic", "chainlink"]).unwrap();
        assert!(matches!(w.args.topic, PriceTopic::Chainlink));
    }

    #[test]
    fn invalid_topic_errors() {
        let result = try_parse(&["test", "--topic", "coinbase"]);
        assert!(result.is_err());
    }

    #[test]
    fn window_defaults_to_none() {
        let w = try_parse(&["test"]).unwrap();
        assert_eq!(w.args.window, None);
    }

    #[test]
    fn window_accepts_30_and_60() {
        let w = try_parse(&["test", "--window", "30"]).unwrap();
        assert_eq!(w.args.window.as_deref(), Some("30"));

        let w = try_parse(&["test", "--window", "60"]).unwrap();
        assert_eq!(w.args.window.as_deref(), Some("60"));
    }

    #[test]
    fn window_rejects_other_values() {
        let result = try_parse(&["test", "--window", "45"]);
        assert!(result.is_err());
    }

    #[test]
    fn symbol_defaults_to_empty() {
        let w = try_parse(&["test"]).unwrap();
        assert!(w.args.symbol.is_empty());
    }

    #[test]
    fn parses_multiple_symbols() {
        let w = try_parse(&["test", "--symbol", "btc/usd", "--symbol", "eth/usd"]).unwrap();
        assert_eq!(w.args.symbol, vec!["btc/usd", "eth/usd"]);
    }

    #[test]
    fn symbols_split_on_commas_and_are_trimmed() {
        let w = try_parse(&["test", "--symbol", "btc/usd, eth/usd"]).unwrap();
        assert_eq!(w.args.symbol, vec!["btc/usd", "eth/usd"]);
    }

    #[test]
    fn an_empty_symbol_entry_is_dropped() {
        let topic = Topic::ChainlinkSpot;
        let filters = |argv: &[&str]| -> Vec<Option<String>> {
            try_parse(argv)
                .unwrap()
                .args
                .subscriptions(topic)
                .iter()
                .map(|s| s.symbol_filter().map(str::to_owned))
                .collect()
        };
        assert_eq!(
            filters(&["test", "--symbol", "btc/usd,"]),
            [Some("btc/usd".to_owned())]
        );
        assert_eq!(
            filters(&["test", "--symbol", "btc/usd,,eth/usd"]),
            [Some("btc/usd".to_owned()), Some("eth/usd".to_owned())]
        );
        // Nothing left, or nothing given, is the one unfiltered subscription.
        assert_eq!(filters(&["test", "--symbol", ","]), [None]);
        assert_eq!(filters(&["test"]), [None]);
    }

    #[test]
    fn default_format_is_pretty() {
        let w = try_parse(&["test"]).unwrap();
        assert!(matches!(w.args.format, OutputFormat::Pretty));
    }

    #[test]
    fn format_json() {
        let w = try_parse(&["test", "--format", "json"]).unwrap();
        assert!(matches!(w.args.format, OutputFormat::Json));
    }

    #[test]
    fn count_flag() {
        let w = try_parse(&["test", "-n", "5"]).unwrap();
        assert_eq!(w.args.count.unwrap(), 5);
    }

    #[test]
    fn timeout_flag() {
        let w = try_parse(&["test", "--timeout", "30s"]).unwrap();
        assert_eq!(w.args.timeout.unwrap(), Duration::from_secs(30));
    }
}
