//! Rust client for Polymarket's live sports feed at
//! `wss://sports-api.polymarket.com/ws`.

#![warn(missing_docs)]

pub mod client;
pub mod error;
pub mod supervised;
pub mod update;

pub use client::{SportsWs, SPORTS_WS_URL};
pub use error::SportsError;
pub use supervised::{Event, SportsWsBuilder, SupervisedSportsWs};
pub use update::{GameKey, MatchUpdate};

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;
