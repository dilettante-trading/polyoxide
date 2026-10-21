//! Predictions' market keys: `kalshi.predictions:<TICKER>:<outcome>`, one per outcome of a market.
//!
//! A Kalshi market is binary, and its two outcomes trade as two books, so a key names one outcome
//! of one market: the market's [`Ticker`] and `yes` or `no`. A ticker's ASCII letters are
//! uppercased and an outcome's lowercased, so
//! `kalshi.predictions:kxbtcd-26oct1806-t82099.99:YES` and
//! `kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes` are one key.
//!
//! ```
//! use polyoxide_kalshi::predictions::key::{self, Outcome, Ticker};
//! use polyoxide_venue::RawKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let raw = RawKey::parse("kalshi.predictions:kxbtcd-26oct1806-t82099.99:YES")?;
//! let parsed = key::parse(&raw)?;
//! let ticker = Ticker::new("KXBTCD-26OCT1806-T82099.99")?;
//! assert_eq!(parsed, key::key(&ticker, Outcome::Yes));
//! assert_eq!(
//!     parsed.to_string(),
//!     "kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes"
//! );
//! assert_eq!(key::outcome(&parsed)?.outcome, Outcome::Yes);
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;
use std::fmt;

use polyoxide_venue::{
    Class, Classify, KeyError, KeySpace, MarketKey, ProductClass, ProductId, RawKey, VenueId,
};
use thiserror::Error;

/// Kalshi's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("kalshi");

/// Predictions' product id, as the crate registers it.
pub const PRODUCT: ProductId = ProductId::from_static("predictions");

/// The most characters a ticker has: the length the OpenAPI gives the tickers of
/// `GET /markets/orderbooks`.
const MAX_TICKER_LEN: usize = 200;

/// A market's ticker, such as `KXBTCD-26OCT1806-T82099.99`: 1 to 200 characters of `A-Z`, `0-9`,
/// `.` and `-`, at least one of them a letter or digit, with ASCII letters uppercased.
///
/// A ticker of dots and hyphens alone is refused, since `.` and `..` are dot segments, which a URL
/// resolves before the request is sent.
///
/// Kalshi's tickers are case-sensitive and every one it serves is uppercase, so a lowercase ticker
/// would name nothing at the venue; uppercasing it names the market the caller meant. A response
/// row keeps its ticker as a `String`: this type is what a caller sends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ticker(String);

impl Ticker {
    /// Checks and canonicalizes `text`.
    ///
    /// # Errors
    ///
    /// [`InvalidTicker`] when `text` is empty, holds a character outside `A-Z`, `0-9`, `.` and `-`
    /// once its letters are uppercased, is longer than 200 characters, or holds no letter or digit.
    pub fn new(text: &str) -> Result<Self, InvalidTicker> {
        let refuse = |reason: &'static str| InvalidTicker {
            text: text.to_owned(),
            reason,
        };
        let ticker = text.to_ascii_uppercase();
        if ticker.is_empty() {
            return Err(refuse("a ticker has at least one character"));
        }
        if !ticker.bytes().all(is_ticker_byte) {
            return Err(refuse("a ticker holds only A-Z, 0-9, '.' and '-'"));
        }
        if ticker.len() > MAX_TICKER_LEN {
            return Err(refuse("a ticker has at most 200 characters"));
        }
        if !ticker.bytes().any(|byte| byte.is_ascii_alphanumeric()) {
            return Err(refuse(
                "a ticker has at least one letter or digit ('.' and '..' are dot segments)",
            ));
        }
        Ok(Self(ticker))
    }

    /// The ticker's text, in its canonical form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether `byte` may appear in an uppercased ticker.
fn is_ticker_byte(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
}

impl fmt::Display for Ticker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Text that is not a ticker.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
#[error("{text:?} is not a Kalshi ticker: {reason}")]
pub struct InvalidTicker {
    /// The text, as given.
    pub text: String,
    /// What is wrong with it.
    pub reason: &'static str,
}

