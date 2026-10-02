//! # polyoxide-sports
//!
//! Rust client for Polymarket's live sports feed at
//! `wss://sports-api.polymarket.com/ws`.
//!
//! The feed needs no credentials and takes no subscription: a connection
//! receives every live match in every league. Each frame is a
//! [`MatchUpdate`] carrying one match's full current state, not a change.
//!
//! Two tiers:
//!
//! - [`SportsWs`] is a bare stream that ends when its connection does.
//! - [`SportsWsBuilder`] builds a [`SupervisedSportsWs`], which reconnects
//!   for as long as it is held and yields [`Event::Disconnected`] and
//!   [`Event::Reconnected`] around every outage. A reconnect refused in a
//!   way retrying cannot fix, such as a `404`, ends it with an error instead.
//!
//! The AsyncAPI document Polymarket publishes for this host does not match
//! the wire. Everything here is modelled on captured frames, and the
//! differences are recorded in `docs/specs/sports/OBSERVED.md` in the
//! repository.

#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod client;
pub mod error;
pub mod supervised;
pub mod update;

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;

pub use client::{SportsWs, SPORTS_WS_URL};
pub use error::SportsError;
pub use supervised::{Event, SportsWsBuilder, SupervisedSportsWs};
pub use update::{GameKey, MatchUpdate};
