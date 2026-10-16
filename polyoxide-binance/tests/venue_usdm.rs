//! USDⓈ-M through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against mock servers serving the captured fixtures: every method returns
//! the values the native client decodes from the same body.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::usdm::{
    self,
    types::{DepthLimit, Filter, Interval, Symbol},
    venue::{BookExt, CandleExt, FundingExt, InstrumentExt, QuoteExt, TradeExt},
    Usdm, UsdmError,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, Classify, DynCandles, DynFunding, DynInstruments,
    DynMarketData, DynTrades, Funding, Instruments, KeySpace, Level, MarketData, MarketKey,
    NotListed, ProductId, Side, SizeUnit, TradeId, Trades, UnixMillis, VenueId,
};
use serde_json::Value;

fn client(server: &ServerGuard) -> Usdm {
    Usdm::builder().base_url(server.url()).build().unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("usdm/rest").text(name)
}

fn btc_symbol() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

fn btc() -> MarketKey {
    usdm::key(&btc_symbol())
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
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

const HOUR: Duration = Duration::from_secs(60 * 60);

/// `fundingInfo` with `BTCUSDT` listed at `hours`, or not listed at all.
fn funding_info_body(btc_hours: Option<u32>) -> String {
    let mut rows: Vec<Value> = serde_json::from_str(&fixture("funding_info")).unwrap();
    let btc = rows
        .iter()
        .position(|row| row["symbol"] == "BTCUSDT")
        .unwrap();
    match btc_hours {
        Some(hours) => rows[btc]["fundingIntervalHours"] = Value::from(hours),
        None => {
            rows.remove(btc);
        }
    }
    Value::Array(rows).to_string()
}

/// The `fundingInfo` the funding methods read, answering once per call.
async fn funding_info(server: &mut ServerGuard, btc_hours: Option<u32>, hits: usize) -> Mock {
    route(
        server,
        "/fapi/v1/fundingInfo",
        "",
        &funding_info_body(btc_hours),
        hits,
    )
    .await
}

#[tokio::test]
async fn a_dyn_market_data_holding_usdm_returns_the_native_quote() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT&limit=5",
        &fixture("depth"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .depth(&btc_symbol())
        .limit(DepthLimit::Five)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(quote.key, btc());
    assert_eq!(
        quote.bid,
        Some(Level::new(native.bids[0].price, native.bids[0].quantity))
    );
    assert_eq!(
        quote.ask,
        Some(Level::new(native.asks[0].price, native.asks[0].quantity))
    );
    assert_eq!(quote.time, Some(UnixMillis(native.transaction_time)));
    let ext = quote.extensions.get::<QuoteExt>().unwrap();
    assert_eq!(ext.last_update_id, Some(native.last_update_id));
    assert_eq!(ext.event_time, Some(UnixMillis(native.event_time)));
}

#[tokio::test]
async fn book_returns_every_level_of_the_native_depth_best_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.market().depth(&btc_symbol()).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let book = venue.book(&btc()).await.unwrap();
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
    assert_eq!(book.time, Some(UnixMillis(native.transaction_time)));
    let ext = book.extensions.get::<BookExt>().unwrap();
    assert_eq!(ext.last_update_id, Some(native.last_update_id));
}

#[tokio::test]
async fn instrument_reads_its_contract_from_exchange_info() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.exchange().exchange_info().send().await.unwrap();
    let row = native
        .symbols
        .iter()
        .find(|row| row.symbol == "BTCUSDT")
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let instrument = venue.instrument(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(instrument.key, btc());
    assert_eq!(instrument.name, "BTCUSDT");
    assert_eq!(instrument.size_unit, SizeUnit::Base);
    assert_eq!(
        instrument.base_asset.as_deref(),
        Some(row.base_asset.as_str())
    );
    assert_eq!(
        instrument.quote_asset.as_deref(),
        Some(row.quote_asset.as_str())
    );
    assert_eq!(instrument.tradable, Some(true));
    for filter in &row.filters {
        match filter {
            Filter::PriceFilter { tick_size, .. } => {
                assert_eq!(instrument.tick_size, Some(*tick_size))
            }
            Filter::LotSize {
                min_qty, step_size, ..
            } => {
                assert_eq!(instrument.min_size, Some(*min_qty));
                assert_eq!(instrument.size_increment, Some(*step_size));
            }
            Filter::MinNotional { notional, .. } => {
                assert_eq!(instrument.min_notional, Some(*notional))
            }
            _ => {}
        }
    }
    assert!(instrument.tick_size.is_some() && instrument.min_size.is_some());
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.contract_type.as_ref(), Some(&row.contract_type));
    assert_eq!(ext.status.as_ref(), Some(&row.status));
    assert_eq!(ext.filters.as_ref(), Some(&row.filters));
    assert_eq!(ext.delivery_date, Some(UnixMillis(row.delivery_date)));
}

