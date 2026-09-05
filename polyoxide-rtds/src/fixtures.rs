//! Frames captured verbatim from `wss://ws-live-data.polymarket.com` on
//! 2026-09-05.
//!
//! Do not edit these to make a test pass. They are what the venue actually
//! sent, and several of them contradict Polymarket's published documentation —
//! see `docs/specs/rtds/OBSERVED.md`. Snapshot fixtures have had their `data`
//! arrays truncated to three points; the original lengths are noted on each.

/// Binance spot update. `full_accuracy_value` is a **plain decimal**.
pub const BINANCE_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697.73000000","symbol":"btcusdt","timestamp":1788600389000,"value":79697.73},"timestamp":1788600389154,"topic":"crypto_prices","type":"update"}"#;

/// Chainlink spot update. `full_accuracy_value` is **E18**.
///
/// Captured roughly one second after [`BINANCE_UPDATE`], reporting the same
/// asset at the same price, with byte-identical payload keys. The pair is the
/// evidence that these two topics cannot share a type.
pub const CHAINLINK_SPOT_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79696948174287960000000","symbol":"btc/usd","timestamp":1788600388000,"value":79696.94817428796},"timestamp":1788600389451,"topic":"crypto_prices_chainlink","type":"update"}"#;

/// 30-second TWAP update. E18, plus `window_s`.
pub const TWAP_THIRTY_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697474565615044788224","symbol":"btc/usd","timestamp":1788600388000,"value":79697.47456561505,"window_s":30},"timestamp":1788600389537,"topic":"crypto_prices_twap_thirty","type":"update"}"#;

/// 60-second TWAP update.
pub const TWAP_SIXTY_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697575317474428059648","symbol":"btc/usd","timestamp":1788600388000,"value":79697.57531747443,"window_s":60},"timestamp":1788600389495,"topic":"crypto_prices_twap_sixty","type":"update"}"#;

/// TWAP snapshot, truncated from 55 points.
///
/// Points carry `full_accuracy_value`, and the payload carries `window_s`.
/// Note the envelope has **no** `connection_id` — snapshots never do.
pub const TWAP_THIRTY_SNAPSHOT: &str = r#"{"payload":{"data":[{"full_accuracy_value":"79696840994573453885440","timestamp":1788600329000,"value":79696.84099457346},{"full_accuracy_value":"79696885010084155883520","timestamp":1788600330000,"value":79696.88501008415},{"full_accuracy_value":"79696928978311781023744","timestamp":1788600331000,"value":79696.92897831179}],"symbol":"btc/usd","window_s":30},"timestamp":1788600388753,"topic":"crypto_prices_twap_thirty","type":"subscribe"}"#;

/// Binance snapshot, truncated from 120 points.
///
/// Points carry **no** `full_accuracy_value`, so no exact value exists in this
/// backfill, and the payload carries no `window_s`.
pub const BINANCE_SNAPSHOT: &str = r#"{"payload":{"data":[{"timestamp":1788600269000,"value":79697.73},{"timestamp":1788600270000,"value":79697.73},{"timestamp":1788600271000,"value":79697.73}],"symbol":"btcusdt"},"timestamp":1788600388752,"topic":"crypto_prices","type":"subscribe"}"#;

/// Chainlink **spot** snapshot, truncated from 59 points, captured
/// 2026-09-05 on a connection subscribed to `crypto_prices_chainlink` and
/// nothing else.
///
/// Note the `topic` field: it says `crypto_prices`, the *Binance* topic. That
/// is what the server sends. Both spot topics' backfills come back under the
/// same label, and the only thing distinguishing them is the symbol format —
/// `btc/usd` here versus `btcusdt` on [`BINANCE_SNAPSHOT`]. Update frames on
/// this topic are labelled correctly (see [`CHAINLINK_SPOT_UPDATE`]); only
/// the snapshot is mislabelled, and only for the spot pair. TWAP snapshots
/// carry their own topic correctly.
///
/// Deriving a snapshot's topic from this field alone therefore files every
/// Chainlink-spot backfill under Binance.
pub const CHAINLINK_SPOT_SNAPSHOT: &str = r#"{"payload":{"data":[{"timestamp":1788608387000,"value":79639.20029609217},{"timestamp":1788608388000,"value":79639.18129358691},{"timestamp":1788608389000,"value":79639.19042885714}],"symbol":"btc/usd"},"timestamp":1788608446826,"topic":"crypto_prices","type":"subscribe"}"#;

