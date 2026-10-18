//! # Predictions
//!
//! Client for Kalshi's Predictions exchange (`external-api.kalshi.com`), behind the `predictions`
//! feature, which is on by default. The module is a walking skeleton: it grows a part at a time,
//! and what it offers today is in its namespaces.
//!
//! Every request runs on core's send loop and costs tokens from a Read throttle. The throttle
//! starts at the size Kalshi gives its Basic tier, and a client confirms the account's own size
//! when it first sends. The hosts are [`PRODUCTION`] and [`DEMO`], and keys do not cross between
//! them.

pub mod error;
pub mod key;
// Task 1.7 uses what these two modules define, and removes both allows.
#[allow(dead_code)]
pub mod request;
#[allow(dead_code)]
mod throttle;
pub mod types;

pub use error::{PredictionsError, VenueError};
pub use key::key;

/// Production REST host: a scheme and host, with no path.
pub const PRODUCTION: &str = "https://external-api.kalshi.com";

/// Demo REST host. Keys do not cross environments.
pub const DEMO: &str = "https://external-api.demo.kalshi.co";

/// In-flight requests the client allows by default.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;
