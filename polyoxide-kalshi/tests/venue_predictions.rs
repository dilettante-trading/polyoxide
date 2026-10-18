//! Kalshi's `predictions` module through the venue-neutral traits, each held as
//! its `Dyn` wrapper, against mock servers serving the captured fixtures: every
//! method returns the values the native client decodes from the same body, and
//! the YES/NO conversion is held to literal prices.

#[allow(dead_code)]
#[path = "../../polyoxide/examples/venue_neutral.rs"]
mod example;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use example::{best_bid_ask, classify, classify_classified, Handling};
use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::{
    key::{self, Outcome, Ticker},
    types::{MarketStatus, PriceLevel},
    venue::{EventGroupExt, InstrumentExt, TradeExt},
    Predictions,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    Book, Class, ClassifiedError, DynMarketData, DynTrades, Instrument, KeySpace, Level,
    MarketData, NotListed, ProductId, SizeUnit, UnixMillis, VenueId,
};
use polyoxide_venue::{DynEventGroups, EventGroups};
use polyoxide_venue::{Side, TradeId, Trades};
use rust_decimal::Decimal;
use serde_json::{json, Value};

/// A ticker of this suite's own, for the tests that serve an inline body.
const TICKER: &str = "KXTEST-26OCT18-T50.5";

/// A book as Kalshi serves it: each side's levels in ascending price, worst
/// first. Four bids: YES 0.40 and 0.45, NO 0.50 and 0.52.
const BOOK: &str = r#"{"orderbook_fp":{"yes_dollars":[["0.4000","10.00"],["0.4500","5.00"]],"no_dollars":[["0.5000","7.00"],["0.5200","3.00"]]}}"#;

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn fixture_json(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

/// No retries, so a refused request answers at once.
fn once() -> RetryConfig {
    RetryConfig {
        max_retries: 0,
        ..RetryConfig::default()
    }
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

/// A server with the account's `endpoint_costs` mounted and never asserted: a
/// client built with Story 4 reads it on its first send, and one built before
/// never asks.
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

/// A route answering 200 with `body` `hits` times, matched on its whole query.
async fn route(
    server: &mut ServerGuard,
    path: &str,
    query: Matcher,
    body: &str,
    hits: usize,
) -> Mock {
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

/// A server that fails the test if it is asked for anything.
async fn silent() -> (ServerGuard, Mock) {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    (server, nothing)
}

fn test_ticker() -> Ticker {
    Ticker::new(TICKER).unwrap()
}

/// The ticker of a `{"market": {...}}` body.
fn ticker_of(body: &Value) -> Ticker {
    Ticker::new(body["market"]["ticker"].as_str().unwrap()).unwrap()
}

fn market_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/markets/{ticker}")
}

fn book_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/markets/{ticker}/orderbook")
}

fn lv(price: &str, size: &str) -> Level {
    Level::new(
        price.parse::<Decimal>().unwrap(),
        size.parse::<Decimal>().unwrap(),
    )
}

/// The levels an opposite book's bids are, as asks: each price `p` is `1 - p`.
fn flipped(levels: &[Level]) -> Vec<Level> {
    levels
        .iter()
        .map(|level| Level::new(Decimal::ONE - level.price, level.size))
        .collect()
}

/// The two outcomes' books of one market are one book read from both sides.
fn assert_mirrored(yes: &Book, no: &Book) {
    assert_eq!(yes.bids, flipped(&no.asks));
    assert_eq!(yes.asks, flipped(&no.bids));
}

/// A book with both sides, whose best bid is below its best ask.
fn assert_uncrossed(book: &Book) {
    let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) else {
        panic!("{}: a side of the book is empty: {book:?}", book.key);
    };
    assert!(
        bid.price < ask.price,
        "{}: {bid:?} crosses {ask:?}",
        book.key
    );
}

/// The highest-priced level of one side, as (price, count).
fn best(levels: &[PriceLevel]) -> (Decimal, Decimal) {
    let top = levels
        .iter()
        .max_by_key(|level| level.price)
        .expect("the captured book has both sides");
    (top.price, top.count)
}

/// `instrument` on a mock server serving `body` for the test ticker's market.
async fn instrument_of(body: &Value, outcome: Outcome) -> Result<Instrument, ClassifiedError> {
    let ticker = ticker_of(body);
    let mut server = start().await;
    let _market = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &body.to_string(),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    venue.instrument(&key::key(&ticker, outcome)).await
}

