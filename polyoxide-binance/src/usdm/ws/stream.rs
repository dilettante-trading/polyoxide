//! Stream names: the only way to name a USDⓈ-M market stream.
//!
//! Binance acknowledges every `SUBSCRIBE`, including a stream type that does
//! not exist, an uppercase symbol, a spelling it does not serve (`@250ms`) and
//! an unknown or delisted symbol, and none of those deliver anything. A name
//! that will never deliver is indistinguishable from a quiet one, so names are
//! built here, by construction, and never accepted as caller strings. The
//! symbol is the one part this cannot check: `exchange().exchange_info()` lists
//! the symbols that trade.

use std::{fmt, str::FromStr};

use crate::usdm::{
    types::{Interval, Symbol},
    ws::StreamPath,
};

/// Levels per side of a partial depth stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthLevels {
    /// `depth5`.
    Five,
    /// `depth10`.
    Ten,
    /// `depth20`.
    Twenty,
}

impl DepthLevels {
    /// Every value, in order.
    pub const ALL: &'static [Self] = &[Self::Five, Self::Ten, Self::Twenty];

    fn as_str(self) -> &'static str {
        match self {
            Self::Five => "5",
            Self::Ten => "10",
            Self::Twenty => "20",
        }
    }
}

/// How often a partial depth stream pushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthSpeed {
    /// `@100ms`.
    Ms100,
    /// No suffix: Binance's default. An explicit `@250ms` is acknowledged and
    /// delivers nothing (measured 2026-10-07).
    Ms250,
    /// `@500ms`.
    Ms500,
}

impl DepthSpeed {
    /// Every value, in order.
    pub const ALL: &'static [Self] = &[Self::Ms100, Self::Ms250, Self::Ms500];

    /// What follows the levels in a stream name.
    fn suffix(self) -> &'static str {
        match self {
            Self::Ms100 => "@100ms",
            Self::Ms250 => "",
            Self::Ms500 => "@500ms",
        }
    }
}

/// A USDⓈ-M market stream.
///
/// `Display` renders the wire name, with the symbol's ASCII letters
/// lowercased (`btcusdt@aggTrade`, `币安人生usdt@markPrice@1s`), and `FromStr`
/// parses an echoed name back; [`Symbol::new`] uppercases ASCII, so the two
/// round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StreamName {
    /// `!ticker@arr`: 24-hour tickers of every symbol that changed.
    AllTickers,
    /// `!markPrice@arr@1s`: mark price and funding of every symbol, each second.
    AllMarkPrices,
    /// `<s>@aggTrade`.
    AggTrade(Symbol),
    /// `<s>@kline_<interval>`.
    Kline(Symbol, Interval),
    /// `<s>@markPrice@1s`.
    MarkPrice(Symbol),
    /// `<s>@ticker`.
    Ticker(Symbol),
    /// `<s>@depth<levels>@<speed>`, on the `/public` path.
    PartialDepth(Symbol, DepthLevels, DepthSpeed),
    /// `<s>@bookTicker`, on the `/public` path.
    BookTicker(Symbol),
}

impl StreamName {
    /// The path whose connection carries this stream.
    pub fn path(&self) -> StreamPath {
        match self {
            Self::PartialDepth(..) | Self::BookTicker(_) => StreamPath::Public,
            Self::AllTickers
            | Self::AllMarkPrices
            | Self::AggTrade(_)
            | Self::Kline(..)
            | Self::MarkPrice(_)
            | Self::Ticker(_) => StreamPath::Market,
        }
    }

    /// The symbol, for a single-symbol stream.
    pub fn symbol(&self) -> Option<&Symbol> {
        match self {
            Self::AllTickers | Self::AllMarkPrices => None,
            Self::AggTrade(s)
            | Self::Kline(s, _)
            | Self::MarkPrice(s)
            | Self::Ticker(s)
            | Self::PartialDepth(s, ..)
            | Self::BookTicker(s) => Some(s),
        }
    }
}

