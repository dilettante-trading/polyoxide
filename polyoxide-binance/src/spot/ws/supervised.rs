//! The supervised tier: the Binance protocol on the kit's `Supervisor`, with
//! keep-alive on the wall clock, staleness detection, reconnect with a paced
//! replay of the streams, rotation before Binance's 24-hour cutoff and outage
//! markers. The protocol, the router and the forwarding are `shared::ws`'s.

use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use polyoxide_ws::Timing;

use crate::{
    shared::ws::{
        supervised::{connect as connect_running, Handle, Running, Settings},
        DEFAULT_MAX_CONNECTION_AGE,
    },
    spot::ws::{
        error::SpotWsError, event::Update, protocol::SpotStreams, stream::StreamName, StreamPath,
        SPOT_WS_BASE,
    },
};

/// What the supervised stream yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// One frame of one stream, boxed, as the other variants are small.
    Update(Box<Update>),
    /// The connection was lost. Every update is stale until the matching
    /// [`Event::Reconnected`], which always follows while the client runs.
    Disconnected {
        /// The path that went down: spot has one, [`StreamPath::Stream`].
        path: StreamPath,
        /// Why.
        reason: DisconnectReason,
    },
    /// The path is current again: a fresh connection has replayed its
    /// streams, or its last stream left during the outage, so nothing on it is
    /// stale. Sent once per outage, however many attempts it took. Drop book
    /// state and resync.
    Reconnected {
        /// The path that came back.
        path: StreamPath,
    },
}

/// Why the connection went down.
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
    Error(SpotWsError),
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
/// The connection runs on its own task, on the socket kit's `Supervisor`, and
/// logs under the `polyoxide_ws` target.
///
/// While the consumer leaves 1024 events unread, the connection stops reading
/// but keeps pinging, and that wait is not silence: a slow consumer is never
/// reported [`Stale`](DisconnectReason::Stale), and no event is dropped. The
/// server's own pings go unanswered meanwhile. Binance closes a connection
/// that has not answered them for a minute; that close is reconnected like any
/// other, and the events already queued are kept.
#[derive(Debug, Clone)]
pub struct SpotWsBuilder {
    base_url: String,
    timing: Timing,
    streams: Vec<StreamName>,
}

impl Default for SpotWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotWsBuilder {
    /// Defaults: the production host, a ping every 20 s (the server's own
    /// cadence), stale after 30 s of silence, reconnect backoff from 500 ms
    /// doubling to 60 s, a 10 s connect timeout, rotation after 23 h 50 min,
    /// and no streams.
    pub fn new() -> Self {
        let mut timing = Timing::default();
        timing.max_connection_age = Some(DEFAULT_MAX_CONNECTION_AGE);
        Self {
            base_url: SPOT_WS_BASE.to_owned(),
            timing,
            streams: Vec::new(),
        }
    }

    /// Connect to another host, such as a local test server. The client
    /// appends `/stream`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    polyoxide_ws::timing_setters! {
        /// How often the connection sends a protocol ping, whatever its
        /// traffic, and also while the consumer leaves 1024 events unread.
        /// None goes out while a membership change is in flight, which
        /// `stale_after` bounds. Binance closes a connection that receives
        /// more than 5 messages a second, requests, pings and pongs counted.
        ping_interval;
        /// Silence after which the connection is presumed dead and replaced.
        /// Pongs and the server's pings count, so a quiet connection that
        /// answers stays up, and time the consumer leaves 1024 events unread
        /// does not count.
        ///
        /// Also bounds each replay and each membership change as a whole, its
        /// 500 ms pacing included: one the server leaves unanswered that long
        /// replaces the connection. A replay is one request, whatever the
        /// number of streams, since a request carries up to 1024 names.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max`, with no cap
        /// on attempts, and back to `initial` after a connection that
        /// delivered an update. Both are raised to at least 1 ms, and an
        /// `initial` above `max` is lowered to `max`. A reconnect refused with
        /// a `Retry-After` waits at least that long, up to `max` (DRIFT R11).
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

    /// Open the connection, subscribe, and start supervising.
    ///
    /// Fails if the first connect fails, or if the streams pass 1024: a setup
    /// that is wrong is reported, not retried. With no streams it opens
    /// nothing until the first [`MembershipHandle::subscribe`].
    pub async fn connect(self) -> Result<SupervisedSpotWs, SpotWsError> {
        let running = connect_running::<SpotStreams>(Settings {
            base_url: self.base_url,
            timing: self.timing,
            streams: self.streams,
            runtime: None,
            socket_transport: None,
        })
        .await?;
        Ok(SupervisedSpotWs { running })
    }
}

/// Changes the streams of a running [`SupervisedSpotWs`].
///
/// The connection is opened for the first stream and closed when the last
/// leaves. The membership is a set: subscribing a stream twice holds it once,
/// and counting references is the caller's job.
///
/// A change is recorded before it is sent. During an outage it is answered
/// `Ok` at once, and the replay applies it; so is a change whose connection
/// fails under it, or one the server leaves unanswered for `stale_after`,
/// which replaces the connection. A change that arrives while a connect or a
/// replay is in flight waits for that attempt. Changes are sent no faster than
/// one per 500 ms.
///
/// A call fails when the server refuses it ([`SpotWsError::Refused`]), when it
/// would pass 1024 streams ([`SpotWsError::TooManyStreams`], nothing sent),
/// when the first connect of a connection it opens fails in a way retrying
/// cannot fix ([`SpotWsError::Connect`]), or when the client has stopped
/// ([`SpotWsError::Stopped`]). Its `cause` is the error the client stopped on
/// once the router has that stop; a call in flight when it stops can get
/// `Stopped { cause: None }`. A request carries up to 1024 names, so a
/// subscription is never split.
///
/// A call waits for the connection's task, which takes no change while the
/// consumer leaves 1024 events unread. Make membership changes from a task
/// other than the one draining the stream, or keep draining while they run.
#[derive(Debug, Clone)]
pub struct MembershipHandle {
    handle: Handle<SpotStreams>,
}