/// A key of a product that is not Kalshi's.
struct Other;

impl KeySpace for Other {
    const VENUE: VenueId = VenueId::from_static("other");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

fn foreign_key() -> polyoxide_venue::MarketKey {
    Other::key(&"ABC".to_owned())
}

#[tokio::test]
async fn a_dyn_market_data_returns_the_native_instrument_of_both_outcomes() {
    // Kalshi sends one subtitle for both outcomes; these differ, so a name
    // taken from the wrong one, or without its outcome, cannot pass.
    let mut body = fixture_json("market");
    body["market"]["yes_sub_title"] = json!("Alpha wins");
    body["market"]["no_sub_title"] = json!("Beta wins");
    let ticker = ticker_of(&body);
    let mut server = start().await;
    let mock = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &body.to_string(),
        3,
    )
    .await;
    let client = client(&server);
    let native = client.markets().get(&ticker).send().await.unwrap().market;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);

    for outcome in [Outcome::Yes, Outcome::No] {
        let k = key::key(&ticker, outcome);
        let instrument = venue.instrument(&k).await.unwrap();
        assert_eq!(instrument.key, k);
        let name = match outcome {
            Outcome::Yes => "Alpha wins — Yes",
            Outcome::No => "Alpha wins — No",
        };
        assert_eq!(instrument.name, name);
        assert_eq!(instrument.size_unit, SizeUnit::Shares);
        assert_eq!(instrument.base_asset, None);
        assert_eq!(instrument.quote_asset.as_deref(), Some("USD"));
        assert_eq!(instrument.min_size, Some(Decimal::new(1, 2)));
        assert_eq!(instrument.size_increment, Some(Decimal::new(1, 2)));
        assert_eq!(instrument.min_notional, None);
        assert_eq!(
            instrument.tick_size,
            match native.price_ranges.as_slice() {
                [only] => Some(only.step),
                _ => None,
            }
        );
        assert_eq!(
            instrument.tradable,
            Some(native.status == MarketStatus::Active)
        );

        let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
        assert_eq!(
            ext.event_ticker.as_deref(),
            Some(native.event_ticker.as_str())
        );
        assert_eq!(ext.status.as_ref(), Some(&native.status));
        assert_eq!(ext.price_ranges.as_ref(), Some(&native.price_ranges));
        assert_eq!(
            ext.price_level_structure.as_deref(),
            Some(native.price_level_structure.as_str())
        );
        assert_eq!(ext.notional_value, Some(native.notional_value_dollars));
        assert_eq!(
            ext.settlement_bounds_type.as_deref(),
            Some(native.settlement_bounds_type.as_str())
        );
        assert_eq!(ext.exchange_index, native.exchange_index);
        assert!(ext.close_time.is_some() && ext.latest_expiration_time.is_some());
        assert_eq!(
            ext.expected_expiration_time.is_some(),
            native.expected_expiration_time.is_some()
        );
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn a_markets_status_and_price_ranges_decide_tradable_and_tick_size() {
    let mut body = fixture_json("market");
    body["market"]["status"] = json!("active");
    body["market"]["price_ranges"] =
        json!([{"start": "0.0000", "end": "1.0000", "step": "0.0100"}]);
    let one = instrument_of(&body, Outcome::Yes).await.unwrap();
    assert_eq!(one.tick_size, Some(Decimal::new(1, 2)));
    assert_eq!(one.tradable, Some(true));
    let ext = one.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.status, Some(MarketStatus::Active));

    // Two ranges have no one step, and a closed market takes no orders.
    body["market"]["status"] = json!("closed");
    body["market"]["price_ranges"] = json!([
        {"start": "0.0000", "end": "0.1000", "step": "0.0010"},
        {"start": "0.1000", "end": "1.0000", "step": "0.0100"}
    ]);
    let two = instrument_of(&body, Outcome::Yes).await.unwrap();
    assert_eq!(two.tick_size, None);
    assert_eq!(two.tradable, Some(false));

    // The name carries the outcome and takes the YES subtitle for both.
    body["market"]["yes_sub_title"] = json!("Alpha wins");
    body["market"]["no_sub_title"] = json!("Beta wins");
    let yes = instrument_of(&body, Outcome::Yes).await.unwrap();
    assert_eq!(yes.name, "Alpha wins — Yes");
    let no = instrument_of(&body, Outcome::No).await.unwrap();
    assert_eq!(no.name, "Alpha wins — No");

    // An empty or whitespace-only YES subtitle falls back to the ticker and
    // the outcome, whatever the NO subtitle says.
    let ticker = ticker_of(&body).to_string();
    for blank in ["", "   "] {
        body["market"]["yes_sub_title"] = json!(blank);
        let yes = instrument_of(&body, Outcome::Yes).await.unwrap();
        assert_eq!(yes.name, format!("{ticker} yes"));
        let no = instrument_of(&body, Outcome::No).await.unwrap();
        assert_eq!(no.name, format!("{ticker} no"));
    }
}

