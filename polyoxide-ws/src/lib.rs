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
//!
//! [`tungstenite`] is re-exported, so a venue names `Message` and
//! `CloseFrame` through this crate rather than depending on it.
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
mod error;
mod timing;
mod tls;

pub use backoff::{Backoff, MIN_BACKOFF};
pub use error::{Kind, WsError};
pub use timing::{next_wait, Timing};
pub use tls::ensure_crypto_provider;
/// The WebSocket library the kit is built on, re-exported so a venue names
/// its types (`Message`, `CloseFrame`, `http`) through this crate.
pub use tokio_tungstenite::tungstenite;
