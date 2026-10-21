//! Kalshi's socket protocol, as the kit's `Supervisor` drives it, and the membership calls on
//! its handle.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
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
use tokio::time::Instant;

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
/// The first id of the commands the protocol numbers itself: the re-snapshot it sends from
/// `follow_up`, which has no connection to take an id from. The kit's own ids count from 1 on
/// each connection, so the two never meet.
const FOLLOW_UP_ID_BASE: u64 = 1_000_000_000;

polyoxide_venue::wire_enum! {
    /// A channel of Kalshi's socket this client carries. One ticker set serves every channel
    /// chosen; Kalshi gives each its own subscription. The order is the order a membership
    /// change goes to the subscriptions in.
    #[derive(PartialOrd, Ord)]
    #[non_exhaustive]
    pub enum Channel {
        /// `orderbook_delta`: a snapshot of each market's book, then its changes, in YES
        /// pricing. The default channel.
        OrderbookDelta => "orderbook_delta",
        /// `ticker`: a market's last price, best bid and ask, volume and open interest. It
        /// carries no `seq`.
        Ticker => "ticker",
        /// `trade`: each public trade.
        Trade => "trade",
    }
}

/// One change to the subscribed markets, made through [`PredictionsMembership`] or
/// `MembershipHandle::apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    /// Subscribe to these markets on every channel; one already subscribed is skipped.
    Subscribe(Vec<Ticker>),
    /// Unsubscribe from these markets on every channel; one not subscribed is skipped.
    Unsubscribe(Vec<Ticker>),
    /// Ask for a fresh snapshot of these subscribed markets' books. The membership does not
    /// change, and the snapshots arrive on the stream.
    Snapshot(Vec<Ticker>),
}

/// What a command the protocol sent outside the kit's `request` was for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FollowUp {
    /// The re-snapshot after a gap on the order-book `sid`.
    Resync,
    /// A `snapshot` call's request, with the number of markets whose snapshot is still to
    /// come: Kalshi answers every market asked for, an unknown one with an empty snapshot.
    Requested(usize),
}

/// A re-snapshot in flight after a gap on the order-book `sid`.
#[derive(Debug)]
struct Resync {
    /// The id of the `get_snapshot` command.
    id: u64,
    /// The order-book subscription.
    sid: u64,
    /// The markets whose snapshot has not arrived: their deltas are dropped meanwhile.
    pending: BTreeSet<String>,
    /// When the command was queued.
    queued_at: Instant,
}

/// Kalshi's refusal of a command that came after another of the same change had been
/// accepted, kept so that the next frame read can end the connection with it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Refusal {
    /// An error frame, with its code and message.
    Server { code: u32, message: String },
    /// An error frame that did not decode: the frame's text, and why.
    Unread { text: String, cause: String },
}

impl Refusal {
    /// What the error that was given to the change's caller says, for the connection to end
    /// with. Only Kalshi's answers are refusals; the connection failing under a command is
    /// handled before one is asked for, and the rest never come out of a command.
    fn of(err: &PredictionsWsError) -> Option<Self> {
        match err {
            PredictionsWsError::Server { code, message } => Some(Self::Server {
                code: *code,
                message: message.clone(),
            }),
            PredictionsWsError::Json { text, source } => Some(Self::Unread {
                text: text.clone(),
                cause: source.to_string(),
            }),
            PredictionsWsError::Connection(_)
            | PredictionsWsError::SequenceGap { .. }
            | PredictionsWsError::InvalidUrl { .. }
            | PredictionsWsError::SnapshotTimeout { .. }
            | PredictionsWsError::InvalidRequest { .. }
            | PredictionsWsError::Credentials(_) => None,
        }
    }

    /// A copy of the error the change's caller was given.
    fn error(&self) -> PredictionsWsError {
        match self {
            Self::Server { code, message } => PredictionsWsError::Server {
                code: *code,
                message: message.clone(),
            },
            Self::Unread { text, cause } => PredictionsWsError::Json {
                text: text.clone(),
                source: serde::de::Error::custom(cause),
            },
        }
    }
}

/// Kalshi's socket, which [`PredictionsWsBuilder`](crate::predictions::ws::PredictionsWsBuilder)
/// hands to the kit's `Supervisor`.
///
/// Its membership is the set of tickers subscribed, one set for every channel chosen, replayed
/// on every fresh connection as one `subscribe` naming the channels (with `use_yes_price: true`
/// when the order book is one of them). Kalshi answers with a `subscribed` for each channel, and
/// the protocol keeps each channel's `sid`. A change goes out as one `update_subscription` on
/// each live `sid`, in channel order; the first add with no subscription sends a `subscribe`,
/// and emptying the membership sends an `unsubscribe` for each `sid` and keeps the connection.
/// Kalshi refuses a `ticker` or `trade` subscribe with no markets (code 14 on the demo host,
/// 2026-10-21); the protocol never sends one, so an empty membership is no subscription on
/// every channel, on the first connection or after a reconnect. A change is recorded before
/// its frames are sent: one whose connection dies under it is answered `Ok` and replayed, one
/// Kalshi refuses is undone, and one made during an outage is answered at once and replayed.
/// A refusal that follows a command of the same change that Kalshi accepted leaves its
/// subscriptions out of step with the undone membership, so the change fails with Kalshi's
/// error and the connection is replaced: the next frame read ends it, with that error as the
/// reason, and the replay brings the subscriptions back to the membership. A refusal of the
/// first command accepted nothing, so the connection stays.
/// Its divergence rows:
///
/// - **D4, the ping:** an empty protocol Ping on the wall clock.
/// - **D5, liveness:** every frame, the server's Pings and the Pongs to the client's own
///   included.
/// - **D6, a delivered connection:** one that decoded an event.
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
///   against the last of its subscription. A gap on the `trade` subscription reconnects. A
///   gap on the order book's does not: the protocol yields `Event::Resyncing`, queues one
///   `get_snapshot` for the whole membership through the kit's `follow_up`, leaves the
///   subscription's frames unchecked until the first snapshot answering it, or until no
///   market's snapshot is awaited any more (the markets awaited were all removed), when the
///   next frame starts the count afresh, re-anchors the numbering on each such snapshot, and
///   withholds each market's deltas until its snapshot has arrived. It reconnects when the
///   snapshots are not all in by the staleness window after the command was queued (checked
///   on each decoded frame), when an error frame answers the command, and on a second gap
///   while one is being repaired.
///
/// Its queue holds 1024 events.
pub struct PredictionsProtocol {
    url: String,
    credentials: Credentials,
    /// The channels carried, in channel order.
    channels: BTreeSet<Channel>,
    /// The staleness window, which also bounds the wait for a re-snapshot.
    stale_after: Duration,
    /// What this connection's frames have said.
    sequences: Sequences,
    /// Frames the protocol has decided to send after a decode, oldest first.
    queued: VecDeque<String>,
    /// The id of the next command the protocol numbers itself.
    next_follow_up: u64,
    /// A copy of the membership, kept as it changes, since `decode` has no access to it.
    tickers: BTreeSet<Ticker>,
    /// The re-snapshot in flight, if any.
    resync: Option<Resync>,
    /// The marker of a re-snapshot that the frame which raised it could not carry, because
    /// that frame was an error: it leads the next frame instead.
    lead: Option<Event>,
    /// Whether Kalshi has just answered the re-snapshot's command with an error frame, so
    /// that `recover`, which the supervisor calls with that error straight after, reconnects
    /// on it where it would skip the same frame elsewhere; `reset` clears it.
    snapshot_refused: bool,
    /// Whether Kalshi has just answered the replay's `subscribe` with an error frame, or
    /// with a frame that does not decode. Set by `replay` before it returns the error, so
    /// that `recover`, which the supervisor calls with that error straight after, can tell
    /// it from the same frame met on a live connection; `reset` clears it for the next
    /// attempt.
    replay_refused: bool,
    /// Kalshi's refusal of a command that followed an accepted command of the same change.
    /// While it is set the connection is spent: `decode` ends it with this error on the next
    /// frame, and `recover`, which the supervisor calls with that error straight after,
    /// reconnects on it where it would skip the same frame elsewhere; `reset` clears it.
    change_refused: Option<Refusal>,
}

impl std::fmt::Debug for PredictionsProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PredictionsProtocol")
            .field("url", &self.url)
            .field("credentials", &self.credentials)
            .field("channels", &self.channels)
            .field("sids", &self.sequences.sids)
            .finish_non_exhaustive()
    }
}

impl PredictionsProtocol {
    pub(crate) fn new(url: String, credentials: Credentials) -> Self {
        Self {
            url,
            credentials,
            channels: BTreeSet::from([Channel::OrderbookDelta]),
            stale_after: Duration::from_secs(30),
            sequences: Sequences::default(),
            queued: VecDeque::new(),
            next_follow_up: FOLLOW_UP_ID_BASE,
            tickers: BTreeSet::new(),
            resync: None,
            lead: None,
            snapshot_refused: false,
            replay_refused: false,
            change_refused: None,
        }
    }

    /// The channels to carry and the staleness window a re-snapshot is held to.
    pub(crate) fn configured(mut self, channels: BTreeSet<Channel>, stale_after: Duration) -> Self {
        self.channels = channels;
        self.stale_after = stale_after;
        self
    }

