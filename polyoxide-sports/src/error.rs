//! The crate's error type.

use std::time::Duration;

use tokio_tungstenite::tungstenite;

/// Everything that can go wrong on the sports feed.
///
/// On the supervised stream a lost connection arrives as
/// [`Transport`](Self::Transport), [`Closed`](Self::Closed) or
/// [`Stale`](Self::Stale) inside
/// [`Event::Disconnected`](crate::Event::Disconnected), after the stream has
/// acted on it. Two errors reach the caller as an `Err`:
///
/// - [`Decode`](Self::Decode), for a frame the stream skipped. It carries on.
/// - [`Connect`](Self::Connect), for a reconnect refused in a way retrying
///   cannot fix: an HTTP status other than 408, 425, 429 or 5xx, or a
///   malformed URL. The stream ends after it.
///
/// Any other failed reconnect attempt is retried and logged at `WARN`
/// through `tracing`, not yielded. [`Connect`](Self::Connect) and
/// [`ConnectTimeout`](Self::ConnectTimeout) also come back from
/// [`SportsWsBuilder::connect`](crate::SportsWsBuilder::connect) when the
/// first connection fails, whatever the cause.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SportsError {
    /// The connection could not be opened: the URL, DNS, TCP, TLS or the
    /// WebSocket upgrade failed.
    #[error("could not connect to the sports feed: {source}")]
    Connect {
        /// What the transport reported.
        #[source]
        source: Box<tungstenite::Error>,
    },
    /// Opening the connection took longer than the connect timeout.
    #[error("connecting to the sports feed took longer than {after:?}")]
    ConnectTimeout {
        /// The timeout that elapsed.
        after: Duration,
    },
    /// The server closed the connection, or the stream ended.
    #[error("the sports feed closed the connection{}", describe_close(.code, .reason))]
    Closed {
        /// The close status code, when the server's close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A read failed on an open connection.
    #[error("the sports feed connection failed: {source}")]
    Transport {
        /// What the transport reported.
        #[source]
        source: Box<tungstenite::Error>,
    },
    /// Nothing arrived, protocol pings included, within the staleness limit.
    #[error("nothing received from the sports feed for {after:?}, pings included")]
    Stale {
        /// The staleness limit that elapsed.
        after: Duration,
    },
    /// A text frame did not parse as a match update.
    #[error("a sports frame did not parse: {source}")]
    Decode {
        /// The frame as received.
        raw: String,
        /// Why it did not parse.
        #[source]
        source: serde_json::Error,
    },
}

impl SportsError {
    /// Whether a reconnect attempt that failed this way is worth repeating.
    ///
    /// The statuses are the ones polyoxide-core's `is_retriable` treats as
    /// passing: a timeout, the matching engine restarting, a rate limit, a
    /// server fault. polyoxide-perps retries only 429 and 5xx.
    pub(crate) fn retrying_can_fix(&self) -> bool {
        let Self::Connect { source } = self else {
            return true;
        };
        match &**source {
            tungstenite::Error::Http(response) => {
                let status = response.status().as_u16();
                matches!(status, 408 | 425 | 429) || status >= 500
            }
            // `Tls` is only an invalid server name here: rustls reports a
            // certificate it rejects as an IO error, which is retried.
            tungstenite::Error::Url(_)
            | tungstenite::Error::Tls(_)
            | tungstenite::Error::HttpFormat(_)
            | tungstenite::Error::AttackAttempt => false,
            _ => true,
        }
    }
}

/// The tail of the close message: the status code and reason when present.
fn describe_close(code: &Option<u16>, reason: &str) -> String {
    match (code, reason.is_empty()) {
        (Some(code), true) => format!(" with code {code}"),
        (Some(code), false) => format!(" with code {code}: {reason}"),
        (None, true) => " without a status code".to_owned(),
        (None, false) => format!(": {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_close_without_a_code_says_so() {
        let text = SportsError::Closed {
            code: None,
            reason: String::new(),
        }
        .to_string();
        assert_eq!(
            text,
            "the sports feed closed the connection without a status code"
        );
    }

    #[test]
    fn errors_cross_threads() {
        // The CLI turns this into an eyre::Report, which needs Send + Sync.
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SportsError>();
    }

    #[test]
    fn a_stale_error_says_pings_count() {
        let text = SportsError::Stale {
            after: Duration::from_secs(45),
        }
        .to_string();
        assert!(
            text.contains("45s") && text.contains("pings included"),
            "{text}"
        );
    }

    fn http_error(status: u16) -> SportsError {
        SportsError::Connect {
            source: Box::new(tungstenite::Error::Http(
                tungstenite::http::Response::builder()
                    .status(status)
                    .body(None)
                    .unwrap(),
            )),
        }
    }

    #[test]
    fn a_refused_request_is_not_retried_but_an_unwell_host_is() {
        for status in [301, 401, 403, 404] {
            assert!(!http_error(status).retrying_can_fix(), "{status}");
        }
        for status in [408, 425, 429, 500, 502, 503] {
            assert!(http_error(status).retrying_can_fix(), "{status}");
        }
    }

    #[test]
    fn a_transport_failure_or_timeout_is_retried() {
        let reset = SportsError::Connect {
            source: Box::new(tungstenite::Error::Io(std::io::Error::from(
                std::io::ErrorKind::ConnectionReset,
            ))),
        };
        assert!(reset.retrying_can_fix());
        let timeout = SportsError::ConnectTimeout {
            after: Duration::from_secs(10),
        };
        assert!(timeout.retrying_can_fix());
    }

    #[test]
    fn a_malformed_url_is_not_retried() {
        let url = SportsError::Connect {
            source: Box::new(tungstenite::Error::Url(
                tungstenite::error::UrlError::NoHostName,
            )),
        };
        assert!(!url.retrying_can_fix());
    }

    #[test]
    fn a_close_error_names_its_code_and_reason() {
        let text = SportsError::Closed {
            code: Some(1001),
            reason: "going away".into(),
        }
        .to_string();
        assert_eq!(
            text,
            "the sports feed closed the connection with code 1001: going away"
        );
    }
}
