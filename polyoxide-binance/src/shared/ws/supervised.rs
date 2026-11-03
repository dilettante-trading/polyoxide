//! The supervised tier: one kit `Supervisor` per routed path, with keep-alive
//! on the wall clock, staleness detection, reconnect with a paced replay of
//! the path's streams, rotation before Binance's 24-hour cutoff and outage
//! markers, behind a router that sends each membership change to its path's
//! connection, opens a path for its first stream, and merges every path's
//! events into one stream.

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    task::{Context, Poll},
};

use futures_util::StreamExt;
use polyoxide_ws::{MembershipHandle as PathHandle, Supervised, Supervisor, Timing};
use tokio::sync::{mpsc, oneshot, watch};

use super::{
    protocol::{BinanceProtocol, Change},
    Shape, SocketError, StreamSet, EVENT_BUFFER, MAX_STREAMS_PER_CONNECTION,
};

/// What a product's builder hands the shared code.
pub(crate) struct Settings<S: StreamSet> {
    /// The host the paths are on.
    pub(crate) base_url: String,
    /// The kit's timing, rotation age included.
    pub(crate) timing: Timing,
    /// The streams to open with.
    pub(crate) streams: Vec<S::Name>,
}

impl<S: StreamSet> Settings<S> {
    /// The supervisor for one path's connection.
    fn supervisor(&self, path: S::Path) -> Supervisor<BinanceProtocol<S>> {
        Supervisor::new(
            BinanceProtocol::new(self.base_url.clone(), path),
            self.timing,
        )
    }
}

type Cause<S> = <<S as StreamSet>::Error as SocketError>::Cause;
type Item<S> = Result<<S as StreamSet>::Event, <S as StreamSet>::Error>;
type Events<S> = mpsc::Sender<Item<S>>;
type Reply<S> = oneshot::Sender<Result<(), <S as StreamSet>::Error>>;

/// Open a connection for each path the streams need, subscribe, and start
/// supervising.
///
/// Fails if any first connect fails, closing the others: a setup that is
/// wrong is reported, not retried. With no streams it opens nothing until the
/// first [`Handle::subscribe`].
pub(crate) async fn connect<S: StreamSet>(settings: Settings<S>) -> Result<Running<S>, S::Error> {
    let mut membership: HashMap<S::Path, Vec<S::Name>> = HashMap::new();
    for stream in settings.streams.iter().cloned() {
        let wanted = membership.entry(S::path(&stream)).or_default();
        if !wanted.contains(&stream) {
            wanted.push(stream);
        }
    }
    for (path, wanted) in &membership {
        if wanted.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(<S::Error as SocketError>::too_many_streams(
                *path,
                MAX_STREAMS_PER_CONNECTION,
            ));
        }
    }
    let mut opened: Vec<(S::Path, Supervised<BinanceProtocol<S>>)> = Vec::new();
    for path in S::PATHS {
        let Some(wanted) = membership.get(path) else {
            continue;
        };
        match settings.supervisor(*path).connect(wanted.clone()).await {
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
    let stop = Stop::<S>::new();
    let mut router = Router {
        config: settings,
        paths: HashMap::new(),
        membership,
        events,
        closing,
        ended,
        forwarders: Vec::new(),
        generation: 0,
        stop: stop.clone(),
    };
    for (path, supervised) in opened {
        router.adopt(path, supervised);
    }
    let task = tokio::spawn(route(router, commands_rx, ended_rx));
    Ok(Running {
        events: events_rx,
        commands,
        task,
        stop,
    })
}

enum Command<S: StreamSet> {
    Subscribe(Vec<S::Name>, Reply<S>),
    Unsubscribe(Vec<S::Name>, Reply<S>),
    /// Close every socket and end. Sent by [`Running::close`]; a dropped
    /// sender is not enough, because every [`Handle`] holds a clone.
    Close,
}

/// Changes the streams of a [`Running`] client.
///
/// Each stream is routed to its path's connection, opening it if this is the
/// path's first stream and closing it when its last leaves. The membership is
/// a set: subscribing a stream twice holds it once, and counting references is
/// the caller's job. A change is recorded before it is sent (see the product's
/// handle for the rules a caller reads).
pub(crate) struct Handle<S: StreamSet> {
    commands: mpsc::Sender<Command<S>>,
    stop: Stop<S>,
}

// By hand: a derive would require `S: Clone` and `S: Debug` of the marker type.
impl<S: StreamSet> Clone for Handle<S> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            stop: self.stop.clone(),
        }
    }
}

