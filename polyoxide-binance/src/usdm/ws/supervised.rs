//! The supervised tier: one kit `Supervisor` per routed path, with keep-alive
//! on the wall clock, staleness detection, reconnect with a paced replay of
//! the path's streams, rotation before Binance's 24-hour cutoff and outage
//! markers, behind a router that sends each membership change to its path's
//! connection, opens a path for its first stream, and merges every path's
//! events into one stream.

use std::{
    collections::HashMap,
    fmt,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_ws::{MembershipHandle as PathHandle, Supervised, Supervisor, Timing};
use tokio::sync::{mpsc, oneshot, watch};

use crate::usdm::ws::{
    error::UsdmWsError,
    event::Update,
    protocol::{Change, UsdmProtocol},
    stream::StreamName,
    StreamPath, MAX_STREAMS_PER_CONNECTION, USDM_WS_BASE,
};

/// Ten minutes under Binance's documented 24-hour connection lifetime.
const DEFAULT_MAX_CONNECTION_AGE: Duration = Duration::from_secs(23 * 3600 + 50 * 60);
/// Events the consumer may leave unread, across both paths, before the paths
/// stop reading.
const EVENT_BUFFER: usize = 1024;

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
        /// `initial` above `max` is lowered to `max`.
        backoff;
        /// How long one connection attempt may take to open.
        connect_timeout;
        /// How long a connection may live before it is replaced, without
        /// backoff, ahead of Binance's 24-hour cutoff. Each replacement is a
        /// new connection, and Binance allows 300 connection attempts per 5
        /// minutes per IP.
        max_connection_age;
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
        let mut membership: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
        for stream in self.streams.iter().cloned() {
            let wanted = membership.entry(stream.path()).or_default();
            if !wanted.contains(&stream) {
                wanted.push(stream);
            }
        }
        for (path, wanted) in &membership {
            if wanted.len() > MAX_STREAMS_PER_CONNECTION {
                return Err(UsdmWsError::TooManyStreams {
                    path: *path,
                    limit: MAX_STREAMS_PER_CONNECTION,
                });
            }
        }
        let mut opened: Vec<(StreamPath, Supervised<UsdmProtocol>)> = Vec::new();
        for path in StreamPath::ALL {
            let Some(wanted) = membership.get(path) else {
                continue;
            };
            match self.supervisor(*path).connect(wanted.clone()).await {
                Ok(supervised) => opened.push((*path, supervised)),
                Err(err) => {
                    for (_, supervised) in opened {
                        let _ = supervised.close().await;
                    }
                    return Err(err);
                }
            }
        }
        let (events, events_rx) = mpsc::channel(EVENT_BUFFER);
        let (commands, commands_rx) = mpsc::channel(16);
        let (ended, ended_rx) = mpsc::unbounded_channel();
        let (closing, _) = watch::channel(false);
        let mut router = Router {
            config: self,
            paths: HashMap::new(),
            membership,
            events,
            closing,
            ended,
            forwarders: Vec::new(),
            generation: 0,
        };
        for (path, supervised) in opened {
            router.adopt(path, supervised);
        }
        let task = tokio::spawn(route(router, commands_rx, ended_rx));
        Ok(SupervisedUsdmWs {
            events: events_rx,
            commands,
            task,
        })
    }

    /// The supervisor for one path's connection.
    fn supervisor(&self, path: StreamPath) -> Supervisor<UsdmProtocol> {
        Supervisor::new(UsdmProtocol::new(self.base_url.clone(), path), self.timing)
    }
}

type Reply = oneshot::Sender<Result<(), UsdmWsError>>;

enum Command {
    Subscribe(Vec<StreamName>, Reply),
    Unsubscribe(Vec<StreamName>, Reply),
    /// Close every socket and end. Sent by [`SupervisedUsdmWs::close`]; a
    /// dropped sender is not enough, because every [`MembershipHandle`] holds
    /// a clone.
    Close,
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
/// ([`UsdmWsError::Stopped`]). A subscription of more than 200 names is sent
/// in batches, so one refused part-way leaves the earlier batches applied.
///
/// A call waits for the path's task, which takes no change while the consumer
/// leaves 1024 events unread. Make membership changes from a task other than
/// the one draining the stream, or keep draining while they run.
#[derive(Debug, Clone)]
pub struct MembershipHandle {
    commands: mpsc::Sender<Command>,
}

impl MembershipHandle {
    /// Subscribe to more streams.
    pub async fn subscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.send(|reply| Command::Subscribe(streams.into_iter().collect(), reply))
            .await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub async fn unsubscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.send(|reply| Command::Unsubscribe(streams.into_iter().collect(), reply))
            .await
    }

    async fn send(&self, make: impl FnOnce(Reply) -> Command) -> Result<(), UsdmWsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.commands
            .send(make(reply_tx))
            .await
            .map_err(|_| UsdmWsError::Stopped)?;
        reply_rx.await.map_err(|_| UsdmWsError::Stopped)?
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
    events: mpsc::Receiver<Item>,
    commands: mpsc::Sender<Command>,
    task: tokio::task::JoinHandle<Result<(), UsdmWsError>>,
}

