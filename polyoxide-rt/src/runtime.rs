//! The `Runtime` trait.

use std::fmt;
use std::time::Duration;

use crate::{BoxFuture, Instant};

/// What a polyoxide crate needs of an async runtime: a detached spawn, a
/// sleep, a yield to the host, and a monotonic clock. Everything else in this
/// crate (`spawn`'s `Task`, `timeout`, `Interval`) is built on these, so an
/// implementor writes only these.
///
/// A client takes its runtime from its builder, and falls back to
/// `default_runtime()`; nothing is process-wide.
pub trait Runtime: Send + Sync + fmt::Debug + 'static {
    /// Run `task` to completion, detached.
    fn spawn(&self, task: BoxFuture<()>);

    /// Complete after `duration`. Any duration, `Duration::MAX` included, is
    /// valid.
    fn sleep(&self, duration: Duration) -> BoxFuture<()>;

    /// Hand the thread back to the host once: work already queued (other
    /// tasks, I/O callbacks, a browser's messages) runs before the caller
    /// continues, and no timer is involved.
    ///
    /// It has no default because nothing generic really yields: re-waking the
    /// task at once only re-queues it, which under a browser's microtask queue
    /// never reaches the event loop, and a zero sleep is no timer at all
    /// there. Tokio's runtime defers to `tokio::task::yield_now`; the
    /// browser's resumes on the next macrotask, through a `MessageChannel`
    /// post, which no timer clamp delays.
    fn yield_now(&self) -> BoxFuture<()>;

    /// The runtime's monotonic clock.
    fn now(&self) -> Instant;

    /// Complete at `deadline`, at once when it has passed.
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<()> {
        self.sleep(deadline.saturating_duration_since(self.now()))
    }
}