/// 60-second TWAP snapshot, truncated from 59 points, captured 2026-09-05.
///
/// Unlike the spot pair above, this one carries its own topic correctly.
pub const TWAP_SIXTY_SNAPSHOT: &str = r#"{"payload":{"data":[{"full_accuracy_value":"79639795829763498573824","timestamp":1788608387000,"value":79639.7958297635},{"full_accuracy_value":"79639790921767903559680","timestamp":1788608388000,"value":79639.7909217679},{"full_accuracy_value":"79639787357732704616448","timestamp":1788608389000,"value":79639.78735773271}],"symbol":"btc/usd","window_s":60},"timestamp":1788608446825,"topic":"crypto_prices_twap_sixty","type":"subscribe"}"#;

/// The error frame produced by including one unrecognised topic in an
/// otherwise valid five-topic batch. All five topics returned zero frames.
pub const REJECTED_SUBSCRIPTION: &str = r#"{"body":{"message":"leger GetTopics error: rpc error: code = NotFound desc = topic: definitely_not_a_topic and type: update not found, status: rpc error: code = NotFound desc = topic: definitely_not_a_topic and type: update not found, message: topic: definitely_not_a_topic and type: update not found"},"statusCode":401}"#;

/// The empty text frame RTDS sends immediately after the connection opens.
pub const EMPTY_GREETING: &str = "";

#[cfg(test)]
mod tests {
    use super::*;

    /// Every fixture except the greeting must be valid JSON. A stray character
    /// introduced while copying these would otherwise surface as a confusing
    /// parser failure several tasks later.
    #[test]
    fn every_fixture_is_valid_json() {
        for (name, frame) in [
            ("BINANCE_UPDATE", BINANCE_UPDATE),
            ("CHAINLINK_SPOT_UPDATE", CHAINLINK_SPOT_UPDATE),
            ("TWAP_THIRTY_UPDATE", TWAP_THIRTY_UPDATE),
            ("TWAP_SIXTY_UPDATE", TWAP_SIXTY_UPDATE),
            ("TWAP_THIRTY_SNAPSHOT", TWAP_THIRTY_SNAPSHOT),
            ("BINANCE_SNAPSHOT", BINANCE_SNAPSHOT),
            ("CHAINLINK_SPOT_SNAPSHOT", CHAINLINK_SPOT_SNAPSHOT),
            ("TWAP_SIXTY_SNAPSHOT", TWAP_SIXTY_SNAPSHOT),
            ("REJECTED_SUBSCRIPTION", REJECTED_SUBSCRIPTION),
        ] {
            serde_json::from_str::<serde_json::Value>(frame)
                .unwrap_or_else(|e| panic!("{name} is not valid JSON: {e}"));
        }
    }