impl fmt::Debug for SupervisedUsdmWs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupervisedUsdmWs")
            .field("running", &!self.task.is_finished())
            .finish_non_exhaustive()
    }
}

impl SupervisedUsdmWs {
    /// A handle for changing streams while the stream runs.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            commands: self.commands.clone(),
        }
    }

    /// Close every connection and stop. Every [`MembershipHandle`] answers
    /// [`UsdmWsError::Stopped`] afterwards.
    pub async fn close(self) -> Result<(), UsdmWsError> {
        let Self {
            events,
            commands,
            task,
        } = self;
        // Dropping the receiver first unblocks a path parked behind a full
        // queue, which would otherwise never see the command.
        drop(events);
        let _ = commands.send(Command::Close).await;
        drop(commands);
        match task.await {
            Ok(result) => result,
            Err(_) => Err(UsdmWsError::Stopped),
        }
    }
}

impl Stream for SupervisedUsdmWs {
    type Item = Result<Event, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.events.poll_recv(cx)
    }
}

type Item = Result<Event, UsdmWsError>;
type Events = mpsc::Sender<Item>;

/// A path's supervised connection, as the router holds it.
struct Path {
    /// Which of the path's supervisors this is; a path closed for want of
    /// streams may still be draining when the next opens.
    generation: u64,
    handle: PathHandle<UsdmProtocol>,
}

/// A path's stream that ended.
struct Ended {
    path: StreamPath,
    generation: u64,
    /// Whether it stopped on an error, rather than because its streams left.
    stopped: bool,
}

/// The router: routes membership changes to the paths, opens a path for its
/// first stream, and lets it close when its last leaves.
struct Router {
    config: UsdmWsBuilder,
    paths: HashMap<StreamPath, Path>,
    /// Each path's streams, as its supervisor last reported them.
    membership: HashMap<StreamPath, Vec<StreamName>>,
    events: Events,
    /// Set when the router closes, which closes every path.
    closing: watch::Sender<bool>,
    ended: mpsc::UnboundedSender<Ended>,
    forwarders: Vec<tokio::task::JoinHandle<()>>,
    generation: u64,
}

impl Router {
    /// Forward a path's events into the consumer's queue, and route its
    /// changes to it from now on.
    fn adopt(&mut self, path: StreamPath, supervised: Supervised<UsdmProtocol>) {
        self.generation += 1;
        let handle = supervised.membership();
        self.forwarders.retain(|forwarder| !forwarder.is_finished());
        self.forwarders.push(tokio::spawn(forward(
            path,
            self.generation,
            supervised,
            self.events.clone(),
            self.closing.subscribe(),
            self.ended.clone(),
        )));
        self.paths.insert(
            path,
            Path {
                generation: self.generation,
                handle,
            },
        );
    }

    /// A path's stream ended. One that stopped on an error stops the client;
    /// one whose streams left is forgotten, unless a newer one holds its path.
    fn ended(&mut self, ended: Ended) -> Result<(), UsdmWsError> {
        if ended.stopped {
            return Err(UsdmWsError::Stopped);
        }
        if self
            .paths
            .get(&ended.path)
            .is_some_and(|path| path.generation == ended.generation)
        {
            self.paths.remove(&ended.path);
            self.membership.remove(&ended.path);
        }
        Ok(())
    }

    async fn subscribe(&mut self, streams: Vec<StreamName>) -> Result<(), UsdmWsError> {
        let by_path = new_by_path(&self.membership, streams);
        // Refuse before sending anything: the server answers the 1025th stream
        // with an error and then closes the connection, losing all 1024.
        for (path, add) in &by_path {
            let held = self.membership.get(path).map_or(0, Vec::len);
            if held + add.len() > MAX_STREAMS_PER_CONNECTION {
                return Err(UsdmWsError::TooManyStreams {
                    path: *path,
                    limit: MAX_STREAMS_PER_CONNECTION,
                });
            }
        }
        for path in StreamPath::ALL {
            let Some(add) = by_path.get(path) else {
                continue;
            };
            match self.paths.get(path) {
                Some(slot) => {
                    let handle = slot.handle.clone();
                    self.change(*path, &handle, Change::Subscribe(add.clone()))
                        .await?;
                }
                None => self.open(*path, add.clone()).await?,
            }
        }
        Ok(())
    }

    async fn unsubscribe(&mut self, streams: Vec<StreamName>) -> Result<(), UsdmWsError> {
        let mut by_path: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
        for stream in streams {
            let path = stream.path();
            if self
                .membership
                .get(&path)
                .is_some_and(|m| m.contains(&stream))
            {
                let batch = by_path.entry(path).or_default();
                if !batch.contains(&stream) {
                    batch.push(stream);
                }
            }
        }
        for path in StreamPath::ALL {
            let (Some(remove), Some(slot)) = (by_path.get(path), self.paths.get(path)) else {
                continue;
            };
            let handle = slot.handle.clone();
            self.change(*path, &handle, Change::Unsubscribe(remove.clone()))
                .await?;
        }
        Ok(())
    }

