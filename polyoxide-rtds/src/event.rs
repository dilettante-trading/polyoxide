//! Stream events and frame dispatch.

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    decode::{decode_e18, decode_plain},
    error::RtdsError,
    payload::{
        BinanceUpdate, ChainlinkSpotUpdate, DisplayPoint, ExactPoint, Snapshot, SnapshotPoints,
        TwapUpdate,
    },
    topic::{Topic, TwapWindow},
};

/// A data frame's envelope, before topic-specific interpretation.
#[derive(Deserialize)]
struct RawFrame {
    topic: String,
    #[serde(rename = "type")]
    kind: String,
    timestamp: i64,
    #[serde(default)]
    connection_id: Option<String>,
    payload: serde_json::Value,
}

/// An update frame's payload.
#[derive(Deserialize)]
struct RawUpdatePayload {
    symbol: String,
    timestamp: i64,
    value: f64,
    full_accuracy_value: String,
    #[serde(default)]
    window_s: Option<u32>,
}

/// A backfill frame's payload.
#[derive(Deserialize)]
struct RawSnapshotPayload {
    symbol: String,
    data: Vec<RawSnapshotPoint>,
    #[serde(default)]
    window_s: Option<u32>,
}

#[derive(Deserialize)]
struct RawSnapshotPoint {
    timestamp: i64,
    value: f64,
    #[serde(default)]
    full_accuracy_value: Option<String>,
}

/// The venue's error envelope, which shares no fields with a data frame.
///
/// `from_json` tries this shape before `RawFrame`, which is only safe while
/// every field here stays required, so that no data frame can satisfy it.
///
/// Measured, rather than assumed. `body` is the load-bearing field: relaxing
/// `status_code` alone changes nothing, because a data frame still has no
/// `body`. Relaxing `body` alone does not compile, because
/// [`RawServerErrorBody`] derives no `Default`. Only doing both — which means
/// deliberately adding `#[derive(Default)]` — lets a price frame through, and
/// that is exactly where
/// `no_data_frame_can_be_mistaken_for_the_error_envelope` fires.
#[derive(Deserialize)]
struct RawServerError {
    #[serde(rename = "statusCode")]
    status_code: u16,
    body: RawServerErrorBody,
}

#[derive(Deserialize)]
struct RawServerErrorBody {
    message: String,
}

/// A live price update, typed by its topic.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PriceUpdate {
    /// Binance spot. Its exact value came from a plain decimal.
    Binance(BinanceUpdate),
    /// Chainlink spot. Its exact value came from an E18 integer.
    ChainlinkSpot(ChainlinkSpotUpdate),
    /// Chainlink TWAP. Its exact value came from an E18 integer.
    Twap(TwapUpdate),
}

impl PriceUpdate {
    /// The topic this update arrived on.
    pub fn topic(&self) -> Topic {
        match self {
            Self::Binance(_) => Topic::BinanceSpot,
            Self::ChainlinkSpot(_) => Topic::ChainlinkSpot,
            Self::Twap(update) => Topic::ChainlinkTwap(update.window),
        }
    }

    /// The symbol, in whatever format the topic uses.
    pub fn symbol(&self) -> &str {
        match self {
            Self::Binance(u) => &u.symbol,
            Self::ChainlinkSpot(u) => &u.symbol,
            Self::Twap(u) => &u.symbol,
        }
    }

    /// Venue observation time, Unix milliseconds.
    pub fn observed_at(&self) -> i64 {
        match self {
            Self::Binance(u) => u.observed_at,
            Self::ChainlinkSpot(u) => u.observed_at,
            Self::Twap(u) => u.observed_at,
        }
    }

    /// When RTDS received this update, Unix milliseconds.
    pub fn published_at(&self) -> i64 {
        match self {
            Self::Binance(u) => u.published_at,
            Self::ChainlinkSpot(u) => u.published_at,
            Self::Twap(u) => u.published_at,
        }
    }

    /// The exact price. Correctly scaled for the topic it arrived on.
    pub fn value(&self) -> Decimal {
        match self {
            Self::Binance(u) => u.value,
            Self::ChainlinkSpot(u) => u.value,
            Self::Twap(u) => u.value,
        }
    }

