//! The supervised user socket (Story 7.2): the user channel on the kit's
//! [`Supervisor`], beside the market socket.
//!
//! [`UserWsBuilder`] hands a [`ClobUserProtocol`] to the supervisor, which
//! reconnects after a close or a stall, replays the credentials and the market
//! filter on every connection, and marks each outage with one
//! [`UserEvent::Disconnected`] and one [`UserEvent::Reconnected`]. The queue is
//! unbounded, so the kit never waits on the consumer, and a consumer that has
//! stopped reading loses no frame; its cost is memory that grows with the
//! backlog.
//!
//! The membership is `Option<Vec<String>>`. `None` follows every market, and
//! the subscribe frame omits `markets`. `Some(markets)` follows those markets,
//! and `Some(vec![])` sends `"markets": []`, which follows none: the two are
//! different requests, and nothing ever turns a `Some` into `None`, so a
//! reconnect after the last market left does not widen the subscription to
//! every market. A subscribe on `None` changes nothing, an unsubscribe on
//! `None` is refused, and on a `Some` each change sends one update frame for
//! the markets it actually changed, at most 500 per connection.
//!
//! The URL must be `wss://`, or `ws://` to a loopback host, so credentials
//! never travel unencrypted to a remote host.

use std::{collections::HashSet, future::Future, time::Duration};

use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, handshake::client::Request, Message},
    Connection, Decoded, MembershipHandle, Offline, Outage, Protocol, Queue, Recovery, Socket,
    Supervisor, Timing, WsError,
};

use super::{
    checked_url, data_text, default_timing, disconnect_reason, recover as recovery,
    DisconnectReason,
};
use crate::clob::ws::{
    client::{dial, validate_subscription_count},
    ApiCredentials, ClobWsError, UserMessage, UserSubscription, UserSubscriptionUpdate,
    WS_USER_URL,
};

/// A supervised user connection: a `Stream` of [`UserEvent`]s, and of the
/// errors it skips past or stops on, that survives drops and stalls.
pub type SupervisedUserWs = polyoxide_ws::Supervised<ClobUserProtocol>;

/// What a supervised user connection yields.
#[derive(Debug)]
#[non_exhaustive]
// The message is the event: a consumer matches `UserEvent::Message(UserMessage::Trade(..))`,
// and the two markers are rare, so boxing the message would only add a hop.
#[allow(clippy::large_enum_variant)]
pub enum UserEvent {
    /// An order or trade update.
    Message(UserMessage),
    /// The connection went down, and the reason. At most one per outage.
    Disconnected {
        /// Why the connection went down.
        reason: DisconnectReason,
    },
    /// A fresh connection has replayed the credentials and the filter. Exactly
    /// one per outage; anything the socket carried during the outage is lost,
    /// so a consumer reconciles here.
    Reconnected,
}

/// One change to a supervised connection's market filter, made through
/// [`UserMembership`] or [`MembershipHandle::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserChange {
    /// Follow these markets.
    Subscribe(Vec<String>),
    /// Stop following these markets.
    Unsubscribe(Vec<String>),
}

/// The user channel's protocol, which [`UserWsBuilder`] hands to the kit's
/// [`Supervisor`].
///
/// - **Membership:** `Option<Vec<String>>`, as the module docs describe.
/// - **Credentials:** held here and sent in the subscribe frame of every
///   connection, built afresh each time.
/// - **Ping:** a text `PING` on the wall clock, answered by a text `PONG`.
/// - **Liveness:** every frame counts, a `PONG` included. A connection that
///   decoded an event resets the backoff when it ends.
/// - **Queue:** unbounded.
/// - **Recovery:** a frame that does not decode is skipped; any other error
///   reconnects exactly when its class is retriable, except a close with code
///   1008, the server's answer to credentials it refuses (it sends no error
///   frame): that is [`ClobWsError::Authentication`], carrying the close
///   reason, and the stream yields it and ends, since retrying cannot fix a
///   bad credential and would reconnect for ever at the backoff ceiling.
#[derive(Debug)]
pub struct ClobUserProtocol {
    url: String,
    credentials: ApiCredentials,
}

impl ClobUserProtocol {
    pub(crate) fn new(url: String, credentials: ApiCredentials) -> Self {
        Self { url, credentials }
    }

    /// The subscribe frame for `markets`, built afresh from the credentials:
    /// `None` omits the filter, and an empty slice sends an empty one.
    fn subscription(&self, markets: Option<&[String]>) -> UserSubscription {
        match markets {
            Some(markets) => UserSubscription::new(markets.to_vec(), self.credentials.clone()),
            None => UserSubscription::all_markets(self.credentials.clone()),
        }
    }
}

/// `markets` without its repeats, in first-seen order.
fn distinct(markets: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    markets
        .into_iter()
        .filter(|market| seen.insert(market.clone()))
        .collect()
}