    /// Whether this connection holds a subscription on every channel chosen.
    fn subscribed(&self) -> bool {
        !self.channels.is_empty()
            && self
                .channels
                .iter()
                .all(|channel| self.sequences.sids.contains_key(channel))
    }

    /// The channels chosen, in channel order.
    fn channel_list(&self) -> Vec<Channel> {
        self.channels.iter().copied().collect()
    }

    /// The subscription of each channel chosen, in channel order.
    fn channel_sids(&self) -> Vec<u64> {
        self.channels
            .iter()
            .filter_map(|channel| self.sequences.sids.get(channel).copied())
            .collect()
    }

    /// Note what Kalshi's answer to `command` settled. A `subscribe`'s answers filled in the
    /// subscriptions as they were read, so only an `unsubscribe` has something to forget.
    fn accepted(&mut self, command: &Command) {
        if let Command::Unsubscribe { sid } = command {
            self.sequences.sids.retain(|_, held| held != sid);
            self.sequences.last.remove(sid);
            self.sequences.unanchored.remove(sid);
            if self
                .resync
                .as_ref()
                .is_some_and(|resync| resync.sid == *sid)
            {
                if let Some(resync) = self.resync.take() {
                    self.sequences.follow_ups.remove(&resync.id);
                }
            }
            // Every requested snapshot is of the order book's subscription, which is gone
            // when it is the one unsubscribed.
            if !self.sequences.sids.contains_key(&Channel::OrderbookDelta) {
                self.sequences
                    .follow_ups
                    .retain(|_, kind| !matches!(kind, FollowUp::Requested(_)));
            }
        }
    }

    /// Stop waiting for the snapshots of markets no longer subscribed.
    fn forget_markets(&mut self, gone: &[String]) {
        let Some(resync) = &mut self.resync else {
            return;
        };
        resync.pending.retain(|market| !gone.contains(market));
        if resync.pending.is_empty() {
            let (id, sid) = (resync.id, resync.sid);
            self.sequences.follow_ups.remove(&id);
            self.resync = None;
            // Nothing is awaited any more. A subscription still unanchored will not be
            // re-anchored by a snapshot now, so its next frame starts the count afresh; one a
            // snapshot already re-anchored keeps the count that snapshot set.
            if self.sequences.unanchored.remove(&sid) {
                self.sequences.last.remove(&sid);
            }
        }
    }

    /// A snapshot of `market` arrived, answering command `id` when it is the re-snapshot's
    /// or a `snapshot` call's.
    fn snapshot_arrived(&mut self, id: Option<u64>, market: &str) {
        if let Some(id) = id {
            if let Some(FollowUp::Requested(left)) = self.sequences.follow_ups.get_mut(&id) {
                *left = left.saturating_sub(1);
                if *left == 0 {
                    self.sequences.follow_ups.remove(&id);
                }
            }
        }
        let Some(resync) = &mut self.resync else {
            return;
        };
        if id != Some(resync.id) {
            return;
        }
        resync.pending.remove(market);
        if resync.pending.is_empty() {
            let id = resync.id;
            self.sequences.follow_ups.remove(&id);
            self.resync = None;
        }
    }

    /// Whether `market`'s snapshot is still awaited, so its deltas are dropped.
    fn is_pending(&self, market: &str) -> bool {
        self.resync
            .as_ref()
            .is_some_and(|resync| resync.pending.contains(market))
    }

    /// Whether `id` is that of a command the protocol sent outside the kit's `request`.
    fn is_follow_up(&self, id: Option<u64>) -> bool {
        id.is_some_and(|id| self.sequences.follow_ups.contains_key(&id))
    }

    /// The timeout of a re-snapshot that has outlived the staleness window, if there is one.
    fn snapshot_overdue(&self) -> Option<PredictionsWsError> {
        let resync = self.resync.as_ref()?;
        let waited = resync.queued_at.elapsed();
        (waited >= self.stale_after).then(|| PredictionsWsError::SnapshotTimeout {
            sid: resync.sid,
            waited,
        })
    }

