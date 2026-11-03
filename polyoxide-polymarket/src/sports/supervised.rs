//! The supervised tier: a feed that reconnects for as long as it is held,
//! unless a reconnect is refused in a way retrying cannot fix, and says when
//! its scores may be stale.

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_rt::{default_runtime, BoxFuture, Runtime};
use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{default_socket_transport, Backoff, Connection, Kind, SocketTransport};

use crate::sports::{
    client::{classify, closed, open_with, Inbound, DEFAULT_CONNECT_TIMEOUT, SPORTS_WS_URL},
    error::SportsError,
    update::MatchUpdate,
};

/// Three missed server pings; the server sends one every 15 seconds.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(45);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);

/// What a supervised feed yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// A match update. Boxed because it is several times larger than the
    /// other variants; it dereferences to [`MatchUpdate`].
    Update(Box<MatchUpdate>),
    /// The connection was lost, and scores are stale from here. After
    /// [`Event::Reconnected`], each game is current again once its next
    /// frame arrives. When last measured the server re-sent esports games
    /// every 20 s and tennis irregularly, 22 to 153 s apart; other sports are
    /// unmeasured.
    Disconnected {
        /// Why the connection was given up.
        reason: SportsError,
    },
    /// A new connection is up.
    ///
    /// The frame saying a match ended is usually sent once, so games that ended
    /// during the gap were not re-sent. Reconcile them through gamma by
    /// [`GameKey::Game`](crate::sports::GameKey::Game). Cricket games cannot be
    /// reconciled.
    Reconnected,
}

/// Builder for a [`SupervisedSportsWs`].
///
/// ```no_run
/// use std::time::Duration;
/// use polyoxide_polymarket::sports::SportsWsBuilder;
///
/// # async fn run() -> Result<(), polyoxide_polymarket::sports::SportsError> {
/// let feed = SportsWsBuilder::new()
///     .stale_after(Duration::from_secs(60))
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SportsWsBuilder {
    url: String,
    stale_after: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
    connect_timeout: Duration,
    runtime: Option<Arc<dyn Runtime>>,
    socket_transport: Option<Arc<dyn SocketTransport>>,
}

impl Default for SportsWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SportsWsBuilder {
    /// The production URL, a 45 s staleness limit, backoff from 500 ms to
    /// 60 s, and a 10 s connect timeout.
    pub fn new() -> Self {
        Self {
            url: SPORTS_WS_URL.to_owned(),
            stale_after: DEFAULT_STALE_AFTER,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            runtime: None,
            socket_transport: None,
        }
    }

    /// Connect somewhere other than production, such as a local test server.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// How long a connection may go without receiving anything, protocol
    /// pings included, before it is treated as dead.
    ///
    /// Keep this above the server's 15-second ping interval. Data cannot set
    /// it: when nothing is live anywhere, no data arrives at all.
    ///
    /// On a transport that hides the server's pings, a browser's, there is
    /// nothing to count in a quiet hour, so the limit is not enforced there
    /// and this value is ignored.
    ///
    /// `Duration::MAX` turns the check off on any transport: no timer is
    /// armed.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect delay bounds. The delay doubles from `initial` up to `max`,
    /// and returns to `initial` after a connection that received anything.
    /// Both are raised to at least 1 ms, and an `initial` above `max` is
    /// lowered to `max`. A reconnect refused with a `Retry-After` waits at
    /// least that long, up to `max` (DRIFT R11).
    ///
    /// A protocol ping counts as receiving something. So a server that
    /// accepts, sends anything at all, and drops the connection resets the
    /// schedule each time, and reconnects repeat at `initial` rather than
    /// backing off.
    pub fn backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max;
        self
    }

    /// How long one connection attempt may take.
    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    polyoxide_ws::transport_setters! {
        /// The runtime the feed's staleness timer and reconnect waits run on.
        /// By default the target's.
        runtime;
        /// How each connect dials. By default the target's.
        socket_transport;
    }

    /// Make the first connection and start supervising.
    ///
    /// Fails if the first connection fails, so a wrong URL or a network with
    /// no route surfaces at once. Later failures are retried, except a
    /// refusal retrying cannot fix: an HTTP status other than 408, 425, 429
    /// or 5xx, or a malformed URL. That one is yielded as
    /// [`SportsError::Connect`] and ends the stream.
    pub async fn connect(mut self) -> Result<SupervisedSportsWs, SportsError> {
        let runtime = self.runtime.clone().unwrap_or_else(default_runtime);
        let transport = self
            .socket_transport
            .clone()
            .unwrap_or_else(default_socket_transport);
        // D9: a transport that hides the server's pings (`Capabilities::
        // sees_server_pings`; a browser's does) leaves a quiet feed nothing to
        // count, since no data arrives when nothing is live, and no text ping
        // is known to be answered (`docs/specs/sports/OBSERVED.md`). The limit
        // is off there, as `Duration::MAX` means never stale and arms no
        // timer, and a lost connection ends through its close event. The
        // transport read is the one dialled.
        if !transport.capabilities().sees_server_pings {
            self.stale_after = Duration::MAX;
        }
        let connection = open_with(
            self.url.clone(),
            self.connect_timeout,
            Arc::clone(&transport),
            Arc::clone(&runtime),
        )
        .await?;
        Ok(SupervisedSportsWs {
            backoff: Backoff::new(self.initial_backoff, self.max_backoff),
            state: State::reading(Box::new(connection), self.stale_after),
            config: self,
            runtime,
            transport,
        })
    }
}

