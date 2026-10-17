# Epic 7: Supervised CLOB Sockets and Normalized Trading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** Consumers trade Polymarket through one venue-neutral `Trading` interface whose fills arrive losslessly on supervised CLOB sockets: the market and user sockets reconnect, replay their membership and credentials, and mark every outage, and every events stream says when to reconcile (Epic 7, Stories 7.1 to 7.5).

**Architecture:** `polyoxide-ws`'s `Protocol` gains a defaulted `connect`, so the `Supervisor` can run clob's per-address dial (D20). `polyoxide-polymarket`'s `clob::ws::supervised` adds a market and a user `Protocol` on the kit's `Supervisor`, beside the unchanged bare tier. `polyoxide-venue` gains a `trading` feature: the `Trading` trait with its `DynTrading` wrapper, its records, opaque ids and the `TradingEvents` stream, adopting Lane B's reviewed design onto Epic 6's types. `clob::venue` implements `Trading` for `Clob` over the native order, cancel, Data and balance routes and the supervised user socket, gated `all(clob-ws, data)`.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), tokio, tokio-tungstenite through `polyoxide-ws`, dynosaur 0.3.1, futures-core 0.3 (new in `polyoxide-venue`), rust_decimal, serde, mockito, the kit's scripted WebSocket server.

**Spec:** `docs/superpowers/specs/2026-10-17-epic-7-supervised-clob-sockets-and-trading-design.md`, read with `_bmad-output/planning-artifacts/epics.md` (Epic 7) and the architecture spine (AD-3, AD-5, AD-7, AD-11, AD-19, AD-20, AD-24). The spec binds where it and the epic's criteria differ (its §1.5).

---

## Rules for every story

- **Run commands with the dev shell's prefix**, `nix develop --command`. A story's gates are the profile's (the spec's §12); its **Verify** line is the developer's done-check.
- **Put code in place from this file by line range**: a new file is the block after its `Create` line, and each edit replaces the block after a `Current (...)` line with the block after the `Replace with:` line that follows it. Each `Current` block occurs exactly once in its file at that point, and a task's edits apply top to bottom. In CLAUDE.md a `Current` block may be part of one long line.
- **Add tests; never change an existing test's body, a fixture or a struct literal in a test** (AD-12; the test-bodies job runs against `v0.40.0`). So `TradeMessage`, `OrderMessage` and `MakerOrder` gain no fields.
- **`docs/MUTANTS.md`'s cited lines do not move.** Tests in `polyoxide-ws/src/supervisor/tests.rs` are appended after its last test. `polyoxide-polymarket/src/clob/error.rs` is not edited.
- **The foundation crates name no venue** (`python3 scripts/venue_gate.py`). `polyoxide-ws` and `polyoxide-venue` docs and tests use made-up venues (`acme`), and never "clob", "Polymarket", "Kalshi", or "relay" as an English verb.
- **Generated regions are regenerated, never edited.** After a `[package.metadata.polyoxide]` change (Stories 4, 5 and 6), run `python3 scripts/gen_registry.py --write` and commit what it rewrites. `docs/ARCHITECTURE.md` is never edited.
- **README additions are prose with no code fence** (module and crate READMEs are doctests).
- **Commit by pathspec**, with a conventional message naming the story, and include `Cargo.lock` when cargo changes it.

## File structure

**Story 1: the kit dials through the protocol.**

- `polyoxide-ws/src/supervisor/mod.rs` (modify): `Protocol::connect`, a provided method with its doc.
- `polyoxide-ws/src/supervisor/task.rs` (modify): `attempt` calls `protocol.connect`.
- `polyoxide-ws/src/supervisor/tests.rs` (modify, append only): `connect` is used for every attempt, and its error is a failed attempt.
- `polyoxide-polymarket/src/clob/ws/client.rs` (modify): `dial` split out of `connect_ws`; four helpers raised to `pub(super)`.
- `CLAUDE.md` (modify): the socket-kit paragraph names the hook.

**Story 2: the supervised market socket.**

- `polyoxide-polymarket/src/clob/ws/supervised/mod.rs` (create): the module doc, `DisconnectReason`, the shared helpers, and the market module's re-exports.
- `polyoxide-polymarket/src/clob/ws/supervised/market.rs` (create): `ClobMarketProtocol`, `MarketWsBuilder`, `MarketEvent`, `MarketChange`, `MarketMembership`, `SupervisedMarketWs`, and their tests.
- `polyoxide-polymarket/src/clob/ws/scripted.rs` (create, `#[cfg(test)]`): the scripted server's `PING`/`PONG` answerer.
- `polyoxide-polymarket/src/clob/ws/mod.rs` (modify): declares the two modules and re-exports the market items.
- `CLAUDE.md` (modify): the bare tier's sentence, and a paragraph on the market socket.

