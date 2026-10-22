//! Kalshi's venue traits for a market archived past the historical cutoff (issue #71, D13): each
//! reads live first and the archive only when the live answer says the market is not there, on
//! mock servers serving the captured archive.

use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::{
    key::{self, Outcome, Ticker},
    types::HistoricalCandlesticksResponse,
    venue::InstrumentExt,
    Predictions,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, ClassifiedError, DynCandles, DynEventGroups,
    DynMarketData, DynTrades, EventGroups, EventOutcome, MarketData, MarketKey, NotListed, TradeId,
    Trades, UnixMillis,
};
use serde_json::{json, Value};

const NOT_FOUND: &str = r#"{"error":{"code":"not_found","message":"not found"}}"#;
const UNAVAILABLE: &str = r#"{"error":{"code":"service_unavailable","message":"try later"}}"#;
const CANDLES: &str = "/trade-api/v2/markets/candlesticks";
const TRADES: &str = "/trade-api/v2/markets/trades";
const ARCHIVED_TRADES: &str = "/trade-api/v2/historical/trades";
const ARCHIVED_MARKETS: &str = "/trade-api/v2/historical/markets";

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn fixture_json(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap()
}

/// A server with the account's `endpoint_costs` mounted and never asserted, which a client reads
/// on its first send.
async fn start() -> ServerGuard {
    let mut server = Server::new_async().await;
    let _costs = server
        .mock("GET", "/trade-api/v2/account/endpoint_costs")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fixture("endpoint_costs"))
        .create_async()
        .await;
    server
}

/// `path` answering `status` with `body` `hits` times, whatever its query.
async fn answer(
    server: &mut ServerGuard,
    path: &str,
    status: usize,
    body: &str,
    hits: usize,
) -> Mock {
    server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

/// `path` answering 200 with `body` once, for exactly `query`.
async fn exact(server: &mut ServerGuard, path: &str, query: String, body: &str) -> Mock {
    server
        .mock("GET", path)
        .match_query(Matcher::Exact(query))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

/// The archived market the capture holds: its ticker and its row.
fn archived() -> (Ticker, Value) {
    let body = fixture_json("historical/market");
    let ticker = Ticker::new(body["market"]["ticker"].as_str().unwrap()).unwrap();
    (ticker, body["market"].clone())
}

/// The live market the capture holds.
fn live() -> Ticker {
    Ticker::new(fixture_json("market")["market"]["ticker"].as_str().unwrap()).unwrap()
}

fn market_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/markets/{ticker}")
}

fn archived_market_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/historical/markets/{ticker}")
}

fn archived_candles_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/historical/markets/{ticker}/candlesticks")
}

fn ms(seconds: i64) -> u64 {
    u64::try_from(seconds).unwrap() * 1000
}

fn assert_not_listed(err: &ClassifiedError, key: &MarketKey) {
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(err.source.downcast_ref::<NotListed>().unwrap().key, *key);
}

// ── instrument ──────────────────────────────────────────────────

