//! Kalshi's order-book socket protocol, as the kit's `Supervisor` drives it, and the
//! membership calls on its handle.

use std::{
    collections::{BTreeSet, HashMap},
    future::Future,
    time::Duration,
};

use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{
        client::IntoClientRequest,
        handshake::client::Request as Handshake,
        http::{HeaderName, HeaderValue},
        Message,
    },
    Connection, Correlated, Decoded, Kind, MembershipHandle, Offline, Outage, Protocol, Queue,
    Recovery, WsError,
};
use serde::Deserialize;
use serde_json::json;

use crate::predictions::{
    key::Ticker,
    ws::{
        event::{frames, DisconnectReason, Event},
        PredictionsWsError,
    },
    Credentials, CredentialsError,
};

/// Events the consumer may leave unread before the task stops reading.
const EVENT_QUEUE: usize = 1024;
/// The one channel this socket carries.
const CHANNEL: &str = "orderbook_delta";

/// One change to the subscribed markets, made through [`PredictionsMembership`] or
/// `MembershipHandle::apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    /// Subscribe to these markets; one already subscribed is skipped.
    Subscribe(Vec<Ticker>),
    /// Unsubscribe from these markets; one not subscribed is skipped.
    Unsubscribe(Vec<Ticker>),
}

/// Kalshi's order-book socket, which [`PredictionsWsBuilder`](crate::predictions::ws::PredictionsWsBuilder)
/// hands to the kit's `Supervisor`.
///
/// Its membership is the set of tickers subscribed, replayed on every fresh connection as
/// one `subscribe` of `orderbook_delta` with `use_yes_price: true`. A change goes out as an
/// `update_subscription` on the live subscription's `sid`; the first add with no
/// subscription sends a `subscribe`, and emptying the membership sends an `unsubscribe`
/// and keeps the connection. A change is recorded before its frame is sent: one whose
/// connection dies under it is answered `Ok` and replayed, one Kalshi refuses is undone,
/// and one made during an outage is answered at once and replayed. Its divergence rows:
///
/// - **D4, the ping:** an empty protocol Ping on the wall clock.
/// - **D5, liveness:** every frame, the server's Pings and the Pongs to the client's own
///   included.
/// - **D6, a delivered connection:** one that decoded a book frame.
/// - **D8:** `Event::Disconnected`, then `Event::Reconnected`, once each per outage.
/// - **D19:** a connection used after it closed reconnects.
/// - **A refused replay stops the stream:** Kalshi refusing the replay's `subscribe` with a
///   code that does not reconnect, or answering it with a frame that does not decode, ends
///   the stream with that error, where the same error frame, or a frame that does not
///   decode, elsewhere is yielded and skipped. A change made during an outage, or whose
///   connection died under it, was answered `Ok`, so this is the only place its caller
///   learns of the refusal.
/// - **The handshake** is signed afresh for every attempt, over the URL's path.
/// - **Sequence:** every frame that carries `sid` and `seq`, answers included, is checked
///   against the last of its subscription; a gap reconnects.
///
/// Its queue holds 1024 events.
pub struct PredictionsProtocol {
    url: String,
    credentials: Credentials,
    /// The live subscription's id on this connection, once Kalshi has answered a
    /// `subscribe`.
    sid: Option<u64>,
    /// What this connection's sequenced frames have said.
    sequences: Sequences,
    /// Whether Kalshi has just answered the replay's `subscribe` with an error frame, or
    /// with a frame that does not decode. Set by `replay` before it returns the error, so
    /// that `recover`, which the supervisor calls with that error straight after, can tell
    /// it from the same frame met on a live connection; `reset` clears it for the next
    /// attempt.
    replay_refused: bool,
}

impl std::fmt::Debug for PredictionsProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PredictionsProtocol")
            .field("url", &self.url)
            .field("credentials", &self.credentials)
            .field("sid", &self.sid)
            .finish_non_exhaustive()
    }
}

impl PredictionsProtocol {
    pub(crate) fn new(url: String, credentials: Credentials) -> Self {
        Self {
            url,
            credentials,
            sid: None,
            sequences: Sequences::default(),
            replay_refused: false,
        }
    }

    /// Note what Kalshi's answer to `command` settled.
    fn accepted(&mut self, command: &Command, answered: &Answered) {
        match (command, answered) {
            (Command::Subscribe { .. }, Answered::Subscribed(sid)) => self.sid = Some(*sid),
            (Command::Unsubscribe { sid }, _) => {
                self.sid = None;
                self.sequences.last.remove(sid);
            }
            _ => {}
        }
    }
}

/// What one connection's sequenced frames have said.
///
/// The first `seq` of a subscription is taken as it comes; after it, each must be the
/// previous plus one. The correlation closure of a request observes every sequenced frame
/// it reads while it waits, and counts in `ahead` each push it keeps for the stream, which
/// `decode` meets later and must not observe again. A gap the closure finds is held in
/// `gap` for the next frame `decode` sees.
#[derive(Debug, Default)]
struct Sequences {
    last: HashMap<u64, u64>,
    ahead: usize,
    gap: Option<PredictionsWsError>,
}

impl Sequences {
    fn advance(&mut self, sid: u64, seq: u64) -> Result<(), PredictionsWsError> {
        match self.last.insert(sid, seq) {
            Some(previous) if seq != previous.saturating_add(1) => {
                Err(PredictionsWsError::SequenceGap {
                    sid,
                    expected: previous.saturating_add(1),
                    received: seq,
                })
            }
            _ => Ok(()),
        }
    }
}

/// The parts of a frame the protocol reads before it reads the rest.
#[derive(Debug, Deserialize)]
struct Peek {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    sid: Option<u64>,
    #[serde(default)]
    seq: Option<u64>,
}

impl Peek {
    fn sequence(&self) -> Option<(u64, u64)> {
        Some((self.sid?, self.seq?))
    }

