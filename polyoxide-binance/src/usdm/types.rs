//! Vocabulary and response rows for USDⓈ-M futures.
//!
//! Prices, quantities and rates are [`Decimal`](rust_decimal::Decimal),
//! decoded from the decimal strings Binance sends, so no value passes through
//! an `f64`. Timestamps are Unix milliseconds. Field names are the long forms;
//! where the wire uses one-letter keys (`aggTrades`, `depth`), they are serde
//! renames.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A Binance symbol such as `BTCUSDT`, `BTCUSDT_261225` or `币安人生USDT`.
///
/// [`Symbol::new`] accepts 1 to 32 characters, each a Unicode letter, a digit
/// or `_`. Binance lists Chinese-character symbols, and quarterly contracts
/// carry their delivery date after an underscore; on 2026-10-07 the 924 listed
/// symbols used no other character and the longest had 17. ASCII letters are
/// uppercased: no listed symbol has a lowercase one, stream names spell every
/// symbol in lowercase, and uppercasing is what lets a stream name be parsed
/// back to the symbol that built it.
///
/// Response rows carry symbols as `String`, taken as sent, so a symbol this
/// type would refuse never fails a whole response.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(String);

/// A string [`Symbol::new`] refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a Binance symbol: 1 to 32 letters, digits or underscores")]
pub struct InvalidSymbol(pub String);

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

/// A string that is not one of a closed set's wire spellings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{value:?} is not a valid {type_name}")]
pub struct UnknownVariant {
    /// The Rust type being parsed.
    pub type_name: &'static str,
    /// The offending input.
    pub value: String,
}

/// A closed set the client sends: one wire spelling per variant, and an `ALL`
/// table so a test can walk every spelling.
macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $( $(#[$vmeta])* #[serde(rename = $wire)] $variant, )+
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

/// A set Binance reports and extends over time: a value this version does not
/// know is kept verbatim in `Other` instead of failing the response.
macro_rules! open_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this version of the SDK does not recognise, kept verbatim.
            Other(String),
        }

        impl $name {
            /// Every variant this SDK knows, in declaration order.
            pub const ALL: &'static [Self] = &[$( Self::$variant ),+];

            /// The wire spelling.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Other(raw) => raw,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = std::convert::Infallible;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(match s {
                    $( $wire => Self::$variant, )+
                    other => Self::Other(other.to_owned()),
                })
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                let Ok(value) = raw.parse();
                Ok(value)
            }
        }
    };
}

wire_enum! {
    /// Kline width, shared by REST `klines` and the kline stream. The docs'
    /// list also has `1s`, which this host refuses (`-1120`).
    Interval {
        M1 => "1m", M3 => "3m", M5 => "5m", M15 => "15m", M30 => "30m",
        H1 => "1h", H2 => "2h", H4 => "4h", H6 => "6h", H8 => "8h", H12 => "12h",
        D1 => "1d", D3 => "3d", W1 => "1w",
        /// One calendar month.
        Mo1 => "1M",
    }
}

wire_enum! {
    /// Levels per side `depth` can return. Any other `limit` is refused
    /// (`-4021`).
    DepthLimit {
        Five => "5", Ten => "10", Twenty => "20", Fifty => "50",
        Hundred => "100", FiveHundred => "500", Thousand => "1000",
    }
}

open_enum! {
    /// A contract's type, from `exchangeInfo`.
    ContractType {
        Perpetual => "PERPETUAL",
        /// A perpetual on a traditional-finance underlying (equities, metals, FX).
        TradifiPerpetual => "TRADIFI_PERPETUAL",
        CurrentMonth => "CURRENT_MONTH",
        NextMonth => "NEXT_MONTH",
        CurrentQuarter => "CURRENT_QUARTER",
        NextQuarter => "NEXT_QUARTER",
        PerpetualDelivering => "PERPETUAL_DELIVERING",
    }
}

