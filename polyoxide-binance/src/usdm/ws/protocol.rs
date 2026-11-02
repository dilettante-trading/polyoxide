//! Binance's socket protocol for one routed path, as the kit's `Supervisor`
//! drives it. The supervised tier runs one supervisor per path.

use std::{future::Future, sync::Arc, time::Duration};

use polyoxide_rt::Runtime;
use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    BoxSocket, Connection, Decoded, Kind, Offline, Outage, Protocol, Queue, Recovery,
    SocketTransport, WsError,
};

use crate::usdm::ws::{
    client::{request, request_frame, Answer},
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
/// - **D4, the ping:** an empty protocol `Ping` on the wall clock; on a
///   transport that cannot send one (`Capabilities::client_pings` false, a
///   browser's) a `LIST_SUBSCRIPTIONS` request under an id of its own, paced
///   as every request is (a tick inside `MIN_REQUEST_INTERVAL` of the last
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
#[derive(Debug)]
pub(crate) struct UsdmProtocol {
    base_url: String,
    path: StreamPath,
    /// Whether the transport of the connection last dialled can send protocol
    /// pings; read from its capabilities in `connect`.
    client_pings: bool,
}

impl UsdmProtocol {
    pub(crate) fn new(base_url: String, path: StreamPath) -> Self {
        Self {
            base_url,
            path,
            client_pings: true,
        }
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

    /// Dials the handshake through the transport, as the default does, and
    /// reads from the transport whether it can send a protocol ping (D9),
    /// which `ping` acts on for this connection.
    fn connect(
        &mut self,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
        timeout: Duration,
    ) -> impl Future<Output = Result<BoxSocket, UsdmWsError>> + Send {
        self.client_pings = transport.capabilities().client_pings;
        let request = self.handshake();
        async move {
            let socket = transport.connect(request?, timeout, runtime).await?;
            Ok::<_, UsdmWsError>(socket)
        }
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
        Some(request_frame("LIST_SUBSCRIPTIONS", None, id))
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
            | UsdmWsError::Stopped { .. } => Recovery::Stop,
        }
    }

    /// A refused handshake's `Retry-After`, as Binance's class reads it,
    /// which the reconnect waits out up to the backoff's ceiling (DRIFT R11).
    fn retry_after(&self, err: &UsdmWsError) -> Option<Duration> {
        err.retry_after()
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

    /// The kit's task stopped; the router adds the cause it saw.
    fn stopped() -> UsdmWsError {
        UsdmWsError::Stopped { cause: None }
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_ws::tungstenite::{self, http};

    use super::*;
    use crate::usdm::{types::Symbol, ws::USDM_WS_BASE};

    /// What the supervisor does about `err`, by Binance's recovery hook.
    fn recovery(err: &UsdmWsError) -> Recovery {
        UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market).recover(err)
    }

    fn http_error(status: u16) -> UsdmWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> UsdmWsError {
        UsdmWsError::from(WsError::from(err))
    }

    fn agg(symbol: &str) -> StreamName {
        StreamName::AggTrade(Symbol::new(symbol).unwrap())
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_and_one_refused_for_us_is_fatal() {
        assert_eq!(recovery(&http_error(503)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(429)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(408)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(425)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(451)), Recovery::Stop);
        assert_eq!(recovery(&http_error(404)), Recovery::Stop);
        assert_eq!(recovery(&http_error(418)), Recovery::Stop);
    }

    #[test]
    fn losing_the_connection_reconnects_and_a_bad_frame_is_skipped() {
        for err in [
            transport(tungstenite::Error::ConnectionClosed),
            UsdmWsError::ConnectTimeout(Duration::from_secs(10)),
            UsdmWsError::Closed {
                code: Some(1008),
                reason: "Too many requests".into(),
            },
            UsdmWsError::NoAnswer {
                id: 3,
                timeout: Duration::from_secs(10),
            },
            UsdmWsError::Response {
                id: 4,
                raw: "{\"id\":4}".into(),
            },
        ] {
            assert_eq!(recovery(&err), Recovery::Reconnect, "{err}");
        }
        let frame = UsdmWsError::Frame {
            stream: "btcusdt@aggTrade".into(),
            raw: "{}".into(),
            reason: "missing field".into(),
        };
        assert_eq!(recovery(&frame), Recovery::Skip);
    }

    #[test]
    fn a_refusal_and_a_request_the_client_refused_to_send_are_fatal() {
        let refused = UsdmWsError::Refused {
            code: 2,
            msg: "Invalid request".into(),
        };
        assert_eq!(recovery(&refused), Recovery::Stop);
        let full = UsdmWsError::TooManyStreams {
            path: StreamPath::Market,
            limit: 1024,
        };
        assert_eq!(recovery(&full), Recovery::Stop);
        assert_eq!(
            full.to_string(),
            "a market connection carries at most 1024 streams"
        );
        let wrong = UsdmWsError::WrongPath {
            stream: StreamName::BookTicker(Symbol::new("BTCUSDT").unwrap()),
            path: StreamPath::Market,
        };
        assert_eq!(recovery(&wrong), Recovery::Stop);
        assert_eq!(
            wrong.to_string(),
            "btcusdt@bookTicker rides the public path, not market"
        );
        let url = transport(tungstenite::Error::Url(
            tungstenite::error::UrlError::NoHostName,
        ));
        assert_eq!(recovery(&url), Recovery::Stop);
        assert_eq!(
            recovery(&UsdmWsError::Stopped { cause: None }),
            Recovery::Stop
        );
    }

    #[test]
    fn a_use_after_close_and_a_capacity_failure_reconnect_d19() {
        // D19 is Binance's: a connection used after it closed reconnects,
        // where perps and rtds stop. A frame or header past tungstenite's
        // limits does too, though `retrying_can_fix` alone gives up on it.
        assert_eq!(
            recovery(&transport(tungstenite::Error::AlreadyClosed)),
            Recovery::Reconnect
        );
        assert_eq!(
            recovery(&transport(tungstenite::Error::Capacity(
                tungstenite::error::CapacityError::TooManyHeaders
            ))),
            Recovery::Reconnect
        );
    }

    #[test]
    fn a_change_during_an_outage_is_recorded_and_answered_at_once() {
        let mut protocol = UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market);
        let mut streams = vec![agg("BTCUSDT")];
        let recorded = protocol.record(
            &mut streams,
            Change::Subscribe(vec![agg("ETHUSDT"), agg("BTCUSDT"), agg("ETHUSDT")]),
        );
        assert!(matches!(recorded, Offline::Recorded(Ok(ref s)) if s.len() == 2));
        let recorded = protocol.record(&mut streams, Change::Unsubscribe(vec![agg("BTCUSDT")]));
        assert!(matches!(recorded, Offline::Recorded(Ok(ref s)) if *s == [agg("ETHUSDT")]));
        // The cap holds offline too, and refuses before recording anything.
        let mut full: Vec<StreamName> = (0..1024).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
        let recorded = protocol.record(&mut full, Change::Subscribe(vec![agg("BTCUSDT")]));
        assert!(matches!(
            recorded,
            Offline::Recorded(Err(UsdmWsError::TooManyStreams { limit: 1024, .. }))
        ));
        assert_eq!(full.len(), 1024);
    }

    #[test]
    fn an_answer_nobody_awaits_decodes_to_nothing_and_a_bad_frame_to_an_error() {
        let mut protocol = UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market);
        let answer = Message::Text(r#"{"result":null,"id":99}"#.into());
        assert!(matches!(protocol.decode(&answer), Decoded::Nothing));
        let bad = Message::Text(
            r#"{"stream":"btcusdt@markPrice@1s","data":{"e":"markPriceUpdate"}}"#.into(),
        );
        let decoded = protocol.decode(&bad);
        assert!(matches!(decoded, Decoded::Error(UsdmWsError::Frame { .. })));
        assert!(!protocol.delivered(&bad, &decoded));
        assert!(protocol.is_live(&Message::Pong(Vec::new().into()), &Decoded::Nothing));
    }

    #[test]
    fn the_answer_to_a_list_subscriptions_keepalive_is_nothing_and_is_live_d9() {
        let mut protocol = UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market);
        let answer = Message::Text(r#"{"result":["btcusdt@markPrice@1s"],"id":7}"#.into());
        let decoded = protocol.decode(&answer);
        assert!(matches!(decoded, Decoded::Nothing));
        assert!(protocol.is_live(&answer, &decoded));
        assert!(!protocol.delivered(&answer, &decoded));
    }
}
