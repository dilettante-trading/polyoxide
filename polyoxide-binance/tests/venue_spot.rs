//! Spot through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against mock servers serving the captured fixtures: every method returns
//! the values the native client decodes from the same body.

use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::spot::{
    self,
    types::{Filter, Interval, Symbol},
    venue::{BookExt, CandleExt, InstrumentExt, TradeExt},
    Spot, SpotError,
};
use polyoxide_test_support::{fixtures, minute};
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, Classify, DynCandles, DynInstruments,
    DynMarketData, DynTrades, Instruments, KeySpace, Level, MarketData, MarketKey, NotListed,
    ProductClass, ProductId, Quote, RawKey, Side, SizeUnit, TradeId, Trades, UnixMillis, VenueId,
};
use serde_json::Value;

fn client(server: &ServerGuard) -> Spot {
    Spot::builder().base_url(server.url()).build().unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("spot/rest").text(name)
}

fn btc_symbol() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

fn btc() -> MarketKey {
    spot::key(&btc_symbol())
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

#[tokio::test]
async fn quote_is_the_native_book_ticker_and_carries_no_extension() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/ticker/bookTicker",
        "symbol=BTCUSDT",
        &fixture("book_ticker"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .book_ticker(&btc_symbol())
        .send()
        .await
        .unwrap();
    assert!(native.bid_price < native.ask_price, "a real quote");

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(quote.key, btc());
    assert_eq!(
        quote.bid,
        Some(Level::new(native.bid_price, native.bid_qty))
    );
    assert_eq!(
        quote.ask,
        Some(Level::new(native.ask_price, native.ask_qty))
    );
    // `ticker/bookTicker` carries no time and nothing beyond the record's fields.
    assert_eq!(quote.time, None);
    assert_eq!(quote.extensions, Quote::new(btc()).extensions);
}

#[tokio::test]
async fn book_returns_every_level_of_the_native_depth_best_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/depth",
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
    assert!(!bids.is_empty() && !asks.is_empty());
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
    // The default depth carries no time.
    assert_eq!(book.time, None);
    let ext = book.extensions.get::<BookExt>().unwrap();
    assert_eq!(ext.last_update_id, Some(native.last_update_id));
}

#[tokio::test]
async fn instrument_reads_its_pair_from_exchange_info() {
    let mut server = Server::new_async().await;
    // The host answers `?symbol=` with that one row; the fixture holds several,
    // so the read has to pick the right one out.
    let mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "symbol=BTCUSDT",
        &fixture("exchange_info"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .exchange()
        .exchange_info()
        .symbol(&btc_symbol())
        .send()
        .await
        .unwrap();
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
    assert!(
        row.filters
            .iter()
            .any(|filter| matches!(filter, Filter::Notional { .. })),
        "the fixture's BTCUSDT carries a NOTIONAL filter"
    );
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
            Filter::Notional { min_notional, .. } => {
                assert_eq!(instrument.min_notional, Some(*min_notional))
            }
            _ => {}
        }
    }
    assert!(instrument.tick_size.is_some() && instrument.min_size.is_some());
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.status.as_ref(), Some(&row.status));
    assert_eq!(ext.permission_sets.as_ref(), Some(&row.permission_sets));
    assert_eq!(ext.order_types.as_ref(), Some(&row.order_types));
    assert_eq!(
        ext.is_spot_trading_allowed,
        Some(row.is_spot_trading_allowed)
    );
    assert_eq!(
        ext.is_margin_trading_allowed,
        Some(row.is_margin_trading_allowed)
    );
    assert_eq!(ext.filters.as_ref(), Some(&row.filters));
}

#[tokio::test]
async fn a_pair_the_answer_does_not_hold_is_not_listed() {
    let mut server = Server::new_async().await;
    let mut body: Value = serde_json::from_str(&fixture("exchange_info")).unwrap();
    body["symbols"] = Value::Array(Vec::new());
    let _mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "symbol=BTCUSDT",
        &body.to_string(),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue.instrument(&btc()).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(
        err.source.downcast_ref::<NotListed>(),
        Some(&NotListed::new(btc()))
    );
}

