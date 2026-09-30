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
/// Default silence before a connection is presumed dead. Tickers and books
/// push several frames a second, so this is generous; raise it for a
/// subscription that is only klines on a quiet instrument.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(30);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);
/// Retries of a subscribe the server refused with `message_rate_limited`.
pub(crate) const MAX_SUBSCRIBE_RETRIES: usize = 3;
/// Events the consumer may leave unread before the task blocks. A blocked
/// task stops pinging, so a consumer must keep up.
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

    /// Silence after which the connection is presumed dead and replaced.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect delay schedule: `initial`, doubling to `max`. Also the
    /// schedule for retrying a rate-limited subscribe.
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
        // A failed send means the task has already ended; its result says why.
        let _ = self.commands.send(Command::Close).await;
        drop(self.commands);
        match self.task.await {
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
                loop {
                    let delay = backoff.take();
                    tracing::debug!(?delay, "waiting before the next perps WebSocket attempt");
                    tokio::time::sleep(delay).await;
                    match PerpsWs::connect_to(&config.url, channels.clone()).await {
                        Ok(fresh) => {
                            stream = fresh;
                            break;
                        }
                        Err(again) if again.recovery() == Recovery::Reconnect => {
                            tracing::warn!(%again, "perps WebSocket reconnect failed, retrying");
                        }
                        Err(fatal) => {
                            let _ = events.send(Err(fatal)).await;
                            return Err(PerpsWsError::Stopped);
                        }
                    }
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

/// Drive one connection until it fails or the consumer closes.
///
/// `Ok(())` means the consumer is gone: it sent `Close`, dropped every
/// handle, or dropped the stream. `last_sq` is per connection: a reconnect
/// replays snapshots with fresh sequence stamps that must not be reported
/// as a regression.
async fn pump(
    config: &PerpsWsBuilder,
    stream: &mut PerpsWs,
    channels: &mut Vec<Channel>,
    commands: &mut mpsc::Receiver<Command>,
    events: &mpsc::Sender<Result<Event, PerpsWsError>>,
    delivered: &mut bool,
) -> Result<(), PerpsWsError> {
    let tick = config.ping_interval.min(config.stale_after);
    let mut last_frame = Instant::now();
    let mut last_ping = Instant::now();
    let mut last_sq: HashMap<Channel, u64> = HashMap::new();

    loop {
        tokio::select! {
            biased;
            command = commands.recv() => match command {
                None | Some(Command::Close) => return Ok(()),
                Some(Command::Subscribe(add, reply)) => {
                    let result = subscribe_with_retry(config, stream, &add).await;
                    if result.is_ok() {
                        for c in add {
                            if !channels.contains(&c) {
                                channels.push(c);
                            }
                        }
                    }
                    let _ = reply.send(result);
                }
                Some(Command::Unsubscribe(remove, reply)) => {
                    let result = stream.unsubscribe(remove.iter().copied()).await;
                    if result.is_ok() {
                        channels.retain(|c| !remove.contains(c));
                    }
                    let _ = reply.send(result);
                }
            },
            next = timeout(tick, stream.next()) => match next {
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
                    if last_ping.elapsed() >= config.ping_interval {
                        // `ping` reads until the pong. On a socket that is
                        // open but dead that is forever, so it gets the
                        // rest of the staleness window and no more. A pong
                        // that arrives late is dropped by the stream as an
                        // uncorrelated response.
                        let budget = config.stale_after.saturating_sub(elapsed);
                        match timeout(budget, stream.ping()).await {
                            Ok(pong) => {
                                pong?;
                                last_ping = Instant::now();
                            }
                            Err(_) => {
                                return Err(PerpsWsError::Stalled {
                                    elapsed: last_frame.elapsed(),
                                })
                            }
                        }
                    }
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
        let server = ScriptedServer::start(vec![
            Script::default(),
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
    async fn a_socket_that_never_answers_pings_is_a_stall_too() {
        // The socket stays open and silent. A ping whose pong never comes
        // must not park the task past the staleness window.
        let server = ScriptedServer::start(vec![
            Script {
                answer_pings: false,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(7)],
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
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 7));
    }

    #[tokio::test]
    async fn membership_changes_reach_the_server_and_survive_a_reconnect() {
        let server = ScriptedServer::start(vec![
            Script {
                pushes: vec![bbo(1)],
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
}
