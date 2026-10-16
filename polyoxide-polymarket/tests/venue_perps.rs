//! Perps through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against mock servers serving the captured fixtures: every method returns
//! the values the native client decodes from the same body.

use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::perps::{
    types::{InstrumentId, Interval},
    venue::{BookExt, FundingExt, InstrumentExt, TradeExt},
    Perps,
};
use polyoxide_polymarket::shared::keys;
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, DynCandles, DynFunding, DynInstruments,
    DynMarketData, DynTrades, Funding, Instruments, Level, MarketData, MarketKey, NotListed, Side,
    SizeUnit, TradeId, Trades, UnixMillis,
};
use rust_decimal::Decimal;
use serde_json::Value;

fn client(server: &ServerGuard) -> Perps {
    Perps::builder().base_url(server.url()).build().unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("perps").text(name)
}

fn key(id: u64) -> MarketKey {
    keys::perps(InstrumentId(id))
}

/// The captured instrument rows whose id is `id`, as the host answers a
/// filtered request.
fn instruments_with(id: u64) -> String {
    let rows: Vec<Value> = serde_json::from_str(&fixture("instruments")).unwrap();
    Value::Array(
        rows.into_iter()
            .filter(|row| row["instrument_id"] == id)
            .collect(),
    )
    .to_string()
}

/// A route answering `body` `hits` times, matched on its whole query string.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str, hits: usize) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

#[tokio::test]
async fn a_dyn_market_data_holding_perps_returns_the_native_bbo() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/bbo",
        "instrument_id=1",
        &fixture("bbo"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .bbo()
        .instrument_id(InstrumentId(1))
        .send()
        .await
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&key(1)).await.unwrap();
    mock.assert_async().await;

    assert_eq!(quote.key, key(1));
    assert_eq!(
        quote.bid,
        Some(Level::new(native[0].bid_price, native[0].bid_quantity))
    );
    assert_eq!(
        quote.ask,
        Some(Level::new(native[0].ask_price, native[0].ask_quantity))
    );
    assert_eq!(quote.time, Some(UnixMillis(native[0].timestamp)));
}

#[tokio::test]
async fn an_instrument_the_host_sends_no_bbo_for_has_an_empty_quote() {
    let mut server = Server::new_async().await;
    let _mock = route(&mut server, "/v1/info/bbo", "instrument_id=999", "[]", 1).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let quote = venue.quote(&key(999)).await.unwrap();
    assert_eq!((quote.bid, quote.ask, quote.time), (None, None, None));
}

#[tokio::test]
async fn book_is_the_native_book_best_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/book",
        "instrument_id=1",
        &fixture("book"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.market().book(InstrumentId(1)).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let book = venue.book(&key(1)).await.unwrap();
    mock.assert_async().await;

    let bids: Vec<Level> = native
        .bids
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    let asks: Vec<Level> = native
        .asks
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    assert_eq!(book.bids, bids);
    assert_eq!(book.asks, asks);
    assert!(
        book.bids.windows(2).all(|w| w[0].price > w[1].price),
        "bids best first"
    );
    assert!(
        book.asks.windows(2).all(|w| w[0].price < w[1].price),
        "asks best first"
    );
    assert_eq!(book.time, Some(UnixMillis(native.timestamp)));
    assert_eq!(
        book.extensions.get::<BookExt>().unwrap().sequence,
        Some(native.sequence)
    );
}