#[tokio::test]
async fn a_contract_exchange_info_does_not_list_is_a_venue_refusal() {
    let mut server = Server::new_async().await;
    let _mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let missing = usdm::key(&Symbol::new("NOTLISTEDUSDT").unwrap());
    let err = venue.instrument(&missing).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(
        err.source.downcast_ref::<NotListed>(),
        Some(&NotListed { key: missing })
    );
}

#[tokio::test]
async fn instruments_lists_every_contract_with_its_status() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.exchange().exchange_info().send().await.unwrap();

    let venue: Arc<DynInstruments<'static>> = DynInstruments::new_arc(client);
    let instruments = venue.instruments().await.unwrap();
    mock.assert_async().await;

    let keys: Vec<String> = instruments.iter().map(|i| i.key.to_string()).collect();
    let expected: Vec<String> = native
        .symbols
        .iter()
        .map(|row| format!("binance.usdm:{}", row.symbol))
        .collect();
    assert_eq!(keys, expected);
    for (instrument, row) in instruments.iter().zip(&native.symbols) {
        assert_eq!(
            instrument.tradable,
            Some(row.status.as_str() == "TRADING"),
            "{}",
            row.symbol
        );
    }
    assert!(
        instruments.iter().any(|i| i.tradable == Some(false)),
        "the fixture holds a SETTLING contract"
    );
}

#[tokio::test]
async fn trades_are_the_native_aggregate_trades_with_the_takers_side() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/aggTrades",
        "symbol=BTCUSDT",
        &fixture("agg_trades"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .agg_trades(&btc_symbol())
        .send()
        .await
        .unwrap();

    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);
    let trades = venue.trades(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(trades.len(), native.len());
    for (trade, row) in trades.iter().zip(&native) {
        assert_eq!(trade.key, btc());
        assert_eq!(trade.id, Some(TradeId::new(row.id.to_string())));
        assert_eq!(trade.price, row.price);
        assert_eq!(trade.size, row.quantity);
        assert_eq!(trade.notional, None);
        assert_eq!(
            trade.taker_side,
            Some(if row.is_buyer_maker {
                Side::Sell
            } else {
                Side::Buy
            })
        );
        assert_eq!(trade.time, UnixMillis(row.time));
        let ext = trade.extensions.get::<TradeExt>().unwrap();
        assert_eq!(ext.first_trade_id, Some(row.first_trade_id));
        assert_eq!(ext.last_trade_id, Some(row.last_trade_id));
        assert_eq!(ext.normal_size, Some(row.normal_quantity));
    }
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn candles_are_the_native_klines_from_start() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&interval=1h&startTime=1791380000000&endTime=1791390000000";
    let mock = route(&mut server, "/fapi/v1/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(&btc_symbol(), Interval::H1)
        .start_time(1_791_380_000_000)
        .end_time(1_791_390_000_000)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000))
        .end(UnixMillis(1_791_390_000_000));
    let candles = venue.candles(&btc(), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.len());
    for (candle, row) in candles.iter().zip(&native) {
        assert_eq!(candle.interval, CandleInterval::Hour1);
        assert_eq!(candle.open_time, UnixMillis(row.open_time));
        assert_eq!(candle.open, Some(row.open));
        assert_eq!(candle.high, Some(row.high));
        assert_eq!(candle.low, Some(row.low));
        assert_eq!(candle.close, row.close);
        assert_eq!(candle.volume, Some(row.volume));
        assert_eq!(candle.notional, Some(row.quote_volume));
        assert_eq!(candle.trades, Some(row.trade_count));
        let ext = candle.extensions.get::<CandleExt>().unwrap();
        assert_eq!(ext.close_time, Some(UnixMillis(row.close_time)));
        assert_eq!(ext.taker_buy_volume, Some(row.taker_buy_base_volume));
        assert_eq!(ext.taker_buy_notional, Some(row.taker_buy_quote_volume));
    }
}

