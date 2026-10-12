//! The supervision task: one connection at a time, its outages, and the
//! consumer's queue.

use std::{
    collections::VecDeque,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::StreamExt;
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};

use super::{Decoded, Offline, Outage, Protocol, Queue, Recovery};
use crate::{connect, Backoff, Connection, Kind, Mode, Timing, WsError};

/// How long a polite close may take before the connection is dropped: a peer
/// that has stopped reading would otherwise hold it.
const CLOSE_WAIT: Duration = Duration::from_secs(1);

/// One item of the consumer's stream.
pub(super) type Item<P> = Result<<P as Protocol>::Event, <P as Protocol>::Error>;

/// One membership change and where its answer goes.
pub(super) struct Command<P: Protocol> {
    pub(super) change: P::Change,
    pub(super) reply: oneshot::Sender<Result<P::Ack, P::Error>>,
}

/// The consumer has dropped the stream.
struct Gone;

/// The task's end of the consumer's queue.
pub(super) enum Events<T> {
    Bounded(mpsc::Sender<T>),
    Unbounded(mpsc::UnboundedSender<T>),
}

/// The consumer's end of the queue.
pub(super) enum Receiver<T> {
    Bounded(mpsc::Receiver<T>),
    Unbounded(mpsc::UnboundedReceiver<T>),
}

/// Room for one item.
enum Slot<'a, T> {
    Bounded(mpsc::Permit<'a, T>),
    Unbounded(&'a mpsc::UnboundedSender<T>),
}

pub(super) fn queue<T>(queue: Queue) -> (Events<T>, Receiver<T>) {
    match queue {
        Queue::Bounded(capacity) => {
            let (tx, rx) = mpsc::channel(capacity.max(1));
            (Events::Bounded(tx), Receiver::Bounded(rx))
        }
        Queue::Unbounded => {
            let (tx, rx) = mpsc::unbounded_channel();
            (Events::Unbounded(tx), Receiver::Unbounded(rx))
        }
    }
}

impl<T> Events<T> {
    /// Queue `item` if there is room now; give it back if there is not.
    fn try_send(&self, item: T) -> Result<Option<T>, Gone> {
        match self {
            Self::Bounded(tx) => match tx.try_reserve() {
                Ok(permit) => {
                    permit.send(item);
                    Ok(None)
                }
                Err(mpsc::error::TrySendError::Full(())) => Ok(Some(item)),
                Err(mpsc::error::TrySendError::Closed(())) => Err(Gone),
            },
            Self::Unbounded(tx) => tx.send(item).map(|()| None).map_err(|_| Gone),
        }
    }

    /// Wait for room for one item.
    async fn reserve(&self) -> Result<Slot<'_, T>, Gone> {
        match self {
            Self::Bounded(tx) => tx.reserve().await.map(Slot::Bounded).map_err(|_| Gone),
            Self::Unbounded(tx) if tx.is_closed() => Err(Gone),
            Self::Unbounded(tx) => Ok(Slot::Unbounded(tx)),
        }
    }

    /// Wait until the consumer drops the stream.
    async fn closed(&self) {
        match self {
            Self::Bounded(tx) => tx.closed().await,
            Self::Unbounded(tx) => tx.closed().await,
        }
    }
}

impl<T> Slot<'_, T> {
    fn send(self, item: T) {
        match self {
            Self::Bounded(permit) => permit.send(item),
            // Only a dropped receiver refuses it, and then nobody reads it.
            Self::Unbounded(tx) => {
                let _ = tx.send(item);
            }
        }
    }
}

impl<T> Receiver<T> {
    pub(super) fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        match self {
            Self::Bounded(rx) => rx.poll_recv(cx),
            Self::Unbounded(rx) => rx.poll_recv(cx),
        }
    }
}

/// How the task begins.
pub(super) enum Start<P: Protocol> {
    /// The first attempt opened the connection.
    Up(Box<Connection>),
    /// The first attempt failed this way, and recovery says to retry.
    Outage(P::Error),
}

