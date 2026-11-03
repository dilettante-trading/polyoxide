//! Binance's socket protocol for one routed path, as the kit's `Supervisor`
//! drives it. The supervised tier runs one supervisor per path.

use std::{fmt, future::Future, sync::Arc, time::Duration};

use polyoxide_rt::Runtime;
use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    BoxSocket, Connection, Decoded, Kind, Offline, Outage, Protocol, Queue, Recovery,
    SocketTransport, WsError,
};

use super::{
    request::{request, request_frame, Answer},
    Shape, SocketError, StreamSet, MAX_STREAMS_PER_CONNECTION,
};

/// One change to a path's streams.
#[derive(Debug, Clone)]
pub(crate) enum Change<N> {
    /// Subscribe to these streams; one already subscribed is skipped.
    Subscribe(Vec<N>),
    /// Unsubscribe from these streams; one not subscribed is skipped.
    Unsubscribe(Vec<N>),
}

/// Binance's protocol on one routed path of a product's streams.
///
/// Its membership is the path's streams, replayed on every fresh connection at
/// most `S::MAX_NAMES_PER_REQUEST` names a request, one request per
/// `S::MIN_REQUEST_INTERVAL`, with the connection's cap checked first (D9).
/// Its divergence rows:
///
/// - **D4, the ping:** an empty protocol `Ping` on the wall clock; on a
///   transport that cannot send one (`Capabilities::client_pings` false, a
///   browser's) a `LIST_SUBSCRIPTIONS` request under an id of its own, paced
///   as every request is (a tick inside `S::MIN_REQUEST_INTERVAL` of the last
///   request sends nothing, and the next tick sends), whose answer is a frame
///   like any other and so is liveness (D9).
/// - **D5, liveness:** every frame, the server's pings and the pongs to the
///   client's own included.
/// - **D6, a delivered connection:** one that decoded an update.
/// - **D8:** `Disconnected { path, reason }`, then `Reconnected { path }`.
/// - **D9:** rotation at 23 h 50 min (the builder's `max_connection_age`)
///   and the paced replay.
/// - **D19:** a connection used after it closed reconnects.
/// - **Outages:** a change made during one is recorded and answered at once,
///   and the replay applies it.
/// - **Not wanted:** a path with no streams; unsubscribing its last sends
///   nothing, and the supervisor closes the connection politely.
///
/// A change is recorded before it is sent, as the old loop recorded it, so a
/// change whose connection dies under it is answered `Ok` and replayed. Its
/// queue holds one event: the supervised tier forwards each path's events
/// into the one queue of 1024 the consumer reads.
pub(crate) struct BinanceProtocol<S: StreamSet> {
    base_url: String,
    path: S::Path,
    /// Whether the transport of the connection last dialled can send protocol
    /// pings; read from its capabilities in `connect`.
    client_pings: bool,
}

// By hand: a derive would require `S: Debug` of the marker type.
impl<S: StreamSet> fmt::Debug for BinanceProtocol<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BinanceProtocol")
            .field("base_url", &self.base_url)
            .field("path", &self.path)
            .field("client_pings", &self.client_pings)
            .finish()
    }
}

impl<S: StreamSet> BinanceProtocol<S> {
    pub(crate) fn new(base_url: String, path: S::Path) -> Self {
        Self {
            base_url,
            path,
            client_pings: true,
        }
    }

    fn too_many(&self) -> S::Error {
        <S::Error as SocketError>::too_many_streams(self.path, MAX_STREAMS_PER_CONNECTION)
    }

    /// The streams of `add` not already in `streams`, once each, refused
    /// before anything is recorded or sent if they would pass the cap.
    fn new_streams(
        &self,
        streams: &[S::Name],
        add: Vec<S::Name>,
    ) -> Result<Vec<S::Name>, S::Error> {
        let mut new: Vec<S::Name> = Vec::new();
        for stream in add {
            if !streams.contains(&stream) && !new.contains(&stream) {
                new.push(stream);
            }
        }
        // The server answers the 1025th stream with an error and then closes
        // the connection, losing all 1024.
        if streams.len() + new.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(self.too_many());
        }
        Ok(new)
    }
}

/// The streams of `remove` that `streams` carries, once each.
fn gone_streams<N: Clone + Eq>(streams: &[N], remove: Vec<N>) -> Vec<N> {
    let mut gone: Vec<N> = Vec::new();
    for stream in remove {
        if streams.contains(&stream) && !gone.contains(&stream) {
            gone.push(stream);
        }
    }
    gone
}

