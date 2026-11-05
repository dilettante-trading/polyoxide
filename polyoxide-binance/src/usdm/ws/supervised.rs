//! The supervised tier: one kit `Supervisor` per routed path, with keep-alive
//! on the wall clock, staleness detection, reconnect with a paced replay of
//! the path's streams, rotation before Binance's 24-hour cutoff and outage
//! markers, behind a router that sends each membership change to its path's
//! connection, opens a path for its first stream, and merges every path's
//! events into one stream.

#[cfg(test)]
use std::time::Duration;
use std::{
    fmt,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures_util::Stream;
use polyoxide_rt::Runtime;
use polyoxide_ws::{SocketTransport, Timing};

use crate::{
    shared::ws::{
        supervised::{self as shared_supervised, Handle, Running, Settings},
        DEFAULT_MAX_CONNECTION_AGE,
    },
    usdm::ws::{
        error::UsdmWsError, event::Update, protocol::UsdmStreams, stream::StreamName, StreamPath,
        USDM_WS_BASE,
    },
};

/// What the supervised stream yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// One frame of one stream, boxed, as the other variants are small.
    Update(Box<Update>),
    /// A path's connection was lost. Every update on that path is stale until
    /// the matching [`Event::Reconnected`], which always follows while the
    /// client runs.
    Disconnected {
        /// The path that went down.
        path: StreamPath,
        /// Why.
        reason: DisconnectReason,
    },
    /// The path is current again: a fresh connection has replayed its
    /// streams, or the path's last stream left during the outage, so nothing
    /// on it is stale. Sent once per outage, however many attempts it took.
    /// Drop book state for the path and resync.
    Reconnected {
        /// The path that came back.
        path: StreamPath,
    },
}

/// Why a path's connection went down.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed it, with the code and reason it sent, or ended the
    /// stream with neither.
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },
    /// A transport error, a replay left unanswered for the staleness window,
    /// or a failed first connect of a path opened by a membership change.
    Error(UsdmWsError),
    /// Nothing arrived, pongs included, for the staleness window, or a
    /// membership change went unanswered for it.
    Stale,
    /// Replaced on purpose before Binance's 24-hour cutoff.
    Rotation,
}

impl fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed { code, reason } => match code {
                Some(code) => write!(f, "closed by the server ({code} {reason})"),
                None => f.write_str("closed by the server"),
            },
            Self::Error(err) => write!(f, "{err}"),
            Self::Stale => f.write_str("no frame within the staleness window"),
            Self::Rotation => f.write_str("rotated before the 24-hour cutoff"),
        }
    }
}

/// Builder for a supervised connection.
///
/// Each routed path's connection runs on its own task, on the socket kit's
/// `Supervisor`, and logs under the `polyoxide_ws` target.
///
/// While the consumer leaves 1024 events unread, the paths stop reading but
/// keep pinging, and that wait is not silence: a slow consumer is never
/// reported [`Stale`](DisconnectReason::Stale), and no event is dropped. The
/// server's own pings go unanswered meanwhile. Binance documents that it
/// closes a connection that has not answered them for 10 minutes; that close
/// is reconnected like any other, and the events already queued are kept.
#[derive(Debug, Clone)]
pub struct UsdmWsBuilder {
    base_url: String,
    timing: Timing,
    streams: Vec<StreamName>,
    runtime: Option<Arc<dyn Runtime>>,
    socket_transport: Option<Arc<dyn SocketTransport>>,
}

impl Default for UsdmWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdmWsBuilder {
    /// Defaults: the production host, a ping every 20 s, stale after 30 s of
    /// silence, reconnect backoff from 500 ms doubling to 60 s, a 10 s connect
    /// timeout, rotation after 23 h 50 min, and no streams.
    pub fn new() -> Self {
        let mut timing = Timing::default();
        timing.max_connection_age = Some(DEFAULT_MAX_CONNECTION_AGE);
        Self {
            base_url: USDM_WS_BASE.to_owned(),
            timing,
            streams: Vec::new(),
            runtime: None,
            socket_transport: None,
        }
    }

