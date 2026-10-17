# Epic 7: supervised CLOB sockets and normalized trading — design

Status: approved by the owner, 2026-10-17. This spec is the design record for the plan
`docs/plans/2026-10-17-epic-7-supervised-clob-sockets-and-trading.md`. It settles what the
acceptance criteria of `_bmad-output/planning-artifacts/epics.md` (Epic 7, Stories 7.1 to 7.5)
leave open, and says where it departs from them. Read it with those criteria and with the
architecture spine,
`_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-SPINE.md`
(AD-3, AD-5, AD-7, AD-11, AD-19, AD-20, AD-24), its amendments A4-4 (`spine-amendments/epic-4.md`)
and A6-1 (`spine-amendments/epic-6.md`), and CLAUDE.md. Where this spec and an acceptance
criterion disagree, this spec binds.

## 1. Scope, stage and sources

**1.1 Stage.** The workspace is in stage S3 (`[workspace.metadata.polyoxide] stage = "S3"`, CI's
removals job `api_removals.py check --baseline v0.40.0`). Every story adds public API and removes
none, so `docs/s3-removals.md` gains nothing. Adding a defaulted method to a public trait, a field
to a struct with private fields, a variant to a `#[non_exhaustive]` enum, a feature or a new item
are all additive. The run makes no version bump and does not change the stage.

**1.2 The plan's stories.**

| Plan story | Epic story | What |
|---|---|---|
| 1 | 7.1 (prerequisite) | The kit dials through the protocol, so clob's per-address dial (D20) runs under the `Supervisor` |
| 2 | 7.1 | The supervised CLOB market socket |
| 3 | 7.2 | The supervised CLOB user socket |
| 4 | 7.3 | The `Trading` trait in `polyoxide-venue` |
| 5 | 7.4 | `Trading` on the Polymarket CLOB |
| 6 | 7.1, 7.2, 7.4 | Live suites for the supervised sockets and `Trading` |
| 7 | 7.5 | The Kalshi trading mapping, the spine amendments, the release note and the planning records |

Stories 1 to 3 run in order (each builds on the last). Story 4 touches only `polyoxide-venue` and
the root manifest and may run beside Stories 1 to 3. Story 5 needs Stories 3 and 4, Story 6 needs
Stories 2, 3 and 5, and Story 7 runs last.

