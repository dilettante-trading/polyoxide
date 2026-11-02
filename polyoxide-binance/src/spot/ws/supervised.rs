//! The supervised tier: the Binance protocol on the kit's `Supervisor`, with
//! keep-alive on the wall clock, staleness detection, reconnect with a paced
//! replay of the streams, rotation before Binance's 24-hour cutoff and outage
//! markers. The protocol, the router and the forwarding are `shared::ws`'s.

use std::fmt;

use crate::spot::ws::{error::SpotWsError, event::Update, StreamPath};

/// What the supervised stream yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// One frame of one stream, boxed, as the other variants are small.
    Update(Box<Update>),
    /// The connection was lost. Every update is stale until the matching
    /// [`Event::Reconnected`], which always follows while the client runs.
    Disconnected {
        /// The path that went down: spot has one, [`StreamPath::Stream`].
        path: StreamPath,
        /// Why.
        reason: DisconnectReason,
    },
    /// The path is current again: a fresh connection has replayed its
    /// streams, or its last stream left during the outage, so nothing on it is
    /// stale. Sent once per outage, however many attempts it took. Drop book
    /// state and resync.
    Reconnected {
        /// The path that came back.
        path: StreamPath,
    },
}

/// Why the connection went down.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed it, with the code and reason it sent, or ended the
    /// stream with neither.
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },
    /// A transport error, a replay left unanswered for the staleness window,
    /// or a failed first connect of a path opened by a membership change.
    Error(SpotWsError),
    /// Nothing arrived, pongs included, for the staleness window, or a
    /// membership change went unanswered for it.
    Stale,
    /// Replaced on purpose before Binance's 24-hour cutoff.
    Rotation,
}

impl fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed { code, reason } => match code {
                Some(code) => write!(f, "closed by the server ({code} {reason})"),
                None => f.write_str("closed by the server"),
            },
            Self::Error(err) => write!(f, "{err}"),
            Self::Stale => f.write_str("no frame within the staleness window"),
            Self::Rotation => f.write_str("rotated before the 24-hour cutoff"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disconnect_reason_reads_as_the_texts_usdm_gives() {
        assert_eq!(
            DisconnectReason::Closed {
                code: Some(1008),
                reason: "Too many requests".to_owned()
            }
            .to_string(),
            "closed by the server (1008 Too many requests)"
        );
        assert_eq!(
            DisconnectReason::Closed {
                code: None,
                reason: String::new()
            }
            .to_string(),
            "closed by the server"
        );
        let refused = SpotWsError::Refused {
            code: 2,
            msg: "Invalid request".to_owned(),
        };
        let text = refused.to_string();
        assert_eq!(DisconnectReason::Error(refused).to_string(), text);
        assert_eq!(
            DisconnectReason::Stale.to_string(),
            "no frame within the staleness window"
        );
        assert_eq!(
            DisconnectReason::Rotation.to_string(),
            "rotated before the 24-hour cutoff"
        );
    }
}
