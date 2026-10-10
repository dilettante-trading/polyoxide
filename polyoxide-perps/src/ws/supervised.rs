//! perps' own supervision loop, superseded by the kit's `Supervisor` on
//! [`PerpsProtocol`](crate::ws::PerpsProtocol): keep-alive, staleness
//! detection, reconnect with resubscribe, and live membership changes, on a
//! task behind channels. Nothing builds it any more; it is deleted next.

use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use tokio::{
    sync::{mpsc, oneshot},
    time::{timeout, Instant},
};

use crate::ws::{
    channel::Channel,
    client::PerpsWs,
    error::{PerpsWsError, Recovery},
    event::{Event, Frame},
    WS_URL,
};

/// Default keep-alive cadence against the host's 60 s idle close.
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(20);
/// Default silence before a connection is presumed dead. A pong counts as
/// an inbound frame, so a quiet but answering connection is not torn down.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(30);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);
/// Retries of a subscribe the server refused with `message_rate_limited`.
pub(crate) const MAX_SUBSCRIBE_RETRIES: usize = 3;
/// Events the consumer may leave unread before the task blocks.
const EVENT_BUFFER: usize = 1024;

/// The reconnect delay schedule: doubling to a ceiling, reset after a
/// connection that delivered at least one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Backoff {
    initial: Duration,
    max: Duration,
    next: Duration,
}

/// The shortest delay. Zero would never grow, so a dead server would be
/// retried on every timer tick.
const MIN_BACKOFF: Duration = Duration::from_millis(1);

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

/// Builder for a supervised connection.
#[derive(Debug, Clone)]
pub struct PerpsWsBuilder {
    url: String,
    ping_interval: Duration,
    stale_after: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl Default for PerpsWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PerpsWsBuilder {
    /// Defaults: the production host, a ping every 20 s, stale after 30 s of
    /// silence, reconnect backoff from 500 ms doubling to 60 s.
    pub fn new() -> Self {
        Self {
            url: WS_URL.to_owned(),
            ping_interval: DEFAULT_PING_INTERVAL,
            stale_after: DEFAULT_STALE_AFTER,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
        }
    }

    /// Connect to a different endpoint.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// How often to send the application ping.
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Silence after which the connection is presumed dead and replaced. A
    /// pong counts as an inbound frame, so a connection that is quiet but
    /// answers pings stays up.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect delay schedule: `initial`, doubling to `max`. There is no
    /// cap on reconnect attempts; only the delay is capped, at `max`. Both
    /// are raised to at least 1 ms, and an `initial` above `max` is lowered
    /// to `max`.
    ///
    /// The same schedule spaces the three retries of a rate-limited
    /// subscribe, and those retries share the request's `stale_after`
    /// bound, so `initial × 7` (the three delays, `1×`, `2×` and `4×`) must
    /// stay under `stale_after` or a rate-limited subscribe is reported as
    /// a stall and becomes a reconnect. No pings are sent during those
    /// retries, so keep `initial` well under the host's 60 s idle limit.
    pub fn backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max;
        self
    }

