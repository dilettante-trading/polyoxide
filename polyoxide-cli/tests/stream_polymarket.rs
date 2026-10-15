//! `polyoxide polymarket clob stream market`, `… clob stream user` and
//! `… rtds stream` over scripted event streams, so a flag that parses but
//! never reaches the output fails here. Sports has its own suite,
//! `ws_sports.rs`, and the runner they share is tested in `commands::stream`.

use std::time::Duration;

use clap::Parser;
use futures_util::{stream, Stream, StreamExt};
use polyoxide_cli::commands::polymarket::{
    clob::stream::{market, user},
    rtds,
};
use polyoxide_polymarket::{
    clob::ws::{
        BookMessage, Channel, ClobWsError, MarketMessage, OrderEventType, OrderMessage,
        PriceChange, PriceChangeMessage, TradeMessage, TradeStatus, UserMessage,
    },
    rtds::{fixtures, PriceEvent, RtdsError},
};
use serde_json::Value;

/// How long a run may take before a test calls it hung. A feed that goes quiet
/// never ends on its own, so a regression must fail here instead of blocking.
const HANG: Duration = Duration::from_secs(2);

/// What one run wrote, and how it ended.
struct Run {
    result: color_eyre::Result<()>,
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
}

// ── market ───────────────────────────────────────────────────────────

#[derive(Parser)]
struct MarketCli {
    #[command(flatten)]
    args: market::MarketArgs,
}

type ClobItem = Result<Channel, ClobWsError>;

fn book(asset_id: &str) -> ClobItem {
    Ok(Channel::Market(MarketMessage::Book(BookMessage {
        event_type: "book".into(),
        asset_id: asset_id.into(),
        market: "0xcondition".into(),
        timestamp: "1700000000000".into(),
        hash: "0xhash".into(),
        bids: vec![],
        asks: vec![],
        last_trade_price: None,
    })))
}

fn price_change(asset_ids: &[&str]) -> ClobItem {
    let price_changes = asset_ids
        .iter()
        .map(|asset_id| PriceChange {
            asset_id: (*asset_id).into(),
            price: "0.55".parse().unwrap(),
            size: "10".parse().unwrap(),
            side: "BUY".into(),
            hash: "0xhash".into(),
            best_bid: None,
            best_ask: None,
        })
        .collect();
    Ok(Channel::Market(MarketMessage::PriceChange(
        PriceChangeMessage {
            event_type: "price_change".into(),
            market: "0xcondition".into(),
            price_changes,
            timestamp: "1700000000000".into(),
        },
    )))
}

/// A frame that did not parse, which the connection reports and carries on
/// after.
fn bad_frame() -> ClobWsError {
    ClobWsError::Json(serde_json::from_str::<Value>("not json").unwrap_err())
}

async fn run_market<S>(argv: &[&str], events: S) -> Run
where
    S: Stream<Item = ClobItem> + Unpin,
{
    let mut full = vec!["market", "asset-1"];
    full.extend_from_slice(argv);
    let cli = MarketCli::try_parse_from(full).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let result = tokio::time::timeout(HANG, market::run_with(cli.args, events, &mut out, &mut err))
        .await
        .expect("the run ended");
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

#[tokio::test]
async fn market_count_counts_only_the_events_the_filter_keeps() {
    // The feed stays open after the second book, so only `-n` can end this.
    let events = stream::iter(vec![
        price_change(&["asset-1"]),
        book("asset-1"),
        book("asset-2"),
    ])
    .chain(stream::pending());
    let run = run_market(&["-n", "2", "--filter", "book"], events).await;
    run.result.as_ref().unwrap();
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 2, "{}", run.out);
    assert!(
        lines.iter().all(|line| line.starts_with("[BOOK]")),
        "{}",
        run.out
    );
    assert!(run.err.contains("Reached 2 update(s)"), "{}", run.err);
}