impl fmt::Display for StreamName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lower = |s: &Symbol| s.as_str().to_ascii_lowercase();
        match self {
            Self::AllTickers => f.write_str("!ticker@arr"),
            Self::AllMarkPrices => f.write_str("!markPrice@arr@1s"),
            Self::AggTrade(s) => write!(f, "{}@aggTrade", lower(s)),
            Self::Kline(s, interval) => write!(f, "{}@kline_{interval}", lower(s)),
            Self::MarkPrice(s) => write!(f, "{}@markPrice@1s", lower(s)),
            Self::Ticker(s) => write!(f, "{}@ticker", lower(s)),
            Self::PartialDepth(s, levels, speed) => {
                write!(f, "{}@depth{}{}", lower(s), levels.as_str(), speed.suffix())
            }
            Self::BookTicker(s) => write!(f, "{}@bookTicker", lower(s)),
        }
    }
}

/// A string that is not a stream name this crate builds.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a USDⓈ-M stream name this crate supports: StreamName builds every one, with its symbol in lowercase")]
pub struct InvalidStreamName(pub String);

/// An `InvalidRequest`: the caller named a stream this crate does not build.
impl polyoxide_venue::Classify for InvalidStreamName {
    fn class(&self) -> polyoxide_venue::Class {
        polyoxide_venue::Class::InvalidRequest
    }
}

