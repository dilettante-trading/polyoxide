use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use clap::Args;
use color_eyre::eyre::Result;
use futures_util::StreamExt;
use polyoxide_rtds::{PriceEvent, Rtds, Subscription, Topic, TwapWindow};

use crate::commands::common::parsing::parse_duration;

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

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable one line per update.
    #[default]
    Pretty,
    /// Compact JSON, one object per line.
    Json,
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

    /// Symbols to filter to. Omit to receive every symbol on the topic.
    #[arg(long)]
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
/// non-TWAP topic, matching `ws market`'s handling of a gated-event filter
/// without `--custom-features`: the flag is harmless noise here, not a
/// contradiction worth refusing to run over.
fn warn_if_window_ignored(window: Option<&str>, topic_name: &str) {
    if window.is_some() {
        eprintln!(
            "warning: --window has no effect with --topic {topic_name}; it only applies to --topic twap"
        );
    }
}

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

    // An empty --symbol list means "every symbol", which is a single
    // unfiltered subscription — not an empty fan-out. `Subscription::symbols`
    // returns an empty vec for an empty input, and `Rtds::connect` refuses an
    // empty subscription set outright, so this has to be special-cased here.
    let subscriptions = if args.symbol.is_empty() {
        vec![Subscription::for_topic(topic)]
    } else {
        Subscription::for_topic(topic).symbols(args.symbol)
    };

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    eprintln!("Connecting to RTDS...");
    if let Some(count) = args.count {
        eprintln!("Will exit after {count} update(s)");
    }
    if let Some(timeout) = args.timeout {
        eprintln!("Will exit after {timeout:?}");
    }
    eprintln!("Press Ctrl+C to exit\n");

    let mut stream = Rtds::connect(subscriptions).await?;
    let mut update_count: u64 = 0;
    let start_time = std::time::Instant::now();

    while running.load(Ordering::SeqCst) {
        if let Some(timeout) = args.timeout {
            if start_time.elapsed() >= timeout {
                eprintln!("\nTimeout reached");
                break;
            }
        }

        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(Ok(PriceEvent::Snapshot(snapshot))) => {
                        eprintln!(
                            "# backfill: {} point(s) for {}",
                            snapshot.points.len(),
                            snapshot.symbol
                        );
                    }
                    Some(Ok(PriceEvent::Update(update))) => {
                        print_update(&update, args.format)?;
                        update_count += 1;

                        if let Some(count) = args.count {
                            if update_count >= count {
                                eprintln!("\nReached {count} update(s)");
                                break;
                            }
                        }
                    }
                    // `PriceEvent` is #[non_exhaustive]; a future variant is
                    // skipped rather than treated as a fault.
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        eprintln!("Error: {e}");
                        break;
                    }
                    None => {
                        eprintln!("Connection closed");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }

    eprintln!("\nDisconnecting... ({update_count} update(s) received)");
    stream.close().await?;

    Ok(())
}

fn print_update(update: &polyoxide_rtds::PriceUpdate, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Pretty => println!(
            "{:>10} {:>20} {}",
            update.symbol(),
            update.value(),
            update.observed_at()
        ),
        // `value` is serialized as a string so the exact decimal survives —
        // a JSON number would round-trip through a float and degrade it.
        OutputFormat::Json => {
            let line = serde_json::json!({
                "symbol": update.symbol(),
                "value": update.value().to_string(),
                "observed_at": update.observed_at(),
                "window_s": update.window().map(TwapWindow::seconds),
            });
            println!("{}", serde_json::to_string(&line)?);
        }
    }
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
