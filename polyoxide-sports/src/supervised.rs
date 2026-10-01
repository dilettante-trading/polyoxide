//! The supervised tier: a feed that reconnects for as long as it is held,
//! and says when its scores may be stale.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{SinkExt, Stream, StreamExt};
use tokio::time::{sleep, Instant, Sleep};

use crate::{
    client::{classify, closed, open, Inbound, Socket, DEFAULT_CONNECT_TIMEOUT, SPORTS_WS_URL},
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
    /// frame arrives, which the server sends every 20 to 90 seconds.
    Disconnected {
        /// Why the connection was given up.
        reason: SportsError,
    },
    /// A new connection is up.
    ///
    /// The frame saying a match ended is sent once, so games that ended
    /// during the gap were not re-sent. Reconcile them through gamma by
    /// [`GameKey::Game`](crate::GameKey::Game). Cricket games cannot be
    /// reconciled.
    Reconnected,
}

/// The reconnect delay schedule: doubling to a ceiling, and back to the start
/// after a connection that received anything.
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

    /// The delay to wait now. The one after it doubles.
    fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = self.next.saturating_mul(2).min(self.max);
        delay
    }

    /// A connection ended. One that received anything resets the schedule.
    /// One that received nothing, such as a server that accepts and closes at
    /// once, keeps it growing.
    fn after_connection_ended(&mut self, received: bool) {
        if received {
            self.next = self.initial;
        }
    }
}

/// Builder for a [`SupervisedSportsWs`].
///
/// ```no_run
/// use std::time::Duration;
/// use polyoxide_sports::SportsWsBuilder;
///
/// # async fn run() -> Result<(), polyoxide_sports::SportsError> {
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
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect delay bounds. The delay doubles from `initial` up to `max`,
    /// and returns to `initial` after a connection that received anything.
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

    /// Make the first connection and start supervising.
    ///
    /// Fails if the first connection fails, so a wrong URL or a network with
    /// no route surfaces at once. Every later failure is retried.
    pub async fn connect(self) -> Result<SupervisedSportsWs, SportsError> {
        let socket = open(self.url.clone(), self.connect_timeout).await?;
        Ok(SupervisedSportsWs {
            backoff: Backoff::new(self.initial_backoff, self.max_backoff),
            state: State::reading(Box::new(socket), self.stale_after),
            config: self,
        })
    }
}

type Attempt = Pin<Box<dyn Future<Output = Result<Socket, SportsError>> + Send>>;

enum State {
    /// Reading an open connection.
    Reading {
        socket: Box<Socket>,
        stale: Pin<Box<Sleep>>,
        received: bool,
    },
    /// Waiting out a backoff delay.
    Waiting(Pin<Box<Sleep>>),
    /// A connection attempt in flight.
    Connecting(Attempt),
}

impl State {
    fn reading(socket: Box<Socket>, stale_after: Duration) -> Self {
        State::Reading {
            socket,
            stale: Box::pin(sleep(stale_after)),
            received: false,
        }
    }
}

/// One iteration's outcome, decided while the state is borrowed and acted on
/// after the borrow ends.
enum Step {
    Pending,
    Yield(Result<Event, SportsError>),
    Lost(SportsError),
    Attempt,
    Connected(Box<Socket>),
    AttemptFailed(SportsError),
}

/// A sports feed that reconnects for as long as it is held.
///
/// Yields [`Event`]s. Only an undecodable frame arrives as `Err`, and the
/// stream carries on after it. Every outage yields one
/// [`Event::Disconnected`] when the connection is lost and one
/// [`Event::Reconnected`] when a new one is up, however many attempts that
/// takes. The stream never ends while held, and dropping it closes the
/// connection.
///
/// A connection that receives nothing, protocol pings included, for the
/// staleness limit is treated as dead.
///
/// No background task runs: all the work happens inside `poll_next`. So the
/// server's pings are answered only while the stream is being polled. A
/// caller that stops polling for long enough will be dropped by the server,
/// and will see a disconnect and a reconnect when it resumes.
pub struct SupervisedSportsWs {
    config: SportsWsBuilder,
    backoff: Backoff,
    state: State,
}

impl SupervisedSportsWs {
    /// Give up the current connection and schedule the next attempt.
    fn lose(&mut self, reason: SportsError) -> Event {
        let received = matches!(self.state, State::Reading { received: true, .. });
        self.backoff.after_connection_ended(received);
        let delay = self.backoff.take();
        tracing::warn!(%reason, ?delay, "lost the sports feed; reconnecting");
        // Replacing the state drops the old socket.
        self.state = State::Waiting(Box::pin(sleep(delay)));
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
                    socket,
                    stale,
                    received,
                } => poll_reading(socket, stale, received, this.config.stale_after, cx),
                State::Waiting(delay) => match delay.as_mut().poll(cx) {
                    Poll::Ready(()) => Step::Attempt,
                    Poll::Pending => Step::Pending,
                },
                State::Connecting(attempt) => match attempt.as_mut().poll(cx) {
                    Poll::Ready(Ok(socket)) => Step::Connected(Box::new(socket)),
                    Poll::Ready(Err(error)) => Step::AttemptFailed(error),
                    Poll::Pending => Step::Pending,
                },
            };
            match step {
                Step::Pending => return Poll::Pending,
                Step::Yield(item) => return Poll::Ready(Some(item)),
                Step::Lost(reason) => return Poll::Ready(Some(Ok(this.lose(reason)))),
                Step::Attempt => {
                    this.state = State::Connecting(Box::pin(open(
                        this.config.url.clone(),
                        this.config.connect_timeout,
                    )));
                }
                Step::Connected(socket) => {
                    tracing::info!("reconnected to the sports feed");
                    this.state = State::reading(socket, this.config.stale_after);
                    return Poll::Ready(Some(Ok(Event::Reconnected)));
                }
                Step::AttemptFailed(error) => {
                    let delay = this.backoff.take();
                    tracing::warn!(%error, ?delay, "sports feed reconnect failed; retrying");
                    this.state = State::Waiting(Box::pin(sleep(delay)));
                }
            }
        }
    }
}

/// Read one open connection until it yields something, goes quiet, or is lost.
fn poll_reading(
    socket: &mut Socket,
    stale: &mut Pin<Box<Sleep>>,
    received: &mut bool,
    stale_after: Duration,
    cx: &mut Context<'_>,
) -> Step {
    loop {
        let message = match socket.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(message))) => message,
            Poll::Ready(Some(Err(source))) => {
                return Step::Lost(SportsError::Transport {
                    source: Box::new(source),
                })
            }
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
            // A limit too large to add, such as `Duration::MAX`, means never
            // stale: the timer `sleep` armed lies at its far-future fallback.
            if let Some(deadline) = Instant::now().checked_add(stale_after) {
                stale.as_mut().reset(deadline);
            }
        }
        match inbound {
            Inbound::Update(update) => return Step::Yield(Ok(Event::Update(update))),
            Inbound::Undecodable(error) => return Step::Yield(Err(error)),
            // Reading again is what sends the pong for a ping just read.
            Inbound::Alive => continue,
            Inbound::Closed(reason) => {
                // Send the close reply tungstenite queued; nothing reads this
                // socket again.
                let _ = socket.poll_flush_unpin(cx);
                return Step::Lost(reason);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

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
}