open_enum! {
    /// A contract's status, from `exchangeInfo`. The variants are the list in
    /// Binance's common definitions; `exchangeInfo` used three of them on
    /// 2026-10-07.
    SymbolStatus {
        PendingTrading => "PENDING_TRADING",
        Trading => "TRADING",
        PreDelivering => "PRE_DELIVERING",
        Delivering => "DELIVERING",
        Delivered => "DELIVERED",
        PreSettle => "PRE_SETTLE",
        /// A delisted perpetual. 134 of 924 symbols on 2026-10-07.
        Settling => "SETTLING",
        Close => "CLOSE",
        TradingHalt => "TRADING_HALT",
        TradingCancelOnly => "TRADING_CANCEL_ONLY",
    }
}

open_enum! {
    /// What a contract's underlying is, from `exchangeInfo`. The values seen on
    /// 2026-10-07; Binance's docs give no list.
    UnderlyingType {
        Coin => "COIN",
        Index => "INDEX",
        Premarket => "PREMARKET",
        Commodity => "COMMODITY",
        Equity => "EQUITY",
        CnEquity => "CN_EQUITY",
        HkEquity => "HK_EQUITY",
        KrEquity => "KR_EQUITY",
        Fx => "FX",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_take_letters_digits_and_underscores_in_any_script() {
        for good in ["BTCUSDT", "BTCUSDT_261225", "币安人生USDT", "1000PEPEUSDT"] {
            assert_eq!(Symbol::new(good).unwrap().as_str(), good);
        }
        for bad in [
            "",
            "BTC USDT",
            "BTC@USDT",
            "BTC/USDT",
            "BTC-USDT",
            &"A".repeat(33),
        ] {
            assert_eq!(
                Symbol::new(bad),
                Err(InvalidSymbol(bad.to_owned())),
                "{bad:?}"
            );
        }
        assert!(Symbol::new("A".repeat(32)).is_ok());
    }

    #[test]
    fn ascii_letters_are_uppercased_and_nothing_else_changes() {
        assert_eq!(
            Symbol::new("btcusdt").unwrap(),
            Symbol::new("BTCUSDT").unwrap()
        );
        assert_eq!(
            Symbol::new("btcusdt_261225").unwrap().as_str(),
            "BTCUSDT_261225"
        );
        assert_eq!(
            Symbol::new("币安人生usdt").unwrap().as_str(),
            "币安人生USDT"
        );
    }

    #[test]
    fn symbols_are_measured_in_characters_and_only_ascii_changes_case() {
        // `to_uppercase` would turn ß into SS, and `len` would count bytes.
        assert_eq!(Symbol::new("straße").unwrap().as_str(), "STRAßE");
        assert!(Symbol::new("币".repeat(32)).is_ok());
        assert!(Symbol::new("币".repeat(33)).is_err());
    }

    #[test]
    fn wire_spellings_are_binance_s() {
        // `as_str` and `from_str` share each literal, so a round trip cannot
        // see a wrong one; these are the spellings the host accepted.
        let intervals: Vec<&str> = Interval::ALL.iter().map(|i| i.as_str()).collect();
        assert_eq!(
            intervals,
            [
                "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "6h", "8h", "12h", "1d", "3d",
                "1w", "1M"
            ]
        );
        let limits: Vec<&str> = DepthLimit::ALL.iter().map(|l| l.as_str()).collect();
        assert_eq!(limits, ["5", "10", "20", "50", "100", "500", "1000"]);
    }

    #[test]
    fn every_interval_round_trips_its_wire_spelling() {
        assert_eq!(Interval::ALL.len(), 15);
        for interval in Interval::ALL {
            assert_eq!(interval.as_str().parse::<Interval>(), Ok(*interval));
        }
        assert!("1s".parse::<Interval>().is_err(), "this host refuses 1s");
    }

    #[test]
    fn an_unknown_contract_type_is_kept_verbatim() {
        let parsed: ContractType = serde_json::from_str(r#""NEXT_DECADE""#).unwrap();
        assert_eq!(parsed, ContractType::Other("NEXT_DECADE".to_owned()));
        assert_eq!(serde_json::to_string(&parsed).unwrap(), r#""NEXT_DECADE""#);
        let known: ContractType = serde_json::from_str(r#""TRADIFI_PERPETUAL""#).unwrap();
        assert_eq!(known, ContractType::TradifiPerpetual);
    }
}
