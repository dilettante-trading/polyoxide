//! A local order book, kept from the socket's events.
//!
//! [`Books`] is a plain value the consumer feeds each [`Event`] the supervised socket
//! yields, through [`Books::apply`]. It does no I/O and has no clock: it keeps one
//! [`OrderBook`] per market ticker, built only from the snapshots the current
//! subscription sent, and changes it by the deltas that follow.
//!
//! A book is kept in YES pricing, as the socket sends it (the subscription asks for
//! `use_yes_price`): the bids are YES offers to buy and the asks are the NO bids, priced
//! as the YES offers to sell they are. [`OrderBook::levels`] turns the ladder over for the
//! NO outcome, where a NO bid at *q* is a YES ask at 1 - *q*, as the REST book is read.
//!
//! The rules are the order-book channel's: a snapshot replaces a market's book, a delta
//! adds its signed size to one level and removes a level whose size reaches zero or
//! below, and a delta for a market with no snapshot is ignored. [`Event::Resyncing`]
//! drops the books of the markets it names, whose deltas stay ignored until each one's
//! fresh snapshot arrives, and [`Event::Reconnected`] drops every book, so no book ever
//! carries a delta onto a snapshot from another connection. Between
//! [`Event::Disconnected`] and [`Event::Reconnected`] the books stay as they were, and
//! are stale.

use std::collections::{BTreeMap, HashMap};

use polyoxide_venue::Level;
use rust_decimal::Decimal;

use crate::predictions::{
    key::Outcome,
    types::BookSide,
    ws::{Event, OrderbookSnapshot},
};

/// One market's book: its YES-priced ladder, price to resting size.
///
/// Built and changed only by [`Books::apply`]; every size in it is above zero.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBook {
    market_ticker: String,
    bids: BTreeMap<Decimal, Decimal>,
    asks: BTreeMap<Decimal, Decimal>,
}

fn level_of((price, size): (&Decimal, &Decimal)) -> Level {
    Level::new(*price, *size)
}

fn ladder_of(levels: &[Level]) -> BTreeMap<Decimal, Decimal> {
    levels
        .iter()
        .filter(|level| level.size > Decimal::ZERO)
        .map(|level| (level.price, level.size))
        .collect()
}

/// Adds `delta` to the size at `price`, and removes the level when the size is not above
/// zero. A level the ladder does not hold starts from zero, so a change that only takes
/// away opens nothing.
fn add(ladder: &mut BTreeMap<Decimal, Decimal>, price: Decimal, delta: Decimal) {
    let size = ladder.get(&price).copied().unwrap_or(Decimal::ZERO) + delta;
    if size > Decimal::ZERO {
        ladder.insert(price, size);
    } else {
        ladder.remove(&price);
    }
}

impl OrderBook {
    fn from_snapshot(snapshot: &OrderbookSnapshot) -> Self {
        Self {
            market_ticker: snapshot.market_ticker.clone(),
            bids: ladder_of(&snapshot.bids),
            asks: ladder_of(&snapshot.asks),
        }
    }

    /// The market's ticker.
    pub fn market_ticker(&self) -> &str {
        &self.market_ticker
    }

    /// The highest YES bid, when the book has one.
    pub fn best_bid(&self) -> Option<Level> {
        self.bids.iter().next_back().map(level_of)
    }

    /// The lowest YES ask, when the book has one.
    pub fn best_ask(&self) -> Option<Level> {
        self.asks.iter().next().map(level_of)
    }

    /// Every YES bid, highest price first.
    pub fn bids(&self) -> Vec<Level> {
        self.bids.iter().rev().map(level_of).collect()
    }

    /// Every YES ask, lowest price first: the NO bids, in YES pricing.
    pub fn asks(&self) -> Vec<Level> {
        self.asks.iter().map(level_of).collect()
    }

    /// One outcome's bids and asks, as `(bids, asks)`, each best first: its bids from the
    /// highest price, and its asks from the lowest.
    ///
    /// For the YES outcome this is the ladder as kept. For NO every price turns over: a
    /// YES ask at *p* is a NO bid at 1 - *p*, and a YES bid at *p* is a NO ask at 1 - *p*.
    pub fn levels(&self, outcome: Outcome) -> (Vec<Level>, Vec<Level>) {
        match outcome {
            Outcome::Yes => (self.bids(), self.asks()),
            Outcome::No => {
                let mirrored =
                    |(price, size): (&Decimal, &Decimal)| Level::new(Decimal::ONE - *price, *size);
                // The lowest YES ask is the highest NO bid, and the highest YES bid is
                // the lowest NO ask, so the YES ladders are walked in the order that
                // puts the NO levels best first.
                let bids = self.asks.iter().map(mirrored).collect();
                let asks = self.bids.iter().rev().map(mirrored).collect();
                (bids, asks)
            }
        }
    }
}

