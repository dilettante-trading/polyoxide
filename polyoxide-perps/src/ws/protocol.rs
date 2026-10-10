//! perps' socket protocol, as the kit's `Supervisor` drives it, and the
//! membership calls on its handle.

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
};

use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    Connection, Decoded, MembershipHandle, Protocol, Queue, Recovery as Supervision, Timing,
    WsError,
};

use crate::ws::{
    channel::Channel,
    client::{control, Answered, Control},
    error::{PerpsWsError, Recovery},
    event::{Event, Frame},
    frame::{Incoming, Request, Response},
};

/// Retries of a subscribe the server refused with `message_rate_limited`.
pub(crate) const MAX_SUBSCRIBE_RETRIES: usize = 3;
/// Events the consumer may leave unread before the task stops reading.
const EVENT_QUEUE: usize = 1024;
/// Pings awaiting their pong on one connection; an older one is forgotten,
/// and its pong, should it come, read as an answer nobody awaits.
const OUTSTANDING_PINGS: usize = 16;

/// One change to a supervised connection's channels, made through
/// [`PerpsMembership`] or [`MembershipHandle::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MembershipChange {
    /// Subscribe to these channels.
    Subscribe(Vec<Channel>),
    /// Unsubscribe from these channels.
    Unsubscribe(Vec<Channel>),
}

/// perps' socket protocol, which [`PerpsWsBuilder`](crate::ws::PerpsWsBuilder)
/// hands to the kit's `Supervisor`.
///
/// Its membership is the channels subscribed, replayed on every fresh
/// connection in one `sub` request. Its divergence rows:
///
/// - **D4, the ping:** a JSON `post/ping` on the wall clock, sent without
///   awaiting its answer.
/// - **D5, liveness:** any text frame (a push, an answer, a frame that does
///   not decode), so a pong counts; never a protocol Ping or Pong.
/// - **D6, a delivered connection:** one that decoded a push.
/// - **D8:** no `Disconnected` marker, only [`Event::Reconnected`].
/// - **D10:** a subscribe refused with `message_rate_limited` is retried up to
///   three times on the same connection, spaced by the backoff schedule, and
///   [`Event::SequenceRegressed`] precedes the update whose `sq` went
///   backwards on its channel, per connection.
/// - **D19:** a connection used after it closed stops the stream.
/// - **A pong that is not `ok`** replaces the connection.
/// - **An empty membership stays wanted:** the connection stays, and a
///   reconnect replays nothing.
///
/// Its queue holds 1024 events.
pub struct PerpsProtocol {
    url: String,
    timing: Timing,
    /// The last `sq` per channel on this connection.
    last_sq: HashMap<Channel, u64>,
    /// The ids of this connection's pings awaiting their pong.
    pings: VecDeque<u64>,
}

impl std::fmt::Debug for PerpsProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerpsProtocol")
            .field("url", &self.url)
            .field("timing", &self.timing)
            .finish_non_exhaustive()
    }
}

impl PerpsProtocol {
    pub(crate) fn new(url: String, timing: Timing) -> Self {
        Self {
            url,
            timing,
            last_sq: HashMap::new(),
            pings: VecDeque::new(),
        }
    }

    /// A response the stream read: a ping's pong, or an answer nobody awaits.
    fn answer(&mut self, response: Response) -> Decoded<Event, PerpsWsError> {
        let Some(id) = response.id else {
            return Decoded::Nothing;
        };
        let Some(at) = self.pings.iter().position(|ping| *ping == id) else {
            return Decoded::Nothing;
        };
        self.pings.remove(at);
        match response.pong() {
            Ok(pong) if pong.is_ok() => Decoded::Nothing,
            // A control channel that answers but faults: a fresh connection
            // is the fix.
            _ => Decoded::Error(PerpsWsError::Response {
                id,
                raw: response.data.to_string(),
            }),
        }
    }

    /// A push frame, preceded by a regression report when its `sq` went
    /// backwards on its channel.
    fn push(&mut self, frame: Frame) -> Decoded<Event, PerpsWsError> {
        let Frame::Update(update) = &frame else {
            return Decoded::Event(Event::from(frame));
        };
        let (channel, got) = (update.channel, update.sq);
        match self.last_sq.insert(channel, got) {
            Some(previous) if got < previous => Decoded::Events(vec![
                Event::SequenceRegressed {
                    channel,
                    previous,
                    got,
                },
                Event::from(frame),
            ]),
            _ => Decoded::Event(Event::from(frame)),
        }
    }
}

/// A control request's answer as one result: [`PerpsWsError::Refused`]
/// naming every channel refused.
fn settled(answered: Answered) -> Result<(), PerpsWsError> {
    if answered.refused.is_empty() {
        Ok(())
    } else {
        Err(PerpsWsError::Refused {
            refused: answered.refused,
        })
    }
}

impl Protocol for PerpsProtocol {
    type Event = Event;
    type Error = PerpsWsError;
    type Membership = Vec<Channel>;
    type Change = MembershipChange;
    type Ack = ();

    fn handshake(&mut self) -> Result<Handshake, PerpsWsError> {
        Ok(self
            .url
            .as_str()
            .into_client_request()
            .map_err(WsError::from)?)
    }