#[tokio::test]
async fn instrument_carries_the_rows_rules_and_its_categorys_fee_tiers() {
    let mut server = Server::new_async().await;
    // Instrument 9 is an equity, the one category the captured schedule has.
    let rows = route(
        &mut server,
        "/v1/info/instruments",
        "instrument_id=9",
        &instruments_with(9),
        2,
    )
    .await;
    let fees = route(&mut server, "/v1/info/fees", "", &fixture("fees"), 2).await;
    let client = client(&server);
    let native = client
        .exchange()
        .instruments()
        .instrument_id(InstrumentId(9))
        .send()
        .await
        .unwrap();
    let schedule = client.exchange().fees().send().await.unwrap();
    let row = &native[0];
    let entry = &schedule.fee_schedule[0];

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let instrument = venue.instrument(&key(9)).await.unwrap();
    rows.assert_async().await;
    fees.assert_async().await;

    assert_eq!(instrument.key, key(9));
    assert_eq!(instrument.name, row.symbol);
    assert_eq!(instrument.size_unit, SizeUnit::Contracts);
    assert_eq!(
        instrument.base_asset.as_deref(),
        Some(row.base_asset.as_str())
    );
    assert_eq!(
        instrument.quote_asset.as_deref(),
        Some(row.quote_asset.as_str())
    );
    assert_eq!(
        instrument.tick_size, None,
        "prices keep significant figures, not a tick"
    );
    assert_eq!(
        instrument.size_increment,
        Some(Decimal::new(1, row.quantity_decimals))
    );
    assert_eq!(instrument.min_notional, Some(row.min_notional));
    assert_eq!(
        instrument.tradable,
        row.close_only.map(|close_only| !close_only)
    );
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.price_bounds, Some(row.price_bounds));
    assert_eq!(ext.risk_tiers.as_ref(), Some(&row.risk_tiers));
    assert_eq!(ext.max_leverage, Some(row.max_leverage));
    assert_eq!(ext.price_decimals, Some(row.price_decimals));
    assert_eq!(entry.category, row.category);
    assert_eq!(ext.fee_tiers.as_ref(), Some(&entry.tiers));
    assert_eq!(ext.taker_fee_rate, Some(entry.taker_fee_rate));
    assert_eq!(ext.maker_fee_rate, Some(entry.maker_fee_rate));
}

#[tokio::test]
async fn an_instrument_whose_category_has_no_fee_schedule_has_no_fee_tiers() {
    let mut server = Server::new_async().await;
    // Instrument 1 is an index, which the captured schedule does not list.
    let _rows = route(
        &mut server,
        "/v1/info/instruments",
        "instrument_id=1",
        &instruments_with(1),
        1,
    )
    .await;
    let _fees = route(&mut server, "/v1/info/fees", "", &fixture("fees"), 1).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let instrument = venue.instrument(&key(1)).await.unwrap();
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.fee_tiers, None);
    assert_eq!(ext.taker_fee_rate, None);
}