    /// The undecoded `full_accuracy_value`, retained for auditing.
    pub fn raw(&self) -> &str {
        match self {
            Self::Binance(u) => &u.raw,
            Self::ChainlinkSpot(u) => &u.raw,
            Self::Twap(u) => &u.raw,
        }
    }

    /// The server-side connection handle, if the frame carried one.
    pub fn connection_id(&self) -> Option<&str> {
        match self {
            Self::Binance(u) => u.connection_id.as_deref(),
            Self::ChainlinkSpot(u) => u.connection_id.as_deref(),
            Self::Twap(u) => u.connection_id.as_deref(),
        }
    }

    /// The TWAP lookback window, for TWAP updates only.
    pub fn window(&self) -> Option<TwapWindow> {
        match self {
            Self::Twap(update) => Some(update.window),
            _ => None,
        }
    }
}

/// An event from the RTDS stream.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PriceEvent {
    /// A live price update.
    Update(PriceUpdate),
    /// The backfill sent immediately after each subscribe. Seen again after
    /// every reconnect, because resubscribing replays it.
    Snapshot(Snapshot),
}

impl PriceEvent {
    /// Parse one text frame.
    ///
    /// Returns `Ok(None)` for frames that carry no event: the empty greeting
    /// sent at connect, keep-alive replies, and frames from topics this crate
    /// does not model. Returns `Err` for a rejected subscription, which must
    /// never be mistaken for an idle feed.
    pub fn from_json(text: &str) -> Result<Option<Self>, RtdsError> {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed == "PONG" || trimmed == "{}" {
            return Ok(None);
        }

        // The error envelope shares no fields with a data frame, so try it
        // first rather than letting it fail as a malformed frame.
        if let Ok(error) = serde_json::from_str::<RawServerError>(trimmed) {
            return Err(RtdsError::Server {
                status_code: error.status_code,
                message: error.body.message,
            });
        }

        let frame: RawFrame =
            serde_json::from_str(trimmed).map_err(|e| RtdsError::json(trimmed, e))?;
        let Some(topic) = Topic::from_wire(&frame.topic) else {
            tracing::debug!(topic = %frame.topic, "skipping unmodelled RTDS topic");
            return Ok(None);
        };

        match frame.kind.as_str() {
            "update" => Ok(Some(Self::Update(parse_update(topic, frame)?))),
            "subscribe" => Ok(Some(Self::Snapshot(parse_snapshot(topic, frame)?))),
            other => {
                tracing::debug!(kind = %other, "skipping unmodelled RTDS frame type");
                Ok(None)
            }
        }
    }
}

fn parse_update(topic: Topic, frame: RawFrame) -> Result<PriceUpdate, RtdsError> {
    let payload = RawUpdatePayload::deserialize(&frame.payload)
        .map_err(|e| RtdsError::json(frame.payload.to_string(), e))?;
    let raw = payload.full_accuracy_value;

    Ok(match topic {
        Topic::BinanceSpot => PriceUpdate::Binance(BinanceUpdate {
            value: decode_plain(&raw, topic)?,
            symbol: payload.symbol,
            observed_at: payload.timestamp,
            published_at: frame.timestamp,
            connection_id: frame.connection_id,
            display_value: payload.value,
            raw,
        }),
        Topic::ChainlinkSpot => PriceUpdate::ChainlinkSpot(ChainlinkSpotUpdate {
            value: decode_e18(&raw, topic)?,
            symbol: payload.symbol,
            observed_at: payload.timestamp,
            published_at: frame.timestamp,
            connection_id: frame.connection_id,
            display_value: payload.value,
            raw,
        }),
        Topic::ChainlinkTwap(window) => {
            warn_on_window_disagreement(topic, payload.window_s, &payload.symbol);
            PriceUpdate::Twap(TwapUpdate {
                value: decode_e18(&raw, topic)?,
                symbol: payload.symbol,
                window,
                observed_at: payload.timestamp,
                published_at: frame.timestamp,
                connection_id: frame.connection_id,
                display_value: payload.value,
                raw,
            })
        }
    })
}

