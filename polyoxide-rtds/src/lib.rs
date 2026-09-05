//! # polyoxide-rtds
//!
//! Rust client for Polymarket's Real-Time Data Service (RTDS), served at
//! `wss://ws-live-data.polymarket.com`.
//!
//! RTDS relays reference prices without credentials: Binance spot, Chainlink
//! spot, and Chainlink-computed 30-second and 60-second TWAPs. It is a
//! different protocol from the CLOB WebSocket channels in `polyoxide-clob` —
//! many topics are multiplexed over one connection.
//!
//! # Precision
//!
//! Each update carries both a lossy `display_value: f64` and an exact
//! [`Decimal`](rust_decimal::Decimal) `value`. **Always use `value` for
//! arithmetic.** The wire field it is decoded from, `full_accuracy_value`, is
//! E18 fixed-point on the Chainlink topics and a plain decimal on Binance, so
//! each topic has its own type and the scale is never a runtime decision.

#![warn(missing_docs)]

pub mod subscription;
pub mod topic;

pub use subscription::{Subscription, SubscriptionRequest};
pub use topic::{Topic, TwapWindow};