#[tokio::test]
async fn funding_rate_is_the_premium_index_now_accruing() {
    let mut server = Server::new_async().await;
    let all: Vec<Value> = serde_json::from_str(&fixture("premium_index")).unwrap();
    let body = all
        .iter()
        .find(|row| row["symbol"] == "BTCUSDT")
        .unwrap()
        .to_string();
    let mock = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "symbol=BTCUSDT",
        &body,
        2,
    )
    .await;
    let info = funding_info(&mut server, Some(4), 1).await;
    let client = client(&server);
    let native = client
        .market()
        .premium_index(&btc_symbol())
        .send()
        .await
        .unwrap();

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let rate = venue.funding_rate(&btc()).await.unwrap();
    mock.assert_async().await;
    info.assert_async().await;

    assert_eq!(rate.rate, native.last_funding_rate);
    assert_eq!(rate.interval, Some(4 * HOUR), "BTCUSDT's listed interval");
    assert_eq!(rate.time, UnixMillis(native.time));
    assert_eq!(
        rate.next_funding,
        Some(UnixMillis(native.next_funding_time))
    );
    let ext = rate.extensions.get::<FundingExt>().unwrap();
    assert_eq!(ext.mark_price, Some(native.mark_price));
    assert_eq!(ext.index_price, Some(native.index_price));
    assert_eq!(
        ext.estimated_settle_price,
        Some(native.estimated_settle_price)
    );
    assert_eq!(ext.interest_rate, Some(native.interest_rate));
}

#[tokio::test]
async fn funding_history_is_the_native_funding_rates_from_start() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&startTime=1791200000000";
    let mock = route(
        &mut server,
        "/fapi/v1/fundingRate",
        query,
        &fixture("funding_rate"),
        2,
    )
    .await;
    let info = funding_info(&mut server, Some(4), 1).await;
    let client = client(&server);
    let native = client
        .market()
        .funding_rate()
        .symbol(&btc_symbol())
        .start_time(1_791_200_000_000)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let history = venue
        .funding_history(&btc(), UnixMillis(1_791_200_000_000))
        .await
        .unwrap();
    mock.assert_async().await;
    info.assert_async().await;

    assert_eq!(history.len(), native.len());
    for (rate, row) in history.iter().zip(&native) {
        assert_eq!(rate.rate, row.funding_rate);
        assert_eq!(rate.time, UnixMillis(row.funding_time));
        assert_eq!(rate.next_funding, None, "a settled rate");
        assert_eq!(
            rate.interval,
            Some(4 * HOUR),
            "the symbol's current interval"
        );
        assert_eq!(
            rate.extensions.get::<FundingExt>().unwrap().mark_price,
            row.mark_price
        );
    }
    assert!(
        history.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn a_venue_error_reaches_the_caller_classified_with_its_native_error_as_source() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/fapi/v1/depth")
        .match_query(Matcher::Any)
        .with_status(400)
        .with_body(r#"{"code":-1121,"msg":"Invalid symbol."}"#)
        .create_async()
        .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue.book(&btc()).await.unwrap_err();
    assert_eq!(
        err.class,
        Class::VenueRefusal {
            code: Some("-1121".into())
        }
    );
    assert!(err.source.downcast_ref::<UsdmError>().is_some());
}

/// A product of another venue, to hand USDⓈ-M a key it does not own.
struct Elsewhere;

impl KeySpace for Elsewhere {
    const VENUE: VenueId = VenueId::from_static("elsewhere");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

#[tokio::test]
async fn a_key_of_another_product_is_refused_before_anything_is_sent() {
    let server = Server::new_async().await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let foreign = Elsewhere::key(&"BTCUSDT".to_owned());
    for err in [
        venue.quote(&foreign).await.unwrap_err(),
        venue.book(&foreign).await.unwrap_err(),
        venue.instrument(&foreign).await.unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
    }
}

#[tokio::test]
async fn a_key_of_another_product_is_refused_by_every_trait_before_anything_is_sent() {
    let mut server = Server::new_async().await;
    let untouched = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let client = client(&server);
    let foreign = Elsewhere::key(&"BTCUSDT".to_owned());
    let trades: Arc<DynTrades<'static>> = DynTrades::new_arc(client.clone());
    let candles: Arc<DynCandles<'static>> = DynCandles::new_arc(client.clone());
    let funding: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000));
    for err in [
        trades.trades(&foreign).await.unwrap_err(),
        candles.candles(&foreign, &request).await.unwrap_err(),
        funding.funding_rate(&foreign).await.unwrap_err(),
        funding
            .funding_history(&foreign, UnixMillis(1_791_200_000_000))
            .await
            .unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
    }
    untouched.assert_async().await;
}