/// How one connection's life ended.
enum End<E> {
    /// The consumer dropped the stream.
    Gone,
    /// The membership is no longer wanted.
    NotWanted,
    /// It reached its maximum age.
    Rotation,
    /// Nothing that counts as liveness arrived, or a request went unanswered,
    /// for the staleness window.
    Stale(Duration),
    /// The server closed it, or the stream ended.
    Closed(WsError),
    /// A read, a ping or a decoded frame failed.
    Failed(E),
}

/// How an outage ended.
enum Recovered<E> {
    /// A fresh connection carries the membership.
    Up(Box<Connection>),
    /// The membership stopped being wanted.
    NotWanted,
    /// The consumer dropped the stream.
    Gone,
    /// An attempt failed in a way recovery stops on.
    Fatal(E),
}

/// One connection's clocks.
struct Clock {
    opened: Instant,
    last_ping: Instant,
    /// The last frame that counted as liveness, moved on by any time the
    /// consumer's queue was full.
    last_live: Instant,
}

impl Clock {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            opened: now,
            last_ping: now,
            last_live: now,
        }
    }
}

/// Open a connection and replay the membership on it, each within its bound:
/// the connect timeout, then the staleness window.
///
/// The membership is borrowed mutably only so the future is `Send` without
/// asking `Sync` of it; the replay reads it.
pub(super) async fn attempt<P: Protocol>(
    protocol: &mut P,
    timing: &Timing,
    membership: &mut P::Membership,
) -> Result<Connection, P::Error> {
    let request = protocol.handshake()?;
    let socket = connect(request, timing.connect_timeout).await?;
    let mut connection = protocol.configure(Connection::new(socket, Mode::All));
    protocol.reset();
    match tokio::time::timeout(
        timing.stale_after,
        protocol.replay(&mut connection, membership),
    )
    .await
    {
        Ok(replayed) => replayed.map(|()| connection),
        Err(_) => Err(P::stalled(timing.stale_after)),
    }
}

/// A change made during an outage: recorded and answered now, or held.
fn record<P: Protocol>(
    protocol: &mut P,
    membership: &mut P::Membership,
    held: &mut VecDeque<Command<P>>,
    command: Command<P>,
) {
    let Command { change, reply } = command;
    match protocol.record(membership, change) {
        Offline::Recorded(answer) => {
            let _ = reply.send(answer);
        }
        Offline::Held(change) => held.push_back(Command { change, reply }),
    }
}

/// Send the protocol's ping, if it has one, within the staleness window.
///
/// Nothing is sent on a connection that has already ended, as one does when a
/// request read its close or its failed read: the next read ends it with the
/// server's close, where a send would fail on the dead socket and hide it.
async fn ping<P: Protocol>(
    protocol: &mut P,
    timing: &Timing,
    connection: &mut Connection,
) -> Result<(), P::Error> {
    if connection.is_finished() {
        return Ok(());
    }
    let Some(ping) = protocol.ping(connection) else {
        return Ok(());
    };
    match tokio::time::timeout(timing.stale_after, connection.send(ping)).await {
        Ok(sent) => sent.map_err(P::Error::from),
        Err(_) => Err(P::stalled(timing.stale_after)),
    }
}

/// Send a close frame, but not wait on a peer that has stopped reading.
async fn close_politely(connection: &mut Connection) {
    let _ = tokio::time::timeout(CLOSE_WAIT, connection.close()).await;
}

pub(super) struct Task<P: Protocol> {
    protocol: P,
    timing: Timing,
    membership: P::Membership,
    backoff: Backoff,
    events: Events<Item<P>>,
    commands: mpsc::Receiver<Command<P>>,
    /// Items decoded or declared and not yet queued, oldest first. A full
    /// queue holds them here, and the connection is not read meanwhile.
    pending: VecDeque<Item<P>>,
    /// Changes made during an outage that the protocol held, applied in
    /// order on the next connection.
    held: VecDeque<Command<P>>,
}