impl FromStr for StreamName {
    type Err = InvalidStreamName;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidStreamName(name.to_owned());
        match name {
            "!ticker@arr" => return Ok(Self::AllTickers),
            "!markPrice@arr@1s" => return Ok(Self::AllMarkPrices),
            _ => {}
        }
        let (symbol, kind) = name.split_once('@').ok_or_else(invalid)?;
        // A stream name spells its symbol in lowercase; one that does not was
        // not built here and, on the wire, delivers nothing.
        if symbol.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(invalid());
        }
        let symbol = Symbol::new(symbol).map_err(|_| invalid())?;
        match kind {
            "aggTrade" => Ok(Self::AggTrade(symbol)),
            "markPrice@1s" => Ok(Self::MarkPrice(symbol)),
            "ticker" => Ok(Self::Ticker(symbol)),
            "bookTicker" => Ok(Self::BookTicker(symbol)),
            _ => {
                if let Some(interval) = kind.strip_prefix("kline_") {
                    let interval = interval.parse().map_err(|_| invalid())?;
                    return Ok(Self::Kline(symbol, interval));
                }
                let depth = kind.strip_prefix("depth").ok_or_else(invalid)?;
                let (levels, speed) = DepthLevels::ALL
                    .iter()
                    .flat_map(|l| DepthSpeed::ALL.iter().map(move |s| (*l, *s)))
                    .find(|(l, s)| depth.strip_prefix(l.as_str()) == Some(s.suffix()))
                    .ok_or_else(invalid)?;
                Ok(Self::PartialDepth(symbol, levels, speed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_kind(symbol: &str) -> Vec<StreamName> {
        let s = Symbol::new(symbol).unwrap();
        let mut names = vec![
            StreamName::AllTickers,
            StreamName::AllMarkPrices,
            StreamName::AggTrade(s.clone()),
            StreamName::MarkPrice(s.clone()),
            StreamName::Ticker(s.clone()),
            StreamName::BookTicker(s.clone()),
        ];
        names.extend(
            Interval::ALL
                .iter()
                .map(|i| StreamName::Kline(s.clone(), *i)),
        );
        for levels in DepthLevels::ALL {
            for speed in DepthSpeed::ALL {
                names.push(StreamName::PartialDepth(s.clone(), *levels, *speed));
            }
        }
        names
    }

    #[test]
    fn every_name_round_trips_through_its_wire_spelling() {
        for symbol in [
            "BTCUSDT",
            "BTCUSDT_261225",
            "币安人生USDT",
            "1000PEPEUSDT",
            "ÉTÉUSDT",
        ] {
            for name in every_kind(symbol) {
                let wire = name.to_string();
                assert_eq!(wire.parse::<StreamName>(), Ok(name.clone()), "{wire}");
            }
        }
    }

    #[test]
    fn names_are_spelled_as_the_wire_spells_them() {
        let btc = Symbol::new("BTCUSDT").unwrap();
        let chinese = Symbol::new("币安人生USDT").unwrap();
        let cases = [
            (StreamName::AllTickers, "!ticker@arr"),
            (StreamName::AllMarkPrices, "!markPrice@arr@1s"),
            (StreamName::AggTrade(btc.clone()), "btcusdt@aggTrade"),
            (
                StreamName::Kline(btc.clone(), Interval::Mo1),
                "btcusdt@kline_1M",
            ),
            (StreamName::MarkPrice(chinese), "币安人生usdt@markPrice@1s"),
            (StreamName::Ticker(btc.clone()), "btcusdt@ticker"),
            (
                StreamName::PartialDepth(btc.clone(), DepthLevels::Twenty, DepthSpeed::Ms100),
                "btcusdt@depth20@100ms",
            ),
            (StreamName::BookTicker(btc), "btcusdt@bookTicker"),
        ];
        for (name, wire) in cases {
            assert_eq!(name.to_string(), wire);
        }
    }

    #[test]
    fn every_depth_name_is_one_binance_delivers() {
        // Measured 2026-10-07: each of these delivered frames; the explicit
        // `@250ms` spelling was acknowledged and delivered nothing.
        let btc = Symbol::new("BTCUSDT").unwrap();
        for (levels, speed, wire) in [
            (DepthLevels::Five, DepthSpeed::Ms100, "btcusdt@depth5@100ms"),
            (DepthLevels::Five, DepthSpeed::Ms250, "btcusdt@depth5"),
            (DepthLevels::Five, DepthSpeed::Ms500, "btcusdt@depth5@500ms"),
            (DepthLevels::Ten, DepthSpeed::Ms100, "btcusdt@depth10@100ms"),
            (DepthLevels::Ten, DepthSpeed::Ms250, "btcusdt@depth10"),
            (DepthLevels::Ten, DepthSpeed::Ms500, "btcusdt@depth10@500ms"),
            (
                DepthLevels::Twenty,
                DepthSpeed::Ms100,
                "btcusdt@depth20@100ms",
            ),
            (DepthLevels::Twenty, DepthSpeed::Ms250, "btcusdt@depth20"),
            (
                DepthLevels::Twenty,
                DepthSpeed::Ms500,
                "btcusdt@depth20@500ms",
            ),
        ] {
            let name = StreamName::PartialDepth(btc.clone(), levels, speed);
            assert_eq!(name.to_string(), wire);
            assert_eq!(wire.parse::<StreamName>(), Ok(name));
        }
    }

    #[test]
    fn depth_and_book_ticker_ride_the_public_path_and_the_rest_the_market_path() {
        for name in every_kind("BTCUSDT") {
            let expected = match name {
                StreamName::PartialDepth(..) | StreamName::BookTicker(_) => StreamPath::Public,
                _ => StreamPath::Market,
            };
            assert_eq!(name.path(), expected, "{name}");
        }
    }

    #[test]
    fn a_name_this_crate_does_not_build_does_not_parse() {
        for bad in [
            "BTCUSDT@aggTrade",
            "btcusdt@nonsense",
            "btcusdt@depth7@100ms",
            "btcusdt@depth5@250ms",
            "btcusdt@depth50",
            "btcusdt@kline_1s",
            "btcusdt",
            "!bookTicker",
            "btc usdt@ticker",
        ] {
            assert_eq!(
                bad.parse::<StreamName>(),
                Err(InvalidStreamName(bad.to_owned())),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_name_that_does_not_parse_is_an_invalid_request() {
        use polyoxide_venue::{Class, Classify};

        let err = "btcusdt".parse::<StreamName>().unwrap_err();
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(err.is_fault());
        assert_eq!(err.retry_after(), None);
        assert!(!err.is_retriable());
    }
}
