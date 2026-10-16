//! Polymarket's market keys, one place for each of its products' constructors.
//!
//! - `polymarket.clob:<token id>` names one outcome token, which an order buys
//!   or sells, and whose book is bids and asks in that outcome's own price
//!   terms. Gamma and data name the clob's outcomes with it, so it is built
//!   with any of `clob`, `gamma` and `data`.
//! - `polymarket.perps:<instrument id>` names one perpetual (`perps`).
//!
//! ```
//! # #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
//! # fn main() -> Result<(), polyoxide_venue::KeyError> {
//! use polyoxide_polymarket::shared::keys::{self, TokenId};
//! use polyoxide_venue::RawKey;
//!
//! let token = TokenId::new("71321045679252212594626385532706912750332728571942532289631379312455583992563")?;
//! let key = keys::parse(&RawKey::parse(&format!("polymarket.clob:{token}"))?)?;
//! assert_eq!(key, keys::clob(&token));
//! assert_eq!(keys::token_id(&key)?, token);
//! # Ok(())
//! # }
//! # #[cfg(not(any(feature = "clob", feature = "gamma", feature = "data")))]
//! # fn main() {}
//! ```

use std::borrow::Cow;
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
use std::fmt;

use polyoxide_venue::{KeyError, MarketKey, RawKey, VenueId};
#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "perps"
))]
use polyoxide_venue::{KeySpace, ProductId};

#[cfg(feature = "perps")]
use crate::perps::types::InstrumentId;

/// Polymarket's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("polymarket");

/// The clob's product id, as the crate registers it.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
pub const CLOB: ProductId = ProductId::from_static("clob");

/// Perps' product id, as the crate registers it.
#[cfg(feature = "perps")]
pub const PERPS: ProductId = ProductId::from_static("perps");

/// An outcome token's id: a 256-bit integer in decimal, as the clob and gamma
/// send it, with no sign and no leading zero.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenId(String);

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
impl TokenId {
    /// The most digits a token id has: 2^256 − 1 has 78.
    pub const MAX_DIGITS: usize = 78;

    /// Checks a token id.
    ///
    /// # Errors
    ///
    /// [`KeyError::InvalidId`] unless `id` is 1 to 78 decimal digits with no
    /// leading zero.
    pub fn new(id: impl Into<String>) -> Result<Self, KeyError> {
        let id = id.into();
        match check_token_id(&id) {
            Ok(()) => Ok(Self(id)),
            Err(reason) => Err(KeyError::InvalidId {
                product: format!("{VENUE}.{CLOB}"),
                id,
                reason: reason.to_owned(),
            }),
        }
    }

    /// The id, as the clob spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
impl fmt::Display for TokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
fn check_token_id(id: &str) -> Result<(), &'static str> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("a token id is decimal digits");
    }
    if id.len() > 1 && id.starts_with('0') {
        return Err("a token id has no leading zero");
    }
    if id.len() > TokenId::MAX_DIGITS {
        return Err("a token id has at most 78 digits");
    }
    Ok(())
}

/// The clob's keys. Each names one outcome token by its [`TokenId`].
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClobKeys;

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
impl KeySpace for ClobKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = CLOB;
    type Id = TokenId;

    fn parse_id(text: &str) -> Result<TokenId, String> {
        check_token_id(text)
            .map(|()| TokenId(text.to_owned()))
            .map_err(str::to_owned)
    }

    fn id_text(id: &TokenId) -> Cow<'_, str> {
        Cow::Borrowed(id.as_str())
    }
}

/// The key of the outcome token `token_id`.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
pub fn clob(token_id: &TokenId) -> MarketKey {
    ClobKeys::key(token_id)
}

/// The outcome token `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `polymarket.clob` key.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
pub fn token_id(key: &MarketKey) -> Result<TokenId, KeyError> {
    ClobKeys::id(key)
}

/// Perps' keys. Each names one perpetual by its [`InstrumentId`], written in
/// decimal without leading zeros.
#[cfg(feature = "perps")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PerpsKeys;

#[cfg(feature = "perps")]
impl KeySpace for PerpsKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PERPS;
    type Id = InstrumentId;

    fn parse_id(text: &str) -> Result<InstrumentId, String> {
        text.parse::<u64>()
            .map(InstrumentId)
            .map_err(|err| format!("an instrument id is an unsigned integer: {err}"))
    }

    fn id_text(id: &InstrumentId) -> Cow<'_, str> {
        Cow::Owned(id.to_string())
    }
}

/// The key of the perpetual `id`.
#[cfg(feature = "perps")]
pub fn perps(id: InstrumentId) -> MarketKey {
    PerpsKeys::key(&id)
}

/// The perpetual `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `polymarket.perps` key.
#[cfg(feature = "perps")]
pub fn instrument_id(key: &MarketKey) -> Result<InstrumentId, KeyError> {
    PerpsKeys::id(key)
}

/// The key `raw` names, by the constructors of the Polymarket product it
/// names, among those this build enables.
///
/// # Errors
///
/// [`KeyError::UnknownProduct`] when `raw` names no product this build
/// enables, and the product's own error when its id does not parse.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    if raw.venue() == &VENUE {
        #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
        if raw.product() == &CLOB {
            return ClobKeys::parse(raw);
        }
        #[cfg(feature = "perps")]
        if raw.product() == &PERPS {
            return PerpsKeys::parse(raw);
        }
    }
    Err(KeyError::unknown(raw))
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
        #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == CLOB.as_str()),
            "{declared:?}"
        );
        #[cfg(feature = "perps")]
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == PERPS.as_str()),
            "{declared:?}"
        );
    }

    #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
    #[test]
    fn a_token_id_is_decimal_digits_without_a_leading_zero() {
        let longest = "9".repeat(TokenId::MAX_DIGITS);
        assert!(TokenId::new(longest.as_str()).is_ok());
        assert!(TokenId::new("0").is_ok());
        for bad in ["", "0x12", "-1", "+1", "1e5", "007", " 1", &"9".repeat(79)] {
            assert!(
                matches!(TokenId::new(bad), Err(KeyError::InvalidId { .. })),
                "{bad:?}"
            );
        }
    }

    #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
    #[test]
    fn a_clob_key_names_its_token_and_gives_it_back() {
        let token = TokenId::new(
            "71321045679252212594626385532706912750332728571942532289631379312455583992563",
        )
        .unwrap();
        let key = parse(&raw(&format!("polymarket.clob:{token}"))).unwrap();
        assert_eq!(key, clob(&token));
        assert_eq!(key.to_string(), format!("polymarket.clob:{token}"));
        assert_eq!(token_id(&key), Ok(token));
        assert!(matches!(
            parse(&raw("polymarket.clob:0x1234")),
            Err(KeyError::InvalidId { .. })
        ));
    }

    #[cfg(feature = "perps")]
    #[test]
    fn a_perps_key_names_its_instrument_without_leading_zeros() {
        let key = parse(&raw("polymarket.perps:007")).unwrap();
        assert_eq!(key, perps(InstrumentId(7)));
        assert_eq!(key.id(), "7");
        assert_eq!(instrument_id(&key), Ok(InstrumentId(7)));
        assert!(matches!(
            parse(&raw("polymarket.perps:seven")),
            Err(KeyError::InvalidId { .. })
        ));
    }

    #[test]
    fn a_product_the_build_does_not_enable_is_unknown() {
        for text in ["polymarket.rtds:btcusdt", "binance.usdm:BTCUSDT"] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::UnknownProduct { .. })),
                "{text}"
            );
        }
    }
}