#[tokio::test]
async fn an_instrument_the_host_does_not_list_is_a_venue_refusal() {
    let mut server = Server::new_async().await;
    let _rows = route(
        &mut server,
        "/v1/info/instruments",
        "instrument_id=999",
        "[]",
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue.instrument(&key(999)).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(
        err.source.downcast_ref::<NotListed>(),
        Some(&NotListed { key: key(999) })
    );
}

#[tokio::test]
async fn instruments_lists_every_row_once() {
    let mut server = Server::new_async().await;
    let rows = route(
        &mut server,
        "/v1/info/instruments",
        "",
        &fixture("instruments"),
        2,
    )
    .await;
    let fees = route(&mut server, "/v1/info/fees", "", &fixture("fees"), 1).await;
    let client = client(&server);
    let native = client.exchange().instruments().send().await.unwrap();

    let venue: Arc<DynInstruments<'static>> = DynInstruments::new_arc(client);
    let instruments = venue.instruments().await.unwrap();
    rows.assert_async().await;
    fees.assert_async().await;

    let keys: Vec<MarketKey> = instruments.iter().map(|i| i.key.clone()).collect();
    let expected: Vec<MarketKey> = native
        .iter()
        .map(|row| keys::perps(row.instrument_id))
        .collect();
    assert_eq!(keys, expected);
    assert_eq!(instruments.len(), 88);
}

#[tokio::test]
async fn trades_are_the_native_trades_oldest_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/trades",
        "instrument_id=1",
        &fixture("trades"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .trades(InstrumentId(1))
        .send()
        .await
        .unwrap();

    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);
    let trades = venue.trades(&key(1)).await.unwrap();
    mock.assert_async().await;

    assert_eq!(trades.len(), native.data.len());
    for (trade, row) in trades.iter().zip(native.data.iter().rev()) {
        assert_eq!(trade.id, Some(TradeId::new(row.trade_id.to_string())));
        assert_eq!(trade.price, row.price);
        assert_eq!(trade.size, row.quantity);
        assert_eq!(trade.time, UnixMillis(row.timestamp));
        assert_eq!(
            trade.taker_side,
            Some(match row.side.as_str() {
                "long" => Side::Buy,
                _ => Side::Sell,
            })
        );
        let ext = trade.extensions.get::<TradeExt>().unwrap();
        assert_eq!(ext.hash.as_deref(), Some(row.hash.as_str()));
        assert_eq!(ext.settlement, row.settlement);
    }
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn candles_are_the_native_klines_from_start() {
    let mut server = Server::new_async().await;
    let query =
        "instrument_id=1&interval=1h&start_timestamp=1790675505663&end_timestamp=1790761905663";
    let mock = route(&mut server, "/v1/info/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(InstrumentId(1), Interval::H1, 1_790_675_505_663)
        .end(1_790_761_905_663)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_790_675_505_663))
        .end(UnixMillis(1_790_761_905_663));
    let candles = venue.candles(&key(1), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.data.len());
    for (candle, row) in candles.iter().zip(&native.data) {
        assert_eq!(candle.open_time, UnixMillis(row.open_time));
        assert_eq!(candle.open, Some(row.open));
        assert_eq!(candle.high, Some(row.high));
        assert_eq!(candle.low, Some(row.low));
        assert_eq!(candle.close, row.close);
        assert_eq!(candle.volume, Some(row.volume));
        assert_eq!(candle.trades, Some(row.trades));
        assert_eq!(candle.notional, None);
    }
}

#[tokio::test]
async fn funding_rate_is_the_instruments_ticker() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/tickers",
        "instrument_id=1",
        &fixture("tickers"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .tickers()
        .instrument_id(InstrumentId(1))
        .send()
        .await
        .unwrap();
    let row = native
        .iter()
        .find(|row| row.instrument_id == InstrumentId(1))
        .unwrap();
    assert!(native.len() > 1, "the host sends every instrument");

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let rate = venue.funding_rate(&key(1)).await.unwrap();
    mock.assert_async().await;

    assert_eq!(rate.rate, row.funding_rate);
    assert_eq!(rate.time, UnixMillis(row.timestamp));
    assert_eq!(rate.next_funding, Some(UnixMillis(row.next_funding)));
    let ext = rate.extensions.get::<FundingExt>().unwrap();
    assert_eq!(ext.mark_price, Some(row.mark_price));
    assert_eq!(ext.index_price, Some(row.index_price));
    assert_eq!(ext.open_interest, Some(row.open_interest));
}

#[tokio::test]
async fn funding_history_is_the_native_history_oldest_first() {
    let mut server = Server::new_async().await;
    let query = "instrument_id=1&start_timestamp=1790675505663";
    let mock = route(
        &mut server,
        "/v1/info/funding",
        query,
        &fixture("funding"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .funding(InstrumentId(1))
        .start(1_790_675_505_663)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let history = venue
        .funding_history(&key(1), UnixMillis(1_790_675_505_663))
        .await
        .unwrap();
    mock.assert_async().await;

    assert_eq!(history.len(), native.data.len());
    for (rate, row) in history.iter().zip(native.data.iter().rev()) {
        assert_eq!(rate.rate, row.funding_rate);
        assert_eq!(rate.time, UnixMillis(row.timestamp));
        assert_eq!(rate.next_funding, None);
    }
    assert!(
        history.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}
