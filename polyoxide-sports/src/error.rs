//! The crate's error type.

use std::time::Duration;

use tokio_tungstenite::tungstenite;

/// Everything that can go wrong on the sports feed.
///
/// On the supervised stream only [`Decode`](Self::Decode) reaches the caller
/// as an `Err`. Every other variant arrives inside `Event::Disconnected`,
/// after the stream has already acted on it.
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