/// [`Class::InvalidRequest`]: the caller named something no request can carry.
impl Classify for InvalidTicker {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

/// One of a binary market's two outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// The outcome the market's question names.
    Yes,
    /// The other one.
    No,
}

impl Outcome {
    /// The outcome's spelling in a key: `yes` or `no`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
        }
    }

    /// The market's other outcome.
    pub fn other(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
        }
    }
}

/// The id of a Predictions key: one outcome of one market.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutcomeId {
    /// The market.
    pub ticker: Ticker,
    /// Which of its two outcomes.
    pub outcome: Outcome,
}

/// Predictions' keys. Each names one outcome of one market, as an [`OutcomeId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PredictionsKeys;

impl KeySpace for PredictionsKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PRODUCT;
    const CLASS: ProductClass = ProductClass::EventContract;
    type Id = OutcomeId;

    /// Splits the id at its last `:`: the ticker before it, uppercased and checked, and the
    /// outcome after it, lowercased. A ticker has no `:`, so the last one is the outcome's.
    fn parse_id(text: &str) -> Result<OutcomeId, String> {
        let (ticker, outcome) = text
            .rsplit_once(':')
            .ok_or_else(|| "expected <TICKER>:yes or <TICKER>:no".to_owned())?;
        let ticker = Ticker::new(ticker).map_err(|err| err.to_string())?;
        let outcome = match outcome.to_ascii_lowercase().as_str() {
            "yes" => Outcome::Yes,
            "no" => Outcome::No,
            _ => return Err(format!("{outcome:?} is not an outcome: expected yes or no")),
        };
        Ok(OutcomeId { ticker, outcome })
    }

    fn id_text(id: &OutcomeId) -> Cow<'_, str> {
        Cow::Owned(format!("{}:{}", id.ticker, id.outcome.as_str()))
    }
}

/// The key of `outcome` of the market `ticker`.
pub fn key(ticker: &Ticker, outcome: Outcome) -> MarketKey {
    PredictionsKeys::key(&OutcomeId {
        ticker: ticker.clone(),
        outcome,
    })
}

/// The key `raw` names, with its ticker uppercased and its outcome lowercased.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `raw` is not `kalshi.predictions`, and
/// [`KeyError::InvalidId`] when its id is not `<TICKER>:yes` or `<TICKER>:no`.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    PredictionsKeys::parse(raw)
}

