//! `spawn` and the `Task` it returns.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_channel::oneshot;
use futures_util::future::{AbortHandle, Abortable};

use crate::Runtime;

/// A spawned future, as a future of its output.
///
/// It resolves to `Some(output)`, or to `None` when the task was aborted or
/// its output never arrived (it panicked, or its runtime dropped it). Dropping
/// a `Task` detaches the task, as dropping tokio's `JoinHandle` does; only
/// [`Task::abort`] stops it.
pub struct Task<T> {
    abort: AbortHandle,
    output: oneshot::Receiver<T>,
    finished: Arc<AtomicBool>,
}

impl<T> Task<T> {
    /// Stop the task at its next suspension point. A task that has already
    /// ended is unaffected.
    pub fn abort(&self) {
        self.abort.abort();
    }

    /// Whether the task has ended: it completed, was aborted and dropped by its
    /// runtime, or panicked. A task just aborted is not finished until its
    /// runtime has dropped it.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

impl<T> Future for Task<T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        match Pin::new(&mut self.output).poll(cx) {
            Poll::Ready(output) => {
                self.finished.store(true, Ordering::Release);
                Poll::Ready(output.ok())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> fmt::Debug for Task<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

/// Sets the task's flag when dropped, so a panic, an abort and a runtime that
/// drops the task unpolled all end it.
struct Finish(Arc<AtomicBool>);

impl Drop for Finish {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Spawn `future` on `runtime`, which needs only its detached spawn.
pub fn spawn<T, F>(runtime: &dyn Runtime, future: F) -> Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (output_tx, output_rx) = oneshot::channel();
    let (abort, registration) = AbortHandle::new_pair();
    let finished = Arc::new(AtomicBool::new(false));
    let finish = Finish(Arc::clone(&finished));
    let run = Abortable::new(
        async move {
            let value = future.await;
            // Flag the end before the output is visible, so a reader woken by
            // the output sees the task finished.
            drop(finish);
            let _ = output_tx.send(value);
        },
        registration,
    );
    runtime.spawn(Box::pin(async move {
        let _ = run.await;
    }));
    Task {
        abort,
        output: output_rx,
        finished,
    }
}
