//! perps' socket protocol, as the kit's `Supervisor` drives it, and the
//! membership calls on its handle.

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
};

use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    Connection, Decoded, Kind, MembershipHandle, Protocol, Queue, Recovery, Timing, WsError,
};

use crate::perps::ws::{
    channel::Channel,
    client::{control, Answered, Control},
    error::{PerpsWsError, MESSAGE_RATE_LIMITED},
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

/// perps' socket protocol, which [`PerpsWsBuilder`](crate::perps::ws::PerpsWsBuilder)
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

/// Whether `apply` resends a refused subscribe on the same connection (D10):
/// every channel was refused for `message_rate_limited`. Anywhere else such a
/// refusal is one like any other.
fn retried_within_the_change(err: &PerpsWsError) -> bool {
    matches!(
        err,
        PerpsWsError::Refused { refused }
            if refused.iter().all(|r| r.reason == MESSAGE_RATE_LIMITED)
    )
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
                            if retried_within_the_change(&err)
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

    /// perps' own recovery, not the class's: a server close of any code, a
    /// `Capacity` failure and a pong that is not `ok` reconnect, a frame that
    /// does not decode is skipped, and a use after close stops (D19).
    fn recover(&self, err: &PerpsWsError) -> Recovery {
        match err {
            PerpsWsError::Connection(err) => match err.kind() {
                // D19: a connection used after it closed stops perps.
                Kind::AlreadyClosed => Recovery::Stop,
                // perps' own rule, kept: a frame or header past tungstenite's
                // limits costs one retry cycle if it turns out permanent.
                Kind::Capacity => Recovery::Reconnect,
                // DRIFT R1: a handshake refused with 408, 425, 429 or 5xx is
                // the host being unwell or throttling, by venue's status rule;
                // any other status, a bad URL or TLS name, or an attack
                // detection is our request. Everything else, a lost or timed
                // out connection or a server close among them, is retried.
                _ if retrying_can_fix(err, |status| {
                    class_for_handshake_status(status).is_retriable()
                }) =>
                {
                    Recovery::Reconnect
                }
                _ => Recovery::Stop,
            },
            PerpsWsError::ConnectionClosed | PerpsWsError::Stalled { .. } => Recovery::Reconnect,
            // A rate-limited subscribe is retried inside `apply` (D10); a
            // refusal that reaches recovery comes from a replay, which a
            // reconnect would only repeat.
            PerpsWsError::Refused { .. } => Recovery::Stop,
            PerpsWsError::Frame { .. } | PerpsWsError::Unrecognised { .. } => Recovery::Skip,
            // A control reply that is malformed or not `ok` means this
            // connection's control channel is unreliable; a fresh one is the
            // fix. Stopping would end the stream on one odd pong.
            PerpsWsError::Response { .. } => Recovery::Reconnect,
            PerpsWsError::EmptySubscription | PerpsWsError::Stopped => Recovery::Stop,
        }
    }

    /// A refused upgrade's `Retry-After`, as perps' class reads it, which
    /// the reconnect waits out up to the backoff's ceiling (DRIFT R11).
    fn retry_after(&self, err: &PerpsWsError) -> Option<std::time::Duration> {
        err.retry_after()
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
    ///
    /// When the server drops some channels and refuses others in one
    /// request, the membership keeps them all, as the request failed as a
    /// whole: the dropped ones stay off the current connection, but a
    /// reconnect subscribes them again.
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use polyoxide_ws::tungstenite::{self, http};

    use super::*;
    use crate::perps::{
        types::InstrumentId,
        ws::{error::Refusal, WS_URL},
    };

    /// What the supervisor does about `err`, by perps' recovery hook.
    fn recovery(err: &PerpsWsError) -> Recovery {
        PerpsProtocol::new(WS_URL.to_owned(), Timing::default()).recover(err)
    }

    fn refusal(channel: Channel, reason: &str) -> Refusal {
        Refusal {
            channel,
            reason: reason.to_owned(),
        }
    }

    fn http_error(status: u16) -> PerpsWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> PerpsWsError {
        PerpsWsError::from(WsError::from(err))
    }

    #[test]
    fn a_refused_subscription_lists_every_refusal() {
        let err = PerpsWsError::Refused {
            refused: vec![refusal(Channel::Bbo(InstrumentId(1)), "invalid channel")],
        };
        assert_eq!(recovery(&err), Recovery::Stop);
        assert!(!retried_within_the_change(&err));
        assert!(
            err.to_string().contains("bbo::1 (invalid channel)"),
            "{err}"
        );
    }

    #[test]
    fn a_rate_limited_subscription_is_retriable_not_fatal() {
        let err = PerpsWsError::Refused {
            refused: vec![refusal(
                Channel::Bbo(InstrumentId(1)),
                "message_rate_limited",
            )],
        };
        assert!(retried_within_the_change(&err));
    }

    #[test]
    fn a_mixed_refusal_is_fatal() {
        // Retrying would replay the invalid channel, so the rate limit does
        // not rescue the batch.
        let err = PerpsWsError::Refused {
            refused: vec![
                refusal(Channel::Bbo(InstrumentId(1)), "message_rate_limited"),
                refusal(Channel::Bbo(InstrumentId(2)), "invalid channel"),
            ],
        };
        assert_eq!(recovery(&err), Recovery::Stop);
        assert!(!retried_within_the_change(&err));
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_but_a_refused_request_is_fatal() {
        assert_eq!(recovery(&http_error(503)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(429)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(401)), Recovery::Stop);
    }

    #[test]
    fn a_handshake_refused_with_408_or_425_reconnects_drift_r1() {
        // DRIFT R1: venue's status rule, through `retrying_can_fix`. Both
        // were fatal before, though `Classify` called them retriable.
        for status in [408, 425, 429, 500, 503] {
            assert_eq!(
                recovery(&http_error(status)),
                Recovery::Reconnect,
                "{status}"
            );
            assert!(http_error(status).is_retriable(), "{status}");
        }
        for status in [400, 401, 403, 404, 418, 451] {
            assert_eq!(recovery(&http_error(status)), Recovery::Stop, "{status}");
        }
    }

    #[test]
    fn the_kits_own_failures_reconnect_and_a_use_after_close_stops() {
        // A connect timeout (DRIFT R2), an unanswered request and a server
        // close, of any code, are retried; D19 stops on `AlreadyClosed`.
        for err in [
            WsError::connect_timeout(Duration::from_secs(10)),
            WsError::no_answer(Duration::from_secs(1)),
            WsError::closed(Some(1008), "policy"),
            WsError::closed(None, ""),
        ] {
            assert_eq!(recovery(&PerpsWsError::from(err)), Recovery::Reconnect);
        }
        assert_eq!(
            recovery(&transport(tungstenite::Error::AlreadyClosed)),
            Recovery::Stop
        );
        assert_eq!(
            recovery(&transport(tungstenite::Error::Capacity(
                tungstenite::error::CapacityError::TooManyHeaders
            ))),
            Recovery::Reconnect
        );
    }

    #[test]
    fn a_transport_close_reconnects() {
        let err = transport(tungstenite::Error::ConnectionClosed);
        assert!(matches!(err, PerpsWsError::Connection(_)));
        assert_eq!(recovery(&err), Recovery::Reconnect);
    }

    #[test]
    fn transport_loss_reconnects_and_bad_frames_are_skipped() {
        assert_eq!(
            recovery(&PerpsWsError::ConnectionClosed),
            Recovery::Reconnect
        );
        assert_eq!(
            recovery(&PerpsWsError::Stalled {
                elapsed: Duration::from_secs(9)
            }),
            Recovery::Reconnect
        );
        let bad = PerpsWsError::Frame {
            channel: "bbo::1".to_owned(),
            raw: "{}".to_owned(),
            source: serde_json::from_str::<u8>("x").unwrap_err(),
        };
        assert_eq!(recovery(&bad), Recovery::Skip);
        assert_eq!(recovery(&PerpsWsError::EmptySubscription), Recovery::Stop);
    }

    #[test]
    fn a_faulted_control_reply_reconnects_rather_than_ending_the_stream() {
        let odd = PerpsWsError::Response {
            id: 2,
            raw: r#"{"status":"err","error":"x"}"#.to_owned(),
        };
        assert_eq!(recovery(&odd), Recovery::Reconnect);
    }
}