/// The books of every market a socket carries, kept from its events.
#[derive(Debug, Default, Clone)]
pub struct Books {
    books: HashMap<String, OrderBook>,
}

impl Books {
    /// No books.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event, and returns the book it changed.
    ///
    /// A snapshot replaces its market's book. A delta changes the book of a market that
    /// has a snapshot, and is ignored (`None`) for one that has none, and for a side that
    /// is neither a bid nor an ask. Every other event changes no book and returns `None`.
    pub fn apply(&mut self, event: &Event) -> Option<&OrderBook> {
        match event {
            Event::Snapshot(snapshot) => {
                let entry = self
                    .books
                    .entry(snapshot.market_ticker.clone())
                    .insert_entry(OrderBook::from_snapshot(snapshot));
                Some(&*entry.into_mut())
            }
            Event::Delta(delta) => {
                let book = self.books.get_mut(&delta.market_ticker)?;
                let side = match &delta.side {
                    BookSide::Bid => &mut book.bids,
                    BookSide::Ask => &mut book.asks,
                    _ => return None,
                };
                add(side, delta.price, delta.delta);
                Some(&*book)
            }
            // The events that carry no book: the ticker and trade channels, and the
            // outage marker. The book-dropping events are Task 6.2's.
            _ => None,
        }
    }