#[tokio::test]
async fn candles_with_no_end_send_no_end_time() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&interval=1h&startTime=1791380000000";
    let mock = route(&mut server, "/fapi/v1/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(&btc_symbol(), Interval::H1)
        .start_time(1_791_380_000_000)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000));
    assert_eq!(request.end, None);
    let candles = venue.candles(&btc(), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.len());
    assert!(!candles.is_empty());
    for (candle, row) in candles.iter().zip(&native) {
        assert_eq!(candle.open_time, UnixMillis(row.open_time));
        assert_eq!(candle.close, row.close);
        assert_eq!(candle.volume, Some(row.volume));
    }
}

#[tokio::test]
async fn funding_history_without_a_mark_price_has_none() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&startTime=1568000000000";
    let mock = route(
        &mut server,
        "/fapi/v1/fundingRate",
        query,
        &fixture("funding_rate_2019"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .funding_rate()
        .symbol(&btc_symbol())
        .start_time(1_568_000_000_000)
        .send()
        .await
        .unwrap();
    assert!(!native.is_empty());
    assert!(native.iter().all(|row| row.mark_price.is_none()));
    let info = funding_info(&mut server, Some(8), 1).await;

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let history = venue
        .funding_history(&btc(), UnixMillis(1_568_000_000_000))
        .await
        .unwrap();
    mock.assert_async().await;
    info.assert_async().await;

    assert_eq!(history.len(), native.len());
    for (rate, row) in history.iter().zip(&native) {
        assert_eq!(rate.rate, row.funding_rate);
        assert_eq!(rate.time, UnixMillis(row.funding_time));
        assert_eq!(rate.next_funding, None);
        assert_eq!(
            rate.extensions.get::<FundingExt>().unwrap().mark_price,
            None
        );
    }
}

#[tokio::test]
async fn a_contract_with_no_funding_scheduled_has_no_next_funding() {
    let mut server = Server::new_async().await;
    let all: Vec<Value> = serde_json::from_str(&fixture("premium_index")).unwrap();
    let mut row = all
        .iter()
        .find(|row| row["symbol"] == "BTCUSDT")
        .unwrap()
        .clone();
    row["nextFundingTime"] = Value::from(0);
    let mock = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "symbol=BTCUSDT",
        &row.to_string(),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .premium_index(&btc_symbol())
        .send()
        .await
        .unwrap();
    assert_eq!(native.next_funding_time, 0);
    let info = funding_info(&mut server, Some(8), 1).await;

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let rate = venue.funding_rate(&btc()).await.unwrap();
    mock.assert_async().await;
    info.assert_async().await;

    assert_eq!(rate.next_funding, None);
    assert_eq!(rate.rate, native.last_funding_rate);
    assert_eq!(rate.time, UnixMillis(native.time));
}

#[tokio::test]
async fn an_unlisted_symbol_funds_every_eight_hours() {
    let mut server = Server::new_async().await;
    let all: Vec<Value> = serde_json::from_str(&fixture("premium_index")).unwrap();
    let body = all
        .iter()
        .find(|row| row["symbol"] == "BTCUSDT")
        .unwrap()
        .to_string();
    let index = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "symbol=BTCUSDT",
        &body,
        1,
    )
    .await;
    let history = route(
        &mut server,
        "/fapi/v1/fundingRate",
        "symbol=BTCUSDT&startTime=1791200000000",
        &fixture("funding_rate"),
        1,
    )
    .await;
    let info = funding_info(&mut server, None, 2).await;
    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client(&server));

    let rate = venue.funding_rate(&btc()).await.unwrap();
    let rows = venue
        .funding_history(&btc(), UnixMillis(1_791_200_000_000))
        .await
        .unwrap();
    index.assert_async().await;
    history.assert_async().await;
    info.assert_async().await;

    assert_eq!(rate.interval, Some(8 * HOUR));
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| row.interval == Some(8 * HOUR)));
}