impl<S: StreamSet> Protocol for BinanceProtocol<S> {
    type Event = S::Event;
    type Error = S::Error;
    type Membership = Vec<S::Name>;
    type Change = Change<S::Name>;
    /// The path's streams after the change.
    type Ack = Vec<S::Name>;

    fn handshake(&mut self) -> Result<Handshake, S::Error> {
        Ok(S::url(&self.base_url, self.path)
            .into_client_request()
            .map_err(WsError::from)?)
    }

    /// Dials the handshake through the transport, as the default does, and
    /// reads from the transport whether it can send a protocol ping (D9),
    /// which `ping` acts on for this connection.
    fn connect(
        &mut self,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
        timeout: Duration,
    ) -> impl Future<Output = Result<BoxSocket, S::Error>> + Send {
        self.client_pings = transport.capabilities().client_pings;
        let request = self.handshake();
        async move {
            let socket = transport.connect(request?, timeout, runtime).await?;
            Ok::<_, S::Error>(socket)
        }
    }

    /// Requests no faster than one per `S::MIN_REQUEST_INTERVAL`. No answer
    /// timeout: the supervisor bounds each replay and change by `stale_after`
    /// (L2-D1).
    fn configure(&self, connection: Connection) -> Connection {
        connection.min_request_interval(S::MIN_REQUEST_INTERVAL)
    }

