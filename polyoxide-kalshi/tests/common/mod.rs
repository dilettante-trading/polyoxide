//! The market the live suites read, chosen by the precondition they assert on. Shared by
//! `live_predictions.rs` and `live_predictions_venue.rs`, by `mod common;`, and the live suites of
//! later stories.

use std::{cmp::Reverse, collections::HashMap};

use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{Market, MarketStatus, MarketStatusFilter, MveFilter},
    Predictions,
};
use polyoxide_test_support::{environmental, ResultExt};
use rust_decimal::Decimal;

/// Pages of the filtered open listing to walk.
const LISTING_PAGES: usize = 5;
/// Quoted markets to confirm on the order book route.
const CANDIDATES: usize = 20;

/// Whether the market trades with a YES bid above 0 and a YES ask below 1.
fn quotes_both_sides(market: &Market) -> bool {
    market.status == MarketStatus::Active
        && market.yes_bid_dollars > Decimal::ZERO
        && market.yes_ask_dollars < Decimal::ONE
}

/// Whether the market's order book, read on the route the suites assert on (a CDN caches per
/// URL), has levels on both sides.
async fn has_both_sides(client: &Predictions, market: &Market) -> bool {
    // A ticker this client would refuse to send is skipped here and caught by
    // `live_every_ticker_the_host_lists_is_one_the_client_accepts`.
    let Ok(ticker) = Ticker::new(&market.ticker) else {
        return false;
    };
    let book = client
        .markets()
        .orderbook(&ticker)
        .send()
        .await
        .or_fail("orderbook")
        .orderbook_fp;
    !book.yes_dollars.is_empty() && !book.no_dollars.is_empty()
}

/// An open market with a quote and an order book on both sides.
///
/// Candidates come from `markets?status=open&mve_filter=exclude&limit=1000`, at most
/// `LISTING_PAGES` pages: the unfiltered open listing serves multivariate combinations first
/// (9,996 of its first 10,000 rows on 2026-10-18), none of them quoted, while the filtered
/// listing's first page held 579 quoted rows. A row with a YES bid above 0 and a YES ask below 1
/// is confirmed on the order book route. With none, the test is an `environmental` skip.
pub async fn two_sided_market(client: &Predictions) -> Market {
    two_sided_market_where(client, "", |_| true).await
}

/// As [`two_sided_market`], among the quoted rows `wanted` accepts. `needing` completes the
/// `environmental` skip's reason when none qualifies, such as `" and a YES bid of at least 0.60"`.
pub async fn two_sided_market_where(
    client: &Predictions,
    needing: &str,
    wanted: impl Fn(&Market) -> bool,
) -> Market {
    let mut tried = 0;
    let mut cursor = String::new();
    for _ in 0..LISTING_PAGES {
        let list = client
            .markets()
            .list()
            .status(MarketStatusFilter::Open)
            .mve_filter(MveFilter::Exclude)
            .limit(1000);
        let list = if cursor.is_empty() {
            list
        } else {
            list.cursor(cursor.clone())
        };
        let page = list.send().await.or_fail("markets");
        for row in page
            .markets
            .iter()
            .filter(|m| quotes_both_sides(m) && wanted(m))
        {
            if tried >= CANDIDATES {
                environmental(&format!(
                    "no suitable market: no quoted open market with a book on both sides{needing}"
                ));
            }
            tried += 1;
            if has_both_sides(client, row).await {
                return row.clone();
            }
        }
        if page.cursor.is_empty() {
            break;
        }
        cursor = page.cursor;
    }
    environmental(&format!(
        "no suitable market: no open market with a quote and a book on both sides{needing}"
    ))
}

/// The market with the most trades in the host's latest 1000. A quoted market, such as
/// `two_sided_market`'s, can have traded never, so its trades say nothing about their order or
/// their ticker. With no recent trades at all, the test is an `environmental` skip.
pub async fn busiest_market(client: &Predictions) -> Ticker {
    let Some(ticker) = tickers_by_trades(client).await.into_iter().next() else {
        environmental("the host served no recent trades");
    };
    Ticker::new(&ticker).or_fail("name the busiest market")
}

/// The tickers in the host's latest 1000 trades, the one with the most trades first (the
/// greater name first among equals).
pub async fn tickers_by_trades(client: &Predictions) -> Vec<String> {
    let page = client
        .markets()
        .trades()
        .limit(1000)
        .send()
        .await
        .or_fail("recent trades");
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for trade in &page.trades {
        *counts.entry(trade.ticker.as_str()).or_default() += 1;
    }
    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
    ranked.sort_by_key(|(ticker, count)| Reverse((*count, *ticker)));
    ranked
        .into_iter()
        .map(|(ticker, _)| ticker.to_owned())
        .collect()
}