impl<P: Protocol> Task<P> {
    pub(super) fn new(
        protocol: P,
        timing: Timing,
        membership: P::Membership,
        events: Events<Item<P>>,
        commands: mpsc::Receiver<Command<P>>,
    ) -> Self {
        Self {
            protocol,
            backoff: timing.backoff(),
            timing,
            membership,
            events,
            commands,
            pending: VecDeque::new(),
            held: VecDeque::new(),
        }
    }

    /// Supervise until the consumer goes, the membership is no longer wanted,
    /// or recovery stops. Whether it stopped on an error.
    pub(super) async fn run(mut self, start: Start<P>) -> bool {
        // The wait the failure that began an outage asked for, which the
        // first attempt waits out (DRIFT R11).
        let mut asked = None;
        let mut up = match start {
            Start::Up(connection) => Some(connection),
            Start::Outage(cause) => {
                tracing::warn!(%cause, "WebSocket connect failed, retrying");
                asked = self.protocol.retry_after(&cause);
                self.declare(Outage::Error(cause));
                None
            }
        };
        // The wait before the next attempt, when it is not the backoff's.
        let mut next_delay = None;
        loop {
            let mut connection = match up.take() {
                Some(connection) => connection,
                None => {
                    let delay = next_delay.take().unwrap_or_else(|| {
                        self.backoff.take_at_least(asked.take().unwrap_or_default())
                    });
                    match self.reconnect(delay).await {
                        Recovered::Up(connection) => {
                            let event = self.protocol.reconnected();
                            self.pending.push_back(Ok(event));
                            connection
                        }
                        Recovered::NotWanted => {
                            let event = self.protocol.reconnected();
                            self.pending.push_back(Ok(event));
                            self.flush().await;
                            return false;
                        }
                        Recovered::Gone => return false,
                        Recovered::Fatal(err) => {
                            self.pending.push_back(Err(err));
                            self.flush().await;
                            return true;
                        }
                    }
                }
            };
            let mut delivered = false;
            let end = self.pump(&mut connection, &mut delivered).await;
            match end {
                End::Gone => {
                    close_politely(&mut connection).await;
                    return false;
                }
                End::NotWanted => {
                    close_politely(&mut connection).await;
                    self.flush().await;
                    return false;
                }
                End::Rotation => {
                    close_politely(&mut connection).await;
                    tracing::debug!("rotating the WebSocket connection at its maximum age");
                    self.backoff.after_connection_ended(delivered);
                    self.declare(Outage::Rotation);
                    next_delay = Some(Duration::ZERO);
                }
                // A lost connection is dropped, not closed: the peer may not
                // be reading, and a close frame would wait on it.
                End::Stale(silent) => {
                    tracing::warn!(?silent, "WebSocket connection stale, reconnecting");
                    self.backoff.after_connection_ended(delivered);
                    self.declare(Outage::Stale { silent });
                }
                End::Closed(closed) => {
                    let (code, reason) = match closed.kind() {
                        Kind::Closed { code, reason } => (code, reason.to_owned()),
                        _ => (None, String::new()),
                    };
                    let err = P::Error::from(closed);
                    if self.protocol.recover(&err) == Recovery::Stop {
                        self.pending.push_back(Err(err));
                        self.flush().await;
                        return true;
                    }
                    tracing::warn!(%err, "WebSocket connection closed, reconnecting");
                    self.backoff.after_connection_ended(delivered);
                    asked = self.protocol.retry_after(&err);
                    self.declare(Outage::Closed { code, reason });
                }
                End::Failed(err) => {
                    if self.protocol.recover(&err) == Recovery::Stop {
                        self.pending.push_back(Err(err));
                        self.flush().await;
                        return true;
                    }
                    tracing::warn!(%err, "WebSocket connection lost, reconnecting");
                    self.backoff.after_connection_ended(delivered);
                    asked = self.protocol.retry_after(&err);
                    self.declare(Outage::Error(err));
                }
            }
        }
    }

    /// Queue the protocol's outage marker, if it declares one.
    fn declare(&mut self, outage: Outage<P::Error>) {
        if let Some(event) = self.protocol.disconnected(outage) {
            self.pending.push_back(Ok(event));
        }
    }

