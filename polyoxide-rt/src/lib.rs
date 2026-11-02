//! # polyoxide-rt
//!
//! The runtime seam of the polyoxide crates: the clock types and the future
//! alias every other module of this crate is written against.

#![warn(missing_docs)]

use std::future::Future;
use std::pin::Pin;

pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

/// An owned, sendable future: what a runtime spawns and sleeps on.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