**Story 3: the supervised user socket.**

- `polyoxide-polymarket/src/clob/ws/supervised/user.rs` (create): `ClobUserProtocol`, `UserWsBuilder`, `UserEvent`, `UserChange`, `UserMembership`, `SupervisedUserWs`, and their tests.
- `polyoxide-polymarket/src/clob/ws/supervised/mod.rs` (modify): `mod user;` and its re-exports.
- `polyoxide-polymarket/src/clob/ws/mod.rs` (modify): re-exports the user items.
- `CLAUDE.md` (modify): a paragraph on the user socket.

**Story 4: the `Trading` trait.**

- `Cargo.toml` (modify): `[workspace.dependencies]` gains `futures-core = "0.3"`.
- `polyoxide-venue/Cargo.toml` (modify): the `trading` feature, the optional `futures-core`, the test dev-dependencies, the docs.rs features, and the metadata `readme` line.
- `polyoxide-venue/src/ids.rs` (create): `OrderId`, `FillId` and `ClientOrderId`.
- `polyoxide-venue/src/trading.rs` (create): the `Trading` trait, `TradingEvents` and the trading records.
- `polyoxide-venue/src/extensions.rs` (modify): `ensure_declared`, `UndeclaredExtension`, and new unit tests.
- `polyoxide-venue/src/lib.rs` (modify): the modules, the re-exports and the `Classify` assertion.
- `polyoxide-venue/tests/trading.rs` (create): static and `Arc<DynTrading<'static>>` proofs.
- `polyoxide-venue/README.md` (modify): a prose line.
- `CLAUDE.md` and `README.md` (modify): the "**The trading trait**" paragraph, and the regenerated crate lists.
- `Cargo.lock`.

**Story 5: `Trading` on the CLOB.**

- `polyoxide-polymarket/src/clob/venue/trading.rs` (create): `impl Trading for Clob`'s request methods, `ClobOrderOptions`, the `Ext` types and the client-id ledger.
- `polyoxide-polymarket/src/clob/venue/fills.rs` (create): the events adapter, from `UserEvent` to `TradingEvent`.
- `polyoxide-polymarket/src/clob/venue.rs` (modify): declares the two modules (gated) and re-exports their public items.
- `polyoxide-polymarket/src/clob/client.rs` (modify): `ClobBuilder::user_socket` and `Clob`'s new fields; `resolve_maker_address` raised to `pub(crate)`.
- `polyoxide-polymarket/src/clob/core/eip712.rs` (modify, in place): `compute_order_digest` raised to `pub(crate)`.
- `polyoxide-polymarket/Cargo.toml` (modify): `clob-ws` turns on `polyoxide-venue/trading`; the `trading_clob` `[[test]]`; the metadata `notes` entry.
- `polyoxide-polymarket/tests/trading_clob.rs` (create): mock and scripted-socket suite.
- `polyoxide-polymarket/src/clob/README.md` (modify): a prose line.
- `polyoxide/Cargo.toml` (modify): `polymarket-clob-ws` turns on `polyoxide-venue/trading`.
- `polyoxide/src/lib.rs` (modify): the `DynTrading` and `Clob: Trading` pins.
- `CLAUDE.md` and `README.md` (modify): the "**The CLOB through `Trading`**" paragraph, and the regenerated crate lists.
- `Cargo.lock`.

**Story 6: live suites.**

- `polyoxide-polymarket/tests/live_clob_ws.rs` (modify, append only): the supervised market and user sockets, live.
- `polyoxide-polymarket/tests/live_clob_trading.rs` (create): `Trading` reads and a post-only place, ack and cancel round trip.
- `polyoxide-polymarket/Cargo.toml` (modify): the `live_clob_trading` `[[test]]` and its live metadata.
- `scripts/live_unwraps.baseline.json` (modify if the script requires an entry for a new live file).
- `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md` (regenerated).

**Story 7: the Kalshi mapping, amendments, release note and records.**

- `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md` (create): the tag's mapping, ported.
- `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md` (create): A7-1 to A7-4 and 7.5's line.
- `docs/s3-migration.md` (modify): Epic 7's consumer-impact section.
- `_bmad-output/implementation-artifacts/deferred-work.md` (modify): close one entry, narrow one, add two.
- `_bmad-output/specs/spec-venue-extensibility/divergences.md` (modify): the clob rows.

## Names contract

Every name one story defines for another, with its type, signature and visibility. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Paths are from the crate root unless they start with a crate name.

**Story 1 → Stories 2, 3 and 5.**

