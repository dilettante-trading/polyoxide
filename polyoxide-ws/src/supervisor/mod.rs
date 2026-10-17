//! The generic supervision loop (AD-11): one task per supervised connection,
//! which keeps it alive, replaces it, replays its membership and reports its
//! outages, for any venue that describes its protocol as a [`Protocol`].

mod task;

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
use tokio::sync::{mpsc, oneshot};

use crate::{
    tungstenite::{self, handshake::client::Request, Message},
    Connection, Timing, WsError,
};
use task::{Command, Receiver, Start, Task};

/// How many membership changes may wait for the task before a caller waits
/// to hand one over.
const COMMANDS: usize = 16;

/// What to do after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Recovery {
    /// Replace the connection: back off, connect again and replay the
    /// membership.
    Reconnect,
    /// Yield the error and keep the connection, such as for one frame that
    /// did not decode. A failed connection attempt has no connection to keep,
    /// so there it reads as [`Reconnect`](Self::Reconnect).
    Skip,
    /// Yield the error and end the stream: retrying replays the same failure.
    Stop,
}

/// The queue between the task and the consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Queue {
    /// At most this many items unread (at least one). While it is full the
    /// task reads nothing more from the connection, and still pings.
    Bounded(usize),
    /// No limit: the task never waits for the consumer.
    Unbounded,
}

/// What one frame decodes to.
#[derive(Debug)]
#[non_exhaustive]
pub enum Decoded<E, Err> {
    /// Nothing for the consumer: a control frame, an answer the protocol
    /// reads itself, or a frame it skips.
    Nothing,
    /// One event.
    Event(E),
    /// Several events, in order, such as one the protocol originates ahead of
    /// the update it concerns.
    Events(Vec<E>),
    /// A failure, which [`Protocol::recover`] decides.
    Error(Err),
}

/// What becomes of a membership change made while no connection is up.
#[derive(Debug)]
#[non_exhaustive]
pub enum Offline<C, A, E> {
    /// Recorded in the membership for the replay to apply; the caller gets
    /// this answer now.
    Recorded(Result<A, E>),
    /// Held, and applied with [`Protocol::apply`] once a connection is up and
    /// its membership replayed; the caller waits for that answer.
    Held(C),
}

/// Why a connection went down, for [`Protocol::disconnected`], which owns it,
/// so a protocol's marker can carry the error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outage<E> {
    /// The server closed the connection: with a close frame, whose code and
    /// reason these are, or by ending the stream with none (no code and an
    /// empty reason).
    Closed {
        /// The close code, when a close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A failure: a read, a ping, a decoded frame [`Protocol::recover`] sent
    /// to a reconnect, or the first connection attempt of
    /// [`Supervisor::start`].
    Error(E),
    /// Nothing that counts as liveness arrived for the staleness window, or a
    /// membership change went unanswered for it.
    Stale {
        /// The silence, less any time the consumer's queue was full.
        silent: Duration,
    },
    /// Replaced on purpose at [`Timing::max_connection_age`].
    Rotation,
}

