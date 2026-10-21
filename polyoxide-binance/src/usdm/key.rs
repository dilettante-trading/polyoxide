//! USDⓈ-M's market keys: `binance.usdm:<SYMBOL>`, one per contract.
//!
//! The id is the contract's [`Symbol`], whose ASCII letters are uppercased, so
//! `binance.usdm:btcusdt` and `binance.usdm:BTCUSDT` are one key.
//!
//! ```
//! use polyoxide_binance::usdm::{key, types::Symbol};
//! use polyoxide_venue::RawKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parsed = key::parse(&RawKey::parse("binance.usdm:btcusdt")?)?;
//! assert_eq!(parsed, key::key(&Symbol::new("BTCUSDT")?));
//! assert_eq!(parsed.to_string(), "binance.usdm:BTCUSDT");
//! assert_eq!(key::symbol(&parsed)?.as_str(), "BTCUSDT");
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;

use polyoxide_venue::{KeyError, KeySpace, MarketKey, ProductClass, ProductId, RawKey, VenueId};

use crate::usdm::types::Symbol;

/// Binance's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("binance");

/// USDⓈ-M's product id, as the crate registers it.
pub const PRODUCT: ProductId = ProductId::from_static("usdm");

/// USDⓈ-M's keys. Each names one contract by its [`Symbol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsdmKeys;

impl KeySpace for UsdmKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PRODUCT;
    const CLASS: ProductClass = ProductClass::PerpetualFuture;
    type Id = Symbol;

    fn parse_id(text: &str) -> Result<Symbol, String> {
        Symbol::new(text).map_err(|err| err.to_string())
    }

    fn id_text(id: &Symbol) -> Cow<'_, str> {
        Cow::Borrowed(id.as_str())
    }
}

/// The key of the contract `symbol`.
pub fn key(symbol: &Symbol) -> MarketKey {
    UsdmKeys::key(symbol)
}

/// The key `raw` names, with its symbol's ASCII letters uppercased.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `raw` is not `binance.usdm`, and
/// [`KeyError::InvalidId`] when its id is not a symbol.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    UsdmKeys::parse(raw)
}

/// The symbol `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `binance.usdm` key.
pub fn symbol(key: &MarketKey) -> Result<Symbol, KeyError> {
    UsdmKeys::id(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
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
    }

    #[test]
    fn a_lowercase_symbol_names_the_same_key() {
        let lower = parse(&raw("binance.usdm:btcusdt")).unwrap();
        let upper = parse(&raw("binance.usdm:BTCUSDT")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.to_string(), "binance.usdm:BTCUSDT");
        assert_eq!(lower, key(&Symbol::new("BTCUSDT").unwrap()));
        assert_eq!(
            parse(&raw("binance.usdm:btcusdt_261225")).unwrap().id(),
            "BTCUSDT_261225"
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
            parse(&raw("binance.usdm:BTC/USDT")),
            Err(KeyError::InvalidId { .. })
        ));
        assert!(matches!(
            parse(&raw("binance.coinm:BTCUSD_PERP")),
            Err(KeyError::WrongProduct { .. })
        ));
    }
}
