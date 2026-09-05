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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TwapWindow {
    /// 30-second lookback.
    Thirty,
    /// 60-second lookback.
    Sixty,
}

impl TwapWindow {
    /// The window length in seconds, as it appears in `payload.window_s`.
    pub fn seconds(self) -> u32 {
        match self {
            Self::Thirty => 30,
            Self::Sixty => 60,
        }
    }

    /// Parse a `payload.window_s` value. Returns `None` for any other length.
    pub fn from_seconds(seconds: u32) -> Option<Self> {
        match seconds {
            30 => Some(Self::Thirty),
            60 => Some(Self::Sixty),
            _ => None,
        }
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
    pub fn from_wire(wire: &str) -> Option<Self> {
        match wire {
            "crypto_prices" => Some(Self::BinanceSpot),
            "crypto_prices_chainlink" => Some(Self::ChainlinkSpot),
            "crypto_prices_twap_thirty" => Some(Self::ChainlinkTwap(TwapWindow::Thirty)),
            "crypto_prices_twap_sixty" => Some(Self::ChainlinkTwap(TwapWindow::Sixty)),
            _ => None,
        }
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
        for topic in [
            Topic::BinanceSpot,
            Topic::ChainlinkSpot,
            Topic::ChainlinkTwap(TwapWindow::Thirty),
            Topic::ChainlinkTwap(TwapWindow::Sixty),
        ] {
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
        assert_eq!(TwapWindow::from_seconds(30), Some(TwapWindow::Thirty));
        assert_eq!(TwapWindow::from_seconds(60), Some(TwapWindow::Sixty));
        assert_eq!(TwapWindow::from_seconds(45), None);
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
}
