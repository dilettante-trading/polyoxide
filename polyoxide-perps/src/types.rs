//! Vocabulary shared by every namespace: identifiers, closed sets the spec
//! enumerates, and the positional rows the host sends as bare arrays.

use std::{fmt, str::FromStr};

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A perps instrument id. One newtype so a REST parameter, a WebSocket channel
/// name and, later, a signed op cannot be handed a bare integer for the wrong
/// market.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentId(pub u64);

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for InstrumentId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// A string that is not one of a closed set's wire spellings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{value:?} is not a valid {type_name}")]
pub struct UnknownVariant {
    /// The Rust type being parsed.
    pub type_name: &'static str,
    /// The offending input.
    pub value: String,
}

/// A closed set with one wire spelling per variant. Generates serde renames,
/// `Display`, `FromStr` and an `ALL` table, so every spelling lives in one
/// place and the agreement test can walk them.
macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $( #[serde(rename = $wire)] $variant, )+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [$name] = &[$( $name::$variant, )+];

            /// The wire spelling.
            pub fn as_str(self) -> &'static str {
                match self { $( $name::$variant => $wire, )+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = UnknownVariant;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $wire => Ok($name::$variant), )+
                    _ => Err(UnknownVariant { type_name: stringify!($name), value: s.to_owned() }),
                }
            }
        }
    };
}

wire_enum! {
    /// Kline and mark-history bucket width. Also the `klines` channel suffix.
    Interval {
        S1 => "1s", M1 => "1m", M5 => "5m", M15 => "15m", M30 => "30m",
        H1 => "1h", H4 => "4h", H6 => "6h", H12 => "12h", D1 => "1d", W1 => "1w",
    }
}

wire_enum! {
    /// Side of a trade or position.
    Side { Long => "long", Short => "short" }
}

wire_enum! {
    /// Instrument type. Only perpetuals are listed today.
    InstrumentType { Perpetual => "perpetual" }
}

wire_enum! {
    /// Instrument category.
    InstrumentCategory { Equity => "equity", Commodity => "commodity", Index => "index", Crypto => "crypto" }
}

wire_enum! {
    /// Leaderboard window.
    LeaderboardWindow { Day => "day", Week => "week", Month => "month", All => "all" }
}

wire_enum! {
    /// Leaderboard ranking key.
    LeaderboardSort { Pnl => "pnl", Notional => "notional", AccountValue => "account_value" }
}

wire_enum! {
    /// Sort direction for paged history.
    SortOrder { Desc => "desc", Asc => "asc" }
}

/// Levels per side that `GET /v1/info/book` can return. The WebSocket `book`
/// channel takes a different set (20 or 50) and has its own type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookDepth {
    /// Ten levels per side.
    Ten,
    /// One hundred levels per side, the server default.
    Hundred,
    /// Five hundred levels per side.
    FiveHundred,
    /// One thousand levels per side.
    Thousand,
}

impl BookDepth {
    /// Every variant, in ascending order.
    pub const ALL: &'static [BookDepth] = &[
        BookDepth::Ten,
        BookDepth::Hundred,
        BookDepth::FiveHundred,
        BookDepth::Thousand,
    ];

    /// The number of levels per side.
    pub fn levels(self) -> u32 {
        match self {
            BookDepth::Ten => 10,
            BookDepth::Hundred => 100,
            BookDepth::FiveHundred => 500,
            BookDepth::Thousand => 1000,
        }
    }
}

impl fmt::Display for BookDepth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.levels())
    }
}

fn parse_decimal<E: serde::de::Error>(s: &str) -> Result<Decimal, E> {
    s.parse::<Decimal>().map_err(E::custom)
}

/// One candle. On the wire this is a positional array:
/// `[open_time, open, high, low, close, volume, trades]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Kline {
    /// Bucket open time, Unix milliseconds.
    pub open_time: u64,
    /// Open price.
    pub open: Decimal,
    /// High price.
    pub high: Decimal,
    /// Low price.
    pub low: Decimal,
    /// Close price.
    pub close: Decimal,
    /// Volume in contracts.
    pub volume: Decimal,
    /// Number of trades in the bucket.
    pub trades: u64,
}

#[derive(Serialize, Deserialize)]
struct KlineWire(u64, String, String, String, String, String, u64);

impl<'de> Deserialize<'de> for Kline {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let KlineWire(open_time, open, high, low, close, volume, trades) =
            KlineWire::deserialize(deserializer)?;
        Ok(Self {
            open_time,
            open: parse_decimal(&open)?,
            high: parse_decimal(&high)?,
            low: parse_decimal(&low)?,
            close: parse_decimal(&close)?,
            volume: parse_decimal(&volume)?,
            trades,
        })
    }
}

impl Serialize for Kline {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        KlineWire(
            self.open_time,
            self.open.to_string(),
            self.high.to_string(),
            self.low.to_string(),
            self.close.to_string(),
            self.volume.to_string(),
            self.trades,
        )
        .serialize(serializer)
    }
}