    /// Connect to another host, such as a local test server.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    polyoxide_ws::timing_setters! {
        /// How often each connection sends a protocol ping, whatever its
        /// traffic, and also while the consumer leaves 1024 events unread.
        /// None goes out while a membership change is in flight, which
        /// `stale_after` bounds. Binance closes a connection that sends more
        /// than 10 messages a second.
        ping_interval;
        /// Silence after which a connection is presumed dead and replaced.
        /// Pongs and the server's pings count, so a quiet connection that
        /// answers stays up, and time the consumer leaves 1024 events unread
        /// does not count.
        ///
        /// Also bounds each replay and each membership change as a whole, its
        /// 200 ms pacing included: one the server leaves unanswered that long
        /// replaces the connection. A replay of 1024 streams is six requests,
        /// about a second.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max`, with no cap
        /// on attempts, and back to `initial` after a connection that
        /// delivered an update. Both are raised to at least 1 ms, and an
        /// `initial` above `max` is lowered to `max`. A reconnect refused with
        /// a `Retry-After` waits at least that long, up to `max` (DRIFT R11).
        backoff;
        /// How long one connection attempt may take to open.
        connect_timeout;
        /// How long a connection may live before it is replaced, without
        /// backoff, ahead of Binance's 24-hour cutoff. Each replacement is a
        /// new connection, and Binance allows 300 connection attempts per 5
        /// minutes per IP.
        max_connection_age;
    }

    polyoxide_ws::transport_setters! {
        /// The runtime the paths' supervision tasks, the router and its
        /// forwarders run on. By default the target's.
        runtime;
        /// How each path's connection dials. By default the target's.
        socket_transport;
    }

    /// The streams to open with.
    pub fn streams(mut self, streams: impl IntoIterator<Item = StreamName>) -> Self {
        self.streams = streams.into_iter().collect();
        self
    }

    /// Open a connection for each path the streams need, subscribe, and start
    /// supervising.
    ///
    /// Fails if any first connect fails, closing the others: a setup that is
    /// wrong is reported, not retried. With no streams it opens nothing until
    /// the first [`MembershipHandle::subscribe`].
    pub async fn connect(self) -> Result<SupervisedUsdmWs, UsdmWsError> {
        let inner = shared_supervised::connect::<UsdmStreams>(Settings {
            base_url: self.base_url,
            timing: self.timing,
            streams: self.streams,
            runtime: self.runtime,
            socket_transport: self.socket_transport,
        })
        .await?;
        Ok(SupervisedUsdmWs { inner })
    }
}

/// Changes the streams of a running [`SupervisedUsdmWs`].
///
/// Each stream is routed to its path's connection, opening it if this is the
/// path's first stream and closing it when its last leaves. The membership is
/// a set: subscribing a stream twice holds it once, and counting references is
/// the caller's job.
///
/// A change is recorded before it is sent. During a path's outage it is
/// answered `Ok` at once, and the replay applies it; so is a change whose
/// connection fails under it, or one the server leaves unanswered for
/// `stale_after`, which replaces the connection. A change that arrives while
/// a connect or a replay is in flight waits for that attempt. Calls are served
/// one at a time across paths, so a change for a healthy path can wait behind
/// another path's attempt.
///
/// A call fails when the server refuses it ([`UsdmWsError::Refused`]), when it
/// would pass 1024 streams on a path ([`UsdmWsError::TooManyStreams`], nothing
/// sent), when the first connect of a path it opens fails in a way retrying
/// cannot fix ([`UsdmWsError::Connect`]), or when the client has stopped
/// ([`UsdmWsError::Stopped`]). Its `cause` is the error a path stopped on once
/// the router has that path's stop; a call in flight when the path stops can
/// get `Stopped { cause: None }`. A subscription of more than 200 names is sent
/// in batches, so one refused part-way leaves the earlier batches applied.
///
/// A call waits for the path's task, which takes no change while the consumer
/// leaves 1024 events unread. Make membership changes from a task other than
/// the one draining the stream, or keep draining while they run.
#[derive(Debug, Clone)]
pub struct MembershipHandle {
    inner: Handle<UsdmStreams>,
}

impl MembershipHandle {
    /// Subscribe to more streams.
    pub async fn subscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.inner.subscribe(streams.into_iter().collect()).await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub async fn unsubscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.inner.unsubscribe(streams.into_iter().collect()).await
    }
}

/// A supervised set of connections: a `Stream` of [`Event`]s that survives
/// drops, stalls and Binance's 24-hour cutoff.
///
/// Every [`Event::Disconnected`] is followed by an [`Event::Reconnected`] for
/// the same path while the client runs. A path that closes because its last
/// stream left, while it is up, yields neither.
///
/// A fatal error on any path, such as the server refusing a replay after a
/// reconnect, is yielded as `Err` and then the stream ends, every path
/// closed. An `Err(UsdmWsError::Frame { .. })`, a frame that did not decode,
/// does not end it.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::usdm::{types::Symbol, ws::{Event, StreamName, UsdmWsBuilder, UsdmWsError}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut feed = UsdmWsBuilder::new()
///     .streams([StreamName::MarkPrice(btc.clone()), StreamName::BookTicker(btc)])
///     .connect()
///     .await?;
/// let membership = feed.membership();
/// while let Some(event) = feed.next().await {
///     let event = match event {
///         Ok(event) => event,
///         // A frame that did not decode is skipped; anything else ends the stream.
///         Err(UsdmWsError::Frame { .. }) => continue,
///         Err(err) => return Err(err.into()),
///     };
///     match event {
///         Event::Update(update) => println!("{}", serde_json::to_string(&update)?),
///         Event::Disconnected { path, reason } => eprintln!("{path} down: {reason}"),
///         Event::Reconnected { path } => eprintln!("{path} back"),
///         _ => {}
///     }
/// }
/// # membership.subscribe([StreamName::AllMarkPrices]).await?;
/// # Ok(())
/// # }
/// ```
pub struct SupervisedUsdmWs {
    inner: Running<UsdmStreams>,
}

