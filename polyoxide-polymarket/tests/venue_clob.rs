//! The CLOB through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against one mock server serving the captured CLOB, Data and Gamma
//! fixtures: every method returns the values the native clients decode from
//! the same bodies.

use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::clob::{
    venue::{BookExt, EventGroupExt, InstrumentExt, QuoteExt, TradeExt},
    Clob, ClobBuilder, ClobError, PricesHistoryQuery,
};
use polyoxide_polymarket::data::{types::TradeSide, DataApi};
use polyoxide_polymarket::gamma::{types::ProtocolVersion, Gamma};
use polyoxide_polymarket::shared::keys::{self, TokenId};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, Classify, DynCandles, DynEventGroups,
    DynMarketData, DynTrades, EventGroups, KeySpace, Level, MarketData, MarketKey, NotListed,
    ProductId, Side, SizeUnit, Trades, UnixMillis, VenueId,
};
use rust_decimal::Decimal;
use serde_json::Value;

/// One client of each host, all pointed at the one mock server.
fn client(server: &ServerGuard) -> Clob {
    ClobBuilder::new()
        .base_url(server.url())
        .gamma(Gamma::builder().base_url(server.url()).build().unwrap())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .build()
        .unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("clob/venue").text(name)
}

fn json(name: &str) -> Value {
    fixtures!("clob/venue").json(name)
}

/// The captured outcome token.
fn token() -> String {
    json("book")["asset_id"].as_str().unwrap().to_owned()
}

