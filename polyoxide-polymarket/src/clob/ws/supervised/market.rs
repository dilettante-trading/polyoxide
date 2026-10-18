//! The supervised market socket: the market channel's protocol as the socket
//! kit's `Supervisor` drives it, its builder, and the membership calls on its
//! handle.

use std::{future::Future, time::Duration};

use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    Connection, Decoded, MembershipHandle, Offline, Outage, Protocol, Queue, Recovery, Socket,
    Supervised, Supervisor, Timing, WsError,
};

use super::{
    checked_url, data_text, default_timing, disconnect_reason, is_array_frame, recover,
    DisconnectReason,
};
use crate::clob::ws::{
    client::{dial, validate_subscription_count},
    BookMessage, ClobWsError, MarketMessage, MarketSubscription, MarketSubscriptionOptions,
    MarketSubscriptionUpdate, WS_MARKET_URL,
};

/// Events the consumer may leave unread before the task stops reading.
const EVENT_QUEUE: usize = 1024;

/// A supervised market-channel connection: a `Stream` of [`MarketEvent`]s, and
/// of the errors it skips past or stops on, that survives drops, stalls and
/// server closes by reconnecting and replaying its subscription. Opened by
/// [`MarketWsBuilder::connect`].
pub type SupervisedMarketWs = Supervised<ClobMarketProtocol>;

/// What a supervised market socket yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum MarketEvent {
    /// A market-channel message. A frame that carries several books, as the
    /// initial dump does, yields one message per book.
    Message(MarketMessage),
    /// The connection was lost. Every book and price held for the
    /// subscribed assets is stale until the matching
    /// [`Reconnected`](Self::Reconnected), which always follows while the
    /// socket runs.
    Disconnected {
        /// Why.
        reason: DisconnectReason,
    },
    /// The outage is over: a fresh connection has replayed the whole
    /// subscription, changes included. Sent once per outage, however many
    /// attempts it took. Drop book state and rebuild it from the books that
    /// follow (the replayed subscription asks for the initial dump again
    /// unless its options turned it off).
    Reconnected,
}

/// One change to a supervised market socket's subscription, made through
/// [`MarketMembership`] or the kit's
/// [`MembershipHandle::apply`](polyoxide_ws::MembershipHandle::apply).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarketChange {
    /// Subscribe to these assets.
    Subscribe(Vec<String>),
    /// Unsubscribe from these assets.
    Unsubscribe(Vec<String>),
}

/// The market channel's socket protocol, which [`MarketWsBuilder`] hands to
/// the kit's `Supervisor`.
///
/// Its membership is the asset ids subscribed, in the order they were first
/// subscribed and without duplicates, replayed on every fresh connection in
/// one subscribe frame. Its divergence rows:
///
/// - **D4, the ping:** a text `PING` on the wall clock, the first one
///   interval after the connection opens, sent without awaiting its `PONG`.
/// - **D5, liveness:** every frame, so a `PONG` counts.
/// - **D6, a delivered connection:** one that decoded an event.
/// - **D8:** [`MarketEvent::Disconnected`] and [`MarketEvent::Reconnected`]
///   mark every outage.
/// - **Several books in a frame:** the initial dump and each subscribe answer
///   carry one book per asset, and every one is yielded.
/// - **D12:** a Binary frame is decoded as text.
/// - **Recovery:** a frame that does not decode is skipped, and any other
///   failure reconnects exactly when its class is retriable. A connection used
///   after it closed reconnects (D19).
/// - **Membership changes:** a change is recorded before its frame is sent,
///   so one whose connection dies under it is answered `Ok` and replayed, and
///   a change made during an outage is answered at once.
/// - **An empty membership stays wanted:** the connection stays, and keeps
///   pinging.
///
/// Its queue holds 1024 events. The connection is dialled by the CLOB's own
/// per-address connect (D20).
#[derive(Debug)]
pub struct ClobMarketProtocol {
    url: String,
    options: MarketSubscriptionOptions,
}

impl ClobMarketProtocol {
    fn new(url: String, options: MarketSubscriptionOptions) -> Self {
        Self { url, options }
    }
}