#[tokio::test]
async fn a_pair_the_host_refuses_is_the_hosts_own_refusal() {
    // The captured answer to `exchangeInfo?symbol=NOPEUSDT`: a 400 with the
    // host's `{code, msg}`.
    let body = fixture("error_invalid_symbol");
    let code = serde_json::from_str::<Value>(&body).unwrap()["code"]
        .as_i64()
        .unwrap();
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v3/exchangeInfo")
        .match_query(Matcher::Exact("symbol=NOPEUSDT".to_owned()))
        .with_status(400)
        .with_body(body)
        .create_async()
        .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let missing = spot::key(&Symbol::new("NOPEUSDT").unwrap());
    let err = venue.instrument(&missing).await.unwrap_err();
    assert_eq!(
        err.class,
        Class::VenueRefusal {
            code: Some(code.to_string().into())
        }
    );
    assert!(matches!(
        err.source.downcast_ref::<SpotError>(),
        Some(SpotError::Venue { status: 400, .. })
    ));
}

#[tokio::test]
async fn instruments_lists_every_pair_with_its_status() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
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
        .map(|row| format!("binance.spot:{}", row.symbol))
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
        "the fixture holds a symbol that is not TRADING"
    );
}

/// Records the target, level and fields of every event, for the skipped row's
/// WARN.
#[derive(Clone, Default)]
struct Events(Arc<std::sync::Mutex<Vec<(String, tracing::Level, String)>>>);

struct Fields(String);

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{}={value:?} ", field.name()));
    }
}

impl tracing::Subscriber for Events {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let metadata = event.metadata();
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        self.0
            .lock()
            .unwrap()
            .push((metadata.target().to_owned(), *metadata.level(), fields.0));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

#[tokio::test]
async fn instruments_skips_a_row_that_does_not_convert_and_returns_the_rest() {
    // The current-thread runtime runs the call on this thread, so the scoped
    // subscriber sees its events.
    let events = Events::default();
    let _scope = tracing::subscriber::set_default(events.clone());
    // With one dispatcher registered, tracing-core works out the interest of
    // a callsite another test's thread hits first from that thread's
    // default, which has none, and caches it as never for every thread. With
    // a second one registered it asks each registered dispatcher instead.
    let _second = tracing::Dispatch::new(Events::default());
    let mut server = Server::new_async().await;
    let mut info: Value = serde_json::from_str(&fixture("exchange_info")).unwrap();
    let rows = info["symbols"].as_array_mut().unwrap();
    let valid = rows.len();
    let mut bad = rows[0].clone();
    bad["symbol"] = Value::from("BAD-SYMBOL");
    rows.insert(1, bad);
    let mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "",
        &info.to_string(),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.exchange().exchange_info().send().await.unwrap();
    assert_eq!(native.symbols.len(), valid + 1);

    let venue: Arc<DynInstruments<'static>> = DynInstruments::new_arc(client);
    let instruments = venue.instruments().await.unwrap();
    mock.assert_async().await;

    assert_eq!(instruments.len(), valid);
    let keys: Vec<String> = instruments.iter().map(|i| i.key.to_string()).collect();
    let expected: Vec<String> = native
        .symbols
        .iter()
        .filter(|row| row.symbol != "BAD-SYMBOL")
        .map(|row| format!("binance.spot:{}", row.symbol))
        .collect();
    assert_eq!(keys, expected);

    // One WARN, under the venue module's own target, naming the symbol.
    let warnings: Vec<(String, String)> = events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, level, _)| *level == tracing::Level::WARN)
        .map(|(target, _, fields)| (target.clone(), fields.clone()))
        .collect();
    assert_eq!(
        warnings.len(),
        1,
        "the skipped row's warnings: {warnings:?}"
    );
    let (target, fields) = &warnings[0];
    assert_eq!(target, "polyoxide_binance::spot::venue");
    assert!(fields.contains("symbol=BAD-SYMBOL"), "{fields}");
    assert!(fields.contains("error="), "{fields}");
}