/// The captured token's market.
fn condition() -> String {
    json("markets_by_token")["condition_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn key() -> MarketKey {
    keys::clob(&TokenId::new(token()).unwrap())
}

/// A number as the trait reads it: through its shortest decimal spelling.
fn decimal(value: f64) -> Decimal {
    Decimal::from_str(&value.to_string()).unwrap()
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
async fn a_dyn_market_data_holding_the_clob_returns_the_books_best_levels() {
    let mut server = Server::new_async().await;
    let query = format!("token_id={}", token());
    let mock = route(&mut server, "/book", &query, &fixture("book"), 2).await;
    let client = client(&server);
    let native = client.markets().order_book(token()).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&key()).await.unwrap();
    mock.assert_async().await;

    let best_bid = native.bids.iter().max_by_key(|l| l.price).unwrap();
    let best_ask = native.asks.iter().min_by_key(|l| l.price).unwrap();
    assert_eq!(quote.key, key());
    assert_eq!(quote.bid, Some(Level::new(best_bid.price, best_bid.size)));
    assert_eq!(quote.ask, Some(Level::new(best_ask.price, best_ask.size)));
    assert!(
        quote.bid.unwrap().price < quote.ask.unwrap().price,
        "an uncrossed book"
    );
    assert_eq!(
        quote.time,
        Some(UnixMillis(native.timestamp.parse().unwrap()))
    );
    assert_eq!(
        quote.extensions.get::<QuoteExt>().unwrap().last_trade_price,
        native
            .last_trade_price
            .as_deref()
            .map(|p| Decimal::from_str(p).unwrap())
    );
}

#[tokio::test]
async fn book_holds_every_native_level_best_first() {
    let mut server = Server::new_async().await;
    let query = format!("token_id={}", token());
    let mock = route(&mut server, "/book", &query, &fixture("book"), 2).await;
    let client = client(&server);
    let native = client.markets().order_book(token()).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let book = venue.book(&key()).await.unwrap();
    mock.assert_async().await;

    let mut bids: Vec<Level> = native
        .bids
        .iter()
        .map(|l| Level::new(l.price, l.size))
        .collect();
    let mut asks: Vec<Level> = native
        .asks
        .iter()
        .map(|l| Level::new(l.price, l.size))
        .collect();
    bids.sort_by_key(|level| std::cmp::Reverse(level.price));
    asks.sort_by_key(|level| level.price);
    assert!(
        bids.len() >= 2 && asks.len() >= 2,
        "the capture keeps two levels a side"
    );
    assert_ne!(
        native.bids[0].price, bids[0].price,
        "the CLOB sends bids worst first, so the trait must reorder them"
    );
    assert_eq!(book.bids, bids);
    assert_eq!(book.asks, asks);
    let ext = book.extensions.get::<BookExt>().unwrap();
    assert_eq!(ext.condition_id.as_deref(), Some(native.market.as_str()));
    assert_eq!(ext.hash.as_deref(), Some(native.hash.as_str()));
    assert_eq!(ext.neg_risk, native.neg_risk);
    assert_eq!(
        ext.tick_size,
        native
            .tick_size
            .as_deref()
            .map(|t| Decimal::from_str(t).unwrap())
    );
}

#[tokio::test]
async fn instrument_is_the_outcome_of_its_market() {
    let mut server = Server::new_async().await;
    let by_token = route(
        &mut server,
        &format!("/markets-by-token/{}", token()),
        "",
        &fixture("markets_by_token"),
        1,
    )
    .await;
    let market_route = route(
        &mut server,
        &format!("/markets/{}", condition()),
        "",
        &fixture("market"),
        2,
    )
    .await;
    let client = client(&server);
    let market = client.markets().get(condition()).send().await.unwrap();
    let outcome = market
        .tokens
        .iter()
        .find(|t| t.token_id.as_deref() == Some(token().as_str()))
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let instrument = venue.instrument(&key()).await.unwrap();
    by_token.assert_async().await;
    market_route.assert_async().await;

    assert_eq!(instrument.key, key());
    assert_eq!(instrument.size_unit, SizeUnit::Shares);
    assert!(
        instrument.name.contains(&outcome.outcome),
        "{}",
        instrument.name
    );
    assert!(
        instrument
            .name
            .contains(market.question.as_deref().unwrap()),
        "{}",
        instrument.name
    );
    assert_eq!(instrument.tick_size, market.minimum_tick_size.map(decimal));
    assert_eq!(instrument.min_size, market.minimum_order_size.map(decimal));
    assert_eq!(instrument.tradable, market.accepting_orders);
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(
        ext.condition_id.as_deref(),
        Some(market.condition_id.as_str())
    );
    assert_eq!(ext.outcome.as_deref(), Some(outcome.outcome.as_str()));
    assert_eq!(ext.neg_risk, market.neg_risk);
    assert_eq!(ext.closed, Some(market.closed));
}

#[tokio::test]
async fn a_token_the_clob_does_not_know_is_its_404() {
    let mut server = Server::new_async().await;
    let unknown = "1";
    let mock = server
        .mock("GET", format!("/markets-by-token/{unknown}").as_str())
        .with_status(404)
        .with_body(r#"{"error":"market not found"}"#)
        .expect(1)
        .create_async()
        .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue
        .instrument(&keys::clob(&TokenId::new(unknown).unwrap()))
        .await
        .unwrap_err();
    mock.assert_async().await;
    assert!(
        matches!(err.class, Class::VenueRefusal { .. }),
        "{:?}",
        err.class
    );
    assert!(err.source.downcast_ref::<ClobError>().is_some());
}

#[tokio::test]
async fn candles_are_the_native_price_history_one_price_an_interval() {
    let mut server = Server::new_async().await;
    let (start, end) = (1_791_914_757_u64, 1_792_173_957_u64);
    let query = format!("market={}&fidelity=60&startTs={start}&endTs={end}", token());
    let mock = route(
        &mut server,
        "/prices-history",
        &query,
        &fixture("prices_history"),
        2,
    )
    .await;
    let client = client(&server);
    let params = PricesHistoryQuery {
        interval: None,
        fidelity: Some(60),
        start_ts: Some(start as i64),
        end_ts: Some(end as i64),
    };
    let native = client
        .markets()
        .prices_history_with(token(), &params)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(start * 1_000))
        .end(UnixMillis(end * 1_000));
    let candles = venue.candles(&key(), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.history.len());
    assert!(!candles.is_empty());
    for (candle, point) in candles.iter().zip(&native.history) {
        assert_eq!(candle.interval, CandleInterval::Hour1);
        assert_eq!(candle.open_time, UnixMillis(point.timestamp as u64 * 1_000));
        assert_eq!(candle.close, decimal(point.price));
        assert_eq!((candle.open, candle.high, candle.low), (None, None, None));
        assert_eq!(candle.volume, None);
    }
}

#[tokio::test]
async fn trades_are_the_data_apis_public_trades_of_the_token_oldest_first() {
    let mut server = Server::new_async().await;
    let by_token = route(
        &mut server,
        &format!("/markets-by-token/{}", token()),
        "",
        &fixture("markets_by_token"),
        1,
    )
    .await;
    let query = format!("market={}&takerOnly=true", condition());
    let trades_route = route(&mut server, "/trades", &query, &fixture("trades"), 2).await;
    let client = client(&server);
    let native = DataApi::builder()
        .base_url(server.url())
        .build()
        .unwrap()
        .trades()
        .list()
        .market([condition()])
        .taker_only(true)
        .send()
        .await
        .unwrap();
    let expected: Vec<_> = native
        .iter()
        .rev()
        .filter(|row| row.asset == token())
        .collect();
    assert!(
        native.iter().any(|row| row.asset != token()),
        "the capture holds another outcome's trades, which the trait leaves out"
    );

    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);
    let trades = venue.trades(&key()).await.unwrap();
    by_token.assert_async().await;
    trades_route.assert_async().await;

    assert_eq!(trades.len(), expected.len());
    for (trade, row) in trades.iter().zip(expected) {
        assert_eq!(trade.key, key());
        assert_eq!(trade.id, None, "the Data API gives a trade no id");
        assert_eq!(trade.price, decimal(row.price));
        assert_eq!(trade.size, decimal(row.size));
        assert_eq!(trade.time, UnixMillis(row.timestamp as u64 * 1_000));
        assert_eq!(
            trade.taker_side,
            match row.side {
                TradeSide::Buy => Some(Side::Buy),
                TradeSide::Sell => Some(Side::Sell),
                _ => None,
            }
        );
        let ext = trade.extensions.get::<TradeExt>().unwrap();
        assert_eq!(ext.transaction_hash, row.transaction_hash);
        assert_eq!(ext.outcome.as_deref(), Some(row.outcome.as_str()));
    }
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn event_group_holds_every_v1_outcome_of_the_tokens_event() {
    let mut server = Server::new_async().await;
    let markets_query = format!("clob_token_ids={}", token());
    let markets = route(
        &mut server,
        "/markets",
        &markets_query,
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let event_id = json("gamma_markets")[0]["events"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let event_route = route(
        &mut server,
        &format!("/events/{event_id}"),
        "",
        &fixture("gamma_event"),
        2,
    )
    .await;
    let client = client(&server);
    let event = Gamma::builder()
        .base_url(server.url())
        .build()
        .unwrap()
        .events()
        .get(event_id.clone())
        .send()
        .await
        .unwrap();

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client);
    let group = venue.event_group(&key()).await.unwrap();
    markets.assert_async().await;
    event_route.assert_async().await;

    assert_eq!(group.id, event.id);
    assert_eq!(group.title, event.title);
    let mut expected = Vec::new();
    // Only a v1 market's outcome ids are the CLOB's tokens.
    for market in event
        .markets
        .iter()
        .filter(|market| market.version == Some(ProtocolVersion::V1))
    {
        let tokens: Vec<String> =
            serde_json::from_str(market.clob_token_ids.as_deref().unwrap()).unwrap();
        let labels: Vec<String> =
            serde_json::from_str(market.outcomes.as_deref().unwrap()).unwrap();
        for (token, label) in tokens.iter().zip(&labels) {
            expected.push((
                keys::clob(&TokenId::new(token.as_str()).unwrap()),
                market.condition_id.clone(),
                label.clone(),
                market.group_item_title.clone(),
            ));
        }
    }
    let actual: Vec<_> = group
        .outcomes
        .iter()
        .map(|o| {
            (
                o.key.clone(),
                o.market.clone(),
                o.label.clone(),
                o.market_title.clone(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert!(
        group.outcomes.iter().any(|o| o.key == key()),
        "the token's own outcome"
    );
    let ext = group.extensions.get::<EventGroupExt>().unwrap();
    assert_eq!(ext.slug, event.slug);
    assert_eq!(ext.neg_risk, event.neg_risk);
}

/// The captured event with its markets edited by `edit`, as the body of
/// `/events/{id}`.
fn event_body(edit: impl FnOnce(&mut Vec<Value>)) -> String {
    let mut event = json("gamma_event");
    edit(event["markets"].as_array_mut().unwrap());
    event.to_string()
}

/// The id of the event the captured token's market is in.
fn event_id() -> String {
    json("gamma_markets")[0]["events"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn event_group_finds_a_resolved_markets_token_under_closed_true() {
    let mut server = Server::new_async().await;
    // Gamma omits a closed market unless asked for it.
    let open = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}", token()),
        "[]",
        1,
    )
    .await;
    let closed = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}&closed=true", token()),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &fixture("gamma_event"),
        1,
    )
    .await;

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue.event_group(&key()).await.unwrap();
    open.assert_async().await;
    closed.assert_async().await;
    event.assert_async().await;
    assert_eq!(group.id, event_id());
    assert!(group.outcomes.iter().any(|o| o.key == key()));
}

#[tokio::test]
async fn event_group_leaves_out_the_outcomes_of_a_v2_market() {
    let mut server = Server::new_async().await;
    let markets = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}", token()),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let body = event_body(|markets| markets[1]["version"] = "v2".into());
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &body,
        1,
    )
    .await;
    let v2_condition = json("gamma_event")["markets"][1]["conditionId"]
        .as_str()
        .unwrap()
        .to_owned();

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue.event_group(&key()).await.unwrap();
    markets.assert_async().await;
    event.assert_async().await;

    let conditions: Vec<&str> = group.outcomes.iter().map(|o| o.market.as_str()).collect();
    assert!(
        !conditions.contains(&v2_condition.as_str()),
        "{conditions:?}"
    );
    assert_eq!(group.outcomes.len(), 4, "the two v1 markets' Yes and No");
}

#[tokio::test]
async fn a_market_whose_tokens_and_outcomes_differ_in_number_is_a_decode_failure() {
    let mut server = Server::new_async().await;
    let markets = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}", token()),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let body = event_body(|markets| markets[0]["outcomes"] = r#"["Yes"]"#.into());
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &body,
        1,
    )
    .await;

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let err = venue.event_group(&key()).await.unwrap_err();
    markets.assert_async().await;
    event.assert_async().await;
    assert_eq!(err.class, Class::Decode, "{err}");
}