type Attempt = Pin<Box<dyn Future<Output = Result<Connection, SportsError>> + Send>>;

enum State {
    /// Reading an open connection.
    Reading {
        connection: Box<Connection>,
        stale: BoxFuture<()>,
        received: bool,
    },
    /// Waiting out a backoff delay.
    Waiting(BoxFuture<()>),
    /// A connection attempt in flight.
    Connecting(Attempt),
    /// A reconnect was refused for good. The stream yields nothing more.
    Ended,
}

impl State {
    fn reading(connection: Box<Connection>, stale_after: Duration) -> Self {
        let stale = stale_timer(&connection, stale_after);
        State::Reading {
            connection,
            stale,
            received: false,
        }
    }
}

/// The timer that ends a silent connection: the runtime's sleep for
/// `stale_after`, or, for `Duration::MAX` (never stale), a future that never
/// resolves, so a connection with the check off arms no timer at all.
fn stale_timer(connection: &Connection, stale_after: Duration) -> BoxFuture<()> {
    if stale_after == Duration::MAX {
        Box::pin(std::future::pending())
    } else {
        connection.runtime().sleep(stale_after)
    }
}

/// One iteration's outcome, decided while the state is borrowed and acted on
/// after the borrow ends.
enum Step {
    Pending,
    Yield(Result<Event, SportsError>),
    Lost(SportsError),
    Attempt,
    Connected(Box<Connection>),
    AttemptFailed(SportsError),
}

/// A sports feed that reconnects for as long as it is held.
///
/// Yields [`Event`]s. Every outage yields one [`Event::Disconnected`] when
/// the connection is lost and one [`Event::Reconnected`] when a new one is
/// up, however many attempts that takes. Two things arrive as `Err`:
///
/// - [`SportsError::Decode`], for an undecodable frame. The stream carries on.
/// - [`SportsError::Connect`], for a reconnect refused in a way retrying
///   cannot fix, such as a `404` or `403` on the upgrade. It follows the
///   outage's `Disconnected`, and the stream ends after it.
///
/// Otherwise the stream never ends while held. Dropping it closes the
/// connection.
///
/// A connection that receives nothing, protocol pings included, for the
/// staleness limit is treated as dead.
///
/// No background task runs: all the work happens inside `poll_next`. So the
/// server's pings are answered only while the stream is being polled. A
/// caller that stops polling for long enough will be dropped by the server,
/// and will see a disconnect and a reconnect when it resumes.
///
/// On a transport that hides the server's pings, a browser's, nothing is
/// counted in a quiet hour, so the staleness limit is not enforced: a lost
/// connection ends through its close event or a failed read (D9).
pub struct SupervisedSportsWs {
    config: SportsWsBuilder,
    backoff: Backoff,
    state: State,
    /// The runtime its backoff waits go through.
    runtime: Arc<dyn Runtime>,
    /// How a reconnect dials.
    transport: Arc<dyn SocketTransport>,
}

impl SupervisedSportsWs {
    /// Give up the current connection and schedule the next attempt.
    fn lose(&mut self, reason: SportsError) -> Event {
        let received = matches!(self.state, State::Reading { received: true, .. });
        self.backoff.after_connection_ended(received);
        let delay = self.backoff.take();
        tracing::warn!(%reason, ?delay, "lost the sports feed; reconnecting");
        // Replacing the state drops the old connection.
        self.state = State::Waiting(self.runtime.sleep(delay));
        Event::Disconnected { reason }
    }
}