#[tokio::test]
async fn the_instruments_times_are_rfc_3339_and_one_that_is_not_is_a_decode_failure() {
    let mut body = fixture_json("market");
    body["market"]["close_time"] = json!("2026-10-18T09:10:11.250Z");
    body["market"]["latest_expiration_time"] = json!("2026-10-18T11:10:12+02:00");
    body["market"]["expected_expiration_time"] = json!("2026-10-18T10:10:11Z");
    let instrument = instrument_of(&body, Outcome::Yes).await.unwrap();
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.close_time, Some(UnixMillis(1_792_314_611_250)));
    assert_eq!(
        ext.latest_expiration_time,
        Some(UnixMillis(1_792_314_612_000))
    );
    assert_eq!(
        ext.expected_expiration_time,
        Some(UnixMillis(1_792_318_211_000))
    );

    // No expected expiration is none, not a failure.
    body["market"]["expected_expiration_time"] = Value::Null;
    let instrument = instrument_of(&body, Outcome::Yes).await.unwrap();
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.expected_expiration_time, None);

    // A time that does not parse names its field; it is never a silent zero.
    body["market"]["close_time"] = json!("tomorrow");
    let err = instrument_of(&body, Outcome::Yes).await.unwrap_err();
    assert_eq!(err.class, Class::Decode);
    assert!(err.to_string().contains("close_time"), "{err}");
}

#[tokio::test]
async fn an_expiration_time_that_does_not_parse_is_a_decode_failure_naming_its_field() {
    for field in ["latest_expiration_time", "expected_expiration_time"] {
        let mut body = fixture_json("market");
        body["market"][field] = json!("tomorrow");
        let err = instrument_of(&body, Outcome::Yes).await.unwrap_err();
        assert_eq!(err.class, Class::Decode, "{field}");
        assert!(err.to_string().contains(field), "{field}: {err}");
    }
}