#[tokio::test]
async fn market_filters_split_on_commas_and_reach_the_output() {
    let events = vec![
        book("asset-1"),
        price_change(&["asset-1"]),
        Ok(Channel::Market(MarketMessage::TickSizeChange(
            serde_json::from_value(serde_json::json!({
                "event_type": "tick_size_change",
                "asset_id": "asset-1",
                "market": "0xcondition",
                "old_tick_size": "0.01",
                "new_tick_size": "0.001",
                "side": "BUY",
                "timestamp": "1700000000000"
            }))
            .unwrap(),
        ))),
    ];
    let run = run_market(
        &["--filter", "book,price", "--format", "json"],
        stream::iter(events),
    )
    .await;
    run.result.as_ref().unwrap();
    let kinds: Vec<String> = run
        .rows()
        .iter()
        .map(|row| row["event_type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["book", "price_change"]);
    assert!(run.err.contains("The feed ended"), "{}", run.err);
}

#[tokio::test]
async fn market_pretty_prints_a_line_per_event_and_per_price_change() {
    let run = run_market(
        &[],
        stream::iter(vec![book("asset-1"), price_change(&["asset-1", "asset-2"])]),
    )
    .await;
    run.result.as_ref().unwrap();
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 3, "{}", run.out);
    assert!(lines[0].starts_with("[BOOK] asset=asset-1"), "{}", run.out);
    assert!(lines[1].starts_with("[PRICE] asset=asset-1"), "{}", run.out);
    assert!(lines[2].starts_with("[PRICE] asset=asset-2"), "{}", run.out);
}

#[tokio::test]
async fn market_skips_a_bad_frame_and_ends_with_any_other_error() {
    let run = run_market(
        &["--format", "json"],
        stream::iter(vec![Err(bad_frame()), book("asset-1")]),
    )
    .await;
    run.result.as_ref().unwrap();
    assert_eq!(run.rows().len(), 1);
    assert!(run.err.contains("# skipped a frame"), "{}", run.err);

    // It used to print `Error:` and exit 0.
    let run = run_market(
        &[],
        stream::iter(vec![Err(ClobWsError::ConnectionClosed), book("asset-1")]),
    )
    .await;
    let error = run.result.unwrap_err();
    assert!(
        error.downcast_ref::<ClobWsError>().is_some(),
        "the report does not carry the stream's error: {error:?}"
    );
    assert!(run.out.is_empty(), "{}", run.out);
}

#[tokio::test]
async fn market_warns_of_a_gated_filter_without_custom_features() {
    let run = run_market(&["--filter", "best-bid-ask"], stream::iter(vec![])).await;
    run.result.as_ref().unwrap();
    assert!(run.err.contains("--custom-features"), "{}", run.err);

    let run = run_market(
        &["--filter", "best-bid-ask", "--custom-features"],
        stream::iter(vec![]),
    )
    .await;
    run.result.as_ref().unwrap();
    assert!(!run.err.contains("warning"), "{}", run.err);
}

// ── user ─────────────────────────────────────────────────────────────

#[derive(Parser)]
struct UserCli {
    #[command(flatten)]
    args: user::UserArgs,
}

fn order(id: &str) -> ClobItem {
    Ok(Channel::User(UserMessage::Order(OrderMessage {
        event_type: "order".into(),
        id: id.into(),
        asset_id: "asset-1".into(),
        market: "0xcondition".into(),
        outcome: "Yes".into(),
        price: "0.5".into(),
        side: "BUY".into(),
        original_size: "10".into(),
        size_matched: "0".into(),
        order_type: OrderEventType::Placement,
        order_owner: Some("0x123".into()),
        timestamp: "123".into(),
    })))
}

fn trade(id: &str) -> ClobItem {
    Ok(Channel::User(UserMessage::Trade(TradeMessage {
        event_type: "trade".into(),
        id: id.into(),
        asset_id: "asset-1".into(),
        market: "0xcondition".into(),
        outcome: "Yes".into(),
        price: "0.6".into(),
        size: "5".into(),
        side: "BUY".into(),
        status: TradeStatus::Confirmed,
        taker_order_id: "order-1".into(),
        maker_orders: vec![],
        owner: Some("0x123".into()),
        transaction_hash: None,
        timestamp: "456".into(),
    })))
}

async fn run_user(argv: &[&str], events: Vec<ClobItem>) -> Run {
    let mut full = vec!["user", "0xcondition"];
    full.extend_from_slice(argv);
    let cli = UserCli::try_parse_from(full).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let events = stream::iter(events).chain(stream::pending());
    let result = tokio::time::timeout(HANG, user::run_with(cli.args, events, &mut out, &mut err))
        .await
        .expect("the run ended");
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

#[tokio::test]
async fn user_filter_and_count_reach_the_output() {
    let events = vec![order("order-1"), trade("trade-1"), trade("trade-2")];
    let run = run_user(
        &["--filter", "trade", "-n", "2", "--format", "json"],
        events,
    )
    .await;
    run.result.as_ref().unwrap();
    let ids: Vec<String> = run
        .rows()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["trade-1", "trade-2"]);
    assert!(run.err.contains("Reached 2 update(s)"), "{}", run.err);
}

#[tokio::test]
async fn user_pretty_prints_one_summary_line_per_event() {
    let run = run_user(
        &["-n", "2"],
        vec![order("order-12345678"), trade("trade-1")],
    )
    .await;
    run.result.as_ref().unwrap();
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 2, "{}", run.out);
    assert!(lines[0].starts_with("[ORDER] id=order-12 "), "{}", run.out);
    assert!(lines[1].starts_with("[TRADE] id=trade-1 "), "{}", run.out);
}