    /// Open a path for its first streams: its connection, or an outage when
    /// the first attempt fails in a way retrying can fix. One that fails for
    /// good opens nothing.
    async fn open(
        &mut self,
        path: StreamPath,
        streams: Vec<StreamName>,
    ) -> Result<(), UsdmWsError> {
        let supervised = self.config.supervisor(path).start(streams.clone()).await?;
        self.adopt(path, supervised);
        self.membership.insert(path, streams);
        Ok(())
    }

    /// Make one change on a path, and note the streams it leaves there.
    async fn change(
        &mut self,
        path: StreamPath,
        handle: &PathHandle<UsdmProtocol>,
        change: Change,
    ) -> Result<(), UsdmWsError> {
        match handle.apply(change.clone()).await {
            Ok(streams) => {
                self.settle(path, streams);
                Ok(())
            }
            // `UsdmProtocol::stalled`: the change outlived the staleness
            // window. It was recorded before it was sent, the connection is
            // being replaced, and the replay applies it, so it stands.
            Err(UsdmWsError::NoAnswer { .. }) => {
                let mut streams = self.membership.get(&path).cloned().unwrap_or_default();
                match change {
                    Change::Subscribe(add) => streams.extend(add),
                    Change::Unsubscribe(remove) => streams.retain(|s| !remove.contains(s)),
                }
                self.settle(path, streams);
                Ok(())
            }
            Err(err @ UsdmWsError::Refused { .. }) => {
                // The batches answered before the refusal stay; an empty
                // change reads what the path now carries.
                if let Ok(streams) = handle.apply(Change::Subscribe(Vec::new())).await {
                    self.settle(path, streams);
                }
                Err(err)
            }
            Err(err) => Err(err),
        }
    }

    /// Note a path's streams after a change. A path left with none closes
    /// itself and is forgotten now, without waiting for it, so a full queue
    /// cannot hold the change's caller.
    fn settle(&mut self, path: StreamPath, streams: Vec<StreamName>) {
        if streams.is_empty() {
            self.membership.remove(&path);
            self.paths.remove(&path);
        } else {
            self.membership.insert(path, streams);
        }
    }

    /// Close every path, and wait for each to go.
    async fn close(self) {
        let _ = self.closing.send(true);
        for forwarder in self.forwarders {
            let _ = forwarder.await;
        }
    }
}

/// Group `streams` by path, dropping any already wanted and duplicates.
fn new_by_path(
    membership: &HashMap<StreamPath, Vec<StreamName>>,
    streams: Vec<StreamName>,
) -> HashMap<StreamPath, Vec<StreamName>> {
    let mut by_path: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
    for stream in streams {
        let path = stream.path();
        let known = membership.get(&path).is_some_and(|m| m.contains(&stream));
        let batch = by_path.entry(path).or_default();
        if !known && !batch.contains(&stream) {
            batch.push(stream);
        }
    }
    by_path.retain(|_, batch| !batch.is_empty());
    by_path
}

/// Serve membership changes one at a time, and stop when the consumer goes,
/// the client is closed, or a path stops on an error.
async fn route(
    mut router: Router,
    mut commands: mpsc::Receiver<Command>,
    mut ended: mpsc::UnboundedReceiver<Ended>,
) -> Result<(), UsdmWsError> {
    let events = router.events.clone();
    let result = loop {
        tokio::select! {
            biased;
            Some(path) = ended.recv() => {
                if let Err(err) = router.ended(path) {
                    break Err(err);
                }
            }
            () = events.closed() => break Ok(()),
            command = commands.recv() => match command {
                None | Some(Command::Close) => break Ok(()),
                Some(Command::Subscribe(streams, reply)) => {
                    let _ = reply.send(router.subscribe(streams).await);
                }
                Some(Command::Unsubscribe(streams, reply)) => {
                    let _ = reply.send(router.unsubscribe(streams).await);
                }
            },
        }
    };
    drop(events);
    router.close().await;
    result
}

/// Move one path's events into the consumer's queue until the path ends, the
/// consumer goes, or the router closes. A path that ends by itself says so,
/// and whether it stopped on an error.
async fn forward(
    path: StreamPath,
    generation: u64,
    mut supervised: Supervised<UsdmProtocol>,
    events: Events,
    mut closing: watch::Receiver<bool>,
    ended: mpsc::UnboundedSender<Ended>,
) {
    loop {
        let next = tokio::select! {
            biased;
            _ = closing.wait_for(|closing| *closing) => break,
            // The consumer is gone: close the path politely before going,
            // so `SupervisedUsdmWs::close` returns after it has.
            () = events.closed() => break,
            next = supervised.next() => next,
        };
        let Some(item) = next else {
            let stopped = supervised.close().await.is_err();
            let _ = ended.send(Ended {
                path,
                generation,
                stopped,
            });
            return;
        };
        let slot = tokio::select! {
            biased;
            _ = closing.wait_for(|closing| *closing) => break,
            slot = events.reserve() => slot,
        };
        match slot {
            Ok(slot) => slot.send(item),
            Err(_) => break,
        }
    }
    let _ = supervised.close().await;
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
}