    /// Open the connection, subscribe, and start supervising.
    ///
    /// A channel the server refuses fails here with
    /// [`PerpsWsError::Refused`]; later refusals arrive through the
    /// [`MembershipHandle`].
    pub async fn connect(
        self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<SupervisedPerpsWs, PerpsWsError> {
        let channels: Vec<Channel> = channels.into_iter().collect();
        let stream = PerpsWs::connect_to(&self.url, channels.clone()).await?;
        let (events_tx, events_rx) = mpsc::channel(EVENT_BUFFER);
        let (commands_tx, commands_rx) = mpsc::channel(16);
        let task = tokio::spawn(run(self, stream, channels, commands_rx, events_tx));
        Ok(SupervisedPerpsWs {
            events: events_rx,
            commands: commands_tx,
            task,
        })
    }
}

enum Command {
    Subscribe(Vec<Channel>, oneshot::Sender<Result<(), PerpsWsError>>),
    Unsubscribe(Vec<Channel>, oneshot::Sender<Result<(), PerpsWsError>>),
    /// Close the socket and end the task. Sent by
    /// [`SupervisedPerpsWs::close`]; a dropped sender is not enough because
    /// every [`MembershipHandle`] holds a clone.
    Close,
}

/// Changes the subscription set of a running [`SupervisedPerpsWs`].
///
/// A call waits for the task to take the command and for the server's
/// acknowledgement. The task cannot take a command while it is parked in a
/// full event buffer (1024 unread events), so a call made from the task
/// that consumes the stream can deadlock: issue membership changes from
/// another task, or keep draining the stream while awaiting them.
#[derive(Debug, Clone)]
pub struct MembershipHandle {
    commands: mpsc::Sender<Command>,
}

impl MembershipHandle {
    /// Subscribe to more channels. A `message_rate_limited` refusal is
    /// retried with backoff before being reported.
    ///
    /// When the server accepts some channels and refuses others in one
    /// request, the accepted ones are live on the current connection but
    /// are not replayed after a reconnect: the request failed as a whole,
    /// and re-issuing it with the accepted channels is the caller's call.
    pub async fn subscribe(
        &self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<(), PerpsWsError> {
        self.send(|reply| Command::Subscribe(channels.into_iter().collect(), reply))
            .await
    }

    /// Unsubscribe from channels.
    pub async fn unsubscribe(
        &self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<(), PerpsWsError> {
        self.send(|reply| Command::Unsubscribe(channels.into_iter().collect(), reply))
            .await
    }

    async fn send(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<(), PerpsWsError>>) -> Command,
    ) -> Result<(), PerpsWsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.commands
            .send(make(reply_tx))
            .await
            .map_err(|_| PerpsWsError::Stopped)?;
        reply_rx.await.map_err(|_| PerpsWsError::Stopped)?
    }
}

/// A supervised connection: a `Stream` of [`Event`]s that survives drops
/// and stalls by reconnecting and replaying its subscriptions.
///
/// The consumer must keep up: after 1024 unread events the task blocks,
/// and a blocked task neither pings, notices a stall, nor takes a
/// [`MembershipHandle`] command. Awaiting a membership change on the same
/// task that consumes the stream can therefore deadlock once the buffer
/// is full; make the call from another task, or keep draining while it
/// is pending.
///
/// Superseded by [`crate::ws::SupervisedPerpsWs`], the kit's supervisor on
/// perps' protocol; this loop is deleted once nothing builds it.
pub struct SupervisedPerpsWs {
    events: mpsc::Receiver<Result<Event, PerpsWsError>>,
    commands: mpsc::Sender<Command>,
    task: tokio::task::JoinHandle<Result<(), PerpsWsError>>,
}

impl std::fmt::Debug for SupervisedPerpsWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupervisedPerpsWs")
            .field("running", &!self.task.is_finished())
            .finish_non_exhaustive()
    }
}

impl SupervisedPerpsWs {
    /// A handle for changing membership while the stream runs.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            commands: self.commands.clone(),
        }
    }

    /// Close the connection and stop the task. Every [`MembershipHandle`]
    /// reports [`PerpsWsError::Stopped`] afterwards.
    pub async fn close(self) -> Result<(), PerpsWsError> {
        let Self {
            events,
            commands,
            task,
        } = self;
        // Dropping the receiver first unblocks a task parked in
        // `events.send` behind a full buffer, which would otherwise never
        // read the command.
        drop(events);
        // A failed send means the task has already ended: the reason went
        // out on the events stream and the task's result is `Stopped`.
        let _ = commands.send(Command::Close).await;
        drop(commands);
        match task.await {
            Ok(result) => result,
            Err(_) => Err(PerpsWsError::Stopped),
        }
    }
}

impl Stream for SupervisedPerpsWs {
    type Item = Result<Event, PerpsWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.events.poll_recv(cx)
    }
}

