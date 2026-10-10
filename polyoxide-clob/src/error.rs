use polyoxide_core::{
    polymarket::{SIGNER_CANCEL, SIGNER_ORDER},
    ApiError, BurstCapacityExceeded, Refused, Tier, TradingBucket,
};
use thiserror::Error;

use crate::types::ParseTickSizeError;

/// Error types for CLOB API operations.
///
/// `#[non_exhaustive]`: downstream matches must carry a wildcard arm. Polymarket
/// keeps introducing outcomes that are only distinguishable by prose (see
/// [`ClobError::FakUnmatched`]), so this enum will keep growing, and each new
/// variant should be a minor bump rather than a breaking one.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ClobError {
    /// Core API error
    #[error(transparent)]
    Api(ApiError),

    /// The Gamma client clob looks profiles up with failed, or could not be
    /// built. Its class is the Gamma error's.
    #[cfg(feature = "gamma")]
    #[error("Gamma error: {0}")]
    Gamma(polyoxide_gamma::GammaError),

    /// Cryptographic operation failed
    #[error("Crypto error: {0}")]
    Crypto(String),

    /// Alloy (Ethereum library) error
    #[error("Alloy error: {0}")]
    Alloy(String),

    /// Invalid tick size
    #[error(transparent)]
    InvalidTickSize(#[from] ParseTickSizeError),

    /// A Fill-And-Kill order was killed because nothing on the book matched it.
    ///
    /// This is FAK's *defined* outcome, not a fault: the order was accepted,
    /// evaluated against the book, found no counterparty, and was killed exactly
    /// as its time-in-force specifies. It is reported as an error only because
    /// Polymarket returns it as HTTP 400, in the same bucket as malformed
    /// payloads and banned addresses.
    ///
    /// It is deterministic — resubmitting the identical order cannot change the
    /// answer — so its class is a [`VenueRefusal`](polyoxide_venue::Class::VenueRefusal),
    /// which is not retriable, and it is not a fault.
    ///
    /// `message` carries the venue's prose verbatim for logging. Match on the
    /// variant, not on the text.
    #[error("FAK order killed unmatched: {message}")]
    FakUnmatched { message: String },

    /// A Fill-Or-Kill order was killed because it could not be filled in full.
    ///
    /// The FOK counterpart of [`ClobError::FakUnmatched`], and equally a defined
    /// outcome rather than a fault. Reached by [`crate::Clob::place_market_order`],
    /// which defaults to [`crate::OrderKind::Fok`].
    #[error("FOK order killed unfilled: {message}")]
    FokUnfilled { message: String },

    /// A batch's token cost exceeds the signer's per-signer burst capacity.
    ///
    /// Rejected client-side, before any request is sent. Polymarket evaluates
    /// order and cancel requests against per-signer token buckets whose cost is
    /// the *number of orders*, not the number of requests — so a batch can cost
    /// more than the bucket can ever hold. Waiting cannot help, and the venue
    /// would answer 429, which the retry loop would misread as transient.
    /// Splitting the batch is the only remedy, so this is
    /// **not** retriable. See `docs/specs/clob/trading-rate-limits.md`.
    #[error(transparent)]
    BurstCapacityExceeded(#[from] BurstCapacityExceeded),
}

/// Recognise the matching-engine kill outcomes Polymarket reports as HTTP 400.
///
/// The venue gives no machine-readable discriminator — no error code, no
/// distinct status — so the message body is the only available signal. See
/// <https://docs.polymarket.com/resources/error-codes>, "Order Processing
/// Errors":
///
/// - `no orders found to match with FAK order. FAK orders are partially filled
///   or killed if no match is found.`
/// - `order couldn't be fully filled. FOK orders are fully filled or killed.`
///
/// Each arm requires both an order-kind token and a kill token, so it stays
/// narrow enough not to capture the neighbouring 400s (tick size, duplicate
/// order, insufficient balance) while surviving light rewording. The apostrophe
/// in "couldn't" is deliberately not part of any key, since straight/curly
/// quoting is exactly the kind of detail that changes silently.
///
/// If Polymarket rewrites these messages, this returns `None` and the caller
/// sees the generic 400, an [`ApiError::Response`] — a visible regression to
/// the old symptom, not a silent misclassification.
fn classify_order_kill(message: &str) -> Option<ClobError> {
    let m = message.to_ascii_lowercase();
    let owned = || message.to_string();

    if m.contains("fak order") && (m.contains("no match") || m.contains("no orders found")) {
        return Some(ClobError::FakUnmatched { message: owned() });
    }
    if m.contains("fok order") && (m.contains("fully filled") || m.contains("killed")) {
        return Some(ClobError::FokUnfilled { message: owned() });
    }
    None
}

impl ClobError {
    /// A request refused before anything was sent.
    pub(crate) fn validation(msg: impl Into<String>) -> Self {
        Self::Api(ApiError::Validation(msg.into()))
    }
}

/// Clob's one decode: what core's loop carries for clob comes back as clob's
/// own variant.
///
/// A 400 whose message is a FAK or FOK kill is [`ClobError::FakUnmatched`] or
/// [`ClobError::FokUnfilled`]. Only a 400 is read: a 5xx carrying similar
/// prose is an engine fault and stays retriable. A refused batch is
/// [`ClobError::BurstCapacityExceeded`], and a signing failure is the clob
/// error the signer raised.
impl From<ApiError> for ClobError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Response(response) if response.status.as_u16() == 400 => {
                classify_order_kill(&response.message)
                    .unwrap_or_else(|| Self::Api(ApiError::Response(response)))
            }
            ApiError::Refused(refused) => burst_from_refused(&refused).map_or(
                Self::Api(ApiError::Refused(refused)),
                Self::BurstCapacityExceeded,
            ),
            ApiError::Sign(err) => err
                .downcast::<ClobError>()
                .map_or_else(|err| Self::Api(ApiError::Sign(err)), |err| *err),
            other => Self::Api(other),
        }
    }
}

