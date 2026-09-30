//! WebSocket streaming of the public Perps channels.
//!
//! One multiplexed connection to [`WS_URL`]. Requests are
//! `{ "id", "req": "sub" | "unsub" | "post", … }`; push frames are
//! `{ "ch", "ts", "ets", "sq", "data" }`. The server closes a connection after
//! 60 s without an inbound message, so a bare [`PerpsWs`] needs its caller to
//! [`ping`](PerpsWs::ping); [`SupervisedPerpsWs`] does that itself and also
//! reconnects.
//! Everything the published AsyncAPI gets wrong about the wire is in
//! `docs/specs/perps/OBSERVED.md`.

pub mod channel;
pub mod client;
pub mod error;
pub mod event;
pub mod frame;
pub mod supervised;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;

pub use channel::{Channel, StreamDepth};
pub use client::PerpsWs;
pub use error::{PerpsWsError, Recovery, Refusal};
pub use event::{Event, Frame, Payload, Update};
pub use frame::{BboData, BookData, StatisticsData, TickerData, TradeData};
pub use supervised::{MembershipHandle, PerpsWsBuilder, SupervisedPerpsWs};

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

/// Parse one text frame as the stream would. For the crate's own
/// integration tests; not API.
#[doc(hidden)]
pub fn frame_from_text_for_tests(text: &str) -> Result<Frame, PerpsWsError> {
    match frame::Incoming::parse(text) {
        Ok(frame::Incoming::Push(push)) => Frame::from_push(push, text),
        Ok(frame::Incoming::Response(_)) | Err(_) => Err(PerpsWsError::Unrecognised {
            raw: text.to_owned(),
        }),
    }
}

/// What [`incoming_from_text_for_tests`] decodes a text frame into. For the
/// crate's own integration tests; not API.
#[doc(hidden)]
#[derive(Debug)]
pub enum IncomingForTests {
    /// A push frame, decoded as the stream would.
    Push(Frame),
    /// A `sub`/`unsub` response: `(accepted, error)` per requested channel.
    Statuses(Vec<(bool, Option<String>)>),
    /// A ping response.
    Pong {
        /// Whether the server answered `ok`.
        ok: bool,
        /// The sequence stamp it carried.
        sq: Option<u64>,
    },
}

/// Parse one text frame through the private envelopes, including the
/// response decoders the control methods use. For the crate's own
/// integration tests; not API.
#[doc(hidden)]
pub fn incoming_from_text_for_tests(text: &str) -> Result<IncomingForTests, PerpsWsError> {
    let incoming = frame::Incoming::parse(text).map_err(|_| PerpsWsError::Unrecognised {
        raw: text.to_owned(),
    })?;
    match incoming {
        frame::Incoming::Push(push) => Frame::from_push(push, text).map(IncomingForTests::Push),
        frame::Incoming::Response(response) => {
            let malformed = || PerpsWsError::Response {
                id: response.id.unwrap_or_default(),
                raw: response.data.to_string(),
            };
            if response.data.is_array() {
                let statuses = response.statuses().map_err(|_| malformed())?;
                Ok(IncomingForTests::Statuses(
                    statuses.into_iter().map(|s| (s.is_ok(), s.error)).collect(),
                ))
            } else {
                let pong = response.pong().map_err(|_| malformed())?;
                Ok(IncomingForTests::Pong {
                    ok: pong.is_ok(),
                    sq: pong.sq,
                })
            }
        }
    }
}
