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

/// The reconnect delay schedule.
///
/// Split out from [`SupervisedRtds::run`] deliberately. As three statements
/// interleaved with a `sleep` and a live socket, the doubling and the ceiling
/// could only be checked by timing a real reconnect — so neither was checked
/// at all, and a schedule pinned at its initial value would have hammered an
/// unwell host indefinitely without failing a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Backoff {
    initial: Duration,
    max: Duration,
    next: Duration,
}

impl Backoff {
    /// A schedule starting at `initial` and doubling towards `max`.
    ///
    /// `initial` is clamped to `max`: [`RtdsBuilder::backoff`] takes the two
    /// independently and nothing stops a caller passing them the wrong way
    /// round, which would otherwise wait the larger delay once before the
    /// ceiling took effect.
    fn new(initial: Duration, max: Duration) -> Self {
        let initial = initial.min(max);
        Self {
            initial,
            max,
            next: initial,
        }
    }

    /// The delay to wait before the next attempt, advancing the schedule.
    fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(self.max);
        delay
    }

    /// Note that a connection ended, and whether it had done any real work.
    ///
    /// Resetting on `delivered` rather than on a successful connect is what
    /// stops a host that accepts and immediately closes from pinning the delay
    /// at its initial value forever. See [`SupervisedRtds::delivered`] for what
    /// this does and does not cover.
    fn after_connection_ended(&mut self, delivered: bool) {
        if delivered {
            self.next = self.initial;
        }
    }
}

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
/// A resubscribe replays the backfill, so callers see [`PriceEvent::Snapshot`]
/// again after each reconnect — that is the intended way to re-initialise
/// state. It applies only to symbol-filtered subscriptions: an unfiltered one
/// receives no backfill at all, so it has nothing to re-initialise from.
pub struct SupervisedRtds {
    config: RtdsBuilder,
    subscriptions: Vec<Subscription>,
    stream: Rtds,
    /// Whether the current connection has yielded at least one event.
    ///
    /// Backoff resets on this, not on `connect_to` returning `Ok`. A server
    /// that accepts and immediately closes would otherwise pin the delay at
    /// its initial value forever, hammering a host that is already unwell.
    ///
    /// This narrows that failure rather than eliminating it. A host that
    /// reliably delivers *exactly one* frame before dying still resets the
    /// backoff every cycle and never escalates. Most unhealthy hosts reject
    /// before delivering anything — an immediate close, a proxy error, a
    /// rejected subscription — and those are handled. Closing the remaining
    /// case needs a stronger signal than a boolean: a minimum connection
    /// lifetime, or a failure count that decays rather than resetting. Worth
    /// doing only if a real host is ever observed behaving that way.
    delivered: bool,
}

impl SupervisedRtds {
    /// Run until the handler errors or the subscription is rejected.
    ///
    /// Recoverable failures — drops and stalls — reconnect with backoff.
    /// Unrecoverable ones, chiefly [`RtdsError::Server`], return: retrying a
    /// rejected subscription replays the same rejection forever.
    ///
    /// A resubscribe replays the backfill, so the handler sees
    /// [`PriceEvent::Snapshot`](crate::PriceEvent) again after each reconnect.
    /// That is how caller state re-initialises; it is not a duplicate.
    ///
    /// This only applies to symbol-filtered subscriptions. An unfiltered
    /// subscription receives no backfill on connect or reconnect, verified
    /// against all three topic families, so such a handler never sees a
    /// snapshot at all and must rebuild its own state from updates.
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
        let mut backoff = Backoff::new(self.config.initial_backoff, self.config.max_backoff);

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
                    // A connection that did real work before dying is a fresh
                    // incident, not a continuation of a host that never worked
                    // in the first place.
                    backoff.after_connection_ended(self.delivered);
                    tracing::warn!(%err, "RTDS connection lost, reconnecting");

