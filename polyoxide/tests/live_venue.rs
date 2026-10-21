//! The example's `best_bid_ask` against the live hosts: the same function
//! reads Binance USDⓈ-M, Polymarket perps, the Polymarket CLOB and Kalshi's
//! Predictions.

#[allow(dead_code)]
#[path = "../examples/venue_neutral.rs"]
mod example;

use std::sync::Arc;

use example::best_bid_ask;
use polyoxide::binance::usdm::{self, types::Symbol, Usdm};
use polyoxide::kalshi::predictions::{
    key::{self as kalshi_key, Outcome, Ticker},
    types::{MarketStatus, MarketStatusFilter, MveFilter},
    Predictions,
};
use polyoxide::polymarket::clob::{Clob, ClobError};
use polyoxide::polymarket::perps::Perps;
use polyoxide::polymarket::shared::keys::{self, TokenId};
use polyoxide::venue::{DynMarketData, Level, MarketKey};
use polyoxide_test_support::{environmental, fail, ClassifiedResultExt, ResultExt};
use rust_decimal::Decimal;

/// Reads `key`'s best bid and ask through `venue`, and checks the two do not
/// cross.
async fn read(
    venue: Arc<DynMarketData<'static>>,
    key: &MarketKey,
) -> (Option<Level>, Option<Level>) {
    let (bid, ask) = best_bid_ask(&venue, key)
        .await
        .or_fail("read the best bid and ask");
    if let (Some(bid), Some(ask)) = (bid, ask) {
        assert!(
            bid.price < ask.price,
            "{key}: bid {bid:?} crosses ask {ask:?}"
        );
    }
    (bid, ask)
}

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_binance_usdm() {
    let venue = DynMarketData::new_arc(Usdm::new().or_fail("build the usdm client"));
    let key = usdm::key(&Symbol::new("BTCUSDT").or_fail("name BTCUSDT"));
    let (bid, ask) = read(venue, &key).await;
    assert!(
        bid.is_some() && ask.is_some(),
        "BTCUSDT always has both sides"
    );
}

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_polymarket_perps() {
    let perps = Perps::new().or_fail("build the perps client");
    // The instrument is picked on the route the trait's quote reads, so it has
    // both sides there.
    let rows = perps
        .market()
        .bbo()
        .send()
        .await
        .or_fail("list the best bids and asks");
    let Some(row) = rows
        .iter()
        .find(|row| !row.bid_quantity.is_zero() && !row.ask_quantity.is_zero())
    else {
        environmental("perps lists no instrument with both a bid and an ask");
    };
    let (bid, ask) = read(
        DynMarketData::new_arc(perps),
        &keys::perps(row.instrument_id),
    )
    .await;
    assert!(
        bid.is_some() && ask.is_some(),
        "the bbo row had both sides: {row:?}"
    );
}

/// Tokens of the first sampling page to try; a token with no book is the
/// CLOB's `404`, so the walk moves on.
const CLOB_TOKENS_TRIED: usize = 40;

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_the_polymarket_clob() {
    let clob = Clob::public();
    let open = clob
        .markets()
        .sampling()
        .send()
        .await
        .or_fail("list the sampling markets");
    let tokens = open
        .data
        .iter()
        .filter(|market| market.accepting_orders == Some(true))
        .flat_map(|market| &market.tokens)
        .filter_map(|token| token.token_id.clone())
        .take(CLOB_TOKENS_TRIED);
    // The token is picked on the route the trait's quote reads, so it has a
    // book with both sides there.
    let mut found = None;
    for token in tokens {
        match clob.markets().order_book(token.clone()).send().await {
            Ok(book) if !book.bids.is_empty() && !book.asks.is_empty() => {
                found = Some(token);
                break;
            }
            Ok(_) => {}
            // "No orderbook exists": the market has none, not a fault of the read.
            Err(ClobError::Api(polyoxide_core::ApiError::Response(r))) if r.status == 404 => {}
            Err(err) => fail("read a sampling token's order book", &err),
        }
    }
    let Some(token) = found else {
        environmental(&format!(
            "none of the first {CLOB_TOKENS_TRIED} accepting-orders tokens on the first sampling page has a book with both sides"
        ));
    };
    let key = keys::clob(&TokenId::new(token).or_fail("read the token id"));
    let (bid, ask) = read(DynMarketData::new_arc(clob), &key).await;
    assert!(
        bid.is_some() && ask.is_some(),
        "the order book had both sides"
    );
}

/// Quoted Kalshi markets to confirm on the order book route the trait's quote reads.
const KALSHI_CANDIDATES: usize = 20;

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_kalshi_predictions() {
    let kalshi = Predictions::new().or_fail("build the predictions client");
    // The unfiltered open listing serves multivariate combinations first, none of them quoted,
    // so the walk asks to leave them out.
    let page = kalshi
        .markets()
        .list()
        .status(MarketStatusFilter::Open)
        .mve_filter(MveFilter::Exclude)
        .limit(1000)
        .send()
        .await
        .or_fail("list the open markets");
    let quoted = page
        .markets
        .iter()
        .filter(|market| {
            market.status == MarketStatus::Active
                && market.yes_bid_dollars > Decimal::ZERO
                && market.yes_ask_dollars < Decimal::ONE
        })
        .take(KALSHI_CANDIDATES);
    // The market is picked on the route the trait's quote reads (depth 1), so it has both
    // sides there.
    let mut found = None;
    for market in quoted {
        let Ok(ticker) = Ticker::new(&market.ticker) else {
            continue;
        };
        let book = kalshi
            .markets()
            .orderbook(&ticker)
            .depth(1)
            .send()
            .await
            .or_fail("read a quoted market's order book")
            .orderbook_fp;
        if !book.yes_dollars.is_empty() && !book.no_dollars.is_empty() {
            found = Some(ticker);
            break;
        }
    }
    let Some(ticker) = found else {
        environmental(&format!(
            "none of the first {KALSHI_CANDIDATES} quoted open markets has an order book with both sides"
        ));
    };
    let key = kalshi_key::key(&ticker, Outcome::Yes);
    let (bid, ask) = read(DynMarketData::new_arc(kalshi), &key).await;
    assert!(
        bid.is_some() && ask.is_some(),
        "the order book had both sides"
    );
}
