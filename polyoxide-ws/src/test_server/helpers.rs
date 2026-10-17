//! The helpers the offline suites share (W15).
//!
//! The basis is one socket crate's former `fast` and `next_event`
//! (`ws/supervised.rs`, deleted in Story 4.6), another socket crate's
//! (`tests/supervision.rs`) and a third's `next_item` and `drain`
//! (`tests/supervision.rs`).
//! Counting pings is [`ScriptedServer::ping_count`](super::ScriptedServer::ping_count)
//! for protocol pings and
//! [`ScriptedServer::count_frames`](super::ScriptedServer::count_frames) for
//! a protocol's own.

use std::{fmt::Debug, time::Duration};

use futures_util::{Stream, StreamExt};

use crate::Timing;

/// How long [`next_item`] and [`next_event`] wait. Generous, to stay steady
/// on a loaded machine.
pub const WINDOW: Duration = Duration::from_secs(3);

/// Test-sized timings: a ping every 50 ms, stale after 300 ms, and backoff
/// from 10 ms to 20 ms. The connect timeout and rotation are the defaults.
pub fn fast() -> Timing {
    Timing {
        ping_interval: Some(Duration::from_millis(50)),
        stale_after: Duration::from_millis(300),
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(20),
        ..Timing::default()
    }
}

/// The next item, which must arrive within [`WINDOW`]; panics if the stream
/// ends.
///
/// The deadline is checked first, so an item that arrives only because the
/// deadline woke the task, which is a lost wakeup, fails rather than passing
/// late.
pub async fn next_item<S: Stream + Unpin>(stream: &mut S) -> S::Item {
    let deadline = tokio::time::sleep(WINDOW);
    tokio::pin!(deadline);
    tokio::select! {
        biased;
        () = &mut deadline => panic!("no item within {WINDOW:?}"),
        item = stream.next() => item.expect("the stream ended"),
    }
}

/// The next item, which must be an `Ok` arriving within [`WINDOW`].
pub async fn next_event<S, T, E>(stream: &mut S) -> T
where
    S: Stream<Item = Result<T, E>> + Unpin,
    E: Debug,
{
    next_item(stream).await.expect("an event, not an error")
}

/// Read the stream until it ends, or until the task reading it is aborted,
/// so the server sees every reconnect.
pub async fn drain<S: Stream + Unpin>(mut stream: S) {
    while stream.next().await.is_some() {}
}