/// One mark-price sample. On the wire: `[time, mark_price]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MarkPoint {
    /// Bucket open time, Unix milliseconds.
    pub time: u64,
    /// Last mark price in the bucket.
    pub mark_price: Decimal,
}

impl<'de> Deserialize<'de> for MarkPoint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (time, mark_price): (u64, String) = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            time,
            mark_price: parse_decimal(&mark_price)?,
        })
    }
}

impl Serialize for MarkPoint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.time, self.mark_price.to_string()).serialize(serializer)
    }
}

/// One order-book level. On the wire: `[price, quantity]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Level {
    /// Price.
    pub price: Decimal,
    /// Quantity in contracts.
    pub quantity: Decimal,
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (price, quantity): (String, String) = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            price: parse_decimal(&price)?,
            quantity: parse_decimal(&quantity)?,
        })
    }
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.price.to_string(), self.quantity.to_string()).serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_enum_serializes_to_the_wire_spelling_and_displays_the_same() {
        fn check<T: Serialize + Copy + std::fmt::Display + FromStr + PartialEq + std::fmt::Debug>(
            all: &[T],
            wire: &[&str],
        ) where
            <T as FromStr>::Err: std::fmt::Debug,
        {
            assert_eq!(all.len(), wire.len());
            for (variant, expected) in all.iter().zip(wire) {
                assert_eq!(
                    serde_json::to_string(variant).unwrap(),
                    format!("\"{expected}\"")
                );
                assert_eq!(variant.to_string(), *expected);
                assert_eq!(expected.parse::<T>().unwrap(), *variant);
            }
        }
        check(
            Interval::ALL,
            &[
                "1s", "1m", "5m", "15m", "30m", "1h", "4h", "6h", "12h", "1d", "1w",
            ],
        );
        check(Side::ALL, &["long", "short"]);
        check(InstrumentType::ALL, &["perpetual"]);
        check(
            InstrumentCategory::ALL,
            &["equity", "commodity", "index", "crypto"],
        );
        check(LeaderboardWindow::ALL, &["day", "week", "month", "all"]);
        check(LeaderboardSort::ALL, &["pnl", "notional", "account_value"]);
        check(SortOrder::ALL, &["desc", "asc"]);
    }

    #[test]
    fn an_unknown_spelling_names_the_type_and_the_value() {
        let err = "2m".parse::<Interval>().unwrap_err();
        assert_eq!(err.to_string(), "\"2m\" is not a valid Interval");
    }

    #[test]
    fn book_depth_displays_its_level_count() {
        assert_eq!(BookDepth::Ten.to_string(), "10");
        assert_eq!(BookDepth::Thousand.levels(), 1000);
        assert_eq!(BookDepth::ALL.len(), 4);
    }

    #[test]
    fn instrument_id_is_a_transparent_integer() {
        let id: InstrumentId = serde_json::from_str("7").unwrap();
        assert_eq!(id, InstrumentId(7));
        assert_eq!(serde_json::to_string(&id).unwrap(), "7");
        assert_eq!(id.to_string(), "7");
    }

    #[test]
    fn a_kline_round_trips_through_its_positional_wire_form() {
        // Captured 2026-09-30 from /v1/info/klines.
        let wire = r#"[1790758080000,"7689.7","7689.7","7689.6","7689.6","1.47861",3]"#;
        let kline: Kline = serde_json::from_str(wire).unwrap();
        assert_eq!(kline.open_time, 1790758080000);
        assert_eq!(kline.high, Decimal::new(76897, 1));
        assert_eq!(kline.trades, 3);
        assert_eq!(serde_json::to_string(&kline).unwrap(), wire);
    }

    #[test]
    fn a_kline_with_the_wrong_arity_is_rejected() {
        assert!(serde_json::from_str::<Kline>(r#"[1,"2","3"]"#).is_err());
        assert!(serde_json::from_str::<Kline>(r#"[1,"2","3","4","5","6",7,8]"#).is_err());
    }

    #[test]
    fn a_kline_with_a_non_numeric_price_is_rejected() {
        assert!(serde_json::from_str::<Kline>(r#"[1,"abc","3","4","5","6",7]"#).is_err());
    }

    #[test]
    fn a_mark_point_and_a_level_round_trip() {
        let point: MarkPoint = serde_json::from_str(r#"[1790668800000,"7684.7"]"#).unwrap();
        assert_eq!(point.mark_price, Decimal::new(76847, 1));
        assert_eq!(
            serde_json::to_string(&point).unwrap(),
            r#"[1790668800000,"7684.7"]"#
        );

        let level: Level = serde_json::from_str(r#"["7688.5","0.31605"]"#).unwrap();
        assert_eq!(level.quantity, Decimal::new(31605, 5));
        assert_eq!(
            serde_json::to_string(&level).unwrap(),
            r#"["7688.5","0.31605"]"#
        );
    }
}
