//! The supervised tier: one connection per path, each on its own task, with
//! keep-alive on the wall clock, staleness detection, reconnect with a paced
//! replay of the path's streams, rotation before Binance's 24-hour cutoff,
//! outage markers, and live membership changes.

use std::{
    collections::HashMap,
    fmt,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};

use crate::usdm::ws::{
    client::UsdmWs,
    error::{Recovery, UsdmWsError},
    event::Update,
    stream::StreamName,
    StreamPath, MAX_STREAMS_PER_CONNECTION, USDM_WS_BASE,
};

/// Default keep-alive cadence. The server pings only about every 180 s, so a
/// connection carrying quiet streams would otherwise go minutes without an
/// inbound frame.
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(20);
/// Default silence before a connection is presumed dead. Pongs and the
/// server's pings count as inbound frames.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(30);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Ten minutes under Binance's documented 24-hour connection lifetime.
const DEFAULT_MAX_CONNECTION_AGE: Duration = Duration::from_secs(23 * 3600 + 50 * 60);
/// Events the consumer may leave unread before the path tasks block.
const EVENT_BUFFER: usize = 1024;
/// The shortest backoff. Zero would never grow, so a dead server would be
/// retried on every timer tick.
const MIN_BACKOFF: Duration = Duration::from_millis(1);

/// The reconnect delay schedule: doubling to a ceiling, reset after a
/// connection that delivered at least one update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Backoff {
    initial: Duration,
    max: Duration,
    next: Duration,
}

impl Backoff {
    fn new(initial: Duration, max: Duration) -> Self {
        let max = max.max(MIN_BACKOFF);
        let initial = initial.clamp(MIN_BACKOFF, max);
        Self {
            initial,
            max,
            next: initial,
        }
    }

    fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = self.next.saturating_mul(2).min(self.max);
        delay
    }

    fn after_connection_ended(&mut self, delivered: bool) {
        if delivered {
            self.next = self.initial;
        }
    }
}

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
    /// The server closed it, with the code and reason it sent.
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },
    /// A transport error, a request left unanswered, or a failed first
    /// connect of a path opened by a membership change.
    Error(UsdmWsError),
    /// Nothing arrived, pongs included, for the staleness window.
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
#[derive(Debug, Clone)]
pub struct UsdmWsBuilder {
    base_url: String,
    ping_interval: Duration,
    stale_after: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
    connect_timeout: Duration,
    max_connection_age: Duration,
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
        Self {
            base_url: USDM_WS_BASE.to_owned(),
            ping_interval: DEFAULT_PING_INTERVAL,
            stale_after: DEFAULT_STALE_AFTER,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            max_connection_age: DEFAULT_MAX_CONNECTION_AGE,
            streams: Vec::new(),
        }
    }

    /// Connect to another host, such as a local test server.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// How often each connection sends a protocol ping, whatever its traffic.
    /// Binance closes a connection that sends more than 10 messages a second.
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Silence after which a connection is presumed dead and replaced. Pongs
    /// and the server's pings count, so a quiet connection that answers stays
    /// up. Also bounds each request's wait for its answer.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect delay schedule: `initial`, doubling to `max`, with no cap on
    /// attempts. Both are raised to at least 1 ms, and an `initial` above
    /// `max` is lowered to `max`.
    pub fn backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max;
        self
    }

    /// How long one connection attempt may take to open.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// How long a connection may live before it is replaced, without backoff,
    /// ahead of Binance's 24-hour cutoff.
    /// Each replacement is a new connection, and Binance allows 300 connection attempts per 5 minutes per IP.
    pub fn max_connection_age(mut self, age: Duration) -> Self {
        self.max_connection_age = age;
        self
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
        let mut opened: Vec<(StreamPath, UsdmWs)> = Vec::new();
        for path in StreamPath::ALL {
            let Some(wanted) = membership.get(path) else {
                continue;
            };
            match self.open(*path, wanted).await {
                Ok(ws) => opened.push((*path, ws)),
                Err(err) => {
                    for (_, mut ws) in opened {
                        let _ = ws.close().await;
                    }
                    return Err(err);
                }
            }
        }
        let (events_tx, events_rx) = mpsc::channel(EVENT_BUFFER);
        let (commands_tx, commands_rx) = mpsc::channel(16);
        let task = tokio::spawn(supervise(self, opened, membership, commands_rx, events_tx));
        Ok(SupervisedUsdmWs {
            events: events_rx,
            commands: commands_tx,
            task,
        })
    }

    async fn open(&self, path: StreamPath, streams: &[StreamName]) -> Result<UsdmWs, UsdmWsError> {
        let mut ws =
            UsdmWs::open(&self.base_url, path, self.connect_timeout, self.stale_after).await?;
        ws.subscribe(streams).await?;
        Ok(ws)
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

enum PathCommand {
    Subscribe(Vec<StreamName>, Reply),
    Unsubscribe(Vec<StreamName>, Reply),
    Close,
}

/// Changes the streams of a running [`SupervisedUsdmWs`].
///
/// Each stream is routed to its path's connection, opening it if this is the
/// path's first stream and closing it when its last leaves. The membership is
/// a set: subscribing a stream twice holds it once, and counting references is
/// the caller's job.
///
/// During a path's outage a change is recorded and answered `Ok`, and the
/// replay applies it. It is answered when the path task is between attempts:
/// a change that arrives while a connect or a replay is in flight waits for
/// that attempt, up to the connect timeout and each unanswered batch's wait.
/// Calls are served one at a time across paths, so a change for a healthy
/// path can wait behind another path's attempt.
///
/// A call fails when the server refuses it ([`UsdmWsError::Refused`]), when it
/// would pass 1024 streams on a path ([`UsdmWsError::TooManyStreams`], nothing
/// sent), when the first connect of a path it opens fails in a way retrying
/// cannot fix ([`UsdmWsError::Connect`]), or when the client has stopped
/// ([`UsdmWsError::Stopped`]).
///
/// A call waits for the path task, which cannot take it while parked behind a
/// full event buffer (1024 unread events). Make membership changes from a task
/// other than the one draining the stream, or keep draining while they run.
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
/// A fatal error, such as the server refusing a replay after a reconnect, is
/// yielded as `Err` and then the stream ends. An `Err` whose
/// [`recovery`](crate::usdm::ws::UsdmWsError::recovery) is
/// [`Recovery::SkipFrame`], a frame that
/// did not decode, does not end it.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::usdm::{types::Symbol, ws::{Event, Recovery, StreamName, UsdmWsBuilder}};
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
///         Err(err) if err.recovery() == Recovery::SkipFrame => continue,
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
    events: mpsc::Receiver<Result<Event, UsdmWsError>>,
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
        // Dropping the receiver first unblocks a path task parked behind a
        // full buffer, which would otherwise never see the command.
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

type Events = mpsc::Sender<Result<Event, UsdmWsError>>;

struct PathSlot {
    commands: mpsc::Sender<PathCommand>,
    task: tokio::task::JoinHandle<()>,
}

/// How a path task starts.
enum Start {
    /// `connect` opened the connection already.
    Open(Box<UsdmWs>),
    /// A membership change wants the path: connect, and say how the first
    /// attempt went.
    Connect(Reply),
}

/// The supervisor: routes membership changes to path tasks, opens a path for
/// its first stream, and lets it close when its last leaves.
async fn supervise(
    config: UsdmWsBuilder,
    opened: Vec<(StreamPath, UsdmWs)>,
    mut membership: HashMap<StreamPath, Vec<StreamName>>,
    mut commands: mpsc::Receiver<Command>,
    events: Events,
) -> Result<(), UsdmWsError> {
    let (fatal_tx, mut fatal_rx) = mpsc::channel::<()>(StreamPath::ALL.len());
    let mut paths: HashMap<StreamPath, PathSlot> = HashMap::new();
    for (path, ws) in opened {
        let streams = membership.get(&path).cloned().unwrap_or_default();
        let slot = spawn_path(
            &config,
            path,
            Start::Open(Box::new(ws)),
            streams,
            &events,
            &fatal_tx,
        );
        paths.insert(path, slot);
    }

    let result = loop {
        tokio::select! {
            biased;
            _ = fatal_rx.recv() => break Err(UsdmWsError::Stopped),
            () = events.closed() => break Ok(()),
            command = commands.recv() => match command {
                None | Some(Command::Close) => break Ok(()),
                Some(Command::Subscribe(streams, reply)) => {
                    let result = subscribe(&config, &mut paths, &mut membership, streams, &events, &fatal_tx).await;
                    let _ = reply.send(result);
                }
                Some(Command::Unsubscribe(streams, reply)) => {
                    let result = unsubscribe(&mut paths, &mut membership, streams).await;
                    let _ = reply.send(result);
                }
            },
        }
    };
    for (_, slot) in paths.drain() {
        let _ = slot.commands.send(PathCommand::Close).await;
        let _ = slot.task.await;
    }
    result
}

fn spawn_path(
    config: &UsdmWsBuilder,
    path: StreamPath,
    start: Start,
    streams: Vec<StreamName>,
    events: &Events,
    fatal: &mpsc::Sender<()>,
) -> PathSlot {
    let (commands_tx, commands_rx) = mpsc::channel(16);
    let task = tokio::spawn(run_path(
        config.clone(),
        path,
        start,
        streams,
        commands_rx,
        events.clone(),
        fatal.clone(),
    ));
    PathSlot {
        commands: commands_tx,
        task,
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

async fn subscribe(
    config: &UsdmWsBuilder,
    paths: &mut HashMap<StreamPath, PathSlot>,
    membership: &mut HashMap<StreamPath, Vec<StreamName>>,
    streams: Vec<StreamName>,
    events: &Events,
    fatal: &mpsc::Sender<()>,
) -> Result<(), UsdmWsError> {
    let by_path = new_by_path(membership, streams);
    // Refuse before sending anything: the server answers the 1025th stream
    // with an error and then closes the connection, losing all 1024.
    for (path, add) in &by_path {
        let held = membership.get(path).map_or(0, Vec::len);
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
        let (reply_tx, reply_rx) = oneshot::channel();
        match paths.get(path) {
            Some(slot) => {
                if slot
                    .commands
                    .send(PathCommand::Subscribe(add.clone(), reply_tx))
                    .await
                    .is_err()
                {
                    return Err(UsdmWsError::Stopped);
                }
            }
            None => {
                let slot = spawn_path(
                    config,
                    *path,
                    Start::Connect(reply_tx),
                    add.clone(),
                    events,
                    fatal,
                );
                paths.insert(*path, slot);
            }
        }
        match reply_rx.await {
            Ok(Ok(())) => membership
                .entry(*path)
                .or_default()
                .extend(add.iter().cloned()),
            Ok(Err(err)) => {
                if !membership.contains_key(path) {
                    // The path's first connect was refused; its task has ended.
                    paths.remove(path);
                }
                return Err(err);
            }
            Err(_) => return Err(UsdmWsError::Stopped),
        }
    }
    Ok(())
}

async fn unsubscribe(
    paths: &mut HashMap<StreamPath, PathSlot>,
    membership: &mut HashMap<StreamPath, Vec<StreamName>>,
    streams: Vec<StreamName>,
) -> Result<(), UsdmWsError> {
    let mut by_path: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
    for stream in streams {
        let path = stream.path();
        if membership.get(&path).is_some_and(|m| m.contains(&stream)) {
            let batch = by_path.entry(path).or_default();
            if !batch.contains(&stream) {
                batch.push(stream);
            }
        }
    }
    for path in StreamPath::ALL {
        let (Some(remove), Some(slot)) = (by_path.get(path), paths.get(path)) else {
            continue;
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if slot
            .commands
            .send(PathCommand::Unsubscribe(remove.clone(), reply_tx))
            .await
            .is_err()
        {
            return Err(UsdmWsError::Stopped);
        }
        match reply_rx.await {
            Ok(Ok(())) => {
                let left = membership.entry(*path).or_default();
                left.retain(|s| !remove.contains(s));
                if left.is_empty() {
                    // The path task ends by itself once its last stream has
                    // left; it is not awaited, so a full event buffer cannot
                    // hold this call.
                    membership.remove(path);
                    paths.remove(path);
                }
            }
            Ok(Err(err)) => return Err(err),
            Err(_) => return Err(UsdmWsError::Stopped),
        }
    }
    Ok(())
}

/// How a path's outage ended.
enum Recovered {
    /// A fresh connection carries the path's streams.
    Up(Box<UsdmWs>),
    /// The path's last stream left; nothing on it can be stale.
    NotWanted,
    /// The consumer is gone or the client is closing.
    Closed,
    /// A failure retrying cannot fix.
    Fatal(UsdmWsError),
}

/// How one connection's life ended.
enum End {
    /// The consumer is gone, the client is closing, or the last stream left.
    Closed,
    /// It reached `max_connection_age`.
    Rotation,
    /// It was lost.
    Lost(DisconnectReason),
}

/// Send a Close frame, but not wait on a peer that has stopped reading.
async fn close_politely(ws: &mut UsdmWs) {
    let _ = tokio::time::timeout(Duration::from_secs(1), ws.close()).await;
}

async fn emit(events: &Events, event: Event) -> bool {
    events.send(Ok(event)).await.is_ok()
}

/// One path: pump its connection, and when it is lost, report the outage,
/// reconnect with backoff, replay, and report the recovery.
async fn run_path(
    config: UsdmWsBuilder,
    path: StreamPath,
    start: Start,
    mut streams: Vec<StreamName>,
    mut commands: mpsc::Receiver<PathCommand>,
    events: Events,
    fatal: mpsc::Sender<()>,
) {
    let mut backoff = Backoff::new(config.initial_backoff, config.max_backoff);
    // `None` while an outage has been reported and not yet ended.
    let mut socket = match start {
        Start::Open(ws) => Some(ws),
        Start::Connect(reply) => match config.open(path, &streams).await {
            Ok(ws) => {
                let _ = reply.send(Ok(()));
                Some(Box::new(ws))
            }
            Err(err) if err.recovery() == Recovery::Reconnect => {
                // Recorded: the caller's change stands, and the path keeps
                // trying, reporting the outage like any other.
                let _ = reply.send(Ok(()));
                tracing::warn!(%err, %path, "binance stream connect failed, retrying");
                if !emit(
                    &events,
                    Event::Disconnected {
                        path,
                        reason: DisconnectReason::Error(err),
                    },
                )
                .await
                {
                    return;
                }
                None
            }
            Err(err) => {
                let _ = reply.send(Err(err));
                return;
            }
        },
    };
    let mut first_delay = None;

    loop {
        let mut ws = match socket.take() {
            Some(ws) => ws,
            None => {
                let delay = first_delay.take().unwrap_or_else(|| backoff.take());
                match recover(
                    &config,
                    path,
                    &mut streams,
                    &mut commands,
                    &events,
                    &mut backoff,
                    delay,
                )
                .await
                {
                    Recovered::Up(ws) => {
                        if !emit(&events, Event::Reconnected { path }).await {
                            return;
                        }
                        ws
                    }
                    Recovered::NotWanted => {
                        let _ = emit(&events, Event::Reconnected { path }).await;
                        return;
                    }
                    Recovered::Closed => return,
                    Recovered::Fatal(err) => {
                        let _ = events.send(Err(err)).await;
                        let _ = fatal.send(()).await;
                        return;
                    }
                }
            }
        };
        let mut delivered = false;
        let end = pump(
            &config,
            &mut ws,
            &mut streams,
            &mut commands,
            &events,
            &mut delivered,
        )
        .await;
        let reason = match end {
            End::Closed => {
                close_politely(&mut ws).await;
                return;
            }
            End::Rotation => {
                close_politely(&mut ws).await;
                first_delay = Some(Duration::ZERO);
                DisconnectReason::Rotation
            }
            // A lost connection is dropped, not closed: the peer may not be
            // reading, and a Close frame would wait on it.
            End::Lost(reason) => {
                backoff.after_connection_ended(delivered);
                tracing::warn!(%path, %reason, "binance stream lost, reconnecting");
                reason
            }
        };
        if !emit(&events, Event::Disconnected { path, reason }).await {
            return;
        }
    }
}

/// Wait out `delay`, then connect and replay, repeating with backoff while
/// retrying can fix the failure. Membership changes during the backoff sleep
/// are recorded and answered at once; one that arrives during a connect or a
/// replay waits for it.
async fn recover(
    config: &UsdmWsBuilder,
    path: StreamPath,
    streams: &mut Vec<StreamName>,
    commands: &mut mpsc::Receiver<PathCommand>,
    events: &Events,
    backoff: &mut Backoff,
    mut delay: Duration,
) -> Recovered {
    loop {
        if streams.is_empty() {
            return Recovered::NotWanted;
        }
        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                biased;
                () = events.closed() => return Recovered::Closed,
                command = commands.recv() => match command {
                    None | Some(PathCommand::Close) => return Recovered::Closed,
                    Some(PathCommand::Subscribe(add, reply)) => {
                        for stream in add {
                            if !streams.contains(&stream) {
                                streams.push(stream);
                            }
                        }
                        let _ = reply.send(Ok(()));
                    }
                    Some(PathCommand::Unsubscribe(remove, reply)) => {
                        streams.retain(|s| !remove.contains(s));
                        let _ = reply.send(Ok(()));
                        if streams.is_empty() {
                            return Recovered::NotWanted;
                        }
                    }
                },
                () = &mut sleep => break,
            }
        }
        tokio::select! {
            biased;
            () = events.closed() => return Recovered::Closed,
            result = config.open(path, streams) => match result {
                Ok(ws) => return Recovered::Up(Box::new(ws)),
                Err(err) if err.recovery() == Recovery::Reconnect => {
                    tracing::warn!(%err, %path, "binance stream reconnect failed, retrying");
                    delay = backoff.take();
                }
                Err(err) => return Recovered::Fatal(err),
            },
        }
    }
}

/// Drive one connection until it is lost, rotated, or no longer wanted.
///
/// The ping is due on the wall clock, traffic or not, and every wait is bounded
/// by whichever of the next ping, the staleness deadline and the rotation
/// comes first.
async fn pump(
    config: &UsdmWsBuilder,
    ws: &mut UsdmWs,
    streams: &mut Vec<StreamName>,
    commands: &mut mpsc::Receiver<PathCommand>,
    events: &Events,
    delivered: &mut bool,
) -> End {
    let opened = Instant::now();
    let mut last_ping = Instant::now();
    loop {
        if opened.elapsed() >= config.max_connection_age {
            return End::Rotation;
        }
        if last_ping.elapsed() >= config.ping_interval {
            if let Err(err) = ws.send_ping().await {
                return End::Lost(DisconnectReason::Error(err));
            }
            last_ping = Instant::now();
        }
        let silent = ws.last_inbound().elapsed();
        if silent >= config.stale_after {
            return End::Lost(DisconnectReason::Stale);
        }
        // A wait, not an `Instant`: a limit such as `Duration::MAX` overflows
        // when added to one.
        let wait = config
            .ping_interval
            .saturating_sub(last_ping.elapsed())
            .min(config.stale_after.saturating_sub(silent))
            .min(config.max_connection_age.saturating_sub(opened.elapsed()));

        tokio::select! {
            biased;
            () = events.closed() => return End::Closed,
            command = commands.recv() => match command {
                None | Some(PathCommand::Close) => return End::Closed,
                Some(PathCommand::Subscribe(add, reply)) => {
                    let result = ws.subscribe(&add).await;
                    *streams = ws.streams().to_vec();
                    match result {
                        Ok(()) => { let _ = reply.send(Ok(())); }
                        Err(err @ (UsdmWsError::Refused { .. } | UsdmWsError::TooManyStreams { .. })) => {
                            let _ = reply.send(Err(err));
                        }
                        Err(err) => {
                            // The connection failed under the request: record
                            // the change, and let the replay apply it.
                            for stream in add {
                                if !streams.contains(&stream) {
                                    streams.push(stream);
                                }
                            }
                            let _ = reply.send(Ok(()));
                            return End::Lost(DisconnectReason::Error(err));
                        }
                    }
                }
                Some(PathCommand::Unsubscribe(remove, reply)) => {
                    if streams.iter().all(|s| remove.contains(s)) {
                        streams.clear();
                        let _ = reply.send(Ok(()));
                        return End::Closed;
                    }
                    let result = ws.unsubscribe(&remove).await;
                    *streams = ws.streams().to_vec();
                    match result {
                        Ok(()) => { let _ = reply.send(Ok(())); }
                        Err(err @ UsdmWsError::Refused { .. }) => { let _ = reply.send(Err(err)); }
                        Err(err) => {
                            streams.retain(|s| !remove.contains(s));
                            let _ = reply.send(Ok(()));
                            return End::Lost(DisconnectReason::Error(err));
                        }
                    }
                }
            },
            next = tokio::time::timeout(wait, ws.next()) => match next {
                // The ping, staleness or rotation is due; the top of the loop
                // sees to it.
                Err(_) => {}
                Ok(None) => {
                    let (code, reason) = ws.close_frame().unwrap_or((None, String::new()));
                    return End::Lost(DisconnectReason::Closed { code, reason });
                }
                Ok(Some(Err(err))) if err.recovery() == Recovery::SkipFrame => {
                    if events.send(Err(err)).await.is_err() {
                        return End::Closed;
                    }
                }
                Ok(Some(Err(err))) => return End::Lost(DisconnectReason::Error(err)),
                Ok(Some(Ok(update))) => {
                    *delivered = true;
                    if !emit(events, Event::Update(Box::new(update))).await {
                        return End::Closed;
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_cadences() {
        let b = UsdmWsBuilder::new();
        assert_eq!(b.base_url, USDM_WS_BASE);
        assert_eq!(b.ping_interval, Duration::from_secs(20));
        assert_eq!(b.stale_after, Duration::from_secs(30));
        assert_eq!(b.initial_backoff, Duration::from_millis(500));
        assert_eq!(b.max_backoff, Duration::from_secs(60));
        assert_eq!(b.connect_timeout, Duration::from_secs(10));
        assert_eq!(b.max_connection_age, Duration::from_secs(85_800));
        assert!(b.max_connection_age < Duration::from_secs(24 * 3600));
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
