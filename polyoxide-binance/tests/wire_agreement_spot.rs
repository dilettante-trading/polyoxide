//! Agreement between the spot types and payloads captured from the live host.
//! Provenance is in `tests/fixtures/spot/PROVENANCE.md`.
//!
//! Each fixture is decoded into its type and encoded back, and:
//!
//! 1. **Nothing unmodelled.** Every key path the server sent is emitted by the
//!    type.
//! 2. **Nothing invented.** Every key path the type emits was sent by the
//!    server. An `Option` encodes as `null`, so a field modelled but absent from
//!    the wire shows up here instead of hiding.
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a
//!    price with more digits than an `f64` keeps survives exactly or fails.
//!
//! A kline's twelfth element, which Binance documents as "ignore", is the one
//! thing dropped on purpose; positional arrays are compared by index only as
//! far as the shorter side goes.

use polyoxide_test_support::agreement as common;

use polyoxide_binance::spot::types::{
    AggTrade, AvgPrice, BookTicker, Depth, ExchangeInfo, Interval, Kline, PriceTicker, ServerTime,
    SymbolStatus, Ticker24h, Trade,
};
use polyoxide_test_support::fixtures;
use serde::{de::DeserializeOwned, Serialize};

fn check<T: DeserializeOwned + Serialize>(fixture: &str) {
    let text = fixtures!("spot/rest").text(fixture);
    let diff = common::compare::<T>(fixture, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{fixture}: the server sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{fixture}: the type emits {:?}, which the server did not send",
        diff.invented
    );
}

fn read<T: DeserializeOwned>(fixture: &str) -> T {
    serde_json::from_str(&fixtures!("spot/rest").text(fixture)).unwrap()
}

#[test]
fn every_rest_fixture_agrees_with_its_type() {
    check::<ServerTime>("time");
    check::<ExchangeInfo>("exchange_info");
    check::<Depth>("depth");
    check::<Vec<Trade>>("trades");
    check::<Vec<AggTrade>>("agg_trades");
    check::<Vec<Kline>>("klines");
    check::<Vec<Kline>>("klines_1s");
    check::<AvgPrice>("avg_price");
    check::<Ticker24h>("ticker_24hr");
    check::<Vec<Ticker24h>>("tickers_24hr");
    check::<PriceTicker>("price");
    check::<Vec<PriceTicker>>("prices");
    check::<BookTicker>("book_ticker");
    check::<Vec<BookTicker>>("book_tickers");
}

#[test]
fn the_fixtures_hold_what_the_types_are_for() {
    // The book is best first on both sides: bids fall, asks rise.
    let book: Depth = read("depth");
    assert!(!book.bids.is_empty() && !book.asks.is_empty());
    assert!(book.bids.windows(2).all(|w| w[0].price > w[1].price));
    assert!(book.asks.windows(2).all(|w| w[0].price < w[1].price));
    assert!(book.bids[0].price < book.asks[0].price, "a crossed book");

    // Trade ids rise, oldest first.
    let trades: Vec<Trade> = read("trades");
    assert!(trades.len() > 1);
    assert!(trades.windows(2).all(|w| w[0].id < w[1].id));
    let aggregates: Vec<AggTrade> = read("agg_trades");
    assert!(aggregates.len() > 1);
    assert!(aggregates.windows(2).all(|w| w[0].id < w[1].id));
    assert!(aggregates
        .iter()
        .all(|t| t.first_trade_id <= t.last_trade_id));

    // The candles are one interval apart: 60 s for `1m`, one second for `1s`,
    // which this host serves (the USDⓈ-M host refuses it).
    let minutes: Vec<Kline> = read("klines");
    assert!(minutes.len() > 1);
    assert_eq!(minutes[1].open_time - minutes[0].open_time, 60_000);
    let seconds: Vec<Kline> = read("klines_1s");
    assert!(seconds.len() > 1);
    assert_eq!(seconds[1].open_time - seconds[0].open_time, 1_000);
    assert!(minutes.iter().all(|k| k.close_time >= k.open_time));

    // The single-symbol and all-symbol forms agree on the symbols they name.
    let one: Ticker24h = read("ticker_24hr");
    assert_eq!(one.symbol, "BTCUSDT");
    let all: Vec<Ticker24h> = read("tickers_24hr");
    for symbol in ["BTCUSDT", "ETHUSDT"] {
        assert!(all.iter().any(|t| t.symbol == symbol), "{symbol}");
    }
    let prices: Vec<PriceTicker> = read("prices");
    assert!(prices.iter().any(|t| t.symbol == "BTCUSDT"));
    let books: Vec<BookTicker> = read("book_tickers");
    assert!(books.iter().any(|t| t.symbol == "BTCUSDT"));
    let book_one: BookTicker = read("book_ticker");
    assert!(book_one.bid_price <= book_one.ask_price);
}

#[test]
fn the_exchange_info_fixture_holds_a_symbol_that_is_not_trading() {
    // An unknown status would land in `Other` and decode fine; failing here is
    // how a status Binance adds gets modelled instead of passing silently.
    let info: ExchangeInfo = read("exchange_info");
    assert!(
        info.symbols
            .iter()
            .any(|s| s.status != SymbolStatus::Trading),
        "a symbol that is not TRADING"
    );
    let unnamed: Vec<_> = info
        .symbols
        .iter()
        .filter(|s| matches!(s.status, SymbolStatus::Other(_)))
        .map(|s| (&s.symbol, &s.status))
        .collect();
    assert!(
        unnamed.is_empty(),
        "statuses SymbolStatus does not name, model them: {unnamed:?}"
    );
}

#[test]
fn every_interval_the_host_takes_is_named() {
    let wire: Vec<&str> = Interval::ALL.iter().map(|i| i.as_str()).collect();
    assert_eq!(
        wire,
        [
            "1s", "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "6h", "8h", "12h", "1d", "3d",
            "1w", "1M"
        ]
    );
}
