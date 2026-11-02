//! Timeouts, on an injected `Runtime`.

use std::future::{poll_fn, Future};
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

use crate::Runtime;

/// Await `future` for at most `duration` on `runtime`'s clock: its output, or
/// `None` when `duration` passed first.
///
/// `future` is polled before the timer, so a ready future wins even at
/// `Duration::ZERO`. Any duration, `Duration::MAX` included, is valid.
pub async fn timeout<F: Future>(
    runtime: &dyn Runtime,
    duration: Duration,
    future: F,
) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut deadline = runtime.sleep(duration);
    poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        deadline.as_mut().poll(cx).map(|()| None)
    })
    .await
}
