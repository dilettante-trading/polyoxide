//! Stream names: the only way to name a spot market stream.
//!
//! Binance acknowledges every `SUBSCRIBE`, including a stream type that does
//! not exist, an uppercase symbol, a spelling it does not serve (`@250ms`) and
//! an unknown symbol, and none of those deliver anything. A name that will
//! never deliver is indistinguishable from a quiet one, so names are built
//! here, by construction, and never accepted as caller strings. The symbol is
//! the one part this cannot check: `exchange().exchange_info()` lists the
//! symbols that trade.

use std::{fmt, str::FromStr};

use crate::spot::{
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
    /// No suffix: every 1000 ms, Binance's default. An explicit `@250ms` or
    /// `@500ms` is not a spot spelling.
    Ms1000,
}

impl DepthSpeed {
    /// Every value, in order.
    pub const ALL: &'static [Self] = &[Self::Ms100, Self::Ms1000];

    /// What follows the levels in a stream name.
    fn suffix(self) -> &'static str {
        match self {
            Self::Ms100 => "@100ms",
            Self::Ms1000 => "",
        }
    }
}

/// A spot market stream.
///
/// `Display` renders the wire name, with the symbol's ASCII letters
/// lowercased (`btcusdt@aggTrade`, `币安人生usdt@miniTicker`), and `FromStr`
/// parses an echoed name back; [`Symbol::new`] uppercases ASCII, so the two
/// round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StreamName {
    /// `<s>@aggTrade`.
    AggTrade(Symbol),
    /// `<s>@trade`.
    Trade(Symbol),
    /// `<s>@kline_<interval>`.
    Kline(Symbol, Interval),
    /// `<s>@miniTicker`.
    MiniTicker(Symbol),
    /// `!miniTicker@arr`: the mini ticker of every symbol that changed.
    AllMiniTickers,
    /// `<s>@ticker`.
    Ticker(Symbol),
    /// `<s>@bookTicker`.
    BookTicker(Symbol),
    /// `<s>@depth<levels>`, or `<s>@depth<levels>@100ms`.
    PartialDepth(Symbol, DepthLevels, DepthSpeed),
    /// `<s>@avgPrice`.
    AvgPrice(Symbol),
}

impl StreamName {
    /// The path whose connection carries this stream: spot has one.
    pub fn path(&self) -> StreamPath {
        StreamPath::Stream
    }

    /// The symbol, for a single-symbol stream.
    pub fn symbol(&self) -> Option<&Symbol> {
        match self {
            Self::AllMiniTickers => None,
            Self::AggTrade(s)
            | Self::Trade(s)
            | Self::Kline(s, _)
            | Self::MiniTicker(s)
            | Self::Ticker(s)
            | Self::BookTicker(s)
            | Self::PartialDepth(s, ..)
            | Self::AvgPrice(s) => Some(s),
        }
    }
}

impl fmt::Display for StreamName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lower = |s: &Symbol| s.as_str().to_ascii_lowercase();
        match self {
            Self::AggTrade(s) => write!(f, "{}@aggTrade", lower(s)),
            Self::Trade(s) => write!(f, "{}@trade", lower(s)),
            Self::Kline(s, interval) => write!(f, "{}@kline_{interval}", lower(s)),
            Self::MiniTicker(s) => write!(f, "{}@miniTicker", lower(s)),
            Self::AllMiniTickers => f.write_str("!miniTicker@arr"),
            Self::Ticker(s) => write!(f, "{}@ticker", lower(s)),
            Self::BookTicker(s) => write!(f, "{}@bookTicker", lower(s)),
            Self::PartialDepth(s, levels, speed) => {
                write!(f, "{}@depth{}{}", lower(s), levels.as_str(), speed.suffix())
            }
            Self::AvgPrice(s) => write!(f, "{}@avgPrice", lower(s)),
        }
    }
}