    /// The markets a `snapshot` call asks for, once each in order, or why it is refused: a
    /// socket without the order-book channel has no books, and a market outside the
    /// membership has none to ask for.
    fn snapshot_names(
        &self,
        membership: &BTreeSet<Ticker>,
        tickers: &[Ticker],
    ) -> Result<Vec<String>, PredictionsWsError> {
        if !self.channels.contains(&Channel::OrderbookDelta) {
            return Err(PredictionsWsError::InvalidRequest {
                reason: "a snapshot needs the orderbook_delta channel".to_owned(),
            });
        }
        if let Some(outside) = tickers.iter().find(|ticker| !membership.contains(*ticker)) {
            return Err(PredictionsWsError::InvalidRequest {
                reason: format!(
                    "{} is not subscribed, so there is no book of it to ask for",
                    outside.as_str()
                ),
            });
        }
        Ok(tickers
            .iter()
            .map(|ticker| ticker.as_str().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    /// Start the re-snapshot of the whole membership after a gap on the order-book `sid`:
    /// queue the `get_snapshot`, withhold every market's deltas, and mark it for the
    /// consumer.
    fn begin_resync(&mut self, sid: u64) -> Event {
        let tickers = names(self.tickers.iter());
        let id = self.next_follow_up;
        self.next_follow_up += 1;
        self.sequences.follow_ups.insert(id, FollowUp::Resync);
        let command = Command::Update {
            sid,
            action: "get_snapshot",
            tickers: tickers.clone(),
        };
        self.queued.push_back(command.frame(id));
        self.resync = Some(Resync {
            id,
            sid,
            pending: tickers.iter().cloned().collect(),
            queued_at: Instant::now(),
        });
        Event::Resyncing { tickers }
    }

    /// What a gap in a subscription's numbering comes to: a re-snapshot when it is the order
    /// book's and none is in flight, a reconnect for any other.
    fn gapped(&mut self, gap: PredictionsWsError) -> Result<Event, PredictionsWsError> {
        let sid = match &gap {
            PredictionsWsError::SequenceGap { sid, .. } => *sid,
            _ => return Err(gap),
        };
        if !self.sequences.is_book(sid) || self.resync.is_some() || self.tickers.is_empty() {
            return Err(gap);
        }
        Ok(self.begin_resync(sid))
    }

    /// What one frame decodes to, before a pending re-snapshot's marker is put ahead of it.
    fn decode_frame(&mut self, frame: &Message) -> Decoded<Event, PredictionsWsError> {
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
        if peek.sequence().is_some() {
            if self.sequences.ahead > 0 {
                // The correlation closure counted this frame while a request waited.
                self.sequences.ahead -= 1;
            } else if let Err(gap) = self.sequences.observe(&peek) {
                // The frame that found the gap is dropped: it belongs to a book that is
                // about to be replaced.
                return match self.gapped(gap) {
                    Ok(resyncing) => Decoded::Event(resyncing),
                    Err(err) => Decoded::Error(err),
                };
            }
        }
        match peek.kind.as_deref() {
            Some("orderbook_snapshot") => match parse::<frames::Snapshot>(text) {
                Ok(frame) => {
                    self.snapshot_arrived(frame.id, &frame.msg.market_ticker);
                    Decoded::Event(Event::Snapshot(frame.msg.into()))
                }
                Err(err) => Decoded::Error(err),
            },
            Some("orderbook_delta") => match parse::<frames::Delta>(text) {
                Ok(frame) if self.is_pending(&frame.msg.market_ticker) => Decoded::Nothing,
                Ok(frame) => Decoded::Event(Event::Delta(frame.msg.into())),
                Err(err) => Decoded::Error(err),
            },
            Some("ticker") if self.channels.contains(&Channel::Ticker) => {
                match parse::<frames::TickerFrame>(text) {
                    Ok(frame) => Decoded::Event(Event::Ticker(frame.msg.into())),
                    Err(err) => Decoded::Error(err),
                }
            }
            Some("trade") if self.channels.contains(&Channel::Trade) => {
                match parse::<frames::TradeFrame>(text) {
                    Ok(frame) => Decoded::Event(Event::Trade(frame.msg.into())),
                    Err(err) => Decoded::Error(err),
                }
            }
            // An error that answers no command of ours is Kalshi reporting on a
            // subscription; one that answers a request the protocol sent itself, which no
            // `request` awaits, is the stream's too.
            Some("error") if !peek.answers_a_command() || self.is_follow_up(peek.id) => {
                match parse::<frames::Rejected>(text) {
                    Ok(frame) => {
                        if peek.id.is_some_and(|id| {
                            self.sequences.follow_ups.get(&id) == Some(&FollowUp::Resync)
                        }) {
                            self.snapshot_refused = true;
                        }
                        // A refused request is answered: nothing more comes of it.
                        if let Some(id) = peek.id {
                            if matches!(
                                self.sequences.follow_ups.get(&id),
                                Some(FollowUp::Requested(_))
                            ) {
                                self.sequences.follow_ups.remove(&id);
                            }
                        }
                        Decoded::Error(PredictionsWsError::Server {
                            code: frame.msg.code,
                            message: frame.msg.msg,
                        })
                    }
                    Err(err) => Decoded::Error(err),
                }
            }
            // An answer nobody awaits, and every type this socket does not carry.
            _ => Decoded::Nothing,
        }
    }

    /// Make one change on a live connection: the commands it takes, one for each `sid`.
    async fn apply_change(
        &mut self,
        connection: &mut Connection,
        membership: &mut BTreeSet<Ticker>,
        change: Change,
    ) -> Result<(), PredictionsWsError> {
        let before = membership.clone();
        let (commands, dropped): (Vec<Command>, Vec<String>) = match change {
            Change::Subscribe(add) => {
                let new = added(membership, add);
                if new.is_empty() {
                    return Ok(());
                }
                membership.extend(new.iter().cloned());
                let commands: Vec<Command> = if self.subscribed() {
                    self.channel_sids()
                        .into_iter()
                        .map(|sid| Command::Update {
                            sid,
                            action: "add_markets",
                            tickers: names(&new),
                        })
                        .collect()
                } else {
                    // Nothing is subscribed on this connection yet: the whole membership
                    // goes in one subscribe.
                    vec![Command::Subscribe {
                        channels: self.channel_list(),
                        tickers: names(membership.iter()),
                    }]
                };
                (commands, Vec::new())
            }
            Change::Unsubscribe(remove) => {
                let gone = removed(membership, remove);
                if gone.is_empty() {
                    return Ok(());
                }
                for ticker in &gone {
                    membership.remove(ticker);
                }
                // Nothing was ever subscribed on this connection.
                if !self.subscribed() {
                    return Ok(());
                }
                let sids = self.channel_sids();
                let commands: Vec<Command> = if membership.is_empty() {
                    sids.into_iter()
                        .map(|sid| Command::Unsubscribe { sid })
                        .collect()
                } else {
                    sids.into_iter()
                        .map(|sid| Command::Update {
                            sid,
                            action: "delete_markets",
                            tickers: names(&gone),
                        })
                        .collect()
                };
                (commands, names(&gone))
            }
            Change::Snapshot(tickers) => {
                return self
                    .ask_for_snapshots(connection, membership, &tickers)
                    .await;
            }
        };
        let mut taken = 0_usize;
        for command in &commands {
            match send(&mut self.sequences, connection, command).await {
                Ok(_) => {
                    self.accepted(command);
                    taken += 1;
                }
                Err(PredictionsWsError::Connection(err)) => {
                    tracing::debug!(%err, "the connection failed under a change; it is recorded for the replay");
                    return Ok(());
                }
                Err(err) => {
                    *membership = before;
                    if taken > 0 {
                        // Kalshi has acted on part of the change: its subscriptions no longer
                        // match the membership just restored. The next frame ends the
                        // connection, and the replay puts the two back in step.
                        self.change_refused = Refusal::of(&err);
                    }
                    return Err(err);
                }
            }
        }
        self.forget_markets(&dropped);
        Ok(())
    }

    /// A `snapshot` call: send one `get_snapshot` for the markets, with an id from the
    /// connection, and answer once it is sent. The snapshots are the answer and arrive on
    /// the stream, so nothing is awaited; a connection that fails under the send is the
    /// replay's to repair, which brings a snapshot of every market.
    async fn ask_for_snapshots(
        &mut self,
        connection: &mut Connection,
        membership: &BTreeSet<Ticker>,
        tickers: &[Ticker],
    ) -> Result<(), PredictionsWsError> {
        let wanted = self.snapshot_names(membership, tickers)?;
        let Some(sid) = self.sequences.sids.get(&Channel::OrderbookDelta).copied() else {
            return Ok(());
        };
        if wanted.is_empty() {
            return Ok(());
        }
        let id = connection.take_id();
        self.sequences
            .follow_ups
            .insert(id, FollowUp::Requested(wanted.len()));
        let frame = Command::Update {
            sid,
            action: "get_snapshot",
            tickers: wanted,
        }
        .frame(id);
        if let Err(err) = connection.send(Message::Text(frame.into())).await {
            tracing::debug!(%err, "the connection failed under a snapshot request; the replay brings a fresh one");
        }
        Ok(())
    }
}

/// What one connection's frames have said.
///
/// The first `seq` of a subscription is taken as it comes; after it, each must be the
/// previous plus one, except on the order book's subscription after a gap: its frames are not
/// checked until the first snapshot that answers the re-snapshot, or until no market's
/// snapshot is awaited any more, when the next frame starts the count afresh; each such
/// snapshot re-anchors the numbering on its own `seq`, and removing the markets still awaited
/// after one has done so leaves the count it set. The correlation closure of a request observes
/// every sequenced frame it reads while it waits, and counts in `ahead` each push it keeps for
/// the stream, which `decode` meets later and must not observe again. A gap the closure finds is
/// held in `gap` for the next frame `decode` sees.
#[derive(Debug, Default)]
struct Sequences {
    last: HashMap<u64, u64>,
    ahead: usize,
    gap: Option<PredictionsWsError>,
    /// Each channel's subscription on this connection, from the `subscribed` answers.
    sids: BTreeMap<Channel, u64>,
    /// How many `subscribed` answers the `subscribe` in flight expects, one for each channel,
    /// and how many of them have been read: `request` ends on the last. Zero when no
    /// `subscribe` is in flight.
    awaiting: usize,
    seen: usize,
    /// Subscriptions whose frames are not checked: the order book's after a gap.
    unanchored: BTreeSet<u64>,
    /// The commands the protocol sent outside the kit's `request`, by id.
    follow_ups: BTreeMap<u64, FollowUp>,
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

    /// Whether `sid` is the order book's subscription on this connection.
    fn is_book(&self, sid: u64) -> bool {
        self.sids.get(&Channel::OrderbookDelta) == Some(&sid)
    }

    /// Check one frame's numbering, if it has any: a snapshot answering the re-snapshot
    /// re-anchors its subscription, a frame of an unanchored subscription is not checked,
    /// and any other must be the previous plus one. A gap on the order book's subscription
    /// leaves it unanchored, so the frames that follow are not each reported as one.
    fn observe(&mut self, peek: &Peek) -> Result<(), PredictionsWsError> {
        let Some((sid, seq)) = peek.sequence() else {
            return Ok(());
        };
        let answers_resync = peek.kind.as_deref() == Some("orderbook_snapshot")
            && peek
                .id
                .is_some_and(|id| self.follow_ups.get(&id) == Some(&FollowUp::Resync));
        if answers_resync {
            self.last.insert(sid, seq);
            self.unanchored.remove(&sid);
            return Ok(());
        }
        if self.unanchored.contains(&sid) {
            return Ok(());
        }
        let checked = self.advance(sid, seq);
        if checked.is_err() && self.is_book(sid) {
            self.unanchored.insert(sid);
        }
        checked
    }

    /// Record the subscription a `subscribed` frame gives its channel, if it reads.
    fn record_sid(&mut self, text: &str) {
        if let Ok(frame) = serde_json::from_str::<frames::Subscribed>(text) {
            if let Ok(channel) = frame.msg.channel.parse::<Channel>() {
                self.sids.insert(channel, frame.msg.sid);
            }
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

/// The correlation of a request: the frame answering command `id`, and the bookkeeping of
/// every frame read meanwhile.
///
/// A `subscribe` is answered by one `subscribed` for each channel, so the request ends on the
/// last of them, or at once on an error or an answer that does not read, and each `subscribed`
/// gives its channel its `sid` whether or not it is the one awaited. A frame that answers
/// another command is dropped by the kit, after it has counted here; a frame that answers none,
/// or one the protocol sent itself, is kept for the stream, and counted in `ahead` if
/// sequenced.
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
    if let Err(gap) = sequences.observe(&peek) {
        if sequences.gap.is_none() {
            sequences.gap = Some(gap);
        }
    }
    let kind = peek.kind.as_deref().unwrap_or_default();
    if kind == "subscribed" {
        sequences.record_sid(text.as_str());
        // Part of the answer awaited: this command's, or, while a `subscribe` waits, one that
        // carries no id.
        let ours = peek.id == Some(id)
            || (sequences.awaiting > 0 && peek.id.is_none_or(|other| other == 0));
        if !ours {
            return Correlated::Uncorrelated;
        }
        let reading = answered(kind, text.as_str());
        sequences.seen += 1;
        if reading.is_ok() && sequences.seen < sequences.awaiting {
            return Correlated::Push;
        }
        return Correlated::Answer(reading);
    }
    let follow_up = peek
        .id
        .is_some_and(|other| sequences.follow_ups.contains_key(&other));
    if peek.answers_a_command() && !follow_up {
        if peek.id != Some(id) {
            return Correlated::Uncorrelated;
        }
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
        channels: Vec<Channel>,
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
            Self::Subscribe { channels, tickers } => {
                let mut params = json!({
                    "channels": channels.iter().map(|channel| channel.as_str()).collect::<Vec<_>>(),
                    "market_tickers": tickers,
                });
                // The flag is the order book's alone, as the mirror words it.
                if channels.contains(&Channel::OrderbookDelta) {
                    params["use_yes_price"] = json!(true);
                }
                json!({"id": id, "cmd": "subscribe", "params": params})
            }
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
    sequences.awaiting = match command {
        Command::Subscribe { channels, .. } => channels.len(),
        _ => 0,
    };
    sequences.seen = 0;
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

    /// One `subscribe` naming every channel for every ticker; nothing for none, since Kalshi
    /// refuses a `ticker` or `trade` subscribe with no markets (code 14). A refusal on a
    /// fresh connection fails the attempt, which recovery decides on: Kalshi's error frame,
    /// or an answer that does not decode, is noted first, so that `recover` stops on a code
    /// that would not reconnect and on an answer that would never decode.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &BTreeSet<Ticker>,
    ) -> Result<(), PredictionsWsError> {
        self.tickers = membership.clone();
        if membership.is_empty() {
            return Ok(());
        }
        let command = Command::Subscribe {
            channels: self.channel_list(),
            tickers: names(membership.iter()),
        };
        match send(&mut self.sequences, connection, &command).await {
            Ok(_) => Ok(()),
            Err(err) => {
                self.replay_refused = matches!(
                    err,
                    PredictionsWsError::Server { .. } | PredictionsWsError::Json { .. }
                );
                Err(err)
            }
        }
    }

    /// A change on the live connection, recorded before it is sent, one command for each
    /// subscription. Kalshi refusing one, or answering with a frame that does not read,
    /// undoes the change and is the caller's error. When the refusal follows a command of the
    /// same change that Kalshi accepted, its subscriptions have changed and the membership
    /// has not, so the connection is replaced as well: the next frame read ends it, and the
    /// replay brings the subscriptions back to the membership. A refusal of the first command
    /// leaves Kalshi's subscriptions as they were, so the connection stays. The connection
    /// failing under a change leaves it recorded and answers `Ok`, since the next read ends
    /// the connection and the replay applies it.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut BTreeSet<Ticker>,
        change: Change,
    ) -> Result<(), PredictionsWsError> {
        let result = self.apply_change(connection, membership, change).await;
        self.tickers = membership.clone();
        result
    }

    /// A change during an outage is recorded and answered at once; the replay applies it. A
    /// snapshot request is not recorded, since the replay brings a fresh snapshot of every
    /// market, but it is checked as it would be on a live connection.
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
            Change::Snapshot(tickers) => {
                return Offline::Recorded(self.snapshot_names(membership, &tickers).map(|_| ()));
            }
        }
        self.tickers = membership.clone();
        Offline::Recorded(Ok(()))
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Event, PredictionsWsError> {
        // A connection whose subscriptions a refused change left out of step reads nothing
        // more: this frame, and any it kept, belong to subscriptions the replay replaces.
        if let Some(refusal) = &self.change_refused {
            return Decoded::Error(refusal.error());
        }
        if let Some(late) = self.snapshot_overdue() {
            return Decoded::Error(late);
        }
        let mut resyncing = self.lead.take();
        // A gap an answer showed is raised by the first frame after it, whatever its kind:
        // the re-snapshot starts, and its marker goes ahead of that frame's own event.
        if let Some(gap) = self.sequences.gap.take() {
            match self.gapped(gap) {
                Ok(event) => resyncing = Some(event),
                Err(err) => {
                    self.lead = resyncing;
                    return Decoded::Error(err);
                }
            }
        }
        let decoded = self.decode_frame(frame);
        let Some(resyncing) = resyncing else {
            return decoded;
        };
        match decoded {
            Decoded::Event(event) => Decoded::Events(vec![resyncing, event]),
            Decoded::Events(mut events) => {
                events.insert(0, resyncing);
                Decoded::Events(events)
            }
            // The error is the frame's to report now; the marker goes ahead of the next.
            Decoded::Error(err) => {
                self.lead = Some(resyncing);
                Decoded::Error(err)
            }
            _ => Decoded::Event(resyncing),
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

    /// The `get_snapshot` a gap queued, sent after the decode that found it, one frame for
    /// each call.
    fn follow_up(&mut self) -> Option<Message> {
        self.queued
            .pop_front()
            .map(|frame| Message::Text(frame.into()))
    }

    fn is_retriable(err: &PredictionsWsError) -> bool {
        err.is_retriable()
    }

    /// Kalshi's recovery: a frame that does not decode is skipped on a live connection, stops
    /// the stream when it was the answer to the replay's `subscribe`, and reconnects when it
    /// is the refusal of a command that came after another command of its change was
    /// accepted; an error frame's code decides (9 stops the stream, 10, 17, 18, 25 and 27
    /// reconnect, any other is skipped on a live connection, stops the stream when it refused
    /// the replay's `subscribe`, and reconnects when it answers the re-snapshot's command or
    /// is such a refusal of a change); a gap reconnects, and so does a snapshot that never
    /// came; a connection used after it closed reconnects (D19); and anything else reconnects
    /// exactly when its class is retriable, so a refused handshake's 401 or 403 stops, and a
    /// 429 or 5xx retries. No code here matches over `tungstenite::Error`: the socket table
    /// decides the class.
    ///
    /// A skipped error during an attempt reads as a reconnect, the kit's rule for a failed
    /// attempt, which is why a refused replay has its own row: it would otherwise
    /// reconnect for ever, refused each time, and so would an answer that never decodes.
    fn recover(&self, err: &PredictionsWsError) -> Recovery {
        match err {
            PredictionsWsError::Json { .. } if self.replay_refused => Recovery::Stop,
            PredictionsWsError::Json { .. } if self.change_refused.is_some() => Recovery::Reconnect,
            PredictionsWsError::Json { .. } => Recovery::Skip,
            PredictionsWsError::Server { code, .. } => match code {
                9 => Recovery::Stop,
                10 | 17 | 18 | 25 | 27 => Recovery::Reconnect,
                _ if self.replay_refused => Recovery::Stop,
                _ if self.snapshot_refused || self.change_refused.is_some() => Recovery::Reconnect,
                _ => Recovery::Skip,
            },
            PredictionsWsError::SequenceGap { .. } | PredictionsWsError::SnapshotTimeout { .. } => {
                Recovery::Reconnect
            }
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

    /// Forget what belongs to the previous connection: its subscriptions and numbering, a
    /// re-snapshot in flight and the request it queued. The markets are the membership's and
    /// stay.
    fn reset(&mut self) {
        self.sequences = Sequences::default();
        self.queued.clear();
        self.next_follow_up = FOLLOW_UP_ID_BASE;
        self.resync = None;
        self.lead = None;
        self.snapshot_refused = false;
        self.replay_refused = false;
        self.change_refused = None;
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(EVENT_QUEUE)
    }
}

/// The membership calls of a supervised Kalshi socket, on its `MembershipHandle`.
///
/// Each waits for the task to take the change and for Kalshi's answer to each command it
/// sends, one for each channel's subscription. During an outage the change is recorded and
/// answered at once, and replayed on the next connection; if Kalshi refuses that replay with
/// a code that does not reconnect, or answers it with a frame that does not decode, the
/// stream yields the error and ends, since the caller was told `Ok`. The task takes no change
/// while the consumer's queue is full (1024 unread events), so a call made from the task that
/// reads the stream can wait for ever then: make changes from another task, or keep reading
/// while one is pending.
pub trait PredictionsMembership {
    /// Subscribe to more markets, on every channel the socket was built with. The first add
    /// with nothing subscribed sends a `subscribe`; later ones go as `update_subscription`
    /// on each live subscription. A market already subscribed is skipped.
    ///
    /// Fails with [`PredictionsWsError::Server`] when Kalshi refuses the change, which
    /// leaves the membership as it was; with a stall when Kalshi left it unanswered for
    /// the staleness window (the connection is then replaced); or once the stream has
    /// stopped. A change sends one command for each channel, so Kalshi can refuse one after
    /// accepting another: the change fails with Kalshi's error all the same, and the
    /// connection is replaced, since its subscriptions no longer match the membership. The
    /// stream yields `Event::Disconnected` with that error as the reason, then
    /// `Event::Reconnected`, and the replay brings the subscriptions back to the membership
    /// (an error of code 9 stops the stream instead, as it does wherever it arrives). A
    /// refusal of the first command keeps the connection.
    fn subscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send;

    /// Unsubscribe from markets, failing as [`subscribe`](Self::subscribe) does.
    /// Unsubscribing every market sends an `unsubscribe` for each subscription and keeps the
    /// connection, which keeps pinging; a reconnect then replays nothing, since Kalshi
    /// refuses a `ticker` or `trade` subscribe with no markets (code 14).
    fn unsubscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send;

    /// Ask Kalshi for a fresh snapshot of the books of markets already subscribed, without
    /// changing the membership. The snapshots arrive on the stream as `Event::Snapshot`s;
    /// this answers `Ok` once the request is sent, or at once during an outage, when the
    /// replay brings a snapshot of every market anyway.
    ///
    /// Fails with [`PredictionsWsError::InvalidRequest`], sending nothing, for a market that
    /// is not subscribed and on a socket built without the order-book channel.
    fn snapshot(
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

    fn snapshot(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send {
        self.apply(Change::Snapshot(tickers.into_iter().collect()))
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
            channels: vec![Channel::OrderbookDelta],
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
        assert!(protocol.sequences.sids.is_empty());
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
    #[test]
    fn channels_are_kalshis_wire_names_and_sort_in_channel_order() {
        assert_eq!(
            Channel::ALL,
            [Channel::OrderbookDelta, Channel::Ticker, Channel::Trade]
        );
        assert_eq!(Channel::OrderbookDelta.as_str(), "orderbook_delta");
        assert_eq!(Channel::Ticker.as_str(), "ticker");
        assert_eq!(Channel::Trade.as_str(), "trade");
        // The order updates and unsubscribes go out in.
        let mut shuffled = [Channel::Trade, Channel::OrderbookDelta, Channel::Ticker];
        shuffled.sort();
        assert_eq!(shuffled, Channel::ALL);
        assert_eq!("trade".parse::<Channel>().unwrap(), Channel::Trade);
        assert!("fill".parse::<Channel>().is_err());
        assert_eq!(
            serde_json::to_string(&Channel::OrderbookDelta).unwrap(),
            r#""orderbook_delta""#
        );
    }
    // The mirror's own examples for the two channels (`docs/specs/kalshi/asyncapi.yaml`).
    const TICKER: &str = r#"{"type":"ticker","sending_ts_ms":1669149841123,"sid":11,"msg":{"market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","market_ticker":"FED-23DEC-T3.00","price_dollars":"0.4800","yes_bid_dollars":"0.4500","yes_ask_dollars":"0.5300","volume_fp":"33896.00","open_interest_fp":"20422.00","dollar_volume":16948,"dollar_open_interest":10211,"yes_bid_size_fp":"300.00","yes_ask_size_fp":"150.00","last_trade_size_fp":"25.00","ts":1669149841,"ts_ms":1669149841000,"time":"2022-11-22T20:44:01Z"}}"#;
    const TRADE: &str = r#"{"type":"trade","sending_ts_ms":1669149841123,"sid":11,"seq":2,"msg":{"trade_id":"d91bc706-ee49-470d-82d8-11418bda6fed","market_ticker":"HIGHNY-22DEC23-B53.5","yes_price_dollars":"0.3600","no_price_dollars":"0.6400","count_fp":"136.00","taker_side":"no","taker_outcome_side":"no","taker_book_side":"ask","is_block_trade":false,"ts":1669149841,"ts_ms":1669149841000}}"#;

    fn protocol_with(channels: impl IntoIterator<Item = Channel>) -> PredictionsProtocol {
        protocol().configured(channels.into_iter().collect(), Duration::from_millis(300))
    }

    /// A protocol subscribed to the order book as subscription 1, holding A-1 and B-2.
    fn on_the_book() -> PredictionsProtocol {
        let mut protocol = protocol();
        protocol.sequences.sids.insert(Channel::OrderbookDelta, 1);
        protocol.tickers = ["A-1", "B-2"]
            .iter()
            .map(|name| Ticker::new(name).unwrap())
            .collect();
        protocol
    }

    fn delta_of(market: &str, sid: u64, seq: u64) -> String {
        json!({"type": "orderbook_delta", "sid": sid, "seq": seq, "msg": {
            "market_ticker": market, "market_id": "id",
            "price_dollars": "0.5000", "delta_fp": "1.00", "side": "yes",
        }})
        .to_string()
    }

    /// A snapshot answering the `get_snapshot` whose id is `id`.
    fn snapshot_answer(id: u64, market: &str, sid: u64, seq: u64) -> String {
        json!({"id": id, "type": "orderbook_snapshot", "sid": sid, "seq": seq, "msg": {
            "market_ticker": market, "market_id": "id",
            "yes_dollars_fp": [["0.1000", "1.00"]], "no_dollars_fp": [["0.9000", "2.00"]],
        }})
        .to_string()
    }

    fn trade_on(sid: u64, seq: u64) -> String {
        let mut frame: Value = serde_json::from_str(TRADE).unwrap();
        frame["sid"] = json!(sid);
        frame["seq"] = json!(seq);
        frame.to_string()
    }

    /// The request the protocol queued for `follow_up`, and its id.
    fn queued_request(protocol: &mut PredictionsProtocol) -> (u64, Value) {
        let Some(Message::Text(frame)) = protocol.follow_up() else {
            panic!("no request was queued");
        };
        let request: Value = serde_json::from_str(frame.as_str()).unwrap();
        (request["id"].as_u64().unwrap(), request)
    }

    /// Reads a snapshot (seq 1) and then a delta that skips a number: the gap, and the
    /// re-snapshot it starts. Returns the id of the request that asks for it.
    fn gap_on_the_book(protocol: &mut PredictionsProtocol) -> u64 {
        decoded(protocol, &snapshot_frame(1, 1));
        match decoded(protocol, &delta_of("A-1", 1, 3)) {
            Decoded::Event(Event::Resyncing { .. }) => {}
            other => panic!("expected the re-snapshot to start, got {other:?}"),
        }
        queued_request(protocol).0
    }

    #[test]
    fn a_subscribe_names_every_channel_and_asks_for_yes_pricing_only_of_the_order_book() {
        let tickers = vec!["A-1".to_owned()];
        let all = Command::Subscribe {
            channels: Channel::ALL.to_vec(),
            tickers: tickers.clone(),
        };
        assert_eq!(
            serde_json::from_str::<Value>(&all.frame(7)).unwrap(),
            json!({"id": 7, "cmd": "subscribe", "params": {
                "channels": ["orderbook_delta", "ticker", "trade"],
                "market_tickers": ["A-1"],
                "use_yes_price": true,
            }})
        );
        // The flag is the order book's alone: the mirror calls it "orderbook channel only".
        let quotes = Command::Subscribe {
            channels: vec![Channel::Ticker, Channel::Trade],
            tickers,
        };
        let parsed: Value = serde_json::from_str(&quotes.frame(8)).unwrap();
        assert_eq!(parsed["params"]["channels"], json!(["ticker", "trade"]));
        assert!(parsed["params"].get("use_yes_price").is_none(), "{parsed}");
        let snapshot = Command::Update {
            sid: 4,
            action: "get_snapshot",
            tickers: vec!["A-1".to_owned()],
        };
        assert_eq!(
            serde_json::from_str::<Value>(&snapshot.frame(9)).unwrap(),
            json!({"id": 9, "cmd": "update_subscription", "params": {
                "sid": 4, "action": "get_snapshot", "market_tickers": ["A-1"],
            }})
        );
    }

    fn subscribed_frame(id: u64, channel: &str, sid: u64) -> String {
        json!({"id": id, "type": "subscribed", "msg": {"channel": channel, "sid": sid}}).to_string()
    }

    #[test]
    fn a_subscribe_is_answered_once_every_channel_has_answered() {
        // Bug: answering at the first `subscribed`, which leaves the other channels' `sid`s
        // unknown until a later frame happens to carry them, and a change made at once
        // then goes to a subscription that is not there.
        let mut sequences = Sequences {
            awaiting: 3,
            ..Sequences::default()
        };
        assert!(matches!(
            answer(
                &mut sequences,
                4,
                &subscribed_frame(4, "orderbook_delta", 1)
            ),
            Correlated::Push
        ));
        assert!(matches!(
            answer(&mut sequences, 4, &subscribed_frame(4, "ticker", 2)),
            Correlated::Push
        ));
        assert!(matches!(
            answer(&mut sequences, 4, &subscribed_frame(4, "trade", 3)),
            Correlated::Answer(Ok(Answered::Subscribed(3)))
        ));
        assert_eq!(
            sequences.sids,
            BTreeMap::from([
                (Channel::OrderbookDelta, 1),
                (Channel::Ticker, 2),
                (Channel::Trade, 3)
            ])
        );
    }

    #[test]
    fn an_error_or_an_undecodable_answer_ends_a_subscribes_wait_at_once() {
        // Bug: waiting for three answers that will never come, until the staleness window
        // replaces the connection, when the first frame already said it was refused.
        let mut sequences = Sequences {
            awaiting: 3,
            ..Sequences::default()
        };
        assert!(matches!(
            answer(&mut sequences, 123, ERROR),
            Correlated::Answer(Err(PredictionsWsError::Server { code: 7, .. }))
        ));
        let no_sid = r#"{"id":4,"type":"subscribed","msg":{"channel":"ticker"}}"#;
        assert!(matches!(
            answer(&mut sequences, 4, no_sid),
            Correlated::Answer(Err(PredictionsWsError::Json { .. }))
        ));
    }

    #[test]
    fn a_subscribed_answer_read_during_a_later_request_still_gives_its_channel_its_sid() {
        // Bug: the kit drops an answer to a command that is not the one awaited, so a
        // `subscribed` read while another request waits would never reach `decode`.
        let mut sequences = Sequences::default();
        assert!(matches!(
            answer(&mut sequences, 9, &subscribed_frame(4, "trade", 3)),
            Correlated::Uncorrelated
        ));
        assert_eq!(sequences.sids, BTreeMap::from([(Channel::Trade, 3)]));
    }

    #[test]
    fn a_ticker_or_trade_frame_is_carried_only_on_a_channel_that_was_asked_for() {
        // Bug: yielding a frame of a channel nobody subscribed, or dropping one that was.
        let mut book_only = protocol();
        assert!(matches!(decoded(&mut book_only, TICKER), Decoded::Nothing));
        assert!(matches!(decoded(&mut book_only, TRADE), Decoded::Nothing));
        let mut all = protocol_with(Channel::ALL.iter().copied());
        let Decoded::Event(Event::Ticker(update)) = decoded(&mut all, TICKER) else {
            panic!("not a ticker update");
        };
        let dec = |text: &str| text.parse::<Decimal>().unwrap();
        assert_eq!(update.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(update.price, dec("0.4800"));
        assert_eq!(update.yes_bid, dec("0.4500"));
        assert_eq!(update.yes_ask, dec("0.5300"));
        assert_eq!(update.yes_bid_size, dec("300.00"));
        assert_eq!(update.yes_ask_size, dec("150.00"));
        assert_eq!(update.volume, dec("33896.00"));
        assert_eq!(update.open_interest, dec("20422.00"));
        assert_eq!(update.last_trade_size, dec("25.00"));
        assert_eq!(
            (update.dollar_volume, update.dollar_open_interest),
            (16_948, 10_211)
        );
        assert_eq!(update.ts, UnixMillis(1_669_149_841_000));
        let Decoded::Event(Event::Trade(trade)) = decoded(&mut all, TRADE) else {
            panic!("not a trade");
        };
        assert_eq!(trade.trade_id, "d91bc706-ee49-470d-82d8-11418bda6fed");
        assert_eq!(trade.yes_price, dec("0.3600"));
        assert_eq!(trade.no_price, dec("0.6400"));
        assert_eq!(trade.count, dec("136.00"));
        assert_eq!(
            trade.taker_outcome_side,
            crate::predictions::types::OutcomeSide::No
        );
        assert_eq!(trade.taker_book_side, BookSide::Ask);
    }

    #[test]
    fn the_captured_ticker_and_trade_decode_to_the_wire_values() {
        let mut protocol = protocol_with(Channel::ALL.iter().copied());
        let dec = |msg: &Value, key: &str| msg[key].as_str().unwrap().parse::<Decimal>().unwrap();

        let wire: Value = serde_json::from_str(&fixture("ticker")).unwrap();
        let Decoded::Event(Event::Ticker(update)) = decoded(&mut protocol, &fixture("ticker"))
        else {
            panic!("the fixture is not a ticker update");
        };
        let msg = &wire["msg"];
        assert_eq!(update.market_ticker, msg["market_ticker"].as_str().unwrap());
        assert_eq!(update.price, dec(msg, "price_dollars"));
        assert_eq!(update.yes_bid, dec(msg, "yes_bid_dollars"));
        assert_eq!(update.yes_ask, dec(msg, "yes_ask_dollars"));
        assert_eq!(update.volume, dec(msg, "volume_fp"));
        assert_eq!(update.open_interest, dec(msg, "open_interest_fp"));
        assert_eq!(update.yes_bid_size, dec(msg, "yes_bid_size_fp"));
        assert_eq!(update.yes_ask_size, dec(msg, "yes_ask_size_fp"));
        assert_eq!(update.last_trade_size, dec(msg, "last_trade_size_fp"));
        assert_eq!(update.dollar_volume, msg["dollar_volume"].as_i64().unwrap());
        assert_eq!(update.ts, UnixMillis(msg["ts_ms"].as_u64().unwrap()));

        let wire: Value = serde_json::from_str(&fixture("trade")).unwrap();
        let Decoded::Event(Event::Trade(trade)) = decoded(&mut protocol, &fixture("trade")) else {
            panic!("the fixture is not a trade");
        };
        let msg = &wire["msg"];
        assert_eq!(trade.trade_id, msg["trade_id"].as_str().unwrap());
        assert_eq!(trade.market_ticker, msg["market_ticker"].as_str().unwrap());
        assert_eq!(trade.yes_price, dec(msg, "yes_price_dollars"));
        assert_eq!(trade.no_price, dec(msg, "no_price_dollars"));
        assert_eq!(trade.count, dec(msg, "count_fp"));
        assert_eq!(
            trade.taker_outcome_side.as_str(),
            msg["taker_outcome_side"].as_str().unwrap()
        );
        assert_eq!(
            trade.taker_book_side.as_str(),
            msg["taker_book_side"].as_str().unwrap()
        );
        assert_eq!(
            trade.is_block_trade,
            msg["is_block_trade"].as_bool().unwrap()
        );
        assert_eq!(trade.ts, UnixMillis(msg["ts_ms"].as_u64().unwrap()));
    }

    #[test]
    fn a_ticker_frame_has_no_seq_and_is_never_a_gap() {
        let mut protocol = protocol_with([Channel::OrderbookDelta, Channel::Ticker]);
        for _ in 0..3 {
            assert!(matches!(
                decoded(&mut protocol, TICKER),
                Decoded::Event(Event::Ticker(_))
            ));
        }
    }

    #[test]
    fn a_gap_on_the_trade_subscription_reconnects_and_never_resnapshots() {
        // Bug: re-snapshotting the order book for a lost trade, which repairs nothing: the
        // trade channel has no snapshot to ask for.
        let mut protocol = protocol_with([Channel::OrderbookDelta, Channel::Trade]);
        protocol.sequences.sids.insert(Channel::OrderbookDelta, 1);
        protocol.sequences.sids.insert(Channel::Trade, 2);
        protocol.tickers = [Ticker::new("A-1").unwrap()].into_iter().collect();
        assert!(matches!(
            decoded(&mut protocol, &trade_on(2, 1)),
            Decoded::Event(Event::Trade(_))
        ));
        let Decoded::Error(err) = decoded(&mut protocol, &trade_on(2, 3)) else {
            panic!("the gap is an error");
        };
        assert!(
            matches!(
                err,
                PredictionsWsError::SequenceGap {
                    sid: 2,
                    expected: 2,
                    received: 3
                }
            ),
            "{err:?}"
        );
        assert_eq!(protocol.recover(&err), Recovery::Reconnect);
        assert!(protocol.follow_up().is_none());
        assert!(protocol.resync.is_none());
    }

    #[test]
    fn a_gap_on_the_order_book_resnapshots_the_whole_membership() {
        let mut protocol = on_the_book();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        match decoded(&mut protocol, &delta_of("A-1", 1, 3)) {
            Decoded::Event(Event::Resyncing { tickers }) => assert_eq!(tickers, ["A-1", "B-2"]),
            other => panic!("expected the re-snapshot to start, got {other:?}"),
        }
        let (id, request) = queued_request(&mut protocol);
        // The protocol's own id range, clear of the kit's, which count from 1.
        assert!(id >= FOLLOW_UP_ID_BASE, "{id}");
        assert_eq!(
            request,
            json!({"id": id, "cmd": "update_subscription", "params": {
                "sid": 1, "action": "get_snapshot", "market_tickers": ["A-1", "B-2"],
            }})
        );
        // One frame per call, and only the one was queued.
        assert!(protocol.follow_up().is_none());
    }

    #[test]
    fn deltas_of_a_market_awaiting_its_snapshot_are_dropped_and_resume_per_market() {
        // Bug: yielding a delta that applies to a book the consumer has dropped, or holding
        // back a market whose snapshot has already arrived.
        let mut protocol = on_the_book();
        let id = gap_on_the_book(&mut protocol);
        // Both markets await a snapshot, so their deltas go, whatever their numbers.
        assert!(matches!(
            decoded(&mut protocol, &delta_of("A-1", 1, 4)),
            Decoded::Nothing
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("B-2", 1, 5)),
            Decoded::Nothing
        ));
        // A's snapshot answers and re-anchors the numbering at 10: A resumes, B still waits.
        assert!(matches!(
            decoded(&mut protocol, &snapshot_answer(id, "A-1", 1, 10)),
            Decoded::Event(Event::Snapshot(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("A-1", 1, 11)),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("B-2", 1, 12)),
            Decoded::Nothing
        ));
        assert!(matches!(
            decoded(&mut protocol, &snapshot_answer(id, "B-2", 1, 13)),
            Decoded::Event(Event::Snapshot(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("B-2", 1, 14)),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(protocol.resync.is_none());
        assert!(protocol.sequences.follow_ups.is_empty());
    }

    #[test]
    fn a_second_gap_while_snapshots_are_awaited_reconnects() {
        // Bug: starting a second re-snapshot over the first, which never finishes under a
        // feed that keeps losing messages.
        let mut protocol = on_the_book();
        let id = gap_on_the_book(&mut protocol);
        decoded(&mut protocol, &snapshot_answer(id, "A-1", 1, 10));
        let Decoded::Error(err) = decoded(&mut protocol, &delta_of("A-1", 1, 13)) else {
            panic!("the second gap is an error");
        };
        assert!(
            matches!(
                err,
                PredictionsWsError::SequenceGap {
                    sid: 1,
                    expected: 11,
                    received: 13
                }
            ),
            "{err:?}"
        );
        assert_eq!(protocol.recover(&err), Recovery::Reconnect);
    }

    #[test]
    fn a_gap_once_every_snapshot_is_in_starts_another_resnapshot() {
        let mut protocol = on_the_book();
        protocol.tickers = [Ticker::new("A-1").unwrap()].into_iter().collect();
        let first = gap_on_the_book(&mut protocol);
        decoded(&mut protocol, &snapshot_answer(first, "A-1", 1, 10));
        assert!(protocol.resync.is_none());
        match decoded(&mut protocol, &delta_of("A-1", 1, 12)) {
            Decoded::Event(Event::Resyncing { tickers }) => assert_eq!(tickers, ["A-1"]),
            other => panic!("expected another re-snapshot, got {other:?}"),
        }
        let (second, _) = queued_request(&mut protocol);
        assert_ne!(first, second);
    }

    #[tokio::test(start_paused = true)]
    async fn snapshots_that_do_not_arrive_are_a_timeout_on_the_first_frame_past_the_window() {
        // Bug: waiting for ever for a snapshot Kalshi never sends, with the book dropped and
        // every delta of it withheld.
        let mut protocol = on_the_book();
        protocol.stale_after = Duration::from_millis(300);
        gap_on_the_book(&mut protocol);
        let pong = || Message::Pong(Vec::new().into());
        tokio::time::advance(Duration::from_millis(299)).await;
        assert!(matches!(protocol.decode(&pong()), Decoded::Nothing));
        tokio::time::advance(Duration::from_millis(1)).await;
        let Decoded::Error(err) = protocol.decode(&pong()) else {
            panic!("the window has passed");
        };
        match &err {
            PredictionsWsError::SnapshotTimeout { sid, waited } => {
                assert_eq!(*sid, 1);
                assert!(*waited >= Duration::from_millis(300), "{waited:?}");
            }
            other => panic!("expected the timeout, got {other:?}"),
        }
        assert_eq!(protocol.recover(&err), Recovery::Reconnect);
    }

    #[test]
    fn an_error_that_answers_the_resnapshot_reconnects_where_the_same_code_is_otherwise_skipped() {
        // Bug: skipping the refusal as an ordinary code-11 frame, which leaves the books
        // dropped and the deltas withheld for ever.
        let mut protocol = on_the_book();
        let id = gap_on_the_book(&mut protocol);
        let refusal =
            json!({"id": id, "type": "error", "msg": {"code": 11, "msg": "Invalid parameter"}})
                .to_string();
        let Decoded::Error(err) = decoded(&mut protocol, &refusal) else {
            panic!("the refusal is an error");
        };
        assert!(matches!(&err, PredictionsWsError::Server { code: 11, .. }));
        assert_eq!(protocol.recover(&err), Recovery::Reconnect);
        // The same code on a subscription is the caller's.
        let mut other = on_the_book();
        let scoped =
            r#"{"type":"error","sid":1,"seq":1,"msg":{"code":11,"msg":"Invalid parameter"}}"#;
        let Decoded::Error(err) = decoded(&mut other, scoped) else {
            panic!("a scoped error is an error");
        };
        assert_eq!(other.recover(&err), Recovery::Skip);
        // A code that stops the stream still does.
        let mut auth = on_the_book();
        let id = gap_on_the_book(&mut auth);
        let denied =
            json!({"id": id, "type": "error", "msg": {"code": 9, "msg": "Authentication required"}})
                .to_string();
        let Decoded::Error(err) = decoded(&mut auth, &denied) else {
            panic!("the refusal is an error");
        };
        assert_eq!(auth.recover(&err), Recovery::Stop);
    }

    #[test]
    fn a_frame_that_answers_a_resnapshot_is_kept_for_the_stream_whichever_request_reads_it() {
        // Bug: the kit drops an answer to a command that is not the one awaited, so an
        // error answering the re-snapshot, read while a membership change waits, would
        // vanish and the books would stay dropped.
        let mut protocol = on_the_book();
        let id = gap_on_the_book(&mut protocol);
        let refusal =
            json!({"id": id, "type": "error", "msg": {"code": 11, "msg": "x"}}).to_string();
        assert!(matches!(
            answer(&mut protocol.sequences, 5, &refusal),
            Correlated::Push
        ));
        // A snapshot answering it re-anchors the numbering as the closure reads it, and is
        // counted so that `decode` does not check it a second time.
        let snapshot = snapshot_answer(id, "A-1", 1, 10);
        assert!(matches!(
            answer(&mut protocol.sequences, 5, &snapshot),
            Correlated::Push
        ));
        assert_eq!(protocol.sequences.last.get(&1), Some(&10));
        assert!(protocol.sequences.unanchored.is_empty());
        assert_eq!(protocol.sequences.ahead, 1);
        assert!(matches!(
            decoded(&mut protocol, &snapshot),
            Decoded::Event(Event::Snapshot(_))
        ));
        assert_eq!(protocol.sequences.ahead, 0);
        assert!(protocol.sequences.gap.is_none());
    }

    #[test]
    fn a_gap_an_answer_showed_starts_the_resnapshot_ahead_of_the_frame_that_raised_it() {
        // Bug: dropping the frame that raised the gap (a ticker update or a trade is not the
        // book's to lose), or yielding it ahead of the marker.
        let mut protocol = protocol_with([Channel::OrderbookDelta, Channel::Ticker]);
        protocol.sequences.sids.insert(Channel::OrderbookDelta, 1);
        protocol.tickers = [Ticker::new("A-1").unwrap()].into_iter().collect();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        // The ok that answers a change skipped number 2: a gap behind it.
        assert!(matches!(
            correlate(&mut protocol.sequences, 5, &text(&ok_frame(5, 1, 3))),
            Correlated::Answer(Ok(Answered::Done))
        ));
        match decoded(&mut protocol, TICKER) {
            Decoded::Events(events) => assert!(
                matches!(&events[..], [Event::Resyncing { .. }, Event::Ticker(_)]),
                "{events:?}"
            ),
            other => panic!("expected the marker and the ticker update, got {other:?}"),
        }
        assert!(protocol.follow_up().is_some());
    }

    #[test]
    fn a_frame_that_does_not_decode_does_not_lose_the_resyncing_marker() {
        let mut protocol = on_the_book();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        assert!(matches!(
            correlate(&mut protocol.sequences, 5, &text(&ok_frame(5, 1, 3))),
            Correlated::Answer(Ok(Answered::Done))
        ));
        assert!(matches!(
            decoded(&mut protocol, "not json"),
            Decoded::Error(PredictionsWsError::Json { .. })
        ));
        // The marker leads the next frame instead, and is said once.
        let pong = || Message::Pong(Vec::new().into());
        assert!(matches!(
            protocol.decode(&pong()),
            Decoded::Event(Event::Resyncing { .. })
        ));
        assert!(matches!(protocol.decode(&pong()), Decoded::Nothing));
    }

    #[test]
    fn a_fresh_connection_forgets_the_resnapshot_and_the_request_it_queued() {
        let mut protocol = on_the_book();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        decoded(&mut protocol, &delta_of("A-1", 1, 3));
        protocol.reset();
        assert!(protocol.follow_up().is_none());
        assert!(protocol.resync.is_none());
        assert!(protocol.sequences.follow_ups.is_empty());
        // The markets are the membership's, and outlive the connection.
        assert_eq!(protocol.tickers.len(), 2);
    }

    #[test]
    fn dropping_a_market_stops_waiting_for_its_snapshot() {
        let mut protocol = on_the_book();
        gap_on_the_book(&mut protocol);
        protocol.forget_markets(&["A-1".to_owned()]);
        assert!(matches!(
            decoded(&mut protocol, &delta_of("A-1", 1, 4)),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("B-2", 1, 5)),
            Decoded::Nothing
        ));
        protocol.forget_markets(&["B-2".to_owned()]);
        assert!(protocol.resync.is_none());
        assert!(protocol.sequences.follow_ups.is_empty());
    }

    #[test]
    fn unsubscribing_a_subscription_forgets_its_numbering_and_any_resnapshot_on_it() {
        let mut protocol = on_the_book();
        gap_on_the_book(&mut protocol);
        // A `snapshot` call's request is of the same subscription.
        protocol
            .sequences
            .follow_ups
            .insert(7, FollowUp::Requested(1));
        protocol.accepted(&Command::Unsubscribe { sid: 1 });
        assert!(protocol.sequences.sids.is_empty());
        assert!(protocol.sequences.last.is_empty());
        assert!(protocol.sequences.unanchored.is_empty());
        assert!(protocol.resync.is_none());
        // Bug: leaving the re-snapshot's and the request's entries, which nothing removes.
        assert!(protocol.sequences.follow_ups.is_empty());
    }

    #[test]
    fn the_numbering_starts_afresh_when_no_snapshot_is_awaited_any_more() {
        // Bug: a gap leaves the order book's `sid` unanchored; when the markets awaited are
        // all removed, no snapshot will re-anchor it, and the `sid` stayed unchecked for the
        // rest of the connection.
        let mut protocol = on_the_book();
        gap_on_the_book(&mut protocol);
        // C is added, and A and B go before either snapshot arrives.
        protocol.tickers.insert(Ticker::new("C-3").unwrap());
        protocol.tickers.remove(&Ticker::new("A-1").unwrap());
        protocol.tickers.remove(&Ticker::new("B-2").unwrap());
        protocol.forget_markets(&["A-1".to_owned(), "B-2".to_owned()]);
        assert!(protocol.resync.is_none());
        assert!(protocol.sequences.unanchored.is_empty());
        // The next frame starts the count, and the one after it is checked against it.
        assert!(matches!(
            decoded(&mut protocol, &delta_of("C-3", 1, 40)),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("C-3", 1, 41)),
            Decoded::Event(Event::Delta(_))
        ));
        // A skipped number is a gap again: with no re-snapshot in flight, it starts one.
        match decoded(&mut protocol, &delta_of("C-3", 1, 43)) {
            Decoded::Event(Event::Resyncing { tickers }) => assert_eq!(tickers, ["C-3"]),
            other => panic!("expected the gap to start a re-snapshot, got {other:?}"),
        }
    }

    #[test]
    fn frames_before_the_first_answering_snapshot_are_not_checked() {
        // Bug: checking them, so that a delta numbered past the gap, which belongs to a
        // book about to be replaced, is a second gap and reconnects over the re-snapshot.
        let mut protocol = on_the_book();
        let id = gap_on_the_book(&mut protocol);
        // The gap was at 3: a delta at 7 is dropped, and no error.
        assert!(matches!(
            decoded(&mut protocol, &delta_of("A-1", 1, 7)),
            Decoded::Nothing
        ));
        assert!(protocol.resync.is_some());
        // The answering snapshot re-anchors, and what follows it is checked again.
        assert!(matches!(
            decoded(&mut protocol, &snapshot_answer(id, "A-1", 1, 10)),
            Decoded::Event(Event::Snapshot(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("A-1", 1, 11)),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_of("A-1", 1, 13)),
            Decoded::Error(PredictionsWsError::SequenceGap {
                sid: 1,
                expected: 12,
                received: 13
            })
        ));
    }

    #[test]
    fn a_snapshot_request_is_forgotten_once_it_is_answered_or_refused() {
        // Bug: an entry for every `snapshot` call, kept until the connection ends.
        let mut protocol = on_the_book();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        protocol
            .sequences
            .follow_ups
            .insert(7, FollowUp::Requested(2));
        // One snapshot of two is not the end of it.
        decoded(&mut protocol, &snapshot_answer(7, "A-1", 1, 2));
        assert_eq!(
            protocol.sequences.follow_ups.get(&7),
            Some(&FollowUp::Requested(1))
        );
        decoded(&mut protocol, &snapshot_answer(7, "B-2", 1, 3));
        assert!(protocol.sequences.follow_ups.is_empty());
        // An error answering it ends it too, and is the stream's.
        protocol
            .sequences
            .follow_ups
            .insert(8, FollowUp::Requested(2));
        decoded(&mut protocol, &snapshot_answer(8, "A-1", 1, 4));
        let refusal =
            json!({"id": 8, "type": "error", "msg": {"code": 11, "msg": "Invalid parameter"}})
                .to_string();
        assert!(matches!(
            decoded(&mut protocol, &refusal),
            Decoded::Error(PredictionsWsError::Server { code: 11, .. })
        ));
        assert!(protocol.sequences.follow_ups.is_empty());
        // A refusal is not the re-snapshot's: it is skipped, not a reason to reconnect.
        assert!(!protocol.snapshot_refused);
    }

    #[test]
    fn a_snapshot_request_is_checked_before_anything_is_sent() {
        let protocol = on_the_book();
        let membership: BTreeSet<Ticker> = protocol.tickers.clone();
        let t = |name: &str| Ticker::new(name).unwrap();
        // Duplicates are asked for once, in order.
        assert_eq!(
            protocol
                .snapshot_names(&membership, &[t("B-2"), t("A-1"), t("B-2")])
                .unwrap(),
            ["A-1", "B-2"]
        );
        let outside = protocol
            .snapshot_names(&membership, &[t("C-3")])
            .unwrap_err();
        assert!(
            matches!(&outside, PredictionsWsError::InvalidRequest { reason } if reason.contains("C-3")),
            "{outside:?}"
        );
        assert_eq!(outside.class(), polyoxide_venue::Class::InvalidRequest);
        // The `ticker` channel has no books to ask for.
        let quotes_only = protocol_with([Channel::Ticker]);
        let err = quotes_only
            .snapshot_names(&membership, &[t("A-1")])
            .unwrap_err();
        assert!(
            matches!(err, PredictionsWsError::InvalidRequest { .. }),
            "{err:?}"
        );
    }

    #[test]
    fn every_frame_of_the_trade_subscription_counts_answers_and_errors_included() {
        // Bug: counting only the pushes of a subscription, so the `ok`s and the error that
        // Kalshi numbers on the trade `sid` (the capture: 4 and 5, then an error at 6) read as a
        // skipped number on the next trade, and a refused or answered command reconnects.
        let ok = |id: u64, seq: u64| {
            json!({"type": "ok", "id": id, "sid": 3, "seq": seq,
                   "msg": {"market_tickers": ["A-1"]}})
            .to_string()
        };
        let refusal = fixture("error_snapshot_trade_sid");
        let wire: Value = serde_json::from_str(&refusal).unwrap();
        assert_eq!(
            (wire["sid"].as_u64(), wire["seq"].as_u64()),
            (Some(3), Some(6))
        );
        assert!(wire.get("id").is_none(), "{wire}");
        for through_a_request in [false, true] {
            let mut protocol = protocol_with([Channel::OrderbookDelta, Channel::Trade]);
            protocol.sequences.sids.insert(Channel::OrderbookDelta, 1);
            protocol.sequences.sids.insert(Channel::Trade, 3);
            protocol.tickers = [Ticker::new("A-1").unwrap()].into_iter().collect();
            assert!(matches!(
                decoded(&mut protocol, &trade_on(3, 3)),
                Decoded::Event(Event::Trade(_))
            ));
            // Number 4 answers the change a request awaits, number 5 one it does not await.
            assert!(matches!(
                answer(&mut protocol.sequences, 6, &ok(6, 4)),
                Correlated::Answer(Ok(Answered::Done))
            ));
            assert!(matches!(
                answer(&mut protocol.sequences, 9, &ok(8, 5)),
                Correlated::Uncorrelated
            ));
            // Number 6 is an error with no `id`: the stream's, whoever reads it first.
            if through_a_request {
                assert!(matches!(
                    answer(&mut protocol.sequences, 9, &refusal),
                    Correlated::Push
                ));
            }
            assert!(matches!(
                decoded(&mut protocol, &refusal),
                Decoded::Error(PredictionsWsError::Server { code: 13, .. })
            ));
            assert!(matches!(
                decoded(&mut protocol, &trade_on(3, 7)),
                Decoded::Event(Event::Trade(_))
            ));
            assert!(protocol.sequences.gap.is_none());
            assert_eq!(protocol.sequences.ahead, 0);
        }
    }

    #[test]
    fn a_refusal_partway_ends_the_connection_with_its_error_on_the_next_frame() {
        // Bug: a refusal after an accepted command is skipped like any error frame, so the
        // connection stays with subscriptions the membership no longer matches.
        let mut protocol = on_the_book();
        assert_eq!(protocol.recover(&server(11)), Recovery::Skip);
        protocol.change_refused = Refusal::of(&server(11));
        assert!(protocol.change_refused.is_some());
        // Any frame is replaced by the refusal, a snapshot of a market nobody holds included,
        // and nothing is queued or counted for it.
        match decoded(&mut protocol, &snapshot_frame(1, 1)) {
            Decoded::Error(PredictionsWsError::Server { code: 11, message }) => {
                assert_eq!(message, "a message");
            }
            other => panic!("expected the refusal, got {other:?}"),
        }
        assert!(protocol.sequences.last.is_empty());
        // The refusal reconnects where the same code is otherwise skipped; the codes that
        // stop the stream or reconnect do so as before.
        assert_eq!(protocol.recover(&server(11)), Recovery::Reconnect);
        assert_eq!(protocol.recover(&server(9)), Recovery::Stop);
        assert_eq!(protocol.recover(&server(17)), Recovery::Reconnect);
        // The next connection starts clean.
        protocol.reset();
        assert!(protocol.change_refused.is_none());
        assert_eq!(protocol.recover(&server(11)), Recovery::Skip);
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(1, 1)),
            Decoded::Event(Event::Snapshot(_))
        ));
    }

    #[test]
    fn an_error_frame_that_did_not_read_is_a_refusal_partway_all_the_same() {
        // Bug: keeping only the refusals that read, so a change answered by an error frame
        // that does not decode fails and leaves the connection out of step.
        let text = r#"{"type":"error"}"#;
        let source = serde_json::from_str::<frames::Rejected>(text).unwrap_err();
        let cause = source.to_string();
        let unread = PredictionsWsError::Json {
            text: text.to_owned(),
            source,
        };
        let refusal = Refusal::of(&unread).expect("an answer of Kalshi's");
        match refusal.error() {
            PredictionsWsError::Json { text: held, source } => {
                assert_eq!(held, text);
                assert_eq!(source.to_string(), cause);
            }
            other => panic!("expected the frame's decode error, got {other:?}"),
        }
        let mut protocol = on_the_book();
        assert_eq!(protocol.recover(&unread), Recovery::Skip);
        protocol.change_refused = Some(refusal);
        assert_eq!(protocol.recover(&unread), Recovery::Reconnect);
        // The connection failing under a command is not a refusal: the replay repairs it.
        let lost = PredictionsWsError::from(WsError::closed(None, ""));
        assert_eq!(Refusal::of(&lost), None);
    }

    #[test]
    fn a_numbering_a_snapshot_re_anchored_survives_the_last_awaited_market_leaving() {
        // Bug: clearing the count when the last market awaited is removed even though a
        // snapshot (A's) had already set it, so the next frame started the count afresh and a
        // skipped number on it went unchecked.
        let mut protocol = on_the_book();
        let id = gap_on_the_book(&mut protocol);
        // A's snapshot re-anchors the subscription at 10; B's is still awaited.
        assert!(matches!(
            decoded(&mut protocol, &snapshot_answer(id, "A-1", 1, 10)),
            Decoded::Event(Event::Snapshot(_))
        ));
        assert!(protocol.resync.is_some());
        assert!(protocol.sequences.unanchored.is_empty());
        // B leaves the membership: nothing is awaited any more.
        protocol.tickers.remove(&Ticker::new("B-2").unwrap());
        protocol.forget_markets(&["B-2".to_owned()]);
        assert!(protocol.resync.is_none());
        // The first frame after it skips 11: still a gap, against the snapshot's number.
        match decoded(&mut protocol, &delta_of("A-1", 1, 12)) {
            Decoded::Event(Event::Resyncing { tickers }) => assert_eq!(tickers, ["A-1"]),
            other => panic!("expected the gap to start a re-snapshot, got {other:?}"),
        }
    }
}
