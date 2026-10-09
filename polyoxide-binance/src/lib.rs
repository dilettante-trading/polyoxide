//! Rust client for Binance USDⓈ-M futures public market data
//! (`fapi.binance.com`) and, with the `ws` feature, its market streams
//! (`fstream.binance.com`).
//!
//! No credentials are needed. Every request is charged against a
//! [`WeightBudget`], because Binance limits each IP by request *weight*, which
//! varies by route and parameters, rather than by request count.
//!
//! ```no_run
//! use polyoxide_binance::{usdm::types::Symbol, Usdm};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let usdm = Usdm::new()?;
//! let btc = Symbol::new("BTCUSDT")?;
//! let index = usdm.market().premium_index(&btc).send().await?;
//! println!("BTCUSDT mark {} funding {}", index.mark_price, index.last_funding_rate);
//! # Ok(())
//! # }
//! ```

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod error;
pub mod usdm;
pub mod weight;

pub use error::BinanceError;
pub use usdm::{Usdm, UsdmBuilder};
pub use weight::WeightBudget;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<BinanceError>();
    is::<usdm::types::InvalidSymbol>();
    #[cfg(feature = "ws")]
    is::<usdm::ws::UsdmWsError>();
    #[cfg(feature = "ws")]
    is::<usdm::ws::InvalidStreamName>();
};