impl Stream for SupervisedSportsWs {
    type Item = Result<Event, SportsError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            let step = match &mut this.state {
                State::Reading {
                    connection,
                    stale,
                    received,
                } => poll_reading(connection, stale, received, this.config.stale_after, cx),
                State::Waiting(delay) => match delay.as_mut().poll(cx) {
                    Poll::Ready(()) => Step::Attempt,
                    Poll::Pending => Step::Pending,
                },
                State::Connecting(attempt) => match attempt.as_mut().poll(cx) {
                    Poll::Ready(Ok(connection)) => Step::Connected(Box::new(connection)),
                    Poll::Ready(Err(error)) => Step::AttemptFailed(error),
                    Poll::Pending => Step::Pending,
                },
                State::Ended => return Poll::Ready(None),
            };
            match step {
                Step::Pending => return Poll::Pending,
                Step::Yield(item) => return Poll::Ready(Some(item)),
                Step::Lost(reason) => return Poll::Ready(Some(Ok(this.lose(reason)))),
                Step::Attempt => {
                    this.state = State::Connecting(Box::pin(open_with(
                        this.config.url.clone(),
                        this.config.connect_timeout,
                        Arc::clone(&this.transport),
                        Arc::clone(&this.runtime),
                    )));
                }
                Step::Connected(connection) => {
                    tracing::info!("reconnected to the sports feed");
                    this.state = State::reading(connection, this.config.stale_after);
                    return Poll::Ready(Some(Ok(Event::Reconnected)));
                }
                Step::AttemptFailed(error) if !retrying_can_fix(&error) => {
                    tracing::warn!(%error, "sports feed refused the reconnect; giving up");
                    this.state = State::Ended;
                    return Poll::Ready(Some(Err(error)));
                }
                Step::AttemptFailed(error) => {
                    // A refusal's `Retry-After` only extends the wait, up to
                    // the ceiling (DRIFT R11).
                    let asked = error.retry_after().unwrap_or_default();
                    let delay = this.backoff.take_at_least(asked);
                    tracing::warn!(%error, ?delay, "sports feed reconnect failed; retrying");
                    this.state = State::Waiting(this.runtime.sleep(delay));
                }
            }
        }
    }
}

/// Whether a reconnect attempt that failed this way is worth repeating:
/// sports' recovery, at its call site, since D7 keeps sports off the kit's
/// `Supervisor`.
///
/// A failed connect follows the kit's `retrying_can_fix` with
/// polyoxide-venue's status rule (`class_for_handshake_status`, which
/// `Classify::is_retriable` reads too): an upgrade refused with 408, 425, 429
/// or 5xx is the host being unwell or throttling, any other status, a bad URL
/// or TLS name, a malformed handshake request or an attack detection is our
/// request, and everything else, a lost or timed out connection among them,
/// passes. Two rows are sports' own, ahead of it: a `Capacity` failure is
/// retried, which the kit gives up on, and so is a connection used after it
/// closed (D19, as Binance; perps and rtds stop).
fn retrying_can_fix(error: &SportsError) -> bool {
    let SportsError::Connect { source } = error else {
        return true;
    };
    match source.kind() {
        Kind::Capacity | Kind::AlreadyClosed => true,
        _ => polyoxide_ws::retrying_can_fix(source, |status| {
            class_for_handshake_status(status).is_retriable()
        }),
    }
}

