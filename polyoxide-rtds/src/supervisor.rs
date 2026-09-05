//! Keep-alive, staleness detection, and reconnect.
//!
//! RTDS fails silently: it never answers a ping, sends no error when a
//! connection dies, and restores nothing server-side after a reconnect. A
//! half-open socket is therefore indistinguishable from a quiet market except
//! by timing the gap between updates, which is what [`SupervisedRtds`] does.

use std::{future::Future, time::Duration};

use futures_util::StreamExt;
use tokio::time::{timeout, Instant};

use crate::{
    client::{Rtds, RTDS_URL},
    error::{Recovery, RtdsError},
    event::PriceEvent,
    subscription::Subscription,
};

/// Default keep-alive cadence, matching the cadence upstream documents.
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(5);
/// Default silence before a connection is presumed dead.
///
/// Observed cadence is roughly one update per second per symbol per topic, so
/// this is about 30x headroom. Raise it for a thinly-traded filtered feed.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(30);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Builder for a supervised RTDS connection.
#[derive(Debug, Clone)]
pub struct RtdsBuilder {
    url: String,
    ping_interval: Duration,
    stale_after: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl Default for RtdsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RtdsBuilder {
    /// A builder with the default cadences.
    pub fn new() -> Self {
        Self {
            url: RTDS_URL.to_string(),
            ping_interval: DEFAULT_PING_INTERVAL,
            stale_after: DEFAULT_STALE_AFTER,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
        }
    }

    /// Point at a different endpoint. Used by tests.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// How long the stream may be idle before sending a keep-alive.
    ///
    /// Not an unconditional cadence: a ping is only sent after a full tick
    /// with no frames, so a healthy feed never sends one. That is fine —
    /// a subscription was observed running 240 seconds with no application
    /// ping at all, so the documented 5-second cadence is not load-bearing.
    ///
    /// Setting this larger than `stale_after` disables pings entirely, since
    /// the staleness check runs first on every idle tick.
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// How long the stream may be silent before it is presumed dead.
    ///
    /// This is the **only** liveness signal available: RTDS sends no reply to
    /// a ping, so a half-open socket cannot otherwise be distinguished from a
    /// quiet market.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect backoff bounds. The delay doubles from `initial` up to `max`.
    pub fn backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max;
        self
    }

    /// Connect and subscribe.
    pub async fn connect(
        self,
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<SupervisedRtds, RtdsError> {
        let subscriptions: Vec<_> = subscriptions.into_iter().collect();
        let stream = Rtds::connect_to(&self.url, subscriptions.clone()).await?;
        Ok(SupervisedRtds {
            config: self,
            subscriptions,
            stream,
            delivered: false,
        })
    }
}

/// A supervised RTDS connection that pings, detects stalls, and reconnects.
///
/// Because every resubscribe replays the backfill, callers see
/// [`PriceEvent::Snapshot`] again after each reconnect. That is the intended
/// way to re-initialise state.
pub struct SupervisedRtds {
    config: RtdsBuilder,
    subscriptions: Vec<Subscription>,
    stream: Rtds,
    /// Whether the current connection has yielded at least one event.
    ///
    /// Backoff resets on this, not on `connect_to` returning `Ok`. A server
    /// that accepts and immediately closes would otherwise pin the delay at
    /// its initial value forever, hammering a host that is already unwell.
    delivered: bool,
}

impl SupervisedRtds {
    /// Run until the handler errors or the subscription is rejected.
    ///
    /// Recoverable failures — drops and stalls — reconnect with backoff.
    /// Unrecoverable ones, chiefly [`RtdsError::Server`], return: retrying a
    /// rejected subscription replays the same rejection forever.
    ///
    /// Because every resubscribe replays the backfill, the handler sees
    /// [`PriceEvent::Snapshot`](crate::PriceEvent) again after each reconnect.
    /// That is how caller state re-initialises; it is not a duplicate.
    ///
    /// There is no stop method. To end the feed, drop the future or race it
    /// against your own shutdown signal — `tokio::select!` on `run` and a
    /// cancellation token is the usual shape. A handler returning `Err` also
    /// ends it, since any error that is not `Recovery::Reconnect` is terminal.
    pub async fn run<F, Fut>(mut self, mut handler: F) -> Result<(), RtdsError>
    where
        F: FnMut(PriceEvent) -> Fut,
        Fut: Future<Output = Result<(), RtdsError>>,
    {
        let mut backoff = self.config.initial_backoff;

        loop {
            self.delivered = false;
            match self.pump(&mut handler).await {
                // `pump`'s loop has no branch that returns `Ok(())` today —
                // every iteration either keeps going or returns `Err`. Kept
                // for forward compatibility (e.g. a documented clean-close
                // reason) so that adding one does not require touching this
                // match.
                Ok(()) => return Ok(()),
                // `pump` only ever returns Reconnect or Fatal errors —
                // SkipFrame ones are handled inside it, without dropping the
                // connection. See `Recovery` in error.rs.
                Err(err) if err.recovery() == Recovery::Reconnect => {
                    tracing::warn!(%err, ?backoff, "RTDS connection lost, reconnecting");
                    if self.delivered {
                        // The connection did real work before dying, so this
                        // is a fresh incident, not a continuation of a host
                        // that never worked in the first place.
                        backoff = self.config.initial_backoff;
                    }

                    // Retry the *connection attempt*, not the outer loop:
                    // `self.stream` is exhausted here and only the success
                    // arm below replaces it. `Rtds` is not `FusedStream`, so
                    // re-polling after it has yielded `None` is not
                    // contractually defined — and the socket is gone anyway.
                    // Do not "optimise" this into reuse.
                    loop {
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(self.config.max_backoff);

                        match Rtds::connect_to(&self.config.url, self.subscriptions.clone()).await {
                            Ok(stream) => {
                                self.stream = stream;
                                break;
                            }
                            Err(again) if again.recovery() == Recovery::Reconnect => {
                                tracing::warn!(
                                    %again,
                                    ?backoff,
                                    "RTDS reconnect attempt failed, retrying"
                                );
                            }
                            Err(fatal) => return Err(fatal),
                        }
                    }
                }
                Err(fatal) => return Err(fatal),
            }
        }
    }

