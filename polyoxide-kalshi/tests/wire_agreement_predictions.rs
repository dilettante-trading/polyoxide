//! Agreement between the types and payloads captured from production. Provenance is in
//! `tests/fixtures/predictions/PROVENANCE.md`.
//!
//! Each fixture is decoded into its type and encoded back, and:
//!
//! 1. **Nothing unmodelled.** Every key path the host sent is emitted by the type.
//! 2. **Nothing invented.** Every key path the type emits was sent by the host, but for the
//!    paths of `SOMETIMES_ABSENT`.
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a price keeps every
//!    digit it was sent with.

use polyoxide_kalshi::predictions::types::{
    BatchCandlesticksResponse, MarketCandlestick, SeriesCandlesticksResponse,
};
use polyoxide_kalshi::predictions::{
    request::DEFAULT_COST,
    types::{
        BookSide, EndpointCosts, EventResponse, ExchangeStatus, MarketResponse, MarketStatus,
        MarketsPage, OrderbookResponse, OutcomeSide, TradesPage,
    },
};
use polyoxide_test_support::{
    agreement::{self, Ledger},
    fixtures,
};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};

/// `(fixture, path, reason)` rows for what the host sends in some responses and not others, which a
/// type that models them may emit as `null` where a capture lacks them. A row no fixture needs
/// fails `every_rest_fixture_agrees_with_its_type`.
const SOMETIMES_ABSENT: &[(&str, &str, &str)] = &[(
    "exchange_status",
    "/exchange_estimated_resume_time",
    "the host sends it only while the exchange is down",
)];

fn read<T: DeserializeOwned>(fixture: &str) -> T {
    serde_json::from_str(&fixtures!("predictions").text(fixture)).unwrap()
}

fn hold_to_fixture<T: DeserializeOwned + Serialize>(
    fixture: &str,
    used: &mut Ledger<'static, (&'static str, &'static str, &'static str)>,
) {
    let text = fixtures!("predictions").text(fixture);
    let diff = agreement::compare::<T>(fixture, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{fixture}: the host sent {:?}, which the type does not model",
        diff.unmodelled
    );
    let invented: Vec<&String> = diff
        .invented
        .iter()
        .filter(|path| used.excuse(SOMETIMES_ABSENT, fixture, path).is_none())
        .collect();
    assert!(
        invented.is_empty(),
        "{fixture}: the type emits {invented:?}, which the host did not send"
    );
}

#[test]
fn every_rest_fixture_agrees_with_its_type() {
    let mut used = Ledger::new(&[SOMETIMES_ABSENT]);
    hold_to_fixture::<ExchangeStatus>("exchange_status", &mut used);
    hold_to_fixture::<EndpointCosts>("endpoint_costs", &mut used);
    hold_to_fixture::<MarketResponse>("market", &mut used);
    hold_to_fixture::<MarketsPage>("markets_page", &mut used);
    hold_to_fixture::<OrderbookResponse>("orderbook", &mut used);
    hold_to_fixture::<OrderbookResponse>("orderbook_depth1", &mut used);
    hold_to_fixture::<TradesPage>("trades", &mut used);
    hold_to_fixture::<EventResponse>("event", &mut used);
    let stale: Vec<_> = used.stale().into_iter().map(|(f, p, _)| (f, p)).collect();
    assert!(
        stale.is_empty(),
        "SOMETIMES_ABSENT rows no fixture needs: {stale:?}"
    );
}

#[test]
fn the_fixtures_cover_the_cases_the_types_exist_for() {
    let market = read::<MarketResponse>("market").market;
    assert!(
        market.yes_bid_dollars > Decimal::ZERO && market.yes_ask_dollars < Decimal::ONE,
        "market.json is a market with a quote on both sides"
    );
    assert!(!market.price_ranges.is_empty());

    let page: MarketsPage = read("markets_page");
    assert_eq!(page.markets.len(), 5);
    assert!(!page.cursor.is_empty(), "a page with more behind it");

    let book = read::<OrderbookResponse>("orderbook").orderbook_fp;
    assert!(!book.yes_dollars.is_empty() && !book.no_dollars.is_empty());
    let shallow = read::<OrderbookResponse>("orderbook_depth1").orderbook_fp;
    assert!(shallow.yes_dollars.len() <= 1 && shallow.no_dollars.len() <= 1);

    let trades: TradesPage = read("trades");
    assert!(trades.trades.len() > 1, "trades.json holds several trades");
    assert!(
        trades.trades.iter().all(|t| t.ticker == market.ticker),
        "trades.json is the market's"
    );

    let event: EventResponse = read("event");
    assert!(
        event
            .event
            .markets
            .iter()
            .any(|m| m.ticker == market.ticker),
        "event.json nests the market"
    );

    // An unknown value would land in `Other` and decode fine; failing here is how a value
    // Kalshi adds gets modelled instead of passing silently.
    let unnamed: Vec<&MarketStatus> = page
        .markets
        .iter()
        .chain(&event.event.markets)
        .chain(std::iter::once(&market))
        .map(|m| &m.status)
        .filter(|s| matches!(s, MarketStatus::Other(_)))
        .collect();
    assert!(
        unnamed.is_empty(),
        "market states the enum does not name: {unnamed:?}"
    );
    let unnamed: Vec<_> = trades
        .trades
        .iter()
        .filter(|t| {
            matches!(t.taker_outcome_side, OutcomeSide::Other(_))
                || matches!(t.taker_book_side, BookSide::Other(_))
                || matches!(t.taker_side, Some(OutcomeSide::Other(_)))
        })
        .map(|t| (&t.taker_outcome_side, &t.taker_book_side))
        .collect();
    assert!(
        unnamed.is_empty(),
        "trade sides the enums do not name: {unnamed:?}"
    );
}