/// Apply `change` to the membership, and return the update frame it needs
/// sent, if any.
///
/// A subscribe on `None` and a change that adds or removes nothing send no
/// frame. An unsubscribe on `None` is refused, and so is a subscribe that
/// would pass 500 markets; a refusal changes nothing.
fn amend(
    membership: &mut Option<Vec<String>>,
    change: UserChange,
) -> Result<Option<UserSubscriptionUpdate>, ClobWsError> {
    match (membership.as_mut(), change) {
        // Every market is already followed.
        (None, UserChange::Subscribe(_)) => Ok(None),
        (None, UserChange::Unsubscribe(_)) => Err(ClobWsError::InvalidMessage(
            "the user channel follows every market; unsubscribing needs a market filter".into(),
        )),
        (Some(held), UserChange::Subscribe(markets)) => {
            let added: Vec<String> = distinct(markets)
                .into_iter()
                .filter(|market| !held.contains(market))
                .collect();
            if added.is_empty() {
                return Ok(None);
            }
            validate_subscription_count(held.len() + added.len())?;
            held.extend(added.iter().cloned());
            Ok(Some(UserSubscriptionUpdate::subscribe(added)))
        }
        (Some(held), UserChange::Unsubscribe(markets)) => {
            let removed: Vec<String> = distinct(markets)
                .into_iter()
                .filter(|market| held.contains(market))
                .collect();
            if removed.is_empty() {
                return Ok(None);
            }
            // The filter stays `Some`, empty when the last market leaves.
            held.retain(|market| !removed.contains(market));
            Ok(Some(UserSubscriptionUpdate::unsubscribe(removed)))
        }
    }
}

/// One text frame, or the array of them the host may send, as events.
fn decode_text(text: &str) -> Decoded<UserEvent, ClobWsError> {
    if !text.trim_start().starts_with('[') {
        return match UserMessage::from_json(text) {
            Ok(message) => Decoded::Event(UserEvent::Message(message)),
            Err(err) => Decoded::Error(ClobWsError::Json(err)),
        };
    }
    // An array yields one event per element. One element that does not decode
    // skips the whole frame, with the error.
    let elements: Vec<serde_json::Value> = match serde_json::from_str(text) {
        Ok(elements) => elements,
        Err(err) => return Decoded::Error(ClobWsError::Json(err)),
    };
    let mut events = Vec::with_capacity(elements.len());
    for element in elements {
        match UserMessage::from_json(&element.to_string()) {
            Ok(message) => events.push(UserEvent::Message(message)),
            Err(err) => return Decoded::Error(ClobWsError::Json(err)),
        }
    }
    if events.is_empty() {
        Decoded::Nothing
    } else {
        Decoded::Events(events)
    }
}

impl Protocol for ClobUserProtocol {
    type Event = UserEvent;
    type Error = ClobWsError;
    type Membership = Option<Vec<String>>;
    type Change = UserChange;
    type Ack = ();

    fn handshake(&mut self) -> Result<Request, ClobWsError> {
        self.url
            .as_str()
            .into_client_request()
            .map_err(|err| ClobWsError::Connection(WsError::from(err)))
    }

    /// Dial through clob's per-address connect (D20), so every address of a
    /// blackholed route is tried within the connect timeout per step.
    fn connect(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<Socket, ClobWsError>> + Send {
        let url = self.url.clone();
        async move { dial(&url, timeout).await }
    }

    async fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &Option<Vec<String>>,
    ) -> Result<(), ClobWsError> {
        let frame = serde_json::to_string(&self.subscription(membership.as_deref()))
            .map_err(ClobWsError::Json)?;
        connection.send(Message::Text(frame.into())).await?;
        Ok(())
    }

    /// The membership is changed before the frame is sent, so a change
    /// whose connection dies under it is answered `Ok`, and the replay on the
    /// next connection carries it.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut Option<Vec<String>>,
        change: UserChange,
    ) -> Result<(), ClobWsError> {
        let Some(update) = amend(membership, change)? else {
            return Ok(());
        };
        let frame = serde_json::to_string(&update).map_err(ClobWsError::Json)?;
        if let Err(err) = connection.send(Message::Text(frame.into())).await {
            tracing::debug!(%err, "the connection failed under a membership change; the replay carries it");
        }
        Ok(())
    }

    /// A change during an outage is recorded and answered at once; the replay
    /// carries it.
    fn record(
        &mut self,
        membership: &mut Option<Vec<String>>,
        change: UserChange,
    ) -> Offline<UserChange, (), ClobWsError> {
        Offline::Recorded(amend(membership, change).map(|_| ()))
    }

    fn decode(&mut self, frame: &Message) -> Decoded<UserEvent, ClobWsError> {
        // What the server closes a refused login with (policy violation).
        const AUTHENTICATION_CLOSE: u16 = 1008;
        if let Message::Close(Some(close)) = frame {
            if u16::from(close.code) == AUTHENTICATION_CLOSE {
                return Decoded::Error(ClobWsError::Authentication(close.reason.to_string()));
            }
        }
        match data_text(frame) {
            Some(text) => decode_text(text),
            None => Decoded::Nothing,
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<UserEvent, ClobWsError>) -> bool {
        true
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<UserEvent, ClobWsError>) -> bool {
        matches!(decoded, Decoded::Event(_) | Decoded::Events(_))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Text("PING".into()))
    }

    fn is_retriable(err: &ClobWsError) -> bool {
        Classify::is_retriable(err)
    }

    fn recover(&self, err: &ClobWsError) -> Recovery {
        recovery(err)
    }

    fn retry_after(&self, err: &ClobWsError) -> Option<Duration> {
        Classify::retry_after(err)
    }

    fn disconnected(&mut self, outage: Outage<ClobWsError>) -> Option<UserEvent> {
        Some(UserEvent::Disconnected {
            reason: disconnect_reason(outage),
        })
    }

    fn reconnected(&mut self) -> UserEvent {
        UserEvent::Reconnected
    }

    fn queue(&self) -> Queue {
        Queue::Unbounded
    }
}

