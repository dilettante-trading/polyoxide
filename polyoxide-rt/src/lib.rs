//! # polyoxide-rt
//!
//! The runtime seam of the polyoxide crates: the `Runtime` trait, which a
//! crate takes instead of calling a runtime's own functions, and `TokioRuntime`,
//! its native implementation.

#![warn(missing_docs)]

mod runtime;
#[cfg(not(target_arch = "wasm32"))]
mod tokio;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use runtime::Runtime;
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
pub use self::tokio::TokioRuntime;

/// An owned, sendable future: what a [`Runtime`] spawns and sleeps on.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// The runtime a client uses when none is injected: `TokioRuntime`.
///
/// Only its native arm exists so far; the wasm32 arm arrives with the browser
/// runtime.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_runtime() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}