impl MembershipHandle {
    /// Subscribe to more streams.
    pub async fn subscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), SpotWsError> {
        self.handle.subscribe(streams.into_iter().collect()).await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub async fn unsubscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), SpotWsError> {
        self.handle.unsubscribe(streams.into_iter().collect()).await
    }
}

/// A supervised connection: a `Stream` of [`Event`]s that survives drops,
/// stalls and Binance's 24-hour cutoff.
///
/// Every [`Event::Disconnected`] is followed by an [`Event::Reconnected`] for
/// the same path while the client runs. A connection that closes because its
/// last stream left, while it is up, yields neither.
///
/// A fatal error, such as the server refusing a replay after a reconnect, is
/// yielded as `Err` and then the stream ends. An `Err(SpotWsError::Frame { .. })`,
/// a frame that did not decode, does not end it.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::spot::{types::Symbol, ws::{Event, SpotWsBuilder, SpotWsError, StreamName}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut feed = SpotWsBuilder::new()
///     .streams([StreamName::MiniTicker(btc.clone()), StreamName::BookTicker(btc)])
///     .connect()
///     .await?;
/// let membership = feed.membership();
/// while let Some(event) = feed.next().await {
///     let event = match event {
///         Ok(event) => event,
///         // A frame that did not decode is skipped; anything else ends the stream.
///         Err(SpotWsError::Frame { .. }) => continue,
///         Err(err) => return Err(err.into()),
///     };
///     match event {
///         Event::Update(update) => println!("{}", serde_json::to_string(&update)?),
///         Event::Disconnected { path, reason } => eprintln!("{path} down: {reason}"),
///         Event::Reconnected { path } => eprintln!("{path} back"),
///         _ => {}
///     }
/// }
/// # membership.subscribe([StreamName::AllMiniTickers]).await?;
/// # Ok(())
/// # }
/// ```
pub struct SupervisedSpotWs {
    running: Running<SpotStreams>,
}

impl fmt::Debug for SupervisedSpotWs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupervisedSpotWs")
            .field("running", &self.running.is_running())
            .finish_non_exhaustive()
    }
}

impl SupervisedSpotWs {
    /// A handle for changing streams while the stream runs.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            handle: self.running.membership(),
        }
    }

    /// Close the connection and stop. Every [`MembershipHandle`] answers
    /// [`SpotWsError::Stopped`] afterwards.
    ///
    /// Fails with [`SpotWsError::Stopped`] if the client had stopped on an
    /// error, which the stream yielded: its `cause` is that error, as
    /// [`Classify`](polyoxide_venue::Classify) read it, and so is the cause
    /// every later membership call carries. That holds once the router has the
    /// stop: a `close()` made right after the stream yielded the stopping
    /// error can return `Ok` or `Stopped { cause: None }`. Read the stream to
    /// its end first to be sure of the cause.
    pub async fn close(self) -> Result<(), SpotWsError> {
        self.running.close().await
    }
}

impl Stream for SupervisedSpotWs {
    type Item = Result<Event, SpotWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.running.poll_event(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_defaults_match_the_measured_cadences() {
        // Spot's server pings every 20 s (OBSERVED.md, `### Pings`); the rest
        // are usdm's.
        let b = SpotWsBuilder::new();
        assert_eq!(b.base_url, "wss://stream.binance.com:9443");
        assert_eq!(b.timing.ping_interval, Some(Duration::from_secs(20)));
        assert_eq!(b.timing.stale_after, Duration::from_secs(30));
        assert_eq!(b.timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(b.timing.max_backoff, Duration::from_secs(60));
        assert_eq!(b.timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(
            b.timing.max_connection_age,
            Some(Duration::from_secs(23 * 3600 + 50 * 60))
        );
        assert!(b.timing.max_connection_age < Some(Duration::from_secs(24 * 3600)));
        assert!(b.streams.is_empty());
    }

    #[test]
    fn the_setters_replace_their_field_and_default_is_new() {
        let b = SpotWsBuilder::default()
            .base_url("ws://127.0.0.1:9")
            .streams([StreamName::AllMiniTickers]);
        assert_eq!(b.base_url, "ws://127.0.0.1:9");
        assert_eq!(b.streams, [StreamName::AllMiniTickers]);
        assert_eq!(
            SpotWsBuilder::default().timing.stale_after,
            SpotWsBuilder::new().timing.stale_after
        );
    }

    #[test]
    fn a_disconnect_reason_reads_as_the_texts_usdm_gives() {
        assert_eq!(
            DisconnectReason::Closed {
                code: Some(1008),
                reason: "Too many requests".to_owned()
            }
            .to_string(),
            "closed by the server (1008 Too many requests)"
        );
        assert_eq!(
            DisconnectReason::Closed {
                code: None,
                reason: String::new()
            }
            .to_string(),
            "closed by the server"
        );
        let refused = SpotWsError::Refused {
            code: 2,
            msg: "Invalid request".to_owned(),
        };
        let text = refused.to_string();
        assert_eq!(DisconnectReason::Error(refused).to_string(), text);
        assert_eq!(
            DisconnectReason::Stale.to_string(),
            "no frame within the staleness window"
        );
        assert_eq!(
            DisconnectReason::Rotation.to_string(),
            "rotated before the 24-hour cutoff"
        );
    }
}
