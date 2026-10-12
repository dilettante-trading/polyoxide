//! # polyoxide-ws
//!
//! The WebSocket kit the polyoxide socket clients share. Each block here was
//! carried as a copy by every socket crate (clob, perps, Binance, rtds and
//! sports); this crate holds the one copy, and each venue keeps only what is
//! its own.
//!
//! - [`ensure_crypto_provider`] installs the process-default rustls provider
//!   a `tokio-tungstenite` connect needs.
//! - [`Backoff`] is the reconnect delay schedule.
//! - [`Timing`] holds a supervised connection's ping, staleness, backoff,
//!   connect and rotation timings, [`timing_setters!`] gives a builder its
//!   setters, and [`next_wait`] is the wait until the first deadline.
//! - [`WsError`] is the one socket error, read through its [`Kind`].
//!   `polyoxide_venue::impl_ws_classification!` classifies it in the venue
//!   crate that wraps it.
//! - [`connect`] opens a connection within a timeout, [`connect_over`]
//!   upgrades a TCP connection a venue opened itself, [`with_timeout`] bounds
//!   a venue's own connect, and [`retrying_can_fix`] says whether a
//!   reconnect can help after one failed.
//! - [`Connection`] is the bare tier: one connection as a `Stream` of its
//!   frames in a [`Mode`], with the server's close captured, and requests
//!   correlated with their answers through [`Correlated`].
//! - [`Supervisor`] runs one supervised connection for a venue's
//!   [`Protocol`]: pings on the wall clock, staleness, reconnect with backoff
//!   and a replay of the membership, rotation, outage markers, and membership
//!   changes through a [`MembershipHandle`], yielding a [`Supervised`] stream.
//!
//! [`tungstenite`] is re-exported, so a venue names `Message` and
//! `CloseFrame` through this crate rather than depending on it.
//!
//! Behind the `test-server` feature, the doc-hidden `test_server` module holds
//! the scripted local server and the helpers the offline suites share. It is
//! for tests, not consumers.
//!
//! The crate depends on neither `polyoxide-core` nor `polyoxide-venue`, and
//! builds no HTTP or signing stack, so a credential-free feed built on it
//! stays light (AD-2). It logs under the `polyoxide_ws` target and never
//! installs a subscriber.

#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod backoff;
mod bare;
mod connect;
mod error;
mod request;
mod supervisor;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;
mod timing;
mod tls;

pub use backoff::{Backoff, MIN_BACKOFF};
pub use bare::{Connection, Mode};
pub use connect::{
    connect, connect_over, retrying_can_fix, with_timeout, Socket, DEFAULT_CONNECT_TIMEOUT,
};
pub use error::{Kind, WsError};
pub use request::Correlated;
pub use supervisor::{
    Decoded, MembershipHandle, Offline, Outage, Protocol, Queue, Recovery, Supervised, Supervisor,
};
pub use timing::{next_wait, Timing};
pub use tls::ensure_crypto_provider;
/// The WebSocket library the kit is built on, re-exported so a venue names
/// its types (`Message`, `CloseFrame`, `http`) through this crate.
pub use tokio_tungstenite::tungstenite;