    /// One `sub` for every channel; nothing for none. A channel refused on a
    /// fresh connection stops the stream, as retrying replays it.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        channels: &Vec<Channel>,
    ) -> Result<(), PerpsWsError> {
        if channels.is_empty() {
            return Ok(());
        }
        settled(control(connection, Control::Subscribe, channels).await?)
    }

    /// A subscribe or an unsubscribe on the live connection. The membership
    /// changes only when every channel was accepted: when the server accepts
    /// some channels of a subscribe and refuses others, the accepted ones are
    /// live on this connection but not replayed after a reconnect.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        channels: &mut Vec<Channel>,
        change: MembershipChange,
    ) -> Result<(), PerpsWsError> {
        match change {
            MembershipChange::Subscribe(add) => {
                if add.is_empty() {
                    return Ok(());
                }
                let mut backoff = self.timing.backoff();
                let mut attempt = 0;
                loop {
                    match settled(control(connection, Control::Subscribe, &add).await?) {
                        Err(err)
                            if err.recovery() == Recovery::Retry
                                && attempt < MAX_SUBSCRIBE_RETRIES =>
                        {
                            attempt += 1;
                            let delay = backoff.take();
                            tracing::warn!(%err, ?delay, attempt, "subscribe rate limited, retrying");
                            tokio::time::sleep(delay).await;
                        }
                        result => {
                            result?;
                            break;
                        }
                    }
                }
                for channel in add {
                    if !channels.contains(&channel) {
                        channels.push(channel);
                    }
                }
            }
            MembershipChange::Unsubscribe(remove) => {
                if remove.is_empty() {
                    return Ok(());
                }
                settled(control(connection, Control::Unsubscribe, &remove).await?)?;
                channels.retain(|channel| !remove.contains(channel));
            }
        }
        Ok(())
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Event, PerpsWsError> {
        // A Binary frame is not part of the protocol (D12), and the kit
        // answers the protocol Ping and the Close.
        let Message::Text(text) = frame else {
            return Decoded::Nothing;
        };
        match Incoming::parse(text) {
            Ok(Incoming::Push(push)) => match Frame::from_push(push, text) {
                Ok(frame) => self.push(frame),
                Err(err) => Decoded::Error(err),
            },
            Ok(Incoming::Response(response)) => self.answer(response),
            Err(_) => Decoded::Error(PerpsWsError::Unrecognised {
                raw: text.to_string(),
            }),
        }
    }

    fn is_live(&self, frame: &Message, _decoded: &Decoded<Event, PerpsWsError>) -> bool {
        matches!(frame, Message::Text(_))
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<Event, PerpsWsError>) -> bool {
        matches!(decoded, Decoded::Event(_) | Decoded::Events(_))
    }

    fn ping(&mut self, connection: &mut Connection) -> Option<Message> {
        let id = connection.take_id();
        if self.pings.len() == OUTSTANDING_PINGS {
            self.pings.pop_front();
        }
        self.pings.push_back(id);
        let ping = serde_json::to_string(&Request::ping(id)).expect("request serialises");
        Some(Message::Text(ping.into()))
    }

    fn is_retriable(err: &PerpsWsError) -> bool {
        err.is_retriable()
    }

    fn recover(&self, err: &PerpsWsError) -> Supervision {
        match err.recovery() {
            Recovery::Reconnect => Supervision::Reconnect,
            Recovery::SkipFrame => Supervision::Skip,
            // A rate-limited subscribe is retried inside `apply`; anywhere
            // else it is a refusal like any other.
            Recovery::Retry | Recovery::Fatal => Supervision::Stop,
        }
    }

    fn reconnected(&mut self) -> Event {
        Event::Reconnected
    }

    fn reset(&mut self) {
        self.last_sq.clear();
        self.pings.clear();
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(EVENT_QUEUE)
    }

    fn stalled(silent: std::time::Duration) -> PerpsWsError {
        PerpsWsError::Stalled { elapsed: silent }
    }

    fn stopped() -> PerpsWsError {
        PerpsWsError::Stopped
    }
}

/// The membership calls of a supervised perps connection, on its
/// [`MembershipHandle`].
///
/// Each waits for the task to take the change and for the server's answer.
/// During an outage the change waits for the reconnect, and is applied once
/// the membership has been replayed. The task takes no change while the
/// consumer's queue is full (1024 unread events), so a call made from the
/// task that reads the stream can wait for ever then: make changes from
/// another task, or keep reading while one is pending.
pub trait PerpsMembership {
    /// Subscribe to more channels. A `message_rate_limited` refusal is
    /// retried with backoff before being reported.
    ///
    /// When the server accepts some channels and refuses others in one
    /// request, the accepted ones are live on the current connection but are
    /// not replayed after a reconnect: the request failed as a whole, and
    /// re-issuing it with the accepted channels is the caller's call.
    ///
    /// Fails with [`PerpsWsError::Refused`], [`PerpsWsError::Stalled`] when
    /// the server left it unanswered for the staleness window (the connection
    /// is then replaced), or [`PerpsWsError::Stopped`] once the stream has
    /// stopped.
    fn subscribe(
        &self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> impl Future<Output = Result<(), PerpsWsError>> + Send;

    /// Unsubscribe from channels, failing as [`subscribe`](Self::subscribe)
    /// does, with no retry. Unsubscribing every channel keeps the connection,
    /// and a reconnect then replays nothing.
    fn unsubscribe(
        &self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> impl Future<Output = Result<(), PerpsWsError>> + Send;
}

impl PerpsMembership for MembershipHandle<PerpsProtocol> {
    fn subscribe(
        &self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> impl Future<Output = Result<(), PerpsWsError>> + Send {
        self.apply(MembershipChange::Subscribe(channels.into_iter().collect()))
    }

    fn unsubscribe(
        &self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> impl Future<Output = Result<(), PerpsWsError>> + Send {
        self.apply(MembershipChange::Unsubscribe(
            channels.into_iter().collect(),
        ))
    }
}
