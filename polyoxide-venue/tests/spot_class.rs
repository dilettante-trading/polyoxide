#![cfg(all(feature = "keys", feature = "market"))]
//! `ProductClass::Spot` and the `SpotMarket` bundle, proved on a made-up venue.

use std::borrow::Cow;

use polyoxide_venue::{
    Book, Class, ClassifiedError, Instrument, KeySpace, Level, MarketData, MarketKey, ProductClass,
    ProductId, Quote, SizeUnit, SpotMarket, VenueId,
};
use rust_decimal::Decimal;

/// A made-up product that trades an asset outright.
struct Cash;

impl KeySpace for Cash {
    const VENUE: VenueId = VenueId::from_static("acme");
    const PRODUCT: ProductId = ProductId::from_static("cash");
    const CLASS: ProductClass = ProductClass::Spot;
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_ascii_uppercase())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

#[test]
fn the_class_is_spelled_as_a_crate_registers_it() {
    assert_eq!(ProductClass::Spot.as_str(), "spot");
    assert_eq!(ProductClass::Spot.to_string(), "spot");
}

#[test]
fn the_class_is_distinct_from_the_others_and_sorts_after_them() {
    let ids = [
        ProductClass::EventContract,
        ProductClass::PerpetualFuture,
        ProductClass::Spot,
    ]
    .map(ProductClass::as_str);
    assert_eq!(ids, ["event-contract", "perpetual-future", "spot"]);
    assert!(ProductClass::EventContract < ProductClass::PerpetualFuture);
    assert!(ProductClass::PerpetualFuture < ProductClass::Spot);
}

#[test]
fn a_key_carries_the_class_its_key_space_declares() {
    let key = Cash::key(&Cash::parse_id("abc").unwrap());
    assert_eq!(key.class(), ProductClass::Spot);
    assert_eq!(key.to_string(), "acme.cash:ABC");
}

/// A made-up product of another class, whose keys `Ledger` must refuse.
struct Swap;

impl KeySpace for Swap {
    const VENUE: VenueId = VenueId::from_static("acme");
    const PRODUCT: ProductId = ProductId::from_static("swap");
    const CLASS: ProductClass = ProductClass::PerpetualFuture;
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_ascii_uppercase())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

/// Implements `MarketData` and nothing else: one instrument, `acme.cash:ABC`,
/// with a one-level book.
struct Ledger;

impl Ledger {
    fn check(key: &MarketKey) -> Result<String, ClassifiedError> {
        Cash::id(key).map_err(ClassifiedError::from)
    }
}

impl MarketData for Ledger {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let id = Self::check(key)?;
        Ok(Instrument::new(key.clone(), id, SizeUnit::Base))
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        Self::check(key)?;
        let mut quote = Quote::new(key.clone());
        quote.bid = Some(Level::new(Decimal::ONE, Decimal::TWO));
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        Self::check(key)?;
        let mut book = Book::new(key.clone());
        book.bids.push(Level::new(Decimal::ONE, Decimal::TWO));
        Ok(book)
    }
}

/// What a caller generic over spot products writes: one bound.
async fn read<M: SpotMarket>(
    market: &M,
    key: &MarketKey,
) -> Result<(Quote, Book), ClassifiedError> {
    Ok((market.quote(key).await?, market.book(key).await?))
}

#[tokio::test]
async fn a_type_with_market_data_alone_satisfies_a_spot_market_bound() {
    let key = Cash::key(&"ABC".to_owned());
    let (quote, book) = read(&Ledger, &key).await.unwrap();
    assert_eq!(quote.key, key);
    assert_eq!(quote.bid, Some(Level::new(Decimal::ONE, Decimal::TWO)));
    assert_eq!(book.bids, vec![Level::new(Decimal::ONE, Decimal::TWO)]);
}

#[tokio::test]
async fn a_key_of_another_product_is_refused_through_the_bound() {
    let swap = Swap::key(&"ABC".to_owned());
    let refusal = read(&Ledger, &swap).await.unwrap_err();
    assert_eq!(refusal.class, Class::InvalidRequest);
}