/// Warn when a frame's `window_s` disagrees with the topic it arrived on.
///
/// The topic wins: it is what the client subscribed to, and the venue already
/// has one proven mislabelling bug on exactly this axis (see
/// `correct_mislabelled_spot_snapshot`). The decoded value is unaffected
/// either way, so this is worth recording, not worth dropping a good price
/// over.
fn warn_on_window_disagreement(topic: Topic, window_s: Option<u32>, symbol: &str) {
    let (Topic::ChainlinkTwap(expected), Some(seconds)) = (topic, window_s) else {
        return;
    };
    if TwapWindow::from_seconds(seconds) != Some(expected) {
        tracing::warn!(
            symbol,
            expected = expected.seconds(),
            received = seconds,
            "RTDS window_s disagrees with its topic"
        );
    }
}

/// Correct a server bug: a Chainlink-spot backfill arrives labelled with the
/// **Binance** topic.
///
/// Verified 2026-09-05 on a connection subscribed to
/// `crypto_prices_chainlink` and nothing else — its updates are labelled
/// correctly, its snapshot is not. Both spot topics' backfills come back as
/// `crypto_prices`, and the symbol format is the only discriminator:
/// Chainlink uses `btc/usd`, Binance uses `btcusdt`.
///
/// Deliberately narrow. It only ever reassigns `BinanceSpot`, only for
/// snapshots, and only when the symbol contains a slash — so if the venue
/// fixes the label, this becomes a no-op rather than a new bug. The
/// `a_chainlink_spot_snapshot_is_mislabelled_as_the_binance_topic` fixture
/// test fails if that happens, which is the prompt to delete this.
fn correct_mislabelled_spot_snapshot(topic: Topic, symbol: &str) -> Topic {
    if topic == Topic::BinanceSpot && symbol.contains('/') {
        tracing::debug!(
            symbol,
            "relabelling a snapshot the venue reported as crypto_prices"
        );
        return Topic::ChainlinkSpot;
    }
    topic
}

