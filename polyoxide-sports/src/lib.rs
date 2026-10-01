//! Rust client for Polymarket's live sports feed at
//! `wss://sports-api.polymarket.com/ws`.

#![warn(missing_docs)]

pub mod update;

pub use update::{GameKey, MatchUpdate};

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;