                    // Retry the *connection attempt*, not the outer loop:
                    // `self.stream` is exhausted here and only the success
                    // arm below replaces it. `Rtds` is not `FusedStream`, so
                    // re-polling after it has yielded `None` is not
                    // contractually defined — and the socket is gone anyway.
                    // Do not "optimise" this into reuse.
                    loop {
                        // Logged rather than merely slept: a delay handed to
                        // `sleep` is otherwise unobservable, and the previous
                        // spelling reported the wrong number twice over — the
                        // value before the reset, then the already-doubled one
                        // on a retry.
                        let delay = backoff.take();
                        tracing::debug!(?delay, "waiting before the next RTDS connection attempt");
                        tokio::time::sleep(delay).await;

                        match Rtds::connect_to(&self.config.url, self.subscriptions.clone()).await {
                            Ok(stream) => {
                                self.stream = stream;
                                break;
                            }
                            Err(again) if again.recovery() == Recovery::Reconnect => {
                                tracing::warn!(%again, "RTDS reconnect attempt failed, retrying");
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

    /// The schedule as arithmetic, with no clock and no socket involved.
    mod backoff_schedule {
        use super::*;

        fn millis(ms: u64) -> Duration {
            Duration::from_millis(ms)
        }

        fn take_n(backoff: &mut Backoff, n: usize) -> Vec<Duration> {
            (0..n).map(|_| backoff.take()).collect()
        }

        #[test]
        fn the_delay_doubles_up_to_the_ceiling_and_then_holds() {
            let mut backoff = Backoff::new(millis(10), millis(80));
            assert_eq!(
                take_n(&mut backoff, 6),
                vec![
                    millis(10),
                    millis(20),
                    millis(40),
                    millis(80),
                    millis(80),
                    millis(80)
                ],
                "a schedule that stops escalating hammers a host that is \
                 already unwell"
            );
        }

        #[test]
        fn the_ceiling_is_a_clamp_rather_than_a_step() {
            // 30 doubles to 60, which is past 50 without ever equalling it. A
            // ceiling implemented as an equality check would run away here.
            let mut backoff = Backoff::new(millis(30), millis(50));
            assert_eq!(
                take_n(&mut backoff, 3),
                vec![millis(30), millis(50), millis(50)]
            );
        }

        #[test]
        fn a_reset_returns_to_the_initial_delay_not_to_zero() {
            let mut backoff = Backoff::new(millis(10), millis(80));
            take_n(&mut backoff, 3);
            backoff.after_connection_ended(true);
            assert_eq!(
                take_n(&mut backoff, 2),
                vec![millis(10), millis(20)],
                "a reset restarts the schedule; it does not remove the wait"
            );
        }

        #[test]
        fn only_a_connection_that_delivered_something_resets_the_schedule() {
            // The distinction the `delivered` flag exists for. A host that
            // accepts and immediately closes must keep escalating.
            let mut escalating = Backoff::new(millis(10), millis(80));
            let mut recovering = Backoff::new(millis(10), millis(80));
            let (mut escalated, mut recovered) = (Vec::new(), Vec::new());

            for _ in 0..4 {
                escalating.after_connection_ended(false);
                escalated.push(escalating.take());
                recovering.after_connection_ended(true);
                recovered.push(recovering.take());
            }

            assert_eq!(
                escalated,
                vec![millis(10), millis(20), millis(40), millis(80)]
            );
            assert_eq!(
                recovered,
                vec![millis(10), millis(10), millis(10), millis(10)]
            );
        }

        #[test]
        fn a_ceiling_below_the_initial_delay_is_still_honoured() {
            // `backoff(initial, max)` takes the two independently, so nothing
            // stops a caller swapping them. Without the clamp the first wait
            // would be a minute regardless of the ceiling.
            let mut backoff = Backoff::new(Duration::from_secs(60), millis(1));
            assert_eq!(take_n(&mut backoff, 2), vec![millis(1), millis(1)]);

            backoff.after_connection_ended(true);
            assert_eq!(backoff.take(), millis(1), "a reset must respect it too");
        }
    }

    /// The parts of supervision that only exist because RTDS fails silently:
    /// the delay actually waited between attempts, and the keep-alive.
    mod against_a_local_server {
        use super::*;
        use crate::{
            fixtures,
            test_log::capture_async,
            test_server::{Script, ScriptedServer},
            topic::{Topic, TwapWindow},
        };

        const INITIAL: Duration = Duration::from_millis(10);
        const MAX: Duration = Duration::from_millis(80);

        fn subs() -> Vec<Subscription> {
            Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbols(["btc/usd"])
        }

        /// Run against `scripts` until it returns, collecting the delays it
        /// waited between connection attempts.
        ///
        /// Every script list must end in a frame that ends the run, so the
        /// delays observed are a complete sequence rather than however many
        /// happened to fit inside a timeout.
        async fn delays_while_running(scripts: Vec<Script>) -> (Vec<String>, usize) {
            let server = ScriptedServer::start(scripts).await;
            let supervised = RtdsBuilder::new()
                .url(&server.url)
                .stale_after(Duration::from_secs(60))
                .backoff(INITIAL, MAX)
                .connect(subs())
                .await
                .expect("connect");

            let (outcome, logs) = capture_async(async {
                tokio::time::timeout(
                    Duration::from_secs(5),
                    supervised.run(|_event| async move { Ok(()) }),
                )
                .await
                .expect("the run must end on the scripted rejection, not a timeout")
            })
            .await;

            match outcome {
                Err(RtdsError::Server { .. }) => {}
                other => panic!("expected the run to end on the rejection, got {other:?}"),
            }
            (logs.field_values("delay"), server.connection_count())
        }

        #[tokio::test]
        async fn a_host_that_never_delivers_anything_is_backed_off_further_each_time() {
            // Accept-and-close, three times over. Nothing is ever delivered, so
            // the schedule must keep escalating rather than restarting.
            let (delays, connections) = delays_while_running(vec![
                Script::SendThenClose(Vec::new()),
                Script::SendThenClose(Vec::new()),
                Script::SendThenClose(Vec::new()),
                Script::SendThenIdle(vec![fixtures::REJECTED_SUBSCRIPTION.into()]),
            ])
            .await;

            assert_eq!(
                delays,
                vec!["10ms", "20ms", "40ms"],
                "a host that never works must not hold the delay at its initial value"
            );
            assert_eq!(connections, 4);
        }

        #[tokio::test]
        async fn a_host_that_delivers_before_dying_starts_the_schedule_over() {
            // The mirror image: each connection does real work before dying,
            // so each drop is a fresh incident and the delay must not creep up.
            let update = fixtures::TWAP_THIRTY_UPDATE.to_string();
            let (delays, connections) = delays_while_running(vec![
                Script::SendThenClose(vec![update.clone()]),
                Script::SendThenClose(vec![update.clone()]),
                Script::SendThenClose(vec![update]),
                Script::SendThenIdle(vec![fixtures::REJECTED_SUBSCRIPTION.into()]),
            ])
            .await;

            assert_eq!(
                delays,
                vec!["10ms", "10ms", "10ms"],
                "a connection that delivered events must reset the schedule"
            );
            assert_eq!(connections, 4);
        }

        #[tokio::test]
        async fn a_refused_connection_attempt_is_retried_rather_than_surfaced() {
            // The reconnect loop's own retry arm. The first connection works
            // and then drops, resetting the schedule; the next two attempts are
            // refused before the WebSocket handshake completes and must be
            // retried, escalating, rather than ending the run.
            let (delays, connections) = delays_while_running(vec![
                Script::SendThenClose(vec![fixtures::TWAP_THIRTY_UPDATE.into()]),
                Script::RejectHandshake,
                Script::RejectHandshake,
                Script::SendThenIdle(vec![fixtures::REJECTED_SUBSCRIPTION.into()]),
            ])
            .await;

            assert_eq!(
                delays,
                vec!["10ms", "20ms", "40ms"],
                "a refused attempt must escalate the wait, not restart it"
            );
            assert_eq!(
                connections, 4,
                "the two refused attempts must be retried, not returned"
            );
        }

        #[tokio::test]
        async fn the_keepalive_is_sent_once_the_stream_goes_quiet() {
            // Nothing else in the suite reaches `pump`'s ping branch: every
            // other test either has frames arriving or trips the staleness
            // timer first.
            let server = ScriptedServer::start(vec![Script::SendThenIdle(Vec::new())]).await;
            let supervised = RtdsBuilder::new()
                .url(&server.url)
                .ping_interval(Duration::from_millis(20))
                .stale_after(Duration::from_secs(60))
                .backoff(INITIAL, MAX)
                .connect(subs())
                .await
                .expect("connect");

            let running = tokio::spawn(supervised.run(|_event| async move { Ok(()) }));
            server
                .wait_for("a PING frame", |s| {
                    s.client_frames().iter().any(|f| f == "PING")
                })
                .await;

            assert_eq!(
                server.connection_count(),
                1,
                "the keep-alive must not have come from a reconnect"
            );
            running.abort();
        }

        #[tokio::test]
        async fn a_ping_interval_past_the_staleness_window_disables_pings() {
            // Documented on `RtdsBuilder::ping_interval`: the staleness check
            // runs first on every idle tick, so a ping that is never due before
            // the connection is declared dead never goes out at all.
            let server = ScriptedServer::start(vec![
                Script::SendThenIdle(Vec::new()),
                Script::SendThenIdle(vec![fixtures::REJECTED_SUBSCRIPTION.into()]),
            ])
            .await;
            let supervised = RtdsBuilder::new()
                .url(&server.url)
                .ping_interval(Duration::from_secs(10))
                .stale_after(Duration::from_millis(60))
                .backoff(INITIAL, MAX)
                .connect(subs())
                .await
                .expect("connect");

            let outcome = tokio::time::timeout(
                Duration::from_secs(5),
                supervised.run(|_event| async move { Ok(()) }),
            )
            .await
            .expect("the staleness watchdog must fire and the run must then end");
            assert!(outcome.is_err());

            assert!(
                server.connection_count() >= 2,
                "the stall must have forced a reconnect, saw {}",
                server.connection_count()
            );
            assert!(
                !server.client_frames().iter().any(|f| f == "PING"),
                "no keep-alive may go out when it is never due before the \
                 staleness window: {:?}",
                server.client_frames()
            );
        }
    }
}
