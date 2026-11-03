//! Binance-wide plumbing the REST products share.
//!
//! The minute-weight engine and its cooldown, the retry decision, the error
//! decode by status, the routed request and `Symbol`: everything about
//! Binance's REST hosts that does not depend on which product's routes a
//! request names. Each product keeps its own weight table, error enum, retry
//! policy handle and response types, and builds them on these.
//!
//! The `ws` module is the socket plumbing the `usdm-ws` and `spot-ws` streams
//! share: the request correlation, the bare connection, the supervised
//! protocol and the router, generic over a product's stream set and error.

pub(crate) mod error;
pub(crate) mod policy;
pub(crate) mod request;
pub(crate) mod symbol;
pub(crate) mod weight;
#[cfg(any(feature = "usdm-ws", feature = "spot-ws"))]
pub(crate) mod ws;
