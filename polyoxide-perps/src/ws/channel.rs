//! Subscription names. The only way to name a channel, so a malformed name is
//! a compile-time impossibility rather than a silent `invalid channel` reply.

use std::{fmt, str::FromStr};

use polyoxide_venue::UnknownVariant;

use crate::types::{InstrumentId, Interval};

/// Levels per side the `book` channel can deliver. The REST route takes a
/// different set (`crate::types::BookDepth`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StreamDepth {
    /// Top 20 levels per side, the server default.
    #[default]
    Twenty,
    /// Top 50 levels per side.
    Fifty,
}

impl StreamDepth {
    /// The number of levels per side.
    pub fn levels(self) -> u32 {
        match self {
            StreamDepth::Twenty => 20,
            StreamDepth::Fifty => 50,
        }
    }
}

/// A public channel, parameterised by instrument.
///
/// `Tickers(None)` and `Statistics(None)` are the `::all` subscriptions. They
/// never appear as a frame label: the server fans them out as one frame per
/// instrument, labelled `tickers::N`. [`Channel::covers`] relates the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Channel {
    /// Best bid and offer, pushed on change.
    Bbo(InstrumentId),
    /// Order-book snapshot, top 20 or 50 levels per side.
    Book(InstrumentId, StreamDepth),
    /// Public trades.
    Trades(InstrumentId),
    /// Candles at one interval.
    Klines(InstrumentId, Interval),
    /// Ticker for one instrument, or every instrument.
    Tickers(Option<InstrumentId>),
    /// 24-hour statistics for one instrument, or every instrument.
    Statistics(Option<InstrumentId>),
}

impl Channel {
    /// Whether a frame labelled `label` belongs to this subscription.
    pub fn covers(&self, label: &Channel) -> bool {
        match (self, label) {
            (Channel::Tickers(None), Channel::Tickers(Some(_))) => true,
            (Channel::Statistics(None), Channel::Statistics(Some(_))) => true,
            (a, b) => a == b,
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Channel::Bbo(iid) => write!(f, "bbo::{iid}"),
            Channel::Book(iid, depth) => write!(f, "book::{iid}::{}", depth.levels()),
            Channel::Trades(iid) => write!(f, "trades::{iid}"),
            Channel::Klines(iid, interval) => write!(f, "klines::{iid}::{interval}"),
            Channel::Tickers(Some(iid)) => write!(f, "tickers::{iid}"),
            Channel::Tickers(None) => f.write_str("tickers::all"),
            Channel::Statistics(Some(iid)) => write!(f, "statistics::{iid}"),
            Channel::Statistics(None) => f.write_str("statistics::all"),
        }
    }
}

fn unknown(s: &str) -> UnknownVariant {
    UnknownVariant {
        type_name: "Channel",
        value: s.to_owned(),
    }
}

fn iid(part: Option<&str>, whole: &str) -> Result<InstrumentId, UnknownVariant> {
    part.and_then(|p| p.parse::<u64>().ok())
        .map(InstrumentId)
        .ok_or_else(|| unknown(whole))
}

fn iid_or_all(part: Option<&str>, whole: &str) -> Result<Option<InstrumentId>, UnknownVariant> {
    match part {
        Some("all") => Ok(None),
        other => iid(other, whole).map(Some),
    }
}

impl FromStr for Channel {
    type Err = UnknownVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split("::");
        let kind = parts.next().ok_or_else(|| unknown(s))?;
        let second = parts.next();
        let third = parts.next();
        if parts.next().is_some() {
            return Err(unknown(s));
        }
        let channel = match (kind, third) {
            ("bbo", None) => Channel::Bbo(iid(second, s)?),
            ("book", depth) => {
                let depth = match depth {
                    None | Some("20") => StreamDepth::Twenty,
                    Some("50") => StreamDepth::Fifty,
                    Some(_) => return Err(unknown(s)),
                };
                Channel::Book(iid(second, s)?, depth)
            }
            ("trades", None) => Channel::Trades(iid(second, s)?),
            ("klines", Some(interval)) => {
                Channel::Klines(iid(second, s)?, interval.parse().map_err(|_| unknown(s))?)
            }
            ("tickers", None) => Channel::Tickers(iid_or_all(second, s)?),
            ("statistics", None) => Channel::Statistics(iid_or_all(second, s)?),
            _ => return Err(unknown(s)),
        };
        Ok(channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Interval;

    #[test]
    fn every_channel_renders_the_documented_name() {
        let cases = [
            (Channel::Bbo(InstrumentId(1)), "bbo::1"),
            (
                Channel::Book(InstrumentId(1), StreamDepth::Twenty),
                "book::1::20",
            ),
            (
                Channel::Book(InstrumentId(7), StreamDepth::Fifty),
                "book::7::50",
            ),
            (Channel::Trades(InstrumentId(1)), "trades::1"),
            (
                Channel::Klines(InstrumentId(1), Interval::M1),
                "klines::1::1m",
            ),
            (Channel::Tickers(Some(InstrumentId(3))), "tickers::3"),
            (Channel::Tickers(None), "tickers::all"),
            (Channel::Statistics(Some(InstrumentId(3))), "statistics::3"),
            (Channel::Statistics(None), "statistics::all"),
        ];
        for (channel, name) in cases {
            assert_eq!(channel.to_string(), name);
            assert_eq!(name.parse::<Channel>().unwrap(), channel);
        }
    }

    #[test]
    fn a_bare_book_name_parses_as_the_default_depth() {
        // The server treats `book::1` and `book::1::20` as the same channel
        // and labels frames `book::1`.
        assert_eq!(
            "book::1".parse::<Channel>().unwrap(),
            Channel::Book(InstrumentId(1), StreamDepth::Twenty)
        );
    }

    #[test]
    fn malformed_names_are_rejected() {
        for bad in [
            "",
            "bbo",
            "bbo::",
            "bbo::x",
            "book::1::30",
            "klines::1",
            "klines::1::2m",
            "nonsense::1",
            "tickers::",
        ] {
            assert!(bad.parse::<Channel>().is_err(), "{bad:?} parsed");
        }
    }

    #[test]
    fn a_fan_out_frame_label_belongs_to_the_all_subscription() {
        // `tickers::all` never arrives as a frame; the server sends
        // `tickers::N`. A frame label matches a subscription if the
        // subscription names that instrument or names all of them.
        let all = Channel::Tickers(None);
        assert!(all.covers(&Channel::Tickers(Some(InstrumentId(9)))));
        assert!(!Channel::Tickers(Some(InstrumentId(1)))
            .covers(&Channel::Tickers(Some(InstrumentId(9)))));
        assert!(Channel::Bbo(InstrumentId(1)).covers(&Channel::Bbo(InstrumentId(1))));
        assert!(!Channel::Statistics(None).covers(&Channel::Tickers(Some(InstrumentId(1)))));
    }
}
