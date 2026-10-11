//! Binance's socket protocol for one routed path, as the kit's `Supervisor`
//! drives it. The supervised tier runs one supervisor per path.

use std::time::Duration;

use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    Connection, Decoded, Kind, Offline, Outage, Protocol, Queue, Recovery, WsError,
};

use crate::usdm::ws::{
    client::{request, Answer},
    error::UsdmWsError,
    event::Update,
    stream::StreamName,
    supervised::{DisconnectReason, Event},
    StreamPath, MAX_NAMES_PER_REQUEST, MAX_STREAMS_PER_CONNECTION, MIN_REQUEST_INTERVAL,
};

/// One change to a path's streams.
#[derive(Debug, Clone)]
pub(crate) enum Change {
    /// Subscribe to these streams; one already subscribed is skipped.
    Subscribe(Vec<StreamName>),
    /// Unsubscribe from these streams; one not subscribed is skipped.
    Unsubscribe(Vec<StreamName>),
}

/// Binance's protocol on one routed path.
///
/// Its membership is the path's streams, replayed on every fresh connection
/// at most 200 names a request, one request per 200 ms, with the 1024-stream
/// cap checked first (D9). Its divergence rows:
///
/// - **D4, the ping:** an empty protocol `Ping` on the wall clock.
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
#[derive(Debug)]
pub(crate) struct UsdmProtocol {
    base_url: String,
    path: StreamPath,
}

impl UsdmProtocol {
    pub(crate) fn new(base_url: String, path: StreamPath) -> Self {
        Self { base_url, path }
    }

    fn too_many(&self) -> UsdmWsError {
        UsdmWsError::TooManyStreams {
            path: self.path,
            limit: MAX_STREAMS_PER_CONNECTION,
        }
    }

    /// The streams of `add` not already in `streams`, once each, refused
    /// before anything is recorded or sent if they would pass the cap.
    fn new_streams(
        &self,
        streams: &[StreamName],
        add: Vec<StreamName>,
    ) -> Result<Vec<StreamName>, UsdmWsError> {
        let mut new: Vec<StreamName> = Vec::new();
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
fn gone_streams(streams: &[StreamName], remove: Vec<StreamName>) -> Vec<StreamName> {
    let mut gone: Vec<StreamName> = Vec::new();
    for stream in remove {
        if streams.contains(&stream) && !gone.contains(&stream) {
            gone.push(stream);
        }
    }
    gone
}

impl Protocol for UsdmProtocol {
    type Event = Event;
    type Error = UsdmWsError;
    type Membership = Vec<StreamName>;
    type Change = Change;
    /// The path's streams after the change.
    type Ack = Vec<StreamName>;

    fn handshake(&mut self) -> Result<Handshake, UsdmWsError> {
        Ok(self
            .path
            .url(&self.base_url)
            .into_client_request()
            .map_err(WsError::from)?)
    }

    /// Requests no faster than one per 200 ms. No answer timeout: the
    /// supervisor bounds each replay and change by `stale_after` (L2-D1).
    fn configure(&self, connection: Connection) -> Connection {
        connection.min_request_interval(MIN_REQUEST_INTERVAL)
    }

    /// Every stream, 200 names a request; the cap is checked first. A stream
    /// refused on a fresh connection stops the path, as retrying replays it.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        streams: &Vec<StreamName>,
    ) -> Result<(), UsdmWsError> {
        if streams.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(self.too_many());
        }
        for batch in streams.chunks(MAX_NAMES_PER_REQUEST) {
            request(connection, "SUBSCRIBE", Some(batch)).await?;
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
        streams: &mut Vec<StreamName>,
        change: Change,
    ) -> Result<Vec<StreamName>, UsdmWsError> {
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
        for (n, batch) in names.chunks(MAX_NAMES_PER_REQUEST).enumerate() {
            match request(connection, method, Some(batch)).await {
                Ok(_) => {}
                Err(err @ UsdmWsError::Refused { .. }) => {
                    let answered = &names[..n * MAX_NAMES_PER_REQUEST];
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
        streams: &mut Vec<StreamName>,
        change: Change,
    ) -> Offline<Change, Vec<StreamName>, UsdmWsError> {
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

    fn decode(&mut self, frame: &Message) -> Decoded<Event, UsdmWsError> {
        // A Binary frame is not part of the protocol (D12), and the kit
        // answers the server's Ping and its Close.
        let Message::Text(text) = frame else {
            return Decoded::Nothing;
        };
        match Update::from_json(text) {
            Ok(update) => Decoded::Event(Event::Update(Box::new(update))),
            // An answer nobody awaits, such as one to a request that
            // outlived the staleness window.
            Err(_) if Answer::parse(text).is_some() => Decoded::Nothing,
            Err(err) => Decoded::Error(err),
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<Event, UsdmWsError>) -> bool {
        true
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<Event, UsdmWsError>) -> bool {
        matches!(decoded, Decoded::Event(Event::Update(_)))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Ping(Vec::new().into()))
    }

    fn wanted(streams: &Vec<StreamName>) -> bool {
        !streams.is_empty()
    }

    fn is_retriable(err: &UsdmWsError) -> bool {
        err.is_retriable()
    }

    /// Binance's own recovery, not the class's: a server close of any code,
    /// a lost or timed out connection, an unanswered replay and a malformed
    /// answer reconnect; a frame that does not decode is skipped; a refusal
    /// and the client's own refusals stop.
    fn recover(&self, err: &UsdmWsError) -> Recovery {
        match err {
            UsdmWsError::Connect(err) => match err.kind() {
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
            UsdmWsError::ConnectTimeout(_)
            | UsdmWsError::Closed { .. }
            | UsdmWsError::NoAnswer { .. }
            | UsdmWsError::Response { .. } => Recovery::Reconnect,
            UsdmWsError::Frame { .. } => Recovery::Skip,
            UsdmWsError::Refused { .. }
            | UsdmWsError::TooManyStreams { .. }
            | UsdmWsError::WrongPath { .. }
            | UsdmWsError::Stopped => Recovery::Stop,
        }
    }

    fn disconnected(&mut self, outage: Outage<UsdmWsError>) -> Option<Event> {
        let reason = match outage {
            Outage::Closed { code, reason } => DisconnectReason::Closed { code, reason },
            Outage::Error(err) => DisconnectReason::Error(err),
            Outage::Rotation => DisconnectReason::Rotation,
            // `Stale`, and any outage a later kit adds.
            _ => DisconnectReason::Stale,
        };
        Some(Event::Disconnected {
            path: self.path,
            reason,
        })
    }

    fn reconnected(&mut self) -> Event {
        Event::Reconnected { path: self.path }
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(1)
    }

    /// A replay or a change the server left unanswered for the staleness
    /// window. The supervised tier answers a change so left `Ok`, as it is
    /// recorded and the replay applies it.
    fn stalled(silent: Duration) -> UsdmWsError {
        UsdmWsError::NoAnswer {
            id: 0,
            timeout: silent,
        }
    }

    fn stopped() -> UsdmWsError {
        UsdmWsError::Stopped
    }
}
