//! Spot market streams on `stream.binance.com`.
//!
//! Binance serves spot's streams on one combined path, `/stream`: every
//! [`StreamName`] is a name on it, the bare [`SpotWs`] holds one connection,
//! and the supervised tier ([`SpotWsBuilder`]) keeps that connection up,
//! reconnecting, replaying its streams and replacing it before Binance's
//! 24-hour cutoff.
//!
//! Binance acknowledges every `SUBSCRIBE` and enforces its connection rules by
//! closing the connection, so both tiers enforce them before sending: at most
//! [`MAX_STREAMS_PER_CONNECTION`] streams, [`MAX_NAMES_PER_REQUEST`] names per
//! request, and one request per [`MIN_REQUEST_INTERVAL`]. The host closes a
//! connection that receives more than five messages a second, requests, pings
//! and pongs counted, and bans an address it disconnects repeatedly, so the
//! pace is not a knob: a request every 250 ms was closed with 1008 "Too many
//! requests" where one every 500 ms held. The measurements are in
//! `docs/specs/binance/OBSERVED.md`.
//!
//! ```no_run
//! use futures_util::StreamExt;
//! use polyoxide_binance::spot::{types::Symbol, ws::{Event, SpotWsBuilder, StreamName}};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let btc = Symbol::new("BTCUSDT")?;
//! let mut feed = SpotWsBuilder::new()
//!     .streams([StreamName::AggTrade(btc.clone()), StreamName::BookTicker(btc)])
//!     .connect()
//!     .await?;
//! while let Some(event) = feed.next().await {
//!     if let Event::Update(update) = event? {
//!         println!("{}", update.stream);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

// The test-only `scripted` below is a second `mod` item over `usdm::ws`'s scripted
// server; the allow sits here, not on the item, because the test-bodies gate reads an
// item's attributes as the gate of the file it declares.
#![cfg_attr(test, allow(clippy::duplicate_mod))]

pub mod client;
pub mod error;
pub mod event;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;
mod protocol;
#[cfg(test)]
#[path = "../../usdm/ws/scripted.rs"]
mod scripted;
pub mod stream;
pub mod supervised;

use std::{fmt, time::Duration};

pub use client::SpotWs;
pub use error::{SpotWsError, StopCause};
pub use event::{
    AggTradeEvent, AvgPriceEvent, BookTickerEvent, KlineBar, KlineEvent, MiniTickerEvent,
    PartialDepthEvent, Payload, TickerEvent, TradeEvent, Update,
};
pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};
pub use supervised::{DisconnectReason, Event, MembershipHandle, SpotWsBuilder, SupervisedSpotWs};

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