/// A venue's socket protocol, as the [`Supervisor`] drives it.
///
/// The supervisor owns the rules every venue shares (AD-11): pings on the
/// wall clock, staleness, backoff, rotation, the outage markers and the
/// consumer's queue. A protocol says what is its own, and each divergence row
/// stays here, at the venue:
///
/// - **D4, the ping:** [`ping`](Self::ping), sent every
///   [`Timing::ping_interval`] whatever the traffic.
/// - **D5, liveness:** [`is_live`](Self::is_live), which restarts the
///   staleness window.
/// - **D6, a delivered connection:** [`delivered`](Self::delivered), which
///   resets the backoff when the connection ends.
/// - **D8, outage markers:** [`disconnected`](Self::disconnected), or none.
/// - **D19 and the rest of recovery:** [`recover`](Self::recover).
///
/// The hooks that talk to the server, [`replay`](Self::replay) and
/// [`apply`](Self::apply), borrow the [`Connection`], so they use its request
/// correlation. A request is not cancel-safe, so the supervisor bounds each by
/// [`Timing::stale_after`] and replaces a connection that outlives it.
///
/// The trait is generic only, and its async hooks return `impl Future + Send`.
pub trait Protocol: Send + Sized + 'static {
    /// What the stream yields.
    type Event: Send + 'static;
    /// What the stream and the membership calls fail with. Every socket
    /// failure arrives as a [`WsError`] first.
    type Error: std::error::Error + From<WsError> + Send + 'static;
    /// What the connection carries, such as its subscriptions.
    type Membership: Send + 'static;
    /// One change to the membership.
    type Change: Send + 'static;
    /// The answer to a change.
    type Ack: Send + 'static;

    /// The handshake request for one connection attempt, built afresh for
    /// each, so any authentication in it is recomputed.
    fn handshake(&mut self) -> Result<Request, Self::Error>;

    /// Shape each fresh connection before anything is sent on it, such as
    /// with [`Connection::min_request_interval`] for a venue that paces its
    /// requests. By default the connection is left as it is.
    fn configure(&self, connection: Connection) -> Connection {
        connection
    }

    /// Put the membership on a fresh connection, paced as the venue requires.
    /// A failure is [`recover`](Self::recover)ed as a failed attempt.
    fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &Self::Membership,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Make one change on a live connection, and record it in the membership
    /// as far as it took effect. The caller gets the answer; an error keeps
    /// the connection, unless the request outlived the staleness window.
    fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut Self::Membership,
        change: Self::Change,
    ) -> impl Future<Output = Result<Self::Ack, Self::Error>> + Send;

    /// What becomes of a change made during an outage. By default it is held
    /// and [`apply`](Self::apply)'d once a connection is up.
    fn record(
        &mut self,
        membership: &mut Self::Membership,
        change: Self::Change,
    ) -> Offline<Self::Change, Self::Ack, Self::Error> {
        let _ = membership;
        Offline::Held(change)
    }

    /// What one frame means: Text, Binary, Ping, Pong or Close.
    fn decode(&mut self, frame: &Message) -> Decoded<Self::Event, Self::Error>;

    /// Whether this frame, decoded so, proves the connection alive and
    /// restarts the staleness window (D5).
    fn is_live(&self, frame: &Message, decoded: &Decoded<Self::Event, Self::Error>) -> bool;

    /// Whether this frame, decoded so, makes the connection one that
    /// delivered something, which resets the backoff when it ends (D6).
    fn delivered(&self, frame: &Message, decoded: &Decoded<Self::Event, Self::Error>) -> bool;

    /// The ping to send at a ping tick, or `None` for none (D4). It is sent
    /// without awaiting an answer; the answer is a frame like any other.
    fn ping(&mut self, connection: &mut Connection) -> Option<Message>;

    /// Whether the membership still wants a connection. One that stops being
    /// wanted on a healthy connection closes it politely, with no marker, and
    /// ends the stream; during an outage it ends the outage with
    /// [`reconnected`](Self::reconnected) and then the stream.
    fn wanted(membership: &Self::Membership) -> bool {
        let _ = membership;
        true
    }

    /// Whether reconnecting can help after this error. Required, since only
    /// the venue knows its classes; it is what [`recover`](Self::recover)
    /// reads by default.
    fn is_retriable(err: &Self::Error) -> bool;

    /// What to do after this error: by default
    /// [`Reconnect`](Recovery::Reconnect) exactly when
    /// [`is_retriable`](Self::is_retriable) says so, and
    /// [`Stop`](Recovery::Stop) otherwise.
    fn recover(&self, err: &Self::Error) -> Recovery {
        if Self::is_retriable(err) {
            Recovery::Reconnect
        } else {
            Recovery::Stop
        }
    }

    /// How long the server asked the client to wait after this error, such
    /// as a refused upgrade's `Retry-After`; `None` by default.
    ///
    /// The reconnect that follows waits at least this, clamped to
    /// [`Timing::max_backoff`], and never less than the backoff's own step:
    /// a server's wait only extends the delay (DRIFT R11). A venue answers
    /// it from its error's class, the one place that reads the header.
    fn retry_after(&self, err: &Self::Error) -> Option<Duration> {
        let _ = err;
        None
    }

    /// The event announcing an outage, or `None` for a protocol that
    /// declares none (the default). Called at most once per outage.
    fn disconnected(&mut self, outage: Outage<Self::Error>) -> Option<Self::Event> {
        let _ = outage;
        None
    }

    /// The event ending an outage, yielded exactly once per outage: after a
    /// fresh connection has replayed the membership, or when the membership
    /// stopped being wanted during the outage.
    fn reconnected(&mut self) -> Self::Event;

    /// Forget what belongs to the previous connection, before a fresh one
    /// replays the membership.
    fn reset(&mut self) {}

    /// The queue between the task and the consumer.
    fn queue(&self) -> Queue;

    /// The error for a membership change, or a connection attempt, that the
    /// server left unanswered for the staleness window. By default a
    /// [`Kind::NoAnswer`](crate::Kind::NoAnswer).
    fn stalled(silent: Duration) -> Self::Error {
        WsError::no_answer(silent).into()
    }

    /// The error for a membership change, or a [`Supervised::close`], once
    /// the task has stopped. By default a
    /// [`Kind::AlreadyClosed`](crate::Kind::AlreadyClosed).
    fn stopped() -> Self::Error {
        WsError::from(tungstenite::Error::AlreadyClosed).into()
    }
}

