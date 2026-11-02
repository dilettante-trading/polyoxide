#![cfg(all(feature = "keys", feature = "market"))]
//! `ProductClass::Spot` and the `SpotMarket` bundle, proved on a made-up venue.

use std::borrow::Cow;

use polyoxide_venue::{KeySpace, ProductClass, ProductId, VenueId};

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
