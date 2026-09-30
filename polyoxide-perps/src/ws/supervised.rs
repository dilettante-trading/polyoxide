//! The supervised tier: keep-alive, staleness detection, reconnect with
//! resubscribe, and live membership changes, on a task behind channels.

use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use tokio::{
    sync::{mpsc, oneshot},
    time::{timeout, timeout_at, Instant},
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

impl Backoff {
    fn new(initial: Duration, max: Duration) -> Self {
        let initial = initial.min(max);
        Self {
            initial,
            max,
            next: initial,
        }
    }

    fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(self.max);
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

    /// Reconnect delay schedule: `initial`, doubling to `max`. The same
    /// schedule spaces the retries of a rate-limited subscribe, during which
    /// no pings are sent, so keep `initial` well under the host's 60 s idle
    /// limit.
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
/// and a blocked task neither pings nor notices a stall.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_perps::{types::InstrumentId, ws::{Channel, Event, PerpsWsBuilder}};
///
/// # async fn example() -> Result<(), polyoxide_perps::ws::PerpsWsError> {
/// let mut ws = PerpsWsBuilder::new().connect([Channel::Book(InstrumentId(1), Default::default())]).await?;
/// let membership = ws.membership();
/// while let Some(event) = ws.next().await {
///     match event? {
///         Event::Update(update) => println!("{} sq={}", update.channel, update.sq),
///         Event::Reconnected | Event::SequenceRegressed { .. } => println!("resync the book"),
///         _ => {}
///     }
/// }
/// # membership.subscribe([Channel::Bbo(InstrumentId(2))]).await?;
/// # Ok(())
/// # }
/// ```
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
        let deadline = (last_ping + config.ping_interval).min(last_frame + config.stale_after);

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
            next = timeout_at(deadline, stream.next()) => match next {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        types::InstrumentId,
        ws::test_server::{Script, ScriptedServer},
    };
    use futures_util::StreamExt;
    use std::time::Duration;

    fn bbo(sq: u64) -> String {
        format!(
            r#"{{"ch":"bbo::1","ts":1,"ets":1,"sq":{sq},"data":{{"iid":1,"bp":"1","bq":"1","ap":"2","aq":"1"}}}}"#
        )
    }

    fn fast() -> PerpsWsBuilder {
        PerpsWsBuilder::new()
            .ping_interval(Duration::from_millis(50))
            .stale_after(Duration::from_millis(300))
            .backoff(Duration::from_millis(10), Duration::from_millis(20))
    }

    async fn next_event(ws: &mut SupervisedPerpsWs) -> Event {
        tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("an event within 2 s")
            .expect("stream open")
            .expect("not an error")
    }

    #[test]
    fn defaults_match_the_documented_cadences() {
        let b = PerpsWsBuilder::new();
        assert_eq!(b.url, WS_URL);
        assert_eq!(b.ping_interval, Duration::from_secs(20));
        assert_eq!(b.stale_after, Duration::from_secs(30));
        assert_eq!(b.initial_backoff, Duration::from_millis(500));
        assert_eq!(b.max_backoff, Duration::from_secs(60));
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

    #[tokio::test]
    async fn a_dropped_connection_is_replaced_and_resubscribed() {
        let server = ScriptedServer::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(2)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert_eq!(server.connection_count(), 2);
        let subs = server.subscriptions();
        assert_eq!(subs.len(), 2);
        assert!(
            subs[1].contains("bbo::1"),
            "reconnect did not resubscribe: {}",
            subs[1]
        );
    }

    #[tokio::test]
    async fn silence_is_a_stall_and_pings_go_out_meanwhile() {
        // A socket that answers pings is alive by definition, so the first
        // connection must not answer them for silence to mean a stall.
        let server = ScriptedServer::start(vec![
            Script {
                answer_pings: false,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(5)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 5));
        let pings = server
            .client_frames()
            .iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .count();
        assert!(
            pings >= 2,
            "expected pings during the silent 300 ms, saw {pings}"
        );
    }

    #[tokio::test]
    async fn membership_changes_reach_the_server_and_survive_a_reconnect() {
        // The first connection answers membership requests but not pings,
        // so once it goes quiet it is a stall and gets replaced.
        let server = ScriptedServer::start(vec![
            Script {
                pushes: vec![bbo(1)],
                answer_pings: false,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(2)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let handle = ws.membership();
        assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
        handle
            .subscribe([Channel::Trades(InstrumentId(2))])
            .await
            .unwrap();
        handle
            .unsubscribe([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        server
            .wait_for("sub and unsub frames", |s| {
                s.client_frames()
                    .iter()
                    .filter(|f| f.contains("trades::2") || f.contains(r#""req":"unsub""#))
                    .count()
                    >= 2
            })
            .await;
        // Force a reconnect by staleness (the first script sends nothing more).
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        let subs = server.subscriptions();
        assert!(
            subs[1].contains("trades::2") && !subs[1].contains("bbo::1"),
            "resubscribed with the wrong set: {}",
            subs[1]
        );
    }

    #[tokio::test]
    async fn a_sequence_going_backwards_is_reported() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![bbo(5), bbo(3)],
            ..Default::default()
        }])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 5));
        assert!(matches!(
            next_event(&mut ws).await,
            Event::SequenceRegressed {
                previous: 5,
                got: 3,
                ..
            }
        ));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 3));
    }

    #[tokio::test]
    async fn a_rate_limited_subscribe_is_retried_then_reported() {
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("trades::2".into(), "message_rate_limited".into())],
            ..Default::default()
        }])
        .await;
        let ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let err = ws
            .membership()
            .subscribe([Channel::Trades(InstrumentId(2))])
            .await
            .unwrap_err();
        assert!(matches!(err, PerpsWsError::Refused { .. }));
        let attempts = server
            .client_frames()
            .iter()
            .filter(|f| f.contains("trades::2"))
            .count();
        assert_eq!(attempts, 1 + MAX_SUBSCRIBE_RETRIES);
    }

    #[tokio::test]
    async fn a_refused_channel_at_connect_is_an_immediate_error() {
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("bbo::1".into(), "invalid channel".into())],
            ..Default::default()
        }])
        .await;
        let err = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap_err();
        assert!(matches!(err, PerpsWsError::Refused { .. }));
    }

    #[tokio::test]
    async fn close_ends_the_stream_and_the_handle() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let handle = ws.membership();
        ws.close().await.unwrap();
        server
            .wait_for("a close frame", |s| s.close_count() == 1)
            .await;
        assert!(matches!(
            handle.subscribe([Channel::Trades(InstrumentId(2))]).await,
            Err(PerpsWsError::Stopped)
        ));
    }

    #[tokio::test]
    async fn a_quiet_connection_that_answers_pings_is_alive() {
        // No pushes at all for three staleness windows; the pongs keep it.
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let quiet = tokio::time::timeout(Duration::from_millis(900), ws.next()).await;
        assert!(quiet.is_err(), "expected silence, got {quiet:?}");
        assert_eq!(server.connection_count(), 1);
        let pings = server
            .client_frames()
            .iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .count();
        assert!(pings >= 10, "expected pings on cadence, saw {pings}");
    }

    #[tokio::test]
    async fn pings_keep_going_under_steady_traffic() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![bbo(1)],
            push_every: Some(Duration::from_millis(10)),
            ..Default::default()
        }])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        let mut updates = 0;
        while let Ok(Some(event)) = tokio::time::timeout_at(deadline, ws.next()).await {
            match event.unwrap() {
                Event::Update(_) => updates += 1,
                other => panic!("unexpected {other:?}"),
            }
        }
        let pings = server
            .client_frames()
            .iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .count();
        assert!(updates >= 20, "traffic stopped flowing: {updates} updates");
        assert!(pings >= 5, "expected pings under traffic, saw {pings}");
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn close_does_not_wait_on_a_consumer_that_never_read() {
        // More pushes than the event buffer holds, and nobody reading: the
        // task is parked in `events.send`. `close` must still return.
        let pushes: Vec<String> = (1..=1100).map(bbo).collect();
        let server = ScriptedServer::start(vec![Script {
            pushes,
            ..Default::default()
        }])
        .await;
        let ws = fast()
            .url(&server.url)
            .stale_after(Duration::from_secs(5))
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        server
            .wait_for("the buffer to fill", |s| !s.client_frames().is_empty())
            .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let closed = tokio::time::timeout(Duration::from_secs(2), ws.close()).await;
        assert!(matches!(closed, Ok(Ok(()))), "close: {closed:?}");
    }

    #[tokio::test]
    async fn close_is_honoured_during_the_reconnect_backoff() {
        let server = ScriptedServer::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                reject_handshake: true,
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .backoff(Duration::from_secs(3), Duration::from_secs(3))
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
        // The server has closed; give the task a moment to enter its sleep.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let started = tokio::time::Instant::now();
        let closed = tokio::time::timeout(Duration::from_secs(2), ws.close()).await;
        assert!(matches!(closed, Ok(Ok(()))), "close: {closed:?}");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "close waited out the backoff: {:?}",
            started.elapsed()
        );
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn a_membership_change_the_server_never_answers_is_a_stall() {
        let server = ScriptedServer::start(vec![
            Script {
                answer_subscribes_after_first: false,
                ..Default::default()
            },
            Script::default(),
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let handle = ws.membership();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            handle.subscribe([Channel::Trades(InstrumentId(2))]),
        )
        .await
        .expect("the subscribe returns");
        assert!(
            matches!(result, Err(PerpsWsError::Stalled { .. })),
            "{result:?}"
        );
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
    }

    #[tokio::test]
    async fn a_failed_reconnect_attempt_is_retried() {
        let server = ScriptedServer::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                reject_handshake: true,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(2)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert_eq!(server.connection_count(), 3);
    }

    #[tokio::test]
    async fn a_refusal_on_reconnect_is_fatal_and_ends_the_stream() {
        let server = ScriptedServer::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                refuse: vec![("bbo::1".into(), "invalid channel".into())],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        let err = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("an item")
            .expect("stream open");
        assert!(matches!(err, Err(PerpsWsError::Refused { .. })), "{err:?}");
        let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("the end");
        assert!(end.is_none(), "{end:?}");
    }
}