/// A string that is not a stream name this crate builds.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a spot stream name this crate supports: StreamName builds every one, with its symbol in lowercase")]
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
        if name == "!miniTicker@arr" {
            return Ok(Self::AllMiniTickers);
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
            "trade" => Ok(Self::Trade(symbol)),
            "miniTicker" => Ok(Self::MiniTicker(symbol)),
            "ticker" => Ok(Self::Ticker(symbol)),
            "bookTicker" => Ok(Self::BookTicker(symbol)),
            "avgPrice" => Ok(Self::AvgPrice(symbol)),
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
            StreamName::AllMiniTickers,
            StreamName::AggTrade(s.clone()),
            StreamName::Trade(s.clone()),
            StreamName::MiniTicker(s.clone()),
            StreamName::Ticker(s.clone()),
            StreamName::BookTicker(s.clone()),
            StreamName::AvgPrice(s.clone()),
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
            "币安人生USDT",
            "1000SATSUSDT",
            "ÉTÉUSDT",
            "BTCUSDT_261225",
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
            (StreamName::AggTrade(btc.clone()), "btcusdt@aggTrade"),
            (StreamName::Trade(btc.clone()), "btcusdt@trade"),
            (
                StreamName::Kline(btc.clone(), Interval::Month1),
                "btcusdt@kline_1M",
            ),
            (
                StreamName::Kline(btc.clone(), Interval::Minute1),
                "btcusdt@kline_1m",
            ),
            (
                StreamName::Kline(btc.clone(), Interval::Second1),
                "btcusdt@kline_1s",
            ),
            (StreamName::MiniTicker(chinese), "币安人生usdt@miniTicker"),
            (StreamName::AllMiniTickers, "!miniTicker@arr"),
            (StreamName::Ticker(btc.clone()), "btcusdt@ticker"),
            (StreamName::BookTicker(btc.clone()), "btcusdt@bookTicker"),
            (
                StreamName::PartialDepth(btc.clone(), DepthLevels::Twenty, DepthSpeed::Ms100),
                "btcusdt@depth20@100ms",
            ),
            (StreamName::AvgPrice(btc), "btcusdt@avgPrice"),
        ];
        for (name, wire) in cases {
            assert_eq!(name.to_string(), wire);
        }
    }

    #[test]
    fn every_depth_name_is_one_binance_delivers() {
        // Measured 2026-11-02 (OBSERVED.md, `### Frames`): each of these
        // delivered frames at its cadence, the bare name every 1000 ms and the
        // `@100ms` name every 100 ms.
        let btc = Symbol::new("BTCUSDT").unwrap();
        for (levels, speed, wire) in [
            (DepthLevels::Five, DepthSpeed::Ms100, "btcusdt@depth5@100ms"),
            (DepthLevels::Five, DepthSpeed::Ms1000, "btcusdt@depth5"),
            (DepthLevels::Ten, DepthSpeed::Ms100, "btcusdt@depth10@100ms"),
            (DepthLevels::Ten, DepthSpeed::Ms1000, "btcusdt@depth10"),
            (
                DepthLevels::Twenty,
                DepthSpeed::Ms100,
                "btcusdt@depth20@100ms",
            ),
            (DepthLevels::Twenty, DepthSpeed::Ms1000, "btcusdt@depth20"),
        ] {
            let name = StreamName::PartialDepth(btc.clone(), levels, speed);
            assert_eq!(name.to_string(), wire);
            assert_eq!(wire.parse::<StreamName>(), Ok(name));
        }
        assert_eq!(DepthLevels::ALL.len() * DepthSpeed::ALL.len(), 6);
    }

    #[test]
    fn every_stream_rides_the_one_path() {
        for name in every_kind("BTCUSDT") {
            assert_eq!(name.path(), StreamPath::Stream, "{name}");
        }
    }

    #[test]
    fn only_the_array_stream_has_no_symbol() {
        for name in every_kind("BTCUSDT") {
            match name {
                StreamName::AllMiniTickers => assert_eq!(name.symbol(), None),
                _ => assert_eq!(name.symbol().map(Symbol::as_str), Some("BTCUSDT"), "{name}"),
            }
        }
    }

    #[test]
    fn a_name_this_crate_does_not_build_does_not_parse() {
        for bad in [
            // Uppercase: acknowledged by Binance and delivers nothing.
            "BTCUSDT@aggTrade",
            "BTCUSDT@trade",
            "btcUSDT@trade",
            // A stream type that does not exist.
            "btcusdt@nonsense",
            // Depth: five levels, a speed spot does not serve, a level count it
            // does not have.
            "btcusdt@depth7@100ms",
            "btcusdt@depth5@250ms",
            "btcusdt@depth5@500ms",
            "btcusdt@depth50",
            "btcusdt@depth",
            // An interval spot does not have.
            "btcusdt@kline_2s",
            "btcusdt@kline_",
            // Streams of the other product and a retired one.
            "btcusdt@markPrice@1s",
            "!ticker@arr",
            "!miniTicker@arr@1s",
            "!bookTicker",
            // Not a stream at all.
            "btcusdt",
            "",
            "@trade",
            "btc usdt@ticker",
        ] {
            assert_eq!(
                bad.parse::<StreamName>(),
                Err(InvalidStreamName(bad.to_owned())),
                "{bad:?}"
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

    #[test]
    fn every_captured_envelope_names_a_stream_this_crate_builds_and_echoes_it() {
        use crate::spot::ws::fixtures;

        let btc = Symbol::new("BTCUSDT").unwrap();
        // Each fixture, with the name its capture subscribed to.
        let expected = [
            (fixtures::AGG_TRADE, "btcusdt@aggTrade"),
            (fixtures::TRADE, "btcusdt@trade"),
            (fixtures::KLINE, "btcusdt@kline_1m"),
            (fixtures::MINI_TICKER, "btcusdt@miniTicker"),
            (fixtures::ALL_MINI_TICKERS, "!miniTicker@arr"),
            (fixtures::TICKER, "btcusdt@ticker"),
            (fixtures::BOOK_TICKER, "btcusdt@bookTicker"),
            (fixtures::PARTIAL_DEPTH, "btcusdt@depth5"),
            (fixtures::PARTIAL_DEPTH_100MS, "btcusdt@depth20@100ms"),
            (fixtures::AVG_PRICE, "btcusdt@avgPrice"),
        ];
        for (frame, wire) in expected {
            let envelope: serde_json::Value = serde_json::from_str(frame).unwrap();
            assert_eq!(envelope["stream"], wire);
            let name: StreamName = wire.parse().unwrap();
            assert_eq!(name.to_string(), wire);
        }
        let parsed: StreamName = "btcusdt@depth20@100ms".parse().unwrap();
        assert_eq!(
            parsed,
            StreamName::PartialDepth(btc, DepthLevels::Twenty, DepthSpeed::Ms100)
        );

        // The non-ASCII symbol's name is echoed as sent, and parses back to the
        // name that renders it.
        let envelope: serde_json::Value =
            serde_json::from_str(fixtures::NON_ASCII_MINI_TICKER).unwrap();
        let echoed = envelope["stream"].as_str().unwrap();
        assert!(!echoed.is_ascii(), "{echoed}");
        let name: StreamName = echoed.parse().unwrap();
        assert!(matches!(&name, StreamName::MiniTicker(s) if !s.as_str().is_ascii()));
        assert_eq!(name.to_string(), echoed);

        // And every file in `ALL` is an envelope whose name parses.
        assert_eq!(fixtures::ALL.len(), 11);
        for (stem, frame) in fixtures::ALL {
            let envelope: serde_json::Value = serde_json::from_str(frame).unwrap();
            let echoed = envelope["stream"].as_str().unwrap();
            let name: StreamName = echoed.parse().unwrap_or_else(|e| panic!("{stem}: {e}"));
            assert_eq!(name.to_string(), echoed, "{stem}");
        }
    }
}