impl<S: StreamSet> std::fmt::Debug for Handle<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").finish_non_exhaustive()
    }
}

impl<S: StreamSet> Handle<S> {
    /// Subscribe to more streams.
    pub(crate) async fn subscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        self.send(|reply| Command::Subscribe(streams, reply)).await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub(crate) async fn unsubscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        self.send(|reply| Command::Unsubscribe(streams, reply))
            .await
    }

    async fn send(&self, make: impl FnOnce(Reply<S>) -> Command<S>) -> Result<(), S::Error> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.commands
            .send(make(reply_tx))
            .await
            .map_err(|_| self.stop.error())?;
        reply_rx.await.map_err(|_| self.stop.error())?
    }
}

/// The error a path stopped the client on, once the router has seen it:
/// shared by the router, the stream and every handle, so `close` and every
/// later membership call carry it (M-D8). A stop's cause is set once.
struct Stop<S: StreamSet>(Arc<OnceLock<Cause<S>>>);

impl<S: StreamSet> Clone for Stop<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<S: StreamSet> Stop<S> {
    fn new() -> Self {
        Self(Arc::new(OnceLock::new()))
    }

    fn set(&self, cause: Cause<S>) {
        let _ = self.0.set(cause);
    }

    /// The product's `Stopped` error, with the cause when there is one.
    fn error(&self) -> S::Error {
        <S::Error as SocketError>::stopped(self.0.get().cloned())
    }
}

/// A supervised set of connections: the events of every path, merged, that
/// survives drops, stalls and Binance's 24-hour cutoff.
///
/// A fatal error on any path, such as the server refusing a replay after a
/// reconnect, is yielded as `Err` and then the stream ends, every path closed.
pub(crate) struct Running<S: StreamSet> {
    events: mpsc::Receiver<Item<S>>,
    commands: mpsc::Sender<Command<S>>,
    task: tokio::task::JoinHandle<Result<(), S::Error>>,
    stop: Stop<S>,
}

impl<S: StreamSet> Running<S> {
    /// A handle for changing streams while the stream runs.
    pub(crate) fn membership(&self) -> Handle<S> {
        Handle {
            commands: self.commands.clone(),
            stop: self.stop.clone(),
        }
    }

    /// Whether the router's task is still running.
    pub(crate) fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    /// The next event or error, for the product's `Stream` impl.
    pub(crate) fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<Item<S>>> {
        self.events.poll_recv(cx)
    }

    /// Close every connection and stop. Every [`Handle`] answers the product's
    /// `Stopped` afterwards.
    ///
    /// Fails with `Stopped` if a path had stopped the client on an error,
    /// which the stream yielded: its cause is that error. That holds once the
    /// router has the path's stop: a `close()` made right after the stream
    /// yielded the stopping error can return `Ok` or `Stopped { cause: None }`.
    pub(crate) async fn close(self) -> Result<(), S::Error> {
        let Self {
            events,
            commands,
            task,
            stop,
        } = self;
        // Dropping the receiver first unblocks a path parked behind a full
        // queue, which would otherwise never see the command.
        drop(events);
        let _ = commands.send(Command::Close).await;
        drop(commands);
        match task.await {
            Ok(result) => result,
            Err(_) => Err(stop.error()),
        }
    }
}

/// A path's supervised connection, as the router holds it.
struct Path<S: StreamSet> {
    /// Which of the path's supervisors this is; a path closed for want of
    /// streams may still be draining when the next opens.
    generation: u64,
    handle: PathHandle<BinanceProtocol<S>>,
}

/// A path's stream that ended.
struct Ended<S: StreamSet> {
    path: S::Path,
    generation: u64,
    /// Whether it stopped on an error, rather than because its streams left.
    stopped: bool,
    /// The error it stopped on: the last item it yielded, when that was an
    /// error.
    cause: Option<Cause<S>>,
}

/// The router: routes membership changes to the paths, opens a path for its
/// first stream, and lets it close when its last leaves.
struct Router<S: StreamSet> {
    config: Settings<S>,
    paths: HashMap<S::Path, Path<S>>,
    /// Each path's streams, as its supervisor last reported them.
    membership: HashMap<S::Path, Vec<S::Name>>,
    events: Events<S>,
    /// Set when the router closes, which closes every path.
    closing: watch::Sender<bool>,
    ended: mpsc::UnboundedSender<Ended<S>>,
    forwarders: Vec<tokio::task::JoinHandle<()>>,
    generation: u64,
    /// The cause the client stopped on, shared with the stream and handles.
    stop: Stop<S>,
}