    /// Queue every pending item, waiting for room, with no connection to
    /// keep alive.
    async fn flush(&mut self) {
        while let Some(item) = self.pending.pop_front() {
            match self.events.reserve().await {
                Ok(slot) => slot.send(item),
                Err(Gone) => return,
            }
        }
    }

    /// Wait out `delay`, then connect and replay, repeating with the backoff
    /// while recovery retries. Pending items are queued meanwhile, and
    /// changes go to [`Protocol::record`] during the wait; one made during an
    /// attempt waits for it.
    async fn reconnect(&mut self, mut delay: Duration) -> Recovered<P::Error> {
        loop {
            if !P::wanted(&self.membership) {
                return Recovered::NotWanted;
            }
            let sleep = tokio::time::sleep(delay);
            tokio::pin!(sleep);
            loop {
                tokio::select! {
                    biased;
                    () = self.events.closed() => return Recovered::Gone,
                    slot = self.events.reserve(), if !self.pending.is_empty() => match slot {
                        Ok(slot) => {
                            if let Some(item) = self.pending.pop_front() {
                                slot.send(item);
                            }
                        }
                        Err(Gone) => return Recovered::Gone,
                    },
                    command = self.commands.recv() => match command {
                        None => return Recovered::Gone,
                        Some(command) => {
                            record(&mut self.protocol, &mut self.membership, &mut self.held, command);
                            if !P::wanted(&self.membership) {
                                return Recovered::NotWanted;
                            }
                        }
                    },
                    () = &mut sleep => break,
                }
            }
            let attempted = tokio::select! {
                biased;
                () = self.events.closed() => return Recovered::Gone,
                attempted = attempt(&mut self.protocol, &self.timing, &mut self.membership) => attempted,
            };
            match attempted {
                Ok(connection) => return Recovered::Up(Box::new(connection)),
                Err(err) => match self.protocol.recover(&err) {
                    Recovery::Stop => return Recovered::Fatal(err),
                    Recovery::Reconnect | Recovery::Skip => {
                        // A refusal's `Retry-After` only extends the wait,
                        // up to the ceiling (DRIFT R11).
                        let asked = self.protocol.retry_after(&err).unwrap_or_default();
                        delay = self.backoff.take_at_least(asked);
                        tracing::warn!(%err, ?delay, "WebSocket reconnect failed, retrying");
                    }
                },
            }
        }
    }

    /// Drive one connection until it ends.
    ///
    /// The ping is due on the wall clock, traffic or not, and every wait is
    /// bounded by whichever of the next ping, the staleness deadline and the
    /// rotation comes first. Pending items go to the queue before anything
    /// more is read.
    async fn pump(&mut self, connection: &mut Connection, delivered: &mut bool) -> End<P::Error> {
        let mut clock = Clock::new();
        while let Some(command) = self.held.pop_front() {
            if let Some(end) = self.apply(connection, command, &clock).await {
                return end;
            }
        }
        loop {
            if !self.pending.is_empty() {
                if let Err(end) = self.deliver(connection, &mut clock).await {
                    return end;
                }
            }
            // Frames a request kept are read first: ending the connection
            // now would lose them, and they may be the liveness it lacks.
            let kept = connection.has_kept();
            if let Some(age) = self.timing.max_connection_age {
                if !kept && clock.opened.elapsed() >= age {
                    return End::Rotation;
                }
            }
            if let Some(interval) = self.timing.ping_interval {
                if clock.last_ping.elapsed() >= interval {
                    if let Err(err) = ping(&mut self.protocol, &self.timing, connection).await {
                        return End::Failed(err);
                    }
                    clock.last_ping = Instant::now();
                }
            }
            let silent = clock.last_live.elapsed();
            if !kept && silent >= self.timing.stale_after {
                return End::Stale(silent);
            }
            let wait =
                self.timing
                    .next_wait(clock.last_ping.elapsed(), silent, clock.opened.elapsed());
            tokio::select! {
                biased;
                () = self.events.closed() => return End::Gone,
                command = self.commands.recv() => match command {
                    None => return End::Gone,
                    Some(command) => {
                        if let Some(end) = self.apply(connection, command, &clock).await {
                            return end;
                        }
                    }
                },
                next = tokio::time::timeout(wait, connection.next()) => match next {
                    // A deadline fell due; the top of the loop sees to it.
                    Err(_) => {}
                    Ok(None) => {
                        // A read that failed inside a request ends the stream
                        // with no close; it reads as an end with no code.
                        let closed = connection
                            .closed()
                            .unwrap_or_else(|| WsError::closed(None, String::new()));
                        return End::Closed(closed);
                    }
                    Ok(Some(Err(err))) => return End::Failed(err.into()),
                    Ok(Some(Ok(frame))) => {
                        let decoded = self.protocol.decode(&frame);
                        if self.protocol.is_live(&frame, &decoded) {
                            clock.last_live = Instant::now();
                        }
                        if self.protocol.delivered(&frame, &decoded) {
                            *delivered = true;
                        }
                        match decoded {
                            Decoded::Nothing => {}
                            Decoded::Event(event) => self.pending.push_back(Ok(event)),
                            Decoded::Events(events) => {
                                self.pending.extend(events.into_iter().map(Ok));
                            }
                            Decoded::Error(err) => match self.protocol.recover(&err) {
                                Recovery::Skip => self.pending.push_back(Err(err)),
                                Recovery::Reconnect | Recovery::Stop => return End::Failed(err),
                            },
                        }
                    }
                },
            }
        }
    }

