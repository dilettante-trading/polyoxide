//! Kalshi's market-data traits against production, unsigned: no credentials
//! and no secrets. Every test is `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-kalshi --no-default-features --features predictions \
//!     --test live_predictions_venue -- --ignored
//! ```
//!
//! The market is chosen by precondition (open, with a two-sided book on the
//! orderbook route the assertions read) by `common::two_sided_market`, so a
//! quiet night is an environmental skip. The conversion's rule is asserted on
//! the live host: no converted book or quote is crossed, and the two outcomes
//! of one market are one book read from both sides. A book that moved between
//! reads is read again, up to five times.

#[allow(dead_code)]
mod common;

use std::collections::HashMap;

use polyoxide_kalshi::predictions::{
    key::{self, Outcome, Ticker},
    Predictions,
};
use polyoxide_test_support::{environmental, transient, ClassifiedResultExt, ResultExt};
use polyoxide_venue::{Book, EventGroups, Level, MarketData, MarketKey, Quote, SizeUnit, Trades};
use rust_decimal::Decimal;

/// How many times a read that moved is tried again.
const READS: usize = 5;

fn client() -> Predictions {
    Predictions::new().or_fail("build the predictions client")
}

/// The keys of both outcomes of the chosen market.
async fn chosen(client: &Predictions) -> (MarketKey, MarketKey) {
    let market = common::two_sided_market(client).await;
    let ticker = Ticker::new(&market.ticker).or_fail("name the chosen market");
    (
        key::key(&ticker, Outcome::Yes),
        key::key(&ticker, Outcome::No),
    )
}

/// The prices a side of one outcome's book is, as the other outcome's asks.
fn flipped(levels: &[Level]) -> Vec<Level> {
    levels
        .iter()
        .map(|level| Level::new(Decimal::ONE - level.price, level.size))
        .collect()
}

/// Both sides of a book, with the best bid below the best ask.
fn assert_uncrossed(key: &MarketKey, bids: &[Level], asks: &[Level]) {
    let (Some(bid), Some(ask)) = (bids.first(), asks.first()) else {
        environmental(&format!("{key}: a side of the book emptied between reads"));
    };
    assert!(
        bid.price < ask.price,
        "{key}: best bid {bid:?} is not below best ask {ask:?}"
    );
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_the_chosen_markets_outcomes_are_tradable_shares() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    for each in [yes, no] {
        let instrument = client.instrument(&each).await.or_fail("instrument");
        assert_eq!(instrument.key, each);
        assert_eq!(instrument.size_unit, SizeUnit::Shares);
        assert_eq!(instrument.tradable, Some(true), "{each} is not tradable");
    }
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_both_outcomes_books_are_uncrossed_and_mirror_each_other() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    for _ in 0..READS {
        let first: Book = client.book(&yes).await.or_fail("YES book");
        let other: Book = client.book(&no).await.or_fail("NO book");
        let again: Book = client.book(&yes).await.or_fail("YES book, again");
        if first.bids != again.bids || first.asks != again.asks {
            continue;
        }
        assert_uncrossed(&yes, &first.bids, &first.asks);
        assert_uncrossed(&no, &other.bids, &other.asks);
        assert_eq!(
            first.bids,
            flipped(&other.asks),
            "{yes}: bids against {no}'s asks"
        );
        assert_eq!(
            first.asks,
            flipped(&other.bids),
            "{yes}: asks against {no}'s bids"
        );
        return;
    }
    transient("the YES book moved in each of five reads");
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_both_outcomes_quotes_are_uncrossed_and_mirror_each_other() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    for _ in 0..READS {
        let first: Quote = client.quote(&yes).await.or_fail("YES quote");
        let other: Quote = client.quote(&no).await.or_fail("NO quote");
        let again: Quote = client.quote(&yes).await.or_fail("YES quote, again");
        if first.bid != again.bid || first.ask != again.ask {
            continue;
        }
        let side = |level: Option<Level>| level.into_iter().collect::<Vec<Level>>();
        assert_uncrossed(&yes, &side(first.bid), &side(first.ask));
        assert_uncrossed(&no, &side(other.bid), &side(other.ask));
        assert_eq!(
            side(first.bid),
            flipped(&side(other.ask)),
            "{yes}: bid against {no}'s ask"
        );
        assert_eq!(
            side(first.ask),
            flipped(&side(other.bid)),
            "{yes}: ask against {no}'s bid"
        );
        return;
    }
    transient("the YES quote moved in each of five reads");
}

/// The YES key of the market with the most trades in the host's latest 1000. The
/// chosen two-sided market is the first quoted row of the listing, and a quoted
/// market can have traded never, so its trades say nothing about their order.
async fn busiest_market(client: &Predictions) -> MarketKey {
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
    let Some((ticker, _)) = counts
        .into_iter()
        .max_by_key(|(ticker, count)| (*count, *ticker))
    else {
        environmental("the host served no recent trades");
    };
    let ticker = Ticker::new(ticker).or_fail("name the busiest market");
    key::key(&ticker, Outcome::Yes)
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_trades_are_oldest_first() {
    let client = client();
    let yes = busiest_market(&client).await;
    let trades = client.trades(&yes).await.or_fail("trades");
    if trades.len() < 2 {
        environmental(&format!(
            "the chosen market has {} trades, too few to check their order",
            trades.len()
        ));
    }
    assert!(
        trades.windows(2).all(|pair| pair[0].time <= pair[1].time),
        "{yes}: the trades are not oldest first"
    );
    assert!(
        trades.iter().all(|trade| trade.id.is_some()),
        "{yes}: a trade has no id"
    );
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_the_event_group_holds_both_outcomes_of_the_market() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    let group = client.event_group(&yes).await.or_fail("event group");
    for each in [yes, no] {
        assert!(
            group.outcomes.iter().any(|outcome| outcome.key == each),
            "the event {} does not hold {each}",
            group.id
        );
    }
}