#[tokio::test]
async fn a_dyn_market_data_returns_the_native_quote_of_both_outcomes() {
    let body = fixture("orderbook_depth1");
    let ticker = ticker_of(&fixture_json("market"));
    let mut server = start().await;
    let mock = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Exact("depth=1".to_owned()),
        &body,
        3,
    )
    .await;
    let client = client(&server);
    let native = client
        .markets()
        .orderbook(&ticker)
        .depth(1)
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    assert!(
        !native.yes_dollars.is_empty() && !native.no_dollars.is_empty(),
        "the captured depth-1 book has both sides"
    );
    let (yes_price, yes_count) = best(&native.yes_dollars);
    let (no_price, no_count) = best(&native.no_dollars);
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);

    let yes = venue.quote(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    assert_eq!(yes.bid, Some(Level::new(yes_price, yes_count)));
    assert_eq!(yes.ask, Some(Level::new(Decimal::ONE - no_price, no_count)));
    assert_eq!(yes.time, None);
    let no = venue.quote(&key::key(&ticker, Outcome::No)).await.unwrap();
    assert_eq!(no.bid, Some(Level::new(no_price, no_count)));
    assert_eq!(
        no.ask,
        Some(Level::new(Decimal::ONE - yes_price, yes_count))
    );

    // Neither quote crosses, and the two outcomes are one quote read twice.
    assert!(yes.bid.unwrap().price < yes.ask.unwrap().price);
    assert!(no.bid.unwrap().price < no.ask.unwrap().price);
    assert_eq!(
        yes.bid,
        no.ask
            .map(|ask| Level::new(Decimal::ONE - ask.price, ask.size))
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn a_dyn_market_data_returns_the_native_book_of_both_outcomes() {
    let body = fixture("orderbook");
    let ticker = ticker_of(&fixture_json("market"));
    let mut server = start().await;
    let mock = route(&mut server, &book_path(&ticker), Matcher::Missing, &body, 3).await;
    let client = client(&server);
    let native = client
        .markets()
        .orderbook(&ticker)
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    assert!(
        !native.yes_dollars.is_empty() && !native.no_dollars.is_empty(),
        "the captured book has both sides"
    );
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);

    let yes_levels: Vec<Level> = native
        .yes_dollars
        .iter()
        .map(|level| Level::new(level.price, level.count))
        .collect();
    let no_levels: Vec<Level> = native
        .no_dollars
        .iter()
        .map(|level| Level::new(level.price, level.count))
        .collect();
    let highest_first = |mut levels: Vec<Level>| {
        levels.sort_by_key(|level| std::cmp::Reverse(level.price));
        levels
    };
    let lowest_first = |mut levels: Vec<Level>| {
        levels.sort_by_key(|level| level.price);
        levels
    };

    let yes = venue.book(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    assert_eq!(yes.bids, highest_first(yes_levels.clone()));
    assert_eq!(yes.asks, lowest_first(flipped(&no_levels)));
    let no = venue.book(&key::key(&ticker, Outcome::No)).await.unwrap();
    assert_eq!(no.bids, highest_first(no_levels.clone()));
    assert_eq!(no.asks, lowest_first(flipped(&yes_levels)));

    assert_uncrossed(&yes);
    assert_uncrossed(&no);
    assert_mirrored(&yes, &no);
    mock.assert_async().await;
}

#[tokio::test]
async fn a_no_bid_is_a_yes_ask_at_one_minus_its_price() {
    let ticker = test_ticker();
    let mut server = start().await;
    let depth1 = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Exact("depth=1".to_owned()),
        BOOK,
        2,
    )
    .await;
    let full = route(&mut server, &book_path(&ticker), Matcher::Missing, BOOK, 2).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let yes_key = key::key(&ticker, Outcome::Yes);
    let no_key = key::key(&ticker, Outcome::No);

    // YES bids 0.45 and 0.40; NO bids 0.52 and 0.50 are YES asks at 0.48 and 0.50.
    let yes = venue.book(&yes_key).await.unwrap();
    assert_eq!(yes.bids, [lv("0.45", "5"), lv("0.40", "10")]);
    assert_eq!(yes.asks, [lv("0.48", "3"), lv("0.50", "7")]);
    // NO bids 0.52 and 0.50; YES bids 0.45 and 0.40 are NO asks at 0.55 and 0.60.
    let no = venue.book(&no_key).await.unwrap();
    assert_eq!(no.bids, [lv("0.52", "3"), lv("0.50", "7")]);
    assert_eq!(no.asks, [lv("0.55", "5"), lv("0.60", "10")]);
    assert_mirrored(&yes, &no);

    let yes_quote = venue.quote(&yes_key).await.unwrap();
    assert_eq!(yes_quote.bid, Some(lv("0.45", "5")));
    assert_eq!(yes_quote.ask, Some(lv("0.48", "3")));
    let no_quote = venue.quote(&no_key).await.unwrap();
    assert_eq!(no_quote.bid, Some(lv("0.52", "3")));
    assert_eq!(no_quote.ask, Some(lv("0.55", "5")));
    depth1.assert_async().await;
    full.assert_async().await;
}

#[tokio::test]
async fn a_one_sided_book_leaves_the_other_side_of_the_quote_empty() {
    let ticker = test_ticker();
    let mut server = start().await;
    let _book = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Any,
        r#"{"orderbook_fp":{"yes_dollars":[["0.4000","10.00"]],"no_dollars":[]}}"#,
        2,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));

    let yes = venue.quote(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    assert_eq!(yes.bid, Some(lv("0.40", "10")));
    assert_eq!(yes.ask, None);
    let no = venue.quote(&key::key(&ticker, Outcome::No)).await.unwrap();
    assert_eq!(no.bid, None);
    assert_eq!(no.ask, Some(lv("0.60", "10")));
}