    /// Every stream, `S::MAX_NAMES_PER_REQUEST` names a request; the cap is
    /// checked first. A stream refused on a fresh connection stops the path,
    /// as retrying replays it.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        streams: &Vec<S::Name>,
    ) -> Result<(), S::Error> {
        if streams.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(self.too_many());
        }
        for batch in streams.chunks(S::MAX_NAMES_PER_REQUEST) {
            request::<S::Name, S::Error>(connection, "SUBSCRIBE", Some(batch)).await?;
        }
        Ok(())
    }

    /// A subscribe or an unsubscribe on the live connection, recorded before
    /// it is sent. A refused batch is undone, with those after it, which were
    /// never sent; the batches answered before it stay. A request the
    /// connection dies under leaves the change recorded and answers it `Ok`:
    /// the next read ends the connection, and the replay applies the change.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        streams: &mut Vec<S::Name>,
        change: Change<S::Name>,
    ) -> Result<Vec<S::Name>, S::Error> {
        let before = streams.clone();
        let (subscribing, names) = match change {
            Change::Subscribe(add) => {
                let new = self.new_streams(streams, add)?;
                streams.extend(new.iter().cloned());
                (true, new)
            }
            Change::Unsubscribe(remove) => {
                let gone = gone_streams(streams, remove);
                streams.retain(|s| !gone.contains(s));
                if streams.is_empty() {
                    // The path's last stream left: nothing is sent, and the
                    // connection is closed as no longer wanted.
                    return Ok(Vec::new());
                }
                (false, gone)
            }
        };
        let method = if subscribing {
            "SUBSCRIBE"
        } else {
            "UNSUBSCRIBE"
        };
        for (n, batch) in names.chunks(S::MAX_NAMES_PER_REQUEST).enumerate() {
            match request::<S::Name, S::Error>(connection, method, Some(batch)).await {
                Ok(_) => {}
                Err(err) if matches!(err.shape(), Shape::Refused) => {
                    let answered = &names[..n * S::MAX_NAMES_PER_REQUEST];
                    *streams = if subscribing {
                        before.into_iter().chain(answered.iter().cloned()).collect()
                    } else {
                        before
                            .into_iter()
                            .filter(|s| !answered.contains(s))
                            .collect()
                    };
                    return Err(err);
                }
                Err(err) => {
                    tracing::debug!(%err, path = %self.path, "the connection failed under a change; it is recorded for the replay");
                    break;
                }
            }
        }
        Ok(streams.clone())
    }

    /// A change during an outage is recorded and answered at once; the
    /// replay applies it.
    fn record(
        &mut self,
        streams: &mut Vec<S::Name>,
        change: Change<S::Name>,
    ) -> Offline<Change<S::Name>, Vec<S::Name>, S::Error> {
        match change {
            Change::Subscribe(add) => match self.new_streams(streams, add) {
                Ok(new) => streams.extend(new),
                Err(err) => return Offline::Recorded(Err(err)),
            },
            Change::Unsubscribe(remove) => {
                let gone = gone_streams(streams, remove);
                streams.retain(|s| !gone.contains(s));
            }
        }
        Offline::Recorded(Ok(streams.clone()))
    }

    fn decode(&mut self, frame: &Message) -> Decoded<S::Event, S::Error> {
        // A Binary frame is not part of the protocol (D12), and the kit
        // answers the server's Ping and its Close.
        let Message::Text(text) = frame else {
            return Decoded::Nothing;
        };
        match S::decode(text) {
            Ok(update) => Decoded::Event(S::update(update)),
            // An answer nobody awaits, such as one to a request that
            // outlived the staleness window.
            Err(_) if Answer::parse(text).is_some() => Decoded::Nothing,
            Err(err) => Decoded::Error(err),
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<S::Event, S::Error>) -> bool {
        true
    }

    /// `decode` yields an event only for an update (the outage markers come
    /// from `disconnected` and `reconnected`), so any event is one.
    fn delivered(&self, _frame: &Message, decoded: &Decoded<S::Event, S::Error>) -> bool {
        matches!(decoded, Decoded::Event(_))
    }

    /// An empty protocol `Ping`; on a transport that cannot send one,
    /// `LIST_SUBSCRIPTIONS` under an id taken from the connection, so a late
    /// answer is never taken for the answer to a later request. The id is the
    /// connection's paced one (`take_paced_id`): a tick that comes within the
    /// connection's minimum request interval of the last request, the
    /// replay's included, sends nothing, and the next tick sends, as every
    /// request waits that interval (D9). The answer decodes to nothing and
    /// counts as liveness, as every frame does.
    fn ping(&mut self, connection: &mut Connection) -> Option<Message> {
        if self.client_pings {
            return Some(Message::Ping(Vec::new().into()));
        }
        let id = connection.take_paced_id()?;
        Some(request_frame::<S::Name>("LIST_SUBSCRIPTIONS", None, id))
    }

    fn wanted(streams: &Vec<S::Name>) -> bool {
        !streams.is_empty()
    }

    fn is_retriable(err: &S::Error) -> bool {
        err.is_retriable()
    }

    /// Binance's own recovery, not the class's: a server close of any code,
    /// a lost or timed out connection, an unanswered replay and a malformed
    /// answer reconnect; a frame that does not decode is skipped; a refusal
    /// and the client's own refusals stop.
    fn recover(&self, err: &S::Error) -> Recovery {
        match err.shape() {
            Shape::Connect(err) => match err.kind() {
                // D19: a connection used after it closed reconnects Binance;
                // so does a frame or header past tungstenite's limits.
                Kind::AlreadyClosed | Kind::Capacity => Recovery::Reconnect,
                // A handshake refused with 408, 425, 429 or 5xx is the host
                // being unwell or throttling, by venue's status rule; any
                // other status, a bad URL or TLS name, or an attack detection
                // is our request. Everything else is retried.
                _ if retrying_can_fix(err, |status| {
                    class_for_handshake_status(status).is_retriable()
                }) =>
                {
                    Recovery::Reconnect
                }
                _ => Recovery::Stop,
            },
            Shape::ConnectTimeout | Shape::Closed | Shape::NoAnswer | Shape::Response => {
                Recovery::Reconnect
            }
            Shape::Frame => Recovery::Skip,
            Shape::Refused | Shape::TooManyStreams | Shape::WrongPath | Shape::Stopped => {
                Recovery::Stop
            }
        }
    }

    /// A refused handshake's `Retry-After`, as Binance's class reads it,
    /// which the reconnect waits out up to the backoff's ceiling (DRIFT R11).
    fn retry_after(&self, err: &S::Error) -> Option<Duration> {
        err.retry_after()
    }

    fn disconnected(&mut self, outage: Outage<S::Error>) -> Option<S::Event> {
        Some(S::disconnected(self.path, outage))
    }

    fn reconnected(&mut self) -> S::Event {
        S::reconnected(self.path)
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(1)
    }

    /// A replay or a change the server left unanswered for the staleness
    /// window. The supervised tier answers a change so left `Ok`, as it is
    /// recorded and the replay applies it.
    fn stalled(silent: Duration) -> S::Error {
        <S::Error as SocketError>::no_answer(0, silent)
    }

    /// The kit's task stopped; the router adds the cause it saw.
    fn stopped() -> S::Error {
        <S::Error as SocketError>::stopped(None)
    }
}