#[tokio::test]
async fn an_archived_market_is_read_from_the_archive_when_the_live_route_answers_404() {
    let (ticker, row) = archived();
    let mut server = start().await;
    let live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let archive = answer(
        &mut server,
        &archived_market_path(&ticker),
        200,
        &fixture("historical/market"),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let instrument = venue
        .instrument(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    live.assert_async().await;
    archive.assert_async().await;
    assert_eq!(
        instrument.tradable,
        Some(false),
        "an archived market does not trade"
    );
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.event_ticker.as_deref(), row["event_ticker"].as_str());
}

#[tokio::test]
async fn a_ticker_neither_route_lists_is_not_listed() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let archive = answer(
        &mut server,
        &archived_market_path(&ticker),
        404,
        NOT_FOUND,
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let key = key::key(&ticker, Outcome::No);
    let err = venue.instrument(&key).await.unwrap_err();
    archive.assert_async().await;
    assert_not_listed(&err, &key);
}

#[tokio::test]
async fn an_archive_that_fails_otherwise_is_that_failure() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let _archive = answer(
        &mut server,
        &archived_market_path(&ticker),
        503,
        UNAVAILABLE,
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue
        .instrument(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap_err();
    assert!(matches!(err.class, Class::Unavailable { .. }), "{err}");
    assert!(err.source.downcast_ref::<NotListed>().is_none(), "{err}");
}

#[tokio::test]
async fn a_live_market_never_asks_the_archive() {
    let ticker = live();
    let mut server = start().await;
    let _live = answer(
        &mut server,
        &market_path(&ticker),
        200,
        &fixture("market"),
        1,
    )
    .await;
    let archive = answer(
        &mut server,
        &archived_market_path(&ticker),
        404,
        NOT_FOUND,
        0,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    venue
        .instrument(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    archive.assert_async().await;
}

// ── candles ─────────────────────────────────────────────────────

/// The captured hourly archive, and the span of open times that covers every row: the first row
/// opens an hour before it ends, and the last row opens an hour before it ends.
fn hourly() -> (String, HistoricalCandlesticksResponse, i64, i64) {
    let body = fixture("historical/candlesticks_60");
    let rows: HistoricalCandlesticksResponse = serde_json::from_str(&body).unwrap();
    let first = rows
        .candlesticks
        .iter()
        .map(|row| row.end_period_ts)
        .min()
        .unwrap();
    let last = rows
        .candlesticks
        .iter()
        .map(|row| row.end_period_ts)
        .max()
        .unwrap();
    (body, rows, first - 3_600, last - 3_600)
}

#[tokio::test]
async fn a_market_the_live_answer_leaves_out_is_read_from_the_archive_with_the_same_span() {
    let (ticker, _) = archived();
    let (body, rows, first_open, last_open) = hourly();
    // Kalshi selects a candle by its end and the trait by its open, so both requests ask for the
    // span moved on by one width.
    let (start_ts, end_ts) = (first_open + 3_600, last_open + 3_600);
    let mut server = start().await;
    let live = exact(
        &mut server,
        CANDLES,
        format!("market_tickers={ticker}&start_ts={start_ts}&end_ts={end_ts}&period_interval=60"),
        r#"{"markets":[]}"#,
    )
    .await;
    let archive = exact(
        &mut server,
        &archived_candles_path(&ticker),
        format!("start_ts={start_ts}&end_ts={end_ts}&period_interval=60"),
        &body,
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(ms(first_open)))
        .end(UnixMillis(ms(last_open)));
    let candles = venue
        .candles(&key::key(&ticker, Outcome::Yes), &query)
        .await
        .unwrap();
    live.assert_async().await;
    archive.assert_async().await;

    // A row with a trade or a previous price is a candle opening one hour before it ends.
    let mut kept: Vec<_> = rows
        .candlesticks
        .iter()
        .filter(|row| row.price.close.is_some() || row.price.previous.is_some())
        .collect();
    kept.sort_by_key(|row| row.end_period_ts);
    assert!(!kept.is_empty(), "the capture holds a candle");
    let opens: Vec<u64> = candles.iter().map(|candle| candle.open_time.0).collect();
    let expected: Vec<u64> = kept
        .iter()
        .map(|row| ms(row.end_period_ts - 3_600))
        .collect();
    assert_eq!(opens, expected);
    for (candle, row) in candles.iter().zip(&kept) {
        assert_eq!(
            candle.close,
            row.price.close.or(row.price.previous).unwrap()
        );
        assert_eq!(candle.volume, Some(row.volume));
        assert_eq!(candle.open.is_some(), row.price.close.is_some());
    }
}

#[tokio::test]
async fn an_archived_span_past_5_000_periods_is_clamped_to_its_first_5_000() {
    let (ticker, _) = archived();
    // 9,001 one-minute periods: within the live route's 10,000, past the archive's 5,000.
    let first_open: i64 = 1_799_971_200;
    let last_open = first_open + 9_000 * 60;
    let start_ts = first_open + 60;
    let mut server = start().await;
    let live = exact(
        &mut server,
        CANDLES,
        format!(
            "market_tickers={ticker}&start_ts={start_ts}&end_ts={}&period_interval=1",
            last_open + 60
        ),
        r#"{"markets":[]}"#,
    )
    .await;
    let archive = exact(
        &mut server,
        &archived_candles_path(&ticker),
        format!(
            "start_ts={start_ts}&end_ts={}&period_interval=1",
            start_ts + 4_999 * 60
        ),
        &json!({"ticker": ticker.as_str(), "candlesticks": []}).to_string(),
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let query = CandleQuery::new(CandleInterval::Minute1, UnixMillis(ms(first_open)))
        .end(UnixMillis(ms(last_open)));
    let candles = venue
        .candles(&key::key(&ticker, Outcome::Yes), &query)
        .await
        .unwrap();
    live.assert_async().await;
    archive.assert_async().await;
    assert!(candles.is_empty());
}

#[tokio::test]
async fn an_answer_that_lists_only_another_market_asks_the_archive_and_its_404_is_not_listed() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let mut server = start().await;
    let other =
        json!({"markets": [{"market_ticker": "KXOTHER-1", "candlesticks": []}]}).to_string();
    let _live = answer(&mut server, CANDLES, 200, &other, 1).await;
    let archive = answer(
        &mut server,
        &archived_candles_path(&ticker),
        404,
        NOT_FOUND,
        1,
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let key = key::key(&ticker, Outcome::Yes);
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(0)).end(UnixMillis(ms(3_600)));
    let err = venue.candles(&key, &query).await.unwrap_err();
    archive.assert_async().await;
    assert_not_listed(&err, &key);
}

#[tokio::test]
async fn a_listed_market_with_no_candle_in_the_span_never_asks_the_archive() {
    let ticker = live();
    let mut server = start().await;
    let empty =
        json!({"markets": [{"market_ticker": ticker.as_str(), "candlesticks": []}]}).to_string();
    let _live = answer(&mut server, CANDLES, 200, &empty, 1).await;
    let archive = answer(
        &mut server,
        &archived_candles_path(&ticker),
        404,
        NOT_FOUND,
        0,
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(0)).end(UnixMillis(ms(3_600)));
    let candles = venue
        .candles(&key::key(&ticker, Outcome::Yes), &query)
        .await
        .unwrap();
    assert!(candles.is_empty());
    archive.assert_async().await;
}

// ── trades ──────────────────────────────────────────────────────

const NO_TRADES: &str = r#"{"cursor":"","trades":[]}"#;

#[tokio::test]
async fn trades_the_live_route_does_not_have_are_read_from_the_archive_oldest_first() {
    let (ticker, _) = archived();
    let mut server = start().await;
    let live = exact(
        &mut server,
        TRADES,
        format!("ticker={ticker}&limit=1000"),
        NO_TRADES,
    )
    .await;
    let archive = exact(
        &mut server,
        ARCHIVED_TRADES,
        format!("ticker={ticker}&limit=1000"),
        &fixture("historical/trades"),
    )
    .await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let trades = venue
        .trades(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    live.assert_async().await;
    archive.assert_async().await;

    let newest_first: Vec<TradeId> = fixture_json("historical/trades")["trades"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| TradeId::new(row["trade_id"].as_str().unwrap()))
        .collect();
    let ids: Vec<TradeId> = trades.iter().filter_map(|trade| trade.id.clone()).collect();
    assert_eq!(ids, newest_first.into_iter().rev().collect::<Vec<_>>());
}

#[tokio::test]
async fn a_market_with_live_trades_never_asks_the_archive() {
    let ticker = live();
    let mut server = start().await;
    let _live = answer(&mut server, TRADES, 200, &fixture("trades"), 1).await;
    let archive = answer(&mut server, ARCHIVED_TRADES, 200, NO_TRADES, 0).await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let trades = venue
        .trades(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    assert!(!trades.is_empty());
    archive.assert_async().await;
}

#[tokio::test]
async fn an_archive_with_no_trades_is_no_trades_and_its_404_is_not_listed() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let key = key::key(&ticker, Outcome::Yes);

    let mut server = start().await;
    let _live = answer(&mut server, TRADES, 200, NO_TRADES, 1).await;
    let _archive = answer(&mut server, ARCHIVED_TRADES, 200, NO_TRADES, 1).await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    assert!(venue.trades(&key).await.unwrap().is_empty());

    let mut server = start().await;
    let _live = answer(&mut server, TRADES, 200, NO_TRADES, 1).await;
    let _archive = answer(&mut server, ARCHIVED_TRADES, 404, NOT_FOUND, 1).await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let err = venue.trades(&key).await.unwrap_err();
    assert_not_listed(&err, &key);
}

// ── event groups ────────────────────────────────────────────────

/// The captured event, renamed `event_ticker`, nesting `markets`.
fn event_body(event_ticker: &str, markets: Vec<Value>) -> String {
    let mut body = fixture_json("event");
    body["event"]["event_ticker"] = json!(event_ticker);
    body["event"]["markets"] = json!(markets);
    body.to_string()
}

/// The archived row, renamed `ticker`: a sibling market of the same event.
fn sibling(row: &Value, ticker: &str) -> Value {
    let mut sibling = row.clone();
    sibling["ticker"] = json!(ticker);
    sibling
}

/// The tickers of a group's outcomes, one per market, in order.
fn group_tickers(outcomes: &[EventOutcome]) -> Vec<String> {
    outcomes
        .chunks(2)
        .map(|pair| pair[0].market.clone())
        .collect()
}

#[tokio::test]
async fn an_archived_markets_group_is_read_from_the_archive() {
    let (ticker, row) = archived();
    let event = row["event_ticker"].as_str().unwrap().to_owned();
    let other = format!("{ticker}-B");
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let _archive = answer(
        &mut server,
        &archived_market_path(&ticker),
        200,
        &fixture("historical/market"),
        1,
    )
    .await;
    let _event = answer(
        &mut server,
        &format!("/trade-api/v2/events/{event}"),
        200,
        &event_body(&event, vec![]),
        1,
    )
    .await;
    let siblings = exact(
        &mut server,
        ARCHIVED_MARKETS,
        format!("event_ticker={event}&limit=1000"),
        &json!({"cursor": "", "markets": [row.clone(), sibling(&row, &other)]}).to_string(),
    )
    .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue
        .event_group(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    siblings.assert_async().await;
    assert_eq!(group.id, event);
    assert_eq!(
        group_tickers(&group.outcomes),
        [ticker.as_str().to_owned(), other]
    );
}

#[tokio::test]
async fn a_partly_archived_event_lists_each_market_once_live_ones_first() {
    let (ticker, row) = archived();
    let event = row["event_ticker"].as_str().unwrap().to_owned();
    let other = format!("{ticker}-B");
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let _archive = answer(
        &mut server,
        &archived_market_path(&ticker),
        200,
        &fixture("historical/market"),
        1,
    )
    .await;
    let _event = answer(
        &mut server,
        &format!("/trade-api/v2/events/{event}"),
        200,
        &event_body(&event, vec![sibling(&row, &other)]),
        1,
    )
    .await;
    let _siblings = answer(
        &mut server,
        ARCHIVED_MARKETS,
        200,
        &json!({"cursor": "", "markets": [row.clone(), sibling(&row, &other)]}).to_string(),
        1,
    )
    .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue
        .event_group(&key::key(&ticker, Outcome::No))
        .await
        .unwrap();
    assert_eq!(
        group_tickers(&group.outcomes),
        [other, ticker.as_str().to_owned()]
    );
}

#[tokio::test]
async fn an_event_that_nests_the_market_never_asks_the_archive() {
    let ticker = live();
    let market = fixture_json("market");
    let event = market["market"]["event_ticker"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut server = start().await;
    let _live = answer(
        &mut server,
        &market_path(&ticker),
        200,
        &fixture("market"),
        1,
    )
    .await;
    let _event = answer(
        &mut server,
        &format!("/trade-api/v2/events/{event}"),
        200,
        &event_body(&event, vec![market["market"].clone()]),
        1,
    )
    .await;
    let archive = answer(
        &mut server,
        ARCHIVED_MARKETS,
        200,
        r#"{"cursor":"","markets":[]}"#,
        0,
    )
    .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue
        .event_group(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    assert_eq!(group_tickers(&group.outcomes), [ticker.as_str().to_owned()]);
    archive.assert_async().await;
}
