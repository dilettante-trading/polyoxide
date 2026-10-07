//! USDⓈ-M market streams on `fstream.binance.com`.
//!
//! Binance routes streams by path: `/public` carries partial depth and book
//! tickers, `/market` the rest of the streams here. Every [`StreamName`] knows
//! its path, the bare [`UsdmWs`] holds one connection on one path, and the
//! supervised tier ([`UsdmWsBuilder`]) keeps one connection per path, opened
//! when its first stream is wanted and closed when its last leaves.
//!
//! Binance acknowledges every `SUBSCRIBE` and enforces its connection rules by
//! closing the connection, so both tiers enforce them before sending: at most
//! [`MAX_STREAMS_PER_CONNECTION`] streams, [`MAX_NAMES_PER_REQUEST`] names per
//! request, and one request per [`MIN_REQUEST_INTERVAL`]. The measurements are
//! in `docs/specs/binance/OBSERVED.md`.
pub mod client;
pub mod error;
pub mod event;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;
pub mod stream;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;

use std::{fmt, time::Duration};

pub use client::UsdmWs;
pub use error::{Recovery, UsdmWsError};
pub use event::{
    AggTradeEvent, BookTickerEvent, KlineBar, KlineEvent, MarkPriceEvent, PartialDepthEvent,
    Payload, SymbolType, TickerEvent, Update,
};
pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};

/// The production stream host.
pub const USDM_WS_BASE: &str = "wss://fstream.binance.com";

/// Streams one connection may carry. The 1025th is answered with an error,
/// and then the server closes the connection.
pub const MAX_STREAMS_PER_CONNECTION: usize = 1024;

/// Stream names per `SUBSCRIBE` or `UNSUBSCRIBE`. 200 were measured accepted.
pub const MAX_NAMES_PER_REQUEST: usize = 200;

/// The shortest gap between two requests on one connection: 5 per second,
/// half the documented 10. Of 40 requests sent back to back, 15 were answered
/// before the server closed the connection.
pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(200);

/// Which routed path a connection is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamPath {
    /// `/market`: tickers, mark prices, aggregate trades, klines.
    Market,
    /// `/public`: partial depth and book tickers.
    Public,
}

impl StreamPath {
    /// Both paths.
    pub const ALL: &'static [Self] = &[Self::Market, Self::Public];

    /// The path segment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Public => "public",
        }
    }

    /// The combined-stream URL on `base`.
    pub(crate) fn url(self, base: &str) -> String {
        format!("{}/{}/stream", base.trim_end_matches('/'), self.as_str())
    }
}

impl fmt::Display for StreamPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Make sure rustls has a default `CryptoProvider` before opening a connection.
///
/// The fifth twin of the function in `polyoxide-clob`, `-rtds`, `-perps` and
/// `-sports`: `tokio-tungstenite` builds its TLS config from the process-wide
/// default provider, and rustls installs one automatically only when exactly
/// one backend is enabled. With `ring` and `aws-lc-rs` both in the graph it
/// installs neither and panics inside `connect_async`.
pub(crate) fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
