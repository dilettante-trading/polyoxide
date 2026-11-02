//! The `Runtime` trait.

use std::fmt;
use std::time::Duration;

use crate::{BoxFuture, Instant};

/// What a polyoxide crate needs of an async runtime: a detached spawn, a
/// sleep, and a monotonic clock. Everything else in this crate (`spawn`'s
/// `Task`, `timeout`, `Interval`) is built on these, so an implementor writes
/// only these.
///
/// A client takes its runtime from its builder, and falls back to
/// `default_runtime()`; nothing is process-wide.
pub trait Runtime: Send + Sync + fmt::Debug + 'static {
    /// Run `task` to completion, detached.
    fn spawn(&self, task: BoxFuture<()>);

    /// Complete after `duration`. Any duration, `Duration::MAX` included, is
    /// valid.
    fn sleep(&self, duration: Duration) -> BoxFuture<()>;

    /// The runtime's monotonic clock.
    fn now(&self) -> Instant;

    /// Complete at `deadline`, at once when it has passed.
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<()> {
        self.sleep(deadline.saturating_duration_since(self.now()))
    }
}
