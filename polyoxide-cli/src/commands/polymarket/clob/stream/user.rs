//! `polyoxide polymarket clob stream user`: stream the CLOB's user channel.

use std::{io::Write, time::Duration};

use clap::{Args, ValueEnum};
use color_eyre::eyre::Result;
use futures_util::Stream;
use polyoxide_polymarket::clob::ws::{
    ApiCredentials, Channel, ClobWsError, UserMessage, WebSocket,
};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};
use crate::commands::stream::{self, Feed, Handled, Limits, OutputFormat};

/// Credential source for authenticated commands
#[cfg(feature = "keychain")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CredentialSource {
    /// Load from environment variables (default)
    Env,
    /// Load from OS keychain
    Keychain,
}

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
    /// Market IDs (condition IDs) to subscribe to, comma-separated or repeated
    #[arg(required = true, value_delimiter = ',', value_parser = parse_list_entry)]
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

    /// Output format: `pretty` prints one summary line per event, `json` the
    /// event as compact JSON
    #[arg(short, long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Filter by event type, comma-separated or repeated
    #[arg(long, value_enum, value_delimiter = ',')]
    filter: Vec<UserEventType>,

    /// Exit after printing N messages
    #[arg(short = 'n', long)]
    count: Option<u64>,

    /// Exit after specified duration (e.g., "30s", "5m", "1h")
    #[arg(short, long, value_parser = parse_duration)]
    timeout: Option<Duration>,

    /// Credential source (env or keychain)
    #[cfg(feature = "keychain")]
    #[arg(long, value_enum)]
    credential_source: Option<CredentialSource>,
}

impl UserArgs {
    /// The market IDs to subscribe to. An empty entry, as in `a,`, is no
    /// market.
    fn markets(&self) -> Vec<String> {
        self.market_ids
            .iter()
            .filter(|id| !id.is_empty())
            .cloned()
            .collect()
    }
}

/// Connect to the user channel and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(mut args: UserArgs) -> Result<()> {
    let credentials = get_credentials(
        args.api_key.take(),
        args.api_secret.take(),
        args.api_passphrase.take(),
        #[cfg(feature = "keychain")]
        args.credential_source,
    )?;
    let market_ids = args.markets();
    eprintln!(
        "Connecting to user channel for {} market(s)...",
        market_ids.len()
    );
    let mut ws = WebSocket::connect_user(market_ids, credentials).await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
    let result = run_with(
        args,
        &mut ws,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await;
    let _ = ws.close().await;
    result
}

/// Filter and print events from any stream until `-n` or `-t` is reached or
/// the stream ends.
///
/// Takes the stream rather than connecting, so tests can drive every flag with
/// a scripted list of events. Events go to `out`; the filter and skipped
/// frames go to `err`, so JSON output stays clean JSONL.
pub async fn run_with<S>(
    args: UserArgs,
    events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<Channel, ClobWsError>> + Unpin,
{
    if !args.filter.is_empty() {
        writeln!(err, "# keeping {:?}", args.filter)?;
    }
    let limits = Limits {
        count: args.count,
        timeout: args.timeout,
    };
    let mut feed = User {
        filter: args.filter,
        format: args.format,
    };
    stream::run(limits, events, &mut feed, out, err).await
}

/// The user channel's half of a run: the filter, and how each event prints.
struct User {
    filter: Vec<UserEventType>,
    format: OutputFormat,
}

impl Feed for User {
    type Event = Channel;
    type Error = ClobWsError;

    fn event(
        &mut self,
        channel: Channel,
        out: &mut dyn Write,
        _: &mut dyn Write,
    ) -> Result<Handled> {
        // Other channels don't appear on a user connection.
        let Channel::User(msg) = &channel else {
            return Ok(Handled::Skipped);
        };
        if !should_print(&channel, &self.filter) {
            return Ok(Handled::Skipped);
        }
        match self.format {
            OutputFormat::Pretty => writeln!(out, "{}", print_user_summary(msg))?,
            OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(msg)?)?,
        }
        out.flush()?;
        Ok(Handled::Printed)
    }

    fn error(&mut self, error: ClobWsError, err: &mut dyn Write) -> Result<()> {
        match error {
            // One frame the connection could not read; it carries on.
            ClobWsError::Json(source) => {
                writeln!(err, "# skipped a frame that did not parse: {source}")?;
                Ok(())
            }
            error => Err(error.into()),
        }
    }
}

fn get_credentials(
    api_key: Option<String>,
    api_secret: Option<String>,
    api_passphrase: Option<String>,
    #[cfg(feature = "keychain")] credential_source: Option<CredentialSource>,
) -> Result<ApiCredentials> {
    #[cfg(feature = "keychain")]
    if matches!(credential_source, Some(CredentialSource::Keychain)) {
        return ApiCredentials::from_keychain()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to load credentials from keychain: {e}"));
    }

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
        _ => false,
    }
}

