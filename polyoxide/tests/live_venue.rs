//! The example's `best_bid_ask` against the live hosts: the same function
//! reads Binance USDⓈ-M, Polymarket perps and the Polymarket CLOB.

#[allow(dead_code)]
#[path = "../examples/venue_neutral.rs"]
mod example;

use std::sync::Arc;

use example::best_bid_ask;
use polyoxide::binance::usdm::{self, types::Symbol, Usdm};
use polyoxide::polymarket::clob::Clob;
use polyoxide::polymarket::perps::Perps;
use polyoxide::polymarket::shared::keys::{self, TokenId};
use polyoxide::venue::{DynInstruments, DynMarketData, Instruments, Level, MarketKey};
use polyoxide_test_support::{environmental, ClassifiedResultExt, ResultExt};

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
    let listing: Arc<DynInstruments<'static>> = DynInstruments::new_arc(perps.clone());
    let instruments = listing.instruments().await.or_fail("list the instruments");
    let Some(instrument) = instruments
        .iter()
        .find(|instrument| instrument.tradable != Some(false))
    else {
        environmental("perps lists no instrument that takes orders");
    };
    read(DynMarketData::new_arc(perps), &instrument.key).await;
}

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
    let Some(token) = open
        .data
        .iter()
        .filter(|market| market.accepting_orders == Some(true))
        .flat_map(|market| &market.tokens)
        .find_map(|token| token.token_id.clone())
    else {
        environmental("no market on the first sampling page takes orders");
    };
    let key = keys::clob(&TokenId::new(token).or_fail("read the token id"));
    read(DynMarketData::new_arc(clob), &key).await;
}
