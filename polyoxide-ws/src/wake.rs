//! The network wake: a handle that cuts a reconnect wait short (issue #93).

use std::{future::Future, sync::Arc};

use tokio::sync::Notify;

/// A handle that tells waiting supervisors the network is back, so each redials
/// at once instead of sitting out its backoff.
///
/// A page knows the moment its network returns: a browser fires `online` on
/// `Window` and `WorkerGlobalScope`. A socket that dropped during the blip
/// would otherwise wait out its backoff, up to its ceiling, after the network
/// has come back. Give one handle to each supervised builder's `wake(..)`, and
/// call [`notify`](Self::notify) from the `online` listener.
///
/// - **Only a wait in progress is cut.** A notify while connected, or while an
///   attempt is dialling, does nothing, and one that lands just before a wait
///   starts is missed: that wait runs its normal backoff.
/// - **The schedule starts again.** A supervisor that is woken redials at once
///   and resets its [`Backoff`](crate::Backoff) to the initial delay, since the
///   failures before the network changed say nothing about the next attempt.
/// - **A `Retry-After` is waited in full.** A wait that a refused attempt's
///   `Retry-After` asked for is not cut: the host was reachable and asked for
///   the time (DRIFT R11).
///
/// The handle is cheap to clone, and every clone fires the same wake. It is
/// target-neutral: nothing calls it on a host unless the caller does, and a
/// supervisor built without one behaves as before.
///
/// ```
/// use polyoxide_ws::NetworkWake;
///
/// let wake = NetworkWake::new();
/// // Handed to each builder, e.g. `.wake(wake.clone())`; then, from the
/// // page's `online` listener:
/// wake.notify();
/// ```
#[derive(Debug, Clone, Default)]
pub struct NetworkWake(Arc<Notify>);

impl NetworkWake {
    /// A wake that nothing has fired.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cut short every reconnect wait in progress on a supervisor holding a
    /// clone of this handle. A supervisor that is not waiting is not affected,
    /// and nothing is stored for a later wait.
    pub fn notify(&self) {
        self.0.notify_waiters();
    }

    /// A future that completes at the next [`notify`](Self::notify) after it is
    /// first polled. A supervisor races its backoff sleep against it.
    pub fn notified(&self) -> impl Future<Output = ()> + Send + 'static {
        let notify = Arc::clone(&self.0);
        async move { notify.notified().await }
    }

    /// Wait for `wait` to finish, or for a [`notify`](Self::notify), whichever
    /// comes first. Whether the wake ended it: `true` when it was cut short.
    pub async fn cut_short(&self, wait: impl Future<Output = ()>) -> bool {
        tokio::select! {
            biased;
            () = wait => false,
            () = self.notified() => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::{pending, ready},
        time::Duration,
    };

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_notify_cuts_a_wait_in_progress_short() {
        let wake = NetworkWake::new();
        let waiting = tokio::spawn({
            let wake = wake.clone();
            async move { wake.cut_short(pending()).await }
        });
        tokio::time::sleep(Duration::from_millis(1)).await;
        wake.notify();
        assert!(waiting.await.unwrap(), "the wait was not cut short");
    }

    #[tokio::test(start_paused = true)]
    async fn a_wait_that_finishes_first_is_not_reported_as_woken() {
        let wake = NetworkWake::new();
        assert!(!wake.cut_short(ready(())).await);
    }

    #[tokio::test(start_paused = true)]
    async fn a_notify_with_no_wait_in_progress_is_not_stored() {
        let wake = NetworkWake::new();
        wake.notify();
        let woken = wake
            .cut_short(tokio::time::sleep(Duration::from_secs(5)))
            .await;
        assert!(!woken, "a notify before the wait started cut it short");
    }

    #[tokio::test(start_paused = true)]
    async fn every_clone_fires_the_same_wake() {
        let wake = NetworkWake::new();
        let waiters: Vec<_> = (0..3)
            .map(|_| {
                let wake = wake.clone();
                tokio::spawn(async move { wake.cut_short(pending()).await })
            })
            .collect();
        tokio::time::sleep(Duration::from_millis(1)).await;
        wake.clone().notify();
        for waiter in waiters {
            assert!(waiter.await.unwrap());
        }
    }
}
