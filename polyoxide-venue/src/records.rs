//! The records every venue's market data is read into.
//!
//! A record holds what venues share, as fields, and a venue's own facts in its
//! [`Extensions`]. A field a venue does not publish is an `Option`. Prices and
//! sizes are [`Decimal`]; a size is in the instrument's native quantity, which
//! [`Instrument::size_unit`] names, and an amount in the quote currency is a
//! separate `notional`.

use rust_decimal::Decimal;

use crate::{Extensions, MarketKey};

/// Which side of the book an order, or the taker of a trade, is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Buying, against the asks.
    Buy,
    /// Selling, against the bids.
    Sell,
}

/// One price level of a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Level {
    /// The price, in the instrument's quote currency.
    pub price: Decimal,
    /// The size resting at the price, in the instrument's native quantity.
    pub size: Decimal,
}

impl Level {
    /// A level of `size` at `price`.
    pub fn new(price: Decimal, size: Decimal) -> Self {
        Self { price, size }
    }
}

/// What an instrument's sizes count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SizeUnit {
    /// Units of the base asset, such as the coins a coin-quoted contract is
    /// written in.
    Base,
    /// Contracts, each a fixed amount of the underlying the venue defines.
    Contracts,
    /// Outcome shares of an event contract, each paying one unit of the
    /// settlement currency if its outcome happens.
    Shares,
}

/// An instrument a venue lists, and the one record that carries its trading
/// rules: tick size, minimum size and [`SizeUnit`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Instrument {
    /// The instrument's key.
    pub key: MarketKey,
    /// A name for people: the venue's symbol, or for an outcome of an event
    /// contract, its market and outcome.
    pub name: String,
    /// The asset the instrument's sizes are written in, when the venue names
    /// one.
    pub base_asset: Option<String>,
    /// The currency its prices are quoted in, when the venue names one.
    pub quote_asset: Option<String>,
    /// What its sizes count.
    pub size_unit: SizeUnit,
    /// The smallest price step, when the venue publishes a fixed one.
    pub tick_size: Option<Decimal>,
    /// The smallest order size, in native quantity.
    pub min_size: Option<Decimal>,
    /// The smallest size step, in native quantity.
    pub size_increment: Option<Decimal>,
    /// The smallest order value, in the quote currency.
    pub min_notional: Option<Decimal>,
    /// Whether the venue takes orders on it now.
    pub tradable: Option<bool>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Instrument {
    /// An instrument with its key, name and size unit, and nothing else known.
    pub fn new(key: MarketKey, name: impl Into<String>, size_unit: SizeUnit) -> Self {
        Self {
            key,
            name: name.into(),
            base_asset: None,
            quote_asset: None,
            size_unit,
            tick_size: None,
            min_size: None,
            size_increment: None,
            min_notional: None,
            tradable: None,
            extensions: Extensions::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::{KeySpace, ProductId, VenueId};

    struct Spot;

    impl KeySpace for Spot {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("spot");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            Ok(text.to_owned())
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct SpotInstrumentExt {
        lot: u32,
    }

    fn instrument(lot: u32) -> Instrument {
        let mut instrument = Instrument::new(Spot::key(&"ABC".to_owned()), "ABC", SizeUnit::Base);
        instrument.tick_size = Some(Decimal::new(1, 2));
        instrument
            .extensions
            .insert(SpotInstrumentExt { lot })
            .unwrap();
        instrument
    }

    #[test]
    fn records_are_clone_debug_and_partial_eq() {
        fn holds<T: Clone + std::fmt::Debug + PartialEq>() {}
        holds::<Instrument>();
        holds::<Level>();
        holds::<Side>();
        holds::<SizeUnit>();
    }

    #[test]
    fn two_records_differing_only_in_one_extension_value_are_unequal() {
        assert_eq!(instrument(1), instrument(1));
        assert_ne!(instrument(1), instrument(2));
    }

    #[test]
    fn a_new_instrument_knows_only_what_it_was_given() {
        let key = Spot::key(&"ABC".to_owned());
        let instrument = Instrument::new(key.clone(), "ABC", SizeUnit::Shares);
        assert_eq!(instrument.key, key);
        assert_eq!(instrument.name, "ABC");
        assert_eq!(instrument.base_asset, None);
        assert_eq!(instrument.quote_asset, None);
        assert_eq!(instrument.size_unit, SizeUnit::Shares);
        assert_eq!(instrument.tick_size, None);
        assert_eq!(instrument.min_size, None);
        assert_eq!(instrument.size_increment, None);
        assert_eq!(instrument.min_notional, None);
        assert_eq!(instrument.tradable, None);
        assert!(instrument.extensions.is_empty());
    }

    #[test]
    fn a_level_holds_its_price_and_native_size() {
        let level = Level::new(Decimal::new(84_1255, 1), Decimal::new(14_663, 3));
        assert_eq!(level.price.to_string(), "84125.5");
        assert_eq!(level.size.to_string(), "14.663");
    }
}