    /// Whether the frame answers a command. Kalshi counts an `id` of 0 as none.
    fn answers_a_command(&self) -> bool {
        matches!(
            self.kind.as_deref(),
            Some("subscribed" | "ok" | "unsubscribed" | "error")
        ) && self.id.is_some_and(|id| id != 0)
    }
}

fn parse<'a, T: Deserialize<'a>>(text: &'a str) -> Result<T, PredictionsWsError> {
    serde_json::from_str(text).map_err(|source| decode_error(text, source))
}

fn decode_error(text: &str, source: serde_json::Error) -> PredictionsWsError {
    PredictionsWsError::Json {
        text: text.to_owned(),
        source,
    }
}

/// What a command's answer says.
#[derive(Debug, PartialEq, Eq)]
enum Answered {
    /// `subscribed`, with the new subscription's id.
    Subscribed(u64),
    /// `ok` or `unsubscribed`.
    Done,
}

fn answered(kind: &str, text: &str) -> Result<Answered, PredictionsWsError> {
    match kind {
        "subscribed" => Ok(Answered::Subscribed(
            parse::<frames::Subscribed>(text)?.msg.sid,
        )),
        "error" => {
            let rejected = parse::<frames::Rejected>(text)?;
            Err(PredictionsWsError::Server {
                code: rejected.msg.code,
                message: rejected.msg.msg,
            })
        }
        _ => Ok(Answered::Done),
    }
}

/// The correlation of a request: the frame answering command `id`, and the sequence
/// bookkeeping of every frame read meanwhile.
///
/// A frame that answers another command is dropped by the kit, after it has counted here;
/// a frame that answers none is kept for the stream, and counted in `ahead` if sequenced.
fn correlate(
    sequences: &mut Sequences,
    id: u64,
    frame: &Message,
) -> Correlated<Result<Answered, PredictionsWsError>> {
    let Message::Text(text) = frame else {
        return Correlated::Push;
    };
    // A frame that is not JSON is the stream's to report.
    let Ok(peek) = serde_json::from_str::<Peek>(text.as_str()) else {
        return Correlated::Push;
    };
    let sequenced = peek.sequence();
    if let Some((sid, seq)) = sequenced {
        if let Err(gap) = sequences.advance(sid, seq) {
            if sequences.gap.is_none() {
                sequences.gap = Some(gap);
            }
        }
    }
    if peek.answers_a_command() {
        if peek.id != Some(id) {
            return Correlated::Uncorrelated;
        }
        let kind = peek.kind.as_deref().unwrap_or_default();
        return Correlated::Answer(answered(kind, text.as_str()));
    }
    if sequenced.is_some() {
        sequences.ahead += 1;
    }
    Correlated::Push
}

/// A command to Kalshi, before it has an id.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Subscribe {
        tickers: Vec<String>,
    },
    Update {
        sid: u64,
        action: &'static str,
        tickers: Vec<String>,
    },
    Unsubscribe {
        sid: u64,
    },
}

impl Command {
    /// The frame, for the connection's request `id`.
    fn frame(&self, id: u64) -> String {
        match self {
            Self::Subscribe { tickers } => json!({
                "id": id,
                "cmd": "subscribe",
                "params": {
                    "channels": [CHANNEL],
                    "market_tickers": tickers,
                    "use_yes_price": true,
                },
            }),
            Self::Update {
                sid,
                action,
                tickers,
            } => json!({
                "id": id,
                "cmd": "update_subscription",
                "params": {"sid": sid, "action": action, "market_tickers": tickers},
            }),
            Self::Unsubscribe { sid } => json!({
                "id": id,
                "cmd": "unsubscribe",
                "params": {"sids": [sid]},
            }),
        }
        .to_string()
    }
}

/// Send `command` as the connection's next request and read its answer.
async fn send(
    sequences: &mut Sequences,
    connection: &mut Connection,
    command: &Command,
) -> Result<Answered, PredictionsWsError> {
    match connection
        .request(
            |id| Message::Text(command.frame(id).into()),
            |id, frame| correlate(sequences, id, frame),
        )
        .await
    {
        Ok(answered) => answered,
        Err(err) => Err(err.into()),
    }
}

/// The tickers' text, for a frame.
fn names<'a>(tickers: impl IntoIterator<Item = &'a Ticker>) -> Vec<String> {
    tickers
        .into_iter()
        .map(|ticker| ticker.as_str().to_owned())
        .collect()
}