/// The `pretty` line for one event.
fn print_user_summary(msg: &UserMessage) -> String {
    match msg {
        UserMessage::Order(order) => format!(
            "[ORDER] id={} type={:?} side={} price={} size={} matched={}",
            &order.id[..order.id.floor_char_boundary(8)],
            order.order_type,
            order.side,
            order.price,
            order.original_size,
            order.size_matched
        ),
        UserMessage::Trade(trade) => format!(
            "[TRADE] id={} side={} price={} size={} status={:?}",
            &trade.id[..trade.id.floor_char_boundary(8)],
            trade.side,
            trade.price,
            trade.size,
            trade.status
        ),
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

    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        #[test]
        fn credential_source_keychain_parses() {
            let w = try_parse(&["test", "id", "--credential-source", "keychain"]).unwrap();
            assert!(matches!(
                w.args.credential_source,
                Some(CredentialSource::Keychain)
            ));
        }

        #[test]
        fn credential_source_env_parses() {
            let w = try_parse(&["test", "id", "--credential-source", "env"]).unwrap();
            assert!(matches!(
                w.args.credential_source,
                Some(CredentialSource::Env)
            ));
        }

        #[test]
        fn credential_source_default_is_none() {
            let w = try_parse(&["test", "id"]).unwrap();
            assert!(w.args.credential_source.is_none());
        }
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
    fn market_ids_and_filters_split_on_commas() {
        let w = try_parse(&["test", "m1, m2", "--filter", "order,trade"]).unwrap();
        assert_eq!(w.args.market_ids, vec!["m1", "m2"]);
        assert_eq!(
            w.args.filter,
            vec![UserEventType::Order, UserEventType::Trade]
        );
    }

    #[test]
    fn an_empty_market_entry_is_dropped() {
        for (list, expected) in [("a,", vec!["a"]), ("a,,b", vec!["a", "b"]), (",", vec![])] {
            let w = try_parse(&["test", list]).unwrap();
            assert_eq!(w.args.markets(), expected, "{list:?}");
        }
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
            #[cfg(feature = "keychain")]
            None,
        );
        assert!(result.is_ok());
    }

    #[cfg(feature = "keychain")]
    #[test]
    fn get_credentials_explicit_env_source_uses_flags() {
        let result = get_credentials(
            Some("key".to_string()),
            Some("secret".to_string()),
            Some("pass".to_string()),
            Some(CredentialSource::Env),
        );
        assert!(result.is_ok());
        let creds = result.unwrap();
        assert_eq!(creds.api_key, "key");
    }

    #[test]
    fn get_credentials_missing_key() {
        let err = get_credentials(
            None,
            Some("secret".to_string()),
            Some("pass".to_string()),
            #[cfg(feature = "keychain")]
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_KEY"), "got: {msg}");
    }

    #[test]
    fn get_credentials_missing_secret() {
        let err = get_credentials(
            Some("key".to_string()),
            None,
            Some("pass".to_string()),
            #[cfg(feature = "keychain")]
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_SECRET"), "got: {msg}");
    }

    #[test]
    fn get_credentials_missing_passphrase() {
        let err = get_credentials(
            Some("key".to_string()),
            Some("secret".to_string()),
            None,
            #[cfg(feature = "keychain")]
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_PASSPHRASE"), "got: {msg}");
    }

    #[test]
    fn get_credentials_all_missing() {
        let err = get_credentials(
            None,
            None,
            None,
            #[cfg(feature = "keychain")]
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_KEY"), "got: {msg}");
        assert!(msg.contains("POLYMARKET_API_SECRET"), "got: {msg}");
        assert!(msg.contains("POLYMARKET_API_PASSPHRASE"), "got: {msg}");
    }

    #[test]
    fn get_credentials_missing_key_and_secret() {
        let err = get_credentials(
            None,
            None,
            Some("pass".to_string()),
            #[cfg(feature = "keychain")]
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("POLYMARKET_API_KEY"), "got: {msg}");
        assert!(msg.contains("POLYMARKET_API_SECRET"), "got: {msg}");
        assert!(!msg.contains("POLYMARKET_API_PASSPHRASE"), "got: {msg}");
    }

    fn make_order_channel() -> Channel {
        Channel::User(UserMessage::Order(
            polyoxide_polymarket::clob::ws::OrderMessage {
                event_type: "order".to_string(),
                id: "test".to_string(),
                asset_id: "test".to_string(),
                market: "test".to_string(),
                outcome: "Yes".to_string(),
                price: "0.5".to_string(),
                side: "BUY".to_string(),
                original_size: "10".to_string(),
                size_matched: "0".to_string(),
                order_type: polyoxide_polymarket::clob::ws::OrderEventType::Placement,
                order_owner: Some("0x123".to_string()),
                timestamp: "123".to_string(),
            },
        ))
    }

    fn make_trade_channel() -> Channel {
        Channel::User(UserMessage::Trade(
            polyoxide_polymarket::clob::ws::TradeMessage {
                event_type: "trade".to_string(),
                id: "trade-1".to_string(),
                asset_id: "test".to_string(),
                market: "test".to_string(),
                outcome: "Yes".to_string(),
                price: "0.6".to_string(),
                size: "5".to_string(),
                side: "BUY".to_string(),
                status: polyoxide_polymarket::clob::ws::TradeStatus::Confirmed,
                taker_order_id: "order-1".to_string(),
                maker_orders: vec![],
                owner: Some("0x123".to_string()),
                transaction_hash: None,
                timestamp: "456".to_string(),
            },
        ))
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
        let channel = Channel::Market(polyoxide_polymarket::clob::ws::MarketMessage::Book(
            polyoxide_polymarket::clob::ws::BookMessage {
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