/// The market and outcome `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `kalshi.predictions` key.
pub fn outcome(key: &MarketKey) -> Result<OutcomeId, KeyError> {
    PredictionsKeys::id(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    fn ticker(text: &str) -> Ticker {
        Ticker::new(text).unwrap()
    }

    #[test]
    fn the_ids_are_the_ones_the_crate_registers() {
        let declared = polyoxide_test_support::registration!();
        assert_eq!(declared.venue.as_deref(), Some(VENUE.as_str()));
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == PRODUCT.as_str()),
            "{declared:?}"
        );
    }

    #[test]
    fn a_lowercase_ticker_and_outcome_name_the_same_key() {
        let lower = parse(&raw("kalshi.predictions:kxbtcd-26oct1806-t82099.99:YES")).unwrap();
        let upper = parse(&raw("kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.id(), "KXBTCD-26OCT1806-T82099.99:yes");
        assert_eq!(
            lower,
            key(&ticker("KXBTCD-26OCT1806-T82099.99"), Outcome::Yes)
        );
    }

    #[test]
    fn a_dotted_ticker_parses_and_keeps_its_dot() {
        // Production serves tickers such as this one, though the AsyncAPI's
        // pattern is `^[A-Z0-9-]+$`.
        let parsed = parse(&raw("kalshi.predictions:FED-23DEC-T3.00:no")).unwrap();
        let id = outcome(&parsed).unwrap();
        assert_eq!(id.ticker.as_str(), "FED-23DEC-T3.00");
        assert_eq!(id.outcome, Outcome::No);
    }

    #[test]
    fn a_key_round_trips_through_its_text() {
        let text = "kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes";
        let parsed = parse(&RawKey::parse(text).unwrap()).unwrap();
        assert_eq!(parsed.to_string(), text);
        let id = outcome(&parsed).unwrap();
        assert_eq!(id.ticker.as_str(), "KXBTCD-26OCT1806-T82099.99");
        assert_eq!(id.outcome, Outcome::Yes);
        assert_eq!(key(&id.ticker, id.outcome), parsed);
    }

    #[test]
    fn the_two_outcomes_of_a_market_are_two_keys() {
        let market = ticker("KXBTCD-26OCT1806-T82099.99");
        let yes = key(&market, Outcome::Yes);
        let no = key(&market, Outcome::No);
        assert_ne!(yes, no);
        assert_eq!(no, key(&market, Outcome::Yes.other()));
        assert_eq!(Outcome::No.other(), Outcome::Yes);
    }

    #[test]
    fn a_ticker_has_one_to_200_characters_of_the_alphabet() {
        assert_eq!(ticker(&"A".repeat(200)).as_str().len(), 200);

        let empty = Ticker::new("").unwrap_err();
        assert_eq!(empty.text, "");
        assert!(empty.reason.contains("at least one"), "{empty:?}");

        let long = Ticker::new(&"A".repeat(201)).unwrap_err();
        assert!(long.reason.contains("at most 200"), "{long:?}");

        for text in [
            "BTC/USD", "BTC USD", "BTC_USD", "BTC:YES", "BTC*", "BTC%41", "BTCé",
        ] {
            let refused = Ticker::new(text).unwrap_err();
            assert_eq!(refused.text, text);
            assert!(refused.reason.contains("holds only"), "{text}: {refused:?}");
        }
    }

    #[test]
    fn a_refused_ticker_names_the_text_and_is_an_invalid_request() {
        let err = Ticker::new("BTC/USD").unwrap_err();
        assert_eq!(
            err.to_string(),
            "\"BTC/USD\" is not a Kalshi ticker: a ticker holds only A-Z, 0-9, '.' and '-'"
        );
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[test]
    fn a_ticker_needs_a_letter_or_a_digit_so_dot_segments_are_refused() {
        // `.` and `..` are dot segments, which a URL resolves before it is sent.
        for text in [".", "..", "-", "...", ".-."] {
            let refused = Ticker::new(text).unwrap_err();
            assert_eq!(refused.text, text);
            assert!(
                refused.reason.contains("letter or digit"),
                "{text}: {refused:?}"
            );
            assert!(
                refused.reason.contains("dot segments"),
                "{text}: {refused:?}"
            );
            assert_eq!(refused.class(), Class::InvalidRequest);
        }

        // The same text as the ticker of a key is a key error, not a dot segment in a path.
        for text in ["kalshi.predictions:.:yes", "kalshi.predictions:..:no"] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::InvalidId { .. })),
                "{text}"
            );
        }

        // One letter or digit is enough, wherever it sits.
        for text in ["A", "0", ".A", "A..", "-9", "a.b"] {
            assert!(Ticker::new(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn an_id_without_a_ticker_or_an_outcome_is_refused() {
        for text in [
            "kalshi.predictions:KXBTCD",
            "kalshi.predictions:KXBTCD:maybe",
            "kalshi.predictions::yes",
            "kalshi.predictions:KX BTC:yes",
        ] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::InvalidId { .. })),
                "{text}"
            );
        }
    }

    #[test]
    fn a_key_of_another_venue_or_product_is_refused() {
        for text in ["binance.usdm:BTCUSDT:yes", "kalshi.margin:KXBTCD:yes"] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::WrongProduct { .. })),
                "{text}"
            );
        }
    }
}