#[tokio::test]
async fn an_unknown_ticker_is_not_listed_but_its_quote_and_book_are_empty() {
    let ticker = test_ticker();
    let mut server = start().await;
    let market = server
        .mock("GET", market_path(&ticker).as_str())
        .with_status(404)
        .with_body(fixture("error_not_found"))
        .expect(2)
        .create_async()
        .await;
    // Kalshi answers an unknown ticker's orderbook with 200 and two empty lists.
    let books = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Any,
        r#"{"orderbook_fp":{"no_dollars":[],"yes_dollars":[]}}"#,
        4,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));

    for outcome in [Outcome::Yes, Outcome::No] {
        let k = key::key(&ticker, outcome);
        let err = venue.instrument(&k).await.unwrap_err();
        assert_eq!(err.class, Class::VenueRefusal { code: None });
        assert_eq!(err.source.downcast_ref::<NotListed>().unwrap().key, k);

        let quote = venue.quote(&k).await.unwrap();
        assert_eq!((quote.bid, quote.ask), (None, None));
        let book = venue.book(&k).await.unwrap();
        assert!(book.bids.is_empty() && book.asks.is_empty(), "{book:?}");
    }
    market.assert_async().await;
    books.assert_async().await;
}

#[tokio::test]
async fn only_a_404_is_not_listed() {
    let ticker = test_ticker();
    let k = key::key(&ticker, Outcome::Yes);

    // A 404 whose body is none of Kalshi's three shapes stays a core error and
    // is still a missing market.
    let mut server = start().await;
    let _plain = server
        .mock("GET", market_path(&ticker).as_str())
        .with_status(404)
        .with_body("not found")
        .create_async()
        .await;
    let venue = DynMarketData::new_arc(client(&server));
    let err = venue.instrument(&k).await.unwrap_err();
    assert!(err.source.downcast_ref::<NotListed>().is_some(), "{err}");

    for status in [400, 401, 429, 503] {
        let mut server = start().await;
        let _refusal = server
            .mock("GET", market_path(&ticker).as_str())
            .with_status(status)
            .with_body(r#"{"error":{"code":"refused","message":"refused"}}"#)
            .create_async()
            .await;
        let venue = DynMarketData::new_arc(client(&server));
        let err = venue.instrument(&k).await.unwrap_err();
        assert!(
            err.source.downcast_ref::<NotListed>().is_none(),
            "{status} is not a missing market: {err}"
        );
    }
}

#[tokio::test]
async fn a_key_of_another_venue_is_refused_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let foreign = foreign_key();

    for err in [
        venue.instrument(&foreign).await.unwrap_err(),
        venue.quote(&foreign).await.unwrap_err(),
        venue.book(&foreign).await.unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest);
    }
    nothing.assert_async().await;
}

#[tokio::test]
async fn best_bid_ask_runs_on_a_dyn_market_data_over_kalshi() {
    let ticker = test_ticker();
    let mut server = start().await;
    let mock = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Exact("depth=1".to_owned()),
        BOOK,
        2,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));

    let yes = best_bid_ask(&venue, &key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    assert_eq!(yes, (Some(lv("0.45", "5")), Some(lv("0.48", "3"))));
    let no = best_bid_ask(&venue, &key::key(&ticker, Outcome::No))
        .await
        .unwrap();
    assert_eq!(no, (Some(lv("0.52", "3")), Some(lv("0.55", "5"))));
    mock.assert_async().await;
}

#[tokio::test]
async fn one_classifier_reads_the_errors_the_traits_return_as_it_reads_the_native_ones() {
    let ticker = test_ticker();
    let cases = [
        (
            429,
            fixture("error_rate_limited"),
            None,
            Handling::Backoff(None),
        ),
        (
            429,
            fixture("error_rate_limited"),
            Some("7"),
            Handling::Backoff(Some(Duration::from_secs(7))),
        ),
        (
            503,
            r#"{"error":{"code":"service_unavailable","message":"try later"}}"#.to_owned(),
            None,
            Handling::Retry,
        ),
        (
            401,
            fixture("error_unauthenticated"),
            None,
            Handling::Reauthenticate,
        ),
        (
            400,
            fixture("error_validation"),
            None,
            Handling::Rejected { alert: true },
        ),
        (
            404,
            fixture("error_not_found"),
            None,
            Handling::Rejected { alert: true },
        ),
    ];
    for (status, body, retry_after, expected) in cases {
        let mut server = start().await;
        let mut mock = server
            .mock("GET", book_path(&ticker).as_str())
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(body);
        if let Some(wait) = retry_after {
            mock = mock.with_header("retry-after", wait);
        }
        let _mock = mock.create_async().await;

        let native = client(&server)
            .markets()
            .orderbook(&ticker)
            .depth(1)
            .send()
            .await
            .unwrap_err();
        assert_eq!(classify(&native), expected, "{status}, native");

        let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
        let err = best_bid_ask(&venue, &key::key(&ticker, Outcome::Yes))
            .await
            .unwrap_err();
        assert_eq!(
            classify_classified(&err),
            expected,
            "{status}, through the traits"
        );
    }
}

