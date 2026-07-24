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
use polyoxide_clob::ws::{Channel, MarketMessage, MarketSubscriptionOptions, WebSocket};

use crate::commands::common::parsing::parse_duration;

/// Market event types to filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MarketEventType {
    /// Order book snapshots
    Book,
    /// Price changes
    Price,
    /// Last trade price
    Trade,
    /// Tick size changes
    Tick,
    /// Best bid/ask updates (requires `--custom-features`)
    BestBidAsk,
    /// New market creations (requires `--custom-features`)
    NewMarket,
    /// Market resolutions (requires `--custom-features`)
    MarketResolved,
}

#[derive(Args)]
pub struct MarketArgs {
    /// Asset IDs (token IDs) to subscribe to
    #[arg(required = true)]
    asset_ids: Vec<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Filter by event type (can be specified multiple times)
    #[arg(long, value_enum)]
    filter: Vec<MarketEventType>,

    /// Subscribe with custom_feature_enabled, which is required for the
    /// best-bid-ask, new-market, and market-resolved events
    #[arg(long)]
    custom_features: bool,

    /// Exit after receiving N messages
    #[arg(short = 'n', long)]
    count: Option<u64>,

    /// Exit after specified duration (e.g., "30s", "5m", "1h")
    #[arg(short, long, value_parser = parse_duration)]
    timeout: Option<Duration>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Pretty-printed JSON
    #[default]
    Pretty,
    /// Compact JSON (one message per line)
    Json,
    /// Human-readable summary
    Summary,
}