impl fmt::Debug for SupervisedUsdmWs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupervisedUsdmWs")
            .field("running", &self.inner.is_running())
            .finish_non_exhaustive()
    }
}

impl SupervisedUsdmWs {
    /// A handle for changing streams while the stream runs.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            inner: self.inner.membership(),
        }
    }

    /// Close every connection and stop. Every [`MembershipHandle`] answers
    /// [`UsdmWsError::Stopped`] afterwards.
    ///
    /// Fails with [`UsdmWsError::Stopped`] if a path had stopped the client
    /// on an error, which the stream yielded: its `cause` is that error, as
    /// [`Classify`](polyoxide_venue::Classify) read it, and so is the cause
    /// every later membership call carries. That holds once the router has the
    /// path's stop: a `close()` made right after the stream yielded the
    /// stopping error can return `Ok` or `Stopped { cause: None }`. Read the
    /// stream to its end first to be sure of the cause.
    pub async fn close(self) -> Result<(), UsdmWsError> {
        self.inner.close().await
    }
}

impl Stream for SupervisedUsdmWs {
    type Item = Result<Event, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_event(cx)
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_ws::Backoff;

    use super::*;

    #[test]
    fn defaults_match_the_documented_cadences() {
        let b = UsdmWsBuilder::new();
        assert_eq!(b.base_url, USDM_WS_BASE);
        assert_eq!(b.timing.ping_interval, Some(Duration::from_secs(20)));
        assert_eq!(b.timing.stale_after, Duration::from_secs(30));
        assert_eq!(b.timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(b.timing.max_backoff, Duration::from_secs(60));
        assert_eq!(b.timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(
            b.timing.max_connection_age,
            Some(Duration::from_secs(85_800))
        );
        assert!(b.timing.max_connection_age < Some(Duration::from_secs(24 * 3600)));
    }

    #[test]
    fn backoff_doubles_to_the_ceiling_and_resets_after_a_working_connection() {
        let mut b = Backoff::new(Duration::from_millis(100), Duration::from_millis(350));
        assert_eq!(b.take(), Duration::from_millis(100));
        assert_eq!(b.take(), Duration::from_millis(200));
        assert_eq!(b.take(), Duration::from_millis(350));
        b.after_connection_ended(true);
        assert_eq!(b.take(), Duration::from_millis(100));
        b.after_connection_ended(false);
        assert_eq!(b.take(), Duration::from_millis(200));
    }

    #[test]
    fn a_zero_initial_delay_still_grows_and_a_huge_one_saturates() {
        let mut b = Backoff::new(Duration::ZERO, Duration::from_secs(1));
        assert_eq!(b.take(), Duration::from_millis(1));
        assert_eq!(b.take(), Duration::from_millis(2));
        let mut b = Backoff::new(Duration::MAX, Duration::MAX);
        assert_eq!(b.take(), Duration::MAX);
        assert_eq!(b.take(), Duration::MAX);
    }

    /// A runtime that counts its spawns and runs them on tokio's.
    #[derive(Debug, Default)]
    struct CountingRuntime {
        inner: polyoxide_rt::TokioRuntime,
        spawns: std::sync::atomic::AtomicUsize,
    }

    impl Runtime for CountingRuntime {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.spawns
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            self.inner.sleep(duration)
        }

        fn yield_now(&self) -> polyoxide_rt::BoxFuture<()> {
            Box::pin(tokio::task::yield_now())
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.inner.now()
        }
    }

    #[tokio::test]
    async fn the_router_and_each_path_are_spawned_on_the_injected_runtime() {
        use crate::usdm::ws::scripted::{Script, ScriptedServer};

        let server = ScriptedServer::start(vec![Script::default()]).await;
        let runtime = std::sync::Arc::new(CountingRuntime::default());
        let btc = crate::usdm::types::Symbol::new("BTCUSDT").unwrap();
        let feed = UsdmWsBuilder::new()
            .base_url(server.url.clone())
            .runtime(runtime.clone())
            .streams([StreamName::MarkPrice(btc)])
            .connect()
            .await
            .expect("connect");
        // One spawn for the path's supervision task, one for its forwarder and
        // one for the router.
        assert_eq!(runtime.spawns.load(std::sync::atomic::Ordering::SeqCst), 3);
        feed.close().await.expect("close");
    }
}