/// Apply `change` to the subscribed ids, and return the frame that tells the
/// venue, or `None` when the change adds or removes nothing.
///
/// A subscribe adds the ids the membership lacks, once each, and is refused
/// before anything changes if the result would pass the per-connection cap.
fn change_membership(
    asset_ids: &mut Vec<String>,
    change: MarketChange,
) -> Result<Option<MarketSubscriptionUpdate>, ClobWsError> {
    match change {
        MarketChange::Subscribe(add) => {
            let mut added: Vec<String> = Vec::new();
            for id in add {
                if !asset_ids.contains(&id) && !added.contains(&id) {
                    added.push(id);
                }
            }
            validate_subscription_count(asset_ids.len() + added.len())?;
            if added.is_empty() {
                return Ok(None);
            }
            asset_ids.extend(added.iter().cloned());
            Ok(Some(MarketSubscriptionUpdate::subscribe(added)))
        }
        MarketChange::Unsubscribe(remove) => {
            let mut removed: Vec<String> = Vec::new();
            for id in remove {
                if asset_ids.contains(&id) && !removed.contains(&id) {
                    removed.push(id);
                }
            }
            if removed.is_empty() {
                return Ok(None);
            }
            asset_ids.retain(|id| !removed.contains(id));
            Ok(Some(MarketSubscriptionUpdate::unsubscribe(removed)))
        }
    }
}

impl Protocol for ClobMarketProtocol {
    type Event = MarketEvent;
    type Error = ClobWsError;
    type Membership = Vec<String>;
    type Change = MarketChange;
    type Ack = ();

    /// The upgrade request for the URL. The supervisor does not send it:
    /// `connect` is overridden, and the per-address dial upgrades each
    /// address's connection from the URL itself. The trait requires it.
    fn handshake(&mut self) -> Result<Handshake, ClobWsError> {
        Ok(self
            .url
            .as_str()
            .into_client_request()
            .map_err(WsError::from)?)
    }

    /// clob's per-address dial (D20), where the kit's default dials only
    /// the first address `handshake`'s request names.
    async fn connect(&mut self, timeout: Duration) -> Result<Socket, ClobWsError> {
        dial(&self.url, timeout).await
    }

    /// One subscribe frame with the whole membership and the builder's
    /// options, even when the membership is empty: the venue keeps such a
    /// socket open.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        asset_ids: &Vec<String>,
    ) -> Result<(), ClobWsError> {
        let subscription = MarketSubscription::with_options(asset_ids.clone(), self.options);
        let frame = serde_json::to_string(&subscription).map_err(ClobWsError::Json)?;
        connection.send(Message::Text(frame.into())).await?;
        Ok(())
    }

    /// A subscribe or an unsubscribe on the live connection, recorded before
    /// its frame is sent. A connection that dies under the send leaves
    /// the change recorded and answers it `Ok`: the next read ends the
    /// connection, and the replay carries the change.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        asset_ids: &mut Vec<String>,
        change: MarketChange,
    ) -> Result<(), ClobWsError> {
        let Some(update) = change_membership(asset_ids, change)? else {
            return Ok(());
        };
        let frame = serde_json::to_string(&update).map_err(ClobWsError::Json)?;
        if let Err(err) = connection.send(Message::Text(frame.into())).await {
            tracing::debug!(%err, "the connection failed under a change; it is recorded for the replay");
        }
        Ok(())
    }

    /// A change during an outage is recorded and answered at once; the replay
    /// carries it.
    fn record(
        &mut self,
        asset_ids: &mut Vec<String>,
        change: MarketChange,
    ) -> Offline<MarketChange, (), ClobWsError> {
        Offline::Recorded(change_membership(asset_ids, change).map(|_| ()))
    }

    fn decode(&mut self, frame: &Message) -> Decoded<MarketEvent, ClobWsError> {
        let Some(text) = data_text(frame) else {
            return Decoded::Nothing;
        };
        if is_array_frame(text) {
            // The initial dump and each subscribe answer carry one book per
            // asset; every one is yielded, where the bare tier keeps the first.
            return match serde_json::from_str::<Vec<BookMessage>>(text) {
                Ok(books) if books.is_empty() => Decoded::Nothing,
                Ok(books) => Decoded::Events(
                    books
                        .into_iter()
                        .map(|book| MarketEvent::Message(MarketMessage::Book(book)))
                        .collect(),
                ),
                Err(err) => Decoded::Error(ClobWsError::Json(err)),
            };
        }
        match MarketMessage::from_json(text) {
            Ok(message) => Decoded::Event(MarketEvent::Message(message)),
            Err(err) => Decoded::Error(ClobWsError::Json(err)),
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<MarketEvent, ClobWsError>) -> bool {
        true
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<MarketEvent, ClobWsError>) -> bool {
        matches!(decoded, Decoded::Event(_) | Decoded::Events(_))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Text("PING".into()))
    }

    fn is_retriable(err: &ClobWsError) -> bool {
        err.is_retriable()
    }

    fn recover(&self, err: &ClobWsError) -> Recovery {
        recover(err)
    }

    /// A refused upgrade's `Retry-After`, as the error's class reads it, which
    /// the reconnect waits out up to the backoff's ceiling (DRIFT R11).
    fn retry_after(&self, err: &ClobWsError) -> Option<Duration> {
        err.retry_after()
    }

    fn disconnected(&mut self, outage: Outage<ClobWsError>) -> Option<MarketEvent> {
        Some(MarketEvent::Disconnected {
            reason: disconnect_reason(outage),
        })
    }

    fn reconnected(&mut self) -> MarketEvent {
        MarketEvent::Reconnected
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(EVENT_QUEUE)
    }
}