    /// Drive one connection until it fails.
    ///
    /// Deliberately not a `tokio::select!` over `ping()` and `next()`:
    /// `select!` builds every branch future before polling, so those two
    /// branches would hold simultaneous mutable borrows of the stream and the
    /// function would not compile. Polling on a single tick also fixes a
    /// second problem — waiting `stale_after` for each read would delay stall
    /// detection whenever `ping_interval` is the shorter of the two.
    async fn pump<F, Fut>(&mut self, handler: &mut F) -> Result<(), RtdsError>
    where
        F: FnMut(PriceEvent) -> Fut,
        Fut: Future<Output = Result<(), RtdsError>>,
    {
        // Wake often enough to honour whichever deadline is nearer.
        let tick = self.config.ping_interval.min(self.config.stale_after);
        let mut last_frame = Instant::now();
        let mut last_ping = Instant::now();

        loop {
            match timeout(tick, self.stream.next()).await {
                // No frame this tick. Check for death first, then keep alive.
                Err(_) => {
                    let elapsed = last_frame.elapsed();
                    if elapsed >= self.config.stale_after {
                        return Err(RtdsError::Stalled { elapsed });
                    }
                    if last_ping.elapsed() >= self.config.ping_interval {
                        self.stream.ping().await?;
                        last_ping = Instant::now();
                    }
                }
                Ok(None) => return Err(RtdsError::ConnectionClosed),
                // A bad frame is not a bad connection. Surface it and keep
                // reading, or one unparseable message ends a 24/7 feed.
                Ok(Some(Err(err))) if err.recovery() == Recovery::SkipFrame => {
                    last_frame = Instant::now();
                    tracing::warn!(%err, "skipping an RTDS frame this client could not read");
                }
                Ok(Some(Err(err))) => return Err(err),
                Ok(Some(Ok(event))) => {
                    last_frame = Instant::now();
                    self.delivered = true;
                    handler(event).await?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_cadences() {
        let builder = RtdsBuilder::new();
        assert_eq!(builder.url, RTDS_URL);
        assert_eq!(builder.ping_interval, DEFAULT_PING_INTERVAL);
        assert_eq!(builder.stale_after, DEFAULT_STALE_AFTER);
        assert_eq!(builder.initial_backoff, DEFAULT_INITIAL_BACKOFF);
        assert_eq!(builder.max_backoff, DEFAULT_MAX_BACKOFF);
    }

    #[test]
    fn default_impl_matches_new() {
        let builder = RtdsBuilder::default();
        assert_eq!(builder.url, RtdsBuilder::new().url);
    }

    #[test]
    fn builder_setters_override_the_defaults() {
        let builder = RtdsBuilder::new()
            .url("ws://example.test")
            .ping_interval(Duration::from_secs(1))
            .stale_after(Duration::from_secs(2))
            .backoff(Duration::from_millis(1), Duration::from_millis(2));

        assert_eq!(builder.url, "ws://example.test");
        assert_eq!(builder.ping_interval, Duration::from_secs(1));
        assert_eq!(builder.stale_after, Duration::from_secs(2));
        assert_eq!(builder.initial_backoff, Duration::from_millis(1));
        assert_eq!(builder.max_backoff, Duration::from_millis(2));
    }
}