#[tokio::test]
async fn user_ends_with_an_error_it_cannot_skip() {
    let run = run_user(
        &[],
        vec![Err(ClobWsError::ConnectionClosed), order("order-1")],
    )
    .await;
    assert!(
        run.result.is_err(),
        "the run carried on past a closed connection"
    );
    assert!(run.out.is_empty(), "{}", run.out);
}

// ── rtds ─────────────────────────────────────────────────────────────

#[derive(Parser)]
struct PricesCli {
    #[command(flatten)]
    args: rtds::PricesArgs,
}

type PriceItem = Result<PriceEvent, RtdsError>;

fn frame(raw: &str) -> PriceItem {
    Ok(PriceEvent::from_json(raw).unwrap().unwrap())
}

async fn run_prices(argv: &[&str], events: Vec<PriceItem>) -> Run {
    let mut full = vec!["prices"];
    full.extend_from_slice(argv);
    let cli = PricesCli::try_parse_from(full).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let events = stream::iter(events).chain(stream::pending());
    let result = tokio::time::timeout(HANG, rtds::run_with(cli.args, events, &mut out, &mut err))
        .await
        .expect("the run ended");
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

#[tokio::test]
async fn prices_backfill_goes_to_stderr_and_count_counts_updates() {
    let events = vec![
        frame(fixtures::TWAP_THIRTY_SNAPSHOT),
        frame(fixtures::TWAP_THIRTY_UPDATE),
        frame(fixtures::TWAP_THIRTY_UPDATE),
    ];
    let run = run_prices(&["-n", "2", "--format", "json"], events).await;
    run.result.as_ref().unwrap();
    let rows = run.rows();
    assert_eq!(rows.len(), 2, "{}", run.out);
    for row in &rows {
        assert_eq!(row["symbol"], "btc/usd", "{row}");
        assert_eq!(row["window_s"], 30, "{row}");
        assert!(row["value"].is_string(), "{row}");
    }
    assert!(
        run.err.contains("# backfill: 3 point(s) for btc/usd"),
        "{}",
        run.err
    );
    assert!(run.err.contains("Reached 2 update(s)"), "{}", run.err);
}

#[tokio::test]
async fn prices_pretty_prints_symbol_value_and_time() {
    let run = run_prices(&["-n", "1"], vec![frame(fixtures::BINANCE_UPDATE)]).await;
    run.result.as_ref().unwrap();
    let words: Vec<&str> = run.out.split_whitespace().collect();
    assert_eq!(words.len(), 3, "{}", run.out);
    assert_eq!(words[0], "btcusdt");
    assert!(words[1].starts_with("79697.73"), "{}", run.out);
}

#[tokio::test]
async fn prices_skip_a_bad_frame_and_end_with_a_rejected_subscription() {
    let source = serde_json::from_str::<Value>("not json").unwrap_err();
    let events = vec![
        Err(RtdsError::Json {
            raw: "not json".into(),
            source,
        }),
        frame(fixtures::BINANCE_UPDATE),
    ];
    let run = run_prices(&["-n", "1"], events).await;
    run.result.as_ref().unwrap();
    assert_eq!(run.out.lines().count(), 1, "{}", run.out);
    assert!(
        run.err.contains("# skipped a frame") && run.err.contains("not json"),
        "{}",
        run.err
    );

    let events = vec![
        Err(RtdsError::Server {
            status_code: 401,
            message: "topic not found".into(),
        }),
        frame(fixtures::BINANCE_UPDATE),
    ];
    let run = run_prices(&[], events).await;
    let error = run.result.unwrap_err();
    assert!(error.to_string().contains("topic not found"), "{error}");
    assert!(run.out.is_empty(), "{}", run.out);
}