/// Builder for a supervised market socket.
///
/// Defaults: the production host, a text `PING` every 10 s whatever the
/// traffic, stale after 30 s without a frame of any kind, reconnect backoff
/// from 500 ms doubling to 60 s, and a 10 s connect timeout for each address
/// and each step of the connect. The connection never rotates. A refusal's
/// `Retry-After` is waited out, up to the backoff's ceiling.
#[derive(Debug, Clone)]
pub struct MarketWsBuilder {
    url: String,
    options: MarketSubscriptionOptions,
    timing: Timing,
}

impl Default for MarketWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketWsBuilder {
    /// A builder with the defaults above and no subscription options.
    pub fn new() -> Self {
        Self {
            url: WS_MARKET_URL.to_owned(),
            options: MarketSubscriptionOptions::default(),
            timing: default_timing(),
        }
    }

    /// Connect to a different endpoint. It must be `wss://`, or `ws://` to a
    /// loopback host (`127.0.0.1`, `[::1]` or `localhost`): credentials never
    /// travel unencrypted to a remote host, and a test reaches a local
    /// server. Anything else is refused with [`ClobWsError::InvalidMessage`].
    pub fn url(mut self, url: impl Into<String>) -> Result<Self, ClobWsError> {
        self.url = checked_url(url.into())?;
        Ok(self)
    }

    /// The subscription options every connection's subscribe frame carries,
    /// the replay after a reconnect included.
    pub fn options(mut self, options: MarketSubscriptionOptions) -> Self {
        self.options = options;
        self
    }

    polyoxide_ws::timing_setters! {
        /// How often to send the text `PING`, whatever the traffic. It is sent
        /// without awaiting its `PONG`, and goes out while the consumer's
        /// queue is full too. None goes out while a membership change is in
        /// flight, which `stale_after` bounds.
        ping_interval;
        /// Silence after which the connection is presumed dead and replaced.
        /// Any frame counts, a `PONG` included, so a connection that is quiet
        /// but answers pings stays up; time spent waiting on a full queue does
        /// not count. Also bounds each replay: one the server leaves
        /// unanswered that long replaces the connection.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max`. There is no
        /// cap on reconnect attempts; only the delay is capped, at `max`. Both
        /// are raised to at least 1 ms, and an `initial` above `max` is lowered
        /// to `max`. A reconnect refused with a `Retry-After` waits at least
        /// that long, up to `max` (DRIFT R11).
        backoff;
        /// How long one step of a connection attempt may take, for each
        /// address the host resolves to: the TCP connect, and then the TLS
        /// handshake and the WebSocket upgrade together. A timed-out attempt
        /// is retried like any other lost connection.
        connect_timeout;
    }

    /// Open the connection, subscribe to `asset_ids`, and start supervising.
    ///
    /// Fails with [`ClobWsError::InvalidMessage`] for more than 500 distinct
    /// ids, before connecting, and returns the first connection attempt's failure
    /// as it is: a setup that is wrong is reported rather than retried. Later
    /// failures reconnect. An empty list is valid and stays open under the
    /// keep-alive, and a duplicate id is subscribed once.
    pub async fn connect(self, asset_ids: Vec<String>) -> Result<SupervisedMarketWs, ClobWsError> {
        // The same rule as `subscribe`: duplicates go before the count.
        let mut membership: Vec<String> = Vec::new();
        change_membership(&mut membership, MarketChange::Subscribe(asset_ids))?;
        Supervisor::new(ClobMarketProtocol::new(self.url, self.options), self.timing)
            .connect(membership)
            .await
    }
}

