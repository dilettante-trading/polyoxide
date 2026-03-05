use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use clap::{Args, ValueEnum};
use color_eyre::eyre::Result;
use futures_util::StreamExt;
use polyoxide_clob::ws::{ApiCredentials, Channel, UserMessage, WebSocket};

use crate::commands::common::parsing::parse_duration;

/// User event types to filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UserEventType {
    /// Order updates
    Order,
    /// Trade updates
    Trade,
}

#[derive(Args)]
pub struct UserArgs {
    /// Market IDs (condition IDs) to subscribe to
    #[arg(required = true)]
    market_ids: Vec<String>,

    /// API key
    #[arg(long, env = "POLYMARKET_API_KEY")]
    api_key: Option<String>,

    /// API secret
    #[arg(long, env = "POLYMARKET_API_SECRET")]
    api_secret: Option<String>,

    /// API passphrase
    #[arg(long, env = "POLYMARKET_API_PASSPHRASE")]
    api_passphrase: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Filter by event type (can be specified multiple times)
    #[arg(long, value_enum)]
    filter: Vec<UserEventType>,

    /// Exit after receiving N messages
    #[arg(short = 'n', long)]
    count: Option<u64>,

    /// Exit after specified duration (e.g., "30s", "5m", "1h")
    #[arg(short, long, value_parser = parse_duration)]
    timeout: Option<Duration>,
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    /// Pretty-printed JSON
    #[default]
    Pretty,
    /// Compact JSON (one message per line)
    Json,
    /// Human-readable summary
    Summary,
}