pub async fn run(args: MarketArgs) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    eprintln!(
        "Connecting to market channel for {} asset(s)...",
        args.asset_ids.len()
    );
    if !args.filter.is_empty() {
        eprintln!("Filtering: {:?}", args.filter);
    }
    if let Some(count) = args.count {
        eprintln!("Will exit after {} message(s)", count);
    }
    if let Some(timeout) = args.timeout {
        eprintln!("Will exit after {:?}", timeout);
    }
    // Warn rather than silently returning nothing: filtering for a gated event
    // without the opt-in flag produces an empty stream, which reads as "no
    // activity" instead of "misconfigured".
    let gated = [
        MarketEventType::BestBidAsk,
        MarketEventType::NewMarket,
        MarketEventType::MarketResolved,
    ];
    if !args.custom_features && args.filter.iter().any(|f| gated.contains(f)) {
        eprintln!(
            "warning: --filter selects a gated event but --custom-features was not passed; \
             the server will never send those events"
        );
    }
    eprintln!("Press Ctrl+C to exit\n");

    let options = if args.custom_features {
        MarketSubscriptionOptions::default().with_custom_features()
    } else {
        MarketSubscriptionOptions::default()
    };
    let mut ws = WebSocket::connect_market_with(args.asset_ids, options).await?;
    let mut message_count: u64 = 0;
    let start_time = std::time::Instant::now();

    while running.load(Ordering::SeqCst) {
        // Check timeout
        if let Some(timeout) = args.timeout {
            if start_time.elapsed() >= timeout {
                eprintln!("\nTimeout reached");
                break;
            }
        }

        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(channel)) => {
                        if should_print(&channel, &args.filter) {
                            print_message(&channel, args.format)?;
                            message_count += 1;

                            // Check count limit
                            if let Some(count) = args.count {
                                if message_count >= count {
                                    eprintln!("\nReached {} message(s)", count);
                                    break;
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("Error: {}", e);
                        break;
                    }
                    None => {
                        eprintln!("Connection closed");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }

    eprintln!("\nDisconnecting... ({} messages received)", message_count);
    ws.close().await?;

    Ok(())
}

fn should_print(channel: &Channel, filters: &[MarketEventType]) -> bool {
    if filters.is_empty() {
        return true;
    }

    match channel {
        Channel::Market(msg) => {
            let event_type = match msg {
                MarketMessage::Book(_) => MarketEventType::Book,
                MarketMessage::PriceChange(_) => MarketEventType::Price,
                MarketMessage::LastTradePrice(_) => MarketEventType::Trade,
                MarketMessage::TickSizeChange(_) => MarketEventType::Tick,
                MarketMessage::BestBidAsk(_) => MarketEventType::BestBidAsk,
                MarketMessage::NewMarket(_) => MarketEventType::NewMarket,
                MarketMessage::MarketResolved(_) => MarketEventType::MarketResolved,
                // A market event this build predates cannot match any filter,
                // so an explicit filter list excludes it.
                _ => return false,
            };
            filters.contains(&event_type)
        }
        _ => false,
    }
}

fn print_message(channel: &Channel, format: OutputFormat) -> Result<()> {
    match channel {
        Channel::Market(msg) => match format {
            OutputFormat::Pretty => {
                println!("{}", serde_json::to_string_pretty(&msg)?);
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(&msg)?);
            }
            OutputFormat::Summary => {
                print_market_summary(msg);
            }
        },
        _ => {
            // Other channels don't appear on a market connection.
        }
    }
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> &str {
    &s[..s.floor_char_boundary(max_len)]
}

fn print_market_summary(msg: &MarketMessage) {
    match msg {
        MarketMessage::Book(book) => {
            println!(
                "[BOOK] asset={}.. bids={} asks={}",
                truncate(&book.asset_id, 10),
                book.bids.len(),
                book.asks.len(),
            );
        }
        MarketMessage::PriceChange(pc) => {
            for change in &pc.price_changes {
                println!(
                    "[PRICE] asset={}.. price={} side={}",
                    truncate(&change.asset_id, 10),
                    change.price,
                    change.side
                );
            }
        }
        MarketMessage::TickSizeChange(tc) => {
            println!(
                "[TICK] asset={}.. old={} new={} side={}",
                truncate(&tc.asset_id, 10),
                tc.old_tick_size,
                tc.new_tick_size,
                tc.side
            );
        }
        MarketMessage::LastTradePrice(ltp) => {
            println!(
                "[TRADE] asset={}.. price={} side={} size={}",
                truncate(&ltp.asset_id, 10),
                ltp.price,
                ltp.side,
                ltp.size
            );
        }
        MarketMessage::BestBidAsk(bba) => {
            println!(
                "[BBO] asset={}.. bid={} ask={} spread={}",
                truncate(&bba.asset_id, 10),
                bba.best_bid,
                bba.best_ask,
                bba.spread
            );
        }
        MarketMessage::NewMarket(nm) => {
            println!(
                "[NEW] market={}.. slug={} outcomes={}",
                truncate(&nm.market, 10),
                nm.slug,
                nm.outcomes.join("/")
            );
        }
        MarketMessage::MarketResolved(mr) => {
            println!(
                "[RESOLVED] market={}.. winner={}",
                truncate(&mr.market, 10),
                mr.winning_outcome
            );
        }
        // `MarketMessage` is #[non_exhaustive]; print something rather than
        // dropping an event this build doesn't know about.
        other => println!("[?] unhandled event: {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestWrapper {
        #[command(flatten)]
        args: MarketArgs,
    }

    fn try_parse(args: &[&str]) -> Result<TestWrapper, clap::Error> {
        TestWrapper::try_parse_from(args)
    }

    #[test]
    fn requires_at_least_one_asset_id() {
        let result = try_parse(&["test"]);
        assert!(result.is_err());
    }

    #[test]
    fn parses_single_asset_id() {
        let w = try_parse(&["test", "asset-1"]).unwrap();
        assert_eq!(w.args.asset_ids, vec!["asset-1"]);
    }

    #[test]
    fn parses_multiple_asset_ids() {
        let w = try_parse(&["test", "asset-1", "asset-2", "asset-3"]).unwrap();
        assert_eq!(w.args.asset_ids, vec!["asset-1", "asset-2", "asset-3"]);
    }

    #[test]
    fn custom_features_defaults_off_and_parses() {
        let w = try_parse(&["test", "id"]).unwrap();
        assert!(!w.args.custom_features, "must stay opt-in");

        let w = try_parse(&["test", "id", "--custom-features"]).unwrap();
        assert!(w.args.custom_features);
    }

    #[test]
    fn parses_gated_event_filters() {
        let w = try_parse(&[
            "test",
            "id",
            "--filter",
            "best-bid-ask",
            "--filter",
            "market-resolved",
        ])
        .unwrap();
        assert_eq!(w.args.filter.len(), 2);
        assert!(matches!(w.args.filter[0], MarketEventType::BestBidAsk));
        assert!(matches!(w.args.filter[1], MarketEventType::MarketResolved));
    }

    #[test]
    fn default_format_is_pretty() {
        let w = try_parse(&["test", "id"]).unwrap();
        assert!(matches!(w.args.format, OutputFormat::Pretty));
    }

    #[test]
    fn format_json() {
        let w = try_parse(&["test", "id", "--format", "json"]).unwrap();
        assert!(matches!(w.args.format, OutputFormat::Json));
    }

    #[test]
    fn format_summary() {
        let w = try_parse(&["test", "id", "--format", "summary"]).unwrap();
        assert!(matches!(w.args.format, OutputFormat::Summary));
    }

    #[test]
    fn invalid_format_errors() {
        let result = try_parse(&["test", "id", "--format", "xml"]);
        assert!(result.is_err());
    }

    #[test]
    fn filter_book() {
        let w = try_parse(&["test", "id", "--filter", "book"]).unwrap();
        assert_eq!(w.args.filter, vec![MarketEventType::Book]);
    }

    #[test]
    fn filter_multiple() {
        let w = try_parse(&["test", "id", "--filter", "book", "--filter", "price"]).unwrap();
        assert_eq!(
            w.args.filter,
            vec![MarketEventType::Book, MarketEventType::Price]
        );
    }

    #[test]
    fn filter_all_types() {
        let w = try_parse(&[
            "test", "id", "--filter", "book", "--filter", "price", "--filter", "trade", "--filter",
            "tick",
        ])
        .unwrap();
        assert_eq!(w.args.filter.len(), 4);
    }

    #[test]
    fn invalid_filter_errors() {
        let result = try_parse(&["test", "id", "--filter", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn count_flag() {
        let w = try_parse(&["test", "id", "-n", "10"]).unwrap();
        assert_eq!(w.args.count.unwrap(), 10);
    }

    #[test]
    fn timeout_flag() {
        let w = try_parse(&["test", "id", "--timeout", "30s"]).unwrap();
        assert_eq!(w.args.timeout.unwrap(), std::time::Duration::from_secs(30));
    }

    #[test]
    fn timeout_minutes() {
        let w = try_parse(&["test", "id", "--timeout", "5m"]).unwrap();
        assert_eq!(w.args.timeout.unwrap(), std::time::Duration::from_secs(300));
    }

    #[test]
    fn timeout_invalid_errors() {
        let result = try_parse(&["test", "id", "--timeout", ""]);
        assert!(result.is_err());
    }

    #[test]
    fn truncate_shorter_than_max() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_longer_than_max() {
        assert_eq!(truncate("hello world", 5), "hello");
    }

    #[test]
    fn truncate_zero_length() {
        assert_eq!(truncate("hello", 0), "");
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn truncate_multibyte_boundary() {
        // "café" = [99, 97, 102, 195, 169] — 'é' is 2 bytes starting at index 3
        // Truncating at 4 would split 'é' with old code; floor_char_boundary rounds down to 3
        assert_eq!(truncate("café", 4), "caf");
    }

    #[test]
    fn truncate_multibyte_after_boundary() {
        // Truncating at 5 includes the full 'é'
        assert_eq!(truncate("café", 5), "café");
    }

    #[test]
    fn truncate_emoji() {
        // '🎲' is 4 bytes — truncating at 2 should give empty string
        assert_eq!(truncate("🎲dice", 2), "");
    }

    #[test]
    fn should_print_no_filters_passes_all() {
        let channel = Channel::Market(MarketMessage::Book(polyoxide_clob::ws::BookMessage {
            event_type: "book".to_string(),
            asset_id: "test".to_string(),
            market: "test".to_string(),
            bids: vec![],
            asks: vec![],
            hash: "test".to_string(),
            timestamp: "0".to_string(),
            last_trade_price: None,
        }));
        assert!(should_print(&channel, &[]));
    }

    #[test]
    fn should_print_with_matching_filter() {
        let channel = Channel::Market(MarketMessage::Book(polyoxide_clob::ws::BookMessage {
            event_type: "book".to_string(),
            asset_id: "test".to_string(),
            market: "test".to_string(),
            bids: vec![],
            asks: vec![],
            hash: "test".to_string(),
            timestamp: "0".to_string(),
            last_trade_price: None,
        }));
        assert!(should_print(&channel, &[MarketEventType::Book]));
    }

    #[test]
    fn should_print_with_non_matching_filter() {
        let channel = Channel::Market(MarketMessage::Book(polyoxide_clob::ws::BookMessage {
            event_type: "book".to_string(),
            asset_id: "test".to_string(),
            market: "test".to_string(),
            bids: vec![],
            asks: vec![],
            hash: "test".to_string(),
            timestamp: "0".to_string(),
            last_trade_price: None,
        }));
        // Filter asks for Price but message is Book
        assert!(!should_print(&channel, &[MarketEventType::Price]));
    }

    #[test]
    fn should_print_user_channel_on_market_returns_false() {
        // User channel messages should be rejected by the market should_print
        // when a filter is active
        let channel = Channel::User(polyoxide_clob::ws::UserMessage::Order(
            polyoxide_clob::ws::OrderMessage {
                event_type: "order".to_string(),
                id: "test".to_string(),
                asset_id: "test".to_string(),
                market: "test".to_string(),
                outcome: "Yes".to_string(),
                price: "0.5".to_string(),
                side: "BUY".to_string(),
                original_size: "10".to_string(),
                size_matched: "0".to_string(),
                order_type: polyoxide_clob::ws::OrderEventType::Placement,
                order_owner: Some("0x123".to_string()),
                timestamp: "123".to_string(),
            },
        ));
        assert!(!should_print(&channel, &[MarketEventType::Book]));
    }

    fn bbo_channel() -> Channel {
        Channel::Market(MarketMessage::BestBidAsk(
            polyoxide_clob::ws::BestBidAskMessage {
                event_type: "best_bid_ask".to_string(),
                asset_id: "asset-1".to_string(),
                market: "0xcond".to_string(),
                best_bid: "0.55".parse().unwrap(),
                best_ask: "0.57".parse().unwrap(),
                spread: "0.02".parse().unwrap(),
                timestamp: "0".to_string(),
            },
        ))
    }

    #[test]
    fn should_print_matches_gated_event_filters() {
        assert!(should_print(&bbo_channel(), &[MarketEventType::BestBidAsk]));
        // A gated event must not slip through a filter for a different type.
        assert!(!should_print(&bbo_channel(), &[MarketEventType::Book]));
        // No filters means everything passes, gated events included.
        assert!(should_print(&bbo_channel(), &[]));
    }

    #[test]
    fn print_market_summary_handles_gated_events() {
        // Exercises the new match arms; the guard is that they do not panic on
        // field access (e.g. truncate on a market id shorter than the cut).
        print_market_summary(match &bbo_channel() {
            Channel::Market(msg) => msg,
            _ => unreachable!(),
        });
    }
}
