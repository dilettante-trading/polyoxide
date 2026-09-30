//! WebSocket streaming of the public Perps channels.
//!
//! One multiplexed connection to [`WS_URL`]. Requests are
//! `{ "id", "req": "sub" | "unsub" | "post", … }`; push frames are
//! `{ "ch", "ts", "ets", "sq", "data" }`. The server closes a connection after
//! 60 s without an inbound message, so a bare [`PerpsWs`] needs its caller to
//! [`ping`](PerpsWs::ping); `SupervisedPerpsWs` does that itself and also
//! reconnects.
//! Everything the published AsyncAPI gets wrong about the wire is in
//! `docs/specs/perps/OBSERVED.md`.

pub mod channel;
pub mod client;
pub mod error;
pub mod event;
pub mod frame;
// pub mod supervised;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;

pub use channel::{Channel, StreamDepth};
pub use client::PerpsWs;
pub use error::{PerpsWsError, Recovery, Refusal};
pub use event::{Event, Frame, Payload, Update};
// pub use supervised::{MembershipHandle, PerpsWsBuilder, SupervisedPerpsWs};

/// The production WebSocket endpoint.
pub const WS_URL: &str = "wss://ws.perpetuals.polymarket.com/v1/ws";

/// Make sure rustls has a default `CryptoProvider` before opening a connection.
///
/// A deliberate twin of the same function in `polyoxide-rtds` and
/// `polyoxide-clob`: `tokio-tungstenite` builds its TLS config from the
/// process-wide default provider, and rustls installs one automatically only
/// when exactly one backend feature is enabled. With `ring` and `aws-lc-rs`
/// both in the graph it installs neither and panics inside `connect_async`.
/// `install_default` returns `Err` when a provider is already set, so crates
/// racing is a no-op.
pub(crate) fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