/// The task: pump one connection until it fails, then reconnect.
async fn run(
    config: PerpsWsBuilder,
    mut stream: PerpsWs,
    mut channels: Vec<Channel>,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Result<Event, PerpsWsError>>,
) -> Result<(), PerpsWsError> {
    let mut backoff = Backoff::new(config.initial_backoff, config.max_backoff);
    loop {
        let mut delivered = false;
        match pump(
            &config,
            &mut stream,
            &mut channels,
            &mut commands,
            &events,
            &mut delivered,
        )
        .await
        {
            Ok(()) => {
                let _ = stream.close().await;
                return Ok(());
            }
            Err(err) if err.recovery() == Recovery::Reconnect => {
                backoff.after_connection_ended(delivered);
                tracing::warn!(%err, "perps WebSocket lost, reconnecting");
                match reconnect(&config, &channels, &mut backoff, &events).await {
                    Some(Ok(fresh)) => stream = fresh,
                    Some(Err(fatal)) => {
                        let _ = events.send(Err(fatal)).await;
                        return Err(PerpsWsError::Stopped);
                    }
                    None => return Ok(()),
                }
                if events.send(Ok(Event::Reconnected)).await.is_err() {
                    let _ = stream.close().await;
                    return Ok(());
                }
            }
            Err(fatal) => {
                let _ = events.send(Err(fatal)).await;
                return Err(PerpsWsError::Stopped);
            }
        }
    }
}

/// Wait out the backoff and open a fresh connection, repeating while the
/// failure is one a retry can fix. `None` means the consumer went away
/// meanwhile (`close`, or the stream dropped) and no connection was opened.
///
/// Membership commands sent during the wait stay queued for the new
/// connection; the consumer's departure is observed through the events
/// channel instead, which `close` drops before anything else.
async fn reconnect(
    config: &PerpsWsBuilder,
    channels: &[Channel],
    backoff: &mut Backoff,
    events: &mpsc::Sender<Result<Event, PerpsWsError>>,
) -> Option<Result<PerpsWs, PerpsWsError>> {
    loop {
        if events.is_closed() {
            return None;
        }
        let delay = backoff.take();
        tracing::debug!(?delay, "waiting before the next perps WebSocket attempt");
        let attempt = async {
            tokio::time::sleep(delay).await;
            PerpsWs::connect_to(&config.url, channels.to_vec()).await
        };
        tokio::select! {
            biased;
            () = events.closed() => return None,
            result = attempt => match result {
                Ok(fresh) => return Some(Ok(fresh)),
                Err(again) if again.recovery() == Recovery::Reconnect => {
                    tracing::warn!(%again, "perps WebSocket reconnect failed, retrying");
                }
                Err(fatal) => return Some(Err(fatal)),
            },
        }
    }
}