pub async fn run(args: UserArgs) -> Result<()> {
    let credentials = get_credentials(args.api_key, args.api_secret, args.api_passphrase)?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    eprintln!(
        "Connecting to user channel for {} market(s)...",
        args.market_ids.len()
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
    eprintln!("Press Ctrl+C to exit\n");

    let mut ws = WebSocket::connect_user(args.market_ids, credentials).await?;
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

fn get_credentials(
    api_key: Option<String>,
    api_secret: Option<String>,
    api_passphrase: Option<String>,
) -> Result<ApiCredentials> {
    match (api_key, api_secret, api_passphrase) {
        (Some(key), Some(secret), Some(passphrase)) => {
            Ok(ApiCredentials::new(key, secret, passphrase))
        }
        (key, secret, passphrase) => {
            let mut missing = Vec::new();
            if key.is_none() {
                missing.push("--api-key / POLYMARKET_API_KEY");
            }
            if secret.is_none() {
                missing.push("--api-secret / POLYMARKET_API_SECRET");
            }
            if passphrase.is_none() {
                missing.push("--api-passphrase / POLYMARKET_API_PASSPHRASE");
            }
            let list = missing
                .iter()
                .map(|m| format!("  - {}", m))
                .collect::<Vec<_>>()
                .join("\n");
            Err(color_eyre::eyre::eyre!(
                "Missing required credentials:\n\n{list}"
            ))
        }
    }
}

fn should_print(channel: &Channel, filters: &[UserEventType]) -> bool {
    if filters.is_empty() {
        return true;
    }

    match channel {
        Channel::User(msg) => {
            let event_type = match msg {
                UserMessage::Order(_) => UserEventType::Order,
                UserMessage::Trade(_) => UserEventType::Trade,
            };
            filters.contains(&event_type)
        }
        Channel::Market(_) => false,
    }
}

fn print_message(channel: &Channel, format: OutputFormat) -> Result<()> {
    match channel {
        Channel::User(msg) => match format {
            OutputFormat::Pretty => {
                println!("{}", serde_json::to_string_pretty(&msg)?);
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(&msg)?);
            }
            OutputFormat::Summary => {
                print_user_summary(msg);
            }
        },
        Channel::Market(_) => {
            // Shouldn't happen on user channel
        }
    }
    Ok(())
}

fn print_user_summary(msg: &UserMessage) {
    match msg {
        UserMessage::Order(order) => {
            println!(
                "[ORDER] id={} type={:?} side={} price={} size={} matched={}",
                &order.id[..order.id.floor_char_boundary(8)],
                order.order_type,
                order.side,
                order.price,
                order.original_size,
                order.size_matched
            );
        }
        UserMessage::Trade(trade) => {
            println!(
                "[TRADE] id={} side={} price={} size={} status={:?}",
                &trade.id[..trade.id.floor_char_boundary(8)],
                trade.side,
                trade.price,
                trade.size,
                trade.status
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestWrapper {
        #[command(flatten)]
        args: UserArgs,
    }

    fn try_parse(args: &[&str]) -> Result<TestWrapper, clap::Error> {
        TestWrapper::try_parse_from(args)
    }

    #[test]
    fn requires_at_least_one_market_id() {
        let result = try_parse(&["test"]);
        assert!(result.is_err());
    }

    #[test]
    fn parses_single_market_id() {
        let w = try_parse(&["test", "market-1"]).unwrap();
        assert_eq!(w.args.market_ids, vec!["market-1"]);
    }

    #[test]
    fn parses_multiple_market_ids() {
        let w = try_parse(&["test", "m1", "m2"]).unwrap();
        assert_eq!(w.args.market_ids, vec!["m1", "m2"]);
    }

    #[test]
    fn default_format_is_pretty() {
        let w = try_parse(&["test", "id"]).unwrap();
        assert!(matches!(w.args.format, OutputFormat::Pretty));
    }

    #[test]
    fn filter_order() {
        let w = try_parse(&["test", "id", "--filter", "order"]).unwrap();
        assert_eq!(w.args.filter, vec![UserEventType::Order]);
    }

    #[test]
    fn filter_trade() {
        let w = try_parse(&["test", "id", "--filter", "trade"]).unwrap();
        assert_eq!(w.args.filter, vec![UserEventType::Trade]);
    }

    #[test]
    fn filter_multiple() {
        let w = try_parse(&["test", "id", "--filter", "order", "--filter", "trade"]).unwrap();
        assert_eq!(
            w.args.filter,
            vec![UserEventType::Order, UserEventType::Trade]
        );
    }

    #[test]
    fn invalid_filter_errors() {
        let result = try_parse(&["test", "id", "--filter", "book"]);
        assert!(result.is_err());
    }

    #[test]
    fn api_credentials_via_flags() {
        let w = try_parse(&[
            "test",
            "id",
            "--api-key",
            "mykey",
            "--api-secret",
            "mysecret",
            "--api-passphrase",
            "mypass",
        ])
        .unwrap();
        assert_eq!(w.args.api_key.unwrap(), "mykey");
        assert_eq!(w.args.api_secret.unwrap(), "mysecret");
        assert_eq!(w.args.api_passphrase.unwrap(), "mypass");
    }

    #[test]
    fn get_credentials_all_present() {
        let result = get_credentials(
            Some("key".to_string()),
            Some("secret".to_string()),
            Some("pass".to_string()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn get_credentials_missing_key() {
        let err = get_credentials(None, Some("secret".to_string()), Some("pass".to_string()))
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_KEY"), "got: {msg}");
    }

    #[test]
    fn get_credentials_missing_secret() {
        let err =
            get_credentials(Some("key".to_string()), None, Some("pass".to_string())).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_SECRET"), "got: {msg}");
    }

    #[test]
    fn get_credentials_missing_passphrase() {
        let err =
            get_credentials(Some("key".to_string()), Some("secret".to_string()), None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_PASSPHRASE"), "got: {msg}");
    }

    #[test]
    fn get_credentials_all_missing() {
        let err = get_credentials(None, None, None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_KEY"), "got: {msg}");
        assert!(msg.contains("POLYMARKET_API_SECRET"), "got: {msg}");
        assert!(msg.contains("POLYMARKET_API_PASSPHRASE"), "got: {msg}");
    }

    #[test]
    fn get_credentials_missing_key_and_secret() {
        let err = get_credentials(None, None, Some("pass".to_string())).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_KEY"), "got: {msg}");
        assert!(msg.contains("POLYMARKET_API_SECRET"), "got: {msg}");
        assert!(!msg.contains("POLYMARKET_API_PASSPHRASE"), "got: {msg}");
    }

    fn make_order_channel() -> Channel {
        Channel::User(UserMessage::Order(polyoxide_clob::ws::OrderMessage {
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
        }))
    }

    fn make_trade_channel() -> Channel {
        Channel::User(UserMessage::Trade(polyoxide_clob::ws::TradeMessage {
            event_type: "trade".to_string(),
            id: "trade-1".to_string(),
            asset_id: "test".to_string(),
            market: "test".to_string(),
            outcome: "Yes".to_string(),
            price: "0.6".to_string(),
            size: "5".to_string(),
            side: "BUY".to_string(),
            status: polyoxide_clob::ws::TradeStatus::Confirmed,
            taker_order_id: "order-1".to_string(),
            maker_orders: vec![],
            owner: Some("0x123".to_string()),
            transaction_hash: None,
            timestamp: "456".to_string(),
        }))
    }

    #[test]
    fn should_print_no_filter_passes_all() {
        assert!(should_print(&make_order_channel(), &[]));
    }

    #[test]
    fn should_print_with_matching_order_filter() {
        assert!(should_print(&make_order_channel(), &[UserEventType::Order]));
    }

    #[test]
    fn should_print_with_non_matching_filter() {
        // Filter asks for Trade but message is Order -> should NOT print
        assert!(!should_print(
            &make_order_channel(),
            &[UserEventType::Trade]
        ));
    }

    #[test]
    fn should_print_trade_with_trade_filter() {
        assert!(should_print(&make_trade_channel(), &[UserEventType::Trade]));
    }

    #[test]
    fn should_print_trade_with_order_filter_fails() {
        assert!(!should_print(
            &make_trade_channel(),
            &[UserEventType::Order]
        ));
    }

    #[test]
    fn should_print_market_channel_on_user_returns_false() {
        // Market channel messages should be rejected by the user should_print
        // when a filter is active
        let channel = Channel::Market(polyoxide_clob::ws::MarketMessage::Book(
            polyoxide_clob::ws::BookMessage {
                event_type: "book".to_string(),
                asset_id: "test".to_string(),
                market: "test".to_string(),
                bids: vec![],
                asks: vec![],
                hash: "test".to_string(),
                timestamp: "0".to_string(),
                last_trade_price: None,
            },
        ));
        assert!(!should_print(&channel, &[UserEventType::Order]));
    }
}