#[test]
fn the_endpoint_costs_default_is_the_compiled_default() {
    let costs: EndpointCosts = read("endpoint_costs");
    assert_eq!(costs.default_cost, DEFAULT_COST);
    assert!(!costs.endpoint_costs.is_empty());
}

// ── candlesticks ────────────────────────────────────────────────

/// The batch captures by width: the file, and the number of seconds every candle's end is a
/// multiple of (a minute for 1-minute candles, an hour for the hourly ones, and for the daily
/// ones too, since the daily candle ends at 04:00 UTC).
const BATCH_FIXTURES: &[(&str, i64)] =
    &[("batch_1", 60), ("batch_60", 3_600), ("batch_1440", 3_600)];

fn hold_candles_to_fixture<T: DeserializeOwned + Serialize>(fixture: &str) {
    let name = format!("candlesticks/{fixture}");
    let text = fixtures!("predictions").text(&name);
    let diff = agreement::compare::<T>(&name, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{name}: the host sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{name}: the type emits {:?}, which the host did not send",
        diff.invented
    );
}

#[test]
fn every_candlestick_fixture_agrees_with_its_type() {
    for (fixture, _) in BATCH_FIXTURES {
        hold_candles_to_fixture::<BatchCandlesticksResponse>(fixture);
    }
    hold_candles_to_fixture::<SeriesCandlesticksResponse>("series_60");
}

/// A traded period and an untraded one that carries `previous_dollars`: the two cases the
/// mapping to candles is built on.
fn candles_cover(what: &str, rows: &[MarketCandlestick]) {
    assert!(
        rows.iter().any(|row| row.price.close_dollars.is_some()),
        "{what}: no traded period"
    );
    assert!(
        rows.iter()
            .any(|row| row.price.close_dollars.is_none() && row.price.previous_dollars.is_some()),
        "{what}: no untraded period that carries previous_dollars"
    );
}

#[test]
fn the_candlestick_fixtures_cover_the_cases_the_types_exist_for() {
    for (fixture, multiple) in BATCH_FIXTURES {
        let batch: BatchCandlesticksResponse = read(&format!("candlesticks/{fixture}"));
        assert_eq!(
            batch.markets.len(),
            1,
            "{fixture}: one market was asked for"
        );
        let rows = &batch.markets[0].candlesticks;
        candles_cover(fixture, rows);
        for row in rows {
            assert_eq!(
                row.end_period_ts % multiple,
                0,
                "{fixture}: a candle ends off its boundary"
            );
        }
    }

    // The series route answers the same rows as the batch route for the same market. The latest
    // shared period may still have been open at one capture and not at the other, so it is the
    // one left out of the comparison.
    let series: SeriesCandlesticksResponse = read("candlesticks/series_60");
    candles_cover("series_60", &series.candlesticks);
    let hourly: BatchCandlesticksResponse = read("candlesticks/batch_60");
    assert_eq!(series.ticker, hourly.markets[0].market_ticker);
    let mut shared: Vec<(&MarketCandlestick, &MarketCandlestick)> = series
        .candlesticks
        .iter()
        .filter_map(|row| {
            hourly.markets[0]
                .candlesticks
                .iter()
                .find(|other| other.end_period_ts == row.end_period_ts)
                .map(|other| (row, other))
        })
        .collect();
    assert!(!shared.is_empty(), "series_60 and batch_60 share no period");
    shared.sort_by_key(|(row, _)| row.end_period_ts);
    shared.pop();
    for (from_series, from_batch) in shared {
        assert_eq!(from_series, from_batch);
    }
}

// ── Catalog routes (issue #71) ──────────────────────────────────

use polyoxide_kalshi::predictions::types::OrderbooksResponse;