/// A trades route: the ticker and the 1000-row limit, in either order.
fn trades_query(ticker: &Ticker) -> Matcher {
    Matcher::AllOf(vec![
        Matcher::UrlEncoded("ticker".to_owned(), ticker.to_string()),
        Matcher::UrlEncoded("limit".to_owned(), "1000".to_owned()),
    ])
}

/// A copy of the fixture's first trade with the fields a test sets.
fn trade_row(
    template: &Value,
    id: &str,
    time: &str,
    yes: &str,
    no: &str,
    taker: &str,
    block: bool,
) -> Value {
    let mut row = template.clone();
    row["trade_id"] = json!(id);
    row["created_time"] = json!(time);
    row["yes_price_dollars"] = json!(yes);
    row["no_price_dollars"] = json!(no);
    row["count_fp"] = json!("5.00");
    row["taker_outcome_side"] = json!(taker);
    row["is_block_trade"] = json!(block);
    row
}

/// `trades` on a mock server serving `body`, for the test ticker.
async fn trades_of(
    body: &Value,
    outcome: Outcome,
) -> Result<Vec<polyoxide_venue::Trade>, ClassifiedError> {
    let ticker = test_ticker();
    let mut server = start().await;
    let _trades = route(
        &mut server,
        "/trade-api/v2/markets/trades",
        trades_query(&ticker),
        &body.to_string(),
        1,
    )
    .await;
    let venue = DynTrades::new_arc(client(&server));
    venue.trades(&key::key(&ticker, outcome)).await
}

