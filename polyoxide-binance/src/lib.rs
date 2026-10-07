//! Rust client for Binance USDⓈ-M futures public market data
//! (`fapi.binance.com`).

pub mod error;
pub mod usdm;
pub mod weight;

pub use error::BinanceError;
pub use weight::WeightBudget;
