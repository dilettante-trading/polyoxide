//! Rows a venue sends as bare JSON arrays, with decimals as strings, such as
//! a kline `[open_time, "open", "high", ..]` or a book level
//! `["price", "quantity"]`.
//!
//! A row of fixed arity can derive serde on a tuple of [`DecimalStr`] and
//! integers. A row that tolerates trailing elements writes a visitor that
//! reads each one with [`element`] and skips the rest with [`drain`].

use rust_decimal::Decimal;
use serde::de::{Error as _, IgnoredAny, SeqAccess};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A decimal sent as a string. It decodes as `rust_decimal::serde::str` does,
/// so every digit is kept and scientific notation is accepted, and encodes
/// through `Display`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecimalStr(pub Decimal);

impl<'de> Deserialize<'de> for DecimalStr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        rust_decimal::serde::str::deserialize(deserializer).map(Self)
    }
}

impl Serialize for DecimalStr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

/// Reads element `index` of a positional array. An array that ends before it
/// is refused as an invalid length naming `index` and `expecting`.
pub fn element<'de, T: Deserialize<'de>, A: SeqAccess<'de>>(
    seq: &mut A,
    index: usize,
    expecting: &str,
) -> Result<T, A::Error> {
    seq.next_element()?
        .ok_or_else(|| A::Error::invalid_length(index, &expecting))
}

/// Skips whatever elements remain, so a row tolerates elements it does not
/// model.
pub fn drain<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<(), A::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use serde::de::Visitor;

    use super::*;

    /// A `[price, quantity]` row that tolerates more elements.
    #[derive(Debug, PartialEq)]
    struct Level(Decimal, Decimal);

    impl<'de> Deserialize<'de> for Level {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct LevelVisitor;

            impl<'de> Visitor<'de> for LevelVisitor {
                type Value = Level;

                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("a [price, quantity] array")
                }

                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Level, A::Error> {
                    const WHAT: &str = "a [price, quantity] array";
                    let price = element::<DecimalStr, _>(&mut seq, 0, WHAT)?.0;
                    let quantity = element::<DecimalStr, _>(&mut seq, 1, WHAT)?.0;
                    drain(&mut seq)?;
                    Ok(Level(price, quantity))
                }
            }

            deserializer.deserialize_seq(LevelVisitor)
        }
    }

    #[test]
    fn a_decimal_str_keeps_every_digit() {
        let wire = r#""0.000000000000000000000000001""#;
        let value: DecimalStr = serde_json::from_str(wire).unwrap();
        assert_eq!(value.0, Decimal::new(1, 27));
        assert_eq!(serde_json::to_string(&value).unwrap(), wire);

        let exponent: DecimalStr = serde_json::from_str(r#""1e-5""#).unwrap();
        assert_eq!(exponent.0, Decimal::new(1, 5));
        assert!(serde_json::from_str::<DecimalStr>(r#""abc""#).is_err());
        assert!(
            serde_json::from_str::<DecimalStr>("1.5").is_err(),
            "a bare number is not a decimal string"
        );
    }

    #[test]
    fn element_names_the_missing_index() {
        let err = serde_json::from_str::<Level>(r#"["7688.5"]"#).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid length 1, expected a [price, quantity] array at line 1 column 10"
        );
    }

    #[test]
    fn drain_skips_the_rest() {
        let level: Level = serde_json::from_str(r#"["7688.5","0.31605",[1,{"a":2}],"x"]"#).unwrap();
        assert_eq!(level, Level(Decimal::new(76885, 1), Decimal::new(31605, 5)));
    }
}
