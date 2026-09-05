//! Decoding `full_accuracy_value` into an exact [`Decimal`].
//!
//! The scale of that field **varies by topic**: it is E18 fixed-point on the
//! Chainlink topics and a plain decimal on Binance. Both parse cleanly as
//! decimal strings, so applying the wrong one is silent — a $79,697 Binance
//! price decoded as E18 becomes $0.00000000000008. Every scale decision in
//! this crate happens in this file.

use std::str::FromStr;

use rust_decimal::Decimal;

use crate::{error::RtdsError, topic::Topic};

/// Number of decimal places in Chainlink's fixed-point encoding.
const E18_SCALE: u32 = 18;

/// Decode an E18 fixed-point value, as sent on the Chainlink topics.
///
/// Uses the fallible constructor deliberately: the panicking
/// `from_i128_with_scale` would abort the caller's task on a value out of
/// range, and a price feed must not be able to do that.
pub fn decode_e18(raw: &str, topic: Topic) -> Result<Decimal, RtdsError> {
    let precision_error = || RtdsError::Precision {
        raw: raw.to_string(),
        topic,
    };

    let units: i128 = raw.parse().map_err(|_| precision_error())?;
    Decimal::try_from_i128_with_scale(units, E18_SCALE).map_err(|_| precision_error())
}

/// Decode a plain decimal value, as sent on [`Topic::BinanceSpot`].
///
/// Accepts anything [`Decimal::from_str`] accepts, which is broader than the
/// venue's own format (a leading `+`, scientific notation, and `_`
/// separators all parse) — the permissiveness is deliberate rather than
/// accidental, since none of it can misinterpret a value and none of it
/// appears in observed venue data.
///
/// Note also that `from_str` does not error on a value with more than 28
/// significant fractional digits — it silently rounds and returns `Ok`.
/// Observed Binance values carry 8 decimal places, so this is unreachable
/// today, and [`decode_e18`] cannot hit it at all: it builds a [`Decimal`]
/// from an exact `i128` plus a scale and never parses a string as one.
pub fn decode_plain(raw: &str, topic: Topic) -> Result<Decimal, RtdsError> {
    Decimal::from_str(raw).map_err(|_| RtdsError::Precision {
        raw: raw.to_string(),
        topic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::{Topic, TwapWindow};

    #[test]
    fn e18_decodes_a_captured_twap_value_exactly() {
        // Captured 2026-09-05 from crypto_prices_twap_thirty, btc/usd.
        let value = decode_e18(
            "79697474565615044788224",
            Topic::ChainlinkTwap(TwapWindow::Thirty),
        )
        .unwrap();
        assert_eq!(value.to_string(), "79697.474565615044788224");
    }

    #[test]
    fn plain_decodes_a_captured_binance_value_exactly() {
        // Captured 2026-09-05 from crypto_prices, btcusdt. NOT E18.
        let value = decode_plain("79697.73000000", Topic::BinanceSpot).unwrap();
        assert_eq!(value.normalize().to_string(), "79697.73");
    }

    #[test]
    fn e18_refuses_an_out_of_range_value_instead_of_panicking() {
        // Decimal's 96-bit mantissa caps out around 7.9e28. The panicking
        // constructor would abort a caller's task; this must not.
        let err = decode_e18(&"9".repeat(30), Topic::ChainlinkSpot).unwrap_err();
        assert!(matches!(err, RtdsError::Precision { .. }), "{err:?}");
    }

    #[test]
    fn e18_refuses_a_non_integer_value() {
        // A plain-decimal string on an E18 topic means our topic mapping is
        // wrong. Fail loudly rather than guess.
        let err = decode_e18("79697.73000000", Topic::ChainlinkSpot).unwrap_err();
        assert!(matches!(err, RtdsError::Precision { .. }), "{err:?}");
    }

    #[test]
    fn e18_handles_negative_values() {
        // Upstream describes the field as a signed fixed-point value.
        let value = decode_e18("-1500000000000000000", Topic::ChainlinkSpot).unwrap();
        assert_eq!(value.normalize().to_string(), "-1.5");
    }

    #[test]
    fn the_two_scales_are_not_interchangeable() {
        // The two captured frames below arrived about a second apart and both
        // report BTC at roughly $79,697, with byte-identical payload keys.
        // Only the topic tells them apart.
        const BINANCE_RAW: &str = "79697.73000000";
        const CHAINLINK_RAW: &str = "79696948174287960000000";

        let binance = decode_plain(BINANCE_RAW, Topic::BinanceSpot).unwrap();
        let chainlink = decode_e18(CHAINLINK_RAW, Topic::ChainlinkSpot).unwrap();
        assert!((binance - chainlink).abs() < Decimal::from(100u32));

        // Decoding the Chainlink value the way Binance's must be decoded is
        // the silent failure: it yields 7.9e22 instead of 7.9e4, off by
        // exactly 10^18.
        let misdecoded = decode_plain(CHAINLINK_RAW, Topic::ChainlinkSpot).unwrap();
        assert_eq!(
            (misdecoded / chainlink).normalize(),
            Decimal::from(1_000_000_000_000_000_000u64)
        );

        // The reverse fails loudly rather than silently: Binance's value is
        // not an integer, so the E18 path cannot parse it at all.
        assert!(decode_e18(BINANCE_RAW, Topic::BinanceSpot).is_err());
    }

    #[test]
    fn e18_accepts_the_largest_value_a_decimal_can_hold_and_rejects_the_next() {
        // `try_from_i128_with_scale` is bounded by Decimal's 96-bit mantissa,
        // not by the scale: the ceiling is 2^96 - 1 whatever the scale. At E18
        // that is a price of about $79.2 billion per unit, against an observed
        // BTC raw value of ~7.97e22 — roughly six orders of magnitude of head
        // room. `i128` parses far past this, so 29-to-38 digit inputs parse
        // and then fail, which is why the cutoff is worth pinning.
        const MAX: &str = "79228162514264337593543950335"; // 2^96 - 1
        const OVER: &str = "79228162514264337593543950336"; // 2^96

        assert!(decode_e18(MAX, Topic::ChainlinkSpot).is_ok());
        assert!(matches!(
            decode_e18(OVER, Topic::ChainlinkSpot),
            Err(RtdsError::Precision { .. })
        ));
    }

    #[test]
    fn plain_decoding_rounds_rather_than_failing_past_28_digits() {
        // Documents a real hazard rather than asserting a desirable property.
        // `Decimal::from_str` does not reject a value too precise to
        // represent — it rounds and reports success. Observed Binance values
        // carry 8 decimal places, so this is unreachable today, and
        // `decode_e18` cannot hit it at all because it builds from an exact
        // i128 plus a scale rather than from a string. This test exists so a
        // `rust_decimal` upgrade that changes rounding behaviour, or a venue
        // that starts sending more precision, shows up as a failure here
        // instead of as a quietly wrong price.
        let rounded = decode_plain("1.00000000000000000000000000005", Topic::BinanceSpot).unwrap();
        assert_eq!(rounded.to_string(), "1.0000000000000000000000000001");
    }

    #[test]
    fn cheap_edge_cases() {
        assert_eq!(
            decode_e18("1", Topic::ChainlinkSpot).unwrap().to_string(),
            "0.000000000000000001"
        );
        assert_eq!(
            decode_plain("0", Topic::BinanceSpot).unwrap(),
            Decimal::ZERO
        );
        assert!(decode_plain("", Topic::BinanceSpot).is_err());
    }
}