/// Read one open connection until it yields something, goes quiet, or is lost.
fn poll_reading(
    connection: &mut Connection,
    stale: &mut BoxFuture<()>,
    received: &mut bool,
    stale_after: Duration,
    cx: &mut Context<'_>,
) -> Step {
    loop {
        let message = match connection.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(message))) => message,
            Poll::Ready(Some(Err(source))) => return Step::Lost(SportsError::transport(source)),
            // The stream ended without a close frame.
            Poll::Ready(None) => return Step::Lost(closed(None)),
            Poll::Pending => {
                return match stale.as_mut().poll(cx) {
                    Poll::Ready(()) => Step::Lost(SportsError::Stale { after: stale_after }),
                    Poll::Pending => Step::Pending,
                };
            }
        };
        let inbound = classify(message);
        // Anything but a close is proof of life, pings included: in a quiet
        // hour the server sends nothing else.
        if !matches!(inbound, Inbound::Closed(_)) {
            *received = true;
            // `Duration::MAX` means never stale, and its timer never fires, so
            // there is nothing to re-arm.
            if stale_after != Duration::MAX {
                *stale = stale_timer(connection, stale_after);
            }
        }
        match inbound {
            Inbound::Update(update) => return Step::Yield(Ok(Event::Update(update))),
            Inbound::Undecodable(error) => return Step::Yield(Err(error)),
            // The connection has queued the pong for a ping and flushed it.
            Inbound::Alive => continue,
            // The connection flushed the RFC 6455 close reply before it
            // yielded the close.
            Inbound::Closed(reason) => return Step::Lost(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_ws::{tungstenite, WsError};

    use super::*;

    const MS: Duration = Duration::from_millis(1);

    /// A connect that failed with this transport error.
    fn connect_error(err: tungstenite::Error) -> SportsError {
        SportsError::Connect {
            source: WsError::from(err),
        }
    }

    fn http_error(status: u16) -> SportsError {
        connect_error(tungstenite::Error::Http(
            tungstenite::http::Response::builder()
                .status(status)
                .body(None)
                .unwrap(),
        ))
    }

    #[test]
    fn a_refused_request_is_not_retried_but_an_unwell_host_is() {
        for status in [301, 401, 403, 404, 600] {
            assert!(!retrying_can_fix(&http_error(status)), "{status}");
        }
        for status in [408, 425, 429, 500, 502, 503] {
            assert!(retrying_can_fix(&http_error(status)), "{status}");
        }
    }

    #[test]
    fn a_transport_failure_or_timeout_is_retried() {
        let reset = connect_error(tungstenite::Error::Io(std::io::Error::from(
            std::io::ErrorKind::ConnectionReset,
        )));
        assert!(retrying_can_fix(&reset));
        let timeout = SportsError::ConnectTimeout {
            after: Duration::from_secs(10),
        };
        assert!(retrying_can_fix(&timeout));
    }

    #[test]
    fn a_capacity_failure_at_connect_is_retried() {
        // sports' own rule, kept at its call site: the kit's
        // `retrying_can_fix` gives up on a `Capacity` failure, and sports
        // retries it, as it always has.
        let capacity = connect_error(tungstenite::Error::Capacity(
            tungstenite::error::CapacityError::TooManyHeaders,
        ));
        assert!(retrying_can_fix(&capacity));
    }

    #[test]
    fn a_malformed_url_is_not_retried() {
        let url = connect_error(tungstenite::Error::Url(
            tungstenite::error::UrlError::NoHostName,
        ));
        assert!(!retrying_can_fix(&url));
    }

    #[tokio::test]
    async fn a_reconnect_waits_out_a_refusals_retry_after_drift_r11() {
        // A 10 ms step and a 400 ms wait: the attempt after the refusal
        // waits the 400 ms, where it retried inside the server's window.
        use polyoxide_ws::test_server::{Script, ScriptedServer};

        let server = ScriptedServer::start(vec![
            Script::close_at_once(),
            Script::refuse_upgrade(503).with_header("Retry-After", "0.4"),
            Script::hold(),
        ])
        .await;
        let mut feed = SportsWsBuilder::new()
            .url(server.url.clone())
            .backoff(MS * 10, Duration::from_secs(5))
            .connect()
            .await
            .unwrap();
        let mut events = Vec::new();
        for _ in 0..2 {
            let item = tokio::time::timeout(Duration::from_secs(3), feed.next())
                .await
                .expect("an event within the window")
                .expect("the feed is open");
            events.push(item.expect("not an error"));
        }
        assert!(
            matches!(
                events.as_slice(),
                [Event::Disconnected { .. }, Event::Reconnected]
            ),
            "{events:?}"
        );
        let accepted = server.accepted_at();
        let waited = accepted[2] - accepted[1];
        assert!(waited >= MS * 400, "{waited:?}");
    }

    #[test]
    fn a_connection_used_after_it_closed_is_retried_d19() {
        // D19: sports reconnects after `AlreadyClosed`, as Binance does;
        // perps and rtds stop.
        let used = connect_error(tungstenite::Error::AlreadyClosed);
        assert!(retrying_can_fix(&used));
    }

    #[test]
    fn backoff_doubles_up_to_its_ceiling() {
        let mut backoff = Backoff::new(MS * 100, MS * 1000);
        let delays: Vec<Duration> = (0..6).map(|_| backoff.take()).collect();
        assert_eq!(
            delays,
            [MS * 100, MS * 200, MS * 400, MS * 800, MS * 1000, MS * 1000]
        );
    }

    #[test]
    fn backoff_resets_only_after_a_connection_that_received_something() {
        let mut backoff = Backoff::new(MS * 100, MS * 1000);
        backoff.take();
        backoff.take();
        backoff.after_connection_ended(false);
        assert_eq!(
            backoff.take(),
            MS * 400,
            "a silent connection reset the schedule"
        );
        backoff.after_connection_ended(true);
        assert_eq!(backoff.take(), MS * 100);
    }

    #[test]
    fn an_initial_delay_above_the_ceiling_is_clamped() {
        let mut backoff = Backoff::new(MS * 5000, MS * 1000);
        assert_eq!(backoff.take(), MS * 1000);
    }

    #[test]
    fn a_zero_initial_delay_still_grows() {
        // Zero doubles to zero, which would retry a dead server on every
        // timer tick for ever.
        let mut backoff = Backoff::new(Duration::ZERO, MS * 1000);
        assert_eq!(backoff.take(), MS);
        assert_eq!(backoff.take(), MS * 2);
    }

    #[test]
    fn the_builder_defaults_are_the_documented_ones() {
        let builder = SportsWsBuilder::new();
        assert_eq!(builder.url, SPORTS_WS_URL);
        assert_eq!(builder.stale_after, Duration::from_secs(45));
        assert_eq!(builder.initial_backoff, Duration::from_millis(500));
        assert_eq!(builder.max_backoff, Duration::from_secs(60));
        assert_eq!(builder.connect_timeout, Duration::from_secs(10));
    }

    #[test]
    fn the_supervised_stream_can_move_between_tasks() {
        fn assert_send_unpin<T: Send + Unpin>() {}
        assert_send_unpin::<SupervisedSportsWs>();
    }

    /// A runtime on which an hour is over at once; every other wait is tokio's.
    #[derive(Debug, Default)]
    struct HourIsOver(polyoxide_rt::TokioRuntime);

    impl Runtime for HourIsOver {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.0.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            if duration == Duration::from_secs(3600) {
                Box::pin(std::future::ready(()))
            } else {
                self.0.sleep(duration)
            }
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.0.now()
        }
    }

    #[tokio::test]
    async fn the_staleness_window_is_the_injected_runtimes_timer() {
        use polyoxide_ws::test_server::{Script, ScriptedServer};

        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut feed = SportsWsBuilder::new()
            .url(server.url.clone())
            .stale_after(Duration::from_secs(3600))
            .runtime(Arc::new(HourIsOver::default()))
            .connect()
            .await
            .unwrap();
        let item = tokio::time::timeout(Duration::from_secs(3), feed.next())
            .await
            .expect("a quiet feed on a spent hour is stale at once")
            .expect("the feed is open");
        assert!(
            matches!(
                item,
                Ok(Event::Disconnected {
                    reason: SportsError::Stale { .. }
                })
            ),
            "{item:?}"
        );
    }

    /// A runtime that keeps every duration it is asked to sleep.
    #[derive(Debug, Default)]
    struct Sleeps {
        inner: polyoxide_rt::TokioRuntime,
        asked: std::sync::Mutex<Vec<Duration>>,
    }

    impl Runtime for Sleeps {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.inner.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            self.asked.lock().unwrap().push(duration);
            self.inner.sleep(duration)
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.inner.now()
        }
    }

    #[tokio::test]
    async fn with_staleness_off_no_timer_is_armed_for_any_frame() {
        use crate::sports::fixtures;
        use polyoxide_ws::test_server::{Script, ScriptedServer};

        let server = ScriptedServer::start(vec![Script::frames(&[
            fixtures::SOCCER,
            fixtures::ESPORTS,
            fixtures::TENNIS_EVENT_STATE,
        ])])
        .await;
        let runtime = Arc::new(Sleeps::default());
        let mut feed = SportsWsBuilder::new()
            .url(server.url.clone())
            .stale_after(Duration::MAX)
            .runtime(runtime.clone())
            .connect()
            .await
            .unwrap();
        for _ in 0..3 {
            let item = tokio::time::timeout(Duration::from_secs(3), feed.next())
                .await
                .expect("a frame within 3 s")
                .expect("the feed is open");
            assert!(matches!(item, Ok(Event::Update(_))), "{item:?}");
        }
        // The connect's own 10 s bound is the only timer asked for, and it
        // proves the injected runtime was consulted at all.
        let asked = runtime.asked.lock().unwrap().clone();
        assert!(
            asked.contains(&Duration::from_secs(10)),
            "the injected runtime was never asked for the connect bound: {asked:?}"
        );
        assert!(
            asked.iter().all(|duration| *duration < Duration::MAX),
            "armed a never-stale timer: {asked:?}"
        );
    }
}