#[tokio::test]
async fn a_dyn_trades_returns_the_native_trades_oldest_first() {
    let body = fixture("trades");
    let ticker = ticker_of(&fixture_json("market"));
    let mut server = start().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/trades",
        trades_query(&ticker),
        &body,
        3,
    )
    .await;
    let client = client(&server);
    let native = client
        .markets()
        .trades()
        .ticker(&ticker)
        .limit(1000)
        .send()
        .await
        .unwrap();
    assert!(
        native.trades.len() > 1,
        "the captured page holds several trades"
    );
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);

    for outcome in [Outcome::Yes, Outcome::No] {
        let k = key::key(&ticker, outcome);
        let trades = venue.trades(&k).await.unwrap();
        assert_eq!(trades.len(), native.trades.len());
        // Kalshi serves newest first; the trait is oldest first.
        for (trade, row) in trades.iter().zip(native.trades.iter().rev()) {
            assert_eq!(trade.key, k);
            assert_eq!(
                trade.price,
                match outcome {
                    Outcome::Yes => row.yes_price_dollars,
                    Outcome::No => row.no_price_dollars,
                }
            );
            assert_eq!(trade.size, row.count_fp);
            assert_eq!(trade.id, Some(TradeId::new(row.trade_id.clone())));
            assert!(trade.taker_side.is_some(), "{row:?}");
            let ext = trade.extensions.get::<TradeExt>().unwrap();
            assert_eq!(ext.is_block_trade, Some(row.is_block_trade));
        }
        assert!(
            trades.windows(2).all(|pair| pair[0].time <= pair[1].time),
            "the trades are not oldest first"
        );
        // 2023-11-14 in milliseconds: a time read as seconds would be far below.
        assert!(
            trades[0].time > UnixMillis(1_700_000_000_000),
            "{:?}",
            trades[0].time
        );
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn a_trades_price_time_and_taker_side_follow_the_keys_outcome() {
    let template = fixture_json("trades")["trades"][0].clone();
    // Newest first, as Kalshi serves them.
    let mut body = fixture_json("trades");
    body["trades"] = json!([
        trade_row(
            &template,
            "t4",
            "2026-10-18T09:10:12Z",
            "0.6200",
            "0.3800",
            "maybe",
            false
        ),
        trade_row(
            &template,
            "t3",
            "2026-10-18T09:10:11.250Z",
            "0.6000",
            "0.4000",
            "no",
            false
        ),
        trade_row(
            &template,
            "t2",
            "2026-10-18T09:10:11Z",
            "0.5900",
            "0.4100",
            "yes",
            false
        ),
        trade_row(
            &template,
            "t1",
            "2026-10-18T09:10:10Z",
            "0.5800",
            "0.4200",
            "yes",
            true
        ),
    ]);

    let yes = trades_of(&body, Outcome::Yes).await.unwrap();
    assert_eq!(
        yes.iter()
            .map(|t| t.id.clone().unwrap())
            .collect::<Vec<_>>(),
        ["t1", "t2", "t3", "t4"].map(TradeId::new)
    );
    assert_eq!(
        yes.iter().map(|t| t.time).collect::<Vec<_>>(),
        [
            UnixMillis(1_792_314_610_000),
            UnixMillis(1_792_314_611_000),
            UnixMillis(1_792_314_611_250),
            UnixMillis(1_792_314_612_000),
        ]
    );
    assert_eq!(
        yes.iter().map(|t| t.price).collect::<Vec<_>>(),
        ["0.58", "0.59", "0.60", "0.62"].map(|p| p.parse::<Decimal>().unwrap())
    );
    // The taker bought the outcome when its outcome side is the key's.
    assert_eq!(
        yes.iter().map(|t| t.taker_side).collect::<Vec<_>>(),
        [Some(Side::Buy), Some(Side::Buy), Some(Side::Sell), None]
    );
    assert!(yes.iter().all(|t| t.size == Decimal::new(5, 0)));
    assert_eq!(
        yes.iter()
            .map(|t| t.extensions.get::<TradeExt>().unwrap().is_block_trade)
            .collect::<Vec<_>>(),
        [Some(true), Some(false), Some(false), Some(false)]
    );

    let no = trades_of(&body, Outcome::No).await.unwrap();
    assert_eq!(
        no.iter().map(|t| t.price).collect::<Vec<_>>(),
        ["0.42", "0.41", "0.40", "0.38"].map(|p| p.parse::<Decimal>().unwrap())
    );
    assert_eq!(
        no.iter().map(|t| t.taker_side).collect::<Vec<_>>(),
        [Some(Side::Sell), Some(Side::Sell), Some(Side::Buy), None]
    );
}

#[tokio::test]
async fn a_trade_time_that_does_not_parse_is_a_decode_failure() {
    let template = fixture_json("trades")["trades"][0].clone();
    let mut body = fixture_json("trades");
    body["trades"] = json!([
        trade_row(
            &template,
            "t2",
            "2026-10-18T09:10:11Z",
            "0.5900",
            "0.4100",
            "yes",
            false
        ),
        trade_row(
            &template,
            "t1",
            "last Tuesday",
            "0.5800",
            "0.4200",
            "yes",
            false
        ),
    ]);
    let err = trades_of(&body, Outcome::Yes).await.unwrap_err();
    assert_eq!(err.class, Class::Decode);
    assert!(err.to_string().contains("created_time"), "{err}");
}

#[tokio::test]
async fn a_key_of_another_venue_is_refused_by_trades_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let err = venue.trades(&foreign_key()).await.unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    nothing.assert_async().await;
}

/// A `{"event": ..}` body with one nested market's ticker replaced.
fn event_with_ticker(ticker: &str) -> Value {
    let mut body = fixture_json("event");
    body["event"]["markets"][0]["ticker"] = json!(ticker);
    body
}

