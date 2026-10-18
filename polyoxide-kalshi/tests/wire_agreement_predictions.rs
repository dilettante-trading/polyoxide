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

use polyoxide_kalshi::predictions::{
    request::DEFAULT_COST,
    types::{
        BookSide, EndpointCosts, EventResponse, ExchangeStatus, MarketResponse, MarketStatus,
        MarketsPage, OrderbookResponse, OutcomeSide, TradesPage,
    },
};
use polyoxide_test_support::{agreement, fixtures};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};

/// `(fixture, path)` pairs the host sends in some responses and not others, which a type that
/// models them may emit as `null` where a capture lacks them.
const SOMETIMES_ABSENT: &[(&str, &str)] = &[("exchange_status", "/exchange_estimated_resume_time")];

fn read<T: DeserializeOwned>(fixture: &str) -> T {
    serde_json::from_str(&fixtures!("predictions").text(fixture)).unwrap()
}

fn hold_to_fixture<T: DeserializeOwned + Serialize>(fixture: &str) {
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
        .filter(|path| !SOMETIMES_ABSENT.contains(&(fixture, path.as_str())))
        .collect();
    assert!(
        invented.is_empty(),
        "{fixture}: the type emits {invented:?}, which the host did not send"
    );
}

#[test]
fn every_rest_fixture_agrees_with_its_type() {
    hold_to_fixture::<ExchangeStatus>("exchange_status");
    hold_to_fixture::<EndpointCosts>("endpoint_costs");
    hold_to_fixture::<MarketResponse>("market");
    hold_to_fixture::<MarketsPage>("markets_page");
    hold_to_fixture::<OrderbookResponse>("orderbook");
    hold_to_fixture::<OrderbookResponse>("orderbook_depth1");
    hold_to_fixture::<TradesPage>("trades");
    hold_to_fixture::<EventResponse>("event");
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
    assert!(!trades.trades.is_empty());
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
