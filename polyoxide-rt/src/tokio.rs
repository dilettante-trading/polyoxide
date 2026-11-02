//! `TokioRuntime`, the native runtime.

use std::time::Duration;

use crate::{BoxFuture, Instant, Runtime};

/// The runtime over tokio's spawn, sleep and clock.
///
/// It reads tokio's clock, so a test on `tokio::time::pause` (or
/// `#[tokio::test(start_paused = true)]`) drives it. `spawn` panics outside a
/// tokio runtime, as `tokio::spawn` does.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRuntime;

impl Runtime for TokioRuntime {
    fn spawn(&self, task: BoxFuture<()>) {
        ::tokio::spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        Box::pin(::tokio::time::sleep(duration))
    }

    fn now(&self) -> Instant {
        ::tokio::time::Instant::now().into_std()
    }
}