#[tokio::test]
async fn trades_are_the_native_aggregate_trades_with_the_takers_side() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/aggTrades",
        "symbol=BTCUSDT&limit=1000",
        &fixture("agg_trades"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .agg_trades(&btc_symbol())
        .limit(1000)
        .send()
        .await
        .unwrap();
    assert!(!native.is_empty());

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
        assert_eq!(ext.is_best_match, Some(row.is_best_match));
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
        "symbol=BTCUSDT&interval=1h&startTime=1791380000000&endTime=1791390000000&limit=1000";
    let mock = route(&mut server, "/api/v3/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(&btc_symbol(), Interval::Hour1)
        .start_time(1_791_380_000_000)
        .end_time(1_791_390_000_000)
        .limit(1000)
        .send()
        .await
        .unwrap();
    assert!(!native.is_empty());

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
async fn candles_with_no_end_send_no_end_time() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&interval=1h&startTime=1791380000000&limit=1000";
    let mock = route(&mut server, "/api/v3/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(&btc_symbol(), Interval::Hour1)
        .start_time(1_791_380_000_000)
        .limit(1000)
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
async fn a_lowercase_key_reaches_the_host_uppercased() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let lower = spot::key::parse(&RawKey::parse("binance.spot:btcusdt").unwrap()).unwrap();
    venue.book(&lower).await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn a_venue_error_reaches_the_caller_classified_with_its_native_error_as_source() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v3/depth")
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
    assert!(err.source.downcast_ref::<SpotError>().is_some());
}

/// A product of another venue that is also a spot product, to hand Spot a key
/// of its own class it does not own.
struct Elsewhere;

impl KeySpace for Elsewhere {
    const VENUE: VenueId = VenueId::from_static("elsewhere");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    const CLASS: ProductClass = ProductClass::Spot;
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
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
    let market: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client.clone());
    let trades: Arc<DynTrades<'static>> = DynTrades::new_arc(client.clone());
    let candles: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000));
    for err in [
        market.quote(&foreign).await.unwrap_err(),
        market.book(&foreign).await.unwrap_err(),
        market.instrument(&foreign).await.unwrap_err(),
        trades.trades(&foreign).await.unwrap_err(),
        candles.candles(&foreign, &request).await.unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
    }
    untouched.assert_async().await;
}

/// The weight a client has charged after `call` made its one request, on a
/// fresh client, clear of a UTC minute boundary. The mock reports a used
/// weight of 1, below every charge here, so the budget's count is the charge.
async fn weight_of<F, Fut>(path: &str, query: &str, body: &str, call: F) -> u32
where
    F: FnOnce(Spot) -> Fut,
    Fut: Future<Output = ()>,
{
    minute::wait_unless_within(0..=57_000, 100).await;
    let mut server = Server::new_async().await;
    let mock = route(&mut server, path, query, body, 1).await;
    let client = client(&server);
    call(client.clone()).await;
    mock.assert_async().await;
    client.weight_budget().used()
}

#[tokio::test]
async fn each_method_is_charged_the_weight_its_route_costs() {
    let info = fixture("exchange_info");
    let by_symbol = weight_of(
        "/api/v3/exchangeInfo",
        "symbol=BTCUSDT",
        &info,
        |venue| async move {
            venue.instrument(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(by_symbol, 20, "instrument");
    let listing = weight_of("/api/v3/exchangeInfo", "", &info, |venue| async move {
        venue.instruments().await.unwrap();
    })
    .await;
    assert_eq!(listing, 20, "instruments");
    let quote = weight_of(
        "/api/v3/ticker/bookTicker",
        "symbol=BTCUSDT",
        &fixture("book_ticker"),
        |venue| async move {
            venue.quote(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(quote, 2, "quote");
    let book = weight_of(
        "/api/v3/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
        |venue| async move {
            venue.book(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(book, 5, "book: the default depth");
    let trades = weight_of(
        "/api/v3/aggTrades",
        "symbol=BTCUSDT&limit=1000",
        &fixture("agg_trades"),
        |venue| async move {
            venue.trades(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(trades, 4, "trades");
    let candles = weight_of(
        "/api/v3/klines",
        "symbol=BTCUSDT&interval=1h&startTime=1791380000000&limit=1000",
        &fixture("klines"),
        |venue| async move {
            let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000));
            venue.candles(&btc(), &request).await.unwrap();
        },
    )
    .await;
    assert_eq!(candles, 2, "candles");
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
            .mock("GET", "/api/v3/depth")
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
        assert!(err.source.downcast_ref::<SpotError>().is_some(), "{status}");
    }
}