/// Starts a supervised connection for a [`Protocol`].
///
/// The task it spawns holds one connection at a time and keeps these rules
/// (AD-11):
///
/// - **Pings on the wall clock.** A ping goes out every
///   [`Timing::ping_interval`] whatever the traffic, and also while the
///   consumer's queue is full: the task then races the queue's space against
///   the ping timer and reads nothing more, and the time it waits does not
///   count as silence. None goes out while a membership change is in flight,
///   which [`Timing::stale_after`] bounds.
/// - **Staleness.** Silence for [`Timing::stale_after`], by the protocol's
///   liveness rule, replaces the connection.
/// - **No drops.** No decoded event is dropped; a bounded queue holds the
///   task back instead.
/// - **Outage markers.** At most one [`Protocol::disconnected`] per outage,
///   and exactly one [`Protocol::reconnected`], even when the membership
///   stops being wanted during the outage.
/// - **Not wanted.** A membership that stops being wanted on a healthy
///   connection closes it politely, with no marker, and ends the stream.
/// - **Rotation.** A connection older than [`Timing::max_connection_age`] is
///   closed politely and replaced at once.
/// - **Membership during an outage.** A change made during an outage goes to
///   [`Protocol::record`], and a held one is applied after the reconnect.
/// - **The consumer gone.** Dropping [`Supervised`] ends the task promptly,
///   and [`Supervised::close`] does not wait on a consumer that never read.
///
/// Reconnects wait out [`Timing::backoff`]'s schedule, with no cap on
/// attempts, or the [`Protocol::retry_after`] of the failure before them when
/// that is longer, up to [`Timing::max_backoff`] (DRIFT R11); each attempt is
/// bounded by [`Timing::connect_timeout`]. The task logs under the
/// `polyoxide_ws` target.
///
/// The basis is one socket crate's former supervised tier (`ws/supervised.rs`,
/// deleted in Story 4.6), with another socket crate's former loop (replaced
/// in Story 4.7 by a router over one supervisor per path) for rotation,
/// outage markers, changes recorded during an outage, the start in an outage
/// and the consumer's departure. Neither kept pinging behind a full queue.
#[derive(Debug)]
pub struct Supervisor<P: Protocol> {
    protocol: P,
    timing: Timing,
}

impl<P: Protocol> Supervisor<P> {
    /// A supervisor for `protocol` on `timing`.
    pub fn new(protocol: P, timing: Timing) -> Self {
        Self { protocol, timing }
    }

    /// Connect, replay `membership`, and start supervising. Any failure of
    /// that first attempt is returned, so a setup that is wrong is reported
    /// rather than retried.
    pub async fn connect(
        mut self,
        mut membership: P::Membership,
    ) -> Result<Supervised<P>, P::Error> {
        let live = task::attempt(&mut self.protocol, &self.timing, &mut membership).await?;
        Ok(self.spawn(membership, Start::Up(Box::new(live))))
    }