**1.3 Lane B's tag is a reference, not a source.** Stories 7.3 and 7.5 were built on 2026-10-12
on a parallel lane whose commits survive only as the local tag `lane-b-bundle-n` (eabcd1f, base
e114454), never merged. Its trait design was reviewed (one layer, 23 findings) and the owner made
its calls (N-D1 to N-D3). Epic 6 has since shipped a different key design (`KeySpace`, not the
tag's `Product`) and a different market-data surface, so the tag's `key.rs`, `market.rs`,
`records.rs` and their tests are superseded and are never taken. A drafter may read the tag's
trading files with `git show lane-b-bundle-n:<path>` as the design this spec adopts (§6), and
Story 7 ports its Kalshi mapping (§10). Nothing checks out, merges or cherry-picks the tag. The
paths:

- `polyoxide-venue/src/trading.rs` (the trait, `TradingEvents` and the trading records);
- `polyoxide-venue/src/ids.rs` (the opaque id newtypes);
- `polyoxide-venue/tests/trading.rs` (its proofs, written against the tag's `Product`);
- `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`;
- `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md` (A7-1);
- `_bmad-output/implementation-artifacts/spec-6-1-6-3-7-3-7-5-records-keys-traits-and-the-kalshi-mapping.md`
  (bundle N's spec, with its review triage).

**1.4 Facts probed on 2026-10-17** (live, read-only, 16:39 to 16:40 UTC, against
`wss://ws-subscriptions-clob.polymarket.com`):

- The market channel's first frame after a subscribe for two assets is one text JSON array of two
  `book` objects, one per asset. A subscribe update for a third asset is answered after about
  160 ms by its `book` as a JSON array of length one. Later book snapshots arrive as bare objects.
- A text `PING` is answered by a text `PONG` in about 150 ms, on both channels.
- A market subscription with `"assets_ids": []` stays open for 30 s and answers every `PING`.
- A user subscription with invalid credentials is closed by the server about 1.2 s after the
  subscribe, with close code 1008 and reason `authentication failed`. It sends no error frame.
- The bare tier's `MarketMessage::from_json` keeps only the first book of an array
  (`polyoxide-polymarket/src/clob/ws/market.rs:244-252`), and a protected test pins that
  (`book_array_with_multiple_entries_takes_first`, `market.rs:503`).

And from the mirrors under `docs/specs/`:

- `POST /order`'s `status` is `live`, `matched` or `delayed` (`docs/specs/clob/openapi.yaml:4866-4873`).
- `postOnly` is "only supported for GTC and GTD orders" (`openapi.yaml:4848-4852`).
- `DELETE /cancel-market-orders` requires both `market` and `asset_id` (`openapi.yaml:5060-5073`).
- `/balance-allowance`'s `balance` is fixed-point with 6 decimals (`openapi.yaml:5566-5574`).
- The collateral token is pUSD (`docs/specs/polymarket-llms.txt:193`).
- The user channel's `owner` on a trade is "API key of the taker", `trader_side` says whether the
  receiving user was TAKER or MAKER, and `timestamp` is in milliseconds
  (`docs/specs/clob/asyncapi-user.json`).
- v1 `/positions`' `sizeThreshold` defaults to 1, with minimum 0 (`docs/specs/data/openapi.yaml:850-854`).

**1.5 Departures from the acceptance criteria.**

- **The clob `Trading` impl is gated `all(clob-ws, data)`, not `all(clob, data)`** (D6): `events()`
  needs the user socket, which is `clob-ws`'s.
- **`ClobOrderOptions` carries no fee terms** (D8): a V2 order signs no fee rate.
- **Story 7.5's mapping stays at `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`**,
  not `docs/specs/kalshi/trading-mapping.md` (D9).
- **Every events stream begins with `Resync`, and a voided fill yields `Resync`** (D3, D4), where
  AD-24 asks for one only after a reconnect.

## 2. Decisions (binding)

Each settles what the criteria leave open or records the owner's choice. A developer who finds the
plan disagreeing with one of them stops and asks.

- **D1. `Trading` follows Lane B's reviewed design, ported onto Epic 6's types.** The trait, its
  records, `TradingEvents` and the ids are the tag's, with the changes §6 lists: a feature of their
  own, `trading`; records that expose `pub extensions` as Epic 6's do (no `ext_accessor!`);
  `TradeId` left where Epic 6 put it; and the tests rewritten onto `KeySpace`.
- **D2. `events()` returns the `TradingEvents` newtype** (the owner, N-D3, 2026-10-12; amendment
  A7-1). dynosaur 0.3.1 drops `'static` from a returned `impl Stream` (`dynosaur_derive`
  `expand.rs:188-208`), so through `Arc<DynTrading<'static>>` an `impl Stream + 'static` return
  would borrow the `Arc` (E0597). A concrete type passes through unchanged.
- **D3. Every events stream begins with `Resync`** (the owner, 2026-10-17; bundle N's finding E15).
  The stream yields `TradingEvent::Resync` as its first item, once its fills connection's first
  connect attempt has ended, whether it went live or began in an outage. A fill between `events()`
  returning and the first subscription is therefore covered by a reconciliation. Amends AD-24
  (A7-2).
- **D4. A trade yields a `Fill` when the CLOB reports it `MATCHED`; a `FAILED` trade yields
  `Resync`** (the owner, 2026-10-17). `MINED`, `CONFIRMED` and `RETRYING` yield nothing, so a trade
  is reported once. A `FAILED` trade voids a fill already reported, and the trait has no event that
  takes a fill back, so the stream yields `Resync` and the consumer reconciles. `Resync` therefore
  means "reconcile through `open_orders()` and the trade history: the stream cannot vouch for what
  it reported or missed", and it comes at the start, after every reconnect of the fills connection,
  and when the venue voids a fill. Part of A7-2.
- **D5. The CLOB honours client order ids in-process** (the owner, 2026-10-17). The CLOB stores no
  client id, so a `Clob` remembers the client id of each order it placed with one, keyed by the
  order's id. The order's id is its EIP-712 hash, which the client computes before sending, so an
  ack or fill that arrives before `place` returns still carries the client id. §7.6 gives the
  ledger's rules. The id is computed the same way for every signature type, a Deposit Wallet's
  (type 3) included (the owner, 2026-10-17): the exchange identifies an order by the EIP-712 hash
  of its `Order` struct under the exchange's domain, whatever signs it, so `place` supports every
  account the native order routes sign for (§7.2, §7.4).
- **D6. The clob `Trading` impl is gated `all(feature = "clob-ws", feature = "data")`.**
  `positions` reads the Data API (AD-3) and `events()` reads the supervised user socket, which is
  `clob-ws`'s; `clob-ws` implies `clob`. `clob-ws` turns on `polyoxide-venue/trading`, and so does
  the umbrella's `polymarket-clob-ws`. The gate is declared in `polyoxide-polymarket`'s
  `[package.metadata.polyoxide] notes`, which the generated crate list in CLAUDE.md and README.md
  shows. Amends AD-3's last bullet (A7-3).
- **D7. The kit dials through the protocol.** `polyoxide_ws::Protocol` gains a provided method,
  `connect`, which the `Supervisor` calls for every attempt; its default builds `handshake()`'s
  request and dials it with the kit's single-address `connect`. The clob protocols override it with
  clob's per-address dial (D20), so every address of a blackholed route is tried within
  `connect_timeout` per step under supervision too. Amends AD-11's list of what a venue implements
  (A7-4).
- **D8. `ClobOrderOptions` is `{ neg_risk, tick_size, expiration }`, with no fee terms.** A V2
  order signs no fee rate: `MarketOrderArgs::fee_rate_bps` is "reserved / ignored"
  (`polyoxide-polymarket/src/clob/types.rs:312-319`), and the fee is taken on chain at match time.
  A fill's `fee` is `None`, and the maker's `fee_rate_bps` rides in `FillExt`.
- **D9. The Kalshi mapping stays a planning companion** (the owner, N-D2, 2026-10-12). A
  `docs/specs/kalshi/` directory needs a `[workspace.metadata.polyoxide.mirrors.kalshi]` table or
  `gen_registry.py` fails (`scripts/gen_registry.py:545-549`), and adding that table is Story 8.1's
  registration (AD-13). Story 7 writes the mapping at
  `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md` and a deferred-work entry
  for 8.1 to move it.
- **D10. The user socket stops on the server's authentication refusal.** A close with code 1008 on
  the user channel is decoded as `ClobWsError::Authentication(<the close reason>)`, classed
  `Unauthorized`, and recovery stops on it: the stream yields it and ends. Retrying cannot fix bad
  credentials, and without this the socket would reconnect forever at the backoff ceiling. The
  variant exists today and nothing constructs it (deferred-work.md:335-337); this is its first use.
- **D11. A supervised socket yields every book of an array frame.** The market channel's initial
  dump and each subscribe answer arrive as JSON arrays of books (§1.4), and AD-11 drops no decoded
  event, so the supervised market decode yields one event per element. The bare tier keeps its
  first-element behaviour, which a protected test pins.
- **D12. The supervised sockets take `wss://` URLs, or `ws://` to a loopback host.** The loopback
  form lets tests reach the scripted server (`ws://127.0.0.1:<port>`), and credentials never travel
  unencrypted to a remote host. The bare `WebSocketBuilder` keeps its `wss://`-only rule.
- **D13. Socket timing.** Both supervised sockets ping a text `PING` every 10 s (D4 of
  `divergences.md`; the first one interval after the connection opens, by the kit's schedule, where
  the bare tier pings at once). They count every inbound frame as liveness and are stale after
  30 s without one. They back off 500 ms to 60 s (the kit's default). Each address and each step
  get a 10 s `connect_timeout` (D20). Neither rotates. A refusal's `Retry-After` is waited out, as
  the kit does for every protocol (DRIFT R11).
- **D14. Queues.** The market socket's queue is `Queue::Bounded(1024)` (as perps'). The user
  socket's is `Queue::Unbounded` (AD-24), so the kit never waits on its consumer.
- **D15. Recovery.** Both protocols share one rule:
  - a frame that does not decode (`ClobWsError::Json`, or `InvalidMessage` from decode) is
    skipped: the stream yields the error and keeps the connection (`Recovery::Skip`);
  - `ClobWsError::Authentication` stops the stream (`Recovery::Stop`);
  - a connection used after it closed (`ClobWsError::Connection` whose kit error's kind is
    `polyoxide_ws::Kind::AlreadyClosed`) reconnects (`Recovery::Reconnect`);
  - any other error reconnects exactly when `Classify::is_retriable` says so, and stops otherwise.

  A refused upgrade is classed through the socket table, whose agreement with
  `polyoxide_ws::retrying_can_fix` and venue's status rule `polyoxide-venue/tests/ws_classification.rs`
  pins (DRIFT R1). The socket table classes a use after close `InvalidRequest` (spine AD-15), so
  the class rule alone would stop on it, and a ping that races a server close would end the
  stream. The `AlreadyClosed` row keeps the reconnect, as Binance's and sports' recovery rows do
  (D19 of `divergences.md`). It reads the kit's `Kind`, and no code matches over
  `tungstenite::Error`'s variants.
- **D16. Membership changes are recorded before their frame is sent** (Binance's rule). A change
  whose connection dies under it is answered `Ok` and replayed after the reconnect. A change made
  during an outage is applied to the membership and answered at once (`Offline::Recorded`), so the
  replay carries it.

## 3. Story 1: the kit dials through the protocol

**3.1 The hook.** In `polyoxide-ws/src/supervisor/mod.rs`, `Protocol` gains, after `handshake`:

    fn connect(&mut self, timeout: Duration) -> impl Future<Output = Result<Socket, Self::Error>> + Send

The default builds `self.handshake()` synchronously, before its async block, and dials that request
with the kit's `connect(request, timeout)`, converting the kit's `WsError` with `?`. Its rustdoc
says: the `Supervisor` calls it for every attempt with `Timing::connect_timeout`; a protocol that
dials its venue's addresses itself overrides it and keeps `handshake` for the request it upgrades
with. The doc names no venue: `polyoxide-ws` is a foundation crate, which `scripts/venue_gate.py`
holds, so it says "a venue that tries each resolved address in turn".

**3.2 The task.** `attempt` in `polyoxide-ws/src/supervisor/task.rs` calls
`protocol.connect(timing.connect_timeout).await?` where it called `protocol.handshake()?` and then
`connect(request, timing.connect_timeout).await?`. Nothing else in the task changes, so perps' and
Binance's protocols, which keep the default, behave as before. `Socket` is the kit's
`pub type Socket` (`polyoxide-ws/src/connect.rs:19`, re-exported at the root).

**3.3 Tests.** New tests in `polyoxide-ws/src/supervisor/tests.rs`, appended after the file's last
test, so the lines `docs/MUTANTS.md` cites (`tests.rs:753`, `:775`, `:795`) do not move:

- a protocol that overrides `connect` is called for the first attempt and again for every
  reconnect, and the socket it returns is the one the stream reads;
- an error its `connect` returns is a failed attempt: `connect` (the `Supervisor` method) returns
  it, and a retriable one is retried.

The tests' protocol is the file's own test protocol, extended or wrapped, with made-up names: no
venue word.

**3.4 clob's dial.** In `polyoxide-polymarket/src/clob/ws/client.rs`, the per-address loop of
`connect_ws` becomes `pub(super) async fn dial(url: &str, connect_timeout: Duration) -> Result<Socket, ClobWsError>`,
returning the upgraded socket. `connect_ws` keeps its name and signature and becomes
`dial(url, connect_timeout).await.map(|socket| Connection::new(socket, Mode::Data))`. Its tests
(`connect_tests`, `connection_tests`) are unchanged and keep passing. The same story raises these
private helpers to `pub(super)`, since Stories 2 and 3 use them:

- `frame_text` (D12 of `divergences.md`: Binary decoded as text);
- `validate_subscription_count`;
- `MAX_SUBSCRIPTIONS_PER_CONNECTION` (500);
- `DEFAULT_CONNECT_TIMEOUT` (10 s).

**3.5 Docs.** CLAUDE.md's socket-kit paragraph lists what a venue's `Protocol` describes; it gains
the dial ("its dial, `connect`, which by default dials the handshake's request and which a venue
that tries each address overrides").

## 4. Story 2: the supervised market socket

**4.1 Layout.** New module `clob::ws::supervised`:

- `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`: the module doc, the shared items (§4.4),
  `DisconnectReason`, and `mod market;`;
- `polyoxide-polymarket/src/clob/ws/supervised/market.rs`: the market protocol, its builder,
  events, change type and membership trait, with its tests inline;
- `polyoxide-polymarket/src/clob/ws/scripted.rs`, `#[cfg(test)]`: the scripted-server answerer the
  supervised tests share (§4.6).

`clob/ws/mod.rs` declares `mod supervised;` (private) and `#[cfg(test)] mod scripted;`, and
re-exports the public items at `clob::ws`. The bare tier (`WebSocket`, `WebSocketBuilder`,
`WebSocketWithPing`, `MembershipHandle`) is not edited, beyond Story 1's `dial`.

**4.2 Public items** (re-exported from `polyoxide_polymarket::clob::ws`):

- `pub type SupervisedMarketWs = polyoxide_ws::Supervised<ClobMarketProtocol>;`
- `MarketWsBuilder`: `Debug`, `Clone` and `Default`. It has `new()` (the default URL `WS_MARKET_URL`,
  no options, D13's timing) and `url(self, url: impl Into<String>) -> Result<Self, ClobWsError>`
  (D12). It has `options(self, options: MarketSubscriptionOptions) -> Self`, and the four
  `polyoxide_ws::timing_setters!` entries `ping_interval`, `stale_after`, `backoff` and
  `connect_timeout`. And it has `async fn connect(self, asset_ids: Vec<String>) -> Result<SupervisedMarketWs, ClobWsError>`,
  which checks the count (at most 500, through `validate_subscription_count`) before connecting and
  returns a first failure (`Supervisor::connect`).
- `ClobMarketProtocol`, with private fields (its URL and options), implementing
  `polyoxide_ws::Protocol` with `Event = MarketEvent`, `Error = ClobWsError`,
  `Membership = Vec<String>`, `Change = MarketChange` and `Ack = ()`.
- `#[non_exhaustive] pub enum MarketEvent { Message(MarketMessage), Disconnected { reason: DisconnectReason }, Reconnected }`,
  deriving `Debug`.
- `#[non_exhaustive] pub enum MarketChange { Subscribe(Vec<String>), Unsubscribe(Vec<String>) }`,
  deriving `Debug`, `Clone`, `PartialEq` and `Eq`.
- `pub trait MarketMembership` with `fn subscribe(&self, asset_ids: Vec<String>) -> impl Future<Output = Result<(), ClobWsError>> + Send`
  and `fn unsubscribe` of the same shape, implemented for
  `polyoxide_ws::MembershipHandle<ClobMarketProtocol>` as `self.apply(MarketChange::…)`, as perps'
  `PerpsMembership` is.
- `#[non_exhaustive] pub enum DisconnectReason { Closed { code: Option<u16>, reason: String }, Error(ClobWsError), Stale { silent: Duration }, Rotation }`,
  deriving `Debug`, in `supervised/mod.rs`; Story 3's user socket uses it too. It is mapped from
  the kit's `Outage` as Binance's `DisconnectReason` is
  (`polyoxide-binance/src/usdm/ws/protocol.rs` `disconnected`), and a future `Outage` variant maps
  to `Stale { silent: Duration::ZERO }`.

**4.3 The protocol.**

- `handshake`: the URL as a client request.
- `connect`: overridden with `dial(&self.url, timeout)` (D7).
- `replay`: always sends one subscribe frame, `MarketSubscription` (subscription.rs) with the whole
  membership and the builder's options, even when the membership is empty (§1.4: an empty
  subscription stays open).
- `apply` (D16): `Subscribe` adds the ids the membership lacks, refusing with the existing "too many
  subscriptions" `InvalidMessage` if the result would pass 500, and sends one
  `{"operation":"subscribe","assets_ids":[…]}` frame (`MarketSubscriptionUpdate`) for the added ids
  only. `Unsubscribe` removes the ids the membership holds and sends one `unsubscribe` frame for
  them. A change that adds or removes nothing sends nothing and is answered `Ok`. The membership
  keeps first-subscribed order and holds no duplicates.
- `record` (D16): applies the change to the membership and answers `Offline::Recorded(Ok(()))`, or
  `Recorded(Err(..))` for the count refusal.
- `decode` (D11):
  - the frame's text, from `frame_text` (Binary decoded as UTF-8), skipping `PONG`, `{}`, empty
    text and any text without the substring `event_type`, as the bare tier does
    (`client.rs:146-167`), all as `Decoded::Nothing`;
  - a text starting with `[` is parsed as `Vec<BookMessage>` and yields `Decoded::Events`, one
    `MarketEvent::Message(MarketMessage::Book(..))` per element;
  - any other text goes through `MarketMessage::from_json`;
  - a failed parse is `Decoded::Error(ClobWsError::Json(..))`, which D15 skips;
  - Close, Ping and Pong frames are `Decoded::Nothing`.
- `is_live`: every frame. `delivered`: a decoded event.
- `ping`: `Some(Message::Text("PING".into()))`.
- `wanted`: the default (true), so an empty membership stays connected.
- `is_retriable`: `Classify::is_retriable`. `recover`: D15. `retry_after`: `Classify::retry_after`.
- `disconnected`: `Some(MarketEvent::Disconnected { reason })` (it declares). `reconnected`:
  `MarketEvent::Reconnected`. `queue`: `Queue::Bounded(1024)`.

**4.4 Shared helpers** in `supervised/mod.rs`, `pub(super)`, which Story 3 reuses:

- `fn checked_url(url: String) -> Result<String, ClobWsError>`: D12's rule. A `wss://` URL is
  accepted. A `ws://` URL is accepted only when its host is `127.0.0.1`, `[::1]` or `localhost`.
  Anything else is refused as `ClobWsError::InvalidMessage("the URL must be wss://, or ws:// to a loopback host: <url>")`.
- `fn default_timing() -> Timing`: D13.
- `fn disconnect_reason(outage: Outage<ClobWsError>) -> DisconnectReason`.
- `fn recover(err: &ClobWsError) -> Recovery`: D15.
- `fn data_text(frame: &Message) -> Option<&str>`: the decode filter of §4.3's first bullet.

**4.5 Tests**, inline in `market.rs`, on the kit's scripted server
(`polyoxide_ws::test_server`, through `scripted.rs`) and the kit's `fast()` timing with the
`PING` schedule left on:

- it reconnects after a server close (`Script::close_with`) and after a stall (`go_silent`);
- it replays the whole membership, changes included, on each new connection (`first_frames`);
- one `Disconnected` then one `Reconnected` per outage, the close's code in `Closed`;
- it sends a text `PING` on the schedule (`count_frames(|f| f == "PING")`), and a `PONG` is no
  event;
- a Binary frame decodes as text (D12);
- an array frame yields every book (D11);
- an empty membership stays connected and keeps pinging;
- a change during an outage is answered at once and replayed;
- a 501st id is refused and changes nothing;
- the URL rules (D12);
- a frame that does not decode is yielded as an error, and the stream goes on.

**4.6 The scripted answerer.** `clob/ws/scripted.rs` (`#[cfg(test)]`) holds
`pub(crate) fn pong(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send + 'static`, the
answerer for `ScriptedServer::start_answering`. It answers a text `PING` with a text `PONG` and
nothing else. Stories 2, 3 and 5's unit tests use it; an integration test cannot see it and writes
its own.

**4.7 Docs.** CLAUDE.md's sentence "clob keeps its own `select!`, its 10 s text `PING`, its outbound
queue and Binary decoded as text (D12), and is not supervised until Epic 7" says that the bare tier
keeps them and that the supervised sockets run on the `Supervisor`. A new paragraph after the
"**WebSocket** support for the CLOB" paragraph, beginning "**The supervised CLOB market socket**",
describes `MarketWsBuilder`/`SupervisedMarketWs` and D11 to D16 as they apply. `divergences.md` is
Story 7's.

## 5. Story 3: the supervised user socket

**5.1 Layout.** `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, declared from
`supervised/mod.rs`, with its tests inline; re-exports added to `clob/ws/mod.rs`.

**5.2 Public items** (re-exported from `clob::ws`):

- `pub type SupervisedUserWs = polyoxide_ws::Supervised<ClobUserProtocol>;`
- `UserWsBuilder`: `Debug`, `Clone` and `Default`, holding no credential. It has `new()` (the
  default URL `WS_USER_URL`, D13's timing), `url(self, url: impl Into<String>) -> Result<Self, ClobWsError>`
  (D12) and the four timing setters. It has
  `async fn connect(self, markets: Option<Vec<String>>, credentials: ApiCredentials) -> Result<SupervisedUserWs, ClobWsError>`,
  which returns a first failure (`Supervisor::connect`). And it has `async fn start(self, markets: Option<Vec<String>>, credentials: ApiCredentials) -> Result<SupervisedUserWs, ClobWsError>`,
  which begins in an outage when the first attempt fails in a way recovery would retry
  (`Supervisor::start`), and returns only an error recovery stops on. Both check the count first
  (at most 500 markets).
- `ClobUserProtocol`, with private fields (its URL and the `ApiCredentials`), implementing
  `Protocol` with `Event = UserEvent`, `Error = ClobWsError`, `Membership = Option<Vec<String>>`,
  `Change = UserChange` and `Ack = ()`.
- `#[non_exhaustive] pub enum UserEvent { Message(UserMessage), Disconnected { reason: DisconnectReason }, Reconnected }`,
  deriving `Debug`.
- `#[non_exhaustive] pub enum UserChange { Subscribe(Vec<String>), Unsubscribe(Vec<String>) }`.
- `pub trait UserMembership` with `subscribe(&self, markets: Vec<String>)` and `unsubscribe`, each
  `-> impl Future<Output = Result<(), ClobWsError>> + Send`, implemented for
  `MembershipHandle<ClobUserProtocol>`.

**5.3 Membership** (AD-11). `None` means every market: the subscribe frame omits `markets`.
`Some(markets)` means those markets: the frame carries `"markets": […]`, and `Some(vec![])` sends
`"markets": []`, which subscribes to none.

- `Subscribe` on `None` changes nothing, sends nothing and is answered `Ok`: every market is
  already followed.
- `Unsubscribe` on `None` is refused as
  `ClobWsError::InvalidMessage("the user channel follows every market; unsubscribing needs a market filter")`.
- On `Some`, `Subscribe` adds (refusing past 500) and `Unsubscribe` removes, each sending one
  update frame (`UserSubscriptionUpdate`) for the markets it actually changed.
- Removing the last market leaves `Some(vec![])`. Nothing ever turns `Some` into `None`; a test
  pins it.

**5.4 The protocol** is the market protocol (§4.3) but for these:

- `replay` sends `UserSubscription` (subscription.rs) built afresh on every attempt from the
  protocol's own `ApiCredentials` and the membership, so the credentials are rebuilt and replayed
  on every reconnect.
- `decode` reads `UserMessage::from_json`; a JSON array yields one event per element, as §4.3's
  does.
- A Close frame with code 1008 decodes as `Decoded::Error(ClobWsError::Authentication(reason))`
  (D10), on which `recover` stops.
- `queue` is `Queue::Unbounded` (D14), and `disconnected` declares
  (`UserEvent::Disconnected { reason }`).

**5.5 Tests**, inline in `user.rs`, on the scripted server:

- `None` sends a frame without `markets`; `Some(vec![])` sends `"markets": []`;
- unsubscribing the last market leaves `Some(vec![])`, and the next connection's subscribe frame
  carries `"markets": []`, never no key (the AD-11 pin);
- every connection's first frame carries the `auth` object (credentials replayed);
- one `Disconnected` and one `Reconnected` per outage;
- with the consumer blocked, a burst of trade frames the server pushes (more than 1024) all arrive,
  in order, once it reads (`Unbounded`);
- a 1008 close yields `ClobWsError::Authentication` and the stream ends, with no reconnect;
- `Subscribe` on `None` sends nothing; `Unsubscribe` on `None` is refused;
- the URL rules.

**5.6 Docs.** A paragraph after Story 2's, beginning "**The supervised CLOB user socket**",
describes the membership (§5.3), the credential replay, D10 and D14.

## 6. Story 4: the `Trading` trait

**6.1 Feature and files.** `polyoxide-venue` gains `trading = ["market", "dep:futures-core"]`, with
`futures-core` an optional dependency taken from a new `[workspace.dependencies]` entry
`futures-core = "0.3"`. AD-3 already allows `futures-core`, and every dependency allowlist under
`ci/dep-allowlists/` already holds it, so no allowlist file changes. The dev-dependencies gain
`tokio` (`macros` and `rt-multi-thread`) and `futures-util`, for the tests.
`[package.metadata.docs.rs]`'s features gain `trading`. New files:

- `polyoxide-venue/src/ids.rs`: `OrderId`, `FillId` and `ClientOrderId`, through a private
  `opaque_id!` macro (the tag's);
- `polyoxide-venue/src/trading.rs`: the trait, `TradingEvents` and the trading records;
- `polyoxide-venue/tests/trading.rs`: `#![cfg(feature = "trading")]`.

`lib.rs` declares both modules under `#[cfg(feature = "trading")]` and re-exports their items at
the crate root.

**6.2 The items** are the tag's `trading.rs` and `ids.rs` (§1.3) with these changes:

- `cfg(feature = "traits")` becomes `cfg(feature = "trading")`;
- the `ext_accessor!` line is dropped;
- the docs carry D3 and D4, as §6.3 lists;
- the imports name HEAD's types.

`TradeId` stays in `records.rs` under `market`, unchanged; `Fill::trade_id` names it. The names
contract in the plan lists every item's exact signature.

**6.3 The docs the items carry** (beyond the tag's text, which already says that a request is
refused, never ignored, when it carries an undeclared extension, a key of another product, or a
`post_only`, `reduce_only` or time in force the venue cannot honour):

- `Trading` and `TradingEvents`: the stream's first item is `Resync` (D3); `Resync` comes again
  after every reconnect of the connection that carries fills and when the venue voids a fill it
  reported (D4); the venue replays nothing.
- `TradingEvent::Resync`: "Reconcile through `Trading::open_orders` and the venue's trade history:
  the stream cannot vouch for what it reported or missed before this point."
- `OrderRequest::client_order_id`: the venue echoes it, or the implementation keeps it for the
  orders it placed, as its documentation says; a venue that can do neither refuses a request that
  carries one.
- `Trading::events`: an order may be reported (acked or filled) before the `place` that created it
  returns.

**6.4 Refusing undeclared options.** `Extensions` gains
`pub fn ensure_declared(&self, declared: &[TypeId]) -> Result<(), UndeclaredExtension>`, under
`market`. It refuses with the first undeclared value in `TypeId` order. The value's type name comes
from the private `Extension` trait, which gains a method returning `std::any::type_name` of the
value's type.

`#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] pub struct UndeclaredExtension { pub type_name: &'static str }`
displays as "the request carries `<type_name>`, an extension this venue does not declare". It
implements `Error` and `Classify` (`Class::InvalidRequest`), and is listed in `lib.rs`'s `Classify`
assertion under `market`.

**6.5 Tests** (`tests/trading.rs`, the tag's proofs rewritten onto `KeySpace` with a made-up
venue, `acme`; the venue gate forbids every registered venue and product word in this crate):

- a test venue implements `Trading`, used both statically and as `Arc<DynTrading<'static>>`;
- `the_stream_outlives_a_dropped_dyn_clone_and_moves_to_another_thread`: take the stream from a
  temporary `Arc<DynTrading<'static>>` clone, drop every `Arc`, and drain it on a
  `std::thread::spawn`ed thread;
- a kill is `Ok` with `Killed { reason }`;
- `ensure_declared` refuses an undeclared extension as `InvalidRequest`, naming its type, and
  accepts the declared one and an empty set;
- the records' constructors fill the documented defaults;
- the ids are their text (`Display`, `as_str`, `From`).

Unit tests for `ensure_declared` go in `extensions.rs`'s test module as new items.

**6.6 Docs.**

- `polyoxide-venue`'s `[package.metadata.polyoxide] readme` line names the trading vocabulary
  (feature `trading`), and `python3 scripts/gen_registry.py --write` regenerates the crate lists.
- `polyoxide-venue/README.md`, which `lib.rs` includes as a doctest, gains a prose line with no
  code fence.
- CLAUDE.md gains a paragraph "**The trading trait**", placed after the "**Market-data traits**"
  paragraph and before "**The CLOB through the traits**": its methods, records, `TradingEvents`
  (D2), D3, D4 and `ensure_declared`.

## 7. Story 5: `Trading` on the CLOB — requests

**7.1 Layout.** `src/clob/venue.rs` declares, gated `all(feature = "clob-ws", feature = "data")`,
two child modules in `src/clob/venue/`:

- `trading.rs`: `impl Trading for Clob`, the request methods, `ClobOrderOptions`, the `Ext` types
  and the client-id ledger;
- `fills.rs`: the events adapter (§8).

`venue.rs` re-exports `ClobOrderOptions`, `OrderExt`, `FillExt`, `PositionExt` and `BalanceExt`,
under the same gate. `src/clob/error.rs` is not edited: `docs/MUTANTS.md` rows 147 to 149 cite its
lines.

**7.2 Wiring.**

- `polyoxide-polymarket`'s `clob-ws` feature gains `"polyoxide-venue/trading"`, and the umbrella's
  `polymarket-clob-ws` gains `"polyoxide-venue/trading"`.
- `ClobBuilder` gains `user_socket(self, socket: crate::clob::ws::UserWsBuilder) -> Self`
  (`clob-ws`), and `Clob` keeps it, defaulting to `UserWsBuilder::new()`.
- `Clob` also gains the client-id ledger (§7.6) under D6's gate.
- `eip712.rs` gains `pub(crate) fn order_hash(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError>`:
  the order's EIP-712 hash under its exchange's domain, for every signature type.
  `compute_order_digest` keeps its type-3 refusal (a Deposit Wallet signs an ERC-7739 envelope, not
  this digest), then calls `order_hash`, and stays private. `docs/MUTANTS.md` cites no line of
  `eip712.rs`. `Clob::resolve_maker_address` becomes `pub(crate)`.
- The umbrella pins `DynTrading` under `polymarket-clob-ws` and `Clob: Trading` under
  `all(polymarket-clob-ws, polymarket-data)`, on the shape of the existing `DynMarketData` pin
  (`polyoxide/src/lib.rs:273-281`).
- The `notes` entry (D6) names the gate.

**7.3 Account.** Every method needs an `Account`; without one each fails with the CLOB's own
"Account required" error (`InvalidRequest`), and `events()` yields that error as its only item.

**7.4 `place`.** Checks first, each refused as `InvalidRequest` with nothing sent:

1. the key is a clob outcome key (`shared::keys::token_id`);
2. `ensure_declared(&[TypeId::of::<ClobOrderOptions>()])`;
3. `reduce_only` is false: the CLOB has no reduce-only orders;
4. `post_only` is set only with `GoodTilCancelled` (the venue's rule, §1.4);
5. `ClobOrderOptions::expiration` is set only with `GoodTilCancelled`;
6. the client order id is not held by an order still in the ledger (§7.6);
7. the price is a whole multiple of the token's tick size and lies strictly between 0 and 1, and
   the size has at most two decimal places and is at least 0.01. The tick size comes from
   `ClobOrderOptions::tick_size`, or else from `GET /tick-size` (the native lookup).

The order is then built, signed and posted:

- **Time in force:** `GoodTilCancelled` maps to `OrderKind::Gtc`, or to `Gtd` when an expiration is
  set; `ImmediateOrCancel` to `Fak`; `FillOrKill` to `Fok`.
- **Build:** the native `create_order` with `PartialCreateOrderOptions { tick_size, neg_risk }`
  filled from the options or the lookups, so nothing is fetched twice. Prices and sizes convert
  from `Decimal` to the native `f64` only after the checks above. The order's signature type is
  the client's (`Clob`'s `signature_type`: `ClobBuilder::signature_type`, by default the account
  target's), passed as the order's own with no funder. So `place` signs and funds from the address
  `balances` reads and `positions` lists: with a proxy signature type, the proxy wallet, which the
  native routes look up through Gamma; without `gamma` such an order is refused, as the native
  routes refuse it.
- **Id:** `eip712::order_hash` gives the order's id before it is signed, for every signature type
  (D5).
- **Sign and post:** `Clob::sign_order`, which signs through the account's `sign_order_as` (a
  Deposit Wallet's order as its ERC-7739 envelope, for the account's
  `SigningTarget::DepositWallet`), then `post_order(signed, kind, post_only)`. So `place` works
  for every account the native order routes sign for, a Deposit Wallet (type 3) included.

The outcomes:

- `ClobError::FakUnmatched` is `Ok`, status `Killed { reason: Unmatched }`, `filled` zero, and the
  computed id. `FokUnfilled` is `Ok` with `Killed { reason: Unfillable }`. The match is on the
  native variants, before any conversion to `ClassifiedError` (AD-20).
- Any other error is returned (`?`).
- A 200 whose `success` is false is a `VenueRefusal`, carrying the venue's `errorMsg`.
- Status `live` or `delayed` is `Open` with `filled` zero.
- Status `matched` is read back with `GET /data/order/{id}` (`orders().get`) for `size_matched`:
  - a full fill is `Filled`;
  - an `ImmediateOrCancel` order short of its size is `Killed { reason: Unmatched }`, with what
    matched in `filled`;
  - a `FillOrKill` short of its size is `Killed { reason: Unfillable }`;
  - a resting order short of its size is `Open`.

  If that read fails, the order is returned `Open` with `filled` zero and a WARN under
  `polyoxide_polymarket`. The order exists on the venue, so an `Err` would invite a duplicate
  placement, and the events stream reports the rest.
- An unknown status is `Open`.

The returned `Order` is filled as follows:

- **Id:** the response's `orderID`, or the computed id when the response has none. If the two
  differ, the ledger entry is re-keyed and a WARN logged.
- **Request fields:** `time_in_force` is `Some(request's)`, and `client_order_id` is the request's.
- **`OrderExt`:** the venue's raw status, the transaction hashes, the resolved `neg_risk` and the
  expiration.

**7.5 The other methods.**

- **`cancel(id)`:** `DELETE /order`. `Ok` when the id is in `canceled`. A `VenueRefusal` carrying
  the venue's reason when it is in `not_canceled`. A `VenueRefusal` "the CLOB did not answer for
  order <id>" when it is in neither.
- **`cancel_all(None)`:** `DELETE /cancel-all`.
- **`cancel_all(Some(key))`:** resolves the token's condition with `/markets-by-token/{token}`, as
  `MarketData::instrument` does, and then sends `DELETE /cancel-market-orders` with both `market`
  and `asset_id` (§1.4).
- **What `cancel_all` returns:** the `canceled` ids. Orders in `not_canceled` stay open (the
  trait's doc). Ledger entries of cancelled orders are dropped.
- **`open_orders(key)`:** `GET /data/orders`, filtered by `asset_id` for a key and paged by
  `next_cursor`. Paging ends at the `LTE=` sentinel or an empty or absent cursor. A cursor seen
  twice is a `Decode` error.
  - Each order maps `LIVE` to `Open`, `MATCHED` to `Filled`, and `CANCELED` and
    `CANCELED_MARKET_RESOLVED` to `Cancelled`; anything else is `Open`, with the raw status in
    `OrderExt`.
  - `size_matched` is `filled`, `order_type` is the time in force, `created_at` (seconds) is
    `created`, and the client id comes from the ledger.
- **`positions()`:** the Data API's v1 `/positions` for the account's maker address, with
  `sizeThreshold=0`, so no dust is left out (§1.4), and `limit=500`.
  - **Maker address:** the address the client's orders are placed from (`resolve_maker_address`
    with no per-order funder and the client's signature type, as `place` uses, §7.4).
  - **Paging:** pages by `offset` until a page holds fewer than 500 rows. Data's offset stops at
    10,000, so a 21st full page is a `VenueRefusal` "the wallet holds more positions than
    `/positions` can list" rather than a silent truncation.
  - **Records:** `key` is the clob key of `asset`, `size` is `size`, and `average_price` is
    `Some(avg_price)`. The other fields go to `PositionExt`. `f64` values convert through their
    shortest decimal spelling (`clob::venue`'s existing helper).
- **`balances()`:** one `Balance` for the collateral, from `GET /balance-allowance` with
  `asset_type=COLLATERAL` (`account_api().usdc_balance()`). Its `asset` is `"pUSD"`, its `total`
  is `balance` divided by 10^6, and its `available` is `None`: the venue reports no reservation by
  open orders. `BalanceExt` holds each allowance, divided by 10^6, keyed by its spender. Outcome
  tokens are positions, not balances.

**7.6 The client-id ledger** (D5) is `Arc<std::sync::Mutex<…>>`, shared by every clone of a `Clob`
and by every events stream it opened. It is never held across an `.await`.

- **Entries:** it maps `OrderId` to `ClientOrderId`, with a reverse index for the duplicate check.
- **When entries are added:** `place` inserts its order's entry before posting.
- **When entries are dropped:**
  - `place` drops the entry when the order ends at once (`Killed`, `Filled`) or the post is
    refused. After a `Network` or `Unavailable` failure, which may have reached the venue, the
    entry stays until the bound evicts it;
  - `cancel` and `cancel_all` drop the entries of the orders they cancelled;
  - the events adapter drops the entry of an order acked `Cancelled` or `Filled`.
- **Bound:** at most 10,000 entries; the oldest is dropped first.
- **Duplicates:** a client id held by an entry is refused on a new `place` with
  "client order id `<id>` is already in use by order `<order id>`".
- **Scope:** the ledger is lost when the process ends, and orders placed elsewhere carry no client
  id. The rustdoc of `impl Trading for Clob` says so.

**7.7 The `Ext` types** derive `Debug`, `Clone`, `PartialEq`, `Default` and are
`#[non_exhaustive]` (`Eq` where their fields allow), as Epic 6's are. Each holds, as `Option`s,
the venue's facts that the shared record lacks (NFR8):

- `OrderExt`: at least the raw status, the condition id (`market`), the outcome, the maker
  address, the expiration, `neg_risk`, the associated trade ids and the transaction hashes;
- `FillExt`: at least the condition id, the outcome, the taker order id, the transaction hash and
  the maker's `fee_rate_bps`;
- `PositionExt`: at least the condition id, the outcome and its index, the current price, the
  current and initial value, the cash and realized P&L, `redeemable`, `mergeable`, `negative_risk`,
  the title and the end date;
- `BalanceExt`: the allowances.

## 8. Story 5: `Trading` on the CLOB — events

**8.1 The stream.** `events()` returns `TradingEvents::new(..)` over a stream that does the
following, in order, when first polled:

1. **Resolve the maker address** (`resolve_maker_address`, as `positions` does, §7.5). A failure is
   the stream's only item.
2. **Build the credentials.** `ApiCredentials` come from the account's `Credentials` (key, secret,
   passphrase).
3. **Start the socket.** `self.user_socket.clone().start(None, credentials)`: every market of the
   account (`None`). An error `start` returns is the stream's only item.
4. **Yield `Resync`** (D3), and then map each `UserEvent` in order (§8.2).

Each call to `events()` opens its own socket and sees every event from its first poll on.

**8.2 The mapping.**

- **`Message(UserMessage::Order(o))` is `TradingEvent::Ack(Order)`:**
  - key: `o.asset_id`'s clob key;
  - side, price, `size` (`original_size`) and `filled` (`size_matched`), parsed from their text;
  - status: `PLACEMENT` is `Open`; `UPDATE` is `Filled` when `size_matched` reaches
    `original_size`, and `Open` otherwise; `CANCELLATION` is `Cancelled`;
  - `created`: the message's `timestamp` (milliseconds) for a `PLACEMENT`, `None` otherwise;
  - `time_in_force`: `None`, since the message does not carry it. So a FAK remainder cancelled on
    the venue is acked `Cancelled`, though `place` returned it `Killed`;
  - the client id from the ledger, whose entry an acked `Cancelled` or `Filled` drops.
- **`Message(UserMessage::Trade(t))` with status `MATCHED` yields one `TradingEvent::Fill` per
  order of the account's in the trade**, the taker's first and then the makers' in their order:
  - **Taker:** `t.owner` equals the account's API key. Its fill takes `taker_order_id`, `t.side`,
    `t.price`, `t.size` and `t.asset_id`, with `Liquidity::Taker`.
  - **Makers:** each `maker_orders` entry whose `maker_address` equals that maker address (§8.1),
    ignoring ASCII case. Its fill takes the entry's `order_id`, `price`, `matched_amount` and
    `asset_id`, with `Liquidity::Maker`. Its side is the opposite of `t.side` when its `asset_id`
    is the trade's, and the same as `t.side` when it is the complementary token (a mint or merge
    match).
  - **Every fill:**
    - `id`: `FillId` `"<trade id>:<order id>"`;
    - `trade_id`: `Some(TradeId(t.id))`;
    - `time`: `t.timestamp` (milliseconds);
    - `fee` and `notional`: `None`;
    - `client_order_id`: from the ledger.
  - **A trade naming none of the account's orders** is an `Err` item (`Decode`), and the stream
    goes on.
- **A trade with status `FAILED` yields `TradingEvent::Resync`** (D4). `MINED`, `CONFIRMED` and
  `RETRYING` yield nothing.
- **`UserEvent::Reconnected` yields `TradingEvent::Resync`.**
- **`UserEvent::Disconnected`** whose reason is `Error(e)` yields `e` as an `Err` item. Any other
  reason yields nothing; the `Resync` after the reconnect follows either way.
- **An `Err` the socket yields** is an `Err` item (`ClassifiedError::from`). The stream ends when
  the socket's ends, as after an error recovery stopped on.
- **Unparseable text in a message** (a number, a side, a timestamp) is an `Err` item (`Decode`),
  and that message is skipped.

**8.3 Tests** (`polyoxide-polymarket/tests/trading_clob.rs`, `[[test]]` with
`required-features = ["clob-ws", "data"]`). The HTTP mocks run on mockito. The socket runs on
`polyoxide_ws::test_server`, with the test's own `PING` answerer: an integration test cannot see
the crate's `#[cfg(test)]` adapter. The account is mock_clob's EOA test account, whose maker
address is its own. The cases:

- **`place`:**
  - a GTC order that rests (`live`);
  - a FAK kill as `Killed { Unmatched }` and a FOK kill as `Killed { Unfillable }`, using
    mock_clob's verbatim venue messages;
  - `matched` read back as `Filled`;
  - an IOC partial match as `Killed { Unmatched }` with its `filled`;
  - `success: false` as a `VenueRefusal`;
  - a Deposit Wallet account's order (type 3): the posted body carries signature type 3, the
    wallet as maker and signer and the signature `sign_order_as` gives, and the returned id is
    `order_hash`'s.
- **`order_hash`**, in `eip712.rs`'s tests (appended): for `order_vectors.json`'s `v2_exchange`
  order (`tests/fixtures/clob/session_keys/`) it equals
  `keccak256(0x1901 ‖ app_domain_separator ‖ contents_hash)`, the two py-sdk values that file
  records, after asserting that `exchange_address(137, false)` is that vector's `exchange` (the
  chain table resolves to the V2 exchange, so `v1_exchange`'s separator cannot compose with it);
  and for a plain order it equals `compute_order_digest`'s digest.
- **Refusals that send nothing:** reduce-only, an undeclared extension, post-only with IOC, an
  off-tick price, a size with three decimals, a key of another product, and a duplicate client id.
  These use mocks that `expect(0)` with `match_query(Any)` on query routes.
- **`cancel`:** `canceled` and `not_canceled`.
- **`cancel_all`:** of everything, and of one key through `markets-by-token`.
- **`open_orders`:** over two pages to the sentinel, and a repeated cursor.
- **`positions`:** over two pages, with `sizeThreshold=0` and the maker address in the query.
- **`balances`:** the 6-decimal scaling.
- **`events()`:**
  - the leading `Resync`;
  - a taker fill and a maker fill (one of two maker entries the account's);
  - a `FAILED` trade as `Resync`;
  - acks of a placement and a cancellation;
  - client ids carried on the acks and fills of an order placed with one;
  - with a slow consumer and a server cut mid-stream: every fill pushed is delivered, in order, and
    a `Resync` sits between the fills before the cut and those after it.
- **`Arc<DynTrading<'static>>` over a `Clob`:** `place` and `events` through it.

**8.4 Docs.**

- CLAUDE.md gains a paragraph "**The CLOB through `Trading`**", placed after "**The CLOB through
  the traits**": the gate (D6), §7.4 to §7.6 and §8.2 in brief.
- The clob module README (`src/clob/README.md`, a doctest under `clob`) gains a prose line, with no
  code fence.

## 9. Story 6: live suites

**9.1 The sockets.** `polyoxide-polymarket/tests/live_clob_ws.rs` (features `clob-ws`) gains two
`#[ignore]`d tests:

- **Market:** `SupervisedMarketWs` subscribed to one live token receives that token's `book`
  within 30 s. The token comes from the CLOB's own sampling markets, read with `Clob::public()`.
  No token is an `environmental(..)` skip.
- **User:** `SupervisedUserWs` with `None` and the account's credentials, obtained as the file's
  existing tests obtain them (`derive_credentials`, `tests/live_clob_ws.rs:66`), opens and stays
  connected for 15 s without an error item. Missing credentials are auth-gated. A 1008 close fails
  through the error's tag.

**9.2 Trading.** A new target, `polyoxide-polymarket/tests/live_clob_trading.rs`, with
`[[test]] required-features = ["clob-ws", "data", "gamma"]` and
`[package.metadata.polyoxide.live.live_clob_trading]`: suite `clob`, timeout 15, features
`["clob-ws", "data", "gamma"]`, and as its secrets exactly the names its tests pass to the credential
loaders, since `.github/scripts/tests/test_live_registry.py` (`test_scanned_env_names_equal_declared_secrets`)
fails a target whose declared secrets differ from the names it reads; the `clob` job's union still
carries every secret `live_clob` reads. `python3 scripts/gen_registry.py --write`
regenerates the nightly regions. Its tests, all `#[ignore]`d and auth-gated without credentials:

- **Reads:** `open_orders(None)`, `positions()` and `balances()` through `Arc<DynTrading<'static>>`
  succeed; the balance's `asset` is `"pUSD"` and its `total` is not negative.
- **Round trip:** with `events()` open, a post-only GTC buy of 5 shares at 0.01 on a token whose
  best ask is above 0.01 rests (`Open`). The stream yields its `Resync` and then an ack for the
  order. `cancel` then succeeds and the stream acks it `Cancelled`.
  - The token is chosen the way `live_v2_place_and_cancel` chooses one (`tests/live_clob.rs:138`,
    `find_token_id_with_min_ask`), copied into the new file, since a test target cannot import
    another's helpers; with no such token the test is an `environmental(..)` skip.
  - The order moves no capital and is cancelled on every path the test controls.

Every live failure goes through `polyoxide-test-support`'s tags (`.or_fail`, `fail_classified`,
the loaders, `environmental`, `transient`), with no `.unwrap()`, so `scripts/live_unwraps.py`
passes. A new live file's counts start at zero in `scripts/live_unwraps.baseline.json` if the
script requires an entry.

## 10. Story 7: the Kalshi mapping, amendments, release note and records

- **The mapping (D9).** `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md` is
  the tag's document, ported onto this epic's trait:
  - the feature is `trading`;
  - key construction follows `KeySpace`;
  - its `events()` paragraph adds the leading `Resync` (D3) and the void rule (D4);
  - its release-note section points at `docs/s3-migration.md`.

  It keeps the tag's sources (Kalshi OpenAPI 3.34.0, AsyncAPI 2.0.0, read 2026-10-12), its
  deferrals and its eight demo-host probes. Every common order field (ticker, side, count, price,
  time in force, client order id, post-only, reduce-only) and fill field (trade and order ids,
  price, size, fee, taker flag) maps to a trait field or a named future Kalshi type, and the
  YES-side conversion follows AD-7. A field that fits neither is recorded as an amendment; the tag
  found none.
- **Amendments.**
  `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md`
  holds A7-1 (the tag's text, with its paths updated), A7-2 (D3 and D4, amending AD-24), A7-3 (D6,
  amending AD-3), A7-4 (D7, amending AD-11) and "7.5: no amendment". They follow the format of
  `epic-6.md`. Merging them into the spine is an owner session after the run, as for Epic 6.
- **Release note (NFR11).** `docs/s3-migration.md` gains a section "## Epic 7: supervised CLOB
  sockets and normalized trading", stating that the epic is additive and naming each new item:
  - the supervised types, and that `WebSocketWithPing` is unchanged;
  - `Protocol::connect`;
  - the `trading` feature and its items;
  - `Extensions::ensure_declared` and `UndeclaredExtension`;
  - `ClobBuilder::user_socket`, and that `clob-ws` now turns on `polyoxide-venue/trading`;
  - `ClobWsError::Authentication`'s first use (D10);
  - the departures of §1.5.
- **Records.**
  - In `_bmad-output/implementation-artifacts/deferred-work.md`:
    - close the entry "surface the server's close code in the supervised clob sockets"
      (`DisconnectReason::Closed`);
    - narrow "prune `ClobWsError::{ConnectionClosed, Authentication}`" to `ConnectionClosed`;
    - add the mapping's move for 8.1 (D9) and the eight Kalshi demo-host probes.
  - In `_bmad-output/specs/spec-venue-extensibility/divergences.md`, the clob rows D4, D12, D19,
    D20 and R11 say what the supervised sockets do (R11's "clob does not reconnect" no longer
    holds).

## 11. Limits

- **The CLOB's sockets.**
  - Connect: 10 s per address per step (D13, D20).
  - Ping, staleness and backoff: a 10 s ping, staleness after 30 s, and a 500 ms to 60 s backoff.
  - `Retry-After`: waited out, clamped to the backoff ceiling (R11).
  - Bound: at most 500 ids or markets per connection (the existing cap), refused before
    connecting or changing.
  - Pacing: change frames are sent unpaced; the CLOB publishes no socket rate limit, and the bare
    tier sends unpaced.
  - Failure: D15. A bad credential ends the user stream (D10).
  - Queues: the market queue holds 1024 events; the user queue is unbounded by the owner's choice
    (AD-24).
- **The CLOB's and Data's HTTP routes** run on the existing send loop, throttles and retry policy:
  - nothing is added to `shared::*_limits`, since every route already has a row;
  - each `place` charges the signer bucket once (`PostOrder`) and each cancel route as today;
  - a request's timeout is the client's.

  Requests per call:

  | Call | Requests |
  |---|---|
  | `place` | at most 4: tick size and neg-risk unless the options give them, the post, and the read-back of a `matched` order |
  | `cancel_all(Some)` | 2 |
  | `open_orders` | one per page |
  | `positions` | at most 21 pages |
  | `balances` | 1 |
- **Bounds on what a caller supplies.**
  - The price lies strictly between 0 and 1 on the token's tick.
  - The size has at most two decimal places and is at least 0.01.
  - Membership changes are capped at 500 per connection.
  - The client-id ledger holds at most 10,000 entries.
  - Paging ends on a repeated cursor, and at Data's 10,000 offset.

## 12. Gates and repo rules every story keeps

- **The profile's story gates**, run with `nix develop --command`:
  - fmt, clippy `-D warnings`, the tests, and rustdoc `-D warnings`;
  - `gen_registry.py --check`, `venue_gate.py` and `live_unwraps.py`;
  - `test_body_diff.py --baseline v0.40.0`.

  The final gates add the removals check (`api_removals.py check --baseline v0.40.0`, expecting no
  removal), `cargo hack --each-feature`, the MSRV build, the package dry run and the Python and
  scripts suites.
- **Add tests; never change an existing test's body** (AD-12). New tests are new items: appended to
  a test module, or in a new file. No story edits an existing test, a fixture, or a struct literal
  in one.
  - So `TradeMessage`, `OrderMessage` and `MakerOrder` gain no fields: the CLI's tests build the
    first two as literals (`polyoxide-cli/src/commands/polymarket/clob/stream/user.rs:499,518`,
    `market.rs:533`, `polyoxide-cli/tests/stream_polymarket.rs:243,260`).
  - So the fills mapping (§8.2) reads only the fields those types have.
- **`docs/MUTANTS.md`'s cited lines do not move.**
  - Story 1 appends its tests to `supervisor/tests.rs`.
  - Story 5 leaves `clob/error.rs` alone. `eip712.rs`, which `docs/MUTANTS.md` does not cite,
    gains `order_hash` (§7.2).
  - A story that moves a cited line moves its row and re-proves its mutant.
- **The foundation crates name no venue.** `polyoxide-venue`, `polyoxide-ws` and
  `polyoxide-test-support` docs and tests use made-up venues, and never "clob", "Polymarket",
  "gamma", "perps", "relay" (as an English verb too), "sports", "usdm", "Binance" or "Kalshi"
  (Kalshi becomes a gate term at Story 8.1).
- **Error types.**
  - Every new public error type is in its crate's `Classify` assertion (Story 4:
    `UndeclaredExtension`).
  - Every new error enum is `#[non_exhaustive]`.
  - No module implements `From<reqwest::Error>`, `From<serde_json::Error>` or `From<url::ParseError>`;
    foreign errors enter through `ApiError` (CLAUDE.md).
- **Generated regions are regenerated, never edited:** `python3 scripts/gen_registry.py --write`
  after a metadata change (Stories 4, 5 and 6). `docs/ARCHITECTURE.md` is not edited.
- **Rules CLAUDE.md states** are edited in the story that changes them (AD-21), in its hand-written
  paragraphs only.
- **Docs that are doctests.** README additions are prose with no code fence, since the module and
  crate READMEs are doctests and `check_plan.py` cannot hold a nested fence.
- **Commits** are by pathspec, in conventional form naming the story, with `Cargo.lock` when cargo
  changes it, never `--no-verify`.

## 13. Not in this epic

- Moving the CLI's `polymarket clob stream market|user` onto the supervised sockets.
- Python bindings for the sockets or `Trading` (a non-goal: no new Python coverage).
- `Trading` for perps or Binance: neither has signed routes.
- Pruning `ClobWsError::ConnectionClosed`, which is a removal.
- Removing `WebSocketWithPing` (AD-16: it stays until a named breaking release).
- Merging the spine amendments and regenerating `docs/ARCHITECTURE.md` (an owner session after the
  run).
- Updating `sprint-status.yaml`.
- The release itself.
