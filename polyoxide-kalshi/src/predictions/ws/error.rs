//! The supervised socket's error, and its classes.

use std::time::Duration;

use polyoxide_venue::{Class, Classify};
use polyoxide_ws::WsError;
use thiserror::Error;

use crate::predictions::CredentialsError;

polyoxide_venue::impl_ws_classification!();

/// The error of the supervised socket, whatever channels it carries, and of its membership
/// calls.
///
/// Every socket failure arrives as the kit's [`WsError`](polyoxide_ws::WsError) first,
/// and is classed by the one socket table. Kalshi's error frames are classed by code.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PredictionsWsError {
    /// The connection could not be opened, or failed mid-stream: the kit's one socket
    /// error, read through its `Kind`. A refused upgrade, a server close and a connect
    /// timeout are all this.
    #[error("WebSocket transport error: {0}")]
    Connection(#[from] WsError),

    /// Kalshi sent an error frame: it answered a command with one, or reported a
    /// problem on a subscription. Its codes run from 1 to 28.
    #[error("Kalshi sent error {code}: {message}")]
    Server {
        /// Kalshi's error code.
        code: u32,
        /// Kalshi's message.
        message: String,
    },

    /// A frame carried a sequence number other than the one after the last of its
    /// subscription: a message was lost, so the book it builds cannot be trusted. The
    /// socket reconnects and takes fresh snapshots.
    #[error("subscription {sid} skipped from sequence {expected} to {received}")]
    SequenceGap {
        /// The subscription.
        sid: u64,
        /// The sequence number that should have come.
        expected: u64,
        /// The one that did.
        received: u64,
    },

    /// A frame did not decode: it is not JSON, or it is a frame this socket knows with a
    /// field it cannot read. The connection stays.
    #[error("a frame did not decode: {source}")]
    Json {
        /// The frame's text.
        text: String,
        /// Why it did not decode.
        #[source]
        source: serde_json::Error,
    },

    /// The socket's URL is neither `wss://` nor `ws://` to a loopback host, so the
    /// signed handshake would not travel encrypted to a remote host.
    #[error("invalid WebSocket URL {url}: {reason}")]
    InvalidUrl {
        /// The URL as given.
        url: String,
        /// Why it was refused.
        reason: &'static str,
    },

    /// Kalshi did not send the snapshots a gap in the order book's sequence asked for within
    /// the staleness window. The socket reconnects, and the replay brings fresh snapshots.
    #[error("the snapshots of subscription {sid} did not arrive within {waited:?}")]
    SnapshotTimeout {
        /// The order-book subscription that gapped.
        sid: u64,
        /// How long the socket waited.
        waited: Duration,
    },

    /// The socket refused a request before sending it: an empty set of channels at
    /// `connect`, or a `snapshot` of a market that is not subscribed or on a socket without
    /// the order-book channel.
    #[error("invalid request: {reason}")]
    InvalidRequest {
        /// Why it was refused.
        reason: String,
    },

    /// The handshake could not be signed.
    #[error("could not sign the handshake: {0}")]
    Credentials(#[from] CredentialsError),
}

/// The transport by the one socket table (`polyoxide_venue::impl_ws_classification!`),
/// whose only non-fault is an upgrade refused with 451 and whose wait is a refusal's
/// `Retry-After`. Kalshi's error frames are classed by code: 9 (authentication
/// required) is `Unauthorized`, 27 (the command rate limit) `RateLimited`, 10 (a channel
/// error), 17 (an internal error), 18 (a command timeout) and 25 (the subscription's
/// buffer overflowed) `Unavailable`, and every other code `InvalidRequest`. Kalshi's
/// AsyncAPI calls codes 10 and 25 terminal ("the user must resubscribe"), and 17 and 18
/// are the venue's own failures, so all four are retriable: the socket reconnects and its
/// replay resubscribes. A gap in the sequence is a lost message, `Network` and a fault; a
/// snapshot that never came is `Unavailable`; a frame that did not decode is `Decode`; a URL
/// or a key that cannot be used, and a request the socket refuses before sending it, are
/// `InvalidRequest`.
impl Classify for PredictionsWsError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => ws_class(err),
            Self::Server { code, .. } => match code {
                9 => Class::Unauthorized,
                10 | 17 | 18 | 25 => Class::Unavailable {
                    code: Some(code.to_string().into()),
                },
                27 => Class::RateLimited { retry_after: None },
                _ => Class::InvalidRequest,
            },
            Self::SequenceGap { .. } => Class::Network,
            Self::Json { .. } => Class::Decode,
            Self::InvalidUrl { .. } | Self::InvalidRequest { .. } => Class::InvalidRequest,
            Self::SnapshotTimeout { .. } => Class::Unavailable { code: None },
            Self::Credentials(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Connection(err) if !ws_is_fault(err))
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Connection(err) => ws_retry_after(err),
            _ => self.class().retry_after(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use polyoxide_ws::tungstenite::{self, http};

    use super::*;

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> PredictionsWsError {
        PredictionsWsError::from(WsError::from(err))
    }

    /// An upgrade refused with `status`, and a `Retry-After` when one is given.
    fn refused(status: u16, retry_after: Option<&str>) -> PredictionsWsError {
        let mut response = http::Response::builder().status(status);
        if let Some(wait) = retry_after {
            response = response.header("Retry-After", wait);
        }
        transport(tungstenite::Error::Http(response.body(None).unwrap()))
    }

    fn server(code: u32) -> PredictionsWsError {
        PredictionsWsError::Server {
            code,
            message: "a message".to_owned(),
        }
    }

    #[test]
    fn a_server_error_is_classed_by_its_code() {
        let rows = [
            (9, Class::Unauthorized, false),
            // Kalshi's AsyncAPI calls codes 10 and 25 terminal ("the user must
            // resubscribe"), and 17 and 18 are the venue's own failures: all four are the
            // socket's to retry.
            (
                10,
                Class::Unavailable {
                    code: Some("10".into()),
                },
                true,
            ),
            (
                17,
                Class::Unavailable {
                    code: Some("17".into()),
                },
                true,
            ),
            (
                18,
                Class::Unavailable {
                    code: Some("18".into()),
                },
                true,
            ),
            (
                25,
                Class::Unavailable {
                    code: Some("25".into()),
                },
                true,
            ),
            (27, Class::RateLimited { retry_after: None }, true),
            (7, Class::InvalidRequest, false),
            (11, Class::InvalidRequest, false),
            (26, Class::InvalidRequest, false),
        ];
        for (code, class, retriable) in rows {
            let err = server(code);
            assert_eq!(err.class(), class, "{code}");
            assert_eq!(err.is_retriable(), retriable, "{code}");
            assert!(err.is_fault(), "{code}");
            assert_eq!(err.retry_after(), None, "{code}");
        }
    }

    #[test]
    fn a_sequence_gap_is_a_retriable_network_fault() {
        let err = PredictionsWsError::SequenceGap {
            sid: 1,
            expected: 2,
            received: 4,
        };
        assert_eq!(err.class(), Class::Network);
        assert!(err.is_retriable() && err.is_fault());
        assert_eq!(
            err.to_string(),
            "subscription 1 skipped from sequence 2 to 4"
        );
    }

    #[test]
    fn a_frame_that_did_not_decode_is_a_decode_error() {
        let err = PredictionsWsError::Json {
            text: "x".to_owned(),
            source: serde_json::from_str::<u8>("x").unwrap_err(),
        };
        assert_eq!(err.class(), Class::Decode);
        assert!(!err.is_retriable());
        assert!(
            err.to_string().starts_with("a frame did not decode: "),
            "{err}"
        );
    }

    #[test]
    fn the_callers_own_mistakes_are_invalid_requests() {
        let url = PredictionsWsError::InvalidUrl {
            url: "http://x".to_owned(),
            reason: "it must be wss://, or ws:// to a loopback host",
        };
        let signing = PredictionsWsError::Credentials(CredentialsError::Sign {
            reason: "x".to_owned(),
        });
        assert!(
            signing
                .to_string()
                .starts_with("could not sign the handshake: "),
            "{signing}"
        );
        for err in [url, signing] {
            assert_eq!(err.class(), Class::InvalidRequest, "{err}");
            assert!(!err.is_retriable(), "{err}");
            assert_eq!(err.retry_after(), None, "{err}");
        }
    }

    #[test]
    fn a_refused_upgrade_follows_the_socket_table() {
        for (status, class) in [
            (401, Class::Unauthorized),
            (404, Class::VenueRefusal { code: None }),
            (429, Class::RateLimited { retry_after: None }),
            (503, Class::Unavailable { code: None }),
            (418, Class::Restricted),
            (451, Class::Restricted),
        ] {
            assert_eq!(refused(status, None).class(), class, "{status}");
        }
        // Only a region block is not a fault; a ban the client earned is one.
        assert!(!refused(451, None).is_fault());
        assert!(refused(418, None).is_fault());
    }

    #[test]
    fn a_refusals_retry_after_is_surfaced() {
        // As HTTP's errors do for every status; only a 429's class carries the wait.
        let wait = Some(Duration::from_secs(30));
        assert_eq!(
            refused(429, Some("30")).class(),
            Class::RateLimited { retry_after: wait }
        );
        assert_eq!(refused(429, Some("30")).retry_after(), wait);
        assert_eq!(
            refused(503, Some("30")).class(),
            Class::Unavailable { code: None }
        );
        assert_eq!(refused(503, Some("30")).retry_after(), wait);
        // A zero is no wait at all.
        assert_eq!(refused(429, Some("0")).retry_after(), None);
    }

    #[test]
    fn a_server_close_follows_the_close_code_table() {
        for (code, class) in [
            (Some(1000), Class::Network),
            (Some(1001), Class::Network),
            (Some(1006), Class::Network),
            (Some(1011), Class::Network),
            (None, Class::Network),
            (
                Some(1008),
                Class::VenueRefusal {
                    code: Some("1008".into()),
                },
            ),
        ] {
            let err = PredictionsWsError::from(WsError::closed(code, "a reason"));
            assert_eq!(err.class(), class, "{code:?}");
        }
    }

    #[test]
    fn the_transport_is_the_kits_table() {
        let rows = [
            (tungstenite::Error::ConnectionClosed, Class::Network),
            (
                tungstenite::Error::Io(std::io::ErrorKind::ConnectionReset.into()),
                Class::Network,
            ),
            (tungstenite::Error::AlreadyClosed, Class::InvalidRequest),
            (
                tungstenite::Error::Url(tungstenite::error::UrlError::NoHostName),
                Class::InvalidRequest,
            ),
        ];
        for (err, class) in rows {
            let err = transport(err);
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
        }
        for err in [
            WsError::connect_timeout(Duration::from_secs(10)),
            WsError::no_answer(Duration::from_secs(1)),
        ] {
            let err = PredictionsWsError::from(err);
            assert_eq!(err.class(), Class::Network, "{err:?}");
        }
    }
    #[test]
    fn a_missing_snapshot_is_an_unavailable_fault_the_socket_retries() {
        let err = PredictionsWsError::SnapshotTimeout {
            sid: 1,
            waited: Duration::from_secs(30),
        };
        assert_eq!(err.class(), Class::Unavailable { code: None });
        assert!(err.is_retriable() && err.is_fault());
        assert_eq!(err.retry_after(), None);
        assert_eq!(
            err.to_string(),
            "the snapshots of subscription 1 did not arrive within 30s"
        );
    }

    #[test]
    fn a_request_the_socket_refuses_is_an_invalid_request_and_is_not_retried() {
        let err = PredictionsWsError::InvalidRequest {
            reason: "the socket needs at least one channel".to_owned(),
        };
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(!err.is_retriable());
        assert_eq!(err.retry_after(), None);
        assert_eq!(
            err.to_string(),
            "invalid request: the socket needs at least one channel"
        );
    }
}