    /// The whole design rests on these two frames being field-identical and
    /// differently scaled. If a copy error broke that, every later test would
    /// still pass while testing the wrong thing.
    #[test]
    fn the_two_spot_fixtures_are_field_identical_and_differently_scaled() {
        let binance: serde_json::Value = serde_json::from_str(BINANCE_UPDATE).unwrap();
        let chainlink: serde_json::Value = serde_json::from_str(CHAINLINK_SPOT_UPDATE).unwrap();

        let mut binance_keys: Vec<&str> = binance["payload"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut chainlink_keys: Vec<&str> = chainlink["payload"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        binance_keys.sort_unstable();
        chainlink_keys.sort_unstable();
        assert_eq!(
            binance_keys, chainlink_keys,
            "the two spot topics must have identical payload keys"
        );

        // Both report BTC at about $79,697, but one string is E18 and the
        // other is a plain decimal.
        assert!(binance["payload"]["full_accuracy_value"]
            .as_str()
            .unwrap()
            .contains('.'));
        assert!(!chainlink["payload"]["full_accuracy_value"]
            .as_str()
            .unwrap()
            .contains('.'));
    }

    /// Only TWAP frames carry `window_s`, and only TWAP snapshots carry an
    /// exact value per point.
    #[test]
    fn snapshot_fixtures_differ_in_exactly_the_way_the_design_depends_on() {
        let twap: serde_json::Value = serde_json::from_str(TWAP_THIRTY_SNAPSHOT).unwrap();
        let binance: serde_json::Value = serde_json::from_str(BINANCE_SNAPSHOT).unwrap();

        assert_eq!(twap["payload"]["window_s"], 30);
        assert!(binance["payload"]["window_s"].is_null());

        assert!(twap["payload"]["data"][0]["full_accuracy_value"].is_string());
        assert!(binance["payload"]["data"][0]["full_accuracy_value"].is_null());

        // Snapshots carry no connection_id; updates do.
        assert!(twap["connection_id"].is_null());
        let update: serde_json::Value = serde_json::from_str(TWAP_THIRTY_UPDATE).unwrap();
        assert!(update["connection_id"].is_string());
    }

    /// Pins a server bug: a Chainlink-spot backfill arrives labelled with the
    /// **Binance** topic. Captured on a connection subscribed to
    /// `crypto_prices_chainlink` and nothing else, so the label cannot be
    /// explained by another subscription on the same socket.
    ///
    /// The consequence is that a snapshot's topic cannot be taken from its
    /// `topic` field alone for the spot pair — the symbol format is the only
    /// discriminator. This test exists so that if the venue ever fixes the
    /// label, the workaround built on top of it is revisited rather than
    /// silently left in place.
    #[test]
    fn a_chainlink_spot_snapshot_is_mislabelled_as_the_binance_topic() {
        let chainlink: serde_json::Value = serde_json::from_str(CHAINLINK_SPOT_SNAPSHOT).unwrap();
        let binance: serde_json::Value = serde_json::from_str(BINANCE_SNAPSHOT).unwrap();

        assert_eq!(chainlink["topic"], "crypto_prices");
        assert_eq!(binance["topic"], "crypto_prices");

        // Only the symbol tells them apart: Chainlink uses a slash, Binance
        // does not.
        assert!(chainlink["payload"]["symbol"]
            .as_str()
            .unwrap()
            .contains('/'));
        assert!(!binance["payload"]["symbol"].as_str().unwrap().contains('/'));

        // Update frames on the same topic are labelled correctly, which is
        // what makes this specifically a snapshot bug.
        let update: serde_json::Value = serde_json::from_str(CHAINLINK_SPOT_UPDATE).unwrap();
        assert_eq!(update["topic"], "crypto_prices_chainlink");

        // TWAP snapshots are unaffected.
        let twap: serde_json::Value = serde_json::from_str(TWAP_SIXTY_SNAPSHOT).unwrap();
        assert_eq!(twap["topic"], "crypto_prices_twap_sixty");
    }

    /// Cross-checks each fixture's exact string against the lossy float the
    /// venue sent beside it. The other tests would all still pass with a
    /// mistyped digit inside a 23-digit `full_accuracy_value`; this one would
    /// not, because the two fields would stop agreeing.
    ///
    /// It also fails if someone swaps the two scales, since it decodes Binance
    /// with [`decode_plain`](crate::decode::decode_plain) and the Chainlink
    /// topics with [`decode_e18`](crate::decode::decode_e18).
    ///
    /// **What it cannot catch:** `value` carries about 16 significant digits,
    /// so it says nothing about the last ~7 digits of a 23-digit exact value.
    /// A typo there is invisible here. Those digits are guaranteed instead by
    /// having byte-compared every fixture against the original packet capture
    /// when they were added.
    #[test]
    fn every_fixture_decodes_to_the_float_it_shipped_with() {
        use crate::decode::{decode_e18, decode_plain};
        use crate::topic::{Topic, TwapWindow};
        use rust_decimal::prelude::ToPrimitive;

        let cases: [(&str, &str, Topic); 4] = [
            ("BINANCE_UPDATE", BINANCE_UPDATE, Topic::BinanceSpot),
            (
                "CHAINLINK_SPOT_UPDATE",
                CHAINLINK_SPOT_UPDATE,
                Topic::ChainlinkSpot,
            ),
            (
                "TWAP_THIRTY_UPDATE",
                TWAP_THIRTY_UPDATE,
                Topic::ChainlinkTwap(TwapWindow::Thirty),
            ),
            (
                "TWAP_SIXTY_UPDATE",
                TWAP_SIXTY_UPDATE,
                Topic::ChainlinkTwap(TwapWindow::Sixty),
            ),
        ];

        for (name, frame, topic) in cases {
            let value: serde_json::Value = serde_json::from_str(frame).unwrap();
            let raw = value["payload"]["full_accuracy_value"].as_str().unwrap();
            let shipped = value["payload"]["value"].as_f64().unwrap();

            let decoded = match topic {
                Topic::BinanceSpot => decode_plain(raw, topic),
                _ => decode_e18(raw, topic),
            }
            .unwrap_or_else(|e| panic!("{name}: {raw} did not decode: {e}"));

            let decoded = decoded.to_f64().unwrap();
            // Exact equality, not an epsilon: measured across all four
            // fixtures, rounding the decoded value to f64 reproduces the
            // venue's own float bit for bit. Anything looser would let a
            // wrong digit through — an epsilon of 0.001 accepts a value that
            // is wrong in its ninth significant figure.
            assert_eq!(
                decoded, shipped,
                "{name}: exact value {decoded} disagrees with the float the \
                 venue sent ({shipped}); one of the two was mistyped"
            );
        }
    }
}