- **`polyoxide_ws::Protocol::connect`.**
  - Signature: `fn connect(&mut self, timeout: Duration) -> impl Future<Output = Result<Socket, Self::Error>> + Send`.
  - A provided method, placed directly after `handshake` in `polyoxide-ws/src/supervisor/mod.rs`.
  - `Socket` is `polyoxide_ws::Socket` (`pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>`, `polyoxide-ws/src/connect.rs:19`).
  - The default calls `self.handshake()` before its async block and dials the request with `crate::connect(request, timeout)`, converting `WsError` with `?`.
  - `attempt` calls `protocol.connect(timing.connect_timeout)`.
- **In `polyoxide-polymarket/src/clob/ws/client.rs`, all `pub(super)`:**
  - `async fn dial(url: &str, connect_timeout: Duration) -> Result<polyoxide_ws::Socket, ClobWsError>`;
  - `fn frame_text(frame: &Message) -> Option<&str>`;
  - `fn validate_subscription_count(count: usize) -> Result<(), ClobWsError>`;
  - `const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 500`;
  - `const DEFAULT_CONNECT_TIMEOUT: Duration` (10 s).

  `connect_ws` keeps its private signature and calls `dial`.

**Story 2 → Stories 3, 5 and 6.**

- **Module.** `clob::ws::supervised` (private: `mod supervised;` in `clob/ws/mod.rs`). Story 2's `clob/ws/mod.rs` adds exactly these three lines, which Story 3 quotes as its anchors:

      mod supervised;
      #[cfg(test)]
      mod scripted;

  It adds this re-export line, which Story 3 leaves unchanged and follows with its own:

      pub use supervised::{ClobMarketProtocol, DisconnectReason, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder, SupervisedMarketWs};

  `supervised/mod.rs` holds `mod market;` and `pub use market::{ClobMarketProtocol, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder, SupervisedMarketWs};` on lines of their own, which Story 3 follows with `mod user;` and its own `pub use user::{…};`.
- **`pub type SupervisedMarketWs = polyoxide_ws::Supervised<ClobMarketProtocol>;`**
- **`pub struct MarketWsBuilder`** (`Debug`, `Clone`, `Default`):
  - `pub fn new() -> Self`;
  - `pub fn url(self, url: impl Into<String>) -> Result<Self, ClobWsError>`;
  - `pub fn options(self, options: MarketSubscriptionOptions) -> Self`;
  - from `polyoxide_ws::timing_setters!`: `ping_interval`, `stale_after`, `backoff`, `connect_timeout`;
  - `pub async fn connect(self, asset_ids: Vec<String>) -> Result<SupervisedMarketWs, ClobWsError>`.
- **`pub struct ClobMarketProtocol`** (private fields) implements `polyoxide_ws::Protocol` with `Event = MarketEvent`, `Error = ClobWsError`, `Membership = Vec<String>`, `Change = MarketChange` and `Ack = ()`.
- **Enums:**
  - `#[non_exhaustive] pub enum MarketEvent { Message(MarketMessage), Disconnected { reason: DisconnectReason }, Reconnected }` (`Debug`);
  - `#[non_exhaustive] pub enum MarketChange { Subscribe(Vec<String>), Unsubscribe(Vec<String>) }` (`Debug`, `Clone`, `PartialEq`, `Eq`);
  - `#[non_exhaustive] pub enum DisconnectReason { Closed { code: Option<u16>, reason: String }, Error(ClobWsError), Stale { silent: Duration }, Rotation }` (`Debug`), defined in `supervised/mod.rs`.
- **`pub trait MarketMembership`** has `fn subscribe(&self, asset_ids: Vec<String>) -> impl Future<Output = Result<(), ClobWsError>> + Send` and `fn unsubscribe(&self, asset_ids: Vec<String>) -> impl Future<Output = Result<(), ClobWsError>> + Send`. It is implemented for `polyoxide_ws::MembershipHandle<ClobMarketProtocol>`.
- **In `supervised/mod.rs`, `pub(super)`:**
  - `fn checked_url(url: String) -> Result<String, ClobWsError>`. It refuses with `ClobWsError::InvalidMessage(format!("the URL must be wss://, or ws:// to a loopback host: {url}"))`.
  - `fn default_timing() -> polyoxide_ws::Timing`: ping 10 s, stale 30 s, backoff 500 ms to 60 s, connect `DEFAULT_CONNECT_TIMEOUT`.
  - `fn disconnect_reason(outage: polyoxide_ws::Outage<ClobWsError>) -> DisconnectReason`.
  - `fn recover(err: &ClobWsError) -> polyoxide_ws::Recovery` (spec D15).
  - `fn data_text(frame: &Message) -> Option<&str>` (spec §4.3, first bullet).
