use polyoxide_core::ApiError;
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
    Api(#[from] ApiError),

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
    /// answer — so [`ClobError::is_retriable`] returns `false` for it.
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
/// sees the previous generic [`ApiError::Validation`] behaviour — a visible
/// regression to the old symptom, not a silent misclassification.
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
    /// Create error from HTTP response
    pub(crate) async fn from_response(response: reqwest::Response) -> Self {
        // Classify only bodies authored by the venue. Matching on `Validation`
        // rather than re-reading the status is what gates this to HTTP 400:
        // `ApiError::from_response` produces that variant for 400 and nothing
        // else, so the two cannot drift apart. A 5xx carrying similar prose is
        // an engine fault and stays retriable.
        match ApiError::from_response(response).await {
            ApiError::Validation(msg) => {
                classify_order_kill(&msg).unwrap_or(Self::Api(ApiError::Validation(msg)))
            }
            other => Self::Api(other),
        }
    }

    /// Whether re-sending the same request could plausibly produce a different result.
    ///
    /// Delegates to [`ApiError::is_retriable`] for transport and HTTP failures.
    /// The FAK/FOK kill outcomes and all local signing/encoding failures are
    /// deterministic and return `false`.
    ///
    /// ```
    /// # use polyoxide_clob::ClobError;
    /// let killed = ClobError::FakUnmatched { message: "no orders found to match".into() };
    /// assert!(!killed.is_retriable());
    /// ```
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(e) => e.is_retriable(),
            Self::FakUnmatched { .. } | Self::FokUnfilled { .. } => false,
            Self::Crypto(_) | Self::Alloy(_) | Self::InvalidTickSize(_) => false,
        }
    }

    /// Create validation error
    pub(crate) fn validation(msg: impl Into<String>) -> Self {
        Self::Api(ApiError::Validation(msg.into()))
    }

    /// Create service error (external dependency failure)
    #[cfg_attr(not(feature = "gamma"), allow(dead_code))]
    pub(crate) fn service(msg: impl Into<String>) -> Self {
        Self::Api(ApiError::Api {
            status: 0,
            message: msg.into(),
        })
    }
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

impl From<reqwest::Error> for ClobError {
    fn from(err: reqwest::Error) -> Self {
        Self::Api(ApiError::Network(err))
    }
}

impl From<url::ParseError> for ClobError {
    fn from(err: url::ParseError) -> Self {
        Self::Api(ApiError::Url(err))
    }
}

impl From<serde_json::Error> for ClobError {
    fn from(err: serde_json::Error) -> Self {
        Self::Api(ApiError::Serialization(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_error_is_api_not_validation() {
        let err = ClobError::service("Gamma client failed");
        match &err {
            ClobError::Api(ApiError::Api { status, message }) => {
                assert_eq!(*status, 0);
                assert_eq!(message, "Gamma client failed");
            }
            other => panic!("Expected ApiError::Api, got {:?}", other),
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

    #[test]
    fn test_service_and_validation_are_distinct() {
        let service = ClobError::service("service failure");
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
        let clob_err = ClobError::from(json_err);
        assert!(matches!(
            clob_err,
            ClobError::Api(ApiError::Serialization(_))
        ));
    }

    #[test]
    fn test_from_url_parse_error() {
        let url_err = url::Url::parse("://bad").unwrap_err();
        let clob_err = ClobError::from(url_err);
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

    // ── is_retriable ────────────────────────────────────────────

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
        assert!(ClobError::Api(ApiError::RateLimit("slow down".into())).is_retriable());
        assert!(ClobError::Api(ApiError::Timeout).is_retriable());
        assert!(ClobError::Api(ApiError::Api {
            status: 500,
            message: "order timed out".into()
        })
        .is_retriable());
        // 425 Too Early — matching engine restarting.
        assert!(ClobError::Api(ApiError::Api {
            status: 425,
            message: String::new()
        })
        .is_retriable());
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
}
