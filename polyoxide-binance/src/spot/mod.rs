//! # Spot
//!
//! Client for Binance spot public market data (`api.binance.com`), behind the
//! `spot` feature, which is on by default. No credentials are needed.

pub mod weight;

pub use weight::WeightBudget;