- **In `clob/ws/scripted.rs`** (`#[cfg(test)]`): `pub(crate) fn pong(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send + 'static`, which answers a text `PING` with a text `PONG` and nothing else.
- **CLAUDE.md.** In the socket-kit paragraph (line 724), Story 2 replaces this sentence:

      clob keeps its own `select!`, its 10 s text `PING`, its outbound queue and Binary decoded as text (D12), and is not supervised until Epic 7.

  It inserts its paragraph, beginning `**The supervised CLOB market socket**`, directly before the line that begins:

      **The socket kit is `polyoxide-ws`; perps and Binance run on its `Supervisor` (Stories 4.6 and 4.7), and rtds, sports and clob on its blocks (Stories 4.8 and 4.9).**

  Story 3 inserts its paragraph, beginning `**The supervised CLOB user socket**`, before that same line, so it follows Story 2's and quotes none of it. Story 1's edit in that paragraph replaces only this text, which neither of those anchors overlaps:

      a venue describes its socket as a `Protocol` (a handshake built per attempt, `configure` of each fresh connection,

**Story 3 → Stories 5 and 6.**

- **`pub type SupervisedUserWs = polyoxide_ws::Supervised<ClobUserProtocol>;`**
- **`pub struct UserWsBuilder`** (`Debug`, `Clone`, `Default`; holds no credential):
  - `pub fn new() -> Self`;
  - `pub fn url(self, url: impl Into<String>) -> Result<Self, ClobWsError>`;
  - the four timing setters;
  - `pub async fn connect(self, markets: Option<Vec<String>>, credentials: ApiCredentials) -> Result<SupervisedUserWs, ClobWsError>`;
  - `pub async fn start(self, markets: Option<Vec<String>>, credentials: ApiCredentials) -> Result<SupervisedUserWs, ClobWsError>`.
- **`pub struct ClobUserProtocol`** (private fields) implements `Protocol` with `Event = UserEvent`, `Error = ClobWsError`, `Membership = Option<Vec<String>>`, `Change = UserChange`, `Ack = ()` and `Queue::Unbounded`.
- **Enums and trait:**
  - `#[non_exhaustive] pub enum UserEvent { Message(UserMessage), Disconnected { reason: DisconnectReason }, Reconnected }` (`Debug`);
  - `#[non_exhaustive] pub enum UserChange { Subscribe(Vec<String>), Unsubscribe(Vec<String>) }`;
  - `pub trait UserMembership` with `subscribe(&self, markets: Vec<String>)` and `unsubscribe(&self, markets: Vec<String>)`, each `-> impl Future<Output = Result<(), ClobWsError>> + Send`, implemented for `MembershipHandle<ClobUserProtocol>`.
- **Re-exports.** All are at `clob::ws` (one `pub use supervised::{…}` line after Story 2's) and in `supervised/mod.rs`.
- **Refusal texts:**
  - `ClobWsError::InvalidMessage("the user channel follows every market; unsubscribing needs a market filter".into())`;
  - the user channel's 1008 close is `ClobWsError::Authentication(<the close reason>)`.

**Story 4 → Stories 5, 6 and 7.**

All are in `polyoxide-venue`, re-exported at the crate root.

**Feature `trading = ["market", "dep:futures-core"]`:**

- `#[dynosaur::dynosaur(pub DynTrading = dyn(box) Trading)] pub trait Trading: Send + Sync`, with exactly these methods:
  - `fn place(&self, order: OrderRequest) -> impl Future<Output = Result<Order, ClassifiedError>> + Send`;
  - `fn cancel(&self, id: &OrderId) -> impl Future<Output = Result<(), ClassifiedError>> + Send`;
  - `fn cancel_all(&self, key: Option<&MarketKey>) -> impl Future<Output = Result<Vec<OrderId>, ClassifiedError>> + Send`;
  - `fn open_orders(&self, key: Option<&MarketKey>) -> impl Future<Output = Result<Vec<Order>, ClassifiedError>> + Send`;
  - `fn positions(&self) -> impl Future<Output = Result<Vec<Position>, ClassifiedError>> + Send`;
  - `fn balances(&self) -> impl Future<Output = Result<Vec<Balance>, ClassifiedError>> + Send`;
  - `fn events(&self) -> TradingEvents`.
- `pub struct TradingEvents(Pin<Box<dyn Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static>>)` (private field). It has `pub fn new(stream: impl Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static) -> Self` and `impl futures_core::Stream` (`poll_next`, `size_hint`).
- Enums:
  - `TimeInForce { #[default] GoodTilCancelled, ImmediateOrCancel, FillOrKill }` (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Default`; `#[non_exhaustive]`);
  - `KillReason { Unmatched, Unfillable }` and `OrderStatus { Open, Filled, Cancelled, Killed { reason: KillReason } }` (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`; `#[non_exhaustive]`);
  - `Liquidity { Maker, Taker }` (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`; `#[non_exhaustive]`);
  - `TradingEvent { Ack(Order), Fill(Fill), Resync }` (`Debug`, `Clone`, `PartialEq`; `#[non_exhaustive]`).
- Records, each `#[non_exhaustive]` with every field `pub` and deriving `Debug`, `Clone` and `PartialEq`:
  - `OrderRequest { key: MarketKey, side: Side, price: Decimal, size: Decimal, time_in_force: TimeInForce, client_order_id: Option<ClientOrderId>, post_only: bool, reduce_only: bool, extensions: Extensions }`, with `pub fn new(key: MarketKey, side: Side, price: Decimal, size: Decimal) -> Self` (the default time in force, no client id, both flags false, no extensions).
  - `Order { id: OrderId, key: MarketKey, side: Side, price: Decimal, size: Decimal, filled: Decimal, status: OrderStatus, time_in_force: Option<TimeInForce>, client_order_id: Option<ClientOrderId>, created: Option<UnixMillis>, extensions: Extensions }`, with `pub fn new(id: OrderId, key: MarketKey, side: Side, price: Decimal, size: Decimal, status: OrderStatus) -> Self` (`filled` zero, the rest `None` or empty).
  - `Fill { id: FillId, order_id: OrderId, trade_id: Option<TradeId>, key: MarketKey, side: Side, price: Decimal, size: Decimal, notional: Option<Decimal>, fee: Option<Decimal>, liquidity: Option<Liquidity>, client_order_id: Option<ClientOrderId>, time: UnixMillis, extensions: Extensions }`, with `pub fn new(id: FillId, order_id: OrderId, key: MarketKey, side: Side, price: Decimal, size: Decimal, time: UnixMillis) -> Self`.
  - `Position { key: MarketKey, size: Decimal, average_price: Option<Decimal>, extensions: Extensions }`, with `pub fn new(key: MarketKey, size: Decimal) -> Self`.
  - `Balance { asset: String, total: Option<Decimal>, available: Option<Decimal>, extensions: Extensions }`, with `pub fn new(asset: impl Into<String>) -> Self`.
- `OrderId`, `FillId` and `ClientOrderId` (`ids.rs`): each `#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct X(Arc<str>)` with `pub fn new(id: impl Into<Arc<str>>) -> Self`, `pub fn as_str(&self) -> &str`, `Display`, `From<&str>` and `From<String>`.

**Feature `market` (where `Extensions` lives):**

- `impl Extensions { pub fn ensure_declared(&self, declared: &[TypeId]) -> Result<(), UndeclaredExtension> }`.
- `#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] pub struct UndeclaredExtension { pub type_name: &'static str }`. It displays as ``the request carries `{type_name}`, an extension this venue does not declare``, implements `Error`, and is `Classify` as `Class::InvalidRequest`.

**Used unchanged from Epic 6:** `ClassifiedError`, `Class`, `Classify`, `Extensions`, `MarketKey`, `KeySpace`, `Side`, `TradeId` (in `records.rs`, `market`) and `UnixMillis`.

**CLAUDE.md.** Story 4 inserts its paragraph, beginning `**The trading trait**`, directly before the line beginning `**The CLOB through the traits**`.

**Story 5 → Stories 6 and 7.**

- **The impl.** `impl polyoxide_venue::Trading for crate::clob::Clob`, in `src/clob/venue/trading.rs`. Its events adapter is in `src/clob/venue/fills.rs`. `src/clob/venue.rs` declares both as `#[cfg(all(feature = "clob-ws", feature = "data"))] mod trading;` and `… mod fills;`.
- **Public, re-exported from `clob::venue`** under the same gate:
  - `ClobOrderOptions { pub neg_risk: Option<bool>, pub tick_size: Option<TickSize>, pub expiration: Option<UnixMillis> }` (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Default`; `#[non_exhaustive]`);
  - `OrderExt`, `FillExt`, `PositionExt` and `BalanceExt` (spec §7.7).
- **`impl ClobBuilder { pub fn user_socket(self, socket: crate::clob::ws::UserWsBuilder) -> Self }`** under `#[cfg(feature = "clob-ws")]`. `Clob` holds `user_socket: UserWsBuilder` (`clob-ws`) and the client-id ledger (D6's gate).
- **Crate-private:** `Clob::resolve_maker_address` and `clob::core::eip712::compute_order_digest` become `pub(crate)`, their signatures unchanged.
- **Values the live suite matches:** the collateral `Balance`'s `asset` is `"pUSD"`; the leading `TradingEvent::Resync` (spec §8.1).
- **Umbrella.** The pins in `polyoxide/src/lib.rs` follow the existing `DynMarketData` pin: `#[cfg(feature = "polymarket-clob-ws")] const _: fn(&crate::venue::DynTrading<'static>) = |_| {};`, and a `Clob: Trading` check under `all(feature = "polymarket-clob-ws", feature = "polymarket-data")`.
- **CLAUDE.md.** Story 5 inserts its paragraph, beginning ``**The CLOB through `Trading`**``, directly before the line beginning `**Wire enums**`.

**Story 7's texts.**

Story 7 writes only new files and new sections. In `docs/s3-migration.md` it appends a section headed `## Epic 7: supervised CLOB sockets and normalized trading`.

## Stories

- **Story 1** — The kit dials through the protocol.

  `polyoxide_ws::Protocol` gains the provided `connect` (spec D7), which the `Supervisor`'s `attempt` calls for every attempt. Its default keeps today's behaviour, so perps and Binance are untouched. clob's per-address loop moves out of `connect_ws` into `dial`, so Stories 2 and 3 can override `connect` with it (D20), and four clob helpers become `pub(super)`. `polyoxide-ws` is a foundation crate: its new doc and tests name no venue. Its tests are appended after `supervisor/tests.rs`'s last test, so the lines `docs/MUTANTS.md` cites stay put.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/supervisor/tests.rs`, `polyoxide-polymarket/src/clob/ws/client.rs`, `CLAUDE.md`
  - **Interfaces:** exposes `polyoxide_ws::Protocol::connect`, and in clob `pub(super)` `dial`, `frame_text`, `validate_subscription_count`, `MAX_SUBSCRIPTIONS_PER_CONNECTION` and `DEFAULT_CONNECT_TIMEOUT` (names contract, Story 1); uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-ws --all-features --lib supervisor && cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`
  - **Review:** deep (a foundation crate's public trait gains a method)
  - **Reading:** spec §2 (D7), §3, §12; `polyoxide-ws/src/supervisor/mod.rs:120-275`, `polyoxide-ws/src/supervisor/task.rs:175-210`, `polyoxide-ws/src/supervisor/tests.rs:1-120,990-1095`, `polyoxide-ws/src/connect.rs:1-70`, `polyoxide-polymarket/src/clob/ws/client.rs:1-210`; CLAUDE.md's socket-kit paragraph (line 724)

- **Story 2** — The supervised CLOB market socket (Story 7.1).

  Adds `clob::ws::supervised` with `ClobMarketProtocol` on the kit's `Supervisor` (spec §4). It reconnects after a close or a stall, replays the membership on every connection, and yields one `Disconnected` and one `Reconnected` per outage. It sends the 10 s text `PING`, skips `PONG`, and decodes Binary as text. It yields every book of an array frame (D11), and keeps an empty membership connected. It also adds the shared helpers and `DisconnectReason` that Story 3 reuses, and the `#[cfg(test)]` `PONG` answerer. The bare tier is not edited, and its existing unit tests show `WebSocketWithPing` unchanged. Recovery (D15) reads `Classify` and never matches over `tungstenite::Error`'s variants.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/clob/ws/mod.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`, `polyoxide-polymarket/src/clob/ws/supervised/market.rs`, `polyoxide-polymarket/src/clob/ws/scripted.rs`, `CLAUDE.md`
  - **Interfaces:** exposes `clob::ws::{SupervisedMarketWs, MarketWsBuilder, ClobMarketProtocol, MarketEvent, MarketChange, MarketMembership, DisconnectReason}`, and crate-private `supervised::{checked_url, default_timing, disconnect_reason, recover, data_text}` and `scripted::pong` (names contract, Story 2); uses Story 1's `Protocol::connect`, `dial`, `frame_text`, `validate_subscription_count`, `MAX_SUBSCRIPTIONS_PER_CONNECTION` and `DEFAULT_CONNECT_TIMEOUT`
  - **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`
  - **Review:** deep (new supervised socket on the kit)
  - **Reading:** spec §2 (D11 to D16), §4, §11, §12; `polyoxide-polymarket/src/clob/ws/mod.rs`, `polyoxide-polymarket/src/clob/ws/client.rs:120-210,1040-1100,1247-1381`, `polyoxide-polymarket/src/clob/ws/subscription.rs:60-130,180-275`, `polyoxide-polymarket/src/clob/ws/market.rs:215-260`, `polyoxide-polymarket/src/clob/ws/error.rs:1-85`, `polyoxide-polymarket/src/perps/ws/protocol.rs:1-400`, `polyoxide-polymarket/src/perps/ws/builder.rs:1-135`, `polyoxide-polymarket/src/perps/ws/scripted.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs:150-332`, `polyoxide-ws/src/test_server/mod.rs:50-110,260-290`, `polyoxide-ws/src/test_server/helpers.rs`; CLAUDE.md lines 722-724

- **Story 3** — The supervised CLOB user socket (Story 7.2).

  Adds `ClobUserProtocol` (spec §5). Its membership is `Option<Vec<String>>`: `None` subscribes every market, an empty `Some` subscribes none, and a test pins that it never collapses into `None`. It rebuilds the credential-bearing subscribe frame on every connection, declares `Disconnected`, and has an unbounded queue, so a blocked consumer loses no frame. It stops on the server's 1008 authentication close (D10), which a live probe observed on 2026-10-17. It builds on Story 2's files and helpers.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`, `polyoxide-polymarket/src/clob/ws/mod.rs`, `CLAUDE.md`
  - **Interfaces:** exposes `clob::ws::{SupervisedUserWs, UserWsBuilder, ClobUserProtocol, UserEvent, UserChange, UserMembership}` (names contract, Story 3); uses Story 2's `DisconnectReason`, `checked_url`, `default_timing`, `disconnect_reason`, `recover`, `data_text` and `scripted::pong`, and Story 1's `dial` and count helpers
  - **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`
  - **Review:** deep (credentials replayed; an unbounded queue)
  - **Reading:** spec §1.4, §2 (D10, D12 to D16), §5, §11, §12; `polyoxide-polymarket/src/clob/ws/subscription.rs:135-215`, `polyoxide-polymarket/src/clob/ws/user.rs:1-206`, `polyoxide-polymarket/src/clob/ws/auth.rs:15-95`, `polyoxide-polymarket/src/clob/ws/error.rs:1-85`, `polyoxide-ws/src/supervisor/task.rs:495-545`; Story 2's entry and its files in this plan

- **Story 4** — The `Trading` trait (Story 7.3).

  Adds `polyoxide-venue`'s `trading` feature (spec §6). It holds the `Trading` trait with `DynTrading`, and `TradingEvents` (A7-1), the newtype that keeps the stream `'static` through the dyn wrapper. It holds the trading records and enums, and the opaque `OrderId`, `FillId` and `ClientOrderId`. And it adds `Extensions::ensure_declared` with `UndeclaredExtension`, the refusal a venue runs on an undeclared option. The design is Lane B's (tag `lane-b-bundle-n`, read with `git show`, never checked out), ported onto Epic 6's types, with D3's leading `Resync` and D4's void rule in the docs. Its tests prove static and `Arc<DynTrading<'static>>` use, and a stream that outlives its `Arc` on another thread. Everything is in a foundation crate, so docs and tests use the made-up venue `acme`.

  - **Developer:** intermediate-dev
  - **Files:** `Cargo.toml`, `polyoxide-venue/Cargo.toml`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/src/ids.rs`, `polyoxide-venue/src/trading.rs`, `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/tests/trading.rs`, `polyoxide-venue/README.md`, `CLAUDE.md`, `README.md`, `Cargo.lock`
  - **Interfaces:** exposes, under `polyoxide-venue/trading`, `Trading`, `DynTrading`, `TradingEvents`, `TradingEvent`, `OrderRequest`, `Order`, `OrderStatus`, `KillReason`, `TimeInForce`, `Fill`, `Liquidity`, `Position`, `Balance`, `OrderId`, `FillId` and `ClientOrderId`, and under `market` `Extensions::ensure_declared` and `UndeclaredExtension` (names contract, Story 4); uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-venue --all-features && cargo hack check -p polyoxide-venue --each-feature --no-dev-deps`
  - **Review:** deep (the public trait every trading venue implements)
  - **Reading:** spec §1.3, §2 (D1 to D5), §6, §12; `git show lane-b-bundle-n:polyoxide-venue/src/trading.rs`, `git show lane-b-bundle-n:polyoxide-venue/src/ids.rs`, `git show lane-b-bundle-n:polyoxide-venue/tests/trading.rs`; `polyoxide-venue/Cargo.toml`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/src/market.rs:1-175`, `polyoxide-venue/src/records.rs:1-60,150-200`, `polyoxide-venue/src/key.rs:295-372`, `polyoxide-venue/src/class.rs:140-220`, `polyoxide-venue/README.md`; CLAUDE.md lines 331-366

- **Story 5** — `Trading` on the Polymarket CLOB (Story 7.4).

  Implements `Trading` for `Clob`, gated `all(clob-ws, data)` (spec §7, §8). `place` runs its client-side checks with nothing sent, then signs and posts. It maps FAK and FOK kills, matched before any conversion, to `Ok` with `Killed { reason }`, and reads back a matched order's fill. `cancel`, `cancel_all`, `open_orders`, `positions` (Data v1, no dust filter) and `balances` (pUSD, 6 decimals) follow the spec. `events()` runs on Story 3's supervised user socket: a leading `Resync`, `Resync` after every reconnect and on a `FAILED` trade, and one `Fill` per account order in a `MATCHED` trade. The in-process client-id ledger is keyed by the order's EIP-712 hash, computed before sending. Leaves `clob/error.rs` alone, since `docs/MUTANTS.md` cites its lines.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/src/clob/venue/trading.rs`, `polyoxide-polymarket/src/clob/venue/fills.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/trading_clob.rs`, `polyoxide-polymarket/src/clob/README.md`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `CLAUDE.md`, `README.md`, `Cargo.lock`
  - **Interfaces:** exposes `impl Trading for clob::Clob`, `clob::venue::{ClobOrderOptions, OrderExt, FillExt, PositionExt, BalanceExt}` and `ClobBuilder::user_socket` (names contract, Story 5); uses Story 3's `UserWsBuilder::start`, `UserEvent` and `ApiCredentials`, and Story 4's trading items and `Extensions::ensure_declared`
  - **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob && cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue && cargo check -p polyoxide --no-default-features --features polymarket-clob-ws,polymarket-data && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py -q'`
  - **Review:** deep (places and cancels orders)
  - **Reading:** spec §2, §7, §8, §11, §12; `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/src/clob/client.rs:40-200,300-760,800-980`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/api/account.rs:15-60,315-330`, `polyoxide-polymarket/src/clob/core/eip712.rs:165-225`, `polyoxide-polymarket/src/clob/error.rs:1-150`, `polyoxide-polymarket/src/clob/types.rs:140-220,280-340`, `polyoxide-polymarket/src/clob/ws/user.rs:1-206`, `polyoxide-polymarket/src/data/api/users.rs:15-115`, `polyoxide-polymarket/src/data/types.rs:520-590`, `polyoxide-polymarket/src/shared/keys.rs`, `polyoxide-polymarket/tests/mock_clob.rs:1-60,3320-3440`, `polyoxide-polymarket/tests/venue_clob.rs:1-120`, `polyoxide-polymarket/Cargo.toml`, `polyoxide/src/lib.rs:225-295`, `polyoxide/Cargo.toml`; CLAUDE.md lines 366-385

- **Story 6** — Live suites for the supervised sockets and `Trading`.

  Appends live tests to `live_clob_ws.rs`: the supervised market socket receives a live book, and the supervised user socket stays connected with the account's credentials. It adds the `live_clob_trading` target (spec §9). That target reads through `Arc<DynTrading<'static>>`, then runs a post-only 0.01 bid of 5 shares: it rests, the stream acks it after its leading `Resync`, and the cancel is acked. The bid moves no capital. All tests are `#[ignore]`d and fail through test-support's tags, auth-gated without credentials and environmental when no token qualifies. The new target's metadata regenerates the nightly regions.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/tests/live_clob_ws.rs`, `polyoxide-polymarket/tests/live_clob_trading.rs`, `polyoxide-polymarket/Cargo.toml`, `scripts/live_unwraps.baseline.json`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
  - **Interfaces:** uses Stories 2, 3 and 5's public items (names contract) and the `"pUSD"` asset; exposes nothing
  - **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma --test live_clob_trading && cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --test live_clob_ws && python3 scripts/live_unwraps.py && python3 scripts/gen_registry.py --check`
  - **Review:** deep (live tests that place and cancel a real order)
  - **Reading:** spec §9, §12; `polyoxide-polymarket/tests/live_clob.rs:60-180,1040-1263`, `polyoxide-polymarket/tests/live_clob_ws.rs`, `polyoxide-polymarket/Cargo.toml:300-330,528-545`, `scripts/live_unwraps.py`, `scripts/live_unwraps.baseline.json`, `polyoxide-test-support/src/tag.rs:90-175`; CLAUDE.md's "Testing Conventions" and "Nightly API Smoketest" sections

- **Story 7** — The Kalshi trading mapping, the amendments, the release note and the records (Story 7.5).

  Ports Lane B's Kalshi mapping onto this epic's trait, writing the whole document as a new file at its planning-companion path (spec §10, D9). It writes `spine-amendments/epic-7.md` with A7-1 to A7-4 and "7.5: no amendment". It adds Epic 7's consumer-impact section to `docs/s3-migration.md` (NFR11). It updates `deferred-work.md` and the clob rows of `divergences.md`. Documents only: no code.

  - **Developer:** junior-dev
  - **Files:** `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md`, `docs/s3-migration.md`, `_bmad-output/implementation-artifacts/deferred-work.md`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`
  - **Interfaces:** uses the names of Stories 1 to 5 in prose; exposes nothing
  - **Verify:** `python3 scripts/gen_registry.py --check && git diff --check HEAD~1 HEAD`
  - **Review:** light (planning documents and a release note; no code)
  - **Reading:** spec §1.5, §2, §10; `git show lane-b-bundle-n:_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `git show lane-b-bundle-n:_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md`; `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`, `docs/s3-migration.md`, `_bmad-output/implementation-artifacts/deferred-work.md:260-340`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->