fn parse_snapshot(topic: Topic, frame: RawFrame) -> Result<Snapshot, RtdsError> {
    let payload = RawSnapshotPayload::deserialize(&frame.payload)
        .map_err(|e| RtdsError::json(frame.payload.to_string(), e))?;

    let topic = correct_mislabelled_spot_snapshot(topic, &payload.symbol);
    warn_on_window_disagreement(topic, payload.window_s, &payload.symbol);

    // Only the TWAP topics backfill exact values. Deciding on the topic rather
    // than on whether the field happens to be present keeps a shape change
    // upstream from silently downgrading TWAP points to display-only.
    let points = match topic {
        Topic::ChainlinkTwap(_) => {
            let mut exact = Vec::with_capacity(payload.data.len());
            for point in payload.data {
                let raw_e18 = point
                    .full_accuracy_value
                    .ok_or_else(|| RtdsError::Precision {
                        raw: format!(
                        "<{} point at timestamp={} has no full_accuracy_value, display_value={}>",
                        payload.symbol, point.timestamp, point.value
                    ),
                        topic,
                    })?;
                exact.push(ExactPoint {
                    value: decode_e18(&raw_e18, topic)?,
                    observed_at: point.timestamp,
                    display_value: point.value,
                    raw_e18,
                });
            }
            SnapshotPoints::Exact(exact)
        }
        Topic::BinanceSpot | Topic::ChainlinkSpot => SnapshotPoints::DisplayOnly(
            payload
                .data
                .into_iter()
                .map(|point| DisplayPoint {
                    observed_at: point.timestamp,
                    display_value: point.value,
                })
                .collect(),
        ),
    };

    Ok(Snapshot {
        topic,
        symbol: payload.symbol,
        published_at: frame.timestamp,
        points,
    })
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    use super::*;
    use crate::{
        decode::decode_e18, error::Recovery, fixtures, payload::SnapshotPoints, topic::TwapWindow,
    };

    fn update(frame: &str) -> PriceUpdate {
        match PriceEvent::from_json(frame) {
            Ok(Some(PriceEvent::Update(update))) => update,
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn the_two_spot_topics_do_not_share_a_scale() {
        // THE test. These frames were captured about one second apart, both
        // reporting BTC at roughly $79,697, with byte-identical payload keys.
        // Only the scale of `full_accuracy_value` differs. A shape-only
        // assertion — "value is a positive Decimal" — passes on both and
        // proves nothing.
        let binance = update(fixtures::BINANCE_UPDATE);
        let chainlink = update(fixtures::CHAINLINK_SPOT_UPDATE);

        assert!(
            (binance.value() - chainlink.value()).abs() < Decimal::from(100u32),
            "same asset, same moment, but got {} vs {}",
            binance.value(),
            chainlink.value()
        );

        // Decoding Chainlink's raw string the way Binance's must be decoded —
        // as a plain decimal — silently yields 7.9e22 instead of 7.9e4. The
        // ratio is exactly 10^18. This is the bug the split types prevent.
        let misdecoded = Decimal::from_str(chainlink.raw()).unwrap();
        assert_eq!(
            (misdecoded / chainlink.value()).normalize(),
            Decimal::from(1_000_000_000_000_000_000u64),
            "misdecoding an E18 value as plain must be off by exactly 10^18"
        );

        // The reverse misdecode fails loudly instead of silently: Binance's
        // raw value is not an integer, so the E18 path cannot parse it at all.
        assert!(decode_e18(binance.raw(), Topic::BinanceSpot).is_err());
    }

    #[test]
    fn binance_decodes_to_its_face_value() {
        let binance = update(fixtures::BINANCE_UPDATE);
        assert_eq!(
            binance.value().normalize().to_string(),
            "79697.73",
            "if this reads 0.00000000000008, the E18 scale leaked onto Binance"
        );
        assert_eq!(binance.symbol(), "btcusdt");
        assert_eq!(binance.observed_at(), 1788600389000);
        assert_eq!(binance.published_at(), 1788600389154);
        assert_eq!(binance.window(), None);
        assert_eq!(binance.topic(), Topic::BinanceSpot);
    }

    #[test]
    fn chainlink_spot_decodes_from_e18() {
        let spot = update(fixtures::CHAINLINK_SPOT_UPDATE);
        assert_eq!(spot.value().to_string(), "79696.948174287960000000");
        assert_eq!(spot.symbol(), "btc/usd");
        assert_eq!(spot.window(), None);
        assert_eq!(spot.topic(), Topic::ChainlinkSpot);
    }

    #[test]
    fn twap_carries_its_window_and_decodes_from_e18() {
        let thirty = update(fixtures::TWAP_THIRTY_UPDATE);
        assert_eq!(thirty.value().to_string(), "79697.474565615044788224");
        assert_eq!(thirty.window(), Some(TwapWindow::Thirty));
        assert_eq!(thirty.topic(), Topic::ChainlinkTwap(TwapWindow::Thirty));

        let sixty = update(fixtures::TWAP_SIXTY_UPDATE);
        assert_eq!(sixty.window(), Some(TwapWindow::Sixty));
        assert_eq!(sixty.topic(), Topic::ChainlinkTwap(TwapWindow::Sixty));
    }

    #[test]
    fn updates_retain_the_undocumented_connection_id() {
        let update = update(fixtures::TWAP_THIRTY_UPDATE);
        assert_eq!(update.connection_id(), Some("gZexFa6cUWeIKEiTDA=="));
    }

    #[test]
    fn twap_snapshots_carry_exact_values() {
        let Ok(Some(PriceEvent::Snapshot(snapshot))) =
            PriceEvent::from_json(fixtures::TWAP_THIRTY_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };

        assert_eq!(snapshot.topic, Topic::ChainlinkTwap(TwapWindow::Thirty));
        assert_eq!(snapshot.symbol, "btc/usd");
        let SnapshotPoints::Exact(points) = &snapshot.points else {
            panic!("TWAP backfills carry full_accuracy_value, so they are Exact");
        };
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value.to_string(), "79696.840994573453885440");
    }

    #[test]
    fn a_mislabelled_chainlink_spot_snapshot_is_attributed_correctly() {
        // The venue labels this backfill `crypto_prices` (Binance) even
        // though it was produced by a `crypto_prices_chainlink` subscription.
        // Taking the label at face value files Chainlink prices under
        // Binance, silently — the points parse either way, because both spot
        // snapshots are display-only.
        let Ok(Some(PriceEvent::Snapshot(snapshot))) =
            PriceEvent::from_json(fixtures::CHAINLINK_SPOT_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };

        assert_eq!(
            snapshot.topic,
            Topic::ChainlinkSpot,
            "a slash in the symbol is the only thing distinguishing this from \
             a Binance backfill"
        );
        assert_eq!(snapshot.symbol, "btc/usd");

        // A genuine Binance backfill must be left alone.
        let Ok(Some(PriceEvent::Snapshot(binance))) =
            PriceEvent::from_json(fixtures::BINANCE_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };
        assert_eq!(binance.topic, Topic::BinanceSpot);
        assert_eq!(binance.symbol, "btcusdt");
    }

    #[test]
    fn spot_snapshots_have_no_exact_values_to_offer() {
        // Binance and Chainlink-spot backfills omit full_accuracy_value
        // entirely. Modelling them as Exact-with-Option would invent a value
        // the venue never sent.
        let Ok(Some(PriceEvent::Snapshot(snapshot))) =
            PriceEvent::from_json(fixtures::BINANCE_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };

        assert_eq!(snapshot.topic, Topic::BinanceSpot);
        let SnapshotPoints::DisplayOnly(points) = &snapshot.points else {
            panic!("Binance backfills carry no exact values");
        };
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].display_value, 79697.73);
    }

    #[test]
    fn a_rejected_subscription_surfaces_as_an_error_not_silence() {
        // If this frame were dropped, a poisoned batch would be
        // indistinguishable from an idle feed.
        let err = PriceEvent::from_json(fixtures::REJECTED_SUBSCRIPTION).unwrap_err();
        match err {
            RtdsError::Server {
                status_code,
                ref message,
            } => {
                assert_eq!(status_code, 401);
                assert!(message.contains("definitely_not_a_topic"), "{message}");
            }
            other => panic!("expected Server, got {other:?}"),
        }
        assert_eq!(err.recovery(), Recovery::Fatal);
    }

    #[test]
    fn keepalive_and_greeting_frames_are_skipped() {
        for frame in [fixtures::EMPTY_GREETING, "PONG", "{}"] {
            assert!(
                matches!(PriceEvent::from_json(frame), Ok(None)),
                "frame {frame:?} should be skipped, not surfaced or errored"
            );
        }
    }

    #[test]
    fn frames_from_unmodelled_topics_are_skipped() {
        // equity_prices and comments ride the same connection. Receiving one
        // must not kill the stream.
        let frame = r#"{"topic":"equity_prices","type":"update","timestamp":1,
            "payload":{"symbol":"aapl","timestamp":1,"value":189.42}}"#;
        assert!(matches!(PriceEvent::from_json(frame), Ok(None)));
    }

    #[test]
    fn a_twap_snapshot_point_missing_its_exact_value_is_reported_usefully() {
        // The venue has never sent this; the fixture is hand-built. It exists
        // because the error it produces is logged and dropped, so a useless
        // message would be the only trace of a real upstream change.
        let frame = r#"{"payload":{"data":[{"timestamp":1788600329000,"value":79696.84}],
            "symbol":"btc/usd","window_s":30},"timestamp":1788600388753,
            "topic":"crypto_prices_twap_thirty","type":"subscribe"}"#;

        let err = PriceEvent::from_json(frame).unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains("btc/usd"), "{rendered}");
        assert!(rendered.contains("1788600329000"), "{rendered}");
        assert_eq!(err.recovery(), Recovery::SkipFrame);
    }

    #[test]
    fn no_data_frame_can_be_mistaken_for_the_error_envelope() {
        // `from_json` tries the error envelope first, which is only safe while
        // `RawServerError`'s fields stay required. Adding `#[serde(default)]`
        // to either would silently route real price frames into
        // `RtdsError::Server`. This test is the trip-wire for that edit.
        for frame in [
            fixtures::BINANCE_UPDATE,
            fixtures::CHAINLINK_SPOT_UPDATE,
            fixtures::TWAP_THIRTY_UPDATE,
            fixtures::TWAP_SIXTY_UPDATE,
            fixtures::TWAP_THIRTY_SNAPSHOT,
            fixtures::TWAP_SIXTY_SNAPSHOT,
            fixtures::BINANCE_SNAPSHOT,
            fixtures::CHAINLINK_SPOT_SNAPSHOT,
        ] {
            assert!(
                serde_json::from_str::<RawServerError>(frame).is_err(),
                "a data frame deserialised as the error envelope: {frame}"
            );
        }
        // And the real thing still does parse.
        assert!(serde_json::from_str::<RawServerError>(fixtures::REJECTED_SUBSCRIPTION).is_ok());
    }

    #[test]
    fn an_unreadable_frame_surfaces_as_a_skippable_json_error() {
        // Nothing else routes malformed text through `from_json`; the error
        // tests build the variant by hand, which cannot catch `from_json`
        // classifying it as something else — a `Server` error, say, which is
        // Fatal and would end the feed.
        let err = PriceEvent::from_json("{not json").unwrap_err();
        assert!(matches!(err, RtdsError::Json { .. }), "{err:?}");
        assert_eq!(err.recovery(), Recovery::SkipFrame);
        assert!(err.to_string().contains("{not json"), "{err}");
    }

    #[test]
    fn a_well_formed_frame_with_an_unreadable_payload_is_also_skippable() {
        // The envelope parses, the payload does not. A different code path
        // from the one above: `parse_update`'s own error, not `from_json`'s.
        let frame = r#"{"topic":"crypto_prices","type":"update","timestamp":1,
            "payload":{"symbol":"btcusdt"}}"#;
        let err = PriceEvent::from_json(frame).unwrap_err();
        assert!(matches!(err, RtdsError::Json { .. }), "{err:?}");
        assert_eq!(err.recovery(), Recovery::SkipFrame);
    }

    #[test]
    fn the_sixty_second_twap_snapshot_parses_as_exact_points() {
        // The thirty-second snapshot is covered above; this fixture was only
        // ever checked as JSON, so nothing proved the second window's backfill
        // takes the same path.
        let Ok(Some(PriceEvent::Snapshot(snapshot))) =
            PriceEvent::from_json(fixtures::TWAP_SIXTY_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };

        assert_eq!(snapshot.topic, Topic::ChainlinkTwap(TwapWindow::Sixty));
        assert_eq!(snapshot.published_at, 1788608446825);
        let SnapshotPoints::Exact(points) = &snapshot.points else {
            panic!("TWAP backfills carry full_accuracy_value, so they are Exact");
        };
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value.to_string(), "79639.795829763498573824");
        assert_eq!(points[0].raw_e18, "79639795829763498573824");
    }

    #[test]
    fn an_empty_backfill_is_reported_as_empty_rather_than_missing() {
        // The venue has never sent a zero-point backfill, but `SnapshotPoints`
        // offers `len`/`is_empty` as public API and nothing exercised either.
        let frame = r#"{"payload":{"data":[],"symbol":"btcusdt"},"timestamp":1,
            "topic":"crypto_prices","type":"subscribe"}"#;
        let Ok(Some(PriceEvent::Snapshot(snapshot))) = PriceEvent::from_json(frame) else {
            panic!("an empty backfill is still a backfill");
        };
        assert_eq!(snapshot.points.len(), 0);
        assert!(snapshot.points.is_empty());

        // And a populated one disagrees, so the two are not both hardcoded.
        let Ok(Some(PriceEvent::Snapshot(populated))) =
            PriceEvent::from_json(fixtures::BINANCE_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };
        assert_eq!(populated.points.len(), 3);
        assert!(!populated.points.is_empty());
    }

    #[test]
    fn an_update_without_a_connection_id_reports_none() {
        // `connection_id` is undocumented, so it may simply stop arriving.
        // Absent must read as absent rather than failing the whole frame.
        let frame = r#"{"payload":{"full_accuracy_value":"79697.73","symbol":"btcusdt",
            "timestamp":1788600389000,"value":79697.73},"timestamp":1788600389154,
            "topic":"crypto_prices","type":"update"}"#;
        let update = update(frame);
        assert_eq!(update.connection_id(), None);
        assert_eq!(update.published_at(), 1788600389154);
    }

    #[test]
    fn every_accessor_reports_the_frame_it_came_from() {
        // The accessors are eight parallel matches over three variants. Most
        // are only ever read for one variant, so a copy-paste slip between two
        // arms would go unnoticed.
        for (frame, topic, symbol, observed, published) in [
            (
                fixtures::CHAINLINK_SPOT_UPDATE,
                Topic::ChainlinkSpot,
                "btc/usd",
                1788600388000i64,
                1788600389451i64,
            ),
            (
                fixtures::TWAP_SIXTY_UPDATE,
                Topic::ChainlinkTwap(TwapWindow::Sixty),
                "btc/usd",
                1788600388000,
                1788600389495,
            ),
            (
                fixtures::BINANCE_UPDATE,
                Topic::BinanceSpot,
                "btcusdt",
                1788600389000,
                1788600389154,
            ),
        ] {
            let update = update(frame);
            assert_eq!(update.topic(), topic, "{frame}");
            assert_eq!(update.symbol(), symbol, "{frame}");
            assert_eq!(update.observed_at(), observed, "{frame}");
            assert_eq!(update.published_at(), published, "{frame}");
            assert_eq!(
                update.connection_id(),
                Some("gZexFa6cUWeIKEiTDA=="),
                "{frame}"
            );
            assert_eq!(update.window(), topic.window(), "{frame}");
        }
    }

    /// `warn_on_window_disagreement` only logs, so the log is the whole
    /// observable behaviour — asserting on the parsed value proves nothing,
    /// because the topic wins either way and the price is identical.
    mod window_disagreement {
        use super::*;

        /// Parse `frame` with a subscriber attached and return the WARN events.
        fn warnings_while_parsing(frame: &str) -> Vec<crate::test_log::CapturedEvent> {
            crate::test_log::capture(|| {
                let _ = PriceEvent::from_json(frame);
            })
            .at(tracing::Level::WARN)
        }

        /// A 30-second TWAP frame carrying `window_s: 60`.
        const MISMATCHED: &str = r#"{"payload":{"full_accuracy_value":"79697474565615044788224",
            "symbol":"btc/usd","timestamp":1788600388000,"value":79697.47456561505,"window_s":60},
            "timestamp":1788600389537,"topic":"crypto_prices_twap_thirty","type":"update"}"#;

        /// The same frame carrying a window this crate does not model at all.
        const UNMODELLED_WINDOW: &str = r#"{"payload":{"full_accuracy_value":"79697474565615044788224",
            "symbol":"btc/usd","timestamp":1788600388000,"value":79697.47456561505,"window_s":45},
            "timestamp":1788600389537,"topic":"crypto_prices_twap_thirty","type":"update"}"#;

        #[test]
        fn a_healthy_frame_warns_about_nothing() {
            // The control. Without it a `warn!` fired unconditionally would
            // pass every assertion below.
            assert!(
                warnings_while_parsing(fixtures::TWAP_THIRTY_UPDATE).is_empty(),
                "a frame whose window_s matches its topic must be silent"
            );
            assert!(warnings_while_parsing(fixtures::TWAP_SIXTY_UPDATE).is_empty());
            assert!(
                warnings_while_parsing(fixtures::BINANCE_UPDATE).is_empty(),
                "a topic with no window cannot disagree about one"
            );
        }

        #[test]
        fn a_window_that_contradicts_its_topic_is_recorded() {
            // The venue already has one proven mislabelling bug on this exact
            // axis, so a second one must leave a trace rather than being
            // silently overridden by the topic.
            let warnings = warnings_while_parsing(MISMATCHED);
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(warnings[0].has_field("expected", "30"), "{warnings:?}");
            assert!(warnings[0].has_field("received", "60"), "{warnings:?}");
            assert!(
                warnings[0].has_field("symbol", "\"btc/usd\""),
                "{warnings:?}"
            );
        }

        #[test]
        fn a_window_this_crate_does_not_model_is_recorded_too() {
            // `TwapWindow::from_seconds` returns None here, a different branch
            // from "parses, but to the wrong window".
            let warnings = warnings_while_parsing(UNMODELLED_WINDOW);
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(warnings[0].has_field("received", "45"), "{warnings:?}");
        }

        #[test]
        fn the_topic_still_wins_and_the_price_is_unaffected() {
            // Warning is the whole remedy: a disagreement must not drop a
            // perfectly good price or relabel it.
            let update = update(MISMATCHED);
            assert_eq!(update.window(), Some(TwapWindow::Thirty));
            assert_eq!(update.value().to_string(), "79697.474565615044788224");
        }
    }
}