/// Changes the market filter of a running [`SupervisedUserWs`].
///
/// A call waits for the task to take the change and for its answer, so make
/// changes from a task other than the one reading the stream.
pub trait UserMembership {
    /// Follow these markets. On a connection that follows every market this
    /// changes nothing and sends nothing.
    fn subscribe(
        &self,
        markets: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send;

    /// Stop following these markets. Refused on a connection that follows
    /// every market, since it has no filter to remove them from.
    fn unsubscribe(
        &self,
        markets: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send;
}

impl UserMembership for MembershipHandle<ClobUserProtocol> {
    fn subscribe(
        &self,
        markets: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send {
        self.apply(UserChange::Subscribe(markets))
    }

    fn unsubscribe(
        &self,
        markets: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send {
        self.apply(UserChange::Unsubscribe(markets))
    }
}

/// Builder for a supervised user connection. It holds no credential: they are
/// handed to [`connect`](Self::connect) or [`start`](Self::start).
#[derive(Debug, Clone)]
pub struct UserWsBuilder {
    url: String,
    timing: Timing,
}

impl Default for UserWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UserWsBuilder {
    /// Defaults: the production host, a text `PING` every 10 s, stale after 30 s
    /// without a frame, reconnect backoff from 500 ms doubling to 60 s, and a
    /// 10 s connect timeout per address and per step.
    pub fn new() -> Self {
        Self {
            url: WS_USER_URL.to_owned(),
            timing: default_timing(),
        }
    }

    /// Connect to a different endpoint: a `wss://` URL, or a `ws://` URL to a
    /// loopback host. Anything else is refused, so credentials never travel
    /// unencrypted to a remote host.
    pub fn url(mut self, url: impl Into<String>) -> Result<Self, ClobWsError> {
        self.url = checked_url(url.into())?;
        Ok(self)
    }

    polyoxide_ws::timing_setters! {
        /// How often to send the text `PING`, whatever the traffic. It goes out
        /// while the consumer's queue is full too, and none goes out while a
        /// membership change is in flight, which `stale_after` bounds.
        ping_interval;
        /// Silence after which the connection is presumed dead and replaced.
        /// Any frame counts, a `PONG` included. Also bounds each membership
        /// change and each replay.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max`. There is no
        /// cap on attempts; only the delay is capped, at `max`. A reconnect
        /// refused with a `Retry-After` waits at least that long, up to `max`.
        backoff;
        /// How long each address, and each step on it (the TCP connect, then the
        /// TLS handshake and the WebSocket upgrade), may take.
        connect_timeout;
    }

    /// Open the connection, subscribe with `markets` (`None` for every market)
    /// and the credentials, and start supervising.
    ///
    /// Fails with `InvalidMessage` for more than 500 markets, before
    /// connecting, and returns the first failure of the first attempt.
    pub async fn connect(
        self,
        markets: Option<Vec<String>>,
        credentials: ApiCredentials,
    ) -> Result<SupervisedUserWs, ClobWsError> {
        let membership = Self::membership(markets)?;
        Supervisor::new(self.protocol(credentials), self.timing)
            .connect(membership)
            .await
    }

    /// As [`connect`](Self::connect), except that a first attempt that fails in
    /// a way recovery would retry starts the stream in an outage: a
    /// [`UserEvent::Disconnected`], the backoff, and the attempts that follow.
    /// Only an error recovery stops on is returned.
    pub async fn start(
        self,
        markets: Option<Vec<String>>,
        credentials: ApiCredentials,
    ) -> Result<SupervisedUserWs, ClobWsError> {
        let membership = Self::membership(markets)?;
        Supervisor::new(self.protocol(credentials), self.timing)
            .start(membership)
            .await
    }

    /// The initial membership: without repeats, then refused past 500 markets.
    /// It is built through the change function a `subscribe` runs, so the two
    /// count the same way; `None` has nothing to count.
    fn membership(markets: Option<Vec<String>>) -> Result<Option<Vec<String>>, ClobWsError> {
        let Some(markets) = markets else {
            return Ok(None);
        };
        let mut membership = Some(Vec::new());
        amend(&mut membership, UserChange::Subscribe(markets))?;
        Ok(membership)
    }

    fn protocol(&self, credentials: ApiCredentials) -> ClobUserProtocol {
        ClobUserProtocol::new(self.url.clone(), credentials)
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use polyoxide_venue::Class;
    use polyoxide_ws::{
        test_server::{next_event, next_item, Script, ScriptedServer, WINDOW},
        tungstenite::protocol::{frame::coding::CloseCode, CloseFrame},
        Mode,
    };
    use serde_json::{json, Value};

    use super::*;
    use crate::clob::ws::{scripted, TradeStatus};

    fn credentials() -> ApiCredentials {
        ApiCredentials::new("key-1", "secret-1", "pass-1")
    }

    /// The credentials as the subscribe frame carries them.
    fn auth() -> Value {
        json!({"apiKey": "key-1", "secret": "secret-1", "passphrase": "pass-1"})
    }

    fn protocol() -> ClobUserProtocol {
        ClobUserProtocol::new("wss://example.invalid/ws/user".to_owned(), credentials())
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    fn text(frame: &str) -> Message {
        Message::Text(frame.into())
    }

    fn trade(id: &str) -> String {
        json!({
            "event_type": "trade",
            "id": id,
            "asset_id": "111",
            "market": "0xmarket",
            "outcome": "Yes",
            "price": "0.5",
            "size": "10",
            "side": "BUY",
            "status": "MATCHED",
            "taker_order_id": "0xtaker",
            "maker_orders": [],
            "timestamp": "1700000000000"
        })
        .to_string()
    }

    fn order(id: &str) -> String {
        json!({
            "event_type": "order",
            "id": id,
            "asset_id": "111",
            "market": "0xmarket",
            "outcome": "Yes",
            "price": "0.5",
            "side": "BUY",
            "original_size": "10",
            "size_matched": "0",
            "type": "PLACEMENT",
            "timestamp": "1700000000000"
        })
        .to_string()
    }

    /// The messages a decode produced, which must be events.
    fn messages(decoded: Decoded<UserEvent, ClobWsError>) -> Vec<UserMessage> {
        let events = match decoded {
            Decoded::Event(event) => vec![event],
            Decoded::Events(events) => events,
            other => panic!("expected events, got {other:?}"),
        };
        events
            .into_iter()
            .map(|event| match event {
                UserEvent::Message(message) => message,
                other => panic!("expected a message, got {other:?}"),
            })
            .collect()
    }

    // The pure parts: the subscribe frame, the membership rules and the decode.

    #[test]
    fn the_subscribe_frame_omits_markets_for_every_market_and_keeps_an_empty_filter() {
        let protocol = protocol();
        let all = serde_json::to_value(protocol.subscription(None)).unwrap();
        assert_eq!(all, json!({"type": "user", "auth": auth()}));
        let none = Vec::<String>::new();
        let nothing = serde_json::to_value(protocol.subscription(Some(none.as_slice()))).unwrap();
        assert_eq!(
            nothing,
            json!({"type": "user", "markets": [], "auth": auth()})
        );
        let some = ids(&["a", "b"]);
        let some = serde_json::to_value(protocol.subscription(Some(some.as_slice()))).unwrap();
        assert_eq!(some["markets"], json!(["a", "b"]));
    }

    #[test]
    fn subscribing_with_no_filter_changes_nothing_and_sends_nothing() {
        let mut membership = None;
        let update = amend(&mut membership, UserChange::Subscribe(ids(&["a"]))).expect("Ok");
        assert!(update.is_none());
        assert_eq!(membership, None);
    }

    #[test]
    fn unsubscribing_with_no_filter_is_refused_and_changes_nothing() {
        let mut membership = None;
        let err =
            amend(&mut membership, UserChange::Unsubscribe(ids(&["a"]))).expect_err("a refusal");
        assert!(
            matches!(
                &err,
                ClobWsError::InvalidMessage(reason)
                    if reason == "the user channel follows every market; unsubscribing needs a market filter"
            ),
            "{err:?}"
        );
        assert_eq!(membership, None);
    }

    #[test]
    fn a_subscribe_adds_only_the_markets_not_held_in_first_seen_order() {
        let mut membership = Some(ids(&["a"]));
        let update = amend(
            &mut membership,
            UserChange::Subscribe(ids(&["b", "a", "b", "c"])),
        )
        .expect("Ok")
        .expect("an update");
        assert_eq!(update.markets, ids(&["b", "c"]));
        assert_eq!(membership, Some(ids(&["a", "b", "c"])));
        let again = amend(&mut membership, UserChange::Subscribe(ids(&["a", "c"]))).expect("Ok");
        assert!(again.is_none(), "markets already held send no frame");
        assert_eq!(membership, Some(ids(&["a", "b", "c"])));
    }

    #[test]
    fn an_unsubscribe_removes_only_the_markets_held() {
        let mut membership = Some(ids(&["a", "b", "c"]));
        let update = amend(
            &mut membership,
            UserChange::Unsubscribe(ids(&["c", "zz", "c"])),
        )
        .expect("Ok")
        .expect("an update");
        assert_eq!(update.markets, ids(&["c"]));
        assert_eq!(membership, Some(ids(&["a", "b"])));
        let none = amend(&mut membership, UserChange::Unsubscribe(ids(&["zz"]))).expect("Ok");
        assert!(none.is_none(), "a market not held sends no frame");
    }

    #[test]
    fn removing_the_last_market_leaves_an_empty_filter_never_every_market() {
        let mut membership = Some(ids(&["a"]));
        amend(&mut membership, UserChange::Unsubscribe(ids(&["a"])))
            .expect("Ok")
            .expect("an update");
        assert_eq!(membership, Some(Vec::new()));
        // Were it `None`, a subscribe would be a no-op that follows everything.
        let update = amend(&mut membership, UserChange::Subscribe(ids(&["x"])))
            .expect("Ok")
            .expect("an update");
        assert_eq!(update.markets, ids(&["x"]));
        assert_eq!(membership, Some(ids(&["x"])));
    }

    #[test]
    fn a_501st_market_is_refused_and_changes_nothing() {
        let held: Vec<String> = (0..500).map(|n| format!("0x{n}")).collect();
        let mut membership = Some(held.clone());
        let err = amend(&mut membership, UserChange::Subscribe(ids(&["0xnew"])))
            .expect_err("a 501st market");
        assert!(
            matches!(&err, ClobWsError::InvalidMessage(reason) if reason.contains("Too many subscriptions (501)")),
            "{err:?}"
        );
        assert_eq!(membership, Some(held.clone()));
        // At the cap, a market already held is no change, and a removal makes room.
        let held_again = amend(&mut membership, UserChange::Subscribe(ids(&["0x7"]))).expect("Ok");
        assert!(held_again.is_none());
        amend(&mut membership, UserChange::Unsubscribe(ids(&["0x0"])))
            .expect("Ok")
            .expect("an update");
        amend(&mut membership, UserChange::Subscribe(ids(&["0xnew"])))
            .expect("room for one")
            .expect("an update");
    }

    #[test]
    fn the_update_frames_have_the_documented_wire_shape() {
        let mut membership = Some(ids(&["a"]));
        let added = amend(&mut membership, UserChange::Subscribe(ids(&["b", "c"])))
            .expect("Ok")
            .expect("an update");
        assert_eq!(
            serde_json::to_string(&added).unwrap(),
            r#"{"operation":"subscribe","markets":["b","c"]}"#
        );
        let removed = amend(&mut membership, UserChange::Unsubscribe(ids(&["b"])))
            .expect("Ok")
            .expect("an update");
        assert_eq!(
            serde_json::to_string(&removed).unwrap(),
            r#"{"operation":"unsubscribe","markets":["b"]}"#
        );
    }

    #[test]
    fn a_trade_frame_decodes_to_a_message_event() {
        let messages = messages(protocol().decode(&text(&trade("t-1"))));
        match messages.as_slice() {
            [UserMessage::Trade(trade)] => {
                assert_eq!(trade.id, "t-1");
                assert_eq!(trade.status, TradeStatus::Matched);
                assert_eq!(trade.price, "0.5");
            }
            other => panic!("expected one trade, got {other:?}"),
        }
    }

    #[test]
    fn an_array_frame_yields_every_element_in_order() {
        let frame = format!("[{},{},{}]", trade("t-1"), order("o-1"), trade("t-2"));
        let messages = messages(protocol().decode(&text(&frame)));
        match messages.as_slice() {
            [UserMessage::Trade(first), UserMessage::Order(second), UserMessage::Trade(third)] => {
                assert_eq!(first.id, "t-1");
                assert_eq!(second.id, "o-1");
                assert_eq!(third.id, "t-2");
            }
            other => panic!("expected trade, order, trade, got {other:?}"),
        }
    }

    #[test]
    fn an_array_with_an_element_that_does_not_decode_is_skipped_whole() {
        let frame = format!(r#"[{}, {{"event_type":"trade"}}]"#, trade("t-1"));
        match protocol().decode(&text(&frame)) {
            Decoded::Error(ClobWsError::Json(_)) => {}
            other => panic!("expected a JSON error, got {other:?}"),
        }
    }

    #[test]
    fn frames_that_carry_no_event_decode_to_nothing() {
        let mut protocol = protocol();
        let frames = [
            text("PONG"),
            text("{}"),
            text(""),
            text(r#"{"hello":"world"}"#),
            Message::Ping(vec![1, 2].into()),
            Message::Pong(Vec::new().into()),
        ];
        for frame in frames {
            assert!(
                matches!(protocol.decode(&frame), Decoded::Nothing),
                "{frame:?}"
            );
        }
    }

    #[test]
    fn a_binary_frame_decodes_as_text() {
        let frame = Message::Binary(trade("t-9").into_bytes().into());
        let messages = messages(protocol().decode(&frame));
        assert!(matches!(messages.as_slice(), [UserMessage::Trade(t)] if t.id == "t-9"));
    }

    #[test]
    fn a_frame_that_does_not_parse_is_skipped_not_fatal() {
        let mut protocol = protocol();
        for frame in [r#"{"event_type":"trade""#, r#"{"event_type":"mystery"}"#] {
            match protocol.decode(&text(frame)) {
                Decoded::Error(err) => {
                    assert!(matches!(err, ClobWsError::Json(_)), "{err:?}");
                    assert_eq!(protocol.recover(&err), Recovery::Skip);
                }
                other => panic!("expected an error for {frame}, got {other:?}"),
            }
        }
    }

    #[test]
    fn every_frame_is_live_and_only_a_decoded_event_is_delivered() {
        let mut protocol = protocol();
        let pong = text("PONG");
        let nothing = protocol.decode(&pong);
        assert!(protocol.is_live(&pong, &nothing));
        assert!(!protocol.delivered(&pong, &nothing));
        let frame = text(&trade("t-1"));
        let event = protocol.decode(&frame);
        assert!(protocol.is_live(&frame, &event));
        assert!(protocol.delivered(&frame, &event));
    }

    #[test]
    fn the_queue_is_unbounded() {
        assert_eq!(protocol().queue(), Queue::Unbounded);
    }

    #[test]
    fn the_builder_takes_wss_or_a_loopback_ws_url() {
        let accepted = [
            "wss://ws.example.com/ws/user",
            "ws://127.0.0.1:9000",
            "ws://localhost:9000/ws/user",
            "ws://[::1]:9000",
        ];
        for url in accepted {
            assert!(UserWsBuilder::new().url(url).is_ok(), "{url}");
        }
        let refused = [
            "ws://ws.example.com/ws/user",
            "ws://127.0.0.1.example.com:9000",
            "http://127.0.0.1:9000",
            "https://ws.example.com",
        ];
        for url in refused {
            let err = UserWsBuilder::new().url(url).expect_err(url);
            assert!(
                matches!(&err, ClobWsError::InvalidMessage(reason) if reason.contains(url)),
                "{err:?}"
            );
        }
    }

    // The socket: the scripted server plays the host.

    /// The scripted server, answering a text `PING` with a text `PONG`.
    async fn serve(scripts: Vec<Script>) -> ScriptedServer {
        ScriptedServer::start_answering(scripts, scripted::pong).await
    }

    /// A builder for `server`, on test-sized timing with the `PING` schedule on.
    fn builder(server: &ScriptedServer) -> UserWsBuilder {
        UserWsBuilder::new()
            .url(server.url.clone())
            .expect("a loopback URL")
            .ping_interval(Duration::from_millis(50))
            .stale_after(Duration::from_millis(300))
            .backoff(Duration::from_millis(10), Duration::from_millis(20))
    }

    /// The `n`-th connection's subscribe frame, as the server recorded it.
    fn frame(server: &ScriptedServer, n: usize) -> Value {
        serde_json::from_str(&server.first_frames()[n]).expect("a JSON frame")
    }

    /// The stream yields nothing for a moment.
    async fn quiet(ws: &mut SupervisedUserWs) {
        let next = tokio::time::timeout(Duration::from_millis(200), ws.next()).await;
        assert!(next.is_err(), "expected quiet, got {next:?}");
    }

    #[tokio::test]
    async fn a_subscription_with_no_filter_omits_markets_and_carries_the_credentials() {
        let server = serve(vec![Script::hold()]).await;
        let ws = builder(&server)
            .connect(None, credentials())
            .await
            .expect("connects");
        server
            .wait_for("the subscribe frame", |s| s.first_frames().len() == 1)
            .await;
        assert_eq!(frame(&server, 0), json!({"type": "user", "auth": auth()}));
        drop(ws);
    }

    #[tokio::test]
    async fn an_empty_filter_subscribes_to_none_and_stays_connected() {
        let server = serve(vec![Script::hold()]).await;
        let mut ws = builder(&server)
            .connect(Some(Vec::new()), credentials())
            .await
            .expect("connects");
        server
            .wait_for("the subscribe frame", |s| s.first_frames().len() == 1)
            .await;
        assert_eq!(
            frame(&server, 0),
            json!({"type": "user", "markets": [], "auth": auth()})
        );
        server
            .wait_for("two PINGs", |s| s.count_frames(|f| f == "PING") >= 2)
            .await;
        quiet(&mut ws).await;
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn a_filter_is_sent_without_repeats() {
        let server = serve(vec![Script::hold()]).await;
        let ws = builder(&server)
            .connect(Some(ids(&["a", "b", "a"])), credentials())
            .await
            .expect("connects");
        server
            .wait_for("the subscribe frame", |s| s.first_frames().len() == 1)
            .await;
        assert_eq!(frame(&server, 0)["markets"], json!(["a", "b"]));
        drop(ws);
    }

    #[tokio::test]
    async fn exactly_500_markets_are_accepted() {
        let server = serve(vec![Script::hold()]).await;
        let markets: Vec<String> = (0..500).map(|n| format!("0x{n}")).collect();
        let ws = builder(&server)
            .connect(Some(markets), credentials())
            .await
            .expect("500 markets connect");
        server
            .wait_for("the subscribe frame", |s| s.first_frames().len() == 1)
            .await;
        assert_eq!(frame(&server, 0)["markets"].as_array().unwrap().len(), 500);
        drop(ws);
    }

    #[tokio::test]
    async fn more_than_500_markets_are_refused_before_connecting() {
        let server = serve(vec![Script::hold()]).await;
        let markets: Vec<String> = (0..501).map(|n| format!("0x{n}")).collect();
        let err = builder(&server)
            .connect(Some(markets.clone()), credentials())
            .await
            .expect_err("501 markets");
        assert!(
            matches!(&err, ClobWsError::InvalidMessage(reason) if reason.contains("Too many subscriptions")),
            "{err:?}"
        );
        let err = builder(&server)
            .start(Some(markets), credentials())
            .await
            .expect_err("501 markets");
        assert!(
            matches!(&err, ClobWsError::InvalidMessage(reason) if reason.contains("Too many subscriptions")),
            "{err:?}"
        );
        assert_eq!(server.connection_count(), 0, "nothing was dialled");
    }

    #[tokio::test]
    async fn repeats_are_removed_before_the_500_count() {
        let server = serve(vec![Script::hold()]).await;
        let copies = vec!["0xsame".to_owned(); 501];
        let ws = builder(&server)
            .connect(Some(copies.clone()), credentials())
            .await
            .expect("501 copies of one market are one market");
        server
            .wait_for("the subscribe frame", |s| s.first_frames().len() == 1)
            .await;
        assert_eq!(frame(&server, 0)["markets"], json!(["0xsame"]));
        drop(ws);
        let ws = builder(&server)
            .start(Some(copies), credentials())
            .await
            .expect("start takes them too");
        server
            .wait_for("the second subscribe frame", |s| {
                s.first_frames().len() == 2
            })
            .await;
        assert_eq!(frame(&server, 1)["markets"], json!(["0xsame"]));
        drop(ws);
    }

    #[tokio::test]
    async fn changes_send_one_update_for_what_they_changed() {
        let server = serve(vec![Script::hold()]).await;
        let ws = builder(&server)
            .connect(Some(ids(&["a"])), credentials())
            .await
            .expect("connects");
        let handle = ws.membership();
        handle
            .subscribe(ids(&["b", "a", "b"]))
            .await
            .expect("added");
        handle.subscribe(ids(&["a"])).await.expect("already held");
        handle.unsubscribe(ids(&["zz"])).await.expect("not held");
        handle
            .unsubscribe(ids(&["a", "zz"]))
            .await
            .expect("removed");
        // The server reads in order, so once it has seen the client end, it has
        // recorded every frame the changes sent.
        ws.close().await.expect("closes");
        server
            .wait_for("the client to end", |s| s.client_ended_count() == 1)
            .await;
        let updates: Vec<String> = server
            .client_frames()
            .into_iter()
            .filter(|f| f.contains("operation"))
            .collect();
        assert_eq!(
            updates,
            [
                r#"{"operation":"subscribe","markets":["b"]}"#,
                r#"{"operation":"unsubscribe","markets":["a"]}"#,
            ]
        );
    }

    #[tokio::test]
    async fn changes_on_every_market_send_nothing_and_unsubscribing_is_refused() {
        let server = serve(vec![Script::hold()]).await;
        let ws = builder(&server)
            .connect(None, credentials())
            .await
            .expect("connects");
        let handle = ws.membership();
        handle
            .subscribe(ids(&["a"]))
            .await
            .expect("every market is already followed");
        let err = handle
            .unsubscribe(ids(&["a"]))
            .await
            .expect_err("unsubscribing needs a filter");
        assert!(
            matches!(
                &err,
                ClobWsError::InvalidMessage(reason)
                    if reason == "the user channel follows every market; unsubscribing needs a market filter"
            ),
            "{err:?}"
        );
        ws.close().await.expect("closes");
        server
            .wait_for("the client to end", |s| s.client_ended_count() == 1)
            .await;
        assert_eq!(
            server.count_frames(|f| f.contains("operation")),
            0,
            "{:?}",
            server.client_frames()
        );
    }

    #[tokio::test]
    async fn every_connection_replays_the_credentials_and_the_filter_with_its_changes() {
        let going = |after: u64| {
            Script::hold()
                .drop_after(Duration::from_millis(after))
                .close_with(1001, "going away")
        };
        let server = serve(vec![going(400), going(250), Script::hold()]).await;
        let ws = builder(&server)
            .connect(Some(ids(&["m1", "m2"])), credentials())
            .await
            .expect("connects");
        let handle = ws.membership();
        handle.subscribe(ids(&["m3"])).await.expect("added");
        handle.unsubscribe(ids(&["m1"])).await.expect("removed");
        server
            .wait_for("three subscribe frames", |s| s.first_frames().len() == 3)
            .await;
        assert_eq!(frame(&server, 0)["markets"], json!(["m1", "m2"]));
        assert_eq!(frame(&server, 1)["markets"], json!(["m2", "m3"]));
        assert_eq!(frame(&server, 2)["markets"], json!(["m2", "m3"]));
        for n in 0..3 {
            assert_eq!(frame(&server, n)["auth"], auth(), "connection {n}");
            assert_eq!(frame(&server, n)["type"], "user", "connection {n}");
        }
        drop(ws);
    }

    #[tokio::test]
    async fn the_last_market_removed_is_replayed_as_an_empty_filter() {
        let server = serve(vec![
            Script::hold()
                .drop_after(Duration::from_millis(400))
                .close_with(1001, "going away"),
            Script::hold(),
        ])
        .await;
        let ws = builder(&server)
            .connect(Some(ids(&["m1"])), credentials())
            .await
            .expect("connects");
        ws.membership()
            .unsubscribe(ids(&["m1"]))
            .await
            .expect("removed");
        server
            .wait_for("the second subscribe frame", |s| {
                s.first_frames().len() == 2
            })
            .await;
        assert_eq!(
            frame(&server, 1),
            json!({"type": "user", "markets": [], "auth": auth()}),
            "an empty filter, never every market"
        );
        drop(ws);
    }

    #[tokio::test]
    async fn an_outage_is_one_disconnected_then_one_reconnected() {
        let server = serve(vec![
            Script::hold()
                .drop_after(Duration::from_millis(200))
                .close_with(1001, "going away"),
            Script::hold(),
        ])
        .await;
        let mut ws = builder(&server)
            .connect(None, credentials())
            .await
            .expect("connects");
        match next_event(&mut ws).await {
            UserEvent::Disconnected {
                reason: DisconnectReason::Closed { code, reason },
            } => {
                assert_eq!(code, Some(1001));
                assert_eq!(reason, "going away");
            }
            other => panic!("expected the close, got {other:?}"),
        }
        assert!(matches!(next_event(&mut ws).await, UserEvent::Reconnected));
        quiet(&mut ws).await;
        assert_eq!(server.connection_count(), 2);
    }

    #[tokio::test]
    async fn a_burst_the_consumer_has_not_read_is_taken_off_the_socket_and_arrives_whole() {
        const BURST: usize = 1500;
        let frames: Vec<Message> = (0..BURST)
            .map(|n| text(&trade(&format!("t-{n}"))))
            .collect();
        let server = serve(vec![
            Script::messages(frames)
                .after_first_frame()
                .then_close()
                .close_with(1001, "going away"),
            Script::hold(),
        ])
        .await;
        let mut ws = builder(&server)
            .connect(None, credentials())
            .await
            .expect("connects");
        // The consumer has read nothing. An unbounded queue lets the task read
        // the whole burst and the server's close behind it, and reply to it; a
        // bounded one holds the task back long before the close.
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
        for n in 0..BURST {
            match next_event(&mut ws).await {
                UserEvent::Message(UserMessage::Trade(trade)) => {
                    assert_eq!(trade.id, format!("t-{n}"));
                }
                other => panic!("event {n}: {other:?}"),
            }
        }
        assert!(matches!(
            next_event(&mut ws).await,
            UserEvent::Disconnected { .. }
        ));
        assert!(matches!(next_event(&mut ws).await, UserEvent::Reconnected));
    }

    #[tokio::test]
    async fn a_start_that_fails_begins_in_an_outage_and_answers_changes_at_once() {
        let server = serve(vec![Script::refuse_upgrade(503), Script::hold()]).await;
        let mut ws = builder(&server)
            .backoff(Duration::from_millis(400), Duration::from_millis(400))
            .start(Some(ids(&["a"])), credentials())
            .await
            .expect("a retriable first failure starts in an outage");
        let answered = tokio::time::timeout(
            Duration::from_millis(200),
            ws.membership().subscribe(ids(&["b"])),
        )
        .await;
        assert!(
            matches!(answered, Ok(Ok(()))),
            "a change in an outage is answered before the next attempt: {answered:?}"
        );
        assert!(matches!(
            next_event(&mut ws).await,
            UserEvent::Disconnected {
                reason: DisconnectReason::Error(_)
            }
        ));
        assert!(matches!(next_event(&mut ws).await, UserEvent::Reconnected));
        server
            .wait_for("the replayed subscription", |s| s.first_frames().len() == 1)
            .await;
        assert_eq!(frame(&server, 0)["markets"], json!(["a", "b"]));
    }

    #[test]
    fn debug_output_redacts_the_credentials() {
        let shown = format!("{:?}", protocol());
        for secret in ["key-1", "secret-1", "pass-1"] {
            assert!(!shown.contains(secret), "{shown}");
        }
    }

    // The server closes a refused login with code 1008, and sends no
    // error frame (observed live, 2026-10-17).

    fn close(code: CloseCode, reason: &str) -> Message {
        Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        }))
    }

    #[test]
    fn a_1008_close_decodes_as_an_authentication_failure() {
        let decoded = protocol().decode(&close(CloseCode::Policy, "authentication failed"));
        match decoded {
            Decoded::Error(ClobWsError::Authentication(reason)) => {
                assert_eq!(reason, "authentication failed");
            }
            other => panic!("expected an authentication failure, got {other:?}"),
        }
    }

    #[test]
    fn an_authentication_failure_is_unauthorized_and_recovery_stops_on_it() {
        let err = ClobWsError::Authentication("authentication failed".into());
        assert_eq!(err.class(), Class::Unauthorized);
        assert!(!<ClobUserProtocol as Protocol>::is_retriable(&err));
        assert_eq!(protocol().recover(&err), Recovery::Stop);
    }

    #[tokio::test]
    async fn a_change_on_a_connection_the_server_closed_is_recorded_and_answered_ok() {
        // The server closes after the first frame. The connection is read to
        // its end, so the send below fails; the change must still be recorded
        // (the replay carries it) and answered `Ok`.
        let server =
            ScriptedServer::start(vec![Script::hold().after_first_frame().then_close()]).await;
        let socket = dial(&server.url, Duration::from_secs(2))
            .await
            .expect("dials");
        let mut connection = Connection::new(socket, Mode::All);
        connection
            .send(Message::Text("first".into()))
            .await
            .expect("the first frame is sent");
        while tokio::time::timeout(WINDOW, connection.next())
            .await
            .expect("the connection ends")
            .is_some()
        {}

        let mut protocol = ClobUserProtocol::new(server.url.clone(), credentials());
        let mut membership = Some(ids(&["a"]));
        let result = protocol
            .apply(
                &mut connection,
                &mut membership,
                UserChange::Subscribe(ids(&["b"])),
            )
            .await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(membership, Some(ids(&["a", "b"])));
    }

    #[test]
    fn any_other_close_decodes_to_nothing() {
        let mut protocol = protocol();
        let closes = [
            close(CloseCode::Away, "going away"),
            close(CloseCode::Error, "internal error"),
            close(CloseCode::Normal, ""),
            Message::Close(None),
        ];
        for frame in closes {
            assert!(
                matches!(protocol.decode(&frame), Decoded::Nothing),
                "{frame:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_refused_login_ends_the_stream_without_a_reconnect() {
        let server = serve(vec![
            Script::hold()
                .after_first_frame()
                .then_close()
                .close_with(1008, "authentication failed"),
            Script::hold(),
        ])
        .await;
        let mut ws = builder(&server)
            .connect(None, credentials())
            .await
            .expect("the first attempt connects");
        // The refusal is the first item: no `Disconnected` precedes it.
        match next_item(&mut ws).await {
            Err(err) => {
                assert!(
                    matches!(&err, ClobWsError::Authentication(reason) if reason == "authentication failed"),
                    "{err:?}"
                );
                assert_eq!(err.class(), Class::Unauthorized);
            }
            Ok(event) => panic!("expected the refusal, got {event:?}"),
        }
        let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("the stream ends");
        assert!(end.is_none(), "{end:?}");
        // A reconnect would follow within the 10 to 20 ms backoff.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(server.connection_count(), 1, "no reconnect");
    }
}