    /// Apply one change on the live connection, within the staleness window.
    /// `Some` ends the connection: the request outlived the window, so the
    /// server's state is unknown, the membership is no longer wanted, or the
    /// consumer went while the change was in flight.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        command: Command<P>,
        clock: &Clock,
    ) -> Option<End<P::Error>> {
        let Command { change, reply } = command;
        let applied = tokio::select! {
            biased;
            () = self.events.closed() => return Some(End::Gone),
            applied = tokio::time::timeout(
                self.timing.stale_after,
                self.protocol.apply(connection, &mut self.membership, change),
            ) => applied,
        };
        match applied {
            Ok(answer) => {
                let _ = reply.send(answer);
            }
            Err(_) => {
                let silent = clock.last_live.elapsed();
                let _ = reply.send(Err(P::stalled(silent)));
                return Some(End::Stale(silent));
            }
        }
        if !P::wanted(&self.membership) {
            return Some(End::NotWanted);
        }
        None
    }

    /// Queue every pending item. While the queue is full nothing is read,
    /// the pings stay on the wall clock, and the time waited does not count
    /// as silence. A ping that fails stops the pinging and ends the
    /// connection once everything pending is queued, so nothing is dropped.
    async fn deliver(
        &mut self,
        connection: &mut Connection,
        clock: &mut Clock,
    ) -> Result<(), End<P::Error>> {
        let mut lost = None;
        while let Some(item) = self.pending.pop_front() {
            let Some(item) = self.events.try_send(item).map_err(|Gone| End::Gone)? else {
                continue;
            };
            let blocked = Instant::now();
            let mut waiting = Some(item);
            while let Some(item) = waiting.take() {
                let next_ping = match (self.timing.ping_interval, &lost) {
                    (Some(interval), None) => interval.saturating_sub(clock.last_ping.elapsed()),
                    _ => Duration::MAX,
                };
                tokio::select! {
                    biased;
                    slot = self.events.reserve() => match slot {
                        Ok(slot) => slot.send(item),
                        Err(Gone) => return Err(End::Gone),
                    },
                    () = tokio::time::sleep(next_ping) => {
                        waiting = Some(item);
                        if let Err(err) = ping(&mut self.protocol, &self.timing, connection).await {
                            lost = Some(err);
                        }
                        clock.last_ping = Instant::now();
                    }
                }
            }
            clock.last_live += blocked.elapsed();
        }
        match lost {
            Some(err) => Err(End::Failed(err)),
            None => Ok(()),
        }
    }
}
