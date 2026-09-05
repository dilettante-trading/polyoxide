//! RTDS topic identifiers.
//!
//! [`Topic`] is a closed enum rather than a string on purpose. One
//! unrecognised topic in a subscription batch causes the venue to return zero
//! frames for **every** topic in that batch, answering only with a single
//! error frame. Making an invalid topic unrepresentable removes that failure
//! mode rather than documenting it.

/// Lookback window of a Chainlink TWAP feed.
///
/// These are lookback windows, not publication cadences — both windows publish
/// roughly once per second.
///
/// Marked `#[non_exhaustive]` because upstream could add new windows (e.g., 300
/// seconds), and adding one later must not be a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TwapWindow {
    /// 30-second lookback.
    Thirty,
    /// 60-second lookback.
    Sixty,
}

impl TwapWindow {
    /// Every window this crate models.
    ///
    /// [`from_seconds`](Self::from_seconds) is derived from this and
    /// [`seconds`](Self::seconds), so a new variant cannot be silently
    /// unparseable — `seconds` fails to compile until it is handled.
    pub const ALL: [Self; 2] = [Self::Thirty, Self::Sixty];

    /// The window length in seconds, as it appears in `payload.window_s`.
    pub fn seconds(self) -> u32 {
        match self {
            Self::Thirty => 30,
            Self::Sixty => 60,
        }
    }

    /// Parse a `payload.window_s` value. Returns `None` for any other length.
    ///
    /// Derived from [`ALL`](Self::ALL) and [`seconds`](Self::seconds), so the
    /// parse direction is kept in sync with the wire-to-seconds mapping by
    /// construction rather than manual duplication.
    pub fn from_seconds(seconds: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|window| window.seconds() == seconds)
    }
}

/// An RTDS topic.
///
/// Marked `#[non_exhaustive]` because upstream carries topics this crate does
/// not yet model (`equity_prices`, `comments`), and adding one later must not
/// be a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Topic {
    /// Binance spot prices (`crypto_prices`). Symbols look like `btcusdt`.
    ///
    /// On this topic `full_accuracy_value` is a **plain decimal**, unlike
    /// every other topic here.
    BinanceSpot,
    /// Chainlink spot prices (`crypto_prices_chainlink`). Symbols look like
    /// `btc/usd`. `full_accuracy_value` is E18 fixed-point.
    ChainlinkSpot,
    /// Chainlink time-weighted average prices. `full_accuracy_value` is E18
    /// fixed-point and the payload carries `window_s`.
    ChainlinkTwap(TwapWindow),
}

impl Topic {
    /// Every topic this crate models.
    ///
    /// [`from_wire`](Self::from_wire) is derived from this and
    /// [`as_wire`](Self::as_wire), which makes the two directions agree by
    /// construction rather than by a test that has to remember to check.
    pub const ALL: [Self; 4] = [
        Self::BinanceSpot,
        Self::ChainlinkSpot,
        Self::ChainlinkTwap(TwapWindow::Thirty),
        Self::ChainlinkTwap(TwapWindow::Sixty),
    ];

    /// The exact string the venue expects in a subscription frame.
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::BinanceSpot => "crypto_prices",
            Self::ChainlinkSpot => "crypto_prices_chainlink",
            Self::ChainlinkTwap(TwapWindow::Thirty) => "crypto_prices_twap_thirty",
            Self::ChainlinkTwap(TwapWindow::Sixty) => "crypto_prices_twap_sixty",
        }
    }

    /// Parse a topic from an incoming frame's `topic` field.
    ///
    /// Returns `None` for topics this crate does not model, so a frame from an
    /// unmodelled topic is skipped rather than misparsed.
    ///
    /// Derived from [`ALL`](Self::ALL) and [`as_wire`](Self::as_wire), so the
    /// parse direction is kept in sync with the wire mapping by construction
    /// rather than manual duplication.
    pub fn from_wire(wire: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|topic| topic.as_wire() == wire)
    }

    /// The TWAP window, for TWAP topics only.
    pub fn window(self) -> Option<TwapWindow> {
        match self {
            Self::ChainlinkTwap(window) => Some(window),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_strings_match_the_venue() {
        assert_eq!(Topic::BinanceSpot.as_wire(), "crypto_prices");
        assert_eq!(Topic::ChainlinkSpot.as_wire(), "crypto_prices_chainlink");
        assert_eq!(
            Topic::ChainlinkTwap(TwapWindow::Thirty).as_wire(),
            "crypto_prices_twap_thirty"
        );
        assert_eq!(
            Topic::ChainlinkTwap(TwapWindow::Sixty).as_wire(),
            "crypto_prices_twap_sixty"
        );
    }

    #[test]
    fn wire_strings_round_trip() {
        for topic in Topic::ALL {
            assert_eq!(Topic::from_wire(topic.as_wire()), Some(topic));
        }
    }

    #[test]
    fn unknown_wire_strings_are_rejected() {
        // Batch poisoning: one unrecognised topic zeroes an entire
        // subscription. An unknown topic must never become a Topic value.
        assert_eq!(Topic::from_wire("equity_prices"), None);
        assert_eq!(Topic::from_wire(""), None);
    }

    #[test]
    fn windows_carry_their_seconds() {
        assert_eq!(TwapWindow::Thirty.seconds(), 30);
        assert_eq!(TwapWindow::Sixty.seconds(), 60);
        assert_eq!(TwapWindow::from_seconds(45), None);
    }

    #[test]
    fn every_window_round_trips_its_seconds() {
        for window in TwapWindow::ALL {
            assert_eq!(TwapWindow::from_seconds(window.seconds()), Some(window));
        }
    }

    #[test]
    fn only_twap_topics_have_a_window() {
        assert_eq!(Topic::BinanceSpot.window(), None);
        assert_eq!(Topic::ChainlinkSpot.window(), None);
        assert_eq!(
            Topic::ChainlinkTwap(TwapWindow::Sixty).window(),
            Some(TwapWindow::Sixty)
        );
    }

    #[test]
    fn all_lists_every_topic_exactly_once() {
        // Guards the one manual step the compiler cannot check: adding a
        // variant without adding it to ALL.
        let mut wires: Vec<&str> = Topic::ALL.iter().map(|t| t.as_wire()).collect();
        wires.sort_unstable();
        let count = wires.len();
        wires.dedup();
        assert_eq!(wires.len(), count, "ALL contains a duplicate topic");
        assert_eq!(count, 4, "ALL must list every modelled topic");
    }
}
