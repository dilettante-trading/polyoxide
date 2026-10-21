//! Kalshi's market-data traits against production, unsigned: no credentials
//! and no secrets. Every test is `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-kalshi --no-default-features --features predictions \
//!     --test live_predictions_venue -- --ignored
//! ```
//!
//! The market is chosen by precondition (open, with a two-sided book on the
//! orderbook route the assertions read) by `common::two_sided_market`, so a
//! quiet night is an environmental skip; the trades test reads the market with
//! the most recent trades, `common::busiest_market`, since a quoted market can
//! have traded never. The conversion's rule is asserted on
//! the live host: no converted book or quote is crossed, and the two outcomes
//! of one market are one book read from both sides. A book that moved between
//! reads is read again, up to five times.

#[allow(dead_code)]
mod common;

use std::{cmp::Reverse, collections::HashMap};

use polyoxide_kalshi::predictions::{
    key::{self, Outcome, Ticker},
    Predictions,
};
use polyoxide_test_support::{environmental, transient, ClassifiedResultExt, ResultExt};
use polyoxide_venue::{
    Book, CandleInterval, CandleQuery, Candles, EventGroups, Level, MarketData, MarketKey, Quote,
    SizeUnit, Trades, UnixMillis,
};
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

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_trades_are_oldest_first() {
    let client = client();
    let yes = key::key(&common::busiest_market(&client).await, Outcome::Yes);
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

/// The days a market must have been open for the check across outcomes to hold a completed
/// daily candle: it skips a candle younger than two periods, so the 15-minute crypto markets
/// that dominate the trades list, which open minutes before they trade, have no completed hourly
/// candle and no completed daily one.
const ESTABLISHED_DAYS: i64 = 3;

/// How many of the busiest markets are looked at for one that qualifies.
const CANDIDATES: usize = 30;

/// The market with the most trades in the host's latest 1000 among those opened at least
/// `ESTABLISHED_DAYS` ago. With none among the busiest `CANDIDATES`, the test is an
/// `environmental` skip.
async fn busiest_established_market(client: &Predictions) -> Ticker {
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
    let mut busiest: Vec<(&str, usize)> = counts.into_iter().collect();
    busiest.sort_by_key(|(ticker, count)| (Reverse(*count), *ticker));
    let latest_open = chrono::Utc::now() - chrono::Duration::days(ESTABLISHED_DAYS);
    for (name, _) in busiest.into_iter().take(CANDIDATES) {
        let ticker = Ticker::new(name).or_fail("name a recently traded market");
        let market = client
            .markets()
            .get(&ticker)
            .send()
            .await
            .or_fail("read a recently traded market")
            .market;
        if let Ok(opened) = chrono::DateTime::parse_from_rfc3339(&market.open_time) {
            if opened <= latest_open {
                return ticker;
            }
        }
    }
    environmental(&format!(
        "none of the busiest {CANDIDATES} markets in the latest trades opened {ESTABLISHED_DAYS} days ago"
    ))
}

/// How far back each served width is read: enough for several candles of it.
const LOOKBACKS: [(CandleInterval, u64); 3] = [
    (CandleInterval::Minute1, 3 * 60 * 60 * 1000),
    (CandleInterval::Hour1, 3 * 24 * 60 * 60 * 1000),
    (CandleInterval::Day1, 14 * 24 * 60 * 60 * 1000),
];

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_candles_at_each_served_width_are_ordered_aligned_and_mirror_each_other() {
    let client = client();
    assert_eq!(
        client.intervals(),
        [
            CandleInterval::Minute1,
            CandleInterval::Hour1,
            CandleInterval::Day1
        ]
    );
    let ticker = busiest_established_market(&client).await;
    let yes = key::key(&ticker, Outcome::Yes);
    let no = key::key(&ticker, Outcome::No);

    let mut widths_compared = 0;
    for (width, lookback) in LOOKBACKS {
        let step = width.duration().as_secs() * 1000;
        let now = UnixMillis::now();
        let query = CandleQuery::new(width, UnixMillis(now.0 - lookback)).end(now);
        let candles = client.candles(&yes, &query).await.or_fail("YES candles");
        if candles.is_empty() {
            environmental(&format!("{yes}: no {width:?} candle in the lookback"));
        }
        for candle in &candles {
            assert_eq!(candle.key, yes);
            assert_eq!(candle.interval, width);
            assert!(
                candle.open_time >= query.start && candle.open_time <= now,
                "{yes}: a {width:?} candle opens at {:?}, outside the query",
                candle.open_time
            );
            // Minute and hourly candles open on their own boundary, and the daily candle ends at
            // 04:00 UTC, so it opens on an hour.
            let boundary = if width == CandleInterval::Day1 {
                3_600_000
            } else {
                step
            };
            assert_eq!(
                candle.open_time.0 % boundary,
                0,
                "{yes}: a {width:?} candle opens off its boundary"
            );
            assert!(
                candle.close >= Decimal::ZERO && candle.close <= Decimal::ONE,
                "{yes}: a close of {} is not a price in dollars",
                candle.close
            );
            if let (Some(open), Some(high), Some(low)) = (candle.open, candle.high, candle.low) {
                assert!(
                    low <= open && open <= high && low <= candle.close && candle.close <= high,
                    "{yes}: {width:?} candle at {:?} has open {open}, high {high}, low {low}, close {}",
                    candle.open_time,
                    candle.close
                );
            }
        }
        assert!(
            candles
                .windows(2)
                .all(|pair| pair[0].open_time < pair[1].open_time),
            "{yes}: the {width:?} candles are not strictly oldest first"
        );

        // The two outcomes are one market read from both sides: a completed candle's closes sum
        // to one, and one outcome's high is one less the other's low.
        let others = client.candles(&no, &query).await.or_fail("NO candles");
        let mut compared = 0;
        for own in &candles {
            if own.open_time.0 + 2 * step + 60_000 >= now.0 {
                continue;
            }
            let Some(other) = others.iter().find(|c| c.open_time == own.open_time) else {
                continue;
            };
            assert_eq!(
                own.close + other.close,
                Decimal::ONE,
                "{ticker}: {width:?} closes at {:?}",
                own.open_time
            );
            if let (Some(high), Some(low)) = (own.high, other.low) {
                assert_eq!(
                    high + low,
                    Decimal::ONE,
                    "{ticker}: {width:?} high against the other outcome's low at {:?}",
                    own.open_time
                );
            }
            compared += 1;
        }
        if compared == 0 {
            eprintln!("{ticker}: no completed {width:?} candle to compare across outcomes");
        } else {
            widths_compared += 1;
        }
    }
    // A width the market is too young for is left out of the comparison; with none compared the
    // test proved nothing about the mirroring.
    if widths_compared == 0 {
        environmental(&format!(
            "{ticker}: no completed candle at any width to compare across outcomes"
        ));
    }
}