    /// The book of `market_ticker`, when it has a snapshot.
    pub fn get(&self, market_ticker: &str) -> Option<&OrderBook> {
        self.books.get(market_ticker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predictions::ws::OrderbookDelta;

    fn level(price: &str, size: &str) -> Level {
        Level::new(price.parse().unwrap(), size.parse().unwrap())
    }

    fn ladder(pairs: &[(&str, &str)]) -> Vec<Level> {
        pairs
            .iter()
            .map(|(price, size)| level(price, size))
            .collect()
    }

    fn snapshot(ticker: &str, bids: &[(&str, &str)], asks: &[(&str, &str)]) -> Event {
        Event::Snapshot(OrderbookSnapshot {
            market_ticker: ticker.to_owned(),
            market_id: "id".to_owned(),
            bids: ladder(bids),
            asks: ladder(asks),
        })
    }

    fn delta(ticker: &str, side: BookSide, price: &str, delta: &str) -> Event {
        Event::Delta(OrderbookDelta {
            market_ticker: ticker.to_owned(),
            side,
            price: price.parse().unwrap(),
            delta: delta.parse().unwrap(),
            ts: None,
            client_order_id: None,
        })
    }

    #[test]
    fn a_snapshot_builds_both_sides_best_first() {
        let mut books = Books::new();
        // Listed worst first on purpose: the book sorts, whatever order it is sent in.
        let snap = snapshot(
            "A-1",
            &[("0.40", "10.00"), ("0.45", "5.00")],
            &[("0.60", "3.00"), ("0.55", "7.00")],
        );
        let book = books.apply(&snap).unwrap();
        assert_eq!(book.market_ticker(), "A-1");
        assert_eq!(book.bids(), ladder(&[("0.45", "5.00"), ("0.40", "10.00")]));
        assert_eq!(book.asks(), ladder(&[("0.55", "7.00"), ("0.60", "3.00")]));
        assert_eq!(book.best_bid(), Some(level("0.45", "5.00")));
        assert_eq!(book.best_ask(), Some(level("0.55", "7.00")));
        assert_eq!(
            books.get("A-1").unwrap().best_bid(),
            Some(level("0.45", "5.00"))
        );
    }

    #[test]
    fn a_second_snapshot_replaces_the_book() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[("0.60", "3.00")]));
        let book = books
            .apply(&snapshot("A-1", &[("0.30", "1.00")], &[("0.70", "2.00")]))
            .unwrap();
        // Nothing of the first book survives: not its bid, not its ask.
        assert_eq!(book.bids(), ladder(&[("0.30", "1.00")]));
        assert_eq!(book.asks(), ladder(&[("0.70", "2.00")]));
    }

    #[test]
    fn a_snapshot_keeps_no_empty_level_and_may_hold_no_side() {
        let mut books = Books::new();
        let book = books
            .apply(&snapshot("A-1", &[("0.40", "0.00")], &[]))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
        assert_eq!(book.asks(), Vec::new());
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn a_delta_adds_to_a_level_and_opens_a_new_one() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[("0.60", "3.00")]));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "2.50"))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.40", "12.50")]));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.42", "4.00"))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.42", "4.00"), ("0.40", "12.50")]));
        // An ask is the other side of the ladder: a level of its own, not a bid.
        let book = books
            .apply(&delta("A-1", BookSide::Ask, "0.60", "-1.00"))
            .unwrap();
        assert_eq!(book.asks(), ladder(&[("0.60", "2.00")]));
        assert_eq!(book.bids(), ladder(&[("0.42", "4.00"), ("0.40", "12.50")]));
    }

    #[test]
    fn a_level_reaching_zero_or_below_is_removed() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[]));
        // Down to six, then to exactly zero: the level goes, not a zero-size row.
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "-4.00"))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.40", "6.00")]));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "-6.00"))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
        // A change that takes a level below zero removes it too, and one on a level the
        // book never had opens nothing.
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.30", "-5.00"))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
        books.apply(&delta("A-1", BookSide::Bid, "0.30", "3.00"));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.30", "-10.00"))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
    }

    #[test]
    fn a_delta_for_a_ticker_with_no_snapshot_is_ignored() {
        let mut books = Books::new();
        assert!(books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "2.50"))
            .is_none());
        assert!(books.get("A-1").is_none());
        // It does not leave a half-built book for a later snapshot to inherit.
        let book = books
            .apply(&snapshot("A-1", &[("0.45", "1.00")], &[]))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.45", "1.00")]));
    }

    #[test]
    fn books_are_kept_per_ticker() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[]));
        books.apply(&snapshot("B-2", &[("0.10", "1.00")], &[]));
        books.apply(&delta("A-1", BookSide::Bid, "0.40", "5.00"));
        assert_eq!(
            books.get("A-1").unwrap().bids(),
            ladder(&[("0.40", "15.00")])
        );
        assert_eq!(
            books.get("B-2").unwrap().bids(),
            ladder(&[("0.10", "1.00")])
        );
    }

    #[test]
    fn a_delta_on_a_side_it_cannot_place_changes_nothing() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[("0.60", "3.00")]));
        let before = books.get("A-1").cloned();
        let sideways = delta("A-1", BookSide::Other("mid".to_owned()), "0.50", "9.00");
        assert!(books.apply(&sideways).is_none());
        assert_eq!(books.get("A-1").cloned(), before);
    }

    #[test]
    fn the_yes_view_is_the_ladder_as_kept() {
        let mut books = Books::new();
        let book = books
            .apply(&snapshot(
                "A-1",
                &[("0.40", "10.00"), ("0.45", "5.00")],
                &[("0.55", "7.00"), ("0.60", "3.00")],
            ))
            .unwrap();
        let (bids, asks) = book.levels(Outcome::Yes);
        assert_eq!(bids, ladder(&[("0.45", "5.00"), ("0.40", "10.00")]));
        assert_eq!(asks, ladder(&[("0.55", "7.00"), ("0.60", "3.00")]));
    }

    #[test]
    fn the_no_view_mirrors_every_price_at_one_minus() {
        let mut books = Books::new();
        let book = books
            .apply(&snapshot(
                "A-1",
                &[("0.40", "10.00"), ("0.45", "5.00")],
                &[("0.55", "7.00"), ("0.60", "3.00")],
            ))
            .unwrap();
        let (bids, asks) = book.levels(Outcome::No);
        // A YES ask at 0.55 is a NO bid at 0.45, and the lowest YES ask is the best NO bid.
        assert_eq!(bids, ladder(&[("0.45", "7.00"), ("0.40", "3.00")]));
        // A YES bid at 0.45 is a NO ask at 0.55, and the highest YES bid is the best NO ask.
        assert_eq!(asks, ladder(&[("0.55", "5.00"), ("0.60", "10.00")]));
    }
}