#[tokio::test]
async fn a_listed_symbol_funds_on_its_own_interval_and_not_the_default() {
    let mut server = Server::new_async().await;
    let all: Vec<Value> = serde_json::from_str(&fixture("premium_index")).unwrap();
    let body = all
        .iter()
        .find(|row| row["symbol"] == "BTCUSDT")
        .unwrap()
        .to_string();
    let index = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "symbol=BTCUSDT",
        &body,
        1,
    )
    .await;
    let info = funding_info(&mut server, Some(4), 1).await;
    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client(&server));

    let rate = venue.funding_rate(&btc()).await.unwrap();
    index.assert_async().await;
    info.assert_async().await;

    assert_eq!(rate.interval, Some(Duration::from_secs(4 * 60 * 60)));
    assert_ne!(rate.interval, Some(8 * HOUR));
}

#[tokio::test]
async fn the_interval_lookup_is_charged_on_the_funding_layer() {
    // `fundingInfo` has no weight: it shares the funding bucket (a depth of
    // one, 300s / 449 or about 668ms between requests). `funding_history` sends
    // `fundingRate` and `fundingInfo`, both funding, so the second goes about
    // 668ms after the first was charged. Charged as weight instead, both would
    // go at once.
    let mut server = Server::new_async().await;
    let history = route(
        &mut server,
        "/fapi/v1/fundingRate",
        "symbol=BTCUSDT&startTime=1791200000000",
        &fixture("funding_rate"),
        1,
    )
    .await;
    let info = funding_info(&mut server, Some(4), 1).await;
    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client(&server));

    let start = Instant::now();
    venue
        .funding_history(&btc(), UnixMillis(1_791_200_000_000))
        .await
        .unwrap();
    let paced = start.elapsed();
    history.assert_async().await;
    info.assert_async().await;
    assert!(
        paced >= Duration::from_millis(600) && paced < Duration::from_secs(5),
        "the interval lookup went {paced:?} after the funding rows were charged"
    );
}

#[tokio::test]
async fn a_trait_error_carries_the_native_errors_class_fault_and_wait() {
    let cases: [(usize, &str, Option<&str>); 3] = [
        (400, r#"{"code":-1121,"msg":"Invalid symbol."}"#, None),
        (418, r#"{"code":-1003,"msg":"Banned."}"#, Some("5")),
        (451, r#"{"code":0,"msg":"Restricted location."}"#, Some("3")),
    ];
    for (status, body, retry_after) in cases {
        let mut server = Server::new_async().await;
        let mut mock = server
            .mock("GET", "/fapi/v1/depth")
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(body);
        if let Some(wait) = retry_after {
            mock = mock.with_header("retry-after", wait);
        }
        let _mock = mock.create_async().await;

        // One client apiece, so the native call's hold is not the trait call's.
        let native = client(&server)
            .market()
            .depth(&btc_symbol())
            .send()
            .await
            .unwrap_err();
        let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
        let err = venue.book(&btc()).await.unwrap_err();

        assert_eq!(err.class, native.class(), "{status}");
        assert_eq!(err.is_fault(), native.is_fault(), "{status}");
        assert_eq!(err.retry_after(), native.retry_after(), "{status}");
        assert!(err.source.downcast_ref::<UsdmError>().is_some(), "{status}");
    }
}