#[tokio::test]
async fn a_token_id_gamma_sends_that_no_key_can_carry_is_a_decode_failure() {
    let mut server = Server::new_async().await;
    let markets = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}", token()),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let body = event_body(|markets| {
        markets[0]["clobTokenIds"] = r#"["not-a-token-id", "2"]"#.into();
    });
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &body,
        1,
    )
    .await;

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let err = venue.event_group(&key()).await.unwrap_err();
    markets.assert_async().await;
    event.assert_async().await;
    // The venue sent a value it cannot key, which is not the caller's fault.
    assert_eq!(err.class, Class::Decode, "{err}");
}

#[tokio::test]
async fn a_token_gamma_lists_under_neither_request_is_not_listed() {
    let mut server = Server::new_async().await;
    let open = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}", token()),
        "[]",
        1,
    )
    .await;
    let closed = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}&closed=true", token()),
        "[]",
        1,
    )
    .await;

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let err = venue.event_group(&key()).await.unwrap_err();
    open.assert_async().await;
    closed.assert_async().await;
    assert!(
        matches!(err.class, Class::VenueRefusal { .. }),
        "{:?}",
        err.class
    );
    assert!(err.source.downcast_ref::<NotListed>().is_some());
}

#[tokio::test]
async fn an_empty_group_item_title_falls_back_to_the_question() {
    let mut server = Server::new_async().await;
    let markets = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}", token()),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let body = event_body(|markets| markets[0]["groupItemTitle"] = "".into());
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &body,
        1,
    )
    .await;
    let question = json("gamma_event")["markets"][0]["question"]
        .as_str()
        .unwrap()
        .to_owned();

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue.event_group(&key()).await.unwrap();
    markets.assert_async().await;
    event.assert_async().await;
    assert_eq!(
        group.outcomes[0].market_title.as_deref(),
        Some(question.as_str())
    );
}