impl<S: StreamSet> Router<S> {
    /// Forward a path's events into the consumer's queue, and route its
    /// changes to it from now on.
    fn adopt(&mut self, path: S::Path, supervised: Supervised<BinanceProtocol<S>>) {
        self.generation += 1;
        let handle = supervised.membership();
        self.forwarders.retain(|forwarder| !forwarder.is_finished());
        self.forwarders.push(tokio::spawn(forward::<S>(
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
    fn ended(&mut self, ended: Ended<S>) -> Result<(), S::Error> {
        if ended.stopped {
            if let Some(cause) = ended.cause {
                self.stop.set(cause);
            }
            return Err(self.stop.error());
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

    async fn subscribe(&mut self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        let by_path = new_by_path::<S>(&self.membership, streams);
        // Refuse before sending anything: the server answers the 1025th stream
        // with an error and then closes the connection, losing all 1024.
        for (path, add) in &by_path {
            let held = self.membership.get(path).map_or(0, Vec::len);
            if held + add.len() > MAX_STREAMS_PER_CONNECTION {
                return Err(<S::Error as SocketError>::too_many_streams(
                    *path,
                    MAX_STREAMS_PER_CONNECTION,
                ));
            }
        }
        for path in S::PATHS {
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

    async fn unsubscribe(&mut self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        let mut by_path: HashMap<S::Path, Vec<S::Name>> = HashMap::new();
        for stream in streams {
            let path = S::path(&stream);
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
        for path in S::PATHS {
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
    async fn open(&mut self, path: S::Path, streams: Vec<S::Name>) -> Result<(), S::Error> {
        let supervised = self.config.supervisor(path).start(streams.clone()).await?;
        self.adopt(path, supervised);
        self.membership.insert(path, streams);
        Ok(())
    }

    /// Make one change on a path, and note the streams it leaves there.
    async fn change(
        &mut self,
        path: S::Path,
        handle: &PathHandle<BinanceProtocol<S>>,
        change: Change<S::Name>,
    ) -> Result<(), S::Error> {
        match handle.apply(change.clone()).await {
            Ok(streams) => {
                self.settle(path, streams);
                Ok(())
            }
            // `BinanceProtocol::stalled`: the change outlived the staleness
            // window. It was recorded before it was sent, the connection is
            // being replaced, and the replay applies it, so it stands.
            Err(err) if matches!(err.shape(), Shape::NoAnswer) => {
                let mut streams = self.membership.get(&path).cloned().unwrap_or_default();
                match change {
                    Change::Subscribe(add) => streams.extend(add),
                    Change::Unsubscribe(remove) => streams.retain(|s| !remove.contains(s)),
                }
                self.settle(path, streams);
                Ok(())
            }
            Err(err) if matches!(err.shape(), Shape::Refused) => {
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
    fn settle(&mut self, path: S::Path, streams: Vec<S::Name>) {
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
fn new_by_path<S: StreamSet>(
    membership: &HashMap<S::Path, Vec<S::Name>>,
    streams: Vec<S::Name>,
) -> HashMap<S::Path, Vec<S::Name>> {
    let mut by_path: HashMap<S::Path, Vec<S::Name>> = HashMap::new();
    for stream in streams {
        let path = S::path(&stream);
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
async fn route<S: StreamSet>(
    mut router: Router<S>,
    mut commands: mpsc::Receiver<Command<S>>,
    mut ended: mpsc::UnboundedReceiver<Ended<S>>,
) -> Result<(), S::Error> {
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
/// and whether it stopped on an error, which is the last item it yielded.
async fn forward<S: StreamSet>(
    path: S::Path,
    generation: u64,
    mut supervised: Supervised<BinanceProtocol<S>>,
    events: Events<S>,
    mut closing: watch::Receiver<bool>,
    ended: mpsc::UnboundedSender<Ended<S>>,
) {
    // The last item, when it was an error: a path that stops yields the
    // error it stopped on last, before its stream ends.
    let mut last_error: Option<Cause<S>> = None;
    loop {
        let next = tokio::select! {
            biased;
            _ = closing.wait_for(|closing| *closing) => break,
            // The consumer is gone: close the path politely before going,
            // so `Running::close` returns after it has.
            () = events.closed() => break,
            next = supervised.next() => next,
        };
        let Some(item) = next else {
            let stopped = supervised.close().await.is_err();
            let _ = ended.send(Ended {
                path,
                generation,
                stopped,
                cause: last_error.filter(|_| stopped),
            });
            return;
        };
        last_error = item.as_ref().err().map(<S::Error as SocketError>::cause);
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