#[tokio::test]
async fn a_dyn_event_groups_returns_both_outcomes_of_each_nested_market() {
    let market_body = fixture_json("market");
    let ticker = ticker_of(&market_body);
    let event_ticker = market_body["market"]["event_ticker"]
        .as_str()
        .unwrap()
        .to_owned();
    let event_path = format!("/trade-api/v2/events/{event_ticker}");
    let nested = Matcher::UrlEncoded("with_nested_markets".to_owned(), "true".to_owned());

    let mut server = start().await;
    let market = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &market_body.to_string(),
        2,
    )
    .await;
    // Each nested market gets different YES and NO subtitles, so a title taken
    // from the wrong one cannot pass.
    let mut event_body = fixture_json("event");
    for (i, (yes, no)) in [
        ("Alpha wins", "Beta wins"),
        ("Gamma wins", "Delta wins"),
        ("Epsilon wins", "Zeta wins"),
    ]
    .into_iter()
    .enumerate()
    {
        event_body["event"]["markets"][i]["yes_sub_title"] = json!(yes);
        event_body["event"]["markets"][i]["no_sub_title"] = json!(no);
    }
    let event = route(&mut server, &event_path, nested, &event_body.to_string(), 3).await;
    let client = client(&server);
    let native = client
        .events()
        .get(&event_ticker)
        .with_nested_markets(true)
        .send()
        .await
        .unwrap()
        .event;
    assert!(
        !native.markets.is_empty(),
        "the captured event nests markets"
    );
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client);

    for outcome in [Outcome::Yes, Outcome::No] {
        let group = venue
            .event_group(&key::key(&ticker, outcome))
            .await
            .unwrap();
        assert_eq!(group.id, native.event_ticker);
        assert_eq!(group.title.as_deref(), Some(native.title.as_str()));
        assert_eq!(group.outcomes.len(), 2 * native.markets.len());
        assert_eq!(native.markets.len(), 3);
        let titles = ["Alpha wins", "Gamma wins", "Epsilon wins"];
        for ((pair, nested), title) in group.outcomes.chunks(2).zip(&native.markets).zip(titles) {
            let nested_ticker = Ticker::new(&nested.ticker).unwrap();
            assert_eq!(pair[0].key, key::key(&nested_ticker, Outcome::Yes));
            assert_eq!(pair[0].label, "Yes");
            assert_eq!(pair[1].key, key::key(&nested_ticker, Outcome::No));
            assert_eq!(pair[1].label, "No");
            for each in pair {
                assert_eq!(each.market, nested.ticker);
                assert_eq!(each.market_title.as_deref(), Some(title));
            }
        }
        let ext = group.extensions.get::<EventGroupExt>().unwrap();
        assert_eq!(
            ext.series_ticker.as_deref(),
            Some(native.series_ticker.as_str())
        );
        assert_eq!(ext.sub_title.as_deref(), Some(native.sub_title.as_str()));
        assert_eq!(ext.mutually_exclusive, Some(native.mutually_exclusive));
        assert_eq!(
            ext.collateral_return_type.as_deref(),
            Some(native.collateral_return_type.as_str())
        );
    }
    market.assert_async().await;
    event.assert_async().await;
}

#[tokio::test]
async fn an_unlisted_key_has_no_event_group_and_asks_for_no_event() {
    let ticker = test_ticker();
    let mut server = start().await;
    let _market = server
        .mock("GET", market_path(&ticker).as_str())
        .with_status(404)
        .with_body(fixture("error_not_found"))
        .create_async()
        .await;
    let events = server
        .mock("GET", Matcher::Regex("^/trade-api/v2/events/".to_owned()))
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));

    let k = key::key(&ticker, Outcome::No);
    let err = venue.event_group(&k).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(err.source.downcast_ref::<NotListed>().unwrap().key, k);
    events.assert_async().await;
}

#[tokio::test]
async fn a_nested_market_ticker_no_key_can_carry_is_a_decode_failure() {
    let market_body = fixture_json("market");
    let ticker = ticker_of(&market_body);
    let event_ticker = market_body["market"]["event_ticker"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut server = start().await;
    let _market = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &market_body.to_string(),
        1,
    )
    .await;
    let _event = route(
        &mut server,
        &format!("/trade-api/v2/events/{event_ticker}"),
        Matcher::Any,
        &event_with_ticker("not a ticker!").to_string(),
        1,
    )
    .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));

    let err = venue
        .event_group(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap_err();
    assert_eq!(err.class, Class::Decode);
}

#[tokio::test]
async fn a_key_of_another_venue_is_refused_by_event_group_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let err = venue.event_group(&foreign_key()).await.unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    nothing.assert_async().await;
}