/// The 14 days `/prices-history` serves at most, in seconds.
const WINDOW: u64 = 1_209_600;

/// Candles asked for `query`, against a history route that must see `window`.
async fn candles_sent_as(query: CandleQuery, window: &str) {
    let mut server = Server::new_async().await;
    let route = route(
        &mut server,
        "/prices-history",
        &format!("market={}&fidelity=60&{window}", token()),
        r#"{"history":[]}"#,
        1,
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    venue.candles(&key(), &query).await.unwrap();
    route.assert_async().await;
}

#[tokio::test]
async fn candles_ask_for_at_most_the_14_days_the_host_serves() {
    let start = 1_700_000_000_u64;
    let window = format!("startTs={start}&endTs={}", start + WINDOW);
    // No end, which is now.
    let open_ended = CandleQuery::new(CandleInterval::Hour1, UnixMillis(start * 1_000));
    candles_sent_as(open_ended, &window).await;
    // An end beyond the window.
    let far = CandleQuery::new(CandleInterval::Hour1, UnixMillis(start * 1_000))
        .end(UnixMillis((start + 3 * WINDOW) * 1_000));
    candles_sent_as(far, &window).await;
    // An end inside it is kept, rounded down.
    let near = CandleQuery::new(CandleInterval::Hour1, UnixMillis(start * 1_000))
        .end(UnixMillis((start + 3_600) * 1_000 + 999));
    candles_sent_as(near, &format!("startTs={start}&endTs={}", start + 3_600)).await;
}

#[tokio::test]
async fn candles_start_at_the_next_whole_second() {
    // A point in the sub-second before `start` must not come back.
    let start = 1_791_914_757_001_u64;
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(start))
        .end(UnixMillis(1_792_173_957_000));
    candles_sent_as(query, "startTs=1791914758&endTs=1792173957").await;
}