/// Holds `T` to the capture `name` with nothing excused: every key the host sent is modelled, and
/// every key the type emits was sent.
fn hold_exactly<T: DeserializeOwned + Serialize>(name: &str) {
    let text = fixtures!("predictions").text(name);
    let diff = agreement::compare::<T>(name, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{name}: the host sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{name}: the type emits {:?}, which the host did not send",
        diff.invented
    );
}

#[test]
fn every_markets_catalog_fixture_agrees_with_its_type() {
    for name in [
        "catalog/markets_updated_page",
        "catalog/markets_tickers_page",
        "catalog/markets_window_settled_page",
    ] {
        hold_exactly::<MarketsPage>(name);
    }
    hold_exactly::<OrderbooksResponse>("catalog/orderbooks");
}

#[test]
fn the_markets_catalog_fixtures_cover_the_cases_the_routes_exist_for() {
    let books: OrderbooksResponse = read("catalog/orderbooks");
    assert_eq!(
        books.orderbooks.len(),
        3,
        "two active tickers and an unknown one"
    );
    assert!(
        books
            .orderbooks
            .iter()
            .any(|book| book.orderbook_fp.yes_dollars.is_empty()
                && book.orderbook_fp.no_dollars.is_empty()),
        "the unknown ticker's book is empty"
    );
    let settled: MarketsPage = read("catalog/markets_window_settled_page");
    assert!(!settled.markets.is_empty(), "a week held settled markets");
    assert!(
        settled
            .markets
            .iter()
            .all(|market| market.settlement_ts.is_some()),
        "a settled market carries its settlement time"
    );
    let named: MarketsPage = read("catalog/markets_tickers_page");
    assert_eq!(named.markets.len(), 2, "the two tickers asked for");
    let updated: MarketsPage = read("catalog/markets_updated_page");
    assert!(!updated.markets.is_empty(), "a day held updated markets");
}

use polyoxide_kalshi::predictions::types::{SeriesList, SeriesResponse};

#[test]
fn every_series_fixture_agrees_with_its_type() {
    hold_exactly::<SeriesList>("catalog/series_list");
    hold_exactly::<SeriesResponse>("catalog/series");
}

#[test]
fn the_series_fixtures_cover_the_cases_the_types_exist_for() {
    let list: SeriesList = read("catalog/series_list");
    assert!(!list.series.is_empty());
    let one: SeriesResponse = read("catalog/series");
    assert!(
        one.series.volume_fp.is_some(),
        "series.json was asked for its volume"
    );
}

use polyoxide_kalshi::predictions::types::EventsPage;

#[test]
fn every_events_listing_fixture_agrees_with_its_type() {
    hold_exactly::<EventsPage>("catalog/events_page");
    hold_exactly::<EventsPage>("catalog/events_page_nested");
}

#[test]
fn the_events_listing_fixtures_cover_the_cases_the_type_exists_for() {
    let flat: EventsPage = read("catalog/events_page");
    assert!(!flat.events.is_empty());
    assert!(!flat.cursor.is_empty(), "a page with more behind it");
    let nested: EventsPage = read("catalog/events_page_nested");
    assert!(
        nested.events.iter().any(|event| !event.markets.is_empty()),
        "a nested page nests markets"
    );
}

use polyoxide_kalshi::predictions::types::{HistoricalCandlesticksResponse, HistoricalCutoff};

#[test]
fn every_historical_fixture_agrees_with_its_type() {
    hold_exactly::<HistoricalCutoff>("historical/cutoff");
    hold_exactly::<MarketsPage>("historical/markets_page");
    hold_exactly::<MarketResponse>("historical/market");
    for width in [1, 60, 1440] {
        hold_exactly::<HistoricalCandlesticksResponse>(&format!("historical/candlesticks_{width}"));
    }
    hold_exactly::<TradesPage>("historical/trades");
}

#[test]
fn the_historical_fixtures_cover_the_cases_the_fallback_is_built_on() {
    let hourly: HistoricalCandlesticksResponse = read("historical/candlesticks_60");
    assert!(
        hourly
            .candlesticks
            .iter()
            .any(|row| row.price.close.is_some()),
        "an hourly period with a trade"
    );
    assert!(
        hourly
            .candlesticks
            .iter()
            .any(|row| row.price.close.is_none() && row.price.previous.is_some()),
        "an hourly period with no trade that carries previous"
    );
    let market: MarketResponse = read("historical/market");
    assert_eq!(hourly.ticker, market.market.ticker);
    let trades: TradesPage = read("historical/trades");
    assert!(!trades.trades.is_empty(), "the archived market traded");
    assert!(trades
        .trades
        .iter()
        .all(|trade| trade.ticker == market.market.ticker));
}
