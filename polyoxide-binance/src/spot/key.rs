//! Spot's market keys: `binance.spot:<SYMBOL>`, one per trading pair.
//!
//! The id is the pair's [`Symbol`], whose ASCII letters are uppercased, so
//! `binance.spot:btcusdt` and `binance.spot:BTCUSDT` are one key.
//!
//! ```
//! use polyoxide_binance::spot::{key, types::Symbol};
//! use polyoxide_venue::RawKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parsed = key::parse(&RawKey::parse("binance.spot:btcusdt")?)?;
//! assert_eq!(parsed, key::key(&Symbol::new("BTCUSDT")?));
//! assert_eq!(parsed.to_string(), "binance.spot:BTCUSDT");
//! assert_eq!(key::symbol(&parsed)?.as_str(), "BTCUSDT");
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;

use polyoxide_venue::{KeyError, KeySpace, MarketKey, ProductClass, ProductId, RawKey, VenueId};

use crate::spot::types::Symbol;

/// Binance's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("binance");

/// Spot's product id, as the crate registers it.
pub const PRODUCT: ProductId = ProductId::from_static("spot");

/// Spot's keys. Each names one trading pair by its [`Symbol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpotKeys;

impl KeySpace for SpotKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PRODUCT;
    const CLASS: ProductClass = ProductClass::Spot;
    type Id = Symbol;

    fn parse_id(text: &str) -> Result<Symbol, String> {
        Symbol::new(text).map_err(|err| err.to_string())
    }

    fn id_text(id: &Symbol) -> Cow<'_, str> {
        Cow::Borrowed(id.as_str())
    }
}

/// The key of the pair `symbol`.
pub fn key(symbol: &Symbol) -> MarketKey {
    SpotKeys::key(symbol)
}

/// The key `raw` names, with its symbol's ASCII letters uppercased.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `raw` is not `binance.spot`, and
/// [`KeyError::InvalidId`] when its id is not a symbol.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    SpotKeys::parse(raw)
}

/// The symbol `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `binance.spot` key.
pub fn symbol(key: &MarketKey) -> Result<Symbol, KeyError> {
    SpotKeys::id(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    #[test]
    fn a_lowercase_symbol_names_the_same_key() {
        let lower = parse(&raw("binance.spot:btcusdt")).unwrap();
        let upper = parse(&raw("binance.spot:BTCUSDT")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.to_string(), "binance.spot:BTCUSDT");
        assert_eq!(lower, key(&Symbol::new("BTCUSDT").unwrap()));
        assert_eq!(
            parse(&raw("binance.spot:1000satsusdt")).unwrap().id(),
            "1000SATSUSDT"
        );
    }

    #[test]
    fn a_key_gives_back_its_symbol() {
        let btc = Symbol::new("BTCUSDT").unwrap();
        assert_eq!(symbol(&key(&btc)), Ok(btc));
    }

    #[test]
    fn a_symbol_binance_cannot_carry_and_another_product_are_refused() {
        assert!(matches!(
            parse(&raw("binance.spot:BTC/USDT")),
            Err(KeyError::InvalidId { .. })
        ));
        for other in ["binance.usdm:BTCUSDT", "binance.margin:BTCUSDT"] {
            assert!(
                matches!(parse(&raw(other)), Err(KeyError::WrongProduct { .. })),
                "{other}"
            );
        }
    }

    #[test]
    fn the_ids_are_the_ones_the_crate_registers() {
        let declared = polyoxide_test_support::registration!();
        assert_eq!(declared.venue.as_deref(), Some(VENUE.as_str()));
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == PRODUCT.as_str()),
            "{declared:?}"
        );
        assert_eq!(
            declared.classes.get(PRODUCT.as_str()).map(String::as_str),
            Some(SpotKeys::CLASS.as_str()),
            "{declared:?}"
        );
        assert_eq!(SpotKeys::CLASS.as_str(), "spot");
    }
}