/// A product of another venue, to hand the CLOB a key it does not own.
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
async fn a_key_of_another_product_is_refused_by_every_trait_before_anything_is_sent() {
    let mut server = Server::new_async().await;
    let untouched = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let client = client(&server);
    let foreign = Elsewhere::key(&"1".to_owned());
    let market: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client.clone());
    let trades: Arc<DynTrades<'static>> = DynTrades::new_arc(client.clone());
    let candles: Arc<DynCandles<'static>> = DynCandles::new_arc(client.clone());
    let events: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_790_675_505_663));
    for err in [
        market.quote(&foreign).await.unwrap_err(),
        market.book(&foreign).await.unwrap_err(),
        market.instrument(&foreign).await.unwrap_err(),
        trades.trades(&foreign).await.unwrap_err(),
        candles.candles(&foreign, &request).await.unwrap_err(),
        events.event_group(&foreign).await.unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
    }
    untouched.assert_async().await;
}

#[tokio::test]
async fn a_trait_error_carries_the_native_errors_class_fault_and_wait() {
    let cases: [(usize, &str, Option<&str>); 3] = [
        (429, r#"{"error":"Too Many Requests"}"#, Some("2")),
        (503, "upstream unavailable", None),
        (404, r#"{"error":"market not found"}"#, None),
    ];
    for (status, body, retry_after) in cases {
        let mut server = Server::new_async().await;
        let mut mock = server
            .mock("GET", "/book")
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(body)
            .expect(2);
        if let Some(wait) = retry_after {
            mock = mock.with_header("retry-after", wait);
        }
        let mock = mock.create_async().await;

        // One client apiece, so the native call's hold is not the trait call's,
        // and no retries, so the 429 comes back at once.
        let build = || {
            ClobBuilder::new()
                .base_url(server.url())
                .with_retry_config(polyoxide_core::RetryConfig {
                    max_retries: 0,
                    initial_backoff_ms: 1,
                    max_backoff_ms: 1,
                })
                .gamma(Gamma::builder().base_url(server.url()).build().unwrap())
                .data(DataApi::builder().base_url(server.url()).build().unwrap())
                .build()
                .unwrap()
        };
        let native = build()
            .markets()
            .order_book(token())
            .send()
            .await
            .unwrap_err();
        let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(build());
        let err = venue.quote(&key()).await.unwrap_err();
        mock.assert_async().await;

        assert_eq!(err.class, native.class(), "{status}");
        assert_eq!(err.is_fault(), native.is_fault(), "{status}");
        assert_eq!(err.retry_after(), native.retry_after(), "{status}");
        assert!(err.source.downcast_ref::<ClobError>().is_some(), "{status}");
    }
}