/// Drive one connection until it fails or the consumer closes.
///
/// `Ok(())` means the consumer is gone: it sent `Close`, dropped every
/// handle, or dropped the stream. `last_sq` is per connection: a reconnect
/// replays snapshots with fresh sequence stamps that must not be reported
/// as a regression.
///
/// The ping is due on the wall clock, traffic or not: the host idle-closes
/// on 60 s without an *inbound* message, so a busy connection that never
/// pings is closed like a silent one. Every wait is bounded by whichever
/// of the next ping and the staleness deadline comes first.
async fn pump(
    config: &PerpsWsBuilder,
    stream: &mut PerpsWs,
    channels: &mut Vec<Channel>,
    commands: &mut mpsc::Receiver<Command>,
    events: &mpsc::Sender<Result<Event, PerpsWsError>>,
    delivered: &mut bool,
) -> Result<(), PerpsWsError> {
    let mut last_frame = Instant::now();
    let mut last_ping = Instant::now();
    let mut last_sq: HashMap<Channel, u64> = HashMap::new();

    loop {
        if last_ping.elapsed() >= config.ping_interval {
            // `ping` reads until the pong, which on a socket that is open
            // but dead is forever. It gets one interval, and never more
            // than the rest of the staleness window; a pong that arrives
            // late is dropped by the stream as an uncorrelated response.
            let remaining = config.stale_after.saturating_sub(last_frame.elapsed());
            match timeout(config.ping_interval.min(remaining), stream.ping()).await {
                Ok(pong) => {
                    pong?;
                    last_ping = Instant::now();
                    last_frame = last_ping;
                }
                Err(_) => {
                    let elapsed = last_frame.elapsed();
                    if elapsed >= config.stale_after {
                        return Err(PerpsWsError::Stalled { elapsed });
                    }
                    last_ping = Instant::now();
                }
            }
        }
        // A wait, not an `Instant`: a limit such as `Duration::MAX` overflows
        // when added to one, and `timeout` falls back to a far-future deadline.
        let wait = config
            .ping_interval
            .saturating_sub(last_ping.elapsed())
            .min(config.stale_after.saturating_sub(last_frame.elapsed()));

        tokio::select! {
            biased;
            command = commands.recv() => match command {
                None | Some(Command::Close) => return Ok(()),
                Some(Command::Subscribe(add, reply)) => {
                    let result = bounded(config, &last_frame, subscribe_with_retry(config, stream, &add)).await;
                    match result {
                        Ok(Ok(())) => {
                            for c in add {
                                if !channels.contains(&c) {
                                    channels.push(c);
                                }
                            }
                            let _ = reply.send(Ok(()));
                        }
                        Ok(refused) => {
                            let _ = reply.send(refused);
                        }
                        Err(stalled) => {
                            let elapsed = stalled.elapsed;
                            let _ = reply.send(Err(PerpsWsError::Stalled { elapsed }));
                            return Err(PerpsWsError::Stalled { elapsed });
                        }
                    }
                }
                Some(Command::Unsubscribe(remove, reply)) => {
                    let result = bounded(config, &last_frame, stream.unsubscribe(remove.iter().copied())).await;
                    match result {
                        Ok(Ok(())) => {
                            channels.retain(|c| !remove.contains(c));
                            let _ = reply.send(Ok(()));
                        }
                        Ok(refused) => {
                            let _ = reply.send(refused);
                        }
                        Err(stalled) => {
                            let elapsed = stalled.elapsed;
                            let _ = reply.send(Err(PerpsWsError::Stalled { elapsed }));
                            return Err(PerpsWsError::Stalled { elapsed });
                        }
                    }
                }
            },
            next = timeout(wait, stream.next()) => match next {
                Err(_) => {
                    // A consumer that dropped the stream without `close`
                    // is only noticed when something is sent to it; check
                    // here so an idle connection does not ping forever.
                    if events.is_closed() {
                        return Ok(());
                    }
                    let elapsed = last_frame.elapsed();
                    if elapsed >= config.stale_after {
                        return Err(PerpsWsError::Stalled { elapsed });
                    }
                    // Otherwise the ping is due; the top of the loop sends it.
                }
                Ok(None) => return Err(PerpsWsError::ConnectionClosed),
                Ok(Some(Err(err))) if err.recovery() == Recovery::SkipFrame => {
                    last_frame = Instant::now();
                    if events.send(Err(err)).await.is_err() {
                        return Ok(());
                    }
                }
                Ok(Some(Err(err))) => return Err(err),
                Ok(Some(Ok(frame))) => {
                    last_frame = Instant::now();
                    *delivered = true;
                    if let Frame::Update(update) = &frame {
                        if let Some(previous) = last_sq.insert(update.channel, update.sq) {
                            if update.sq < previous {
                                let regression = Event::SequenceRegressed {
                                    channel: update.channel,
                                    previous,
                                    got: update.sq,
                                };
                                if events.send(Ok(regression)).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    if events.send(Ok(Event::from(frame))).await.is_err() {
                        return Ok(());
                    }
                }
            },
        }
    }
}

/// How long a membership command has waited for the socket.
struct HandlerStalled {
    elapsed: Duration,
}

/// Bound a command handler's socket work by the staleness window. A
/// request the server never answers must not park the task forever.
async fn bounded<F>(
    config: &PerpsWsBuilder,
    last_frame: &Instant,
    work: F,
) -> Result<Result<(), PerpsWsError>, HandlerStalled>
where
    F: std::future::Future<Output = Result<(), PerpsWsError>>,
{
    match timeout(config.stale_after, work).await {
        Ok(result) => Ok(result),
        Err(_) => Err(HandlerStalled {
            elapsed: last_frame.elapsed(),
        }),
    }
}

/// Subscribe, retrying a `message_rate_limited` refusal with backoff.
async fn subscribe_with_retry(
    config: &PerpsWsBuilder,
    stream: &mut PerpsWs,
    channels: &[Channel],
) -> Result<(), PerpsWsError> {
    let mut backoff = Backoff::new(config.initial_backoff, config.max_backoff);
    let mut attempt = 0;
    loop {
        match stream.subscribe(channels.iter().copied()).await {
            Err(err) if err.recovery() == Recovery::Retry && attempt < MAX_SUBSCRIBE_RETRIES => {
                attempt += 1;
                let delay = backoff.take();
                tracing::warn!(%err, ?delay, attempt, "subscribe rate limited, retrying");
                tokio::time::sleep(delay).await;
            }
            other => return other,
        }
    }
}