    /// As [`connect`](Self::connect), except that a first failure
    /// [`Protocol::recover`] would retry starts the stream in an outage: the
    /// protocol's [`disconnected`](Protocol::disconnected) for it, then the
    /// backoff and the attempts that follow. Only a failure it would
    /// [`Stop`](Recovery::Stop) on is returned.
    pub async fn start(mut self, mut membership: P::Membership) -> Result<Supervised<P>, P::Error> {
        match task::attempt(&mut self.protocol, &self.timing, &mut membership).await {
            Ok(live) => Ok(self.spawn(membership, Start::Up(Box::new(live)))),
            Err(err) => match self.protocol.recover(&err) {
                Recovery::Stop => Err(err),
                Recovery::Reconnect | Recovery::Skip => {
                    Ok(self.spawn(membership, Start::Outage(err)))
                }
            },
        }
    }

    fn spawn(self, membership: P::Membership, start: Start<P>) -> Supervised<P> {
        let (events, receiver) = task::queue(self.protocol.queue());
        let (commands, commands_rx) = mpsc::channel(COMMANDS);
        let task = Task::new(self.protocol, self.timing, membership, events, commands_rx);
        Supervised {
            events: receiver,
            commands,
            task: tokio::spawn(task.run(start)),
        }
    }
}

/// A supervised connection: a `Stream` of the protocol's events, and of the
/// errors it skips past or stops on, that survives drops, stalls and
/// rotations by reconnecting and replaying its membership.
///
/// It ends after an error [`Protocol::recover`] stops on, which it yields
/// first, or when the membership stops being wanted. Dropping it ends the
/// task, closing the connection politely.
pub struct Supervised<P: Protocol> {
    events: Receiver<task::Item<P>>,
    commands: mpsc::Sender<Command<P>>,
    /// Whether the task stopped on an error.
    task: tokio::task::JoinHandle<bool>,
}

impl<P: Protocol> std::fmt::Debug for Supervised<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Supervised")
            .field("running", &!self.task.is_finished())
            .finish_non_exhaustive()
    }
}

impl<P: Protocol> Supervised<P> {
    /// A handle for changing the membership while the stream runs.
    pub fn membership(&self) -> MembershipHandle<P> {
        MembershipHandle {
            commands: self.commands.clone(),
        }
    }

    /// Close the connection politely and stop the task, without waiting on
    /// the consumer's queue. Fails with [`Protocol::stopped`] if the task had
    /// already stopped on an error, which the stream yielded. Every
    /// [`MembershipHandle`] answers [`Protocol::stopped`] afterwards.
    pub async fn close(self) -> Result<(), P::Error> {
        let Self {
            events,
            commands,
            task,
        } = self;
        // The task sees the queue's receiver go, wherever it is waiting,
        // a full queue included.
        drop(events);
        drop(commands);
        match task.await {
            Ok(false) => Ok(()),
            Ok(true) | Err(_) => Err(P::stopped()),
        }
    }
}

impl<P: Protocol> Stream for Supervised<P> {
    type Item = Result<P::Event, P::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().events.poll_recv(cx)
    }
}

/// Changes the membership of a running [`Supervised`].
///
/// A call waits for the task to take the change and for its answer. The task
/// takes changes between frames on a live connection, and during an outage's
/// backoff, but not while the consumer's queue is full: a call made from the
/// task that reads the stream can then wait for ever. Make changes from
/// another task, or keep reading while one is pending.
pub struct MembershipHandle<P: Protocol> {
    commands: mpsc::Sender<Command<P>>,
}

impl<P: Protocol> Clone for MembershipHandle<P> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
        }
    }
}

impl<P: Protocol> std::fmt::Debug for MembershipHandle<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MembershipHandle")
            .field("running", &!self.commands.is_closed())
            .finish()
    }
}

impl<P: Protocol> MembershipHandle<P> {
    /// Make one change, and wait for its answer: from
    /// [`Protocol::apply`] on a live connection, from [`Protocol::record`]
    /// during an outage, [`Protocol::stalled`] for a request left unanswered
    /// for the staleness window, or [`Protocol::stopped`] once the task has
    /// stopped.
    pub async fn apply(&self, change: P::Change) -> Result<P::Ack, P::Error> {
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(Command { change, reply })
            .await
            .map_err(|_| P::stopped())?;
        answer.await.map_err(|_| P::stopped())?
    }
}

#[cfg(test)]
mod tests;