/// The membership calls of a supervised market socket, on the kit's
/// [`MembershipHandle`](polyoxide_ws::MembershipHandle) that
/// `Supervised::membership` returns.
///
/// Each waits for the task to take the change. A change is recorded before
/// its frame is sent and sends the ids that change only: a subscribe sends
/// the ids not yet subscribed, an unsubscribe those that are, and a change
/// that adds or removes nothing sends nothing. During an outage the change is
/// recorded and answered at once, and the reconnect replays it. The task takes
/// no change while the consumer's queue is full (1024 unread events), so a
/// call made from the task that reads the stream can wait for ever then: make
/// changes from another task, or keep reading while one is pending.
pub trait MarketMembership {
    /// Subscribe to more assets. The venue sends a `book` for each newly added
    /// asset. Fails with [`ClobWsError::InvalidMessage`] if the subscription
    /// would pass 500 ids, and changes nothing then.
    fn subscribe(
        &self,
        asset_ids: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send;

    /// Unsubscribe from assets. Unsubscribing every asset keeps the
    /// connection, and a reconnect then replays an empty subscription.
    fn unsubscribe(
        &self,
        asset_ids: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send;
}

impl MarketMembership for MembershipHandle<ClobMarketProtocol> {
    fn subscribe(
        &self,
        asset_ids: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send {
        self.apply(MarketChange::Subscribe(asset_ids))
    }

    fn unsubscribe(
        &self,
        asset_ids: Vec<String>,
    ) -> impl Future<Output = Result<(), ClobWsError>> + Send {
        self.apply(MarketChange::Unsubscribe(asset_ids))
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use polyoxide_venue::Class;
    use polyoxide_ws::{
        test_server::{fast, next_event, next_item, Script, ScriptedServer, WINDOW},
        Mode,
    };

    use super::*;
    use crate::clob::ws::{client::MAX_SUBSCRIPTIONS_PER_CONNECTION, scripted, SubscriptionLevel};

    fn book_json(asset: &str) -> String {
        format!(
            r#"{{"event_type":"book","asset_id":"{asset}","market":"m","timestamp":"1","hash":"h","bids":[],"asks":[],"last_trade_price":null}}"#
        )
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    /// A builder for `server`, on the kit's test timing: a `PING` every 50 ms,
    /// stale after 300 ms, and a 10 to 20 ms backoff.
    fn builder(server: &ScriptedServer) -> MarketWsBuilder {
        scripted::hold_a_second_dispatcher();
        let mut builder = MarketWsBuilder::new()
            .url(server.url.clone())
            .expect("a loopback URL is accepted");
        builder.timing = fast();
        builder
    }

    /// A server that answers every `PING` with a `PONG`, playing `scripts`.
    async fn serve(scripts: Vec<Script>) -> ScriptedServer {
        ScriptedServer::start_answering(scripts, scripted::pong).await
    }

    fn pings(server: &ScriptedServer) -> usize {
        server.count_frames(|frame| frame == "PING")
    }

    /// The membership frames the server received, in order, `PING`s excluded.
    fn operations(server: &ScriptedServer) -> Vec<String> {
        server
            .client_frames()
            .into_iter()
            .filter(|frame| frame.contains("operation"))
            .collect()
    }

    async fn next_book(ws: &mut SupervisedMarketWs) -> String {
        match next_event(ws).await {
            MarketEvent::Message(MarketMessage::Book(book)) => book.asset_id,
            other => panic!("expected a book, got {other:?}"),
        }
    }

    async fn assert_quiet(ws: &mut SupervisedMarketWs) {
        if let Ok(item) = tokio::time::timeout(Duration::from_millis(200), ws.next()).await {
            panic!("expected a quiet stream, got {item:?}");
        }
    }

    #[test]
    fn the_defaults_are_the_documented_ones() {
        let builder = MarketWsBuilder::default();
        assert_eq!(builder.url, WS_MARKET_URL);
        assert_eq!(builder.options, MarketSubscriptionOptions::default());
        assert_eq!(builder.timing, default_timing());
        assert_eq!(builder.timing.ping_interval, Some(Duration::from_secs(10)));
        assert_eq!(builder.timing.stale_after, Duration::from_secs(30));
        assert_eq!(builder.timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(builder.timing.max_backoff, Duration::from_secs(60));
        assert_eq!(builder.timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(builder.timing.max_connection_age, None);
    }

    #[test]
    fn the_builder_takes_a_wss_url_or_a_loopback_ws_url() {
        for url in [
            "wss://example.com/ws/market",
            "ws://127.0.0.1:9/ws/market",
            "ws://localhost:9/ws/market",
            "ws://[::1]:9/ws/market",
        ] {
            let builder = MarketWsBuilder::new().url(url).expect(url);
            assert_eq!(builder.url, url);
        }
        for url in [
            "ws://example.com/ws/market",
            "https://example.com/ws/market",
        ] {
            let err = MarketWsBuilder::new().url(url).expect_err(url);
            assert!(
                matches!(&err, ClobWsError::InvalidMessage(text)
                    if text == &format!("the URL must be wss://, or ws:// to a loopback host: {url}")),
                "{err:?}"
            );
            assert_eq!(err.class(), Class::InvalidRequest);
        }
    }

    #[test]
    fn the_handshake_is_the_url_as_a_client_request() {
        let mut protocol = ClobMarketProtocol::new(
            "wss://example.com/ws/market".to_owned(),
            MarketSubscriptionOptions::default(),
        );
        let request = protocol.handshake().expect("a client request");
        assert_eq!(request.uri().host(), Some("example.com"));
        assert_eq!(request.uri().path(), "/ws/market");
    }

    #[tokio::test]
    async fn a_server_close_is_one_outage_that_names_its_code() {
        let server = serve(vec![
            Script::hold()
                .after_first_frame()
                .then_close()
                .close_with(1001, "restart"),
            Script::hold(),
        ])
        .await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");

        let down = next_event(&mut ws).await;
        assert!(
            matches!(&down, MarketEvent::Disconnected {
                reason: DisconnectReason::Closed { code: Some(1001), reason }
            } if reason == "restart"),
            "{down:?}"
        );
        let up = next_event(&mut ws).await;
        assert!(matches!(up, MarketEvent::Reconnected), "{up:?}");
        // The outage is over: no second marker, and one more connection only.
        assert_quiet(&mut ws).await;
        assert_eq!(server.connection_count(), 2);
    }

    #[tokio::test]
    async fn a_dropped_connection_names_its_error() {
        let server = serve(vec![
            Script::hold().after_first_frame().then_cut(),
            Script::hold(),
        ])
        .await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");

        let down = next_event(&mut ws).await;
        assert!(
            matches!(&down, MarketEvent::Disconnected {
                reason: DisconnectReason::Error(err)
            } if err.class() == Class::Network),
            "{down:?}"
        );
        let up = next_event(&mut ws).await;
        assert!(matches!(up, MarketEvent::Reconnected), "{up:?}");
    }

    #[tokio::test]
    async fn a_stalled_socket_is_replaced() {
        // The server reads the subscription and then falls silent: it answers
        // no `PING`, so nothing arrives for the staleness window.
        let server = serve(vec![
            Script::hold().after_first_frame().go_silent(),
            Script::hold(),
        ])
        .await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");

        let down = next_event(&mut ws).await;
        assert!(
            matches!(&down, MarketEvent::Disconnected {
                reason: DisconnectReason::Stale { silent }
            } if *silent >= Duration::from_millis(300)),
            "{down:?}"
        );
        let up = next_event(&mut ws).await;
        assert!(matches!(up, MarketEvent::Reconnected), "{up:?}");
        assert_eq!(server.connection_count(), 2);
    }

    #[tokio::test]
    async fn every_connection_replays_the_whole_membership_changes_included() {
        // The first connection falls silent after its subscription, so the
        // changes below land on it, or on the outage that follows; either way
        // the second connection's subscription carries them.
        let server = serve(vec![
            Script::hold().after_first_frame().go_silent(),
            Script::hold(),
        ])
        .await;
        let options = MarketSubscriptionOptions::default()
            .with_custom_features()
            .level(SubscriptionLevel::Three);
        let ws = builder(&server)
            .options(options)
            .connect(ids(&["a", "b"]))
            .await
            .expect("connects");
        let membership = ws.membership();
        membership.subscribe(ids(&["c"])).await.expect("subscribes");
        membership
            .unsubscribe(ids(&["a"]))
            .await
            .expect("unsubscribes");

        server
            .wait_for("the second subscription", |s| s.first_frames().len() == 2)
            .await;
        assert_eq!(
            server.first_frames(),
            [
                r#"{"assets_ids":["a","b"],"type":"market","custom_feature_enabled":true,"level":3}"#,
                r#"{"assets_ids":["b","c"],"type":"market","custom_feature_enabled":true,"level":3}"#,
            ]
        );
    }

    #[tokio::test]
    async fn a_change_sends_the_ids_that_change_and_nothing_else() {
        let server = serve(vec![Script::hold()]).await;
        let ws = builder(&server)
            .connect(ids(&["a", "b", "a"]))
            .await
            .expect("connects");
        let membership = ws.membership();

        // Duplicates, and an id already subscribed, are not sent again.
        membership
            .subscribe(ids(&["c", "a", "c"]))
            .await
            .expect("subscribes");
        // A change that adds nothing sends nothing, and is answered.
        membership
            .subscribe(ids(&["b"]))
            .await
            .expect("an id already subscribed");
        membership
            .unsubscribe(ids(&["a", "x"]))
            .await
            .expect("unsubscribes");

        server
            .wait_for("both membership frames", |s| operations(s).len() == 2)
            .await;
        assert_eq!(
            operations(&server),
            [
                r#"{"operation":"subscribe","assets_ids":["c"]}"#,
                r#"{"operation":"unsubscribe","assets_ids":["a"]}"#,
            ]
        );
        assert_eq!(
            server.first_frames(),
            [r#"{"assets_ids":["a","b"],"type":"market"}"#],
            "the initial subscription holds no duplicate"
        );
    }

    #[tokio::test]
    async fn a_change_during_an_outage_is_answered_at_once_and_replayed() {
        // The reconnect waits 800 ms, which leaves time for the change.
        let server = serve(vec![
            Script::hold().after_first_frame().then_close(),
            Script::hold(),
        ])
        .await;
        let mut ws = builder(&server)
            .backoff(Duration::from_millis(800), Duration::from_millis(800))
            .connect(ids(&["a"]))
            .await
            .expect("connects");
        let membership = ws.membership();

        let down = next_event(&mut ws).await;
        assert!(matches!(down, MarketEvent::Disconnected { .. }), "{down:?}");
        // Held until the reconnect, this would outlast the timeout.
        tokio::time::timeout(
            Duration::from_millis(400),
            membership.subscribe(ids(&["b"])),
        )
        .await
        .expect("answered during the outage")
        .expect("recorded");

        let up = next_event(&mut ws).await;
        assert!(matches!(up, MarketEvent::Reconnected), "{up:?}");
        server
            .wait_for("the second subscription", |s| s.first_frames().len() == 2)
            .await;
        assert_eq!(
            server.first_frames()[1],
            r#"{"assets_ids":["a","b"],"type":"market"}"#
        );
        assert!(
            operations(&server).is_empty(),
            "the replay is the only frame"
        );
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

        let mut protocol =
            ClobMarketProtocol::new(server.url.clone(), MarketSubscriptionOptions::default());
        let mut membership = ids(&["a"]);
        let result = protocol
            .apply(
                &mut connection,
                &mut membership,
                MarketChange::Subscribe(ids(&["b"])),
            )
            .await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(membership, ids(&["a", "b"]));
    }

    #[test]
    fn a_change_during_an_outage_past_the_cap_is_refused_and_changes_nothing() {
        let mut protocol = ClobMarketProtocol::new(
            WS_MARKET_URL.to_owned(),
            MarketSubscriptionOptions::default(),
        );
        let mut membership: Vec<String> = (0..MAX_SUBSCRIPTIONS_PER_CONNECTION)
            .map(|n| n.to_string())
            .collect();
        let before = membership.clone();
        match protocol.record(&mut membership, MarketChange::Subscribe(ids(&["extra"]))) {
            Offline::Recorded(Err(ClobWsError::InvalidMessage(text))) => {
                assert!(
                    text.contains("Too many subscriptions (501), max 500"),
                    "{text}"
                );
            }
            _ => panic!("expected a refusal recorded as the answer"),
        }
        assert_eq!(membership, before);
    }

    #[tokio::test]
    async fn connecting_with_501_copies_of_one_id_subscribes_it_once() {
        let server = serve(vec![Script::hold()]).await;
        let copies = vec!["a".to_owned(); MAX_SUBSCRIPTIONS_PER_CONNECTION + 1];
        let _ws = builder(&server)
            .connect(copies)
            .await
            .expect("501 copies are one id");
        server
            .wait_for("the subscription", |s| s.first_frames().len() == 1)
            .await;
        assert_eq!(
            server.first_frames(),
            [r#"{"assets_ids":["a"],"type":"market"}"#]
        );
    }

    #[tokio::test]
    async fn a_501st_id_is_refused_and_changes_nothing() {
        let server = serve(vec![Script::hold()]).await;
        let full: Vec<String> = (0..MAX_SUBSCRIPTIONS_PER_CONNECTION)
            .map(|n| n.to_string())
            .collect();
        let ws = builder(&server)
            .connect(full)
            .await
            .expect("500 ids connect");
        let membership = ws.membership();

        let err = membership
            .subscribe(ids(&["500"]))
            .await
            .expect_err("a 501st id is refused");
        assert!(
            matches!(&err, ClobWsError::InvalidMessage(text)
                if text.contains("Too many subscriptions (501), max 500")),
            "{err:?}"
        );
        // It changed nothing: the refused id is no member, so leaving sends
        // nothing, and one id leaving makes room for it.
        membership
            .unsubscribe(ids(&["500"]))
            .await
            .expect("a non-member leaves quietly");
        membership.unsubscribe(ids(&["0"])).await.expect("leaves");
        membership
            .subscribe(ids(&["500"]))
            .await
            .expect("fits once one has left");

        server
            .wait_for("both membership frames", |s| operations(s).len() == 2)
            .await;
        assert_eq!(
            operations(&server),
            [
                r#"{"operation":"unsubscribe","assets_ids":["0"]}"#,
                r#"{"operation":"subscribe","assets_ids":["500"]}"#,
            ]
        );
    }

    #[tokio::test]
    async fn connecting_with_more_than_500_ids_is_refused_before_connecting() {
        let server = serve(vec![Script::hold()]).await;
        let too_many: Vec<String> = (0..=MAX_SUBSCRIPTIONS_PER_CONNECTION)
            .map(|n| n.to_string())
            .collect();
        let err = builder(&server)
            .connect(too_many)
            .await
            .expect_err("501 ids are refused");
        assert!(
            matches!(&err, ClobWsError::InvalidMessage(text)
                if text.contains("Too many subscriptions (501), max 500")),
            "{err:?}"
        );
        assert_eq!(server.connection_count(), 0);
    }

    #[tokio::test]
    async fn it_pings_on_the_schedule_and_a_pong_is_no_event() {
        let server = serve(vec![Script::hold()]).await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");

        // Eight pings take about 400 ms, longer than the 300 ms staleness
        // window: the `PONG`s are what keep the one connection alive.
        server.wait_for("eight pings", |s| pings(s) >= 8).await;
        assert_eq!(server.connection_count(), 1);
        assert_quiet(&mut ws).await;
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn an_empty_membership_stays_connected_and_keeps_pinging() {
        let server = serve(vec![Script::hold()]).await;
        let mut ws = builder(&server)
            .connect(Vec::new())
            .await
            .expect("an empty list is valid");

        server.wait_for("eight pings", |s| pings(s) >= 8).await;
        assert_eq!(server.connection_count(), 1);
        assert_eq!(
            server.first_frames(),
            [r#"{"assets_ids":[],"type":"market"}"#]
        );
        assert_quiet(&mut ws).await;
    }

    #[tokio::test]
    async fn unsubscribing_the_last_id_keeps_the_connection() {
        let server = serve(vec![Script::hold()]).await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");
        ws.membership()
            .unsubscribe(ids(&["a"]))
            .await
            .expect("unsubscribes");

        server.wait_for("eight pings", |s| pings(s) >= 8).await;
        assert_eq!(server.connection_count(), 1);
        assert_eq!(
            operations(&server),
            [r#"{"operation":"unsubscribe","assets_ids":["a"]}"#]
        );
        assert_quiet(&mut ws).await;
    }

    #[tokio::test]
    async fn keep_alives_are_skipped_and_a_binary_frame_is_decoded_as_text() {
        let frames = vec![
            Message::Text("PONG".into()),
            Message::Text("{}".into()),
            Message::Text("".into()),
            Message::Text(r#"{"subscribed":true}"#.into()),
            Message::Binary(book_json("a").into_bytes().into()),
        ];
        let server = serve(vec![Script::messages(frames).after_first_frame()]).await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");

        assert_eq!(next_book(&mut ws).await, "a");
        assert_quiet(&mut ws).await;
    }

    #[tokio::test]
    async fn an_array_frame_yields_every_book() {
        let array = format!("[{},{}]", book_json("a"), book_json("b"));
        let frames = vec![
            Message::Text("[]".into()),
            Message::Text(array.into()),
            Message::Text(book_json("c").into()),
        ];
        let server = serve(vec![Script::messages(frames).after_first_frame()]).await;
        let mut ws = builder(&server)
            .connect(ids(&["a", "b"]))
            .await
            .expect("connects");

        assert_eq!(next_book(&mut ws).await, "a");
        assert_eq!(next_book(&mut ws).await, "b");
        assert_eq!(next_book(&mut ws).await, "c");
        assert_quiet(&mut ws).await;
    }

    #[tokio::test]
    async fn an_array_frame_with_leading_whitespace_yields_every_book() {
        let array = format!("  \n[{},{}]", book_json("a"), book_json("b"));
        let server = serve(vec![
            Script::messages(vec![Message::Text(array.into())]).after_first_frame()
        ])
        .await;
        let mut ws = builder(&server)
            .connect(ids(&["a", "b"]))
            .await
            .expect("connects");

        assert_eq!(next_book(&mut ws).await, "a");
        assert_eq!(next_book(&mut ws).await, "b");
        assert_quiet(&mut ws).await;
    }

    #[tokio::test]
    async fn a_frame_that_does_not_decode_is_an_error_and_the_stream_goes_on() {
        let frames = vec![
            Message::Text(r#"{"event_type":"book"}"#.into()),
            Message::Text(r#"{"event_type":"mystery"}"#.into()),
            Message::Text("[1,2]".into()),
            Message::Text(book_json("a").into()),
        ];
        let server = serve(vec![Script::messages(frames).after_first_frame()]).await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");

        // `[1,2]` has no `event_type`, so it is skipped like any keep-alive.
        for _ in 0..2 {
            let item = next_item(&mut ws).await;
            assert!(matches!(item, Err(ClobWsError::Json(_))), "{item:?}");
        }
        assert_eq!(next_book(&mut ws).await, "a");
        assert_eq!(server.connection_count(), 1, "no reconnect");
    }

    #[tokio::test]
    async fn a_close_the_venue_refuses_with_ends_the_stream() {
        let server = serve(vec![Script::hold()
            .after_first_frame()
            .then_close()
            .close_with(1008, "policy")])
        .await;
        let mut ws = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect("connects");

        let err = next_item(&mut ws)
            .await
            .expect_err("a refusal is yielded, not retried");
        assert_eq!(
            err.class(),
            Class::VenueRefusal {
                code: Some("1008".into())
            }
        );
        let end = tokio::time::timeout(WINDOW, ws.next())
            .await
            .expect("the stream ends");
        assert!(end.is_none(), "{end:?}");
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn the_first_failure_is_returned_not_retried() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(503)]).await;
        let err = builder(&server)
            .connect(ids(&["a"]))
            .await
            .expect_err("a refused upgrade does not connect");
        assert_eq!(err.class(), Class::Unavailable { code: None });
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn the_supervisor_dials_with_clobs_per_address_connect() {
        // clob's dial maps a step that outlived its bound to its own
        // `ConnectTimeout`, naming the URL; the kit's default connect would
        // return the kit's error wrapped in `Connection`.
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let step = Duration::from_millis(200);
        let err = builder(&server)
            .connect_timeout(step)
            .connect(ids(&["a"]))
            .await
            .expect_err("a server that never upgrades does not connect");
        assert!(
            matches!(&err, ClobWsError::ConnectTimeout { url, timeout }
                if url == &server.url && *timeout == step),
            "{err:?}"
        );
    }
}
