//! The symbol a Binance request names, whichever product it is sent to.

use std::{fmt, str::FromStr};

/// A Binance symbol such as `BTCUSDT`, `BTCUSDT_261225` or `币安人生USDT`.
///
/// [`Symbol::new`] accepts 1 to 32 characters, each a Unicode letter, a digit
/// or `_`. Binance lists Chinese-character symbols, and its quarterly futures
/// carry their delivery date after an underscore; on 2026-10-07 the 924
/// USDⓈ-M symbols used no other character and the longest had 17. ASCII
/// letters are uppercased: no listed symbol has a lowercase one, stream names
/// spell every symbol in lowercase, and uppercasing is what lets a stream name
/// be parsed back to the symbol that built it.
///
/// Response rows carry symbols as `String`, taken as sent, so a symbol this
/// type would refuse never fails a whole response.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(String);

/// A string [`Symbol::new`] refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a Binance symbol: 1 to 32 letters, digits or underscores")]
pub struct InvalidSymbol(pub String);

/// An `InvalidRequest`: the caller named a symbol no request can carry.
impl polyoxide_venue::Classify for InvalidSymbol {
    fn class(&self) -> polyoxide_venue::Class {
        polyoxide_venue::Class::InvalidRequest
    }
}

impl Symbol {
    /// The longest symbol [`Symbol::new`] accepts, in characters.
    pub const MAX_LEN: usize = 32;

    /// Checks a symbol and uppercases its ASCII letters.
    pub fn new(symbol: impl Into<String>) -> Result<Self, InvalidSymbol> {
        let symbol = symbol.into();
        let chars = symbol.chars().count();
        let valid = (1..=Self::MAX_LEN).contains(&chars)
            && symbol.chars().all(|c| c.is_alphanumeric() || c == '_');
        if valid {
            Ok(Self(symbol.to_ascii_uppercase()))
        } else {
            Err(InvalidSymbol(symbol))
        }
    }

    /// The symbol, as REST spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Symbol {
    type Err = InvalidSymbol;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
