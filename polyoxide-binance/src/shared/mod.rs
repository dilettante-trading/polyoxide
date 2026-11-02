//! Binance-wide plumbing the REST products share.
//!
//! The minute-weight engine and its cooldown, the retry decision, the error
//! decode by status, the routed request and `Symbol`: everything about
//! Binance's REST hosts that does not depend on which product's routes a
//! request names. Each product keeps its own weight table, error enum, retry
//! policy handle and response types, and builds them on these.

pub(crate) mod error;
pub(crate) mod policy;
pub(crate) mod symbol;
pub(crate) mod weight;