/// The per-signer refusal core's throttle reports, as the burst-capacity error
/// clob has always returned.
///
/// The bucket is the layer's, and the tier is the one whose published burst
/// for that bucket is the refused capacity: each bucket's eight bursts are
/// distinct, so the match is exact. `None` for any other layer.
pub(crate) fn burst_from_refused(refused: &Refused) -> Option<BurstCapacityExceeded> {
    let bucket = match refused.layer {
        SIGNER_ORDER => TradingBucket::Order,
        SIGNER_CANCEL => TradingBucket::Cancel,
        _ => return None,
    };
    let tier = [
        Tier::Standard,
        Tier::Copper,
        Tier::Bronze,
        Tier::Silver,
        Tier::Gold,
        Tier::Platinum,
        Tier::Diamond,
        Tier::Elite,
    ]
    .into_iter()
    .find(|tier| tier.burst(bucket) == refused.capacity)?;
    Some(BurstCapacityExceeded {
        cost: refused.units,
        capacity: refused.capacity,
        tier,
        bucket,
    })
}

impl From<alloy::signers::Error> for ClobError {
    fn from(err: alloy::signers::Error) -> Self {
        Self::Alloy(err.to_string())
    }
}

impl From<alloy::hex::FromHexError> for ClobError {
    fn from(err: alloy::hex::FromHexError) -> Self {
        Self::Alloy(err.to_string())
    }
}

