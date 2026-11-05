//! # polyoxide-rt
//!
//! The runtime seam of the polyoxide crates. A crate that sleeps, reads the
//! clock or spawns takes a [`Runtime`] instead of calling a runtime's own
//! functions, so a runtime can be injected rather than assumed.
//!
//! - [`Runtime`] is a detached `spawn`, a `sleep`, a `yield_now` that hands
//!   the thread to the host once with no timer, a monotonic `now` and a
//!   defaulted `sleep_until`. An implementor writes only those.
//! - [`spawn`] turns any future into a [`Task`], which resolves to `Some`
//!   output, or to `None` when it was aborted or panicked, and keeps running
//!   when it is dropped.
//! - [`timeout`] bounds a future by a duration on a runtime's clock, polling
//!   the future first, so a ready future wins at `Duration::ZERO`.
//! - [`interval`] ticks at once and then every period, and an [`Interval`]
//!   takes a [`MissedTick`] to say what to do about ticks it was late for.
//! - `TokioRuntime` is the native runtime, on tokio's own clock so a paused
//!   clock drives it, and [`default_runtime`] returns it off wasm32. On wasm32
//!   `BrowserRuntime` is the default: `spawn_local`, `setTimeout`, a
//!   `MessageChannel` post and `performance.now`.
//! - [`Instant`], [`SystemTime`] and [`UNIX_EPOCH`] are `web-time`'s: std's own
//!   types on native.
//!
//! ```
//! use std::time::Duration;
//!
//! use polyoxide_rt::{default_runtime, spawn, timeout};
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!     let runtime = default_runtime();
//!
//!     let task = spawn(&*runtime, async { 6 * 7 });
//!     assert_eq!(task.await, Some(42));
//!
//!     let slow = runtime.sleep(Duration::from_secs(60));
//!     let waited = timeout(&*runtime, Duration::from_millis(10), slow).await;
//!     assert_eq!(waited, None);
//! }
//! ```
//!
//! The crate depends on nothing in the workspace and names no venue, so it
//! publishes first and every other crate can use it.

#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod browser;
mod runtime;
mod task;
mod time;
#[cfg(not(target_arch = "wasm32"))]
mod tokio;

use std::future::Future;
use std::pin::Pin;

pub use runtime::Runtime;
pub use task::{spawn, Task};
pub use time::{interval, timeout, Interval, MissedTick};
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
pub use self::tokio::TokioRuntime;
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserRuntime;

/// An owned, sendable future: what a [`Runtime`] spawns and sleeps on.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// The runtime a client uses when none is injected: `TokioRuntime` on a host
/// target and `BrowserRuntime` on `wasm32`.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_runtime() -> std::sync::Arc<dyn Runtime> {
    std::sync::Arc::new(TokioRuntime)
}

/// The runtime a client uses when none is injected: `TokioRuntime` on a host
/// target and `BrowserRuntime` on `wasm32`.
#[cfg(target_arch = "wasm32")]
pub fn default_runtime() -> std::sync::Arc<dyn Runtime> {
    std::sync::Arc::new(BrowserRuntime)
}