/// The tickers of `add` that `membership` lacks, once each.
fn added(membership: &BTreeSet<Ticker>, add: Vec<Ticker>) -> Vec<Ticker> {
    add.into_iter()
        .filter(|ticker| !membership.contains(ticker))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The tickers of `remove` that `membership` holds, once each.
fn removed(membership: &BTreeSet<Ticker>, remove: Vec<Ticker>) -> Vec<Ticker> {
    remove
        .into_iter()
        .filter(|ticker| membership.contains(ticker))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The handshake request for one connection attempt, signed afresh: Kalshi's three
/// headers over `timestamp + "GET" + the URL's path`, by the same signer as REST.
///
/// Doc-hidden: the live suite signs its raw socket with it. Not API.
pub fn signed_request(
    credentials: &Credentials,
    url: &str,
) -> Result<Handshake, PredictionsWsError> {
    let mut request = url.into_client_request().map_err(WsError::from)?;
    let signed = credentials.sign_request("GET", request.uri().path())?;
    for (name, value) in signed.pairs() {
        let header = HeaderName::from_bytes(name.as_bytes()).map_err(|_| unfit(name))?;
        let value = HeaderValue::from_str(value).map_err(|_| unfit(name))?;
        request.headers_mut().insert(header, value);
    }
    Ok(request)
}

/// A header value that cannot be sent, as the signing error it is, as REST's is: nothing
/// reached the host, so it is not a transport error.
fn unfit(name: &str) -> PredictionsWsError {
    PredictionsWsError::Credentials(CredentialsError::Sign {
        reason: format!("the {name} value is not a valid header value"),
    })
}

impl Protocol for PredictionsProtocol {
    type Event = Event;
    type Error = PredictionsWsError;
    type Membership = BTreeSet<Ticker>;
    type Change = Change;
    type Ack = ();

    /// Signed afresh for every attempt: the supervisor calls this for the first and for
    /// each reconnect, and the default `connect` dials its request through the kit.
    fn handshake(&mut self) -> Result<Handshake, PredictionsWsError> {
        signed_request(&self.credentials, &self.url)
    }

    /// One `subscribe` for every ticker; nothing for none. A refusal on a fresh connection
    /// fails the attempt, which recovery decides on: Kalshi's error frame, or an answer
    /// that does not decode, is noted first, so that `recover` stops on a code that would
    /// not reconnect and on an answer that would never decode.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &BTreeSet<Ticker>,
    ) -> Result<(), PredictionsWsError> {
        if membership.is_empty() {
            return Ok(());
        }
        let command = Command::Subscribe {
            tickers: names(membership.iter()),
        };
        let answered = match send(&mut self.sequences, connection, &command).await {
            Ok(answered) => answered,
            Err(err) => {
                self.replay_refused = matches!(
                    err,
                    PredictionsWsError::Server { .. } | PredictionsWsError::Json { .. }
                );
                return Err(err);
            }
        };
        self.accepted(&command, &answered);
        Ok(())
    }

    /// A change on the live connection, recorded before it is sent. Kalshi refusing it, or
    /// answering with a frame that does not read, undoes it and is the caller's error; the
    /// connection failing under it leaves it recorded and answers `Ok`, since the next read
    /// ends the connection and the replay applies it.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut BTreeSet<Ticker>,
        change: Change,
    ) -> Result<(), PredictionsWsError> {
        let before = membership.clone();
        let command = match change {
            Change::Subscribe(add) => {
                let new = added(membership, add);
                if new.is_empty() {
                    return Ok(());
                }
                membership.extend(new.iter().cloned());
                match self.sid {
                    Some(sid) => Command::Update {
                        sid,
                        action: "add_markets",
                        tickers: names(&new),
                    },
                    // Nothing is subscribed on this connection yet: the whole
                    // membership goes in one subscribe.
                    None => Command::Subscribe {
                        tickers: names(membership.iter()),
                    },
                }
            }
            Change::Unsubscribe(remove) => {
                let gone = removed(membership, remove);
                if gone.is_empty() {
                    return Ok(());
                }
                for ticker in &gone {
                    membership.remove(ticker);
                }
                match self.sid {
                    // Nothing was ever subscribed on this connection.
                    None => return Ok(()),
                    Some(sid) if membership.is_empty() => Command::Unsubscribe { sid },
                    Some(sid) => Command::Update {
                        sid,
                        action: "delete_markets",
                        tickers: names(&gone),
                    },
                }
            }
        };
        match send(&mut self.sequences, connection, &command).await {
            Ok(answered) => {
                self.accepted(&command, &answered);
                Ok(())
            }
            Err(PredictionsWsError::Connection(err)) => {
                tracing::debug!(%err, "the connection failed under a change; it is recorded for the replay");
                Ok(())
            }
            Err(err) => {
                *membership = before;
                Err(err)
            }
        }
    }

    /// A change during an outage is recorded and answered at once; the replay applies it.
    fn record(
        &mut self,
        membership: &mut BTreeSet<Ticker>,
        change: Change,
    ) -> Offline<Change, (), PredictionsWsError> {
        match change {
            Change::Subscribe(add) => membership.extend(add),
            Change::Unsubscribe(remove) => {
                for ticker in &remove {
                    membership.remove(ticker);
                }
            }
        }
        Offline::Recorded(Ok(()))
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Event, PredictionsWsError> {
        // A gap an answer showed is raised by the first frame after it, whatever its kind.
        if let Some(gap) = self.sequences.gap.take() {
            return Decoded::Error(gap);
        }
        // The kit answers the server's Ping and its Close, and a Binary frame is not part
        // of the protocol.
        let Message::Text(text) = frame else {
            return Decoded::Nothing;
        };
        let text = text.as_str();
        let peek = match serde_json::from_str::<Peek>(text) {
            Ok(peek) => peek,
            Err(source) => return Decoded::Error(decode_error(text, source)),
        };
        if let Some((sid, seq)) = peek.sequence() {
            if self.sequences.ahead > 0 {
                // The correlation closure counted this frame while a request waited.
                self.sequences.ahead -= 1;
            } else if let Err(gap) = self.sequences.advance(sid, seq) {
                return Decoded::Error(gap);
            }
        }
        match peek.kind.as_deref() {
            Some("orderbook_snapshot") => match parse::<frames::Snapshot>(text) {
                Ok(frame) => Decoded::Event(Event::Snapshot(frame.msg.into())),
                Err(err) => Decoded::Error(err),
            },
            Some("orderbook_delta") => match parse::<frames::Delta>(text) {
                Ok(frame) => Decoded::Event(Event::Delta(frame.msg.into())),
                Err(err) => Decoded::Error(err),
            },
            // An error that answers no command of ours is Kalshi reporting on a
            // subscription.
            Some("error") if !peek.answers_a_command() => match parse::<frames::Rejected>(text) {
                Ok(frame) => Decoded::Error(PredictionsWsError::Server {
                    code: frame.msg.code,
                    message: frame.msg.msg,
                }),
                Err(err) => Decoded::Error(err),
            },
            // An answer nobody awaits, and every type this socket does not carry.
            _ => Decoded::Nothing,
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<Event, PredictionsWsError>) -> bool {
        true
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<Event, PredictionsWsError>) -> bool {
        matches!(decoded, Decoded::Event(_) | Decoded::Events(_))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Ping(Vec::new().into()))
    }

    fn is_retriable(err: &PredictionsWsError) -> bool {
        err.is_retriable()
    }

    /// Kalshi's recovery: a frame that does not decode is skipped on a live connection and
    /// stops the stream when it was the answer to the replay's `subscribe`; an error frame's
    /// code decides (9 stops the stream, 10, 17, 18, 25 and 27 reconnect, any other is
    /// skipped on a live connection and stops the stream when it refused the replay's
    /// `subscribe`); a gap reconnects; a connection used after it closed reconnects (D19);
    /// and anything else reconnects exactly when its class is retriable, so a refused
    /// handshake's 401 or 403 stops, and a 429 or 5xx retries. No code here matches over
    /// `tungstenite::Error`: the socket table decides the class.
    ///
    /// A skipped error during an attempt reads as a reconnect, the kit's rule for a failed
    /// attempt, which is why a refused replay has its own row: it would otherwise
    /// reconnect for ever, refused each time, and so would an answer that never decodes.
    fn recover(&self, err: &PredictionsWsError) -> Recovery {
        match err {
            PredictionsWsError::Json { .. } if self.replay_refused => Recovery::Stop,
            PredictionsWsError::Json { .. } => Recovery::Skip,
            PredictionsWsError::Server { code, .. } => match code {
                9 => Recovery::Stop,
                10 | 17 | 18 | 25 | 27 => Recovery::Reconnect,
                _ if self.replay_refused => Recovery::Stop,
                _ => Recovery::Skip,
            },
            PredictionsWsError::SequenceGap { .. } => Recovery::Reconnect,
            PredictionsWsError::Connection(err) if matches!(err.kind(), Kind::AlreadyClosed) => {
                Recovery::Reconnect
            }
            err if err.is_retriable() => Recovery::Reconnect,
            _ => Recovery::Stop,
        }
    }

    /// A refused upgrade's `Retry-After`, as Kalshi's class reads it, which the reconnect
    /// waits out up to the backoff's ceiling and never undercuts (DRIFT R11).
    fn retry_after(&self, err: &PredictionsWsError) -> Option<Duration> {
        err.retry_after()
    }

    fn disconnected(&mut self, outage: Outage<PredictionsWsError>) -> Option<Event> {
        Some(Event::Disconnected {
            reason: DisconnectReason::from(outage),
        })
    }

    fn reconnected(&mut self) -> Event {
        Event::Reconnected
    }

    fn reset(&mut self) {
        self.sid = None;
        self.sequences = Sequences::default();
        self.replay_refused = false;
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(EVENT_QUEUE)
    }
}

/// The membership calls of a supervised Kalshi socket, on its `MembershipHandle`.
///
/// Each waits for the task to take the change and for Kalshi's answer. During an outage
/// the change is recorded and answered at once, and replayed on the next connection; if
/// Kalshi refuses that replay with a code that does not reconnect, or answers it with a
/// frame that does not decode, the stream yields the error and ends, since the caller was
/// told `Ok`. The task takes no change while the consumer's queue is full (1024 unread
/// events), so a call made from the task that reads the stream can wait for ever then:
/// make changes from another task, or keep reading while one is pending.
pub trait PredictionsMembership {
    /// Subscribe to more markets. The first add with nothing subscribed sends a
    /// `subscribe`; later ones go as `update_subscription` on the live subscription. A
    /// market already subscribed is skipped.
    ///
    /// Fails with [`PredictionsWsError::Server`] when Kalshi refuses the change, which
    /// leaves the membership as it was; with a stall when Kalshi left it unanswered for
    /// the staleness window (the connection is then replaced); or once the stream has
    /// stopped.
    fn subscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send;

    /// Unsubscribe from markets, failing as [`subscribe`](Self::subscribe) does.
    /// Unsubscribing every market sends an `unsubscribe` and keeps the connection, which
    /// keeps pinging; a reconnect then replays nothing.
    fn unsubscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send;
}

impl PredictionsMembership for MembershipHandle<PredictionsProtocol> {
    fn subscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send {
        self.apply(Change::Subscribe(tickers.into_iter().collect()))
    }

    fn unsubscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send {
        self.apply(Change::Unsubscribe(tickers.into_iter().collect()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use polyoxide_test_support::fixtures;
    use polyoxide_venue::{Level, UnixMillis};
    use polyoxide_ws::tungstenite::{self, http};
    use ring::signature::{UnparsedPublicKey, ED25519};
    use rust_decimal::Decimal;
    use serde_json::{json, Value};

    use super::*;
    use crate::predictions::{
        auth::{ACCESS_KEY, ACCESS_SIGNATURE, ACCESS_TIMESTAMP},
        types::BookSide,
    };

    // The mirror's own examples (`docs/specs/kalshi/asyncapi.yaml`, version 2.0.0).
    const SNAPSHOT: &str = r#"{"type":"orderbook_snapshot","sending_ts_ms":1669149841234,"sid":2,"seq":2,"msg":{"market_ticker":"FED-23DEC-T3.00","market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","yes_dollars_fp":[["0.0800","300.00"],["0.2200","333.00"]],"no_dollars_fp":[["0.5400","20.00"],["0.5600","146.00"]]}}"#;
    const DELTA: &str = r#"{"type":"orderbook_delta","sending_ts_ms":1669149841123,"sid":2,"seq":3,"msg":{"market_ticker":"FED-23DEC-T3.00","market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","price_dollars":"0.9600","delta_fp":"-54.00","side":"yes","ts":"2022-11-22T20:44:01Z","ts_ms":1669149841000}}"#;
    const SUBSCRIBED: &str = r#"{"id":1,"type":"subscribed","sending_ts_ms":1669149841234,"msg":{"channel":"orderbook_delta","sid":1}}"#;
    const UNSUBSCRIBED: &str =
        r#"{"id":102,"sid":2,"seq":7,"type":"unsubscribed","sending_ts_ms":1669149841234}"#;
    const ERROR: &str = r#"{"id":123,"type":"error","sending_ts_ms":1669149841234,"msg":{"code":7,"msg":"Unknown subscription ID"}}"#;

    fn credentials() -> Credentials {
        credentials_with_id("test-key-id")
    }

    fn credentials_with_id(key_id: &str) -> Credentials {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let body = vectors["ed25519"]["pkcs8_body"]
            .as_str()
            .expect("the vectors hold an Ed25519 key");
        // The fixture stores the key body without its armour lines (spec §6.4).
        let pem = format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n");
        Credentials::from_pem(key_id, &pem).expect("the vector key parses")
    }

    fn protocol() -> PredictionsProtocol {
        protocol_for(credentials())
    }

    fn protocol_for(credentials: Credentials) -> PredictionsProtocol {
        PredictionsProtocol::new("ws://127.0.0.1:9/trade-api/ws/v2".to_owned(), credentials)
    }

    fn text(frame: &str) -> Message {
        Message::Text(frame.into())
    }

    fn decoded(
        protocol: &mut PredictionsProtocol,
        frame: &str,
    ) -> Decoded<Event, PredictionsWsError> {
        protocol.decode(&text(frame))
    }

    fn answer(
        sequences: &mut Sequences,
        id: u64,
        frame: &str,
    ) -> Correlated<Result<Answered, PredictionsWsError>> {
        correlate(sequences, id, &text(frame))
    }

    fn snapshot_frame(sid: u64, seq: u64) -> String {
        json!({"type": "orderbook_snapshot", "sid": sid, "seq": seq, "msg": {
            "market_ticker": "A-1", "market_id": "id",
            "yes_dollars_fp": [["0.1000", "1.00"]], "no_dollars_fp": [["0.9000", "2.00"]],
        }})
        .to_string()
    }

    fn delta_frame(sid: u64, seq: u64, side: &str) -> String {
        json!({"type": "orderbook_delta", "sid": sid, "seq": seq, "msg": {
            "market_ticker": "A-1", "market_id": "id",
            "price_dollars": "0.5000", "delta_fp": "1.00", "side": side,
        }})
        .to_string()
    }

    fn ok_frame(id: u64, sid: u64, seq: u64) -> String {
        json!({"id": id, "sid": sid, "seq": seq, "type": "ok", "msg": {"market_tickers": ["A-1"]}})
            .to_string()
    }

    fn level(price: &str, size: &str) -> Level {
        Level::new(price.parse().unwrap(), size.parse().unwrap())
    }

    fn server(code: u32) -> PredictionsWsError {
        PredictionsWsError::Server {
            code,
            message: "a message".to_owned(),
        }
    }

    /// An upgrade refused with `status`, and a `Retry-After` when one is given.
    fn refused(status: u16, retry_after: Option<&str>) -> PredictionsWsError {
        let mut response = http::Response::builder().status(status);
        if let Some(wait) = retry_after {
            response = response.header("Retry-After", wait);
        }
        PredictionsWsError::from(WsError::from(tungstenite::Error::Http(
            response.body(None).unwrap(),
        )))
    }

    fn fixture(name: &str) -> String {
        fixtures!("predictions/ws").text(name)
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
            .collect()
    }

    #[test]
    fn recovery_follows_the_rows_of_the_error_codes() {
        let protocol = protocol();
        assert_eq!(protocol.recover(&server(9)), Recovery::Stop);
        // Kalshi calls codes 10 and 25 terminal ("the user must resubscribe"): the
        // reconnect's replay does. Codes 17 and 18 are the venue's own failures.
        for code in [10, 17, 18, 25, 27] {
            assert_eq!(
                protocol.recover(&server(code)),
                Recovery::Reconnect,
                "{code}"
            );
        }
        for code in [1, 7, 8, 11, 14, 26] {
            assert_eq!(protocol.recover(&server(code)), Recovery::Skip, "{code}");
        }
        let json = PredictionsWsError::Json {
            text: "x".to_owned(),
            source: serde_json::from_str::<u8>("x").unwrap_err(),
        };
        assert_eq!(protocol.recover(&json), Recovery::Skip);
        let gap = PredictionsWsError::SequenceGap {
            sid: 1,
            expected: 2,
            received: 4,
        };
        assert_eq!(protocol.recover(&gap), Recovery::Reconnect);
        let url = PredictionsWsError::InvalidUrl {
            url: "x".to_owned(),
            reason: "y",
        };
        assert_eq!(protocol.recover(&url), Recovery::Stop);
        let signing = PredictionsWsError::Credentials(CredentialsError::Sign {
            reason: "x".to_owned(),
        });
        assert_eq!(protocol.recover(&signing), Recovery::Stop);
    }

    #[test]
    fn a_refused_replay_stops_on_a_code_that_would_otherwise_be_skipped() {
        let mut protocol = protocol();
        // On a live connection an ordinary code is yielded and skipped.
        assert_eq!(protocol.recover(&server(11)), Recovery::Skip);
        // Refusing the replay's `subscribe`, the same frame ends the stream; the codes
        // that reconnect or stop do so as before.
        protocol.replay_refused = true;
        assert_eq!(protocol.recover(&server(11)), Recovery::Stop);
        assert_eq!(protocol.recover(&server(9)), Recovery::Stop);
        for code in [10, 17, 18, 25, 27] {
            assert_eq!(
                protocol.recover(&server(code)),
                Recovery::Reconnect,
                "{code}"
            );
        }
        // The next connection starts clean.
        protocol.reset();
        assert_eq!(protocol.recover(&server(11)), Recovery::Skip);
    }

    #[test]
    fn an_undecodable_replay_answer_stops_where_the_same_frame_on_a_live_connection_is_skipped() {
        let mut protocol = protocol();
        let json = || PredictionsWsError::Json {
            text: "x".to_owned(),
            source: serde_json::from_str::<u8>("x").unwrap_err(),
        };
        // On a live connection a frame that does not decode is yielded and skipped.
        assert_eq!(protocol.recover(&json()), Recovery::Skip);
        // As the answer to the replay's `subscribe` it ends the stream, since a skip
        // during an attempt is a reconnect and the answer would never decode.
        protocol.replay_refused = true;
        assert_eq!(protocol.recover(&json()), Recovery::Stop);
        // The next connection starts clean.
        protocol.reset();
        assert_eq!(protocol.recover(&json()), Recovery::Skip);
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_but_a_refused_request_stops() {
        let protocol = protocol();
        for status in [408, 425, 429, 500, 503] {
            let err = refused(status, None);
            assert_eq!(protocol.recover(&err), Recovery::Reconnect, "{status}");
            assert!(err.is_retriable(), "{status}");
        }
        for status in [400, 401, 403, 404, 418, 451] {
            assert_eq!(
                protocol.recover(&refused(status, None)),
                Recovery::Stop,
                "{status}"
            );
        }
    }

    #[test]
    fn the_kits_own_failures_follow_their_class_and_a_use_after_close_reconnects() {
        let protocol = protocol();
        for err in [
            WsError::connect_timeout(Duration::from_secs(10)),
            WsError::no_answer(Duration::from_secs(1)),
            WsError::closed(Some(1000), "bye"),
            WsError::closed(Some(1001), "going away"),
            WsError::closed(Some(1011), "internal"),
            WsError::closed(None, ""),
            WsError::from(tungstenite::Error::ConnectionClosed),
            // A ping that raced a server close: the class is InvalidRequest, and the
            // connection used after it closed still reconnects.
            WsError::from(tungstenite::Error::AlreadyClosed),
        ] {
            let err = PredictionsWsError::from(err);
            assert_eq!(protocol.recover(&err), Recovery::Reconnect, "{err:?}");
        }
        for err in [
            WsError::closed(Some(1008), "policy"),
            WsError::from(tungstenite::Error::AttackAttempt),
            WsError::from(tungstenite::Error::Capacity(
                tungstenite::error::CapacityError::TooManyHeaders,
            )),
        ] {
            let err = PredictionsWsError::from(err);
            assert_eq!(protocol.recover(&err), Recovery::Stop, "{err:?}");
        }
    }

    #[test]
    fn a_refusals_retry_after_is_the_wait_the_reconnect_floors() {
        // The kit waits at least what this returns, up to its ceiling (DRIFT R11). A
        // protocol that returned nothing would reconnect on its own schedule into a 429.
        let protocol = protocol();
        let wait = Some(Duration::from_secs(30));
        assert_eq!(protocol.retry_after(&refused(429, Some("30"))), wait);
        assert_eq!(protocol.retry_after(&refused(503, Some("30"))), wait);
        assert_eq!(protocol.retry_after(&refused(503, None)), None);
        // A zero is no wait: the backoff's own step is the floor.
        assert_eq!(protocol.retry_after(&refused(429, Some("0"))), None);
        assert_eq!(protocol.retry_after(&server(27)), None);
    }

    #[test]
    fn commands_are_written_as_the_asyncapi_gives_them() {
        let parsed = |command: &Command, id: u64| -> Value {
            serde_json::from_str(&command.frame(id)).unwrap()
        };
        let subscribe = Command::Subscribe {
            tickers: vec!["A-1".to_owned(), "B-2".to_owned()],
        };
        assert_eq!(
            parsed(&subscribe, 1),
            json!({"id": 1, "cmd": "subscribe", "params": {
                "channels": ["orderbook_delta"],
                "market_tickers": ["A-1", "B-2"],
                "use_yes_price": true,
            }})
        );
        let add = Command::Update {
            sid: 4,
            action: "add_markets",
            tickers: vec!["C-3".to_owned()],
        };
        assert_eq!(
            parsed(&add, 2),
            json!({"id": 2, "cmd": "update_subscription", "params": {
                "sid": 4, "action": "add_markets", "market_tickers": ["C-3"],
            }})
        );
        assert_eq!(
            parsed(&Command::Unsubscribe { sid: 4 }, 3),
            json!({"id": 3, "cmd": "unsubscribe", "params": {"sids": [4]}})
        );
    }

    #[test]
    fn an_answer_is_matched_by_its_id() {
        let mut sequences = Sequences::default();
        assert!(matches!(
            answer(&mut sequences, 1, SUBSCRIBED),
            Correlated::Answer(Ok(Answered::Subscribed(1)))
        ));
        // An answer to another command nobody awaits any more.
        assert!(matches!(
            answer(&mut sequences, 2, SUBSCRIBED),
            Correlated::Uncorrelated
        ));
        assert!(matches!(
            answer(&mut sequences, 8, &ok_frame(8, 3, 1)),
            Correlated::Answer(Ok(Answered::Done))
        ));
        assert!(matches!(
            answer(&mut sequences, 123, ERROR),
            Correlated::Answer(Err(PredictionsWsError::Server { code: 7, .. }))
        ));
        // An error with no id is Kalshi reporting on a subscription: the stream's.
        let scoped = r#"{"type":"error","sid":3,"seq":2,"msg":{"code":25,"msg":"Subscription buffer overflow"}}"#;
        assert!(matches!(
            answer(&mut sequences, 9, scoped),
            Correlated::Push
        ));
        // A frame that is not JSON is left for the stream to report.
        assert!(matches!(
            answer(&mut sequences, 9, "not json"),
            Correlated::Push
        ));
    }

    #[test]
    fn a_push_read_while_a_request_waits_is_counted_once() {
        // Bugs: counting a kept push twice, which reads the next frame as a gap, and not
        // counting it at all, which reads the answer behind it as one.
        let mut protocol = protocol();
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(1, 1)),
            Decoded::Event(Event::Snapshot(_))
        ));
        // On the wire: a delta (2), then the ok (3) the request waits for.
        let mut sequences = std::mem::take(&mut protocol.sequences);
        assert!(matches!(
            answer(&mut sequences, 5, &delta_frame(1, 2, "yes")),
            Correlated::Push
        ));
        assert!(matches!(
            answer(&mut sequences, 5, &ok_frame(5, 1, 3)),
            Correlated::Answer(Ok(Answered::Done))
        ));
        protocol.sequences = sequences;
        assert!(protocol.sequences.gap.is_none());
        // The kept delta is decoded afterwards, and is not counted again.
        assert!(matches!(
            decoded(&mut protocol, &delta_frame(1, 2, "yes")),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_frame(1, 4, "no")),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(protocol.sequences.gap.is_none());
    }

    #[test]
    fn a_gap_in_an_answer_is_raised_by_the_next_frame() {
        // The request that read the ok is for another command, so it cannot return the
        // gap; the next frame does, whatever its kind.
        let mut protocol = protocol();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        assert!(matches!(
            correlate(&mut protocol.sequences, 5, &text(&ok_frame(5, 1, 3))),
            Correlated::Answer(Ok(Answered::Done))
        ));
        match protocol.decode(&Message::Ping(Vec::new().into())) {
            Decoded::Error(PredictionsWsError::SequenceGap {
                sid: 1,
                expected: 2,
                received: 3,
            }) => {}
            other => panic!("expected the gap, got {other:?}"),
        }
        // It is raised once.
        assert!(matches!(
            protocol.decode(&Message::Ping(Vec::new().into())),
            Decoded::Nothing
        ));
    }

    #[test]
    fn a_snapshot_is_the_yes_book_best_first() {
        let mut protocol = protocol();
        let Decoded::Event(Event::Snapshot(book)) = decoded(&mut protocol, SNAPSHOT) else {
            panic!("not a snapshot");
        };
        assert_eq!(book.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(book.market_id, "9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1");
        assert_eq!(
            book.bids,
            [level("0.2200", "333.00"), level("0.0800", "300.00")]
        );
        assert_eq!(
            book.asks,
            [level("0.5400", "20.00"), level("0.5600", "146.00")]
        );
    }

    #[test]
    fn a_delta_is_a_bid_or_an_ask_in_yes_pricing() {
        let mut protocol = protocol();
        let Decoded::Event(Event::Delta(bid)) = decoded(&mut protocol, DELTA) else {
            panic!("not a delta");
        };
        assert_eq!(bid.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(bid.side, BookSide::Bid);
        assert_eq!(bid.price, "0.9600".parse::<Decimal>().unwrap());
        assert_eq!(bid.delta, "-54.00".parse::<Decimal>().unwrap());
        assert_eq!(bid.ts, Some(UnixMillis(1_669_149_841_000)));
        let Decoded::Event(Event::Delta(ask)) = decoded(&mut protocol, &delta_frame(2, 4, "no"))
        else {
            panic!("not a delta");
        };
        assert_eq!(ask.side, BookSide::Ask);
    }

    #[test]
    fn answers_nobody_awaits_and_unknown_types_are_nothing() {
        for frame in [
            SUBSCRIBED,
            UNSUBSCRIBED,
            ERROR,
            r#"{"type":"ok","id":9,"msg":{}}"#,
            r#"{"type":"ticker","sid":5,"msg":{}}"#,
            "{}",
        ] {
            let mut protocol = protocol();
            assert!(
                matches!(decoded(&mut protocol, frame), Decoded::Nothing),
                "{frame}"
            );
        }
    }

    #[test]
    fn an_error_with_no_id_is_kalshi_reporting_on_a_subscription() {
        let mut protocol = protocol();
        let scoped = r#"{"type":"error","sid":3,"seq":1,"msg":{"code":25,"msg":"Subscription buffer overflow"}}"#;
        match decoded(&mut protocol, scoped) {
            Decoded::Error(PredictionsWsError::Server { code, message }) => {
                assert_eq!(code, 25);
                assert_eq!(message, "Subscription buffer overflow");
            }
            other => panic!("expected the server's error, got {other:?}"),
        }
    }

    #[test]
    fn a_frame_that_is_not_json_is_a_decode_error() {
        let mut protocol = protocol();
        match decoded(&mut protocol, "not json") {
            Decoded::Error(PredictionsWsError::Json { text, .. }) => {
                assert_eq!(text, "not json");
            }
            other => panic!("expected a decode error, got {other:?}"),
        }
        // A frame of a known type that cannot be read is the same error.
        let no_ticker = r#"{"type":"orderbook_snapshot","sid":1,"seq":1,"msg":{"market_id":"id"}}"#;
        assert!(matches!(
            decoded(&mut protocol, no_ticker),
            Decoded::Error(PredictionsWsError::Json { .. })
        ));
    }

    #[test]
    fn control_frames_are_nothing() {
        let mut protocol = protocol();
        for frame in [
            Message::Ping(b"heartbeat".to_vec().into()),
            Message::Pong(Vec::new().into()),
            Message::Binary(vec![1, 2, 3].into()),
            Message::Close(None),
        ] {
            assert!(
                matches!(protocol.decode(&frame), Decoded::Nothing),
                "{frame:?}"
            );
            // Every frame is liveness, and none is a delivered book.
            assert!(protocol.is_live(&frame, &Decoded::Nothing));
            assert!(!protocol.delivered(&frame, &Decoded::Nothing));
        }
    }

    #[test]
    fn a_skipped_sequence_number_is_a_gap_per_subscription() {
        let mut protocol = protocol();
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(1, 1)),
            Decoded::Event(_)
        ));
        // Another subscription counts on its own, from whatever number it starts at.
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(2, 7)),
            Decoded::Event(_)
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_frame(2, 8, "yes")),
            Decoded::Event(_)
        ));
        match decoded(&mut protocol, &delta_frame(1, 3, "yes")) {
            Decoded::Error(PredictionsWsError::SequenceGap {
                sid: 1,
                expected: 2,
                received: 3,
            }) => {}
            other => panic!("expected a gap, got {other:?}"),
        }
        // A number seen twice is a gap too: the next after 3 is 4.
        match decoded(&mut protocol, &delta_frame(1, 3, "yes")) {
            Decoded::Error(PredictionsWsError::SequenceGap {
                sid: 1,
                expected: 4,
                received: 3,
            }) => {}
            other => panic!("expected a gap, got {other:?}"),
        }
    }

    #[test]
    fn a_fresh_connection_counts_from_scratch() {
        let mut protocol = protocol();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        decoded(&mut protocol, &delta_frame(1, 2, "yes"));
        protocol.reset();
        assert!(protocol.sid.is_none());
        // The same subscription number comes back on the next connection, from 1.
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(1, 1)),
            Decoded::Event(Event::Snapshot(_))
        ));
    }

    #[test]
    fn the_captured_snapshot_decodes_to_a_sorted_book() {
        let wire: Value = serde_json::from_str(&fixture("snapshot")).unwrap();
        let mut protocol = protocol();
        let Decoded::Event(Event::Snapshot(book)) = decoded(&mut protocol, &fixture("snapshot"))
        else {
            panic!("the fixture is not a snapshot");
        };
        let count = |key: &str| wire["msg"][key].as_array().map_or(0, Vec::len);
        assert_eq!(book.bids.len(), count("yes_dollars_fp"));
        assert_eq!(book.asks.len(), count("no_dollars_fp"));
        assert!(
            book.bids
                .windows(2)
                .all(|pair| pair[0].price > pair[1].price),
            "{:?}",
            book.bids
        );
        assert!(
            book.asks
                .windows(2)
                .all(|pair| pair[0].price < pair[1].price),
            "{:?}",
            book.asks
        );
        assert_eq!(
            book.market_ticker,
            wire["msg"]["market_ticker"].as_str().unwrap()
        );
    }

    #[test]
    fn every_captured_delta_decodes_with_its_side_price_and_size() {
        let deltas: Value = serde_json::from_str(&fixture("deltas")).unwrap();
        let rows = deltas.as_array().unwrap();
        assert!(!rows.is_empty());
        let mut protocol = protocol();
        for wire in rows {
            let Decoded::Event(Event::Delta(delta)) = decoded(&mut protocol, &wire.to_string())
            else {
                panic!("not a delta: {wire}");
            };
            let msg = &wire["msg"];
            let side = if msg["side"] == "yes" {
                BookSide::Bid
            } else {
                BookSide::Ask
            };
            assert_eq!(delta.side, side, "{wire}");
            let price = msg["price_dollars"].as_str().unwrap().parse::<Decimal>();
            assert_eq!(delta.price, price.unwrap(), "{wire}");
            let size = msg["delta_fp"].as_str().unwrap().parse::<Decimal>();
            assert_eq!(delta.delta, size.unwrap(), "{wire}");
        }
    }

    #[test]
    fn the_captured_answers_are_nothing_to_the_stream_and_carry_what_it_reads() {
        for name in ["subscribed", "ok", "unsubscribed", "error"] {
            let wire: Value = serde_json::from_str(&fixture(name)).unwrap();
            // The correlation matches an answer by its id.
            assert!(wire["id"].is_u64(), "{name}: {wire}");
            let mut protocol = protocol();
            assert!(
                matches!(decoded(&mut protocol, &fixture(name)), Decoded::Nothing),
                "{name}"
            );
        }
        // An ok and an unsubscribed count in their subscription's sequence.
        for name in ["ok", "unsubscribed"] {
            let wire: Value = serde_json::from_str(&fixture(name)).unwrap();
            assert!(
                wire["sid"].is_u64() && wire["seq"].is_u64(),
                "{name}: {wire}"
            );
        }
    }

    #[test]
    fn the_handshake_is_signed_over_the_urls_path() {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let public = hex(vectors["ed25519"]["public_key_hex"].as_str().unwrap());
        for (url, path) in [
            (
                "wss://external-api-ws.kalshi.com/trade-api/ws/v2",
                "/trade-api/ws/v2",
            ),
            ("ws://127.0.0.1:9/", "/"),
            ("ws://127.0.0.1:9", "/"),
        ] {
            let request = signed_request(&credentials(), url).unwrap();
            let header = |name: &str| -> String {
                request
                    .headers()
                    .get(name)
                    .map(|value| value.to_str().unwrap().to_owned())
                    .unwrap_or_else(|| panic!("no {name} header on {url}"))
            };
            assert_eq!(header(ACCESS_KEY), "test-key-id");
            let timestamp = header(ACCESS_TIMESTAMP);
            assert!(timestamp.chars().all(|c| c.is_ascii_digit()), "{timestamp}");
            let message = format!("{timestamp}GET{path}");
            let signature = STANDARD.decode(header(ACCESS_SIGNATURE)).unwrap();
            UnparsedPublicKey::new(&ED25519, &public)
                .verify(message.as_bytes(), &signature)
                .unwrap_or_else(|_| panic!("the signature over {message} does not verify"));
        }
    }

    #[test]
    fn a_key_id_that_cannot_be_a_header_is_a_signing_error_and_not_a_transport_one() {
        // Bug: the kit's `HttpFormat`, `Connection(..)`, which reads as a transport error
        // although nothing was sent. REST's `unfit` says it as the signing error it is.
        let credentials = credentials_with_id("bad\nid");
        let err = signed_request(&credentials, "ws://127.0.0.1:9/trade-api/ws/v2").unwrap_err();
        match &err {
            PredictionsWsError::Credentials(CredentialsError::Sign { reason }) => {
                assert_eq!(
                    reason,
                    &format!("the {ACCESS_KEY} value is not a valid header value")
                );
            }
            other => panic!("expected the signing error, got {other:?}"),
        }
        assert_eq!(err.class(), polyoxide_venue::Class::InvalidRequest);
        assert!(!err.is_retriable());
        assert_eq!(protocol_for(credentials).recover(&err), Recovery::Stop);
    }

    #[test]
    fn a_url_that_is_not_a_request_is_refused_before_signing() {
        let err = signed_request(&credentials(), "not a url").unwrap_err();
        assert_eq!(
            err.class(),
            polyoxide_venue::Class::InvalidRequest,
            "{err:?}"
        );
    }
}