/// The FAK and FOK kills are a [`VenueRefusal`](polyoxide_venue::Class::VenueRefusal)
/// that is not a fault: the venue killed the order as its time-in-force
/// says. `Api`, `Gamma` and `BurstCapacityExceeded` delegate, local signing
/// failures are an `InvalidRequest`, and a tick size that did not parse is a
/// `Decode`.
impl polyoxide_venue::Classify for ClobError {
    fn class(&self) -> polyoxide_venue::Class {
        use polyoxide_venue::Class;
        match self {
            Self::Api(err) => err.class(),
            #[cfg(feature = "gamma")]
            Self::Gamma(err) => err.class(),
            Self::Crypto(_) | Self::Alloy(_) => Class::InvalidRequest,
            Self::InvalidTickSize(_) => Class::Decode,
            Self::FakUnmatched { .. } | Self::FokUnfilled { .. } => {
                Class::VenueRefusal { code: None }
            }
            Self::BurstCapacityExceeded(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            #[cfg(feature = "gamma")]
            Self::Gamma(err) => err.is_fault(),
            Self::FakUnmatched { .. } | Self::FokUnfilled { .. } => false,
            Self::BurstCapacityExceeded(err) => err.is_fault(),
            Self::Crypto(_) | Self::Alloy(_) | Self::InvalidTickSize(_) => true,
        }
    }

    fn retry_after(&self) -> Option<std::time::Duration> {
        use polyoxide_venue::Classify;
        match self {
            Self::Api(err) => Classify::retry_after(err),
            #[cfg(feature = "gamma")]
            Self::Gamma(err) => Classify::retry_after(err),
            Self::BurstCapacityExceeded(err) => Classify::retry_after(err),
            Self::Crypto(_)
            | Self::Alloy(_)
            | Self::InvalidTickSize(_)
            | Self::FakUnmatched { .. }
            | Self::FokUnfilled { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_core::ErrorResponse;
    use polyoxide_venue::Classify;

    /// A response with `status` and the body `{"error": message}`.
    fn response(status: u16, message: &str) -> ApiError {
        ErrorResponse::new(
            polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),
            Default::default(),
            serde_json::json!({ "error": message }).to_string(),
        )
        .into()
    }

    /// A Gamma failure, as clob's profile lookup sees one.
    #[cfg(feature = "gamma")]
    fn gamma(status: u16, message: &str) -> ClobError {
        ClobError::Gamma(response(status, message).into())
    }

    #[cfg(feature = "gamma")]
    #[test]
    fn test_gamma_error_is_gamma_not_validation() {
        let err = gamma(404, "Gamma client failed");
        match &err {
            ClobError::Gamma(polyoxide_gamma::GammaError::Api(ApiError::Response(r))) => {
                assert_eq!(r.status.as_u16(), 404);
                assert_eq!(r.message, "Gamma client failed");
            }
            other => panic!("Expected ClobError::Gamma, got {:?}", other),
        }
    }

    #[test]
    fn test_validation_error() {
        let err = ClobError::validation("bad input");
        match &err {
            ClobError::Api(ApiError::Validation(msg)) => {
                assert_eq!(msg, "bad input");
            }
            other => panic!("Expected ApiError::Validation, got {:?}", other),
        }
    }

    #[cfg(feature = "gamma")]
    #[test]
    fn test_service_and_validation_are_distinct() {
        let service = gamma(503, "service failure");
        let validation = ClobError::validation("validation failure");

        let service_msg = format!("{}", service);
        let validation_msg = format!("{}", validation);

        // They should produce different Display output
        assert_ne!(service_msg, validation_msg);
        assert!(service_msg.contains("service failure"));
        assert!(validation_msg.contains("validation failure"));
    }

    #[test]
    fn test_crypto_error() {
        let err = ClobError::Crypto("signing failed".into());
        assert!(err.to_string().contains("signing failed"));
        assert!(matches!(err, ClobError::Crypto(_)));
    }

    #[test]
    fn test_alloy_error() {
        let err = ClobError::Alloy("hex decode failed".into());
        assert!(err.to_string().contains("hex decode failed"));
        assert!(matches!(err, ClobError::Alloy(_)));
    }

    #[test]
    fn test_invalid_tick_size_from_str() {
        let err: Result<crate::types::TickSize, _> = "0.5".try_into();
        let clob_err = ClobError::from(err.unwrap_err());
        assert!(matches!(clob_err, ClobError::InvalidTickSize(_)));
        assert!(clob_err.to_string().contains("0.5"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("not valid json").unwrap_err();
        let clob_err = ClobError::from(ApiError::from(json_err));
        assert!(matches!(
            clob_err,
            ClobError::Api(ApiError::Serialization(_))
        ));
    }

    #[test]
    fn test_from_url_parse_error() {
        let url_err = url::Url::parse("://bad").unwrap_err();
        let clob_err = ClobError::from(ApiError::from(url_err));
        assert!(matches!(clob_err, ClobError::Api(ApiError::Url(_))));
    }

    // ── FAK/FOK kill classification ─────────────────────────────

    /// Verbatim venue prose, from docs.polymarket.com/resources/error-codes.
    const FAK_UNMATCHED: &str = "no orders found to match with FAK order. \
FAK orders are partially filled or killed if no match is found.";
    const FOK_UNFILLED: &str =
        "order couldn't be fully filled. FOK orders are fully filled or killed.";

    #[test]
    fn test_classify_recognizes_verbatim_venue_messages() {
        assert!(matches!(
            classify_order_kill(FAK_UNMATCHED),
            Some(ClobError::FakUnmatched { .. })
        ));
        assert!(matches!(
            classify_order_kill(FOK_UNFILLED),
            Some(ClobError::FokUnfilled { .. })
        ));
    }

    #[test]
    fn test_classify_preserves_message_verbatim() {
        match classify_order_kill(FAK_UNMATCHED) {
            Some(ClobError::FakUnmatched { message }) => assert_eq!(message, FAK_UNMATCHED),
            other => panic!("expected FakUnmatched, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_is_case_insensitive() {
        // The venue capitalizes "FAK"/"FOK"; casing must not be load-bearing.
        assert!(matches!(
            classify_order_kill(&FAK_UNMATCHED.to_uppercase()),
            Some(ClobError::FakUnmatched { .. })
        ));
        assert!(matches!(
            classify_order_kill(&FOK_UNFILLED.to_lowercase()),
            Some(ClobError::FokUnfilled { .. })
        ));
    }

    #[test]
    fn test_classify_tolerates_curly_apostrophe_in_fok_message() {
        // "couldn't" is not part of any match key precisely so that straight vs
        // curly quoting cannot break classification.
        let curly = "order couldn\u{2019}t be fully filled. FOK orders are fully filled or killed.";
        assert!(matches!(
            classify_order_kill(curly),
            Some(ClobError::FokUnfilled { .. })
        ));
    }

    #[test]
    fn test_classify_does_not_capture_neighbouring_400s() {
        // Every other documented 400 from the "Place Orders" and "Order Processing
        // Errors" tables. A kill classifier that swallowed any of these would turn a
        // real fault into a normal-outcome signal — strictly worse than the bug.
        for msg in [
            "Invalid order payload",
            "the order owner has to be the owner of the API KEY",
            "the order signer address has to be the address of the API KEY",
            "'0x1234' address banned",
            "'0x1234' address in closed only mode",
            "Too many orders in payload: 20, max allowed: 15",
            "invalid post-only order: order crosses book",
            "order 0xabc is invalid. Price (100) breaks minimum tick size rule: 0.1",
            "order 0xabc is invalid. Size (1) lower than the minimum: 5",
            "order 0xabc is invalid. Duplicated.",
            "order 0xabc crosses the book",
            "not enough balance / allowance",
            "invalid expiration",
            "order canceled in the CTF exchange contract",
            "order match delayed due to market conditions",
            "the market is not yet ready to process new orders",
            "invalid amount for a marketable BUY order ($0.50), min size: 1",
        ] {
            assert!(
                classify_order_kill(msg).is_none(),
                "must not classify as a kill outcome: {msg}"
            );
        }
    }

    #[test]
    fn test_classify_requires_both_tokens() {
        // An order-kind token alone is not enough, nor a kill token alone.
        assert!(classify_order_kill("FAK order rejected: bad payload").is_none());
        assert!(classify_order_kill("no orders found for this market").is_none());
        assert!(classify_order_kill("FOK order rejected: bad payload").is_none());
    }

    // ── retriability, through the class ─────────────────────────

    #[test]
    fn test_kill_outcomes_are_not_retriable() {
        // The point of the whole change: an unmatched FAK is deterministic.
        assert!(!ClobError::FakUnmatched {
            message: FAK_UNMATCHED.into()
        }
        .is_retriable());
        assert!(!ClobError::FokUnfilled {
            message: FOK_UNFILLED.into()
        }
        .is_retriable());
    }

    #[test]
    fn test_local_failures_are_not_retriable() {
        assert!(!ClobError::Crypto("signing failed".into()).is_retriable());
        assert!(!ClobError::Alloy("hex decode failed".into()).is_retriable());
        assert!(!ClobError::validation("bad input").is_retriable());
    }

    #[test]
    fn test_transient_failures_are_retriable() {
        assert!(ClobError::from(response(429, "slow down")).is_retriable());
        assert!(ClobError::from(response(408, "timeout")).is_retriable());
        assert!(ClobError::from(response(500, "order timed out")).is_retriable());
        // 425 Too Early — matching engine restarting.
        assert!(ClobError::from(response(425, "")).is_retriable());
    }

    #[test]
    fn the_decode_splits_out_a_kill_only_on_a_400() {
        assert!(matches!(
            ClobError::from(response(400, FAK_UNMATCHED)),
            ClobError::FakUnmatched { message } if message == FAK_UNMATCHED
        ));
        assert!(matches!(
            ClobError::from(response(400, FOK_UNFILLED)),
            ClobError::FokUnfilled { .. }
        ));
        assert!(matches!(
            ClobError::from(response(400, "Invalid order payload")),
            ClobError::Api(ApiError::Response(r)) if r.status.as_u16() == 400
        ));
        // A 5xx with the same prose is an engine fault.
        assert!(matches!(
            ClobError::from(response(500, FAK_UNMATCHED)),
            ClobError::Api(ApiError::Response(r)) if r.status.as_u16() == 500
        ));
    }

    #[test]
    fn test_kill_outcome_display_names_the_order_type() {
        // Logs should say what happened without the reader parsing venue prose.
        let fak = ClobError::FakUnmatched {
            message: FAK_UNMATCHED.into(),
        };
        assert!(fak.to_string().starts_with("FAK order killed unmatched:"));
        let fok = ClobError::FokUnfilled {
            message: FOK_UNFILLED.into(),
        };
        assert!(fok.to_string().starts_with("FOK order killed unfilled:"));
    }

    // ── Classify ────────────────────────────────────────────────

    #[test]
    fn every_variant_classifies() {
        use polyoxide_venue::Class;

        let tick = crate::types::TickSize::try_from("0.5").unwrap_err();
        let burst = BurstCapacityExceeded {
            cost: 2_000,
            capacity: 120,
            tier: Tier::Standard,
            bucket: TradingBucket::Cancel,
        };
        let refusal = Class::VenueRefusal { code: None };
        // (error, class, is_fault). No ClobError here carries a wait.
        #[allow(unused_mut)]
        let mut rows = vec![
            (
                ClobError::from(response(408, "timeout")),
                Class::Unavailable { code: None },
                true,
            ),
            // A refusal made before sending.
            (
                ClobError::validation("bad input"),
                Class::InvalidRequest,
                true,
            ),
            // The venue's own 400 that is not a kill.
            (
                ClobError::from(response(400, "Invalid order payload")),
                refusal.clone(),
                true,
            ),
            (
                ClobError::Crypto("signing failed".into()),
                Class::InvalidRequest,
                true,
            ),
            (
                ClobError::Alloy("hex decode failed".into()),
                Class::InvalidRequest,
                true,
            ),
            (ClobError::InvalidTickSize(tick), Class::Decode, true),
            (
                ClobError::FakUnmatched {
                    message: FAK_UNMATCHED.into(),
                },
                refusal.clone(),
                false,
            ),
            (
                ClobError::FokUnfilled {
                    message: FOK_UNFILLED.into(),
                },
                refusal.clone(),
                false,
            ),
            (
                ClobError::BurstCapacityExceeded(burst),
                Class::InvalidRequest,
                true,
            ),
        ];
        // A failure of the Gamma dependency is classed as Gamma's error.
        #[cfg(feature = "gamma")]
        rows.extend([
            (gamma(404, "no profile"), refusal, true),
            (gamma(503, "down"), Class::Unavailable { code: None }, true),
        ]);
        for (err, class, fault) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }

    #[test]
    fn burst_from_refused_recovers_the_tier_and_bucket_of_every_tier() {
        use polyoxide_core::LayerId;

        for tier in [
            Tier::Standard,
            Tier::Copper,
            Tier::Bronze,
            Tier::Silver,
            Tier::Gold,
            Tier::Platinum,
            Tier::Diamond,
            Tier::Elite,
        ] {
            for (layer, bucket) in [
                (SIGNER_ORDER, TradingBucket::Order),
                (SIGNER_CANCEL, TradingBucket::Cancel),
            ] {
                let capacity = tier.burst(bucket);
                let refused = Refused {
                    layer,
                    units: capacity + 1,
                    capacity,
                };
                assert_eq!(
                    burst_from_refused(&refused),
                    Some(BurstCapacityExceeded {
                        cost: capacity + 1,
                        capacity,
                        tier,
                        bucket,
                    }),
                    "{tier:?} {bucket:?}"
                );
                // Through `From`, as the request path maps it.
                assert!(matches!(
                    ClobError::from(ApiError::Refused(refused)),
                    ClobError::BurstCapacityExceeded(e) if e.tier == tier && e.bucket == bucket
                ));
            }
        }

        // Another layer's refusal is not a signer burst, and stays core's.
        let other = Refused {
            layer: LayerId("cloudflare"),
            units: 2,
            capacity: 1,
        };
        assert_eq!(burst_from_refused(&other), None);
        assert!(matches!(
            ClobError::from(ApiError::Refused(other)),
            ClobError::Api(ApiError::Refused(_))
        ));
    }
}
