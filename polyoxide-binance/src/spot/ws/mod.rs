//! Spot market streams on `stream.binance.com`.
//!
//! Spot has one combined-stream path, `/stream`, on `SPOT_WS_BASE`. Binance
//! acknowledges every `SUBSCRIBE`, including a stream type that does not exist,
//! an uppercase symbol and an unknown symbol, and none of those deliver
//! anything, so a stream is named only through `StreamName`, which builds the
//! spellings Binance serves. A frame is an `Update`: the stream it arrived on
//! and its `Payload`.
//!
//! Binance enforces its connection rules by closing the connection, so the
//! clients hold them first: at most `MAX_STREAMS_PER_CONNECTION` streams,
//! `MAX_NAMES_PER_REQUEST` names per request and one request per
//! `MIN_REQUEST_INTERVAL`. The measurements are in
//! `docs/specs/binance/OBSERVED.md`, under `## Spot`.

pub mod error;

use std::{fmt, time::Duration};

pub use error::{SpotWsError, StopCause};

/// The production stream host. The client appends `/stream`.
pub const SPOT_WS_BASE: &str = "wss://stream.binance.com:9443";

/// Streams one connection may carry. Binance documents 1024 and 1024 were
/// measured held (2026-11-02); the client never sends a 1025th.
pub const MAX_STREAMS_PER_CONNECTION: usize = 1024;

/// Stream names per `SUBSCRIBE` or `UNSUBSCRIBE`. Binance documents no limit;
/// one request of 1024 names (23,598 bytes) was acknowledged and held, as were
/// 200 and 500 (2026-11-02), so a request carries as many names as a
/// connection holds.
pub const MAX_NAMES_PER_REQUEST: usize = 1024;

/// The shortest gap between two requests on one connection. Binance allows 5
/// incoming messages a second, pings, pongs and requests counted. A request
/// every 250 ms, with a client ping and a pong, was closed about 16 s in with
/// `1008 "Too many requests"` (at most 6 messages in a second); a request every
/// 500 ms held for 45 s with two pings and two pongs (at most 4 in a second).
pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(500);

/// Which routed path a connection is on. Spot has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamPath {
    /// `/stream`: every spot market stream.
    Stream,
}

impl StreamPath {
    /// Every path.
    pub const ALL: &'static [Self] = &[Self::Stream];

    /// The path segment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stream => "stream",
        }
    }

    /// The combined-stream URL on `base`.
    // Story 4's `SpotStreams::url` is the first caller; it removes this allow.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn url(self, base: &str) -> String {
        format!("{}/{}", base.trim_end_matches('/'), self.as_str())
    }
}

impl fmt::Display for StreamPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_has_one_path_and_it_is_stream() {
        assert_eq!(StreamPath::ALL, &[StreamPath::Stream]);
        assert_eq!(StreamPath::Stream.as_str(), "stream");
        assert_eq!(StreamPath::Stream.to_string(), "stream");
    }

    #[test]
    fn the_url_appends_the_one_path_to_the_base() {
        assert_eq!(
            StreamPath::Stream.url(SPOT_WS_BASE),
            "wss://stream.binance.com:9443/stream"
        );
        // A scripted server's base, with or without a trailing slash.
        assert_eq!(
            StreamPath::Stream.url("ws://127.0.0.1:4000"),
            "ws://127.0.0.1:4000/stream"
        );
        assert_eq!(
            StreamPath::Stream.url("ws://127.0.0.1:4000/"),
            "ws://127.0.0.1:4000/stream"
        );
    }

    #[test]
    fn the_limits_are_the_measured_ones() {
        // OBSERVED.md, `## Spot`: 1024 names in one request held; a request every
        // 250 ms was closed with 1008, every 500 ms held for 45 s.
        assert_eq!(MAX_STREAMS_PER_CONNECTION, 1024);
        assert_eq!(MAX_NAMES_PER_REQUEST, 1024);
        assert_eq!(MIN_REQUEST_INTERVAL, Duration::from_millis(500));
    }
}
