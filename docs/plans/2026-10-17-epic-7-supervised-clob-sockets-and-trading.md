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
- **A bare `D<n>` in code, docs, READMEs, CLAUDE.md and `docs/s3-migration.md` names a row of `divergences.md`**, as CLAUDE.md already uses it (D8, D12, D19, D20). The spec's decisions D1 to D16 share those numbers, so those texts state a spec decision in words, or name the `divergences.md` or DRIFT row it concerns, and never cite it as `D<n>`. Story 7's planning documents (the mapping, the amendments and the deferred-work entries) may cite one as "spec D<n>".
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
- `polyoxide-polymarket/src/clob/core/eip712.rs` (modify): `order_hash`, which `compute_order_digest` calls after its type-3 refusal, and its tests (appended).
- `polyoxide-polymarket/Cargo.toml` (modify): `clob-ws` turns on `polyoxide-venue/trading`; the `trading_clob` `[[test]]`; the metadata `notes` entry.
- `polyoxide-polymarket/tests/trading_clob.rs` (create): mock and scripted-socket suite.
- `polyoxide-polymarket/src/clob/README.md` (modify): a prose line.
- `polyoxide/Cargo.toml` (modify): `polymarket-clob-ws` turns on `polyoxide-venue/trading`.
- `polyoxide/src/lib.rs` (modify): the `DynTrading` and `Clob: Trading` pins.
- `CLAUDE.md` (modify): the "**The CLOB through `Trading`**" paragraph, `user_socket` in the "Builder pattern" list of clob's own knobs, and the regenerated crate list and umbrella-features region (README.md's crate list reads only `readme`, which this story does not change).
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

- **Module.** `clob::ws::supervised` (private: `mod supervised;` in `clob/ws/mod.rs`). rustfmt sorts the `mod` and `use` items of a group (a run of items with no blank line between them) and wraps past 100 columns, so each text below is in the form `cargo fmt` leaves (checked with rustfmt, edition 2021).
  - In `clob/ws/mod.rs`, Story 2 replaces the module block, `mod auth;` to `mod user;` (lines 117-122), with:

        mod auth;
        mod client;
        mod error;
        mod market;
        #[cfg(test)]
        mod scripted;
        mod subscription;
        mod supervised;
        mod user;

    It inserts this block between the `pub use subscription::{…};` block and `pub use user::{…};`:

        pub use supervised::{
            ClobMarketProtocol, DisconnectReason, MarketChange, MarketEvent, MarketMembership,
            MarketWsBuilder, SupervisedMarketWs,
        };

    Story 3 quotes that block as its `Current`, leaves it unchanged, and follows it with its own, so its `Replace with:` is that block and then:

        pub use supervised::{
            ClobUserProtocol, SupervisedUserWs, UserChange, UserEvent, UserMembership, UserWsBuilder,
        };

  - In `supervised/mod.rs`, Story 2 writes this block as a group of its own, with a blank line before and after it:

        mod market;

        pub use market::{
            ClobMarketProtocol, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder,
            SupervisedMarketWs,
        };

    Story 3 quotes it as its `Current` and replaces it with:

        mod market;
        mod user;

        pub use market::{
            ClobMarketProtocol, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder,
            SupervisedMarketWs,
        };
        pub use user::{
            ClobUserProtocol, SupervisedUserWs, UserChange, UserEvent, UserMembership, UserWsBuilder,
        };
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
  - `#[non_exhaustive] pub enum UserChange { Subscribe(Vec<String>), Unsubscribe(Vec<String>) }` (`Debug`, `Clone`, `PartialEq`, `Eq`, as `MarketChange`);
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
  - `ClobOrderOptions { pub neg_risk: Option<bool>, pub tick_size: Option<TickSize>, pub expiration: Option<UnixMillis> }` (`Debug`, `Clone`, `Copy`, `PartialEq`, `Default`; `#[non_exhaustive]`; no `Eq`, since `TickSize` derives none);
  - `OrderExt`, `FillExt`, `PositionExt` and `BalanceExt` (spec §7.7).
- **`impl ClobBuilder { pub fn user_socket(self, socket: crate::clob::ws::UserWsBuilder) -> Self }`** under `#[cfg(feature = "clob-ws")]`. `Clob` holds `user_socket: UserWsBuilder` (`clob-ws`) and the client-id ledger (D6's gate).
- **Crate-private:** `Clob::resolve_maker_address` becomes `pub(crate)`, its signature unchanged. `clob::core::eip712` gains `pub(crate) fn order_hash(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError>`, the order's id for every signature type (spec §7.2); `compute_order_digest` stays private. `place` signs through `Clob::sign_order`, so a Deposit Wallet (type 3) account is supported.
- **Values the live suite matches:** the collateral `Balance`'s `asset` is `"pUSD"`; the leading `TradingEvent::Resync` (spec §8.1).
- **Signature type** (spec §7.4, §7.5, §8.1). `place` passes `Clob`'s `signature_type` (`ClobBuilder::signature_type`, by default the account target's) as the order's own, with no funder, and `positions` and the events adapter resolve the maker with `resolve_maker_address(None, <that signature type>, account)`, as `balances` reads with it. So Story 6's live client, built with `ClobBuilder::signature_type(SignatureType::PolyProxy)` and the `gamma` feature, places, lists and matches fills as its proxy wallet.
- **Umbrella.** The pins in `polyoxide/src/lib.rs` follow the existing `DynMarketData` pin: `#[cfg(feature = "polymarket-clob-ws")] const _: fn(&crate::venue::DynTrading<'static>) = |_| {};`, and a `Clob: Trading` check under `all(feature = "polymarket-clob-ws", feature = "polymarket-data")`.
- **CLAUDE.md.** Story 5 inserts its paragraph, beginning ``**The CLOB through `Trading`**``, directly before the line beginning `**Wire enums**`.

**Story 7's texts.**

Story 7 writes only new files and new sections. In `docs/s3-migration.md` it appends a section headed `## Epic 7: supervised CLOB sockets and normalized trading`.

**Design stage:** /flow-draft session 3d849835-a909-47db-a108-48b400a70ce9, 2026-10-17T19:38:47Z to 2026-10-17T20:53:30Z UTC: $41.09 logged, $46.80 with estimated thinking; 11954 plan lines, $3.44 per 1,000 lines.

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

  Adds `polyoxide-venue`'s `trading` feature (spec §6). It holds the `Trading` trait with `DynTrading`, and `TradingEvents` (A7-1), the newtype that keeps the stream `'static` through the dyn wrapper. It holds the trading records and enums, and the opaque `OrderId`, `FillId` and `ClientOrderId`. And it adds `Extensions::ensure_declared` with `UndeclaredExtension`, the refusal a venue runs on an undeclared option. The design is Lane B's (tag `lane-b-bundle-n`, read with `git show`, never checked out), ported onto Epic 6's types, with spec D3's leading `Resync` and spec D4's void rule in the docs. Its tests prove static and `Arc<DynTrading<'static>>` use, and a stream that outlives its `Arc` on another thread. Everything is in a foundation crate, so docs and tests use the made-up venue `acme`.

  - **Developer:** intermediate-dev
  - **Files:** `Cargo.toml`, `polyoxide-venue/Cargo.toml`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/src/ids.rs`, `polyoxide-venue/src/trading.rs`, `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/tests/trading.rs`, `polyoxide-venue/README.md`, `CLAUDE.md`, `README.md`, `Cargo.lock`
  - **Interfaces:** exposes, under `polyoxide-venue/trading`, `Trading`, `DynTrading`, `TradingEvents`, `TradingEvent`, `OrderRequest`, `Order`, `OrderStatus`, `KillReason`, `TimeInForce`, `Fill`, `Liquidity`, `Position`, `Balance`, `OrderId`, `FillId` and `ClientOrderId`, and under `market` `Extensions::ensure_declared` and `UndeclaredExtension` (names contract, Story 4); uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-venue --all-features && cargo hack check -p polyoxide-venue --each-feature --no-dev-deps`
  - **Review:** deep (the public trait every trading venue implements)
  - **Reading:** spec §1.3, §2 (D1 to D5), §6, §12; `git show lane-b-bundle-n:polyoxide-venue/src/trading.rs`, `git show lane-b-bundle-n:polyoxide-venue/src/ids.rs`, `git show lane-b-bundle-n:polyoxide-venue/tests/trading.rs`; `polyoxide-venue/Cargo.toml`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/src/market.rs:1-175`, `polyoxide-venue/src/records.rs:1-60,150-200`, `polyoxide-venue/src/key.rs:295-372`, `polyoxide-venue/src/class.rs:140-220`, `polyoxide-venue/README.md`; CLAUDE.md lines 331-366

- **Story 5** — `Trading` on the Polymarket CLOB (Story 7.4).

  Implements `Trading` for `Clob`, gated `all(clob-ws, data)` (spec §7, §8). `place` runs its client-side checks with nothing sent, then signs and posts. It maps FAK and FOK kills, matched before any conversion, to `Ok` with `Killed { reason }`, and reads back a matched order's fill. `cancel`, `cancel_all`, `open_orders`, `positions` (Data v1, no dust filter) and `balances` (pUSD, 6 decimals) follow the spec. `events()` runs on Story 3's supervised user socket: a leading `Resync`, `Resync` after every reconnect and on a `FAILED` trade, and one `Fill` per account order in a `MATCHED` trade. The in-process client-id ledger is keyed by the order's EIP-712 hash, computed before sending. Leaves `clob/error.rs` alone, since `docs/MUTANTS.md` cites its lines.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/src/clob/venue/trading.rs`, `polyoxide-polymarket/src/clob/venue/fills.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/trading_clob.rs`, `polyoxide-polymarket/src/clob/README.md`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `CLAUDE.md`, `Cargo.lock`
  - **Interfaces:** exposes `impl Trading for clob::Clob`, `clob::venue::{ClobOrderOptions, OrderExt, FillExt, PositionExt, BalanceExt}` and `ClobBuilder::user_socket` (names contract, Story 5); uses Story 3's `UserWsBuilder::start`, `UserEvent` and `ApiCredentials`, and Story 4's trading items and `Extensions::ensure_declared`
  - **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob && cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue && cargo check -p polyoxide --no-default-features --features polymarket-clob-ws,polymarket-data && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py -q'`
  - **Review:** deep (places and cancels orders)
  - **Reading:** spec §2, §7, §8, §11, §12; `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/src/clob/client.rs:40-200,300-760,800-980`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/api/account.rs:15-60,315-330`, `polyoxide-polymarket/src/clob/core/eip712.rs:165-225`, `polyoxide-polymarket/src/clob/error.rs:1-150`, `polyoxide-polymarket/src/clob/types.rs:140-220,280-340`, `polyoxide-polymarket/src/clob/ws/user.rs:1-206`, `polyoxide-polymarket/src/data/api/users.rs:15-115`, `polyoxide-polymarket/src/data/types.rs:520-590`, `polyoxide-polymarket/src/shared/keys.rs`, `polyoxide-polymarket/tests/mock_clob.rs:1-60,3320-3440`, `polyoxide-polymarket/tests/venue_clob.rs:1-120`, `polyoxide-polymarket/Cargo.toml`, `polyoxide/src/lib.rs:225-294`, `polyoxide/Cargo.toml`; CLAUDE.md lines 366-385

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

## Story 1: The kit dials through the protocol

- **Developer:** intermediate-dev
- **Files:** `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/supervisor/tests.rs`, `polyoxide-polymarket/src/clob/ws/client.rs`, `CLAUDE.md`
- **Interfaces:** exposes `polyoxide_ws::Protocol::connect`, and in clob `pub(super)` `dial`, `frame_text`, `validate_subscription_count`, `MAX_SUBSCRIPTIONS_PER_CONNECTION` and `DEFAULT_CONNECT_TIMEOUT` (names contract, Story 1); uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-ws --all-features --lib supervisor && cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`
- **Review:** deep (a foundation crate's public trait gains a method)

`polyoxide_ws::Protocol` gains the provided method `connect` (spec D7), placed directly after `handshake`, which the `Supervisor`'s `attempt` now calls for every attempt, the first and each reconnect, with `Timing::connect_timeout`. Its default builds `handshake()`'s request before its async block and dials it with the kit's `connect`, so perps' and Binance's protocols, which keep the default, behave as before. clob's per-address loop moves out of `connect_ws` into `dial`, which returns the upgraded `Socket`, so Stories 2 and 3 can override `connect` with it (D20); `connect_ws` keeps its private signature and wraps the socket in the bare tier's `Connection`. Four clob helpers (`frame_text`, `validate_subscription_count`, `MAX_SUBSCRIPTIONS_PER_CONNECTION`, `DEFAULT_CONNECT_TIMEOUT`) become `pub(super)`. `polyoxide-ws` is a foundation crate, so its new doc and tests name no venue, and its tests are appended after `supervisor/tests.rs`'s last test so the lines `docs/MUTANTS.md` cites (`tests.rs:753`, `:775`, `:795`) stay put. CLAUDE.md's socket-kit paragraph names the hook in the same commit as the code (AD-21). No landmine marker appears in this story's code: `connect_async(` is not called, and the existing `connect_over` and `with_timeout` calls move unchanged.

### Task 1.1: `Protocol::connect`, the attempt that calls it, and its tests

**Files:**
- Modify: `polyoxide-ws/src/supervisor/mod.rs:17-20,144-146`
- Modify: `polyoxide-ws/src/supervisor/task.rs:17,183-201`
- Test: `polyoxide-ws/src/supervisor/tests.rs` (append after line 1095)
- Modify: `CLAUDE.md:724`

- [ ] **Step 1: Write the failing tests**

Append this block to the end of `polyoxide-ws/src/supervisor/tests.rs`, after the last test, which ends with these lines (do not edit any line above them):

```rust
    assert_eq!(Plain.retry_after(&refused), None);
}
```

The appended block:

```rust
use std::{
    ops::Range,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

/// A protocol that dials for itself, as a venue that tries each address of
/// its host in turn does. Its `handshake` fails if anything calls it, so only
/// a supervisor that dials through `connect` gets a connection. Its `connect`
/// opens the scripted server, counts every dial, and fails the dials numbered
/// in `failing` (the first is 0) with `failure`. The rest is the file's
/// `Feed`.
struct Dialer {
    feed: Feed,
    dials: Arc<AtomicUsize>,
    failing: Range<usize>,
    failure: fn() -> TestError,
}

fn dialer(
    server: &ScriptedServer,
    dials: &Arc<AtomicUsize>,
    failing: Range<usize>,
    failure: fn() -> TestError,
) -> Dialer {
    Dialer {
        feed: feed(server),
        dials: Arc::clone(dials),
        failing,
        failure,
    }
}

impl Protocol for Dialer {
    type Event = Ev;
    type Error = TestError;
    type Membership = Topics;
    type Change = Change;
    type Ack = usize;

    fn handshake(&mut self) -> Result<Request, TestError> {
        Err(TestError::Refused(
            "the supervisor called handshake itself".into(),
        ))
    }

    fn connect(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<crate::Socket, TestError>> + Send {
        let dial = self.dials.fetch_add(1, Ordering::SeqCst);
        let failure = self.failing.contains(&dial).then_some(self.failure);
        let url = self.feed.url.clone();
        async move {
            if let Some(failure) = failure {
                return Err(failure());
            }
            let request = url.as_str().into_client_request().map_err(WsError::from)?;
            let socket = crate::connect(request, timeout).await?;
            Ok::<_, TestError>(socket)
        }
    }

    async fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &Topics,
    ) -> Result<(), TestError> {
        self.feed.replay(connection, membership).await
    }

    async fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut Topics,
        change: Change,
    ) -> Result<usize, TestError> {
        self.feed.apply(connection, membership, change).await
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Ev, TestError> {
        self.feed.decode(frame)
    }

    fn is_live(&self, frame: &Message, decoded: &Decoded<Ev, TestError>) -> bool {
        self.feed.is_live(frame, decoded)
    }

    fn delivered(&self, frame: &Message, decoded: &Decoded<Ev, TestError>) -> bool {
        self.feed.delivered(frame, decoded)
    }

    fn ping(&mut self, connection: &mut Connection) -> Option<Message> {
        self.feed.ping(connection)
    }

    fn is_retriable(err: &TestError) -> bool {
        <Feed as Protocol>::is_retriable(err)
    }

    fn disconnected(&mut self, outage: Outage<TestError>) -> Option<Ev> {
        self.feed.disconnected(outage)
    }

    fn reconnected(&mut self) -> Ev {
        self.feed.reconnected()
    }

    fn queue(&self) -> Queue {
        self.feed.queue()
    }
}

#[tokio::test]
async fn connect_dials_every_attempt_and_the_stream_reads_its_socket() {
    // Two connections: the first closes after one event, the second serves
    // another. `Dialer::handshake` fails if called, so every frame read here
    // came over a socket `connect` returned, for the first attempt and for
    // the reconnect.
    let server = server(vec![
        after_replay(data([1])).then_close(),
        after_replay(data([2])),
    ])
    .await;
    let dials = Arc::new(AtomicUsize::new(0));
    let mut ws = Supervisor::new(
        dialer(&server, &dials, 0..0, || TestError::Stopped),
        fast(),
    )
    .connect(topics(&["a"]))
    .await
    .expect("the first attempt dials through connect");
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    assert_eq!(dials.load(Ordering::SeqCst), 2);
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_error_from_connect_is_a_failed_attempt() {
    let server = server(vec![after_replay(data([1]))]).await;

    // `Supervisor::connect` returns the first attempt's failure and dials
    // nothing more.
    let dials = Arc::new(AtomicUsize::new(0));
    let err = Supervisor::new(
        dialer(&server, &dials, 0..1, || TestError::Refused("no route".into())),
        fast(),
    )
    .connect(topics(&["a"]))
    .await
    .expect_err("connect returns the dial's failure");
    assert!(
        matches!(&err, TestError::Refused(why) if why == "no route"),
        "{err:?}"
    );
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(server.connection_count(), 0);

    // `start` retries a retriable one, the second dial failing too: an
    // outage with its marker, then the attempt that connects.
    let dials = Arc::new(AtomicUsize::new(0));
    let mut ws = Supervisor::new(
        dialer(&server, &dials, 0..2, || {
            TestError::Stalled(Duration::from_millis(1))
        }),
        fast(),
    )
    .start(topics(&["a"]))
    .await
    .expect("a retriable dial failure starts in an outage");
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("error"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(dials.load(Ordering::SeqCst), 3);
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");

    // and `start` returns one recovery stops on.
    let dials = Arc::new(AtomicUsize::new(0));
    let err = Supervisor::new(
        dialer(&server, &dials, 0..1, || TestError::Refused("no route".into())),
        fast(),
    )
    .start(topics(&["a"]))
    .await
    .expect_err("a dial failure recovery stops on is returned");
    assert!(matches!(err, TestError::Refused(_)), "{err:?}");
    assert_eq!(dials.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_dial_failure_recovery_stops_on_ends_a_running_stream_after_it_is_yielded() {
    // The first dial connects; the reconnect's dial fails in a way recovery
    // stops on, so the stream yields it and ends.
    let server = server(vec![after_replay(data([1])).then_close()]).await;
    let dials = Arc::new(AtomicUsize::new(0));
    let mut ws = Supervisor::new(
        dialer(&server, &dials, 1..2, || TestError::Refused("no route".into())),
        fast(),
    )
    .connect(topics(&["a"]))
    .await
    .expect("the first dial connects");
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    let failed = next_item(&mut ws).await;
    assert!(
        matches!(&failed, Err(TestError::Refused(why)) if why == "no route"),
        "{failed:?}"
    );
    let end = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(end.is_none(), "{end:?}");
    assert_eq!(dials.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn the_default_connect_reports_a_handshake_that_cannot_be_built() {
    // `Feed` keeps the default `connect`: the handshake's failure is the
    // attempt's, and nothing is dialled.
    let server = server(vec![Script::hold()]).await;
    let unbuildable = Feed {
        url: "not a url".into(),
        ..feed(&server)
    };
    let err = Supervisor::new(unbuildable, fast())
        .connect(topics(&["a"]))
        .await
        .expect_err("the request cannot be built");
    assert!(matches!(err, TestError::Ws(_)), "{err:?}");
    assert_eq!(server.connection_count(), 0);
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-ws --all-features --lib supervisor`

Expected: the crate's tests do not compile, because `Protocol` has no `connect` yet. The failure includes `error[E0407]: method `connect` is not a member of trait `Protocol``, pointing at `Dialer`'s `fn connect`.

- [ ] **Step 3: Implement**

Edit 1, `polyoxide-ws/src/supervisor/mod.rs`: bring `Socket` into scope. Current (lines 17-20):

```rust
use crate::{
    tungstenite::{self, handshake::client::Request, Message},
    Connection, Timing, WsError,
};
```

Replace with:

```rust
use crate::{
    tungstenite::{self, handshake::client::Request, Message},
    Connection, Socket, Timing, WsError,
};
```

Edit 2, same file: add the provided method directly after `handshake`. Current (lines 144-146):

```rust
    /// The handshake request for one connection attempt, built afresh for
    /// each, so any authentication in it is recomputed.
    fn handshake(&mut self) -> Result<Request, Self::Error>;
```

Replace with:

```rust
    /// The handshake request for one connection attempt, built afresh for
    /// each, so any authentication in it is recomputed.
    fn handshake(&mut self) -> Result<Request, Self::Error>;

    /// Open the connection for one attempt, within `timeout`.
    ///
    /// The [`Supervisor`] calls it for every attempt, the first and each
    /// reconnect, with [`Timing::connect_timeout`]. By default it builds
    /// [`handshake`](Self::handshake)'s request first, so a failure there is
    /// the attempt's, and dials it with the kit's [`connect`](crate::connect).
    /// A protocol that dials its venue a step at a time, such as a venue that
    /// tries each resolved address in turn, overrides it: it bounds each step
    /// by `timeout` itself, upgrades with [`connect_over`](crate::connect_over),
    /// and keeps `handshake` for the request it upgrades with. An error is a
    /// failed attempt, which [`recover`](Self::recover) decides on.
    fn connect(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<Socket, Self::Error>> + Send {
        let request = self.handshake();
        async move {
            let socket = crate::connect(request?, timeout).await?;
            Ok::<_, Self::Error>(socket)
        }
    }
```

Edit 3, `polyoxide-ws/src/supervisor/task.rs`: the import. Current (line 17):

```rust
use crate::{connect, Backoff, Connection, Kind, Mode, Timing, WsError};
```

Replace with:

```rust
use crate::{Backoff, Connection, Kind, Mode, Timing, WsError};
```

Edit 4, same file: `attempt` dials through the protocol. Current (lines 183-190):

```rust
pub(super) async fn attempt<P: Protocol>(
    protocol: &mut P,
    timing: &Timing,
    membership: &mut P::Membership,
) -> Result<Connection, P::Error> {
    let request = protocol.handshake()?;
    let socket = connect(request, timing.connect_timeout).await?;
    let mut connection = protocol.configure(Connection::new(socket, Mode::All));
```

Replace with:

```rust
pub(super) async fn attempt<P: Protocol>(
    protocol: &mut P,
    timing: &Timing,
    membership: &mut P::Membership,
) -> Result<Connection, P::Error> {
    let socket = protocol.connect(timing.connect_timeout).await?;
    let mut connection = protocol.configure(Connection::new(socket, Mode::All));
```

Edit 5, `CLAUDE.md` (line 724, the socket-kit paragraph, which begins `**The socket kit is `polyoxide-ws`;`). It is one long line; replace only this text inside it. Current:

```text
a venue describes its socket as a `Protocol` (a handshake built per attempt, `configure` of each fresh connection, such as Binance's 200 ms request pacing,
```

Replace with:

```text
a venue describes its socket as a `Protocol` (a handshake built per attempt, `connect`, the dial of each attempt, which by default dials the handshake's request and which a venue that tries each address overrides, `configure` of each fresh connection, such as Binance's 200 ms request pacing,
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-ws --all-features --lib supervisor`

Expected: every supervisor test passes, including `connect_dials_every_attempt_and_the_stream_reads_its_socket`, `an_error_from_connect_is_a_failed_attempt`, `a_dial_failure_recovery_stops_on_ends_a_running_stream_after_it_is_yielded` and `the_default_connect_reports_a_handshake_that_cannot_be_built`, and the output ends `test result: ok.` with no failures.

Then run the crate's other gates, which cover perps' and Binance's unchanged default:

Run: `nix develop --command cargo clippy -p polyoxide-ws -p polyoxide-polymarket -p polyoxide-binance --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-ws --all-features'`

Expected: finishes with no warnings (the new doc links `connect` and `connect_over` resolve).

Run: `nix develop --command python3 scripts/venue_gate.py`

Expected: passes (the new doc and tests name no venue).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(ws): Protocol::connect, the dial the Supervisor calls for every attempt (Epic 7, Story 1)" -- polyoxide-ws/src/supervisor/mod.rs polyoxide-ws/src/supervisor/task.rs polyoxide-ws/src/supervisor/tests.rs CLAUDE.md
```

### Task 1.2: clob's `dial`, and four helpers raised to `pub(super)`

**Files:**
- Modify: `polyoxide-polymarket/src/clob/ws/client.rs:9-13,28-29,36-38,71-82,118,127-133,197-198`
- Test: `polyoxide-polymarket/src/clob/ws/client.rs` (a new `dial_tests` module at the end of the file)

- [ ] **Step 1: Write the failing test**

Append this module to the end of `polyoxide-polymarket/src/clob/ws/client.rs`, after the last module, `membership_tests`, whose final lines are:

```rust
        let err = handle.subscribe_assets(vec!["b".into()]).await.unwrap_err();
        assert!(matches!(err, ClobWsError::InvalidMessage(_)), "{err}");
    }
}
```

The appended module:

```rust
#[cfg(test)]
mod dial_tests {
    use polyoxide_venue::Classify;
    use polyoxide_ws::test_server::{Script, ScriptedServer};

    use super::*;

    #[tokio::test]
    async fn dial_returns_the_upgraded_socket_with_nothing_read() {
        let server = ScriptedServer::start(vec![Script::frames(&["hello"])]).await;
        let mut socket = dial(&server.url, Duration::from_secs(2))
            .await
            .expect("dial");
        let first = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("a frame within the window")
            .expect("the stream has not ended")
            .expect("a frame, not an error");
        assert_eq!(first, Message::Text("hello".into()));
    }

    #[tokio::test]
    async fn dial_returns_a_refused_upgrade_as_the_connection_error() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(503)]).await;
        let err = dial(&server.url, Duration::from_secs(2))
            .await
            .expect_err("a refused upgrade must not connect");
        assert!(matches!(err, ClobWsError::Connection(_)), "{err:?}");
        assert!(err.is_retriable(), "{err:?}");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws::client::dial_tests`

Expected: does not compile: ``error[E0425]: cannot find function `dial` in this scope``, twice, in `dial_tests`.

- [ ] **Step 3: Implement**

Edit 1, `polyoxide-polymarket/src/clob/ws/client.rs`: import the kit's `Socket`. Current (lines 9-13):

```rust
use polyoxide_ws::{
    connect_over,
    tungstenite::{self, Message},
    with_timeout, Connection, Kind, Mode, WsError,
};
```

Replace with:

```rust
use polyoxide_ws::{
    connect_over,
    tungstenite::{self, Message},
    with_timeout, Connection, Kind, Mode, Socket, WsError,
};
```

Edit 2, same file: the subscription limit. Current (lines 28-29):

```rust
/// Maximum number of subscriptions per WebSocket connection.
const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 500;
```

Replace with:

```rust
/// Maximum number of subscriptions per WebSocket connection.
pub(super) const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 500;
```

Edit 3, same file: the default connect timeout. Current (lines 36-38):

```rust
/// Default per-address connect timeout, overridable via
/// [`WebSocketBuilder::connect_timeout`].
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
```

Replace with:

```rust
/// Default per-address connect timeout, overridable via
/// [`WebSocketBuilder::connect_timeout`].
pub(super) const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
```

Edit 4, same file: `connect_ws` becomes `dial`. Current (lines 71-82):

```rust
/// Open a TLS WebSocket connection with a per-address timeout and fallback,
/// read through the kit's [`Connection`] in [`Mode::Data`].
///
/// [`polyoxide_ws::connect`] resolves DNS and connects to only the *first*
/// address — a blackholed route (the classic broken-IPv6 case) burns the
/// whole timeout there. This resolves every address up front, tries each in
/// family-interleaved order with `connect_timeout` covering its TCP connect
/// and again its TLS + WebSocket handshake ([`with_timeout`] around each,
/// the upgrade through [`connect_over`]), and falls back to the next address
/// on any failure but a refused upgrade, which it returns at once. This
/// connect is clob's own (divergence row D20).
async fn connect_ws(url: &str, connect_timeout: Duration) -> Result<Connection, ClobWsError> {
```

Replace with:

```rust
/// Open a TLS WebSocket connection with a per-address timeout and fallback,
/// and return the upgraded socket unread.
///
/// [`polyoxide_ws::connect`] resolves DNS and connects to only the *first*
/// address — a blackholed route (the classic broken-IPv6 case) burns the
/// whole timeout there. This resolves every address up front, tries each in
/// family-interleaved order with `connect_timeout` covering its TCP connect
/// and again its TLS + WebSocket handshake ([`with_timeout`] around each,
/// the upgrade through [`connect_over`]), and falls back to the next address
/// on any failure but a refused upgrade, which it returns at once. This
/// connect is clob's own (divergence row D20). The bare tier reads the socket
/// through [`connect_ws`]; the supervised sockets return it from
/// [`polyoxide_ws::Protocol::connect`].
pub(super) async fn dial(url: &str, connect_timeout: Duration) -> Result<Socket, ClobWsError> {
```

Edit 5, same file: `dial` returns the socket. Current (line 118):

```rust
            Ok(Ok(socket)) => return Ok(Connection::new(socket, Mode::Data)),
```

Replace with:

```rust
            Ok(Ok(socket)) => return Ok(socket),
```

Edit 6, same file: `connect_ws` wraps `dial`, and `frame_text` is raised. Current (lines 127-133):

```rust
    Err(last_err)
}

/// The text a data frame carries. A Binary frame is decoded as text (D12),
/// since the venue has sent events that way; one that is not UTF-8 carries
/// none, and is skipped.
fn frame_text(frame: &Message) -> Option<&str> {
```

Replace with:

```rust
    Err(last_err)
}

/// Open a TLS WebSocket connection through [`dial`], read through the kit's
/// [`Connection`] in [`Mode::Data`].
async fn connect_ws(url: &str, connect_timeout: Duration) -> Result<Connection, ClobWsError> {
    dial(url, connect_timeout)
        .await
        .map(|socket| Connection::new(socket, Mode::Data))
}

/// The text a data frame carries. A Binary frame is decoded as text (D12),
/// since the venue has sent events that way; one that is not UTF-8 carries
/// none, and is skipped.
pub(super) fn frame_text(frame: &Message) -> Option<&str> {
```

Edit 7, same file: the count check. Current (lines 197-198):

```rust
/// Validate that the subscription count does not exceed the per-connection limit.
fn validate_subscription_count(count: usize) -> Result<(), ClobWsError> {
```

Replace with:

```rust
/// Validate that the subscription count does not exceed the per-connection limit.
pub(super) fn validate_subscription_count(count: usize) -> Result<(), ClobWsError> {
```

Edit 8, same file: append the `dial_tests` module of Step 1 at the end of the file.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`

Expected: all `clob::ws` tests pass, the two new `dial_tests` and the existing `connect_tests`, `connection_tests`, `dispatch_tests` and `membership_tests` unchanged.

Run: `nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws --all-targets -- -D warnings`

Expected: no warnings (no new item is dead: `dial` is called by `connect_ws`, and the four raised items were already used in this file).

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket --all-features'`

Expected: finishes with no warnings.

Run: `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`

Expected: passes (no existing test body changed; the new module is a new item).

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(clob): split dial out of connect_ws and raise four helpers to pub(super) (Epic 7, Story 1)" -- polyoxide-polymarket/src/clob/ws/client.rs
```

## Story 2: The supervised CLOB market socket (Story 7.1)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/src/clob/ws/mod.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`, `polyoxide-polymarket/src/clob/ws/supervised/market.rs`, `polyoxide-polymarket/src/clob/ws/scripted.rs`, `CLAUDE.md`
- **Interfaces:** exposes `clob::ws::{SupervisedMarketWs, MarketWsBuilder, ClobMarketProtocol, MarketEvent, MarketChange, MarketMembership, DisconnectReason}`, and crate-private `supervised::{checked_url, default_timing, disconnect_reason, recover, data_text}` and `scripted::pong` (names contract, Story 2); uses Story 1's `Protocol::connect`, `dial`, `frame_text`, `validate_subscription_count`, `MAX_SUBSCRIPTIONS_PER_CONNECTION` and `DEFAULT_CONNECT_TIMEOUT`
- **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`
- **Review:** deep (new supervised socket on the kit)

Adds `clob::ws::supervised` with `ClobMarketProtocol` on the kit's `Supervisor` (spec §4), beside the unchanged bare tier. The socket reconnects after a server close or a stall, replays the whole membership and the builder's options on every connection, and yields one `Disconnected` and one `Reconnected` per outage. It sends the 10 s text `PING`, skips `PONG`, decodes Binary as text, yields every book of an array frame and keeps an empty membership connected. Its `Protocol::connect` override calls clob's per-address `dial` (Story 1), so D20's connect runs under supervision. Task 2.1 lands what Story 3 reuses, the helpers `checked_url`, `default_timing`, `disconnect_reason`, `recover` and `data_text` with `DisconnectReason`, and the `#[cfg(test)]` `PONG` answerer, behind a temporary `#![allow(dead_code)]` that Task 2.2 removes when the market socket calls them. Task 2.2 adds the socket, its module docs and CLAUDE.md's two edits. Recovery reads the error's `Classify` class and never matches over `tungstenite::Error`; the one row that departs from the class is a connection used after it closed, which reconnects (D19) and is read off the kit's `Kind`, as Binance's recovery does. The `mod` and `pub use` lines of `clob/ws/mod.rs` are written in the order and the wrapping `cargo fmt` gives them.

### Task 2.1: The shared items and the scripted answerer

**Files:**
- Create: `polyoxide-polymarket/src/clob/ws/scripted.rs`
- Create: `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`
- Modify: `polyoxide-polymarket/src/clob/ws/mod.rs:117-122`
- Test: `polyoxide-polymarket/src/clob/ws/scripted.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`

- [ ] **Step 1: Write the failing tests.** Declare the two modules, then create each file with its tests and the lines the items will follow. The `mod` lines go where `cargo fmt` sorts them. The `#![allow(dead_code)]` in `supervised/mod.rs` is a temporary stub: nothing outside the tests calls the helpers until Task 2.2, and clippy runs with `-D warnings`.

Current (`polyoxide-polymarket/src/clob/ws/mod.rs`):

```rust
mod auth;
mod client;
mod error;
mod market;
mod subscription;
mod user;
```

Replace with:

```rust
mod auth;
mod client;
mod error;
mod market;
#[cfg(test)]
mod scripted;
mod subscription;
mod supervised;
mod user;
```

Create `polyoxide-polymarket/src/clob/ws/scripted.rs` with:

```rust
//! The scripted-server answerer the supervised sockets' unit tests share, to
//! be passed to `polyoxide_ws::test_server::ScriptedServer::start_answering`.
//!
//! An integration test cannot see this module and writes its own.

use polyoxide_ws::tungstenite::Message;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ping_is_answered_with_a_pong_and_nothing_else_is_answered() {
        let mut answer = pong(0);
        assert_eq!(answer("PING"), vec![Message::Text("PONG".into())]);
        assert_eq!(answer("PING"), vec![Message::Text("PONG".into())]);
        assert!(answer(r#"{"assets_ids":[],"type":"market"}"#).is_empty());
        assert!(answer(r#"{"operation":"subscribe","assets_ids":["a"]}"#).is_empty());
        assert!(answer("ping").is_empty());
        assert!(answer("").is_empty());
    }
}
```

Create `polyoxide-polymarket/src/clob/ws/supervised/mod.rs` with:

```rust
//! The CLOB's supervised sockets: a channel's connection run by the socket
//! kit's `Supervisor` (AD-11), which reconnects it, replays its membership and
//! marks every outage, where the bare tier ends on the first failure.
//!
//! This file holds what the supervised sockets share: the rule for the URL
//! they take, their timing, the reason an outage gives, their recovery, and
//! the filter their decode applies to a frame's text. Each channel's protocol,
//! builder, events and membership calls are in its own file.

// Temporary, until Task 2.2: nothing outside the tests calls what follows yet.
#![allow(dead_code)]

use std::time::Duration;

use polyoxide_venue::Classify;
use polyoxide_ws::{tungstenite::Message, Kind, Outage, Recovery, Timing};

use super::{
    client::{frame_text, DEFAULT_CONNECT_TIMEOUT},
    ClobWsError,
};

#[cfg(test)]
mod tests {
    use polyoxide_venue::Class;
    use polyoxide_ws::{
        tungstenite::{self, error, http},
        WsError,
    };

    use super::*;

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> ClobWsError {
        ClobWsError::from(WsError::from(err))
    }

    /// An upgrade the server refused with this status.
    fn refused(status: u16) -> ClobWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    fn closed(code: Option<u16>) -> ClobWsError {
        ClobWsError::from(WsError::closed(code, ""))
    }

    #[test]
    fn recovery_follows_the_class_with_the_clob_rows() {
        let json = serde_json::from_str::<()>("{").unwrap_err();
        let rows = [
            // A frame that did not decode keeps the connection.
            (ClobWsError::Json(json), Recovery::Skip),
            (
                ClobWsError::InvalidMessage("a frame".into()),
                Recovery::Skip,
            ),
            // Retrying cannot fix bad credentials.
            (
                ClobWsError::Authentication("authentication failed".into()),
                Recovery::Stop,
            ),
            // A server close reconnects, but for the codes the venue refuses
            // with.
            (closed(Some(1000)), Recovery::Reconnect),
            (closed(Some(1001)), Recovery::Reconnect),
            (closed(Some(1011)), Recovery::Reconnect),
            (closed(None), Recovery::Reconnect),
            (closed(Some(1008)), Recovery::Stop),
            (closed(Some(1002)), Recovery::Stop),
            (closed(Some(4001)), Recovery::Stop),
            // The kit's own timeouts and the connect timeout of clob's dial.
            (
                ClobWsError::from(WsError::connect_timeout(Duration::from_secs(1))),
                Recovery::Reconnect,
            ),
            (
                ClobWsError::from(WsError::no_answer(Duration::from_secs(1))),
                Recovery::Reconnect,
            ),
            (
                ClobWsError::ConnectTimeout {
                    url: "wss://example.com".into(),
                    timeout: Duration::from_secs(1),
                },
                Recovery::Reconnect,
            ),
            (ClobWsError::ConnectionClosed, Recovery::Reconnect),
            (
                transport(tungstenite::Error::Io(
                    std::io::ErrorKind::ConnectionReset.into(),
                )),
                Recovery::Reconnect,
            ),
            // D19: a connection used after it closed reconnects, though its
            // class is `InvalidRequest`.
            (
                transport(tungstenite::Error::AlreadyClosed),
                Recovery::Reconnect,
            ),
            // The client's own refusals stop.
            (
                transport(tungstenite::Error::Url(error::UrlError::NoHostName)),
                Recovery::Stop,
            ),
            (ClobWsError::Url(url::ParseError::EmptyHost), Recovery::Stop),
            (ClobWsError::MembershipClosed, Recovery::Stop),
            // A refused upgrade follows its status: 408, 425, 429 and 5xx
            // reconnect, and any other refusal stops.
            (refused(408), Recovery::Reconnect),
            (refused(425), Recovery::Reconnect),
            (refused(429), Recovery::Reconnect),
            (refused(503), Recovery::Reconnect),
            (refused(401), Recovery::Stop),
            (refused(404), Recovery::Stop),
            (refused(418), Recovery::Stop),
            (refused(451), Recovery::Stop),
        ];
        for (err, expected) in rows {
            assert_eq!(recover(&err), expected, "{err:?}");
        }
    }

    #[test]
    fn a_refusal_the_class_calls_retriable_agrees_with_the_kits_rule() {
        // DRIFT R1: the socket table and `retrying_can_fix` agree on every
        // refused status, so recovery reads the class and not the status.
        for status in [400, 401, 403, 404, 408, 418, 425, 429, 451, 500, 502, 503] {
            let err = refused(status);
            let ClobWsError::Connection(inner) = &err else {
                panic!("a refusal is a connection error: {err:?}");
            };
            let by_kit = polyoxide_ws::retrying_can_fix(inner, |status| {
                polyoxide_venue::class_for_handshake_status(status).is_retriable()
            });
            assert_eq!(
                recover(&err) == Recovery::Reconnect,
                by_kit,
                "status {status}"
            );
        }
    }

    #[test]
    fn every_outage_maps_to_its_reason() {
        let down = disconnect_reason(Outage::Closed {
            code: Some(1001),
            reason: "restart".into(),
        });
        assert!(
            matches!(&down, DisconnectReason::Closed { code: Some(1001), reason }
                if reason == "restart"),
            "{down:?}"
        );
        let down = disconnect_reason(Outage::Error(ClobWsError::ConnectionClosed));
        assert!(
            matches!(down, DisconnectReason::Error(ClobWsError::ConnectionClosed)),
            "{down:?}"
        );
        let down = disconnect_reason(Outage::Stale {
            silent: Duration::from_secs(3),
        });
        assert!(
            matches!(&down, DisconnectReason::Stale { silent }
                if *silent == Duration::from_secs(3)),
            "{down:?}"
        );
        let down = disconnect_reason(Outage::Rotation);
        assert!(matches!(down, DisconnectReason::Rotation), "{down:?}");
    }

    #[test]
    fn the_url_rule_is_wss_or_a_loopback_ws() {
        for url in [
            "wss://ws-subscriptions-clob.polymarket.com/ws/market",
            "ws://127.0.0.1:8080/ws/market",
            "ws://localhost/ws/user",
            "ws://[::1]:8080",
        ] {
            assert_eq!(checked_url(url.to_owned()).expect(url), url);
        }
        for url in [
            "ws://ws-subscriptions-clob.polymarket.com/ws/market",
            "ws://127.0.0.2:8080",
            "ws://localhost.example.com",
            "ws://10.0.0.1",
            "http://localhost",
            "https://ws-subscriptions-clob.polymarket.com",
            "ws-subscriptions-clob.polymarket.com",
            "",
        ] {
            let err = checked_url(url.to_owned()).expect_err(url);
            assert!(
                matches!(&err, ClobWsError::InvalidMessage(text)
                    if text == &format!("the URL must be wss://, or ws:// to a loopback host: {url}")),
                "{err:?}"
            );
            assert_eq!(err.class(), Class::InvalidRequest);
        }
    }

    #[test]
    fn the_timing_is_the_documented_one() {
        let timing = default_timing();
        assert_eq!(timing.ping_interval, Some(Duration::from_secs(10)));
        assert_eq!(timing.stale_after, Duration::from_secs(30));
        assert_eq!(timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(timing.max_backoff, Duration::from_secs(60));
        assert_eq!(timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(timing.max_connection_age, None);
    }

    #[test]
    fn the_decode_filter_passes_events_and_skips_the_rest() {
        let book = r#"{"event_type":"book","asset_id":"a"}"#;
        assert_eq!(data_text(&Message::Text(book.into())), Some(book));
        // A Binary frame is decoded as text (divergence D12).
        assert_eq!(
            data_text(&Message::Binary(book.as_bytes().to_vec().into())),
            Some(book)
        );
        let array = r#"[{"event_type":"book","asset_id":"a"}]"#;
        assert_eq!(data_text(&Message::Text(array.into())), Some(array));

        for text in ["PONG", "{}", "", "[]", r#"{"subscribed":true}"#] {
            assert_eq!(data_text(&Message::Text(text.into())), None, "{text:?}");
        }
        // Not UTF-8, so no text; and the control frames carry none.
        assert_eq!(data_text(&Message::Binary(vec![0xff].into())), None);
        assert_eq!(data_text(&Message::Ping(Vec::new().into())), None);
        assert_eq!(data_text(&Message::Pong(Vec::new().into())), None);
        assert_eq!(data_text(&Message::Close(None)), None);
    }
}
```

- [ ] **Step 2: Run it to see it fail.**

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`

Expected: the build fails. `scripted.rs` reports ``cannot find function `pong` in this scope``, and `supervised/mod.rs` reports ``cannot find function `recover` in this scope`` and the same for `checked_url`, `default_timing`, `disconnect_reason` and `data_text`, and ``failed to resolve: use of undeclared type `DisconnectReason` ``. The run ends with ``error: could not compile `polyoxide-polymarket` (lib test)``.

- [ ] **Step 3: Implement.** Put the answerer in `scripted.rs`, then the shared items in `supervised/mod.rs`.

Current (`polyoxide-polymarket/src/clob/ws/scripted.rs`):

```rust
use polyoxide_ws::tungstenite::Message;

#[cfg(test)]
mod tests {
```

Replace with:

```rust
use polyoxide_ws::tungstenite::Message;

/// One connection's answerer: it answers the keep-alive text `PING` with a
/// text `PONG`, as the venue does, and every other frame with nothing, the
/// subscription frames included.
pub(crate) fn pong(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send + 'static {
    |text| {
        if text == "PING" {
            vec![Message::Text("PONG".into())]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
```

Current (`polyoxide-polymarket/src/clob/ws/supervised/mod.rs`):

```rust
use super::{
    client::{frame_text, DEFAULT_CONNECT_TIMEOUT},
    ClobWsError,
};

#[cfg(test)]
mod tests {
```

Replace with:

```rust
use super::{
    client::{frame_text, DEFAULT_CONNECT_TIMEOUT},
    ClobWsError,
};

/// Why a supervised connection went down, in a `Disconnected` event.
///
/// Mapped from the socket kit's `Outage` as Binance's own `DisconnectReason`
/// is. The market and user sockets never rotate, so [`Rotation`](Self::Rotation)
/// is kept for completeness.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed the connection, with the code and reason it sent, or
    /// ended the stream with neither.
    Closed {
        /// The close code, when a close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A failure: a read, a ping or a decoded frame that recovery sent to a
    /// reconnect, or the failed first connection attempt of a socket that
    /// starts in an outage.
    Error(ClobWsError),
    /// Nothing arrived for the staleness window, or a membership change or
    /// replay went unanswered for it.
    Stale {
        /// The silence, less any time the consumer's queue was full.
        silent: Duration,
    },
    /// Replaced on purpose at the connection's maximum age.
    Rotation,
}

/// The URL a supervised socket takes: `wss://`, or `ws://` to a loopback host
/// (`127.0.0.1`, `[::1]` or `localhost`), so a test reaches a local server and
/// credentials never travel unencrypted to a remote one. Anything else,
/// and a URL that does not parse, is refused with
/// [`ClobWsError::InvalidMessage`]. The bare tier keeps its `wss://`-only rule.
pub(super) fn checked_url(url: String) -> Result<String, ClobWsError> {
    let allowed = url::Url::parse(&url).is_ok_and(|parsed| match parsed.scheme() {
        "wss" => true,
        "ws" => match parsed.host() {
            Some(url::Host::Domain(host)) => host == "localhost",
            Some(url::Host::Ipv4(ip)) => ip == std::net::Ipv4Addr::LOCALHOST,
            Some(url::Host::Ipv6(ip)) => ip == std::net::Ipv6Addr::LOCALHOST,
            None => false,
        },
        _ => false,
    });
    if allowed {
        Ok(url)
    } else {
        Err(ClobWsError::InvalidMessage(format!(
            "the URL must be wss://, or ws:// to a loopback host: {url}"
        )))
    }
}

/// The timing both supervised sockets start from: a text `PING` every
/// 10 s, stale after 30 s without a frame, backoff from 500 ms to 60 s, and a
/// 10 s connect timeout for each address and each step. No rotation.
pub(super) fn default_timing() -> Timing {
    let mut timing = Timing::default();
    timing.ping_interval = Some(Duration::from_secs(10));
    timing.stale_after = Duration::from_secs(30);
    timing.initial_backoff = Duration::from_millis(500);
    timing.max_backoff = Duration::from_secs(60);
    timing.connect_timeout = DEFAULT_CONNECT_TIMEOUT;
    timing
}

/// The reason an outage gives: the kit's `Outage` as a [`DisconnectReason`]. An
/// outage a later kit adds reads as a stall of no length.
pub(super) fn disconnect_reason(outage: Outage<ClobWsError>) -> DisconnectReason {
    match outage {
        Outage::Closed { code, reason } => DisconnectReason::Closed { code, reason },
        Outage::Error(err) => DisconnectReason::Error(err),
        Outage::Stale { silent } => DisconnectReason::Stale { silent },
        Outage::Rotation => DisconnectReason::Rotation,
        _ => DisconnectReason::Stale {
            silent: Duration::ZERO,
        },
    }
}

/// What the supervisor does after a failure of a supervised socket:
///
/// - a frame that does not decode, a [`Json`](ClobWsError::Json) error or an
///   [`InvalidMessage`](ClobWsError::InvalidMessage), is skipped: the stream
///   yields the error and keeps the connection;
/// - an [`Authentication`](ClobWsError::Authentication) failure stops the
///   stream, since retrying cannot fix bad credentials;
/// - a connection used after it closed, a [`Connection`](ClobWsError::Connection)
///   whose kit error's kind is `Kind::AlreadyClosed`, reconnects, as Binance's
///   and sports' recovery rows do (D19 of `divergences.md`). The socket table
///   classes a use after close `InvalidRequest`, so the class rule alone would
///   stop on it, and a ping that races a server close would end the stream;
/// - any other failure reconnects exactly when its class is retriable, so a
///   refused upgrade follows the socket table, which agrees with the kit's
///   `retrying_can_fix` on every status, and a server close reconnects unless
///   its code is one the venue refuses with.
///
/// The error's class decides; no code here matches over `tungstenite::Error`.
pub(super) fn recover(err: &ClobWsError) -> Recovery {
    match err {
        ClobWsError::Json(_) | ClobWsError::InvalidMessage(_) => Recovery::Skip,
        ClobWsError::Authentication(_) => Recovery::Stop,
        ClobWsError::Connection(err) if matches!(err.kind(), Kind::AlreadyClosed) => {
            Recovery::Reconnect
        }
        err if err.is_retriable() => Recovery::Reconnect,
        _ => Recovery::Stop,
    }
}

/// The text of a data frame that can carry an event, or `None` for every frame
/// the supervised decode leaves alone: a Close, Ping or Pong, a Binary frame
/// that is not UTF-8, and the keep-alives and acknowledgements the venue sends
/// as text, which are `PONG`, `{}`, an empty text, and any text without an
/// `event_type`, as the bare tier skips them. A Binary frame is decoded as
/// text (divergence D12).
pub(super) fn data_text(frame: &Message) -> Option<&str> {
    let text = frame_text(frame)?;
    if text == "PONG" || text == "{}" || text.is_empty() || !text.contains("event_type") {
        return None;
    }
    Some(text)
}

#[cfg(test)]
mod tests {
```

- [ ] **Step 4: Run it to see it pass.**

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`

Expected: `test result: ok.` with the existing `clob::ws` tests and these seven new ones, each `ok`: `clob::ws::scripted::tests::a_ping_is_answered_with_a_pong_and_nothing_else_is_answered`, and in `clob::ws::supervised::tests` `recovery_follows_the_class_with_the_clob_rows`, `a_refusal_the_class_calls_retriable_agrees_with_the_kits_rule`, `every_outage_maps_to_its_reason`, `the_url_rule_is_wss_or_a_loopback_ws`, `the_timing_is_the_documented_one` and `the_decode_filter_passes_events_and_skips_the_rest`.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings`

Expected: no output from `cargo fmt`, and clippy finishes with no warning.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-polymarket/src/clob/ws/scripted.rs polyoxide-polymarket/src/clob/ws/supervised/mod.rs
git commit -m "feat(polymarket): the supervised sockets' shared items and the scripted PONG answerer (Epic 7, Story 2)" -- polyoxide-polymarket/src/clob/ws/mod.rs polyoxide-polymarket/src/clob/ws/scripted.rs polyoxide-polymarket/src/clob/ws/supervised/mod.rs
```

### Task 2.2: The supervised market socket

**Files:**
- Create: `polyoxide-polymarket/src/clob/ws/supervised/market.rs`
- Modify: `polyoxide-polymarket/src/clob/ws/supervised/mod.rs:10-13`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs:18-21`
- Modify: `polyoxide-polymarket/src/clob/ws/mod.rs:113-117`, `polyoxide-polymarket/src/clob/ws/mod.rs:139`
- Modify: `CLAUDE.md:724`
- Test: `polyoxide-polymarket/src/clob/ws/supervised/market.rs`

- [ ] **Step 1: Write the failing tests.** Remove Task 2.1's stub and declare the market module with its re-exports, re-export the items at `clob::ws`, then create `market.rs` with its imports and its tests. The `pub use` is wrapped as `cargo fmt` wraps it.

Current (`polyoxide-polymarket/src/clob/ws/supervised/mod.rs`):

```rust
// Temporary, until Task 2.2: nothing outside the tests calls what follows yet.
#![allow(dead_code)]

use std::time::Duration;
```

Replace with:

```rust
use std::time::Duration;
```

Current (`polyoxide-polymarket/src/clob/ws/supervised/mod.rs`):

```rust
use super::{
    client::{frame_text, DEFAULT_CONNECT_TIMEOUT},
    ClobWsError,
};
```

Replace with:

```rust
use super::{
    client::{frame_text, DEFAULT_CONNECT_TIMEOUT},
    ClobWsError,
};

mod market;

pub use market::{
    ClobMarketProtocol, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder,
    SupervisedMarketWs,
};
```

Current (`polyoxide-polymarket/src/clob/ws/mod.rs`):

```rust
pub use user::{MakerOrder, OrderEventType, OrderMessage, TradeMessage, TradeStatus, UserMessage};
```

Replace with:

```rust
pub use supervised::{
    ClobMarketProtocol, DisconnectReason, MarketChange, MarketEvent, MarketMembership,
    MarketWsBuilder, SupervisedMarketWs,
};
pub use user::{MakerOrder, OrderEventType, OrderMessage, TradeMessage, TradeStatus, UserMessage};
```

Create `polyoxide-polymarket/src/clob/ws/supervised/market.rs` with:

```rust
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

use super::{checked_url, data_text, default_timing, disconnect_reason, recover, DisconnectReason};
use crate::clob::ws::{
    client::{dial, validate_subscription_count},
    BookMessage, ClobWsError, MarketMessage, MarketSubscription, MarketSubscriptionOptions,
    MarketSubscriptionUpdate, WS_MARKET_URL,
};

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use polyoxide_venue::Class;
    use polyoxide_ws::test_server::{fast, next_event, next_item, Script, ScriptedServer, WINDOW};

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
```

- [ ] **Step 2: Run it to see it fail.**

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`

Expected: the build fails. `supervised/mod.rs` reports ``error[E0432]: unresolved imports `market::ClobMarketProtocol`, `market::MarketChange`, `market::MarketEvent`, `market::MarketMembership`, `market::MarketWsBuilder`, `market::SupervisedMarketWs` ``, and `market.rs`'s tests report ``cannot find type `MarketWsBuilder` in this scope`` and the same for `MarketEvent`, `SupervisedMarketWs` and `ClobMarketProtocol`. The run ends with ``error: could not compile `polyoxide-polymarket` (lib test)``.

- [ ] **Step 3: Implement.** The socket first, then the module docs, then CLAUDE.md.

Current (`polyoxide-polymarket/src/clob/ws/supervised/market.rs`):

```rust
use crate::clob::ws::{
    client::{dial, validate_subscription_count},
    BookMessage, ClobWsError, MarketMessage, MarketSubscription, MarketSubscriptionOptions,
    MarketSubscriptionUpdate, WS_MARKET_URL,
};

#[cfg(test)]
mod tests {
```

Replace with:

```rust
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
        if text.starts_with('[') {
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
    /// Fails with [`ClobWsError::InvalidMessage`] for more than 500 ids,
    /// before connecting, and returns the first connection attempt's failure
    /// as it is: a setup that is wrong is reported rather than retried. Later
    /// failures reconnect. An empty list is valid and stays open under the
    /// keep-alive, and a duplicate id is subscribed once.
    pub async fn connect(self, asset_ids: Vec<String>) -> Result<SupervisedMarketWs, ClobWsError> {
        validate_subscription_count(asset_ids.len())?;
        let mut membership: Vec<String> = Vec::with_capacity(asset_ids.len());
        for id in asset_ids {
            if !membership.contains(&id) {
                membership.push(id);
            }
        }
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
```

Add the module docs. The new section follows the last example, and nothing below it changes.

Current (`polyoxide-polymarket/src/clob/ws/mod.rs`):

```rust
//!     Ok(())
//! }
//! ```

mod auth;
```

Replace with:

```rust
//!     Ok(())
//! }
//! ```
//!
//! # Supervised Market Socket
//!
//! [`MarketWsBuilder`] opens the market channel on the socket kit's `Supervisor`,
//! which keeps it alive where the types above end on their first failure. It
//! sends the keep-alive `PING` itself, reconnects after a server close or a
//! stall, replays the whole subscription (membership changes included) on every
//! new connection, and marks each outage with a [`MarketEvent::Disconnected`]
//! and a [`MarketEvent::Reconnected`], so a consumer knows when its books are
//! stale and when to rebuild them. A frame that carries several books yields
//! one event for each. Membership changes on a live socket go through
//! [`MarketMembership`], on the handle that `Supervised::membership` returns.
//!
//! ```no_run
//! use futures_util::StreamExt;
//! use polyoxide_polymarket::clob::ws::{MarketEvent, MarketMessage, MarketWsBuilder};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut ws = MarketWsBuilder::new()
//!         .connect(vec!["asset_id".to_string()])
//!         .await?;
//!
//!     while let Some(item) = ws.next().await {
//!         match item {
//!             Ok(MarketEvent::Message(MarketMessage::Book(book))) => {
//!                 println!("{}: {} bids, {} asks", book.asset_id, book.bids.len(), book.asks.len());
//!             }
//!             Ok(MarketEvent::Disconnected { reason }) => eprintln!("down: {reason:?}"),
//!             Ok(MarketEvent::Reconnected) => eprintln!("up again: the books that follow replace yours"),
//!             Ok(_) => {}
//!             // A frame that did not decode, or the error that ended the stream.
//!             Err(err) => eprintln!("{err}"),
//!         }
//!     }
//!     Ok(())
//! }
//! ```

mod auth;
```

State the rule in CLAUDE.md: the new paragraph goes directly before the socket-kit paragraph, and that paragraph's last sentence no longer says clob is not supervised.

Current (`CLAUDE.md`):

```markdown
**The socket kit is `polyoxide-ws`; perps and Binance run on its `Supervisor` (Stories 4.6 and 4.7), and rtds, sports and clob on its blocks (Stories 4.8 and 4.9).**
```

Replace with:

```markdown
**The supervised CLOB market socket** (Story 7.1) is `clob::ws::MarketWsBuilder` and its
`SupervisedMarketWs`, in `polyoxide-polymarket/src/clob/ws/supervised/`, beside the unchanged bare
tier (`WebSocket`, `WebSocketBuilder`, `WebSocketWithPing`). `ClobMarketProtocol` describes the
market channel to the kit's `Supervisor`. The stream yields `MarketEvent`s, which are the channel's
messages, one `Disconnected` and one `Reconnected` per outage, and a `Disconnected` carries a
`DisconnectReason`, the kit's `Outage` as `Closed`, `Error`, `Stale` or `Rotation`. It reconnects
after a server close, a stall or a lost connection, and replays the whole membership, changes and
the builder's `MarketSubscriptionOptions` included, as one subscribe frame on every connection, an
empty one too, which the venue keeps open. It sends the text `PING` every 10 s on the wall clock
(D4), the first one interval after the connection opens, counts every inbound frame as liveness and
is stale after 30 s (D5), backs off 500 ms to 60 s with the kit's `Retry-After` floor, never
rotates, and queues 1024 events. `MarketWsBuilder::connect(asset_ids)` refuses more than 500 ids
before connecting and returns the first attempt's failure instead of retrying it. A frame that
carries several books yields every one, where the bare tier's `MarketMessage::from_json` keeps the
first and a protected test pins that; `PONG`, `{}`, empty text and any text without an `event_type`
are skipped, and a Binary frame is decoded as text (D12). A frame that does not decode is yielded as
`ClobWsError::Json` and the connection stays; `ClobWsError::Authentication` stops the stream; a
connection used after it closed reconnects (D19), though its class is `InvalidRequest`; and any
other failure reconnects exactly when its class is retriable, with no match over
`tungstenite::Error`. The connection is dialled by clob's per-address `dial` through
`Protocol::connect` (D20), so every address of a blackholed route is tried within `connect_timeout`
per step under supervision too. The URL is `wss://`, or `ws://` to a loopback host, which
`MarketWsBuilder::url` enforces, where the bare `WebSocketBuilder` keeps its `wss://`-only rule. A
change through `MarketMembership`, on the kit's `MembershipHandle<ClobMarketProtocol>`, is recorded
before its frame is sent and sends the ids that change, once each; one whose connection dies under
it is answered `Ok` and replayed, one made during an outage is answered at once and replayed, and a
subscribe past 500 ids is refused as `InvalidMessage` and changes nothing. `clob::ws::scripted`,
built for the unit tests, holds the `PING`/`PONG` answerer the supervised sockets' tests share.

**The socket kit is `polyoxide-ws`; perps and Binance run on its `Supervisor` (Stories 4.6 and 4.7), and rtds, sports and clob on its blocks (Stories 4.8 and 4.9).**
```

Current (`CLAUDE.md`):

```markdown
clob keeps its own `select!`, its 10 s text `PING`, its outbound queue and Binary decoded as text (D12), and is not supervised until Epic 7.
```

Replace with:

```markdown
clob's bare tier keeps its own `select!`, its 10 s text `PING`, its outbound queue and Binary decoded as text (D12); its supervised sockets (Epic 7) run on the `Supervisor` instead, as described above.
```

- [ ] **Step 4: Run it to see it pass.**

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`

Expected: `test result: ok.` with the earlier `clob::ws` tests and these twenty new ones in `clob::ws::supervised::market::tests`, each `ok`: `the_defaults_are_the_documented_ones`, `the_builder_takes_a_wss_url_or_a_loopback_ws_url`, `the_handshake_is_the_url_as_a_client_request`, `a_server_close_is_one_outage_that_names_its_code`, `a_dropped_connection_names_its_error`, `a_stalled_socket_is_replaced`, `every_connection_replays_the_whole_membership_changes_included`, `a_change_sends_the_ids_that_change_and_nothing_else`, `a_change_during_an_outage_is_answered_at_once_and_replayed`, `a_501st_id_is_refused_and_changes_nothing`, `connecting_with_more_than_500_ids_is_refused_before_connecting`, `it_pings_on_the_schedule_and_a_pong_is_no_event`, `an_empty_membership_stays_connected_and_keeps_pinging`, `unsubscribing_the_last_id_keeps_the_connection`, `keep_alives_are_skipped_and_a_binary_frame_is_decoded_as_text`, `an_array_frame_yields_every_book`, `a_frame_that_does_not_decode_is_an_error_and_the_stream_goes_on`, `a_close_the_venue_refuses_with_ends_the_stream`, `the_first_failure_is_returned_not_retried` and `the_supervisor_dials_with_clobs_per_address_connect`.

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --doc clob::ws`

Expected: `test result: ok.`, the module's examples compiling without running, the new `Supervised Market Socket` one among them.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket --all-features'`

Expected: no output from `cargo fmt`, clippy and rustdoc finish with no warning.

Run: `nix develop --command python3 scripts/gen_registry.py --check && nix develop --command python3 scripts/venue_gate.py && nix develop --command python3 scripts/live_unwraps.py && nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`

Expected: each exits 0; the test-bodies check reports no changed and no lost item, since every test here is new.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-polymarket/src/clob/ws/supervised/market.rs
git commit -m "feat(polymarket): the supervised CLOB market socket (Epic 7, Story 2)" -- polyoxide-polymarket/src/clob/ws/mod.rs polyoxide-polymarket/src/clob/ws/supervised/mod.rs polyoxide-polymarket/src/clob/ws/supervised/market.rs CLAUDE.md
```

## Story 3: The supervised CLOB user socket (Story 7.2)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`, `polyoxide-polymarket/src/clob/ws/mod.rs`, `CLAUDE.md`
- **Interfaces:** exposes `clob::ws::{SupervisedUserWs, UserWsBuilder, ClobUserProtocol, UserEvent, UserChange, UserMembership}` (names contract, Story 3); uses Story 2's `DisconnectReason`, `checked_url`, `default_timing`, `disconnect_reason`, `recover`, `data_text` and `scripted::pong`, and Story 1's `dial` and count helpers
- **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws`
- **Review:** deep (credentials replayed; an unbounded queue)

Adds `clob::ws::supervised::user`: `ClobUserProtocol` on the kit's `Supervisor` (spec §5), with `UserWsBuilder`, `UserEvent`, `UserChange`, `UserMembership` and `SupervisedUserWs`. Its membership is `Option<Vec<String>>`: `None` subscribes every market and the subscribe frame omits `markets`, an empty `Some` sends `"markets": []` (subscribes none), and a test pins that unsubscribing the last market leaves `Some(vec![])` and never collapses into `None`. `replay` builds the credential-bearing `UserSubscription` afresh on every connection, so a reconnect replays the credentials; the builder holds none and takes them in `connect` and `start`. The queue is `Queue::Unbounded` (the owner's choice, so the kit never waits on the consumer), and a test with a blocked consumer proves it through the server's close reply, which a bounded queue never lets the task reach. Task 3.1 builds all of that on Story 2's helpers and Story 1's `dial` (the protocol overrides `Protocol::connect` with it, D20); Task 3.2 adds the authentication stop: the server's 1008 authentication close decoded as `ClobWsError::Authentication`, on which recovery stops, so a bad credential ends the stream instead of reconnecting at the backoff ceiling; Task 3.3 adds the CLAUDE.md paragraph and edits the sentence that called `Authentication` never constructed (AD-21). No landmine marker appears in this story's code: it dials only through `dial`, matches nothing over `tungstenite::Error` and adds no `From` for a foreign error, and the stream's errors reach `recover` as `ClobWsError` values.

### Task 3.1: The user protocol, its builder and its membership

**Files:**
- Create: `polyoxide-polymarket/src/clob/ws/supervised/user.rs`
- Modify: `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`
- Modify: `polyoxide-polymarket/src/clob/ws/mod.rs`
- Test: `polyoxide-polymarket/src/clob/ws/supervised/user.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-polymarket/src/clob/ws/supervised/user.rs` holding a placeholder first line and the tests. The placeholder is replaced by the implementation in Step 3.

```rust
// The implementation goes here (Task 3.1, Step 3).

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use polyoxide_ws::test_server::{next_event, Script, ScriptedServer};
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
        assert_eq!(nothing, json!({"type": "user", "markets": [], "auth": auth()}));
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
        let err = amend(&mut membership, UserChange::Unsubscribe(ids(&["a"])))
            .expect_err("a refusal");
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
        assert_eq!(
            frame(&server, 0),
            json!({"type": "user", "auth": auth()})
        );
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
    async fn changes_send_one_update_for_what_they_changed() {
        let server = serve(vec![Script::hold()]).await;
        let ws = builder(&server)
            .connect(Some(ids(&["a"])), credentials())
            .await
            .expect("connects");
        let handle = ws.membership();
        handle.subscribe(ids(&["b", "a", "b"])).await.expect("added");
        handle.subscribe(ids(&["a"])).await.expect("already held");
        handle.unsubscribe(ids(&["zz"])).await.expect("not held");
        handle.unsubscribe(ids(&["a", "zz"])).await.expect("removed");
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
            .wait_for("the second subscribe frame", |s| s.first_frames().len() == 2)
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
            .wait_for("the replayed subscription", |s| {
                s.first_frames().len() == 1
            })
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
}
```

Declare the module and re-export its items so the tests can find them.

Edit 1, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`. Current (the group Story 2 wrote, with a blank line before and after it):

```rust
mod market;

pub use market::{
    ClobMarketProtocol, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder,
    SupervisedMarketWs,
};
```

Replace with:

```rust
mod market;
mod user;

pub use market::{
    ClobMarketProtocol, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder,
    SupervisedMarketWs,
};
pub use user::{
    ClobUserProtocol, SupervisedUserWs, UserChange, UserEvent, UserMembership, UserWsBuilder,
};
```

Edit 2, `polyoxide-polymarket/src/clob/ws/mod.rs`. Current (the block Story 2 inserted between the `pub use subscription::{…};` block and `pub use user::{…};`):

```rust
pub use supervised::{
    ClobMarketProtocol, DisconnectReason, MarketChange, MarketEvent, MarketMembership,
    MarketWsBuilder, SupervisedMarketWs,
};
```

Replace with:

```rust
pub use supervised::{
    ClobMarketProtocol, DisconnectReason, MarketChange, MarketEvent, MarketMembership,
    MarketWsBuilder, SupervisedMarketWs,
};
pub use supervised::{
    ClobUserProtocol, SupervisedUserWs, UserChange, UserEvent, UserMembership, UserWsBuilder,
};
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws::supervised::user
```

Expected: the crate does not compile. The errors include `cannot find type `ClobUserProtocol` in this scope`, `cannot find function `amend` in this scope`, `cannot find type `UserWsBuilder` in this scope` and `cannot find type `UserChange` in this scope`, and the two `pub use` lines fail with `unresolved imports `user::ClobUserProtocol`, `user::SupervisedUserWs`, ...`.

- [ ] **Step 3: Implement**

In `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, replace the placeholder first line. Current:

```rust
// The implementation goes here (Task 3.1, Step 3).
```

Replace with:

```rust
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
///   reconnects exactly when its class is retriable.
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
    fn subscribe(&self, markets: Vec<String>) -> impl Future<Output = Result<(), ClobWsError>> + Send;

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

    /// The initial membership: refused past 500 markets, else without repeats.
    fn membership(markets: Option<Vec<String>>) -> Result<Option<Vec<String>>, ClobWsError> {
        if let Some(markets) = &markets {
            validate_subscription_count(markets.len())?;
        }
        Ok(markets.map(distinct))
    }

    fn protocol(&self, credentials: ApiCredentials) -> ClobUserProtocol {
        ClobUserProtocol::new(self.url.clone(), credentials)
    }
}
```

Then run `nix develop --command cargo fmt --all` so the file's line breaks match rustfmt (the `subscribe` signature and the `tracing::debug!` line are the ones it may rewrap).

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws::supervised::user
nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws --all-targets -- -D warnings
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws
```

Expected: the first prints `test result: ok. 30 passed; 0 failed`, and the third passes every `clob::ws` test, the bare tier's and Story 2's included. Clippy and rustfmt are clean. Under a mutant that returns `Queue::Bounded(1024)` from `queue`, both `the_queue_is_unbounded` and `a_burst_the_consumer_has_not_read_is_taken_off_the_socket_and_arrives_whole` fail (the second on `timed out waiting for the close reply`); under one that makes `amend` collapse an emptied filter to `None`, `removing_the_last_market_leaves_an_empty_filter_never_every_market` and `the_last_market_removed_is_replayed_as_an_empty_filter` fail.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-polymarket/src/clob/ws/supervised/user.rs
git commit -m "feat(polymarket): supervised CLOB user socket (Story 7.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-polymarket/src/clob/ws/supervised/user.rs polyoxide-polymarket/src/clob/ws/supervised/mod.rs polyoxide-polymarket/src/clob/ws/mod.rs
```

### Task 3.2: The server's 1008 authentication close stops the stream

**Files:**
- Modify: `polyoxide-polymarket/src/clob/ws/supervised/user.rs`
- Test: `polyoxide-polymarket/src/clob/ws/supervised/user.rs`

- [ ] **Step 1: Write the failing test**

Edit 1, `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, in the `tests` module: bring in the items the new tests use. Current:

```rust
    use polyoxide_ws::test_server::{next_event, Script, ScriptedServer};
```

Replace with:

```rust
    use polyoxide_venue::Class;
    use polyoxide_ws::{
        test_server::{next_event, next_item, Script, ScriptedServer},
        tungstenite::protocol::{frame::coding::CloseCode, CloseFrame},
    };
```

Edit 2, `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, same module: append the tests after the last one. Current (the end of `debug_output_redacts_the_credentials`, and of the module):

```rust
        for secret in ["key-1", "secret-1", "pass-1"] {
            assert!(!shown.contains(secret), "{shown}");
        }
    }
}
```

Replace with:

```rust
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

    #[test]
    fn any_other_close_decodes_to_nothing_and_ends_through_the_kit() {
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
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws::supervised::user
```

Expected: the module compiles, and three of the four new tests fail while `any_other_close_decodes_to_nothing_and_ends_through_the_kit` passes, since a close already decodes to nothing:

```
test clob::ws::supervised::user::tests::a_1008_close_decodes_as_an_authentication_failure ... FAILED
test clob::ws::supervised::user::tests::a_refused_login_ends_the_stream_without_a_reconnect ... FAILED
```

The first panics with `expected an authentication failure, got Nothing`; the second with `expected the refusal, got Disconnected { reason: Closed { code: Some(1008), reason: "authentication failed" } }`, since the stream reconnects. `an_authentication_failure_is_unauthorized_and_recovery_stops_on_it` already passes, because Story 2's `recover` stops on the variant; it pins that rule for this socket.

- [ ] **Step 3: Implement**

Edit 1, `polyoxide-polymarket/src/clob/ws/supervised/user.rs`: decode the close. Current:

```rust
    fn decode(&mut self, frame: &Message) -> Decoded<UserEvent, ClobWsError> {
        match data_text(frame) {
            Some(text) => decode_text(text),
            None => Decoded::Nothing,
        }
    }
```

Replace with:

```rust
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
```

Edit 2, `polyoxide-polymarket/src/clob/ws/supervised/user.rs`: say so in the protocol's docs. Current:

```rust
/// - **Recovery:** a frame that does not decode is skipped; any other error
///   reconnects exactly when its class is retriable.
```

Replace with:

```rust
/// - **Recovery:** a frame that does not decode is skipped; any other error
///   reconnects exactly when its class is retriable, except a close with code
///   1008, the server's answer to credentials it refuses (it sends no error
///   frame): that is [`ClobWsError::Authentication`], carrying the close
///   reason, and the stream yields it and ends, since retrying cannot fix a
///   bad credential and would reconnect for ever at the backoff ceiling.
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --lib clob::ws::supervised::user
nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws --all-targets -- -D warnings
nix develop --command cargo fmt --all -- --check
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket --no-default-features --features clob-ws'
```

Expected: `test result: ok. 34 passed; 0 failed`, then clippy, rustfmt and rustdoc print nothing to fix. Without Edit 1, `a_refused_login_ends_the_stream_without_a_reconnect` fails as in Step 2.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): the user socket stops on a refused login (Story 7.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-polymarket/src/clob/ws/supervised/user.rs
```

### Task 3.3: The CLAUDE.md paragraph (AD-21)

**Files:**
- Modify: `CLAUDE.md:722,724`

- [ ] **Step 1: Write the failing check**

The paragraph does not exist yet, and a sentence in the WebSocket paragraph still calls `Authentication` never constructed.

```bash
grep -c 'The supervised CLOB user socket' CLAUDE.md
grep -c 'never-constructed `ConnectionClosed` and `Authentication` variants' CLAUDE.md
```

- [ ] **Step 2: Run it to see it fail**

Expected: the first prints `0` (the paragraph is missing) and the second prints `1` (the stale sentence is still there).

- [ ] **Step 3: Implement**

Edit 1, `CLAUDE.md:722` (part of one long line, the "**WebSocket** support for the CLOB" paragraph). Current:

```
whose never-constructed `ConnectionClosed` and `Authentication` variants stay until S3, because a protected test builds them.
```

Replace with:

```
whose never-constructed `ConnectionClosed` variant stays until S3, because a protected test builds it (`Authentication` is built by the supervised user socket, on the server's 1008 close).
```

Edit 2, `CLAUDE.md`, the line that begins with the text below (the start of the socket-kit paragraph, part of one long line; line 724 before Story 2 inserts its paragraph, and a few lines later after). Insert this paragraph, then a blank line, in front of it, so it follows Story 2's paragraph. Current:

```
**The socket kit is `polyoxide-ws`; perps and Binance run on its `Supervisor` (Stories 4.6 and 4.7), and rtds, sports and clob on its blocks (Stories 4.8 and 4.9).**
```

Replace with (the first line is the new paragraph, one line in the file; then a blank line; then the Current text unchanged):

```
**The supervised CLOB user socket** (Story 7.2) is `UserWsBuilder` and `SupervisedUserWs` in `polyoxide-polymarket`'s `clob::ws::supervised`, beside the market socket and on the same kit `Supervisor`, with `ClobUserProtocol`, `UserEvent`, `UserChange` and `UserMembership`, all at `clob::ws`. Its membership is `Option<Vec<String>>` (AD-11): `None` follows every market and the subscribe frame omits `markets`; `Some(markets)` carries them; `Some(vec![])` sends `"markets": []`, which follows none. Nothing ever turns a `Some` into `None`, so unsubscribing the last market and then reconnecting replays an empty filter and does not widen the subscription to every market (a test pins it). A subscribe on `None` changes nothing and sends nothing, an unsubscribe on `None` is refused as `InvalidMessage`, and on a `Some` each change sends one update frame for the markets it actually changed, at most 500 per connection. A change is recorded before its frame is sent and one made during an outage is answered at once, so the replay carries both. The protocol holds the account's `ApiCredentials` and rebuilds the credential-bearing `UserSubscription` on every connection, so a reconnect replays them; the builder holds none, and `connect` and `start` take them (`start` begins in an outage when the first attempt fails in a way recovery would retry). The queue is `Queue::Unbounded` by the owner's choice (AD-24), so the kit never waits on the consumer and a consumer that stops reading loses no fill, at the cost of memory that grows with its backlog. A server close with code 1008 is how the host refuses credentials (about 1.2 s after the subscribe, with no error frame; probed live 2026-10-17): it decodes as `ClobWsError::Authentication(<the close reason>)`, classed `Unauthorized`, and recovery stops on it, so the stream yields the error and ends instead of reconnecting for ever at the backoff ceiling; it is that variant's first use. Every `UserEvent::Disconnected` is followed by one `UserEvent::Reconnected`, and what the socket carried in between is lost, so a consumer reconciles there. A frame that does not decode is yielded as an error and skipped, a JSON array yields one event per element, and an array with an element that does not decode is skipped whole. The URL must be `wss://`, or `ws://` to a loopback host, so credentials never travel unencrypted to a remote host. Its suites drive the kit's scripted server through `src/clob/ws/scripted.rs`'s `PING`/`PONG` answerer.

**The socket kit is `polyoxide-ws`; perps and Binance run on its `Supervisor` (Stories 4.6 and 4.7), and rtds, sports and clob on its blocks (Stories 4.8 and 4.9).**
```

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'The supervised CLOB user socket' CLAUDE.md
grep -c 'never-constructed `ConnectionClosed` and `Authentication` variants' CLAUDE.md
python3 scripts/gen_registry.py --check
python3 scripts/venue_gate.py
```

Expected: `1`, then `0`, and the two scripts exit 0 (CLAUDE.md's generated regions are untouched, and a hand-written paragraph names no foundation-crate term). Then run the story's gates, `nix develop --command cargo test -p polyoxide-polymarket --all-features --lib clob::ws` among them.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(claude): the supervised CLOB user socket (Story 7.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- CLAUDE.md
```

## Story 4: The `Trading` trait (Story 7.3)

- **Developer:** intermediate-dev
- **Files:** `Cargo.toml`, `polyoxide-venue/Cargo.toml`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/src/ids.rs`, `polyoxide-venue/src/trading.rs`, `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/tests/trading.rs`, `polyoxide-venue/README.md`, `CLAUDE.md`, `README.md`, `Cargo.lock`
- **Interfaces:** exposes, under `polyoxide-venue/trading`, `Trading`, `DynTrading`, `TradingEvents`, `TradingEvent`, `OrderRequest`, `Order`, `OrderStatus`, `KillReason`, `TimeInForce`, `Fill`, `Liquidity`, `Position`, `Balance`, `OrderId`, `FillId` and `ClientOrderId`, and under `market` `Extensions::ensure_declared` and `UndeclaredExtension` (names contract, Story 4); uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-venue --all-features && cargo hack check -p polyoxide-venue --each-feature --no-dev-deps`
- **Review:** deep (the public trait every trading venue implements)

Adds `polyoxide-venue`'s `trading` feature (spec §6). It holds the `Trading` trait with `DynTrading`, and `TradingEvents` (A7-1), the newtype that keeps the stream `'static` through the dyn wrapper. It holds the trading records and enums, and the opaque `OrderId`, `FillId` and `ClientOrderId`. And it adds `Extensions::ensure_declared` with `UndeclaredExtension`, the refusal a venue runs on an undeclared option. The design is Lane B's (tag `lane-b-bundle-n`), ported onto Epic 6's types with D3's leading `Resync` and D4's void rule in the docs: the records expose `pub extensions` and have no `ext_accessor!`, `TradeId` stays in `records.rs`, and the tests are written onto `KeySpace`. Its tests prove static and `Arc<DynTrading<'static>>` use, and a stream that outlives its `Arc` on another thread. Everything is in a foundation crate, so docs and tests use the made-up venue `acme` and never a registered venue or product word. No landmine marker is in this story's code. Every command below runs with the `nix develop --command` prefix. The story's four tasks are the manifests with the ids, `Extensions::ensure_declared`, the trait with its records and proofs, and the docs with the regenerated crate lists.

### Task 4.1: The feature, the dependency and the opaque ids

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]`, the line `futures-util = "0.3"`)
- Modify: `polyoxide-venue/Cargo.toml:14-48`
- Modify: `polyoxide-venue/src/lib.rs:56-71,78-79`
- Create: `polyoxide-venue/src/ids.rs`
- Modify: `Cargo.lock` (cargo writes it)
- Test: `polyoxide-venue/src/ids.rs`

- [ ] **Step 1: Write the failing test**

In the root `Cargo.toml`, add the workspace entry. Current:

```toml
futures-util = "0.3"
```

Replace with:

```toml
futures-util = "0.3"
# `Stream`, for the trading trait's event stream (`polyoxide-venue`'s `trading`).
futures-core = "0.3"
```

In `polyoxide-venue/Cargo.toml`, make `futures-core` an optional dependency. Current:

```toml
dynosaur = { workspace = true, optional = true }
rust_decimal = { workspace = true, optional = true }
```

Replace with:

```toml
dynosaur = { workspace = true, optional = true }
# The `Stream` of `TradingEvents` (`trading`).
futures-core = { workspace = true, optional = true }
rust_decimal = { workspace = true, optional = true }
```

Add the feature. Current:

```toml
market = ["keys", "decimal", "dep:dynosaur"]
```

Replace with:

```toml
market = ["keys", "decimal", "dep:dynosaur"]
# The `Trading` trait (with its `Dyn` wrapper), the order, fill, position and
# balance records, the opaque order, fill and client-order ids, and the
# `TradingEvents` stream.
trading = ["market", "dep:futures-core"]
```

Add the dev-dependencies the trading proofs use. Current:

```toml
polyoxide-ws = { path = "../polyoxide-ws" }

# docs.rs documents `positional`, the keys and the records, which only their
# features build.
[package.metadata.docs.rs]
features = ["decimal", "keys", "market"]
```

Replace with:

```toml
polyoxide-ws = { path = "../polyoxide-ws" }
# `tests/trading.rs` drives the event stream from a spawned task and from
# polled-once futures.
futures-util = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time"] }

# docs.rs documents `positional`, the keys, the records and the trading
# vocabulary, which only their features build.
[package.metadata.docs.rs]
features = ["decimal", "keys", "market", "trading"]
```

In `polyoxide-venue/src/lib.rs`, declare the module. Current:

```rust
#[cfg(feature = "market")]
mod extensions;
#[cfg(feature = "keys")]
mod key;
```

Replace with:

```rust
#[cfg(feature = "market")]
mod extensions;
#[cfg(feature = "trading")]
mod ids;
#[cfg(feature = "keys")]
mod key;
```

Create `polyoxide-venue/src/ids.rs` holding the tests alone, which name items the next step adds:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_its_text() {
        let id = OrderId::from("ee587a1c-0001");
        assert_eq!(id.as_str(), "ee587a1c-0001");
        assert_eq!(id.to_string(), "ee587a1c-0001");
        assert_eq!(format!("{id:?}"), r#"OrderId("ee587a1c-0001")"#);
        assert_eq!(id, OrderId::new(String::from("ee587a1c-0001")));
        assert_eq!(FillId::new("f").as_str(), "f");
        assert_eq!(ClientOrderId::from(String::from("c")).as_str(), "c");
    }

    #[test]
    fn ids_compare_and_hash_by_their_text() {
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(OrderId::from("a")));
        assert!(!seen.insert(OrderId::new("a")), "one text is one id");
        assert!(seen.insert(OrderId::from("b")));
        assert!(OrderId::from("a") < OrderId::from("b"));
        assert_ne!(OrderId::from("a"), OrderId::from("A"));
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features trading --lib ids`

Expected: it does not compile, with `error[E0433]` or `error[E0412]` naming `OrderId`, `FillId` and `ClientOrderId` as not found in this scope.

- [ ] **Step 3: Implement**

Replace the whole of `polyoxide-venue/src/ids.rs` with the macro, the three ids and the tests above:

```rust
//! Opaque ids for orders, fills and the caller's own order names, as each
//! venue spells them.

use std::fmt;
use std::sync::Arc;

macro_rules! opaque_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        ///
        /// Opaque: compared and hashed as the venue's text, never parsed.
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Arc<str>);

        impl $name {
            /// The id the venue sent, or the caller chose.
            pub fn new(id: impl Into<Arc<str>>) -> Self {
                Self(id.into())
            }

            /// The id, as the venue spells it.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.into())
            }
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id.into())
            }
        }
    };
}

opaque_id! {
    /// The venue's id for an order.
    OrderId
}

opaque_id! {
    /// The venue's id for a fill: one order's part in a trade.
    FillId
}

opaque_id! {
    /// The id a caller gives an order, which the venue echoes back or the
    /// implementation keeps for the orders it placed.
    ClientOrderId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_its_text() {
        let id = OrderId::from("ee587a1c-0001");
        assert_eq!(id.as_str(), "ee587a1c-0001");
        assert_eq!(id.to_string(), "ee587a1c-0001");
        assert_eq!(format!("{id:?}"), r#"OrderId("ee587a1c-0001")"#);
        assert_eq!(id, OrderId::new(String::from("ee587a1c-0001")));
        assert_eq!(FillId::new("f").as_str(), "f");
        assert_eq!(ClientOrderId::from(String::from("c")).as_str(), "c");
    }

    #[test]
    fn ids_compare_and_hash_by_their_text() {
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(OrderId::from("a")));
        assert!(!seen.insert(OrderId::new("a")), "one text is one id");
        assert!(seen.insert(OrderId::from("b")));
        assert!(OrderId::from("a") < OrderId::from("b"));
        assert_ne!(OrderId::from("a"), OrderId::from("A"));
    }
}
```

Export them from `polyoxide-venue/src/lib.rs`. Current:

```rust
#[cfg(feature = "keys")]
pub use key::{KeyError, KeySpace, MarketKey, ProductId, RawKey, VenueId};
```

Replace with:

```rust
#[cfg(feature = "trading")]
pub use ids::{ClientOrderId, FillId, OrderId};
#[cfg(feature = "keys")]
pub use key::{KeyError, KeySpace, MarketKey, ProductId, RawKey, VenueId};
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --features trading --lib ids`

Expected: `an_id_is_its_text` and `ids_compare_and_hash_by_their_text` pass.

Then run `nix develop --command cargo check -p polyoxide-venue --no-default-features` (nothing of `trading` builds without the feature) and `nix develop --command cargo fmt --all -- --check`.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-venue/src/ids.rs
git commit -m "feat(venue): Story 7.3, the trading feature and the opaque order, fill and client-order ids" -- Cargo.toml Cargo.lock polyoxide-venue/Cargo.toml polyoxide-venue/src/lib.rs polyoxide-venue/src/ids.rs
```

### Task 4.2: `Extensions::ensure_declared` and `UndeclaredExtension`

**Files:**
- Modify: `polyoxide-venue/src/extensions.rs:12-19,36-39,183-187,247-253,345-351`
- Modify: `polyoxide-venue/src/lib.rs:76-77,100-109`
- Test: `polyoxide-venue/src/extensions.rs`

- [ ] **Step 1: Write the failing test**

Append four tests to `extensions.rs`'s test module as new items, after its last test. Current:

```rust
        assert_eq!(format!("{:?}", Extensions::new()), "{}");
    }
}
```

Replace with:

```rust
        assert_eq!(format!("{:?}", Extensions::new()), "{}");
    }

    #[test]
    fn declared_extensions_and_an_empty_set_pass() {
        let ext = Extensions::of(QuoteExt { update_id: 7 });
        assert_eq!(ext.ensure_declared(&[TypeId::of::<QuoteExt>()]), Ok(()));
        assert_eq!(
            ext.ensure_declared(&[TypeId::of::<BookExt>(), TypeId::of::<QuoteExt>()]),
            Ok(())
        );
        assert_eq!(Extensions::new().ensure_declared(&[]), Ok(()));
        assert_eq!(
            Extensions::new().ensure_declared(&[TypeId::of::<QuoteExt>()]),
            Ok(())
        );
    }

    #[test]
    fn an_undeclared_value_is_refused_by_its_type_name_as_invalid_request() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        ext.insert(BookExt { sequence: 9 }).unwrap();

        let err = ext
            .ensure_declared(&[TypeId::of::<QuoteExt>()])
            .unwrap_err();
        assert!(err.type_name.ends_with("BookExt"), "{err}");
        assert_eq!(err.class(), Class::InvalidRequest);
        assert_eq!(
            err.to_string(),
            format!(
                "the request carries `{}`, an extension this venue does not declare",
                err.type_name
            )
        );

        let err = ext.ensure_declared(&[]).unwrap_err();
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[test]
    fn the_first_undeclared_value_in_type_id_order_is_the_one_named() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        ext.insert(BookExt { sequence: 9 }).unwrap();

        let first = if TypeId::of::<QuoteExt>() < TypeId::of::<BookExt>() {
            "QuoteExt"
        } else {
            "BookExt"
        };
        let err = ext.ensure_declared(&[]).unwrap_err();
        assert!(err.type_name.ends_with(first), "{err}");
    }

    #[test]
    fn declaring_a_type_the_request_lacks_does_not_excuse_one_it_has() {
        let ext = Extensions::of(QuoteExt { update_id: 7 });
        let err = ext.ensure_declared(&[TypeId::of::<BookExt>()]).unwrap_err();
        assert!(err.type_name.ends_with("QuoteExt"), "{err}");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features market --lib extensions`

Expected: it does not compile: `error[E0599]: no method named \`ensure_declared\` found for struct \`Extensions\``.

- [ ] **Step 3: Implement**

In `extensions.rs`, the private trait gains the type name. Current:

```rust
    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
    fn as_any(&self) -> &dyn Any;
```

Replace with:

```rust
    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
    fn type_name_dyn(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
```

And its blanket impl. Current:

```rust
    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
```

Replace with:

```rust
    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }

    fn type_name_dyn(&self) -> &'static str {
        std::any::type_name::<T>()
    }
```

Add the method to `Extensions`. Current:

````rust
    /// Whether no value is held.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
````

Replace with:

````rust
    /// Whether no value is held.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Checks that every value held is of a type in `declared`, the option
    /// types a venue reads.
    ///
    /// A request carries the venue's own options as extensions, and a venue
    /// that reads only some option types refuses the rest rather than ignoring
    /// them. An empty set always passes.
    ///
    /// # Errors
    ///
    /// [`UndeclaredExtension`] naming the first value held, in `TypeId` order,
    /// whose type is not in `declared`.
    ///
    /// ```
    /// use std::any::TypeId;
    ///
    /// use polyoxide_venue::Extensions;
    ///
    /// #[derive(Debug, Clone, PartialEq)]
    /// struct Options;
    ///
    /// #[derive(Debug, Clone, PartialEq)]
    /// struct Other;
    ///
    /// let ext = Extensions::of(Options);
    /// assert!(ext.ensure_declared(&[TypeId::of::<Options>()]).is_ok());
    /// assert!(ext.ensure_declared(&[TypeId::of::<Other>()]).is_err());
    /// assert!(Extensions::new().ensure_declared(&[]).is_ok());
    /// ```
    pub fn ensure_declared(&self, declared: &[TypeId]) -> Result<(), UndeclaredExtension> {
        match self.values.iter().find(|(id, _)| !declared.contains(id)) {
            Some((_, value)) => Err(UndeclaredExtension {
                type_name: value.as_ref().type_name_dyn(),
            }),
            None => Ok(()),
        }
    }
}
````

Add the error type. Current:

```rust
impl Classify for DuplicateExtension {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}
```

Replace with:

```rust
impl Classify for DuplicateExtension {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

/// A request carried an extension of a type its venue does not declare
/// ([`Extensions::ensure_declared`]). An
/// [`InvalidRequest`](Class::InvalidRequest): the caller's misuse, refused
/// before anything is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UndeclaredExtension {
    /// The type the venue does not declare.
    pub type_name: &'static str,
}

impl fmt::Display for UndeclaredExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the request carries `{}`, an extension this venue does not declare",
            self.type_name
        )
    }
}

impl Error for UndeclaredExtension {}

impl Classify for UndeclaredExtension {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}
```

Export it from `polyoxide-venue/src/lib.rs`. Current:

```rust
#[cfg(feature = "market")]
pub use extensions::{DuplicateExtension, Extensions};
```

Replace with:

```rust
#[cfg(feature = "market")]
pub use extensions::{DuplicateExtension, Extensions, UndeclaredExtension};
```

Then list it in `polyoxide-venue/src/lib.rs`'s `Classify` assertion. Current:

```rust
    #[cfg(feature = "market")]
    is::<NotListed>();
};
```

Replace with:

```rust
    #[cfg(feature = "market")]
    is::<NotListed>();
    #[cfg(feature = "market")]
    is::<UndeclaredExtension>();
};
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --all-features extensions`

Expected: the four new tests, the existing ones and the `ensure_declared` doctest pass. Then run `nix develop --command cargo clippy -p polyoxide-venue --all-targets --all-features -- -D warnings` and `nix develop --command cargo fmt --all -- --check`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(venue): Story 7.3, Extensions::ensure_declared and UndeclaredExtension" -- polyoxide-venue/src/extensions.rs polyoxide-venue/src/lib.rs
```

### Task 4.3: The `Trading` trait, `TradingEvents` and the trading records

**Files:**
- Create: `polyoxide-venue/src/trading.rs`
- Modify: `polyoxide-venue/src/lib.rs:27-33,70-72,94-95`
- Create: `polyoxide-venue/tests/trading.rs`
- Test: `polyoxide-venue/tests/trading.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-venue/tests/trading.rs`. The proofs are Lane B's, rewritten onto `KeySpace` with the made-up venue `acme`. The paper venue follows the stream rule: each `events()` call queues the leading `Resync`.

```rust
#![cfg(feature = "trading")]
//! `Trading` on a synthetic paper venue, `acme.binary`: used statically and
//! as `Arc<DynTrading<'static>>`, with kills as `Ok`, undeclared options
//! refused before anything is sent, and an event stream that begins with
//! `Resync`, yields `Ack`, `Fill` and `Resync` in order and outlives the venue
//! handle it came from.
//!
//! The paper venue answers at once, so `now_or_never` drives every call
//! without a runtime.

use std::any::TypeId;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::time::Duration;

use futures_core::Stream;
use futures_util::{FutureExt, StreamExt};
use polyoxide_venue::{
    Balance, Class, ClassifiedError, ClientOrderId, DynTrading, Fill, FillId, KeyError, KeySpace,
    KillReason, Liquidity, MarketKey, Order, OrderId, OrderRequest, OrderStatus, Position,
    ProductId, Side, TimeInForce, TradeId, Trading, TradingEvent, TradingEvents,
    UndeclaredExtension, UnixMillis, VenueId,
};
use rust_decimal::Decimal;

/// Event contracts: one key per outcome, `<TICKER>:yes|no`.
struct Binary;

impl KeySpace for Binary {
    const VENUE: VenueId = VenueId::from_static("acme");
    const PRODUCT: ProductId = ProductId::from_static("binary");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        match text.rsplit_once(':') {
            Some((ticker, "yes" | "no")) if !ticker.is_empty() => Ok(text.to_owned()),
            _ => Err(format!("`{text}` has no `:yes` or `:no` outcome")),
        }
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

/// The one option type the paper venue declares.
#[derive(Debug, Clone, PartialEq)]
struct PaperOrderOptions {
    expires_at: Option<UnixMillis>,
}

/// An option it does not.
#[derive(Debug, Clone, PartialEq)]
struct Iceberg {
    visible: Decimal,
}

/// The paper venue's own fill facts.
#[derive(Debug, Clone, PartialEq)]
struct PaperFillExt {
    post_position: Decimal,
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn yes() -> MarketKey {
    Binary::key(&"RAIN:yes".to_owned())
}

/// The paper venue's state: liquidity to take at any price, what it holds,
/// and the unbounded queue `events()` drains. One queue serves one consumer.
#[derive(Default)]
struct State {
    liquidity: Decimal,
    sent: usize,
    next: u64,
    orders: Vec<Order>,
    position: Decimal,
    queue: VecDeque<Result<TradingEvent, ClassifiedError>>,
    waker: Option<Waker>,
}

#[derive(Clone)]
struct Paper {
    state: Arc<Mutex<State>>,
}

impl Paper {
    fn with_liquidity(liquidity: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                liquidity: dec(liquidity),
                ..State::default()
            })),
        }
    }

    /// Requests that reached the venue.
    fn sent(&self) -> usize {
        self.state.lock().unwrap().sent
    }

    fn push(state: &mut State, event: TradingEvent) {
        state.queue.push_back(Ok(event));
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    /// What a `Trading` impl does when its fills connection reconnects.
    fn reconnected(&self) {
        Self::push(&mut self.state.lock().unwrap(), TradingEvent::Resync);
    }

    fn check_key(key: &MarketKey) -> Result<(), ClassifiedError> {
        Binary::id(key)?;
        Ok(())
    }
}

impl Trading for Paper {
    async fn place(&self, request: OrderRequest) -> Result<Order, ClassifiedError> {
        Self::check_key(&request.key)?;
        request
            .extensions
            .ensure_declared(&[TypeId::of::<PaperOrderOptions>()])?;
        // The paper venue rests nothing it cannot match and holds no position
        // to reduce, so it honours neither flag: refused, never ignored.
        if request.post_only || request.reduce_only {
            return Err(ClassifiedError::new(
                Class::InvalidRequest,
                "the paper venue takes neither post-only nor reduce-only orders",
            ));
        }

        let mut state = self.state.lock().unwrap();
        state.sent += 1;
        state.next += 1;
        let id = OrderId::from(format!("o-{}", state.next));
        let matchable = request.size.min(state.liquidity);
        let (filled, status) = match request.time_in_force {
            TimeInForce::FillOrKill if matchable < request.size => (
                Decimal::ZERO,
                OrderStatus::Killed {
                    reason: KillReason::Unfillable,
                },
            ),
            TimeInForce::ImmediateOrCancel if matchable < request.size => (
                matchable,
                OrderStatus::Killed {
                    reason: KillReason::Unmatched,
                },
            ),
            _ if matchable == request.size => (matchable, OrderStatus::Filled),
            _ => (matchable, OrderStatus::Open),
        };

        let mut order = Order::new(
            id.clone(),
            request.key.clone(),
            request.side,
            request.price,
            request.size,
            status,
        );
        order.filled = filled;
        order.time_in_force = Some(request.time_in_force);
        order.client_order_id = request.client_order_id.clone();
        order.created = Some(UnixMillis(1_760_000_000_000));
        state.liquidity -= filled;
        if status == OrderStatus::Open {
            state.orders.push(order.clone());
        }
        Self::push(&mut state, TradingEvent::Ack(order.clone()));

        if filled > Decimal::ZERO {
            state.position += match request.side {
                Side::Buy => filled,
                Side::Sell => -filled,
            };
            let mut fill = Fill::new(
                FillId::from(format!("f-{}", state.next)),
                id,
                request.key,
                request.side,
                request.price,
                filled,
                UnixMillis(1_760_000_000_001),
            );
            fill.trade_id = Some(TradeId::new(format!("t-{}", state.next)));
            fill.fee = Some(dec("0.01"));
            fill.liquidity = Some(Liquidity::Taker);
            fill.notional = Some(request.price * filled);
            fill.client_order_id = request.client_order_id;
            let post_position = state.position;
            fill.extensions.insert(PaperFillExt { post_position })?;
            Self::push(&mut state, TradingEvent::Fill(fill));
        }
        Ok(order)
    }

    async fn cancel(&self, id: &OrderId) -> Result<(), ClassifiedError> {
        let mut state = self.state.lock().unwrap();
        state.sent += 1;
        let before = state.orders.len();
        state.orders.retain(|order| order.id != *id);
        if state.orders.len() == before {
            return Err(ClassifiedError::new(
                Class::VenueRefusal { code: None },
                format!("no open order {id}"),
            ));
        }
        Ok(())
    }

    async fn cancel_all(&self, key: Option<&MarketKey>) -> Result<Vec<OrderId>, ClassifiedError> {
        let mut state = self.state.lock().unwrap();
        state.sent += 1;
        let (cancelled, kept) = std::mem::take(&mut state.orders)
            .into_iter()
            .partition::<Vec<_>, _>(|order| key.is_none_or(|key| order.key == *key));
        state.orders = kept;
        Ok(cancelled.into_iter().map(|order| order.id).collect())
    }

    async fn open_orders(&self, key: Option<&MarketKey>) -> Result<Vec<Order>, ClassifiedError> {
        let state = self.state.lock().unwrap();
        Ok(state
            .orders
            .iter()
            .filter(|order| key.is_none_or(|key| order.key == *key))
            .cloned()
            .collect())
    }

    async fn positions(&self) -> Result<Vec<Position>, ClassifiedError> {
        let state = self.state.lock().unwrap();
        Ok(vec![Position::new(yes(), state.position)])
    }

    async fn balances(&self) -> Result<Vec<Balance>, ClassifiedError> {
        let mut balance = Balance::new("USD");
        balance.available = Some(dec("100"));
        Ok(vec![balance])
    }

    fn events(&self) -> TradingEvents {
        // The stream's first item is `Resync`: a fill between this call and
        // the first subscription is covered by a reconciliation.
        Self::push(&mut self.state.lock().unwrap(), TradingEvent::Resync);
        let state = Arc::clone(&self.state);
        TradingEvents::new(futures_util::stream::poll_fn(move |cx| {
            let mut state = state.lock().unwrap();
            match state.queue.pop_front() {
                Some(event) => Poll::Ready(Some(event)),
                None => {
                    state.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }))
    }
}

/// Drives a future the paper venue completes at once, checking it is `Send`.
fn run<F: Future + Send>(future: F) -> F::Output {
    future
        .now_or_never()
        .expect("the paper venue answers at once")
}

/// The bounds a consumer needs to move the stream onto a task of its own.
fn movable<S>(stream: S) -> S
where
    S: Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + Unpin + 'static,
{
    stream
}

fn next(stream: &mut TradingEvents) -> Option<TradingEvent> {
    stream
        .next()
        .now_or_never()
        .flatten()
        .map(|event| event.unwrap())
}

/// Venue-neutral strategy code: buys and reports where the order stands.
async fn buy<T: Trading + ?Sized>(
    venue: &T,
    size: &str,
    time_in_force: TimeInForce,
) -> Result<Order, ClassifiedError> {
    let mut request = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec(size));
    request.time_in_force = time_in_force;
    venue.place(request).await
}

#[test]
fn a_fill_or_kill_that_cannot_fill_is_ok_and_killed_unfillable() {
    let venue = Paper::with_liquidity("3");
    let order = run(buy(&venue, "5", TimeInForce::FillOrKill)).unwrap();
    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unfillable
        }
    );
    assert_eq!(order.filled, Decimal::ZERO);

    let dyn_venue: Arc<DynTrading<'static>> = DynTrading::new_arc(Paper::with_liquidity("3"));
    let order = run(buy(&*dyn_venue, "5", TimeInForce::FillOrKill)).unwrap();
    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unfillable
        }
    );
}

#[test]
fn an_immediate_or_cancel_remainder_is_killed_unmatched_and_says_what_filled() {
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(Paper::with_liquidity("3"));
    let order = run(buy(&*venue, "5", TimeInForce::ImmediateOrCancel)).unwrap();
    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unmatched
        }
    );
    assert_eq!(order.filled, dec("3"));
    assert_eq!(order.time_in_force, Some(TimeInForce::ImmediateOrCancel));
}

#[test]
fn an_undeclared_option_is_refused_as_invalid_request_and_nothing_is_sent() {
    let venue = Paper::with_liquidity("3");
    let mut request = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec("1"));
    request
        .extensions
        .insert(Iceberg {
            visible: dec("0.5"),
        })
        .unwrap();
    assert_eq!(
        request.extensions.get::<Iceberg>().unwrap().visible,
        dec("0.5")
    );

    let dyn_venue: Arc<DynTrading<'static>> = DynTrading::new_arc(venue.clone());
    let err = run(dyn_venue.place(request.clone())).unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    let undeclared = err.source.downcast_ref::<UndeclaredExtension>().unwrap();
    assert!(undeclared.type_name.ends_with("Iceberg"), "{undeclared}");

    assert_eq!(
        run(venue.place(request)).unwrap_err().class,
        Class::InvalidRequest
    );
    assert_eq!(venue.sent(), 0, "nothing reached the venue");

    let mut events = venue.events();
    assert_eq!(next(&mut events), Some(TradingEvent::Resync));
    assert!(next(&mut events).is_none(), "and nothing was acknowledged");
}

#[test]
fn a_declared_option_is_accepted() {
    let venue = Paper::with_liquidity("0");
    let mut request = OrderRequest::new(yes(), Side::Sell, dec("0.60"), dec("2"));
    request.client_order_id = Some(ClientOrderId::from("mine-1"));
    request
        .extensions
        .insert(PaperOrderOptions {
            expires_at: Some(UnixMillis(1_760_003_600_000)),
        })
        .unwrap();
    assert_eq!(
        request
            .extensions
            .get::<PaperOrderOptions>()
            .unwrap()
            .expires_at,
        Some(UnixMillis(1_760_003_600_000))
    );

    let order = run(venue.place(request)).unwrap();
    assert_eq!(order.status, OrderStatus::Open);
    assert_eq!(order.client_order_id, Some(ClientOrderId::from("mine-1")));
    assert_eq!(venue.sent(), 1);
}

#[test]
fn a_flag_the_venue_cannot_honour_is_refused_before_sending() {
    let venue = Paper::with_liquidity("3");
    let mut post_only = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec("1"));
    post_only.post_only = true;
    let mut reduce_only = OrderRequest::new(yes(), Side::Sell, dec("0.56"), dec("1"));
    reduce_only.reduce_only = true;

    for request in [post_only, reduce_only] {
        let err = run(venue.place(request)).unwrap_err();
        assert_eq!(err.class, Class::InvalidRequest);
    }
    assert_eq!(venue.sent(), 0, "nothing reached the venue");
}

#[test]
fn a_key_of_another_product_is_refused_before_sending() {
    struct Swap;
    impl KeySpace for Swap {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("swap");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            Ok(text.to_owned())
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    let venue = Paper::with_liquidity("3");
    let request = OrderRequest::new(
        Swap::key(&"BTCUSD".to_owned()),
        Side::Buy,
        dec("1"),
        dec("1"),
    );
    let err = run(venue.place(request)).unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    assert!(err.source.downcast_ref::<KeyError>().is_some());
    assert_eq!(venue.sent(), 0);
}

#[test]
fn events_begin_with_resync_then_yield_ack_fill_and_resync_in_order_through_dyn() {
    let paper = Paper::with_liquidity("3");
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());
    let mut events = movable(venue.events());
    // The stream needs nothing of the handle it came from.
    drop(venue);

    assert_eq!(
        next(&mut events),
        Some(TradingEvent::Resync),
        "the first item is Resync"
    );
    assert!(next(&mut events).is_none(), "nothing has happened yet");
    let order = run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    assert_eq!(order.status, OrderStatus::Filled);
    paper.reconnected();
    let later = run(buy(&paper, "1", TimeInForce::GoodTilCancelled)).unwrap();

    match next(&mut events) {
        Some(TradingEvent::Ack(acked)) => assert_eq!(acked, order),
        other => panic!("expected the order's Ack, got {other:?}"),
    }
    match next(&mut events) {
        Some(TradingEvent::Fill(fill)) => {
            assert_eq!(fill.order_id, order.id);
            assert_eq!(fill.size, dec("2"));
            assert_eq!(fill.notional, Some(dec("1.12")));
            assert_eq!(fill.liquidity, Some(Liquidity::Taker));
            assert_eq!(
                fill.extensions.get::<PaperFillExt>(),
                Some(&PaperFillExt {
                    post_position: dec("2")
                })
            );
        }
        other => panic!("expected the order's Fill, got {other:?}"),
    }
    assert_eq!(next(&mut events), Some(TradingEvent::Resync));
    match next(&mut events) {
        Some(TradingEvent::Ack(acked)) => assert_eq!(acked.id, later.id),
        other => panic!("expected the later Ack after Resync, got {other:?}"),
    }
    assert!(matches!(next(&mut events), Some(TradingEvent::Fill(_))));
    assert!(next(&mut events).is_none());
}

#[test]
fn the_stream_outlives_a_dropped_dyn_clone_and_moves_to_another_thread() {
    let paper = Paper::with_liquidity("3");
    let held: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());

    // Taken from a temporary clone that is dropped at once, then the original
    // dropped too: nothing the stream could borrow from is left.
    let events = Arc::clone(&held).events();
    drop(held);

    let order = run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    paper.reconnected();

    // `std::thread::spawn` takes only a `Send + 'static` closure, so this
    // compiles only because the stream is both.
    let seen = std::thread::spawn(move || {
        let mut events = events;
        std::iter::from_fn(|| next(&mut events)).collect::<Vec<_>>()
    })
    .join()
    .unwrap();

    assert_eq!(seen.len(), 4, "{seen:?}");
    assert_eq!(seen[0], TradingEvent::Resync);
    assert!(matches!(&seen[1], TradingEvent::Ack(acked) if acked.id == order.id));
    assert!(matches!(&seen[2], TradingEvent::Fill(fill) if fill.order_id == order.id));
    assert_eq!(seen[3], TradingEvent::Resync);
}

#[test]
fn events_from_the_static_venue_have_the_same_bounds() {
    let paper = Paper::with_liquidity("3");
    let mut events = movable(paper.events());
    paper.reconnected();
    assert_eq!(next(&mut events), Some(TradingEvent::Resync), "the first");
    assert_eq!(next(&mut events), Some(TradingEvent::Resync), "the reconnect");
    assert!(next(&mut events).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_consumer_waiting_on_another_task_is_woken_by_the_next_event() {
    let paper = Paper::with_liquidity("3");
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());
    let mut events = venue.events();
    drop(venue);

    let consumer = tokio::spawn(async move {
        let mut seen = Vec::new();
        while seen.len() < 3 {
            seen.push(events.next().await.expect("the stream is open").unwrap());
        }
        seen
    });

    run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    let seen = tokio::time::timeout(Duration::from_secs(5), consumer)
        .await
        .expect("the waiting consumer was woken")
        .unwrap();

    assert_eq!(seen[0], TradingEvent::Resync);
    assert!(matches!(&seen[1], TradingEvent::Ack(_)));
    assert!(matches!(&seen[2], TradingEvent::Fill(_)));
}

#[test]
fn reconciliation_reads_back_statically_and_through_dyn() {
    let paper = Paper::with_liquidity("1");
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());

    let resting = run(buy(&*venue, "4", TimeInForce::GoodTilCancelled)).unwrap();
    assert_eq!(resting.status, OrderStatus::Open);
    assert_eq!(resting.filled, dec("1"));

    let open = run(venue.open_orders(Some(&yes()))).unwrap();
    assert_eq!(open, vec![resting.clone()]);
    assert_eq!(run(paper.open_orders(None)).unwrap(), open);

    let positions = run(venue.positions()).unwrap();
    assert_eq!(positions[0].key, yes());
    assert_eq!(positions[0].size, dec("1"));

    let balances = run(venue.balances()).unwrap();
    assert_eq!(balances[0].asset, "USD");
    assert_eq!(balances[0].available, Some(dec("100")));

    run(venue.cancel(&resting.id)).unwrap();
    let err = run(venue.cancel(&resting.id)).unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert!(run(venue.open_orders(None)).unwrap().is_empty());

    let first = run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    let cancelled = run(venue.cancel_all(Some(&yes()))).unwrap();
    assert_eq!(cancelled, vec![first.id]);
    assert!(run(paper.cancel_all(None)).unwrap().is_empty());
}

#[test]
fn the_constructors_fill_the_documented_defaults() {
    assert_eq!(TimeInForce::default(), TimeInForce::GoodTilCancelled);

    let request = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec("2"));
    assert_eq!(request.key, yes());
    assert_eq!(request.side, Side::Buy);
    assert_eq!(request.price, dec("0.56"));
    assert_eq!(request.size, dec("2"));
    assert_eq!(request.time_in_force, TimeInForce::GoodTilCancelled);
    assert_eq!(request.client_order_id, None);
    assert!(!request.post_only);
    assert!(!request.reduce_only);
    assert!(request.extensions.is_empty());

    let order = Order::new(
        OrderId::from("o"),
        yes(),
        Side::Sell,
        dec("0.6"),
        dec("2"),
        OrderStatus::Open,
    );
    assert_eq!(order.filled, Decimal::ZERO);
    assert_eq!(order.time_in_force, None);
    assert_eq!(order.client_order_id, None);
    assert_eq!(order.created, None);
    assert!(order.extensions.is_empty());

    let fill = Fill::new(
        FillId::from("f"),
        OrderId::from("o"),
        yes(),
        Side::Buy,
        dec("0.56"),
        dec("1"),
        UnixMillis(5),
    );
    assert_eq!(fill.trade_id, None);
    assert_eq!(fill.notional, None);
    assert_eq!(fill.fee, None);
    assert_eq!(fill.liquidity, None);
    assert_eq!(fill.client_order_id, None);
    assert_eq!(fill.time, UnixMillis(5));
    assert!(fill.extensions.is_empty());

    let position = Position::new(yes(), dec("-1"));
    assert_eq!(position.size, dec("-1"));
    assert_eq!(position.average_price, None);
    assert!(position.extensions.is_empty());

    let balance = Balance::new("USD");
    assert_eq!(balance.asset, "USD");
    assert_eq!(balance.total, None);
    assert_eq!(balance.available, None);
    assert!(balance.extensions.is_empty());
}

#[test]
fn the_ids_are_their_text() {
    let order = OrderId::from("ee587a1c-0001");
    assert_eq!(order.as_str(), "ee587a1c-0001");
    assert_eq!(order.to_string(), "ee587a1c-0001");
    assert_eq!(order, OrderId::new(String::from("ee587a1c-0001")));
    assert_ne!(order, OrderId::from("ee587a1c-0002"));
    assert_eq!(FillId::new("f-1").to_string(), "f-1");
    assert_eq!(ClientOrderId::from(String::from("mine-1")).as_str(), "mine-1");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features trading --test trading`

Expected: it does not compile: `error[E0432]: unresolved imports` for `polyoxide_venue::{Balance, DynTrading, Fill, KillReason, Liquidity, Order, OrderRequest, OrderStatus, Position, TimeInForce, Trading, TradingEvent, TradingEvents}`.

- [ ] **Step 3: Implement**

Create `polyoxide-venue/src/trading.rs`:

```rust
//! The trading trait, its records, and the stream it reports on.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use rust_decimal::Decimal;

use crate::{
    ClassifiedError, ClientOrderId, Extensions, FillId, MarketKey, OrderId, Side, TradeId,
    UnixMillis,
};

/// Places, cancels and reconciles orders on one venue, and reports what
/// happens to them.
///
/// Every method returns [`ClassifiedError`], which an implementation builds
/// from its module error with `?`. Held as `Arc<DynTrading<'static>>` by code
/// that picks its venue at run time.
///
/// # Requests
///
/// An [`OrderRequest`] names an outcome or instrument key, a [`Side`], a price
/// and a size in the instrument's native quantity. The venue's own options
/// ride in its `extensions`, and the venue reads only the option types it
/// declares: [`place`](Self::place) refuses a request carrying any other, as
/// [`Class::InvalidRequest`](crate::Class::InvalidRequest), and sends nothing
/// ([`Extensions::ensure_declared`]). So does a key of another venue or
/// product, and so does a request whose `post_only`, `reduce_only` or time in
/// force the venue cannot honour: it is refused, never ignored.
///
/// # Kills
///
/// An immediate-or-cancel order that matched nothing, or a fill-or-kill order
/// that could not fill in full, is not an error: `place` returns `Ok` with
/// [`OrderStatus::Killed`], and its reason. `filled` says how much an
/// immediate-or-cancel order matched before its remainder was cancelled.
///
/// # Events
///
/// [`events`](Self::events) is fed from an unbounded queue, so a slow consumer
/// loses no acknowledgement or fill. The venue replays nothing: what happened
/// while the connection that carries fills was down is not sent again.
///
/// The stream's first item is [`TradingEvent::Resync`], yielded once the
/// connection's first attempt to connect has ended, whether it went live or
/// began in an outage, so a fill between `events()` returning and the first
/// subscription is covered by a reconciliation. `Resync` comes again after
/// every reconnect of the connection that carries fills, and when the venue
/// voids a fill it reported. The consumer reconciles its inventory each
/// time through [`open_orders`](Self::open_orders) and the venue's trade history.
#[dynosaur::dynosaur(pub DynTrading = dyn(box) Trading)]
pub trait Trading: Send + Sync {
    /// Places `order` and returns it as the venue accepted it: open, filled,
    /// or killed.
    fn place(
        &self,
        order: OrderRequest,
    ) -> impl Future<Output = Result<Order, ClassifiedError>> + Send;

    /// Cancels the order `id`.
    ///
    /// Cancelling an order that has already ended, or that the venue does not
    /// know, is an error classed
    /// [`Class::VenueRefusal`](crate::Class::VenueRefusal). The order's final
    /// state arrives through [`events`](Self::events) and
    /// [`open_orders`](Self::open_orders).
    fn cancel(&self, id: &OrderId) -> impl Future<Output = Result<(), ClassifiedError>> + Send;

    /// Cancels every open order, or every open order on `key`, and returns the
    /// ids the venue cancelled. An order the venue did not cancel is left out
    /// of them and stays open, so [`open_orders`](Self::open_orders) shows it.
    fn cancel_all(
        &self,
        key: Option<&MarketKey>,
    ) -> impl Future<Output = Result<Vec<OrderId>, ClassifiedError>> + Send;

    /// Every open order, or every open order on `key`.
    fn open_orders(
        &self,
        key: Option<&MarketKey>,
    ) -> impl Future<Output = Result<Vec<Order>, ClassifiedError>> + Send;

    /// Every position the account holds.
    fn positions(&self) -> impl Future<Output = Result<Vec<Position>, ClassifiedError>> + Send;

    /// Every balance the account holds.
    fn balances(&self) -> impl Future<Output = Result<Vec<Balance>, ClassifiedError>> + Send;

    /// The account's acknowledgements and fills from now on, beginning with a
    /// [`TradingEvent::Resync`] and carrying one after every reconnect.
    ///
    /// Each call returns a stream of its own, which sees every event from that
    /// call on. An order may be reported (acked or filled) before the
    /// [`place`](Self::place) that created it returns. See [`TradingEvents`]
    /// for when the stream ends.
    fn events(&self) -> TradingEvents;
}

/// The stream [`Trading::events`] returns: the account's acknowledgements and
/// fills, from an unbounded queue.
///
/// It is `Send`, `Unpin` and `'static`, so a consumer can move it onto a task
/// of its own, whether it reached the venue statically or through
/// `Arc<DynTrading<'static>>`. (A trait method returning `impl Stream + 'static`
/// would lose the `'static` through the dyn wrapper, so the stream is this
/// named type.)
///
/// Its first item is [`TradingEvent::Resync`]. The venue replays nothing: after
/// every reconnect of the connection that carries fills, and when the venue
/// voids a fill it reported (a trade that failed after it matched, which the
/// stream has no event to take back), the stream yields `Resync` before any
/// later fill. What happened while the connection was down will not arrive, and
/// the consumer reconciles through [`Trading::open_orders`] and the venue's
/// trade history.
///
/// An `Err` item the venue recovered from, such as a lost connection it
/// re-established, is followed by more items. After one it could not recover
/// from, the stream ends. `None` means the feed has stopped for good.
pub struct TradingEvents(
    Pin<Box<dyn Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static>>,
);

impl TradingEvents {
    /// Wraps the stream a [`Trading`] implementation feeds from its queue.
    pub fn new(
        stream: impl Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static,
    ) -> Self {
        Self(Box::pin(stream))
    }
}

impl Stream for TradingEvents {
    type Item = Result<TradingEvent, ClassifiedError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// How long an order rests.
///
/// An order that expires at a set time is a venue option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TimeInForce {
    /// Rests until it fills or is cancelled.
    #[default]
    GoodTilCancelled,
    /// Matches what it can at once; the remainder is cancelled.
    ImmediateOrCancel,
    /// Fills in full at once, or not at all. A venue that cancels one partway,
    /// such as for self-trade prevention, ends it
    /// [`Cancelled`](OrderStatus::Cancelled), possibly partly filled, not
    /// [`Killed`](OrderStatus::Killed).
    FillOrKill,
}

/// An order to place.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OrderRequest {
    /// The instrument, or the outcome of an event, to trade.
    pub key: MarketKey,
    /// Buy or sell.
    pub side: Side,
    /// The limit price.
    pub price: Decimal,
    /// The size, in the instrument's native quantity.
    pub size: Decimal,
    /// How long it rests.
    pub time_in_force: TimeInForce,
    /// The caller's id for it. The venue echoes it, or the implementation
    /// keeps it for the orders it placed, as its documentation says; a venue
    /// that can do neither refuses a request that carries one.
    pub client_order_id: Option<ClientOrderId>,
    /// Rest only: refuse to take liquidity.
    pub post_only: bool,
    /// Only reduce a position, never open or grow one.
    pub reduce_only: bool,
    /// The venue's own options; one it does not declare is refused.
    pub extensions: Extensions,
}

impl OrderRequest {
    /// A good-til-cancelled order to `side` `size` of `key` at `price`, with
    /// no client id, neither flag and no extensions.
    pub fn new(key: MarketKey, side: Side, price: Decimal, size: Decimal) -> Self {
        Self {
            key,
            side,
            price,
            size,
            time_in_force: TimeInForce::default(),
            client_order_id: None,
            post_only: false,
            reduce_only: false,
            extensions: Extensions::new(),
        }
    }
}

/// Why an order was killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KillReason {
    /// An immediate-or-cancel order's remainder, cancelled because nothing
    /// more matched at once. `filled` says how much did.
    Unmatched,
    /// A fill-or-kill order that could not fill in full.
    Unfillable,
}

/// Where an order stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OrderStatus {
    /// Resting, possibly partly filled.
    Open,
    /// Filled in full.
    Filled,
    /// Cancelled, by the caller or the venue, possibly partly filled.
    Cancelled,
    /// Killed by its time in force: a defined outcome, not an error.
    Killed {
        /// Why.
        reason: KillReason,
    },
}

/// An order as the venue reports it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Order {
    /// The venue's id for it.
    pub id: OrderId,
    /// The instrument or outcome it trades.
    pub key: MarketKey,
    /// Buy or sell.
    pub side: Side,
    /// The limit price.
    pub price: Decimal,
    /// The size ordered, in the instrument's native quantity.
    pub size: Decimal,
    /// How much has filled.
    pub filled: Decimal,
    /// Where it stands.
    pub status: OrderStatus,
    /// How long it rests.
    pub time_in_force: Option<TimeInForce>,
    /// The caller's id for it.
    pub client_order_id: Option<ClientOrderId>,
    /// When the venue accepted it.
    pub created: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Order {
    /// The order `id` to `side` `size` of `key` at `price`, with nothing
    /// filled yet.
    pub fn new(
        id: OrderId,
        key: MarketKey,
        side: Side,
        price: Decimal,
        size: Decimal,
        status: OrderStatus,
    ) -> Self {
        Self {
            id,
            key,
            side,
            price,
            size,
            filled: Decimal::ZERO,
            status,
            time_in_force: None,
            client_order_id: None,
            created: None,
            extensions: Extensions::new(),
        }
    }
}

/// Whether a fill took or made liquidity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Liquidity {
    /// The order rested and was matched.
    Maker,
    /// The order matched a resting one.
    Taker,
}

/// One order's part in a trade.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Fill {
    /// The venue's id for the fill.
    pub id: FillId,
    /// The order filled.
    pub order_id: OrderId,
    /// The trade it was part of.
    pub trade_id: Option<TradeId>,
    /// The instrument or outcome traded.
    pub key: MarketKey,
    /// The order's side.
    pub side: Side,
    /// The price.
    pub price: Decimal,
    /// The size, in the instrument's native quantity.
    pub size: Decimal,
    /// The quote-currency amount, where the venue reports one.
    pub notional: Option<Decimal>,
    /// The fee charged, in the quote currency unless the venue's own facts say
    /// otherwise.
    pub fee: Option<Decimal>,
    /// Whether the order made or took liquidity.
    pub liquidity: Option<Liquidity>,
    /// The caller's id for the order.
    pub client_order_id: Option<ClientOrderId>,
    /// When it filled.
    pub time: UnixMillis,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Fill {
    /// The fill `id` of order `order_id`: `size` of `key` at `price`, at
    /// `time`.
    pub fn new(
        id: FillId,
        order_id: OrderId,
        key: MarketKey,
        side: Side,
        price: Decimal,
        size: Decimal,
        time: UnixMillis,
    ) -> Self {
        Self {
            id,
            order_id,
            trade_id: None,
            key,
            side,
            price,
            size,
            notional: None,
            fee: None,
            liquidity: None,
            client_order_id: None,
            time,
            extensions: Extensions::new(),
        }
    }
}

/// What the account holds of one instrument or outcome.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Position {
    /// The instrument or outcome.
    pub key: MarketKey,
    /// The size held, in the instrument's native quantity: positive long,
    /// negative short. A position in an event's outcome is never negative,
    /// since each outcome has its own key.
    pub size: Decimal,
    /// The average price paid.
    pub average_price: Option<Decimal>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Position {
    /// `size` of `key`.
    pub fn new(key: MarketKey, size: Decimal) -> Self {
        Self {
            key,
            size,
            average_price: None,
            extensions: Extensions::new(),
        }
    }
}

/// What the account holds of one asset.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Balance {
    /// The asset, as the venue names it.
    pub asset: String,
    /// The whole balance.
    pub total: Option<Decimal>,
    /// What is free to trade.
    pub available: Option<Decimal>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Balance {
    /// The balance of `asset`, with no amounts yet.
    pub fn new(asset: impl Into<String>) -> Self {
        Self {
            asset: asset.into(),
            total: None,
            available: None,
            extensions: Extensions::new(),
        }
    }
}

/// What [`Trading::events`] reports.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TradingEvent {
    /// The venue accepted, changed or ended an order.
    Ack(Order),
    /// An order filled, in part or in full.
    Fill(Fill),
    /// Reconcile through [`Trading::open_orders`] and the venue's trade
    /// history: the stream cannot vouch for what it reported or missed before
    /// this point.
    ///
    /// It is the stream's first item, comes again after every reconnect of
    /// the connection that carries fills, and comes when the venue voids a fill
    /// it already reported.
    Resync,
}
```

Wire it into `polyoxide-venue/src/lib.rs`. The module doc lists the vocabulary. Current:

```rust
//!   `Extensions`, a venue's own facts on a shared record, one value per type.
//!
//! By default the crate depends on nothing, so a credential-free socket crate
```

Replace with:

```rust
//!   `Extensions`, a venue's own facts on a shared record, one value per type.
//! - With the `trading` feature, `Trading` places and cancels orders, reads
//!   open orders, positions and balances, and streams a venue's
//!   acknowledgements and fills as `TradingEvents`, which begins with a
//!   `Resync` and carries another after every reconnect. It works in
//!   `OrderRequest`, `Order`, `Fill`, `Position` and `Balance`, with the opaque
//!   `OrderId`, `FillId` and `ClientOrderId`, each record carrying
//!   `Extensions`; `Extensions::ensure_declared` is how a venue refuses an
//!   option it does not declare.
//!
//! By default the crate depends on nothing, so a credential-free socket crate
```

Declare the module. Current:

```rust
mod time;
mod wire;
```

Replace with:

```rust
mod time;
#[cfg(feature = "trading")]
mod trading;
mod wire;
```

Export it. Current:

```rust
pub use time::UnixMillis;
pub use wire::UnknownVariant;
```

Replace with:

```rust
pub use time::UnixMillis;
#[cfg(feature = "trading")]
pub use trading::{
    Balance, DynTrading, Fill, KillReason, Liquidity, Order, OrderRequest, OrderStatus, Position,
    TimeInForce, Trading, TradingEvent, TradingEvents,
};
pub use wire::UnknownVariant;
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --all-features`

Expected: every test passes, including this file's thirteen (`a_fill_or_kill_that_cannot_fill_is_ok_and_killed_unfillable` through `the_ids_are_their_text`, with the tokio one waking the spawned consumer) and the doctests. Then run:

```bash
nix develop --command cargo hack check -p polyoxide-venue --each-feature --no-dev-deps
nix develop --command cargo clippy -p polyoxide-venue --all-targets --all-features -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-venue --all-features'
nix develop --command cargo fmt --all -- --check
nix develop --command python3 scripts/venue_gate.py
```

All pass; the venue gate finds no registered venue or product word in the new files.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-venue/src/trading.rs polyoxide-venue/tests/trading.rs
git commit -m "feat(venue): Story 7.3, the Trading trait, TradingEvents and the trading records" -- polyoxide-venue/src/trading.rs polyoxide-venue/src/lib.rs polyoxide-venue/tests/trading.rs
```

### Task 4.4: The docs and the regenerated crate lists

**Files:**
- Modify: `polyoxide-venue/Cargo.toml:54-55`
- Modify: `polyoxide-venue/README.md:9-17`
- Modify: `CLAUDE.md:366`
- Modify: `README.md` (generated region `readme-crates`, written by `gen_registry.py`)
- Modify: `CLAUDE.md:82` (generated crate list, written by `gen_registry.py`)
- Test: `scripts/gen_registry.py --check`

- [ ] **Step 1: Write the failing test**

The crate's one-line description in the registry names the trading vocabulary; `gen_registry.py --check` is the test, and it fails until the generated lists follow. In `polyoxide-venue/Cargo.toml`, current:

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product), and the market-data traits with the records they return and their typed `Extensions` (feature `market`)"
```

Replace with:

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product), the market-data traits with the records they return and their typed `Extensions` (feature `market`), and the `Trading` trait with its order, fill, position and balance records and its `TradingEvents` stream (feature `trading`)"
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command python3 scripts/gen_registry.py --check`

Expected: it fails, reporting that the `readme-crates` region of `README.md` and the crate list of `CLAUDE.md` differ from what the generator writes.

- [ ] **Step 3: Implement**

`polyoxide-venue/README.md` is a doctest, so the addition is prose with no code fence. Current:

```markdown
`Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper) return the
shared records, each carrying a venue's own facts in its `Extensions`.
```

Replace with:

```markdown
`Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper) return the
shared records, each carrying a venue's own facts in its `Extensions`.
Behind the `trading` feature, `Trading` places and cancels orders, reads open
orders, positions and balances, and streams acknowledgements and fills as
`TradingEvents`, which begins with a `Resync`; `Extensions::ensure_declared` is
how a venue refuses an option it does not declare.
```

In `CLAUDE.md`, insert the new paragraph directly before the "**The CLOB through the traits**" paragraph (it follows the "**Market-data traits**" one). Current, the paragraph's first line:

```markdown
**The CLOB through the traits** — `clob::venue` (Story 6.5) implements `MarketData` and `Candles`
```

Replace with:

```markdown
**The trading trait** — `polyoxide-venue`'s `trading` feature (Story 7.3, which turns on `market`)
holds `Trading`, the one interface a venue with signed order routes implements so a consumer
trades the same way on each (AD-3, AD-5, amendment A7-1), and its `DynTrading` wrapper, held as
`Arc<DynTrading<'static>>`. Its methods return `Result<_, ClassifiedError>`: `place(OrderRequest)`
gives the `Order` as the venue accepted it, `cancel(&OrderId)`, `cancel_all(Option<&MarketKey>)`
the ids cancelled, `open_orders(Option<&MarketKey>)`, `positions()` and `balances()`, and
`events()` is not async and returns a `TradingEvents`. The records are `#[non_exhaustive]` with a
`new` that takes their required fields and a `pub extensions: Extensions`: `OrderRequest` (a
`TimeInForce` defaulting to `GoodTilCancelled`, an optional `ClientOrderId`, `post_only` and
`reduce_only`), `Order` (an `OrderStatus` of `Open`, `Filled`, `Cancelled` or
`Killed { reason: KillReason }`), `Fill` (its `trade_id` is `market`'s `TradeId`, its `liquidity`
a `Liquidity`), `Position` and `Balance`; `OrderId`, `FillId` and `ClientOrderId` are opaque
`Arc<str>` ids. A request is refused, never ignored, as `InvalidRequest` with nothing sent when it
carries an extension its venue does not declare, a key of another product, or a `post_only`,
`reduce_only` or time in force the venue cannot honour: `Extensions::ensure_declared(&[TypeId])`
(feature `market`) refuses the first undeclared value in `TypeId` order with an
`UndeclaredExtension` (`InvalidRequest`, in the `Classify` assertion). An immediate-or-cancel order
that matched nothing and a fill-or-kill order that could not fill are `Ok` with `Killed`, not
errors. `TradingEvents` is a newtype over a pinned boxed stream, `Send + Unpin + 'static`, because
dynosaur 0.3.1 drops `'static` from a returned `impl Stream` and the stream would borrow the
`Arc` (E0597, A7-1). The queue behind it is unbounded, so a slow consumer loses no ack or fill, and
the venue replays nothing: the stream's first item is `TradingEvent::Resync` (A7-2, once the fills
connection's first attempt has ended, whether it went live or began in an outage), and it yields
`Resync` after every reconnect of that connection and when the venue voids a fill it reported (a
trade that failed after it matched), which means "reconcile through `open_orders()` and the
trade history: the stream cannot vouch for what it reported or missed". An order may be reported
before the `place` that created it returns. `polyoxide-venue/tests/trading.rs` proves the trait on
a made-up venue, statically and through `Arc<DynTrading<'static>>`, including a stream that
outlives every `Arc` on another thread.

**The CLOB through the traits** — `clob::venue` (Story 6.5) implements `MarketData` and `Candles`
```

Regenerate the lists the metadata feeds:

```bash
nix develop --command python3 scripts/gen_registry.py --write
```

It rewrites the `readme-crates` region of `README.md` and the `polyoxide-venue` bullet of `CLAUDE.md`'s crate list, and nothing else; `git diff --stat` shows only those two files beside this task's edits. The line is regenerated, never typed by hand.

- [ ] **Step 4: Run it to see it pass**

Run:

```bash
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command cargo test -p polyoxide-venue --all-features --doc
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-venue --all-features'
nix develop --command python3 scripts/venue_gate.py
```

Expected: all four pass; the doctest run includes the README's.

Then run the whole story's verify command: `nix develop --command cargo test -p polyoxide-venue --all-features && nix develop --command cargo hack check -p polyoxide-venue --each-feature --no-dev-deps`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(venue): Story 7.3, the trading trait in CLAUDE.md, the venue README and the registry lists" -- polyoxide-venue/Cargo.toml polyoxide-venue/README.md CLAUDE.md README.md
```

## Story 5: `Trading` on the Polymarket CLOB (Story 7.4)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/src/clob/venue/trading.rs`, `polyoxide-polymarket/src/clob/venue/fills.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/trading_clob.rs`, `polyoxide-polymarket/src/clob/README.md`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `CLAUDE.md`, `Cargo.lock`
- **Interfaces:** exposes `impl Trading for clob::Clob`, `clob::venue::{ClobOrderOptions, OrderExt, FillExt, PositionExt, BalanceExt}` and `ClobBuilder::user_socket` (names contract, Story 5); uses Story 3's `UserWsBuilder::start`, `UserEvent` and `ApiCredentials`, and Story 4's trading items and `Extensions::ensure_declared`
- **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob && cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue && cargo check -p polyoxide --no-default-features --features polymarket-clob-ws,polymarket-data && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py -q'`
- **Review:** deep (places and cancels orders)

Implements `Trading` for `Clob`, gated `all(clob-ws, data)` (spec §7, §8). `place` runs its client-side checks with nothing sent, then signs and posts. It maps FAK and FOK kills, matched before any conversion, to `Ok` with `Killed { reason }`, and reads back a matched order's fill. `cancel`, `cancel_all`, `open_orders`, `positions` (Data v1, no dust filter) and `balances` (pUSD, 6 decimals) follow the spec. `events()` runs on Story 3's supervised user socket: a leading `Resync`, `Resync` after every reconnect and on a `FAILED` trade, and one `Fill` per account order in a `MATCHED` trade. The in-process client-id ledger is keyed by the order's EIP-712 hash, computed before sending. Leaves `clob/error.rs` alone, since `docs/MUTANTS.md` cites its lines.

The story builds `venue/trading.rs` in three tasks and `venue/fills.rs` in the fourth. Task 5.2 leaves five request methods and `events()` as named stubs that Tasks 5.3 and 5.4 replace. Every task keeps the gates green, and none of the code holds a landmine marker. One point where the real code parts from the spec's text is marked in the task that meets it: `ClobOrderOptions` cannot derive `Eq` (`TickSize` does not). `place` supports a Deposit Wallet account: Task 5.1 adds `eip712::order_hash`, the order's id under its exchange's domain for every signature type (`compute_order_digest` keeps its type-3 refusal and stays private), and `place` signs through `Clob::sign_order`, which gives a Deposit Wallet's order its ERC-7739 envelope.

Each task's `use` additions are single-line statements placed after an anchor line, so a `cargo fmt --all` that reorders the group never invalidates the next task's anchor. Run `nix develop --command cargo fmt --all` before each commit.

### Task 5.1: The options, the `Ext` types, `order_hash` and the wiring

**Files:**
- Create: `polyoxide-polymarket/src/clob/venue/trading.rs`
- Modify: `polyoxide-polymarket/src/clob/venue.rs:19,504-505`
- Modify: `polyoxide-polymarket/src/clob/core/eip712.rs:201-214,1390`
- Modify: `polyoxide-polymarket/src/clob/client.rs:546`
- Modify: `polyoxide-polymarket/Cargo.toml:42-49`
- Test: `polyoxide-polymarket/src/clob/core/eip712.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-polymarket/src/clob/venue/trading.rs` with its module doc and the first tests:

```rust
//! `Trading` for the CLOB: orders placed and cancelled through the native
//! routes, the account's orders, positions and balance read back, and the
//! in-process ledger of client order ids. The events stream is in `fills`.
//!
//! This module also holds the options a request carries to the CLOB
//! ([`ClobOrderOptions`]) and the CLOB's facts that ride beside each trading
//! record, one `<Record>Ext` type per record (NFR8).

#[cfg(test)]
mod option_tests {
    use polyoxide_venue::Extensions;

    use super::*;

    #[test]
    fn the_options_default_to_nothing_set() {
        assert_eq!(
            ClobOrderOptions::default(),
            ClobOrderOptions {
                neg_risk: None,
                tick_size: None,
                expiration: None,
            }
        );
    }

    #[test]
    fn the_options_ride_in_a_requests_extensions() {
        let options = ClobOrderOptions {
            neg_risk: Some(true),
            tick_size: Some(TickSize::Thousandth),
            expiration: Some(UnixMillis(1_792_000_000_000)),
        };
        let extensions = Extensions::of(options);
        assert_eq!(extensions.get::<ClobOrderOptions>(), Some(&options));
    }

    #[test]
    fn the_ext_types_hold_nothing_until_filled() {
        assert_eq!(OrderExt::default().trade_ids, Vec::<String>::new());
        assert_eq!(FillExt::default().fee_rate_bps, None);
        assert_eq!(PositionExt::default().redeemable, None);
        assert!(BalanceExt::default().allowances.is_empty());
    }
}
```

Declare the module and its re-exports. Current (`polyoxide-polymarket/src/clob/venue.rs:504-505`):

```rust
#[cfg(feature = "gamma")]
pub use events::EventGroupExt;
```

Replace with:

```rust
#[cfg(feature = "gamma")]
pub use events::EventGroupExt;

#[cfg(all(feature = "clob-ws", feature = "data"))]
mod trading;

#[cfg(all(feature = "clob-ws", feature = "data"))]
pub use trading::{BalanceExt, ClobOrderOptions, FillExt, OrderExt, PositionExt};
```

Append the `order_hash` tests to `eip712.rs`, as a new module after the file's last one. In `polyoxide-polymarket/src/clob/core/eip712.rs`, Current (the end of the file):

```rust
        assert_eq!(
            signature,
            super::clob_auth_reference_tests::REFERENCE_SIGNATURE
        );
    }
}
```

Replace with:

```rust
        assert_eq!(
            signature,
            super::clob_auth_reference_tests::REFERENCE_SIGNATURE
        );
    }
}

/// `order_hash` is the id `Trading::place` gives an order before it signs it:
/// the order's EIP-712 hash under its exchange's domain, for every signature
/// type. A Deposit Wallet's order has no plain digest to sign
/// (`compute_order_digest` refuses it), but the exchange names it by this
/// hash all the same.
#[cfg(test)]
mod order_hash_vectors {
    use super::*;
    use crate::clob::types::OrderSide;
    use alloy::primitives::address;

    const VECTORS: &str =
        include_str!("../../../tests/fixtures/clob/session_keys/order_vectors.json");
    const WALLET: Address = address!("57ffbc34de23124faeb8387fcd689d314e57accd");
    const MAKER: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    #[derive(serde::Deserialize)]
    struct Vector {
        exchange: Address,
        app_domain_separator: B256,
        contents_hash: B256,
    }

    fn vector(name: &str) -> Vector {
        let mut all: std::collections::HashMap<String, Vector> =
            serde_json::from_str(VECTORS).unwrap();
        all.remove(name)
            .unwrap_or_else(|| panic!("no vector {name}"))
    }

    /// py-sdk's golden order: maker = signer = the wallet, salt 1, token 1,
    /// 1_000_000 / 500_000, BUY, timestamp 0, zero metadata and builder.
    fn golden_order() -> ClobOrder {
        ClobOrder {
            salt: "1".to_string(),
            maker: WALLET,
            signer: WALLET,
            token_id: "1".to_string(),
            maker_amount: "1000000".to_string(),
            taker_amount: "500000".to_string(),
            side: OrderSide::Buy,
            expiration: "0".to_string(),
            signature_type: SignatureType::Poly1271,
            timestamp: "0".to_string(),
            metadata: B256::ZERO,
            builder: B256::ZERO,
            neg_risk: false,
        }
    }

    fn plain_order() -> ClobOrder {
        ClobOrder {
            maker: MAKER,
            signer: MAKER,
            signature_type: SignatureType::Eoa,
            ..golden_order()
        }
    }

    #[test]
    fn a_deposit_wallet_orders_hash_composes_py_sdks_domain_and_contents_hashes() {
        // The chain table's exchange for chain 137 is the one py-sdk's
        // `v2_exchange` vector names, so `order_hash` reaches its separator.
        // (`v1_exchange` names the previous contract, which the table no
        // longer holds.) Both vectors share one contents hash.
        let v = vector("v2_exchange");
        assert_eq!(exchange_address(137, false).unwrap(), v.exchange);
        let mut message = Vec::with_capacity(66);
        message.extend_from_slice(b"\x19\x01");
        message.extend_from_slice(v.app_domain_separator.as_slice());
        message.extend_from_slice(v.contents_hash.as_slice());

        let hash = order_hash(&golden_order(), 137).unwrap();

        assert_eq!(hash, keccak256(&message));
        // The plain digest still refuses the same order.
        assert!(compute_order_digest(&golden_order(), 137).is_err());
    }

    #[test]
    fn a_plain_orders_hash_is_the_digest_a_plain_signature_covers() {
        let order = plain_order();
        let exchange = exchange_address(137, false).unwrap();
        let independent = eip712_digest(
            exchange_domain(137, exchange).eip712_hash_struct(),
            order_to_protocol(&order).unwrap().eip712_hash_struct(),
        );

        assert_eq!(order_hash(&order, 137).unwrap(), independent);
        assert_eq!(
            order_hash(&order, 137).unwrap(),
            compute_order_digest(&order, 137).unwrap()
        );
    }

    #[test]
    fn the_hash_follows_the_orders_exchange_and_refuses_an_unsupported_chain() {
        let mut order = plain_order();
        let plain = order_hash(&order, 137).unwrap();
        order.neg_risk = true;

        assert_ne!(order_hash(&order, 137).unwrap(), plain);
        assert!(order_hash(&order, 1).is_err());
    }
}
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue::trading
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::core::eip712::order_hash_vectors
```

Expected: the build fails with `error[E0432]: unresolved imports` for `trading::BalanceExt`, `trading::ClobOrderOptions` and the other three names, `cannot find type ClobOrderOptions in this scope` in the tests, and `cannot find function order_hash in this scope` in `order_hash_vectors`.

- [ ] **Step 3: Implement**

Add the `trading` feature to `clob-ws`. Current (`polyoxide-polymarket/Cargo.toml:42-49`):

```toml
clob-ws = [
    "clob",
    "dep:polyoxide-ws",
    "dep:futures-util",
    "tokio/net",
    "tokio/macros",
    "tokio/sync",
]
```

Replace with:

```toml
clob-ws = [
    "clob",
    "dep:polyoxide-ws",
    "dep:futures-util",
    "polyoxide-venue/trading",
    "tokio/net",
    "tokio/macros",
    "tokio/sync",
]
```

Give the order an id for every signature type. `order_hash` is the order's EIP-712 hash under its exchange's domain; `compute_order_digest` keeps its type-3 refusal, since a Deposit Wallet signs an ERC-7739 envelope and not this digest, then calls `order_hash`, and stays private. `docs/MUTANTS.md` cites no line of `eip712.rs`, so the new function goes directly after `compute_order_digest`. Current (`polyoxide-polymarket/src/clob/core/eip712.rs:201-214`):

```rust
/// Compute the EIP-712 digest for a plain (type 0–2) order, without signing.
fn compute_order_digest(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError> {
    if order.signature_type == SignatureType::Poly1271 {
        return Err(ClobError::Crypto(
            "signature type 3 (Poly1271) is signed as an ERC-7739 envelope for a Deposit \
             Wallet; use sign_order_as with SigningTarget::DepositWallet"
                .into(),
        ));
    }
    let exchange = exchange_address(chain_id, order.neg_risk)?;
    let domain = exchange_domain(chain_id, exchange);
    let struct_hash = order_to_protocol(order)?.eip712_hash_struct();
    Ok(eip712_digest(domain.eip712_hash_struct(), struct_hash))
}
```

Replace with:

```rust
/// Compute the EIP-712 digest for a plain (type 0–2) order, without signing.
fn compute_order_digest(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError> {
    if order.signature_type == SignatureType::Poly1271 {
        return Err(ClobError::Crypto(
            "signature type 3 (Poly1271) is signed as an ERC-7739 envelope for a Deposit \
             Wallet; use sign_order_as with SigningTarget::DepositWallet"
                .into(),
        ));
    }
    order_hash(order, chain_id)
}

/// The EIP-712 hash of an order under its exchange's domain, for every
/// signature type: the id the exchange gives the order, known before it is
/// signed.
///
/// For a plain order (type 0–2) this is the digest [`sign_order`] signs. A
/// Deposit Wallet's order (type 3) signs an ERC-7739 envelope instead, but the
/// exchange names it by this same hash. A type-3 order whose maker is not its
/// signer is refused, as the order conversion refuses it.
pub(crate) fn order_hash(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError> {
    let exchange = exchange_address(chain_id, order.neg_risk)?;
    let domain = exchange_domain(chain_id, exchange);
    let struct_hash = order_to_protocol(order)?.eip712_hash_struct();
    Ok(eip712_digest(domain.eip712_hash_struct(), struct_hash))
}
```

Raise one function to `pub(crate)`, changing no line number. Current (`polyoxide-polymarket/src/clob/client.rs:546`):

```rust
    async fn resolve_maker_address(
```

Replace with:

```rust
    pub(crate) async fn resolve_maker_address(
```

Say what the module now holds. In `polyoxide-polymarket/src/clob/venue.rs`, Current (the last row of the module doc's table, line 19):

```rust
//! | `event_group` (`gamma`) | Gamma's `/markets?clob_token_ids=`, then `/events/{id}` |
```

Replace with:

```rust
//! | `event_group` (`gamma`) | Gamma's `/markets?clob_token_ids=`, then `/events/{id}` |
//!
//! With `clob-ws` and `data` as well, [`Clob`] implements the `Trading` trait
//! of `polyoxide-venue`: orders placed and cancelled, open orders, positions
//! and the pUSD balance, and an events stream on the supervised user socket.
//! The options a request carries and the CLOB's facts beside each trading
//! record are `ClobOrderOptions`, `OrderExt`, `FillExt`, `PositionExt` and
//! `BalanceExt`.
```

Add the module's items. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
#[cfg(test)]
mod option_tests {
```

Replace with:

```rust
use std::collections::BTreeMap;

use polyoxide_venue::UnixMillis;
use rust_decimal::Decimal;

use crate::clob::types::TickSize;

/// Options only the CLOB reads, carried in an
/// [`OrderRequest`](polyoxide_venue::OrderRequest)'s `extensions`.
///
/// A request carrying any other extension type is refused. Each field is a
/// fact the caller already holds, so the client does not fetch it: it looks a
/// missing `neg_risk` or `tick_size` up. A V2 order signs no fee rate, so
/// there are no fee terms here.
///
/// It is not `Eq`, because [`TickSize`] is not.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct ClobOrderOptions {
    /// Whether the market is in a negative-risk event, which selects the
    /// exchange contract the order is signed for.
    pub neg_risk: Option<bool>,
    /// The market's tick size now in force.
    pub tick_size: Option<TickSize>,
    /// When a good-til-cancelled order expires. Set, it becomes a
    /// good-til-date order; the CLOB reads whole seconds, so the time is
    /// rounded down to one.
    pub expiration: Option<UnixMillis>,
}

/// The CLOB's facts about an order, beside
/// [`Order`](polyoxide_venue::Order)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct OrderExt {
    /// The CLOB's own status word: `live`, `matched` or `delayed` from a
    /// placement, `LIVE`, `MATCHED`, `CANCELED` and the rest from a listing.
    pub status: Option<String>,
    /// The condition id of the market the order is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The maker address: the proxy or deposit wallet, not the signing key.
    pub maker_address: Option<String>,
    /// When the order expires, when it does.
    pub expiration: Option<UnixMillis>,
    /// Whether the order was signed for the negative-risk exchange.
    pub neg_risk: Option<bool>,
    /// The ids of the trades the order took part in.
    pub trade_ids: Vec<String>,
    /// The settling transactions' hashes a placement reported.
    pub transaction_hashes: Vec<String>,
}

/// The CLOB's facts about a fill, beside
/// [`Fill`](polyoxide_venue::Fill)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FillExt {
    /// The condition id of the market the trade is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The taker's order id.
    pub taker_order_id: Option<String>,
    /// The settling transaction's hash, once the CLOB reports one.
    pub transaction_hash: Option<String>,
    /// The maker's fee rate in basis points. A taker's fill has none: the fee
    /// is taken on chain at match time.
    pub fee_rate_bps: Option<Decimal>,
}

/// The CLOB's facts about a position, beside
/// [`Position`](polyoxide_venue::Position)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PositionExt {
    /// The condition id of the market the outcome is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The outcome's index in its market.
    pub outcome_index: Option<u32>,
    /// The outcome's current price.
    pub current_price: Option<Decimal>,
    /// The position's current value.
    pub current_value: Option<Decimal>,
    /// The position's initial value.
    pub initial_value: Option<Decimal>,
    /// The cash profit and loss.
    pub cash_pnl: Option<Decimal>,
    /// The realized profit and loss.
    pub realized_pnl: Option<Decimal>,
    /// Whether the position can be redeemed.
    pub redeemable: Option<bool>,
    /// Whether the position can be merged.
    pub mergeable: Option<bool>,
    /// Whether the market is in a negative-risk event.
    pub negative_risk: Option<bool>,
    /// The market's title.
    pub title: Option<String>,
    /// When the market ends, as the Data API writes it.
    pub end_date: Option<String>,
}

/// The CLOB's facts about a balance, beside
/// [`Balance`](polyoxide_venue::Balance)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BalanceExt {
    /// Each spender's allowance, by the spender's address, divided by 10^6
    /// as the balance is. An unlimited approval is larger than a `Decimal`
    /// holds and reads as `Decimal::MAX`.
    pub allowances: BTreeMap<String, Decimal>,
}

#[cfg(test)]
mod option_tests {
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue::trading
```

Expected: `test result: ok. 3 passed; 0 failed`. The new module has no unused item: its types are public through `clob::venue`. Then run the `order_hash` tests:

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::core::eip712
```

Expected: every test of `eip712.rs` passes, among them the three of `order_hash_vectors`, and the existing `compute_order_digest_rejects_poly1271` and `plain_digest_rejection_points_at_sign_order_as` still pass.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws,data --all-targets -- -D warnings
git add -N polyoxide-polymarket/src/clob/venue/trading.rs
git commit -m "feat(clob): ClobOrderOptions and the trading Ext types (Story 7.4)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-polymarket/src/clob/venue.rs polyoxide-polymarket/src/clob/venue/trading.rs polyoxide-polymarket/src/clob/core/eip712.rs polyoxide-polymarket/src/clob/client.rs polyoxide-polymarket/Cargo.toml Cargo.lock
```

### Task 5.2: `place`, the client-id ledger and the checks

`impl Trading for Clob` lands here with `place` complete. The five other request methods and `events()` are **temporary stubs**: Task 5.3 replaces the five, and Task 5.4 replaces `events()`. A stub answers a `VenueRefusal` (or an empty stream), so the gates stay green.

**Files:**
- Modify: `polyoxide-polymarket/src/clob/venue/trading.rs`
- Modify: `polyoxide-polymarket/src/clob/venue.rs:504-512`
- Modify: `polyoxide-polymarket/src/clob/client.rs:64-70,966-970`
- Modify: `polyoxide-polymarket/Cargo.toml:334-337`
- Test: `polyoxide-polymarket/tests/trading_clob.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-polymarket/tests/trading_clob.rs`:

```rust
//! `Trading` on the CLOB against mock servers: the HTTP routes run on
//! mockito and the user socket on the kit's scripted server. The account is
//! Hardhat account 0, an EOA whose maker address is its own.

use std::borrow::Cow;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::clob::venue::{ClobOrderOptions, OrderExt};
use polyoxide_polymarket::clob::{
    Account, Clob, ClobBuilder, Credentials, SignatureType, TickSize,
};
use polyoxide_polymarket::data::DataApi;
use polyoxide_polymarket::shared::keys::{self, TokenId};
use polyoxide_venue::{
    Class, ClientOrderId, KeySpace, KillReason, MarketKey, OrderRequest, OrderStatus, ProductId,
    Side, TimeInForce, Trading, UnixMillis, VenueId,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};

const TOKEN: &str = "100";
const MARKET: &str = "0xcondition";

/// Venue prose for a FAK order that found nothing to match against, verbatim.
const FAK_UNMATCHED_MSG: &str = "no orders found to match with FAK order. \
FAK orders are partially filled or killed if no match is found.";

/// Venue prose for a FOK order that could not be filled in full, verbatim.
const FOK_UNFILLED_MSG: &str =
    "order couldn't be fully filled. FOK orders are fully filled or killed.";

const LIVE: &str =
    r#"{"success":true,"orderID":"0xorder1","status":"live","transactionsHashes":[]}"#;
const MATCHED: &str =
    r#"{"success":true,"orderID":"0xorder1","status":"matched","transactionsHashes":["0xtx1"]}"#;

fn d(text: &str) -> Decimal {
    text.parse().expect("a decimal literal")
}

/// Hardhat account 0 with test credentials.
fn account() -> Account {
    let creds = Credentials {
        key: "test-key".into(),
        secret: "c2VjcmV0".into(), // base64("secret")
        passphrase: "test-pass".into(),
    };
    Account::new(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        creds,
    )
    .unwrap()
}

/// A CLOB client and a Data client, both on the one mock server.
fn client(server: &ServerGuard) -> Clob {
    ClobBuilder::new()
        .base_url(server.url())
        .with_account(account())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .build()
        .unwrap()
}

/// A client that trades as a proxy wallet: built with `signature_type(PolyProxy)`
/// over an EOA account, as a live client for a proxy account is. With the
/// `gamma` feature it looks the proxy up on a Gamma client on the same server.
fn proxy_client(server: &ServerGuard) -> Clob {
    let builder = ClobBuilder::new()
        .base_url(server.url())
        .with_account(account())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .signature_type(SignatureType::PolyProxy);
    #[cfg(feature = "gamma")]
    let builder = builder.gamma(
        polyoxide_polymarket::gamma::Gamma::builder()
            .base_url(server.url())
            .build()
            .unwrap(),
    );
    builder.build().unwrap()
}

/// The proxy wallet the Gamma mock reports for the account. Digits only, so
/// its checksum and lowercase spellings are the same text.
#[cfg(feature = "gamma")]
const PROXY: &str = "0x1111111111111111111111111111111111111111";

/// `GET /public-profile` answering with a profile whose proxy wallet is
/// `PROXY`, once.
#[cfg(feature = "gamma")]
async fn proxy_profile(server: &mut ServerGuard) -> Mock {
    server
        .mock("GET", "/public-profile")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "proxyWallet": PROXY,
                "takerTier": 0,
                "takerTierName": "Tier 0",
                "weightedVolume": 0.0,
            })
            .to_string(),
        )
        .expect(1)
        .create_async()
        .await
}

fn key() -> MarketKey {
    keys::clob(&TokenId::new(TOKEN).unwrap())
}

fn buy(price: &str, size: &str) -> OrderRequest {
    OrderRequest::new(key(), Side::Buy, d(price), d(size))
}

fn with_options(mut request: OrderRequest, options: ClobOrderOptions) -> OrderRequest {
    request.extensions.insert(options).unwrap();
    request
}

/// Options that name the tick size and the exchange, so no lookup is sent.
#[allow(clippy::field_reassign_with_default)]
fn declared(tick: TickSize) -> ClobOrderOptions {
    let mut options = ClobOrderOptions::default();
    options.tick_size = Some(tick);
    options.neg_risk = Some(false);
    options
}

/// `/neg-risk` and `/tick-size`, which the client reads unless the options
/// name them. Each expects one request.
async fn lookups(server: &mut ServerGuard) -> (Mock, Mock) {
    let neg_risk = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), TOKEN.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .expect(1)
        .create_async()
        .await;
    let tick = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), TOKEN.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .expect(1)
        .create_async()
        .await;
    (neg_risk, tick)
}

/// `POST /order` answering 200 with `body`, `hits` times.
async fn post_ok(server: &mut ServerGuard, body: &str, hits: usize) -> Mock {
    server
        .mock("POST", "/order")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

/// `POST /order` refusing with 400 and the venue's prose, `hits` times.
async fn post_refused(server: &mut ServerGuard, message: &str, hits: usize) -> Mock {
    server
        .mock("POST", "/order")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(json!({ "error": message }).to_string())
        .expect(hits)
        .create_async()
        .await
}

/// A `GET /data/order/{id}` body: the shape `OpenOrder` decodes.
fn open_order(id: &str, status: &str, matched: &str) -> Value {
    json!({
        "id": id,
        "status": status,
        "owner": "test-key",
        "maker_address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "market": MARKET,
        "asset_id": TOKEN,
        "side": "BUY",
        "original_size": "10",
        "size_matched": matched,
        "price": "0.52",
        "outcome": "Yes",
        "expiration": "0",
        "order_type": "GTC",
        "associate_trades": ["trade-1"],
        "created_at": 1_792_000_000_i64,
    })
}

/// `GET /data/order/0xorder1` answering with `matched` shares matched.
async fn read_back(server: &mut ServerGuard, matched: &str) -> Mock {
    server
        .mock("GET", "/data/order/0xorder1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(open_order("0xorder1", "MATCHED", matched).to_string())
        .expect(1)
        .create_async()
        .await
}

/// Mocks that fail the test if any request reaches the server.
async fn nothing_sent(server: &mut ServerGuard) -> Vec<Mock> {
    let mut mocks = Vec::new();
    for method in ["GET", "POST", "DELETE"] {
        mocks.push(
            server
                .mock(method, Matcher::Any)
                .match_query(Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        );
    }
    mocks
}

/// A product of another venue, to hand the CLOB a key it does not own.
struct Elsewhere;

impl KeySpace for Elsewhere {
    const VENUE: VenueId = VenueId::from_static("elsewhere");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

/// An option type the CLOB does not declare.
#[derive(Debug, Clone, PartialEq)]
struct Stray;

// --- place ---

#[tokio::test]
async fn a_resting_post_only_order_is_open_and_carries_the_request() {
    let mut server = Server::new_async().await;
    let (neg_risk, tick) = lookups(&mut server).await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(
            json!({"owner": "test-key", "orderType": "GTC", "postOnly": true}),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(LIVE)
        .expect(1)
        .create_async()
        .await;
    let mut request = buy("0.52", "10");
    request.post_only = true;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(order.id.as_str(), "0xorder1");
    assert_eq!(order.key, key());
    assert_eq!(order.side, Side::Buy);
    assert_eq!(order.price, d("0.52"));
    assert_eq!(order.size, d("10"));
    assert_eq!(order.filled, Decimal::ZERO);
    assert_eq!(order.status, OrderStatus::Open);
    assert_eq!(order.time_in_force, Some(TimeInForce::GoodTilCancelled));
    let ext = order.extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.status.as_deref(), Some("live"));
    assert_eq!(ext.neg_risk, Some(false));
    neg_risk.assert_async().await;
    tick.assert_async().await;
    post.assert_async().await;
}

#[tokio::test]
async fn a_good_til_date_order_is_posted_with_its_expiration_in_whole_seconds() {
    let mut server = Server::new_async().await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(
            json!({"orderType": "GTD", "order": {"expiration": "1900000000"}}),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(LIVE)
        .expect(1)
        .create_async()
        .await;
    let mut options = declared(TickSize::Hundredth);
    options.expiration = Some(UnixMillis(1_900_000_000_999));

    let order = client(&server)
        .place(with_options(buy("0.52", "10"), options))
        .await
        .unwrap();

    assert_eq!(order.status, OrderStatus::Open);
    let ext = order.extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.expiration, Some(UnixMillis(1_900_000_000_999)));
    post.assert_async().await;
}

#[tokio::test]
async fn a_fak_kill_is_ok_and_killed_unmatched() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_refused(&mut server, FAK_UNMATCHED_MSG, 1).await;
    let mut request = buy("0.01", "100");
    request.time_in_force = TimeInForce::ImmediateOrCancel;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unmatched
        }
    );
    assert_eq!(order.filled, Decimal::ZERO);
    // The id is the order's EIP-712 hash, computed before sending.
    assert!(order.id.as_str().starts_with("0x"));
    assert_eq!(order.id.as_str().len(), 66);
    assert_eq!(order.time_in_force, Some(TimeInForce::ImmediateOrCancel));
    post.assert_async().await;
}

#[tokio::test]
async fn a_fok_kill_is_ok_and_killed_unfillable() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_refused(&mut server, FOK_UNFILLED_MSG, 1).await;
    let mut request = buy("0.01", "100");
    request.time_in_force = TimeInForce::FillOrKill;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unfillable
        }
    );
    assert_eq!(order.filled, Decimal::ZERO);
    post.assert_async().await;
}

#[tokio::test]
async fn a_matched_order_is_read_back_as_filled() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_ok(&mut server, MATCHED, 1).await;
    let read = read_back(&mut server, "10").await;

    let order = client(&server).place(buy("0.52", "10")).await.unwrap();

    assert_eq!(order.status, OrderStatus::Filled);
    assert_eq!(order.filled, d("10"));
    let ext = order.extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.status.as_deref(), Some("matched"));
    assert_eq!(ext.transaction_hashes, ["0xtx1"]);
    post.assert_async().await;
    read.assert_async().await;
}

#[tokio::test]
async fn an_ioc_partial_match_is_killed_unmatched_with_what_matched() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(json!({"orderType": "FAK"})))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(MATCHED)
        .expect(1)
        .create_async()
        .await;
    let _read = read_back(&mut server, "4").await;
    let mut request = buy("0.52", "10");
    request.time_in_force = TimeInForce::ImmediateOrCancel;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unmatched
        }
    );
    assert_eq!(order.filled, d("4"));
    post.assert_async().await;
}

#[tokio::test]
async fn a_matched_order_whose_read_back_fails_is_reported_open() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let _post = post_ok(&mut server, MATCHED, 1).await;
    // A 500 is not retried, so the read-back fails at once.
    let read = server
        .mock("GET", "/data/order/0xorder1")
        .with_status(500)
        .with_body("upstream down")
        .expect(1)
        .create_async()
        .await;

    // The order exists on the venue, so an error here would invite a duplicate.
    let order = client(&server).place(buy("0.52", "10")).await.unwrap();

    assert_eq!(order.id.as_str(), "0xorder1");
    assert_eq!(order.status, OrderStatus::Open);
    assert_eq!(order.filled, Decimal::ZERO);
    read.assert_async().await;
}

#[tokio::test]
async fn a_post_the_clob_refuses_is_a_venue_refusal_carrying_its_message() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_ok(
        &mut server,
        r#"{"success":false,"errorMsg":"not enough balance / allowance"}"#,
        1,
    )
    .await;

    let err = client(&server)
        .place(buy("0.52", "10"))
        .await
        .unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(err.to_string().contains("not enough balance"), "{err}");
    post.assert_async().await;
}

#[tokio::test]
async fn the_tick_size_and_neg_risk_are_looked_up_only_when_the_options_lack_them() {
    let mut server = Server::new_async().await;
    let (neg_risk, tick) = lookups(&mut server).await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let clob = client(&server);

    clob.place(buy("0.52", "10")).await.unwrap();
    clob.place(with_options(
        buy("0.52", "10"),
        declared(TickSize::Hundredth),
    ))
    .await
    .unwrap();

    // One lookup each, from the first order alone.
    neg_risk.assert_async().await;
    tick.assert_async().await;
    post.assert_async().await;
}

#[tokio::test]
async fn requests_the_clob_cannot_honour_are_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let untouched = nothing_sent(&mut server).await;
    let clob = client(&server);
    let tick = declared(TickSize::Hundredth);

    let mut reduce_only = buy("0.52", "10");
    reduce_only.reduce_only = true;
    let mut post_only_ioc = buy("0.52", "10");
    post_only_ioc.post_only = true;
    post_only_ioc.time_in_force = TimeInForce::ImmediateOrCancel;
    let mut expiring = declared(TickSize::Hundredth);
    expiring.expiration = Some(UnixMillis(1_900_000_000_000));
    let mut expiring_fok = with_options(buy("0.52", "10"), expiring);
    expiring_fok.time_in_force = TimeInForce::FillOrKill;
    let mut stray = buy("0.52", "10");
    stray.extensions.insert(Stray).unwrap();
    let foreign = OrderRequest::new(
        Elsewhere::key(&"1".to_owned()),
        Side::Buy,
        d("0.52"),
        d("10"),
    );

    let cases = [
        ("reduce-only", reduce_only, "reduce-only"),
        ("post-only with IOC", post_only_ioc, "post-only"),
        ("expiration with FOK", expiring_fok, "expiration"),
        ("an undeclared extension", stray, "does not declare"),
        ("a key of another product", foreign, ""),
        (
            "an off-tick price",
            with_options(buy("0.525", "10"), tick),
            "tick size",
        ),
        (
            "a price of one",
            with_options(buy("1", "10"), tick),
            "strictly between",
        ),
        (
            "a size of three decimals",
            with_options(buy("0.52", "5.001"), tick),
            "decimal places",
        ),
        (
            "a size below the minimum",
            with_options(buy("0.52", "0.00"), tick),
            "smallest order",
        ),
    ];
    for (label, request, expected) in cases {
        let err = clob.place(request).await.unwrap_err();
        assert_eq!(err.class, Class::InvalidRequest, "{label}: {err}");
        assert!(err.to_string().contains(expected), "{label}: {err}");
    }

    for mock in untouched {
        mock.assert_async().await;
    }
}

#[tokio::test]
async fn a_client_order_id_in_use_is_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 1).await;
    let clob = client(&server);
    let mut first = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    first.client_order_id = Some(ClientOrderId::new("mine-1"));
    let mut second = first.clone();
    second.price = d("0.53");

    let order = clob.place(first).await.unwrap();
    let err = clob.place(second).await.unwrap_err();

    assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine-1")));
    assert_eq!(err.class, Class::InvalidRequest);
    assert!(
        err.to_string()
            .contains("client order id `mine-1` is already in use by order `0xorder1`"),
        "{err}"
    );
    // The first order alone was posted.
    post.assert_async().await;
}

#[tokio::test]
async fn a_killed_order_frees_its_client_order_id() {
    let mut server = Server::new_async().await;
    let post = post_refused(&mut server, FAK_UNMATCHED_MSG, 2).await;
    let clob = client(&server);
    let mut request = with_options(buy("0.01", "100"), declared(TickSize::Hundredth));
    request.time_in_force = TimeInForce::ImmediateOrCancel;
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    clob.place(request.clone()).await.unwrap();
    // The kill ended the order, so its client id is free again.
    let order = clob.place(request).await.unwrap();

    assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine-1")));
    post.assert_async().await;
}

#[tokio::test]
async fn a_client_without_an_account_refuses_to_place() {
    let err = Clob::public().place(buy("0.52", "10")).await.unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest);
    assert!(err.to_string().contains("Account required"), "{err}");
}

/// `place` signs with the client's signature type, not the account target's:
/// this client is built for a proxy wallet, and without `gamma` (which `clob`
/// does not imply) it has no way to find that wallet, so the native refusal of
/// a proxy order with no funder comes back and nothing is sent. A `place` that
/// left the type to the account's EOA target would post from the EOA instead.
#[cfg(not(feature = "gamma"))]
#[tokio::test]
async fn a_proxy_client_without_gamma_refuses_to_place_and_sends_nothing() {
    let mut server = Server::new_async().await;
    let untouched = nothing_sent(&mut server).await;
    let request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));

    let err = proxy_client(&server).place(request).await.unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(err.to_string().contains("requires the `gamma` feature"), "{err}");
    for mock in untouched {
        mock.assert_async().await;
    }
}

/// With `gamma`, the same client posts the order from the proxy wallet Gamma
/// reports, as a proxy order (signature type 1), and not from the EOA.
#[cfg(feature = "gamma")]
#[tokio::test]
async fn a_proxy_client_places_from_its_proxy_wallet() {
    let mut server = Server::new_async().await;
    let profile = proxy_profile(&mut server).await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(
            json!({"order": {"maker": PROXY, "signatureType": 1}}),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(LIVE)
        .expect(1)
        .create_async()
        .await;
    let request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));

    let order = proxy_client(&server).place(request).await.unwrap();

    assert_eq!(order.status, OrderStatus::Open);
    profile.assert_async().await;
    post.assert_async().await;
}
```

Add the unit tests for the ledger and the checks. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
#[cfg(test)]
mod option_tests {
```

Replace with:

```rust
#[cfg(test)]
mod place_tests {
    use super::*;
    use crate::shared::keys::TokenId;

    fn d(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn order(n: usize) -> OrderId {
        OrderId::new(format!("0xorder{n}"))
    }

    fn client(n: usize) -> ClientOrderId {
        ClientOrderId::new(format!("client-{n}"))
    }

    fn request() -> OrderRequest {
        OrderRequest::new(
            keys::clob(&TokenId::new("100").unwrap()),
            Side::Buy,
            d("0.52"),
            d("10"),
        )
    }

    #[test]
    fn a_claimed_client_id_is_refused_to_the_next_order() {
        let mut ids = ClientIds::default();
        ids.claim(order(1), client(1)).unwrap();

        assert_eq!(ids.claim(order(2), client(1)), Err(order(1)));
        assert_eq!(ids.holder(&client(1)), Some(order(1)));
    }

    #[test]
    fn removing_an_order_frees_its_client_id() {
        let mut ids = ClientIds::default();
        ids.claim(order(1), client(1)).unwrap();
        ids.remove(&order(1));

        assert_eq!(ids.holder(&client(1)), None);
        ids.claim(order(2), client(1)).unwrap();
        assert_eq!(ids.holder(&client(1)), Some(order(2)));
    }

    #[test]
    fn the_ledger_drops_its_oldest_entry_past_its_bound() {
        let mut ids = ClientIds::default();
        for n in 0..=LEDGER_BOUND {
            ids.claim(order(n), client(n)).unwrap();
        }

        assert_eq!(ids.by_order.len(), LEDGER_BOUND);
        assert_eq!(ids.by_client.len(), LEDGER_BOUND);
        assert_eq!(ids.by_age.len(), LEDGER_BOUND);
        assert_eq!(ids.holder(&client(0)), None);
        assert_eq!(ids.holder(&client(1)), Some(order(1)));
        assert_eq!(
            ids.holder(&client(LEDGER_BOUND)),
            Some(order(LEDGER_BOUND))
        );
    }

    #[test]
    fn rekeying_moves_the_entry_and_keeps_its_client_id() {
        let mut ids = ClientIds::default();
        ids.claim(order(1), client(1)).unwrap();
        ids.rekey(&order(1), order(9));

        assert_eq!(ids.holder(&client(1)), Some(order(9)));
        ids.remove(&order(1));
        assert_eq!(ids.holder(&client(1)), Some(order(9)));
        ids.remove(&order(9));
        assert_eq!(ids.holder(&client(1)), None);
        assert!(ids.by_age.is_empty());
    }

    #[test]
    fn the_ledger_keeps_an_order_whose_post_may_have_reached_the_venue() {
        let cases = [
            (Class::Network, true),
            (Class::Unavailable { code: None }, true),
            (Class::VenueRefusal { code: None }, false),
            (Class::InvalidRequest, false),
        ];
        for (class, kept) in cases {
            let label = format!("{class:?}");
            let ids = Mutex::new(ClientIds::default());
            lock(&ids).claim(order(1), client(1)).unwrap();

            tidy(&ids, &order(1), &Err(ClassifiedError::new(class, "boom")));

            assert_eq!(lock(&ids).holder(&client(1)).is_some(), kept, "{label}");
        }
    }

    #[test]
    fn the_ledger_drops_an_order_that_ended_at_once_and_keeps_one_that_rests() {
        let ended = [
            (OrderStatus::Filled, false),
            (
                OrderStatus::Killed {
                    reason: KillReason::Unmatched,
                },
                false,
            ),
            (OrderStatus::Open, true),
        ];
        for (status, kept) in ended {
            let ids = Mutex::new(ClientIds::default());
            lock(&ids).claim(order(1), client(1)).unwrap();
            let placed = Order::new(
                order(1),
                request().key,
                Side::Buy,
                d("0.52"),
                d("10"),
                status,
            );

            tidy(&ids, &order(1), &Ok(placed));

            assert_eq!(lock(&ids).holder(&client(1)).is_some(), kept);
        }
    }

    #[test]
    fn a_price_must_sit_on_the_tick_and_strictly_between_zero_and_one() {
        for (price, tick) in [
            ("0.52", TickSize::Hundredth),
            ("0.01", TickSize::Hundredth),
            ("0.99", TickSize::Hundredth),
            ("0.525", TickSize::Thousandth),
            ("0.5", TickSize::Tenth),
            ("0.0001", TickSize::TenThousandth),
        ] {
            assert!(
                check_price_and_size(d(price), d("10"), tick).is_ok(),
                "{price}"
            );
        }
        for (price, tick, why) in [
            ("0", TickSize::Hundredth, "strictly between"),
            ("1", TickSize::Hundredth, "strictly between"),
            ("1.5", TickSize::Hundredth, "strictly between"),
            ("-0.1", TickSize::Hundredth, "strictly between"),
            ("0.525", TickSize::Hundredth, "tick size"),
            ("0.55", TickSize::Tenth, "tick size"),
        ] {
            let err = check_price_and_size(d(price), d("10"), tick).unwrap_err();
            assert_eq!(err.class, Class::InvalidRequest, "{price}");
            assert!(err.to_string().contains(why), "{price}: {err}");
        }
    }

    #[test]
    fn a_size_has_at_most_two_decimals_and_at_least_a_hundredth() {
        for size in ["0.01", "5", "5.1", "5.10", "5.00", "123.45"] {
            assert!(
                check_price_and_size(d("0.52"), d(size), TickSize::Hundredth).is_ok(),
                "{size}"
            );
        }
        for (size, why) in [
            ("5.001", "decimal places"),
            ("0.005", "decimal places"),
            ("0", "smallest order"),
            ("-1", "smallest order"),
        ] {
            let err = check_price_and_size(d("0.52"), d(size), TickSize::Hundredth).unwrap_err();
            assert_eq!(err.class, Class::InvalidRequest, "{size}");
            assert!(err.to_string().contains(why), "{size}: {err}");
        }
    }

    #[test]
    fn the_time_in_force_picks_the_order_kind_and_the_flags_it_allows() {
        let expiring = ClobOrderOptions {
            expiration: Some(UnixMillis(1_900_000_000_000)),
            ..ClobOrderOptions::default()
        };
        let plain = ClobOrderOptions::default();
        let with = |time_in_force, post_only, reduce_only| {
            let mut request = request();
            request.time_in_force = time_in_force;
            request.post_only = post_only;
            request.reduce_only = reduce_only;
            request
        };
        let gtc = TimeInForce::GoodTilCancelled;
        let ioc = TimeInForce::ImmediateOrCancel;
        let fok = TimeInForce::FillOrKill;

        assert_eq!(order_kind(&with(gtc, false, false), &plain).unwrap(), OrderKind::Gtc);
        assert_eq!(order_kind(&with(gtc, true, false), &plain).unwrap(), OrderKind::Gtc);
        assert_eq!(order_kind(&with(gtc, false, false), &expiring).unwrap(), OrderKind::Gtd);
        assert_eq!(order_kind(&with(ioc, false, false), &plain).unwrap(), OrderKind::Fak);
        assert_eq!(order_kind(&with(fok, false, false), &plain).unwrap(), OrderKind::Fok);
        for refused in [
            order_kind(&with(gtc, false, true), &plain),
            order_kind(&with(ioc, true, false), &plain),
            order_kind(&with(fok, false, false), &expiring),
        ] {
            assert_eq!(refused.unwrap_err().class, Class::InvalidRequest);
        }
    }

    #[test]
    fn a_matched_order_settles_by_its_time_in_force_and_what_matched() {
        let gtc = TimeInForce::GoodTilCancelled;
        let ioc = TimeInForce::ImmediateOrCancel;
        let fok = TimeInForce::FillOrKill;

        for time_in_force in [gtc, ioc, fok] {
            assert_eq!(settled(time_in_force, d("10"), d("10")), OrderStatus::Filled);
        }
        assert_eq!(
            settled(ioc, d("4"), d("10")),
            OrderStatus::Killed {
                reason: KillReason::Unmatched
            }
        );
        assert_eq!(
            settled(fok, d("4"), d("10")),
            OrderStatus::Killed {
                reason: KillReason::Unfillable
            }
        );
        assert_eq!(settled(gtc, d("4"), d("10")), OrderStatus::Open);
    }

    /// Hardhat account 0, and the Deposit Wallet it owns in py-sdk's fixtures.
    const KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    const WALLET: alloy::primitives::Address =
        alloy::primitives::address!("57ffbc34de23124faeb8387fcd689d314e57accd");

    /// A Deposit Wallet account can place: the order goes out as signature
    /// type 3 with the wallet as maker and signer, carries the signature
    /// `sign_order_as` gives, and `place` names it by `order_hash`, which no
    /// plain digest could (`compute_order_digest` refuses type 3). The test
    /// sits here because `order_hash` is crate-private.
    #[tokio::test]
    async fn a_deposit_wallet_account_places_a_type_3_order_named_by_its_hash() {
        use crate::clob::types::{SignatureType, SignedOrder};
        use crate::clob::{Account, ClobBuilder, Credentials, DepositWalletRole, SigningTarget};

        let posted = std::sync::Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let sink = posted.clone();
        let mut server = mockito::Server::new_async().await;
        // The answer names no order, so the id `place` returns is the one it
        // computed before signing.
        let post = server
            .mock("POST", "/order")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body_from_request(move |request| {
                sink.lock().unwrap().push(request.body().unwrap().clone());
                br#"{"success":true,"status":"live","transactionsHashes":[]}"#.to_vec()
            })
            .expect(1)
            .create_async()
            .await;
        let account = Account::new(
            KEY,
            Credentials {
                key: "test-key".into(),
                secret: "c2VjcmV0".into(),
                passphrase: "test-pass".into(),
            },
        )
        .unwrap()
        .with_target(SigningTarget::DepositWallet {
            wallet: WALLET,
            role: DepositWalletRole::Owner,
        });
        let clob = ClobBuilder::new()
            .base_url(server.url())
            .with_account(account)
            .build()
            .unwrap();
        let mut asked = request();
        asked
            .extensions
            .insert(ClobOrderOptions {
                tick_size: Some(TickSize::Hundredth),
                neg_risk: Some(false),
                ..ClobOrderOptions::default()
            })
            .unwrap();

        let placed = clob.place(asked).await.unwrap();

        post.assert_async().await;
        let body: serde_json::Value = serde_json::from_slice(&posted.lock().unwrap()[0]).unwrap();
        // The wire writes the salt as a number and the order holds it as text.
        let mut wire = body["order"].clone();
        wire["salt"] = serde_json::Value::String(wire["salt"].to_string());
        let signed: SignedOrder = serde_json::from_value(wire).unwrap();
        assert_eq!(signed.order.signature_type, SignatureType::Poly1271);
        assert_eq!(signed.order.maker, WALLET);
        assert_eq!(signed.order.signer, WALLET);
        let expected = clob.sign_order(&signed.order).await.unwrap();
        assert_eq!(signed.signature, expected.signature);
        assert_eq!(
            signed.signature.len(),
            2 + 317 * 2,
            "the owner's ERC-7739 envelope"
        );
        let hash = order_hash(&signed.order, clob.chain_id).unwrap();
        assert_eq!(placed.id, OrderId::new(format!("0x{}", hex::encode(hash))));
        assert_eq!(placed.status, OrderStatus::Open);
    }
}

#[cfg(test)]
mod option_tests {
```

Register the test target. Current (`polyoxide-polymarket/Cargo.toml:334-337`):

```toml
# The clob's traits read Data's public trades and Gamma's events too.
[[test]]
name = "venue_clob"
required-features = ["clob", "gamma", "data"]
```

Replace with:

```toml
# The clob's traits read Data's public trades and Gamma's events too.
[[test]]
name = "venue_clob"
required-features = ["clob", "gamma", "data"]

# `Trading` for the CLOB reads Data's positions and the supervised user socket.
[[test]]
name = "trading_clob"
required-features = ["clob-ws", "data"]
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue::trading
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob
```

Expected: the library build fails with `cannot find struct ClientIds in this scope`, `cannot find function check_price_and_size`, `cannot find function order_kind` and the same for `settled`, `tidy`, `lock` and `Posting`; the integration build fails with `no method named place found for struct Clob in the current scope` (the trait bound `Clob: Trading` is not satisfied).

- [ ] **Step 3: Implement**

Hold the ledger on the client. Current (`polyoxide-polymarket/src/clob/client.rs:64-70`):

```rust
    #[cfg(feature = "gamma")]
    pub(crate) gamma: Gamma,
    /// The Data client the `Trades` implementation reads public trades
    /// through, since the CLOB serves none.
    #[cfg(feature = "data")]
    pub(crate) data: DataApi,
}
```

Replace with:

```rust
    #[cfg(feature = "gamma")]
    pub(crate) gamma: Gamma,
    /// The Data client the `Trades` implementation reads public trades
    /// through, since the CLOB serves none.
    #[cfg(feature = "data")]
    pub(crate) data: DataApi,
    /// The client order ids of the orders this client placed, which the CLOB
    /// stores nowhere. Every clone shares it, and so does every events stream.
    #[cfg(all(feature = "clob-ws", feature = "data"))]
    pub(crate) ids: std::sync::Arc<std::sync::Mutex<crate::clob::venue::ClientIds>>,
}
```

Current (`polyoxide-polymarket/src/clob/client.rs:966-970`):

```rust
            #[cfg(feature = "gamma")]
            gamma,
            #[cfg(feature = "data")]
            data,
        })
```

Replace with:

```rust
            #[cfg(feature = "gamma")]
            gamma,
            #[cfg(feature = "data")]
            data,
            #[cfg(all(feature = "clob-ws", feature = "data"))]
            ids: Default::default(),
        })
```

Re-export the ledger inside the crate. Current (`polyoxide-polymarket/src/clob/venue.rs`, from Task 5.1):

```rust
#[cfg(all(feature = "clob-ws", feature = "data"))]
pub use trading::{BalanceExt, ClobOrderOptions, FillExt, OrderExt, PositionExt};
```

Replace with:

```rust
#[cfg(all(feature = "clob-ws", feature = "data"))]
pub use trading::{BalanceExt, ClobOrderOptions, FillExt, OrderExt, PositionExt};

#[cfg(all(feature = "clob-ws", feature = "data"))]
pub(crate) use trading::ClientIds;
```

Add the imports. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
use crate::clob::types::TickSize;
```

Replace with:

```rust
use crate::clob::types::TickSize;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use alloy::primitives::hex;
use polyoxide_venue::{
    Balance, Class, ClassifiedError, ClientOrderId, Extensions, KillReason, MarketKey, Order,
    OrderId, OrderRequest, OrderStatus, Position, Side, TimeInForce, Trading, TradingEvents,
};
use rust_decimal::prelude::ToPrimitive;

use crate::clob::core::eip712::order_hash;
use crate::clob::error::ClobError;
use crate::clob::types::{
    Order as NativeOrder, OrderKind, OrderSide, PartialCreateOrderOptions,
};
use crate::clob::{Clob, CreateOrderParams};
use crate::shared::keys;
```

Add the ledger, the checks and the impl. In the same file, Current:

```rust
#[cfg(test)]
mod place_tests {
```

Replace with:

```rust
/// The most client order ids the ledger holds. The oldest goes first.
const LEDGER_BOUND: usize = 10_000;

/// The in-process record of the client order ids of the orders a `Clob`
/// placed.
///
/// The CLOB stores no client id, so the client keeps the ones it was given,
/// by the order's id. Every clone of a `Clob` shares one ledger, and so does
/// every events stream it opens. It is lost when the process ends, and an
/// order placed elsewhere has no entry.
#[derive(Debug, Default)]
pub(crate) struct ClientIds {
    by_order: HashMap<OrderId, (ClientOrderId, u64)>,
    by_client: HashMap<ClientOrderId, OrderId>,
    by_age: BTreeMap<u64, OrderId>,
    next_age: u64,
}

impl ClientIds {
    /// The order holding `client`, when one does.
    pub(crate) fn holder(&self, client: &ClientOrderId) -> Option<OrderId> {
        self.by_client.get(client).cloned()
    }

    /// Records that `order` carries `client`, dropping the oldest entry once
    /// the ledger is full. Refuses with the order already holding `client`.
    pub(crate) fn claim(&mut self, order: OrderId, client: ClientOrderId) -> Result<(), OrderId> {
        if let Some(holder) = self.by_client.get(&client) {
            return Err(holder.clone());
        }
        self.remove(&order);
        while self.by_order.len() >= LEDGER_BOUND {
            let Some((_, oldest)) = self.by_age.pop_first() else {
                break;
            };
            if let Some((evicted, _)) = self.by_order.remove(&oldest) {
                self.by_client.remove(&evicted);
            }
        }
        self.next_age += 1;
        self.by_age.insert(self.next_age, order.clone());
        self.by_client.insert(client.clone(), order.clone());
        self.by_order.insert(order, (client, self.next_age));
        Ok(())
    }

    /// Moves an entry to the id the CLOB gave the order, keeping its age.
    pub(crate) fn rekey(&mut self, from: &OrderId, to: OrderId) {
        if let Some((client, age)) = self.by_order.remove(from) {
            self.by_age.insert(age, to.clone());
            self.by_client.insert(client.clone(), to.clone());
            self.by_order.insert(to, (client, age));
        }
    }

    /// Drops the entry of `order`.
    pub(crate) fn remove(&mut self, order: &OrderId) {
        if let Some((client, age)) = self.by_order.remove(order) {
            self.by_client.remove(&client);
            self.by_age.remove(&age);
        }
    }
}

/// Locks the ledger. No code panics while holding it, so a poisoned lock
/// still holds whole maps and is read as it is.
pub(crate) fn lock(ids: &Mutex<ClientIds>) -> MutexGuard<'_, ClientIds> {
    ids.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A request refused before anything was sent.
fn refuse(message: impl Into<String>) -> ClassifiedError {
    ClassifiedError::new(Class::InvalidRequest, message.into())
}

/// The CLOB answered, and what it answered is a refusal.
fn refusal(message: impl Into<String>) -> ClassifiedError {
    ClassifiedError::new(Class::VenueRefusal { code: None }, message.into())
}

fn duplicate(client: &ClientOrderId, holder: &OrderId) -> ClassifiedError {
    refuse(format!(
        "client order id `{client}` is already in use by order `{holder}`"
    ))
}

/// The order kind a request maps to, after the checks that need no lookup:
/// no reduce-only order, post-only and an expiration only with
/// good-til-cancelled.
fn order_kind(
    request: &OrderRequest,
    options: &ClobOrderOptions,
) -> Result<OrderKind, ClassifiedError> {
    if request.reduce_only {
        return Err(refuse("the CLOB has no reduce-only orders"));
    }
    let resting = request.time_in_force == TimeInForce::GoodTilCancelled;
    if request.post_only && !resting {
        return Err(refuse(
            "the CLOB supports post-only only with good-til-cancelled orders",
        ));
    }
    if options.expiration.is_some() && !resting {
        return Err(refuse(
            "the CLOB supports an expiration only with good-til-cancelled orders",
        ));
    }
    match request.time_in_force {
        TimeInForce::GoodTilCancelled if options.expiration.is_some() => Ok(OrderKind::Gtd),
        TimeInForce::GoodTilCancelled => Ok(OrderKind::Gtc),
        TimeInForce::ImmediateOrCancel => Ok(OrderKind::Fak),
        TimeInForce::FillOrKill => Ok(OrderKind::Fok),
        _ => Err(refuse("the CLOB cannot honour this time in force")),
    }
}

/// The price and size checks: the price lies strictly between 0 and 1 on the
/// tick, and the size has at most two decimals and is at least a hundredth.
fn check_price_and_size(
    price: Decimal,
    size: Decimal,
    tick: TickSize,
) -> Result<(), ClassifiedError> {
    let step = Decimal::new(1, tick.decimals());
    if price <= Decimal::ZERO || price >= Decimal::ONE {
        return Err(refuse(format!(
            "the price {price} must lie strictly between 0 and 1"
        )));
    }
    if !(price % step).is_zero() {
        return Err(refuse(format!(
            "the price {price} is not a whole multiple of the tick size {step}"
        )));
    }
    if size.normalize().scale() > 2 {
        return Err(refuse(format!(
            "the size {size} has more than two decimal places"
        )));
    }
    if size < Decimal::new(1, 2) {
        return Err(refuse(format!(
            "the size {size} is below the smallest order, 0.01 shares"
        )));
    }
    Ok(())
}

/// What a `matched` placement became, once the order was read back.
fn settled(time_in_force: TimeInForce, matched: Decimal, size: Decimal) -> OrderStatus {
    if matched >= size {
        return OrderStatus::Filled;
    }
    match time_in_force {
        TimeInForce::ImmediateOrCancel => OrderStatus::Killed {
            reason: KillReason::Unmatched,
        },
        TimeInForce::FillOrKill => OrderStatus::Killed {
            reason: KillReason::Unfillable,
        },
        _ => OrderStatus::Open,
    }
}

/// Drops the ledger entry of an order that ended at once or never existed.
/// After a `Network` or `Unavailable` failure the post may have reached the
/// venue, so the entry stays until the bound evicts it.
fn tidy(ids: &Mutex<ClientIds>, id: &OrderId, result: &Result<Order, ClassifiedError>) {
    let drop_entry = match result {
        Err(err) => !matches!(err.class, Class::Network | Class::Unavailable { .. }),
        Ok(order) => matches!(order.status, OrderStatus::Killed { .. } | OrderStatus::Filled),
    };
    if drop_entry {
        let mut ids = lock(ids);
        ids.remove(id);
        if let Ok(order) = result {
            ids.remove(&order.id);
        }
    }
}

/// One placement in flight: the request, the options it carried and what the
/// client resolved before sending.
struct Posting<'a> {
    request: &'a OrderRequest,
    options: ClobOrderOptions,
    kind: OrderKind,
    neg_risk: bool,
    /// The order's EIP-712 hash, computed before it is signed.
    id: OrderId,
}

impl Posting<'_> {
    /// The order the request became, with the CLOB's facts in its extensions.
    fn order(&self, id: OrderId, status: OrderStatus, filled: Decimal, ext: OrderExt) -> Order {
        let mut order = Order::new(
            id,
            self.request.key.clone(),
            self.request.side,
            self.request.price,
            self.request.size,
            status,
        );
        order.filled = filled;
        order.time_in_force = Some(self.request.time_in_force);
        order.client_order_id = self.request.client_order_id.clone();
        order.extensions = Extensions::of(OrderExt {
            expiration: self.options.expiration,
            neg_risk: Some(self.neg_risk),
            ..ext
        });
        order
    }

    /// An order the CLOB killed on arrival.
    fn ended(&self, reason: KillReason) -> Order {
        self.order(
            self.id.clone(),
            OrderStatus::Killed { reason },
            Decimal::ZERO,
            OrderExt::default(),
        )
    }
}

impl Clob {
    /// The market's tick size: the options', else the CLOB's.
    async fn trading_tick_size(
        &self,
        token: &keys::TokenId,
        options: &ClobOrderOptions,
    ) -> Result<TickSize, ClassifiedError> {
        if let Some(tick) = options.tick_size {
            return Ok(tick);
        }
        let response = self.markets().tick_size(token.as_str()).send().await?;
        let value: f64 = response
            .minimum_tick_size
            .parse()
            .map_err(|_| super::decode("a tick size that is not a number"))?;
        Ok(TickSize::try_from(value)?)
    }

    /// Signs and posts one order, and reads the answer into an [`Order`].
    async fn trading_submit(
        &self,
        posting: &Posting<'_>,
        native: &NativeOrder,
    ) -> Result<Order, ClassifiedError> {
        let signed = self.sign_order(native).await?;
        let response = match self
            .post_order(&signed, posting.kind, posting.request.post_only)
            .await
        {
            Ok(response) => response,
            // The kill outcomes are matched on the native variants, before
            // any conversion to a `ClassifiedError` (AD-20).
            Err(ClobError::FakUnmatched { .. }) => {
                return Ok(posting.ended(KillReason::Unmatched));
            }
            Err(ClobError::FokUnfilled { .. }) => {
                return Ok(posting.ended(KillReason::Unfillable));
            }
            Err(err) => return Err(err.into()),
        };
        if !response.success {
            return Err(refusal(
                response
                    .error_msg
                    .clone()
                    .unwrap_or_else(|| "the CLOB refused the order".to_owned()),
            ));
        }
        let id = match response.order_id.as_deref() {
            Some(venue) if !venue.is_empty() && venue != posting.id.as_str() => {
                let venue = OrderId::new(venue);
                tracing::warn!(
                    computed = %posting.id,
                    venue = %venue,
                    "the CLOB named an order other than the one computed; keeping its id"
                );
                lock(&self.ids).rekey(&posting.id, venue.clone());
                venue
            }
            _ => posting.id.clone(),
        };
        let ext = OrderExt {
            status: response.status.clone(),
            transaction_hashes: response.transaction_hashes.clone(),
            ..OrderExt::default()
        };
        let word = response.status.as_deref().map(str::to_ascii_lowercase);
        let (status, filled) = if word.as_deref() == Some("matched") {
            match self.trading_matched_size(&id).await {
                Ok(matched) => (
                    settled(posting.request.time_in_force, matched, posting.request.size),
                    matched,
                ),
                // The order exists on the venue, so an error would invite a
                // duplicate placement; the events stream reports the rest.
                Err(err) => {
                    tracing::warn!(
                        order = %id,
                        error = %err,
                        "could not read back a matched order; reporting it open"
                    );
                    (OrderStatus::Open, Decimal::ZERO)
                }
            }
        } else {
            (OrderStatus::Open, Decimal::ZERO)
        };
        Ok(posting.order(id, status, filled, ext))
    }

    /// What has matched of an order, from `GET /data/order/{id}`.
    async fn trading_matched_size(&self, id: &OrderId) -> Result<Decimal, ClassifiedError> {
        let row = self.orders()?.get(id.as_str()).send().await?;
        row.size_matched
            .parse()
            .map_err(|_| super::decode("a matched size that is not a decimal"))
    }
}

/// `Trading` on the CLOB, through the native order, cancel, Data and balance
/// routes and, for `events`, the supervised user socket.
///
/// Every method needs an account, and fails with the CLOB's own "Account
/// required" error without one. The address that trades is the client's
/// signature type's: `ClobBuilder::signature_type`, else the account target's.
/// `place` signs as it, and `positions` and `events` resolve their maker
/// address from it, so a client built for a proxy wallet trades and reads as
/// that wallet. The CLOB stores no client order id, so a
/// `Clob` remembers the ones it placed orders with, in memory: the ledger is
/// shared by its clones and by the streams `events` opens, holds at most
/// 10,000 entries, is lost when the process ends, and knows nothing of an
/// order placed elsewhere, which carries no client id on any record.
impl Trading for Clob {
    async fn place(&self, request: OrderRequest) -> Result<Order, ClassifiedError> {
        self.account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to place an order"))?;
        let token = keys::token_id(&request.key)?;
        request
            .extensions
            .ensure_declared(&[TypeId::of::<ClobOrderOptions>()])?;
        let options = request
            .extensions
            .get::<ClobOrderOptions>()
            .copied()
            .unwrap_or_default();
        let kind = order_kind(&request, &options)?;
        if let Some(client) = &request.client_order_id {
            let holder = lock(&self.ids).holder(client);
            if let Some(holder) = holder {
                return Err(duplicate(client, &holder));
            }
        }
        let tick = self.trading_tick_size(&token, &options).await?;
        check_price_and_size(request.price, request.size, tick)?;

        let price = request
            .price
            .to_f64()
            .ok_or_else(|| refuse("the price does not fit a float"))?;
        let size = request
            .size
            .to_f64()
            .ok_or_else(|| refuse("the size does not fit a float"))?;
        let params = CreateOrderParams {
            token_id: token.as_str().to_owned(),
            price,
            size,
            side: match request.side {
                Side::Buy => OrderSide::Buy,
                Side::Sell => OrderSide::Sell,
            },
            order_type: kind,
            post_only: request.post_only,
            expiration: options.expiration.map(|time| time.0 / 1_000),
            // The order is signed with the client's own signature type (the
            // builder's override, else the account target's), as `balances`
            // reads with it, and its maker is resolved from it: a client built
            // for a proxy wallet places from that wallet. No per-order funder.
            funder: None,
            signature_type: Some(self.signature_type),
        };
        // The options name what they know, so nothing is fetched twice.
        let native = self
            .create_order(
                &params,
                Some(PartialCreateOrderOptions {
                    tick_size: Some(tick),
                    neg_risk: options.neg_risk,
                }),
            )
            .await?;
        let neg_risk = native.neg_risk;
        // The order's id is its EIP-712 hash, which the client computes before
        // signing, for every signature type: a Deposit Wallet's order is signed
        // as an ERC-7739 envelope, but the exchange names it by this hash too.
        let hash = order_hash(&native, self.chain_id)?;
        let id = OrderId::new(format!("0x{}", hex::encode(hash)));
        if let Some(client) = &request.client_order_id {
            let claimed = lock(&self.ids).claim(id.clone(), client.clone());
            if let Err(holder) = claimed {
                return Err(duplicate(client, &holder));
            }
        }

        let posting = Posting {
            request: &request,
            options,
            kind,
            neg_risk,
            id: id.clone(),
        };
        let result = self.trading_submit(&posting, &native).await;
        tidy(&self.ids, &id, &result);
        result
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn cancel(&self, _id: &OrderId) -> Result<(), ClassifiedError> {
        Err(refusal("cancel is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn cancel_all(&self, _key: Option<&MarketKey>) -> Result<Vec<OrderId>, ClassifiedError> {
        Err(refusal("cancel_all is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn open_orders(&self, _key: Option<&MarketKey>) -> Result<Vec<Order>, ClassifiedError> {
        Err(refusal("open_orders is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn positions(&self) -> Result<Vec<Position>, ClassifiedError> {
        Err(refusal("positions is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn balances(&self) -> Result<Vec<Balance>, ClassifiedError> {
        Err(refusal("balances is not implemented yet"))
    }

    // Temporary stub; Task 5.4 replaces it.
    fn events(&self) -> TradingEvents {
        TradingEvents::new(futures_util::stream::empty::<
            Result<polyoxide_venue::TradingEvent, ClassifiedError>,
        >())
    }
}

#[cfg(test)]
mod place_tests {
```

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue::trading
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob
```

Expected: the library run prints `test result: ok. 14 passed; 0 failed` (eleven in `place_tests` and three in `option_tests`), and the integration run prints `test result: ok. 14 passed; 0 failed` (the `gamma` feature's `a_proxy_client_places_from_its_proxy_wallet` is not built here).

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws,data --all-targets -- -D warnings
git add -N polyoxide-polymarket/tests/trading_clob.rs
git commit -m "feat(clob): Trading::place with the client-id ledger and its checks (Story 7.4)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-polymarket/src/clob/venue.rs polyoxide-polymarket/src/clob/venue/trading.rs polyoxide-polymarket/src/clob/client.rs polyoxide-polymarket/Cargo.toml polyoxide-polymarket/tests/trading_clob.rs
```

### Task 5.3: `cancel`, `cancel_all`, `open_orders`, `positions` and `balances`

Replaces the five stubs of Task 5.2 with the real methods, and adds the parsing helpers and mappers they share. `decimal_text`, `side_text` and `key_of` are `pub(super)` because Task 5.4's events adapter reads them too.

**Files:**
- Modify: `polyoxide-polymarket/src/clob/venue/trading.rs`
- Test: `polyoxide-polymarket/tests/trading_clob.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-polymarket/tests/trading_clob.rs`, add imports. Current:

```rust
use serde_json::{json, Value};
```

Replace with:

```rust
use polyoxide_polymarket::clob::venue::{BalanceExt, PositionExt};
use polyoxide_venue::OrderId;
use serde_json::{json, Value};
```

Add the read-side tests before the place section of `polyoxide-polymarket/tests/trading_clob.rs`. Current:

```rust
// --- place ---
```

Replace with:

```rust
// --- cancel, orders, positions and balances ---

/// A `DELETE` answering with a `BatchCancelResponse` body.
async fn delete_answering(
    server: &mut ServerGuard,
    path: &str,
    body: Value,
    answer: &str,
) -> Mock {
    server
        .mock("DELETE", path)
        .match_body(Matcher::Json(body))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(answer)
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn cancel_is_ok_when_the_order_is_among_the_canceled() {
    let mut server = Server::new_async().await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xa"}),
        r#"{"canceled":["0xa"],"notCanceled":{}}"#,
    )
    .await;

    client(&server).cancel(&OrderId::new("0xa")).await.unwrap();

    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_is_a_venue_refusal_carrying_the_venues_reason() {
    let mut server = Server::new_async().await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xa"}),
        r#"{"canceled":[],"notCanceled":{"0xa":"order can't be found - already canceled or matched"}}"#,
    )
    .await;

    let err = client(&server)
        .cancel(&OrderId::new("0xa"))
        .await
        .unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(err.to_string().contains("already canceled or matched"), "{err}");
    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_of_an_order_the_clob_does_not_answer_for_is_a_venue_refusal() {
    let mut server = Server::new_async().await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xa"}),
        r#"{"canceled":[],"notCanceled":{}}"#,
    )
    .await;

    let err = client(&server)
        .cancel(&OrderId::new("0xa"))
        .await
        .unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(
        err.to_string()
            .contains("the CLOB did not answer for order 0xa"),
        "{err}"
    );
    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_all_returns_only_the_ids_the_clob_cancelled() {
    let mut server = Server::new_async().await;
    let delete = server
        .mock("DELETE", "/cancel-all")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"canceled":["0xa","0xb"],"notCanceled":{"0xc":"order can't be found"}}"#)
        .expect(1)
        .create_async()
        .await;

    let ids = client(&server).cancel_all(None).await.unwrap();

    // `0xc` stays open: it is in `notCanceled`.
    assert_eq!(ids, [OrderId::new("0xa"), OrderId::new("0xb")]);
    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_all_for_a_key_resolves_the_market_and_names_both_it_and_the_token() {
    let mut server = Server::new_async().await;
    let lookup = server
        .mock("GET", "/markets-by-token/100")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"condition_id":"0xcond","primary_token_id":"100","secondary_token_id":"101"}"#)
        .expect(1)
        .create_async()
        .await;
    let delete = delete_answering(
        &mut server,
        "/cancel-market-orders",
        json!({"market": "0xcond", "asset_id": "100"}),
        r#"{"canceled":["0xa"],"notCanceled":{}}"#,
    )
    .await;

    let ids = client(&server).cancel_all(Some(&key())).await.unwrap();

    assert_eq!(ids, [OrderId::new("0xa")]);
    lookup.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn cancelling_an_order_frees_the_client_order_id_the_ledger_held() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let delete = server
        .mock("DELETE", "/cancel-all")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"canceled":["0xorder1"],"notCanceled":{}}"#)
        .expect(1)
        .create_async()
        .await;
    let clob = client(&server);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    clob.place(request.clone()).await.unwrap();
    let refused = clob.place(request.clone()).await.unwrap_err();
    clob.cancel_all(None).await.unwrap();
    let again = clob.place(request).await.unwrap();

    assert!(refused.to_string().contains("already in use"), "{refused}");
    assert_eq!(again.client_order_id, Some(ClientOrderId::new("mine-1")));
    post.assert_async().await;
    delete.assert_async().await;
}

/// A `/data/orders` body: `rows` and the cursor to the next page.
fn orders_page(rows: Vec<Value>, next_cursor: &str) -> String {
    json!({"data": rows, "next_cursor": next_cursor}).to_string()
}

#[tokio::test]
async fn open_orders_pages_to_the_end_sentinel() {
    let mut server = Server::new_async().await;
    let first = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::Missing)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(vec![open_order("0xa", "LIVE", "0")], "MQ=="))
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::UrlEncoded("next_cursor".into(), "MQ==".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(vec![open_order("0xb", "MATCHED", "10")], "LTE="))
        .expect(1)
        .create_async()
        .await;

    let orders = client(&server).open_orders(None).await.unwrap();

    let ids: Vec<&str> = orders.iter().map(|order| order.id.as_str()).collect();
    assert_eq!(ids, ["0xa", "0xb"]);
    assert_eq!(orders[0].status, OrderStatus::Open);
    assert_eq!(orders[1].status, OrderStatus::Filled);
    assert_eq!(orders[1].filled, d("10"));
    assert_eq!(orders[0].key, key());
    assert_eq!(orders[0].price, d("0.52"));
    assert_eq!(orders[0].time_in_force, Some(TimeInForce::GoodTilCancelled));
    assert_eq!(orders[0].created, Some(UnixMillis(1_792_000_000_000)));
    let ext = orders[0].extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.status.as_deref(), Some("LIVE"));
    assert_eq!(ext.condition_id.as_deref(), Some(MARKET));
    assert_eq!(ext.trade_ids, ["trade-1"]);
    first.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn open_orders_of_a_key_filters_by_its_token() {
    let mut server = Server::new_async().await;
    let list = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::UrlEncoded("asset_id".into(), TOKEN.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(vec![open_order("0xa", "LIVE", "0")], "LTE="))
        .expect(1)
        .create_async()
        .await;

    let orders = client(&server).open_orders(Some(&key())).await.unwrap();

    assert_eq!(orders.len(), 1);
    list.assert_async().await;
}

#[tokio::test]
async fn open_orders_stops_on_a_cursor_it_has_already_followed() {
    let mut server = Server::new_async().await;
    // Every page names the same next cursor.
    let list = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(vec![open_order("0xa", "LIVE", "0")], "MQ=="))
        .expect(2)
        .create_async()
        .await;

    let err = client(&server).open_orders(None).await.unwrap_err();

    assert_eq!(err.class, Class::Decode, "{err}");
    list.assert_async().await;
}

/// A `/positions` row with the asset `asset`.
fn position_row(asset: String) -> Value {
    json!({
        "proxyWallet": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "asset": asset,
        "conditionId": MARKET,
        "size": 12.5,
        "avgPrice": 0.4,
        "initialValue": 5.0,
        "currentValue": 6.25,
        "cashPnl": 1.25,
        "percentPnl": 25.0,
        "totalBought": 12.5,
        "realizedPnl": 0.0,
        "percentRealizedPnl": 0.0,
        "curPrice": 0.5,
        "redeemable": false,
        "mergeable": false,
        "title": "A market",
        "slug": "a-market",
        "icon": null,
        "eventSlug": null,
        "outcome": "Yes",
        "outcomeIndex": 0,
        "oppositeOutcome": "No",
        "oppositeAsset": "101",
        "endDate": null,
        "negativeRisk": false
    })
}

/// The query of one `/positions` page: the maker, no dust filter, 500 rows.
fn positions_query(user: &str, offset: &str) -> Matcher {
    Matcher::AllOf(vec![
        Matcher::UrlEncoded("user".into(), user.into()),
        Matcher::UrlEncoded("sizeThreshold".into(), "0".into()),
        Matcher::UrlEncoded("limit".into(), "500".into()),
        Matcher::UrlEncoded("offset".into(), offset.into()),
    ])
}

fn position_rows(count: usize) -> String {
    let rows: Vec<Value> = (0..count)
        .map(|n| position_row((1_000 + n).to_string()))
        .collect();
    Value::Array(rows).to_string()
}

#[tokio::test]
async fn positions_pages_by_offset_with_no_dust_filter_for_the_maker_address() {
    let mut server = Server::new_async().await;
    let clob = client(&server);
    let user = clob.account().unwrap().address().to_string();
    let first = server
        .mock("GET", "/positions")
        .match_query(positions_query(&user, "0"))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(500))
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("GET", "/positions")
        .match_query(positions_query(&user, "500"))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(3))
        .expect(1)
        .create_async()
        .await;

    let positions = clob.positions().await.unwrap();

    assert_eq!(positions.len(), 503);
    assert_eq!(
        positions[0].key,
        keys::clob(&TokenId::new("1000").unwrap())
    );
    assert_eq!(positions[0].size, d("12.5"));
    assert_eq!(positions[0].average_price, Some(d("0.4")));
    let ext = positions[0].extensions.get::<PositionExt>().unwrap();
    assert_eq!(ext.condition_id.as_deref(), Some(MARKET));
    assert_eq!(ext.current_price, Some(d("0.5")));
    assert_eq!(ext.outcome_index, Some(0));
    assert_eq!(ext.negative_risk, Some(false));
    first.assert_async().await;
    second.assert_async().await;
}

/// `positions` reads the address `place` trades from, the client's signature
/// type's. Without `gamma` a proxy client cannot find its proxy wallet, so the
/// native refusal comes back and nothing is sent; a `positions` that resolved
/// from the account target's EOA would list the EOA's rows instead.
#[cfg(not(feature = "gamma"))]
#[tokio::test]
async fn a_proxy_client_without_gamma_refuses_to_list_positions_and_sends_nothing() {
    let mut server = Server::new_async().await;
    let untouched = nothing_sent(&mut server).await;

    let err = proxy_client(&server).positions().await.unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(err.to_string().contains("requires the `gamma` feature"), "{err}");
    for mock in untouched {
        mock.assert_async().await;
    }
}

/// With `gamma`, the proxy client lists the positions of its proxy wallet.
#[cfg(feature = "gamma")]
#[tokio::test]
async fn a_proxy_client_lists_the_positions_of_its_proxy_wallet() {
    let mut server = Server::new_async().await;
    let profile = proxy_profile(&mut server).await;
    let list = server
        .mock("GET", "/positions")
        .match_query(positions_query(PROXY, "0"))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(2))
        .expect(1)
        .create_async()
        .await;

    let positions = proxy_client(&server).positions().await.unwrap();

    assert_eq!(positions.len(), 2);
    profile.assert_async().await;
    list.assert_async().await;
}

#[tokio::test]
async fn positions_refuse_a_wallet_with_more_rows_than_the_offset_reaches() {
    let mut server = Server::new_async().await;
    // Every page is full: offsets 0 to 10000 are 21 pages, and a 22nd would
    // need offset 10500.
    let pages = server
        .mock("GET", "/positions")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(500))
        .expect(21)
        .create_async()
        .await;

    let err = client(&server).positions().await.unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(err.to_string().contains("more positions"), "{err}");
    pages.assert_async().await;
}

#[tokio::test]
async fn balances_are_scaled_from_six_decimals_and_name_every_allowance() {
    let mut server = Server::new_async().await;
    let balance = server
        .mock("GET", "/balance-allowance")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("asset_type".into(), "COLLATERAL".into()),
            Matcher::UrlEncoded("signature_type".into(), "0".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "balance": "141171137",
                "allowances": {
                    "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E": "999999",
                    "0xC5d563A36AE78145C45a50134d48A1215220f80a":
                        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
                }
            })
            .to_string(),
        )
        .expect(1)
        .create_async()
        .await;

    let balances = client(&server).balances().await.unwrap();

    assert_eq!(balances.len(), 1);
    assert_eq!(balances[0].asset, "pUSD");
    assert_eq!(balances[0].total, Some(d("141.171137")));
    assert_eq!(balances[0].available, None);
    let ext = balances[0].extensions.get::<BalanceExt>().unwrap();
    assert_eq!(
        ext.allowances["0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"],
        d("0.999999")
    );
    // An unlimited approval is larger than a decimal holds.
    assert_eq!(
        ext.allowances["0xC5d563A36AE78145C45a50134d48A1215220f80a"],
        Decimal::MAX
    );
    balance.assert_async().await;
}

#[tokio::test]
async fn every_read_needs_an_account() {
    let clob = Clob::public();

    let errs = [
        clob.cancel(&OrderId::new("0xa")).await.unwrap_err(),
        clob.cancel_all(None).await.unwrap_err(),
        clob.open_orders(None).await.unwrap_err(),
        clob.positions().await.unwrap_err(),
        clob.balances().await.unwrap_err(),
    ];

    for err in errs {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
        assert!(err.to_string().contains("Account required"), "{err}");
    }
}

// --- place ---
```

Add the unit tests for the helpers. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
#[cfg(test)]
mod place_tests {
```

Replace with:

```rust
#[cfg(test)]
mod read_tests {
    use serde_json::json;

    use super::*;

    fn d(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn row(status: &str, order_type: &str, expiration: &str) -> OpenOrder {
        serde_json::from_value(json!({
            "id": "0xa",
            "status": status,
            "owner": "key",
            "maker_address": "0xMaker",
            "market": "0xcondition",
            "asset_id": "100",
            "side": "SELL",
            "original_size": "10",
            "size_matched": "4",
            "price": "0.52",
            "outcome": "Yes",
            "expiration": expiration,
            "order_type": order_type,
            "associate_trades": ["t1", "t2"],
            "created_at": 1_792_000_000_i64
        }))
        .unwrap()
    }

    #[test]
    fn a_balance_is_divided_by_a_million() {
        assert_eq!(scaled("141171137", "a balance").unwrap(), d("141.171137"));
        assert_eq!(scaled("0", "a balance").unwrap(), Decimal::ZERO);
        assert_eq!(scaled("1000000", "a balance").unwrap(), Decimal::ONE);
        let err = scaled("12abc", "a balance").unwrap_err();
        assert_eq!(err.class, Class::Decode);
    }

    #[test]
    fn an_unlimited_allowance_reads_as_the_largest_decimal() {
        let unlimited =
            "115792089237316195423570985008687907853269984665640564039457584007913129639935";

        assert_eq!(allowance(unlimited).unwrap(), Decimal::MAX);
        assert_eq!(allowance("999999").unwrap(), d("0.999999"));
        assert_eq!(allowance("12abc").unwrap_err().class, Class::Decode);
        assert_eq!(allowance("").unwrap_err().class, Class::Decode);
    }

    #[test]
    fn a_side_is_buy_or_sell_in_any_case() {
        assert_eq!(side_text("BUY").unwrap(), Side::Buy);
        assert_eq!(side_text("sell").unwrap(), Side::Sell);
        assert_eq!(side_text("HOLD").unwrap_err().class, Class::Decode);
    }

    #[test]
    fn an_asset_that_is_no_token_id_is_a_decode_error() {
        assert!(key_of("100").is_ok());
        assert_eq!(key_of("0x10").unwrap_err().class, Class::Decode);
        assert_eq!(key_of("").unwrap_err().class, Class::Decode);
    }

    #[test]
    fn a_listed_order_maps_its_status_and_keeps_the_raw_word() {
        let ids = Mutex::new(ClientIds::default());
        for (word, status) in [
            ("LIVE", OrderStatus::Open),
            ("MATCHED", OrderStatus::Filled),
            ("CANCELED", OrderStatus::Cancelled),
            ("CANCELED_MARKET_RESOLVED", OrderStatus::Cancelled),
            ("INVALID", OrderStatus::Open),
        ] {
            let order = open_order(&row(word, "GTC", "0"), &ids).unwrap();

            assert_eq!(order.status, status, "{word}");
            let ext = order.extensions.get::<OrderExt>().unwrap();
            assert_eq!(ext.status.as_deref(), Some(word));
        }
    }

    #[test]
    fn a_listed_order_carries_its_size_side_time_and_expiry() {
        let ids = Mutex::new(ClientIds::default());
        lock(&ids)
            .claim(OrderId::new("0xa"), ClientOrderId::new("mine"))
            .unwrap();

        let order = open_order(&row("LIVE", "GTD", "1900000000"), &ids).unwrap();

        assert_eq!(order.side, Side::Sell);
        assert_eq!(order.size, d("10"));
        assert_eq!(order.filled, d("4"));
        assert_eq!(order.price, d("0.52"));
        assert_eq!(order.created, Some(UnixMillis(1_792_000_000_000)));
        assert_eq!(order.time_in_force, Some(TimeInForce::GoodTilCancelled));
        assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine")));
        let ext = order.extensions.get::<OrderExt>().unwrap();
        assert_eq!(ext.expiration, Some(UnixMillis(1_900_000_000_000)));
        assert_eq!(ext.maker_address.as_deref(), Some("0xMaker"));
        assert_eq!(ext.trade_ids, ["t1", "t2"]);
    }

    #[test]
    fn the_order_type_is_the_time_in_force() {
        let ids = Mutex::new(ClientIds::default());
        for (word, expected) in [
            ("GTC", Some(TimeInForce::GoodTilCancelled)),
            ("GTD", Some(TimeInForce::GoodTilCancelled)),
            ("FAK", Some(TimeInForce::ImmediateOrCancel)),
            ("FOK", Some(TimeInForce::FillOrKill)),
            ("SOMETHING", None),
        ] {
            let order = open_order(&row("LIVE", word, "0"), &ids).unwrap();

            assert_eq!(order.time_in_force, expected, "{word}");
        }
    }

    #[test]
    fn a_listed_order_with_unreadable_text_is_a_decode_error() {
        let ids = Mutex::new(ClientIds::default());
        let mut bad = row("LIVE", "GTC", "0");
        bad.price = "cheap".to_owned();

        assert_eq!(open_order(&bad, &ids).unwrap_err().class, Class::Decode);
    }
}

#[cfg(test)]
mod place_tests {
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue::trading
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob
```

Expected: the library build fails with `cannot find function scaled`, `allowance`, `side_text`, `key_of` and `open_order` in this scope. Were the helpers present and the stubs still in place, each read-side integration test would fail on a stub's `... is not implemented yet` refusal.

- [ ] **Step 3: Implement**

Add the imports. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
use crate::clob::types::TickSize;
```

Replace with:

```rust
use crate::clob::types::TickSize;
use std::collections::HashSet;

use super::{decimal, decode};
use crate::clob::OpenOrder;
use crate::data::types::Position as DataPosition;
use crate::shared::keys::TokenId;
```

Add the helpers and mappers. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
#[cfg(test)]
mod read_tests {
```

Replace with:

```rust
/// The rows of a `/positions` page, the most it serves.
const POSITIONS_PAGE: u32 = 500;

/// The last offset `/positions` accepts.
const POSITIONS_LAST_OFFSET: u32 = 10_000;

/// The cursor `/data/orders` ends on.
const PAGES_END: &str = "LTE=";

/// What the account's collateral is called.
const COLLATERAL_ASSET: &str = "pUSD";

/// The collateral's fixed point: balances and allowances carry six decimals.
const COLLATERAL_SCALE: u32 = 1_000_000;

/// A decimal the CLOB sent as text, or a `Decode` error naming `what`.
pub(super) fn decimal_text(text: &str, what: &str) -> Result<Decimal, ClassifiedError> {
    text.parse()
        .map_err(|_| decode(&format!("{what} that is not a decimal")))
}

/// A side the CLOB sent as `BUY` or `SELL`.
pub(super) fn side_text(text: &str) -> Result<Side, ClassifiedError> {
    if text.eq_ignore_ascii_case("BUY") {
        Ok(Side::Buy)
    } else if text.eq_ignore_ascii_case("SELL") {
        Ok(Side::Sell)
    } else {
        Err(decode("a side that is neither BUY nor SELL"))
    }
}

/// The key of the outcome token `asset`, or a `Decode` error when the CLOB
/// sent an id no key can carry.
pub(super) fn key_of(asset: &str) -> Result<MarketKey, ClassifiedError> {
    let token = TokenId::new(asset).map_err(|err| ClassifiedError::new(Class::Decode, err))?;
    Ok(keys::clob(&token))
}

/// A collateral amount in the CLOB's six-decimal fixed point, as a decimal.
fn scaled(text: &str, what: &str) -> Result<Decimal, ClassifiedError> {
    decimal_text(text, what).map(|amount| amount / Decimal::from(COLLATERAL_SCALE))
}

/// An allowance. An unlimited approval is a 78-digit integer, larger than a
/// `Decimal` holds, and reads as `Decimal::MAX`.
fn allowance(text: &str) -> Result<Decimal, ClassifiedError> {
    scaled(text, "an allowance").or_else(|err| {
        if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
            Ok(Decimal::MAX)
        } else {
            Err(err)
        }
    })
}

/// An order the CLOB lists, as the trait's [`Order`].
fn open_order(row: &OpenOrder, ids: &Mutex<ClientIds>) -> Result<Order, ClassifiedError> {
    let status = match row.status.as_str() {
        "MATCHED" => OrderStatus::Filled,
        "CANCELED" | "CANCELED_MARKET_RESOLVED" => OrderStatus::Cancelled,
        // `LIVE`, and any word the CLOB adds, which the extension keeps.
        _ => OrderStatus::Open,
    };
    let id = OrderId::new(row.id.as_str());
    let mut order = Order::new(
        id.clone(),
        key_of(&row.asset_id)?,
        side_text(&row.side)?,
        decimal_text(&row.price, "a price")?,
        decimal_text(&row.original_size, "an original size")?,
        status,
    );
    order.filled = decimal_text(&row.size_matched, "a matched size")?;
    order.time_in_force = match row.order_type.as_str() {
        "GTC" | "GTD" => Some(TimeInForce::GoodTilCancelled),
        "FAK" => Some(TimeInForce::ImmediateOrCancel),
        "FOK" => Some(TimeInForce::FillOrKill),
        _ => None,
    };
    order.created = u64::try_from(row.created_at)
        .ok()
        .map(|seconds| UnixMillis(seconds.saturating_mul(1_000)));
    order.client_order_id = lock(ids).client_of(&id);
    order.extensions = Extensions::of(OrderExt {
        status: Some(row.status.clone()),
        condition_id: Some(row.market.clone()),
        outcome: Some(row.outcome.clone()),
        maker_address: Some(row.maker_address.clone()),
        // `0` is no expiry.
        expiration: row
            .expiration
            .parse::<u64>()
            .ok()
            .filter(|seconds| *seconds > 0)
            .map(|seconds| UnixMillis(seconds.saturating_mul(1_000))),
        neg_risk: None,
        trade_ids: row.associate_trades.clone(),
        transaction_hashes: Vec::new(),
    });
    Ok(order)
}

/// A Data API position, as the trait's [`Position`].
fn position(row: &DataPosition) -> Result<Position, ClassifiedError> {
    let size =
        decimal(row.size).ok_or_else(|| decode("a position size that is not a decimal"))?;
    let mut position = Position::new(key_of(&row.asset)?, size);
    position.average_price = decimal(row.avg_price);
    position.extensions = Extensions::of(PositionExt {
        condition_id: Some(row.condition_id.clone()),
        outcome: Some(row.outcome.clone()),
        outcome_index: Some(row.outcome_index),
        current_price: decimal(row.cur_price),
        current_value: decimal(row.current_value),
        initial_value: decimal(row.initial_value),
        cash_pnl: decimal(row.cash_pnl),
        realized_pnl: decimal(row.realized_pnl),
        redeemable: Some(row.redeemable),
        mergeable: Some(row.mergeable),
        negative_risk: Some(row.negative_risk),
        title: Some(row.title.clone()),
        end_date: row.end_date.clone(),
    });
    Ok(position)
}

#[cfg(test)]
mod read_tests {
```

The client-id lookup the mappers use belongs on the ledger. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
    /// The order holding `client`, when one does.
    pub(crate) fn holder(&self, client: &ClientOrderId) -> Option<OrderId> {
        self.by_client.get(client).cloned()
    }
```

Replace with:

```rust
    /// The order holding `client`, when one does.
    pub(crate) fn holder(&self, client: &ClientOrderId) -> Option<OrderId> {
        self.by_client.get(client).cloned()
    }

    /// The client order id `order` was placed with, when this client placed it
    /// with one.
    pub(crate) fn client_of(&self, order: &OrderId) -> Option<ClientOrderId> {
        self.by_order.get(order).map(|(client, _)| client.clone())
    }
```

Replace the five stubs with the methods. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current:

```rust
    // Temporary stub; Task 5.3 replaces it.
    async fn cancel(&self, _id: &OrderId) -> Result<(), ClassifiedError> {
        Err(refusal("cancel is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn cancel_all(&self, _key: Option<&MarketKey>) -> Result<Vec<OrderId>, ClassifiedError> {
        Err(refusal("cancel_all is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn open_orders(&self, _key: Option<&MarketKey>) -> Result<Vec<Order>, ClassifiedError> {
        Err(refusal("open_orders is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn positions(&self) -> Result<Vec<Position>, ClassifiedError> {
        Err(refusal("positions is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn balances(&self) -> Result<Vec<Balance>, ClassifiedError> {
        Err(refusal("balances is not implemented yet"))
    }
```

Replace with:

```rust
    async fn cancel(&self, id: &OrderId) -> Result<(), ClassifiedError> {
        let response = self.orders()?.cancel(id.as_str()).send().await?;
        if response
            .canceled
            .iter()
            .any(|canceled| canceled == id.as_str())
        {
            lock(&self.ids).remove(id);
            return Ok(());
        }
        match response.not_canceled.get(id.as_str()) {
            Some(reason) => Err(refusal(format!(
                "the CLOB did not cancel order {id}: {reason}"
            ))),
            None => Err(refusal(format!("the CLOB did not answer for order {id}"))),
        }
    }

    async fn cancel_all(&self, key: Option<&MarketKey>) -> Result<Vec<OrderId>, ClassifiedError> {
        let orders = self.orders()?;
        let response = match key {
            None => orders.cancel_all().await?,
            Some(key) => {
                // The CLOB cancels by market and token together.
                let token = keys::token_id(key)?;
                let market = self
                    .markets()
                    .market_by_token(token.as_str())
                    .send()
                    .await?;
                orders
                    .cancel_market(market.condition_id, token.as_str())
                    .await?
            }
        };
        let cancelled: Vec<OrderId> = response
            .canceled
            .iter()
            .map(|id| OrderId::new(id.as_str()))
            .collect();
        {
            let mut ids = lock(&self.ids);
            for id in &cancelled {
                ids.remove(id);
            }
        }
        Ok(cancelled)
    }

    async fn open_orders(&self, key: Option<&MarketKey>) -> Result<Vec<Order>, ClassifiedError> {
        let orders = self.orders()?;
        let token = key.map(keys::token_id).transpose()?;
        let mut cursor: Option<String> = None;
        let mut followed = HashSet::new();
        let mut open = Vec::new();
        loop {
            let mut list = orders.list();
            if let Some(token) = &token {
                list = list.asset_id(token.as_str());
            }
            if let Some(cursor) = &cursor {
                list = list.next_cursor(cursor.as_str());
            }
            let page = list.send().await?;
            for row in &page.data {
                open.push(open_order(row, &self.ids)?);
            }
            match page.next_cursor {
                Some(next) if !next.is_empty() && next != PAGES_END => {
                    if !followed.insert(next.clone()) {
                        return Err(decode("a page cursor it had already followed"));
                    }
                    cursor = Some(next);
                }
                _ => return Ok(open),
            }
        }
    }

    async fn positions(&self) -> Result<Vec<Position>, ClassifiedError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to read positions"))?;
        // The address orders are placed from: the maker of the client's own
        // signature type (the builder's override, else the account target's),
        // with no per-order funder, as `place` resolves it.
        let maker = self
            .resolve_maker_address(None, self.signature_type, account)
            .await?;
        let user = self.data.user(maker.to_string());
        let mut positions = Vec::new();
        let mut offset = 0;
        loop {
            // No dust filter: the default threshold of one share hides small
            // positions.
            let rows = user
                .list_positions()
                .size_threshold(0.0)
                .limit(POSITIONS_PAGE)
                .offset(offset)
                .send()
                .await?;
            for row in &rows {
                positions.push(position(row)?);
            }
            if rows.len() < POSITIONS_PAGE as usize {
                return Ok(positions);
            }
            if offset >= POSITIONS_LAST_OFFSET {
                return Err(refusal(
                    "the wallet holds more positions than /positions can list",
                ));
            }
            offset += POSITIONS_PAGE;
        }
    }

    async fn balances(&self) -> Result<Vec<Balance>, ClassifiedError> {
        let response = self.account_api()?.usdc_balance().send().await?;
        let mut allowances = BTreeMap::new();
        for (spender, amount) in &response.allowances {
            allowances.insert(spender.clone(), allowance(amount)?);
        }
        // Outcome tokens are positions, not balances: the collateral is the
        // account's one balance. The CLOB reports no reservation by open
        // orders, so nothing is `available`.
        let mut collateral = Balance::new(COLLATERAL_ASSET);
        collateral.total = Some(scaled(&response.balance, "a balance")?);
        collateral.extensions = Extensions::of(BalanceExt { allowances });
        Ok(vec![collateral])
    }
```

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue::trading
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob
```

Expected: the library run prints `test result: ok. 22 passed; 0 failed`, and the integration run prints `test result: ok. 28 passed; 0 failed` (the two `gamma` tests of a proxy client are not built here).

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws,data --all-targets -- -D warnings
git commit -m "feat(clob): Trading cancel, open orders, positions and balances (Story 7.4)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-polymarket/src/clob/venue/trading.rs polyoxide-polymarket/tests/trading_clob.rs
```

### Task 5.4: `events()` on the supervised user socket

Replaces Task 5.2's `events()` stub with the adapter from Story 3's `UserEvent` to `TradingEvent`, and gives `Clob` the user socket it opens (`ClobBuilder::user_socket`). Each call to `events()` opens its own socket when its stream is first polled: it resolves the maker address, starts the socket on every market of the account (`None`) and yields the leading `Resync`; then every `Reconnected` and every `FAILED` trade is a `Resync`, and a `MATCHED` trade is one `Fill` per order of the account's in it.

**Files:**
- Create: `polyoxide-polymarket/src/clob/venue/fills.rs`
- Modify: `polyoxide-polymarket/src/clob/venue.rs`
- Modify: `polyoxide-polymarket/src/clob/venue/trading.rs`
- Modify: `polyoxide-polymarket/src/clob/client.rs`
- Modify: `CLAUDE.md:209`
- Test: `polyoxide-polymarket/tests/trading_clob.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-polymarket/src/clob/venue/fills.rs` with its unit tests first. The module under test does not exist yet, so Step 3 adds its code above the tests:

```rust
//! The events adapter: `Trading::events` for the CLOB, from the supervised
//! user socket's `UserEvent`s to the trait's `TradingEvent`s.

#[cfg(test)]
mod tests {
    use polyoxide_venue::{Class, ClientOrderId};
    use serde_json::{json, Value};

    use super::*;

    const KEY: &str = "api-key";
    const MAKER: &str = "0xabc0000000000000000000000000000000000001";
    const OTHER: &str = "0xdead000000000000000000000000000000000002";

    fn ids() -> Mutex<ClientIds> {
        Mutex::new(ClientIds::default())
    }

    fn d(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn trade(status: &str, owner: &str, makers: Vec<Value>) -> TradeMessage {
        serde_json::from_value(json!({
            "event_type": "trade",
            "id": "t1",
            "asset_id": "100",
            "market": "0xcondition",
            "outcome": "Yes",
            "price": "0.52",
            "size": "4",
            "side": "BUY",
            "status": status,
            "taker_order_id": "taker-1",
            "maker_orders": makers,
            "owner": owner,
            "transaction_hash": "0xhash",
            "timestamp": "1792000000123"
        }))
        .unwrap()
    }

    fn maker(order: &str, address: &str, asset: &str, price: &str) -> Value {
        json!({
            "order_id": order,
            "maker_address": address,
            "matched_amount": "4",
            "fee_rate_bps": "30",
            "asset_id": asset,
            "price": price
        })
    }

    fn order_message(kind: &str, matched: &str, timestamp: &str) -> OrderMessage {
        serde_json::from_value(json!({
            "event_type": "order",
            "id": "0xo1",
            "asset_id": "100",
            "market": "0xcondition",
            "outcome": "Yes",
            "price": "0.52",
            "side": "BUY",
            "original_size": "10",
            "size_matched": matched,
            "type": kind,
            "order_owner": "api-key",
            "timestamp": timestamp
        }))
        .unwrap()
    }

    fn run(trade: &TradeMessage, ids: &Mutex<ClientIds>) -> VecDeque<Item> {
        let mut out = VecDeque::new();
        on_trade(trade, KEY, MAKER, ids, &mut out);
        out
    }

    fn fill_of(item: &Item) -> &Fill {
        match item {
            Ok(TradingEvent::Fill(fill)) => fill,
            other => panic!("expected a fill, got {other:?}"),
        }
    }

    fn translated(item: Result<UserEvent, ClobWsError>) -> VecDeque<Item> {
        let mut out = VecDeque::new();
        translate(item, KEY, MAKER, &ids(), &mut out);
        out
    }

    #[test]
    fn a_taker_and_a_maker_of_the_account_each_get_a_fill() {
        let trade = trade(
            "MATCHED",
            KEY,
            vec![
                maker("m1", OTHER, "100", "0.52"),
                // The account's address in another case, on the complementary
                // token: a mint or merge match.
                maker("m2", "0xABC0000000000000000000000000000000000001", "101", "0.48"),
            ],
        );

        let out = run(&trade, &ids());

        assert_eq!(out.len(), 2);
        let taker = fill_of(&out[0]);
        assert_eq!(taker.id.as_str(), "t1:taker-1");
        assert_eq!(taker.order_id.as_str(), "taker-1");
        assert_eq!(taker.trade_id, Some(TradeId::new("t1")));
        assert_eq!(taker.key, key_of("100").unwrap());
        assert_eq!(taker.side, Side::Buy);
        assert_eq!(taker.price, d("0.52"));
        assert_eq!(taker.size, d("4"));
        assert_eq!(taker.liquidity, Some(Liquidity::Taker));
        assert_eq!(taker.time, UnixMillis(1_792_000_000_123));
        assert_eq!(taker.fee, None);
        assert_eq!(taker.notional, None);
        let ext = taker.extensions.get::<FillExt>().unwrap();
        assert_eq!(ext.transaction_hash.as_deref(), Some("0xhash"));
        assert_eq!(ext.fee_rate_bps, None);

        let maker = fill_of(&out[1]);
        assert_eq!(maker.id.as_str(), "t1:m2");
        assert_eq!(maker.order_id.as_str(), "m2");
        assert_eq!(maker.key, key_of("101").unwrap());
        // The complementary token's order takes the trade's own side.
        assert_eq!(maker.side, Side::Buy);
        assert_eq!(maker.price, d("0.48"));
        assert_eq!(maker.liquidity, Some(Liquidity::Maker));
        let ext = maker.extensions.get::<FillExt>().unwrap();
        assert_eq!(ext.fee_rate_bps, Some(d("30")));
        assert_eq!(ext.taker_order_id.as_deref(), Some("taker-1"));
    }

    #[test]
    fn a_maker_on_the_trades_own_token_takes_the_opposite_side() {
        let trade = trade("MATCHED", "someone-else", vec![maker("m1", MAKER, "100", "0.52")]);

        let out = run(&trade, &ids());

        assert_eq!(out.len(), 1);
        let fill = fill_of(&out[0]);
        assert_eq!(fill.side, Side::Sell);
        assert_eq!(fill.liquidity, Some(Liquidity::Maker));
    }

    #[test]
    fn a_trade_naming_none_of_the_accounts_orders_is_a_decode_error() {
        let trade = trade("MATCHED", "someone-else", vec![maker("m1", OTHER, "100", "0.52")]);

        let out = run(&trade, &ids());

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].as_ref().unwrap_err().class, Class::Decode);
    }

    #[test]
    fn a_trade_with_unreadable_text_is_one_decode_error_and_no_fill() {
        let mut bad = trade("MATCHED", KEY, vec![maker("m2", MAKER, "100", "0.52")]);
        bad.timestamp = "soon".to_owned();

        let out = run(&bad, &ids());

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].as_ref().unwrap_err().class, Class::Decode);
    }

    #[test]
    fn only_a_matched_trade_is_a_fill_and_a_failed_one_asks_for_a_resync() {
        for (status, expected) in [("MINED", 0), ("CONFIRMED", 0), ("RETRYING", 0)] {
            let out = run(&trade(status, KEY, vec![]), &ids());
            assert_eq!(out.len(), expected, "{status}");
        }
        let matched = run(&trade("MATCHED", KEY, vec![]), &ids());
        assert_eq!(matched.len(), 1);
        assert!(matches!(matched[0], Ok(TradingEvent::Fill(_))));

        let failed = run(&trade("FAILED", KEY, vec![]), &ids());

        assert_eq!(failed.len(), 1);
        assert!(matches!(failed[0], Ok(TradingEvent::Resync)));
    }

    #[test]
    fn a_fill_carries_the_client_order_id_the_ledger_holds() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("taker-1"), ClientOrderId::new("mine"))
            .unwrap();

        let out = run(&trade("MATCHED", KEY, vec![]), &ids);

        assert_eq!(
            fill_of(&out[0]).client_order_id,
            Some(ClientOrderId::new("mine"))
        );
    }

    #[test]
    fn a_placement_is_an_open_order_created_at_the_messages_time() {
        let order = ack(&order_message("PLACEMENT", "0", "1792000000123"), &ids()).unwrap();

        assert_eq!(order.id.as_str(), "0xo1");
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.created, Some(UnixMillis(1_792_000_000_123)));
        assert_eq!(order.size, d("10"));
        assert_eq!(order.filled, Decimal::ZERO);
        assert_eq!(order.side, Side::Buy);
        // The message does not carry the time in force.
        assert_eq!(order.time_in_force, None);
    }

    #[test]
    fn an_update_that_reaches_the_size_is_filled_and_drops_the_ledger_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        let order = ack(&order_message("UPDATE", "10", "1792000000999"), &ids).unwrap();

        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.filled, d("10"));
        assert_eq!(order.created, None);
        assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine")));
        assert_eq!(lock(&ids).holder(&ClientOrderId::new("mine")), None);
    }

    #[test]
    fn a_cancellation_is_cancelled_and_drops_the_ledger_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        let order = ack(&order_message("CANCELLATION", "0", "1792000000999"), &ids).unwrap();

        assert_eq!(order.status, OrderStatus::Cancelled);
        assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine")));
        assert_eq!(lock(&ids).holder(&ClientOrderId::new("mine")), None);
    }

    #[test]
    fn a_partial_update_stays_open_and_keeps_its_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        // Only a placement's timestamp is read, so this one is not an error.
        let order = ack(&order_message("UPDATE", "4", "not read"), &ids).unwrap();

        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.filled, d("4"));
        assert_eq!(
            lock(&ids).holder(&ClientOrderId::new("mine")),
            Some(OrderId::new("0xo1"))
        );
    }

    #[test]
    fn a_placement_with_an_unreadable_time_is_a_decode_error_and_keeps_its_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        let err = ack(&order_message("PLACEMENT", "0", "soon"), &ids).unwrap_err();

        assert_eq!(err.class, Class::Decode);
        assert!(lock(&ids).holder(&ClientOrderId::new("mine")).is_some());
    }

    #[test]
    fn a_reconnect_is_a_resync_and_a_disconnect_is_silent_unless_it_carries_an_error() {
        let reconnected = translated(Ok(UserEvent::Reconnected));
        assert_eq!(reconnected.len(), 1);
        assert!(matches!(reconnected[0], Ok(TradingEvent::Resync)));

        let rotated = translated(Ok(UserEvent::Disconnected {
            reason: DisconnectReason::Rotation,
        }));
        assert!(rotated.is_empty());

        let failed = translated(Ok(UserEvent::Disconnected {
            reason: DisconnectReason::Error(ClobWsError::ConnectionClosed),
        }));
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].as_ref().unwrap_err().class, Class::Network);

        let refused = translated(Err(ClobWsError::Authentication(
            "authentication failed".to_owned(),
        )));
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].as_ref().unwrap_err().class, Class::Unauthorized);
    }

    #[test]
    fn a_message_goes_to_the_adapter_of_its_kind() {
        let order = translated(Ok(UserEvent::Message(UserMessage::Order(order_message(
            "PLACEMENT",
            "0",
            "1792000000123",
        )))));
        assert!(matches!(order[0], Ok(TradingEvent::Ack(_))));

        let trade = translated(Ok(UserEvent::Message(UserMessage::Trade(trade(
            "MATCHED",
            KEY,
            vec![],
        )))));
        assert!(matches!(trade[0], Ok(TradingEvent::Fill(_))));
    }
}
```

In `polyoxide-polymarket/tests/trading_clob.rs`, add imports. Current:

```rust
use serde_json::{json, Value};
```

Replace with:

```rust
use futures_util::StreamExt;
use polyoxide_polymarket::clob::ws::UserWsBuilder;
use polyoxide_venue::{DynTrading, Fill, Liquidity, TradingEvent};
use polyoxide_ws::test_server::{next_item, Script, ScriptedServer};
use polyoxide_ws::tungstenite::Message;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
```

Add the events tests before the read-side section of `polyoxide-polymarket/tests/trading_clob.rs`. Current:

```rust
// --- cancel, orders, positions and balances ---
```

Replace with:

```rust
// --- events ---

const COMPLEMENT: &str = "101";

/// The account's maker address: Hardhat account 0 is an EOA.
fn ours() -> String {
    account().address().to_string()
}

/// A client whose user socket is `ws`, quick to ping and to reconnect.
fn client_on(server: &ServerGuard, ws: &ScriptedServer) -> Clob {
    let socket = UserWsBuilder::new()
        .url(ws.url.clone())
        .unwrap()
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_secs(2))
        .backoff(Duration::from_millis(10), Duration::from_millis(20));
    ClobBuilder::new()
        .base_url(server.url())
        .with_account(account())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .user_socket(socket)
        .build()
        .unwrap()
}

/// Answers the client's text `PING` with a `PONG`, as the CLOB does. An
/// integration test cannot see the crate's `#[cfg(test)]` answerer.
fn pong(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send + 'static {
    |text: &str| {
        if text == "PING" {
            vec![Message::Text("PONG".into())]
        } else {
            Vec::new()
        }
    }
}

async fn user_socket(scripts: Vec<Script>) -> ScriptedServer {
    ScriptedServer::start_answering(scripts, pong).await
}

fn order_frame(id: &str, kind: &str, matched: &str, timestamp: &str) -> String {
    json!({
        "event_type": "order",
        "id": id,
        "asset_id": TOKEN,
        "market": MARKET,
        "outcome": "Yes",
        "price": "0.52",
        "side": "BUY",
        "original_size": "10",
        "size_matched": matched,
        "type": kind,
        "order_owner": "test-key",
        "timestamp": timestamp
    })
    .to_string()
}

fn maker_entry(order: &str, address: &str, asset: &str, price: &str) -> Value {
    json!({
        "order_id": order,
        "maker_address": address,
        "matched_amount": "4",
        "fee_rate_bps": "0",
        "asset_id": asset,
        "price": price
    })
}

fn trade_frame(id: &str, status: &str, owner: &str, taker_order: &str, makers: Vec<Value>) -> String {
    json!({
        "event_type": "trade",
        "id": id,
        "asset_id": TOKEN,
        "market": MARKET,
        "outcome": "Yes",
        "price": "0.52",
        "size": "4",
        "side": "BUY",
        "status": status,
        "taker_order_id": taker_order,
        "maker_orders": makers,
        "owner": owner,
        "transaction_hash": "0xhash",
        "timestamp": "1792000000123"
    })
    .to_string()
}

/// A trade the account took, alone on the book.
fn taker_trade(id: &str) -> String {
    trade_frame(id, "MATCHED", "test-key", "0xtaker", vec![])
}

fn fill_of(event: TradingEvent) -> Fill {
    match event {
        TradingEvent::Fill(fill) => fill,
        other => panic!("expected a fill, got {other:?}"),
    }
}

#[tokio::test]
async fn the_stream_leads_with_a_resync_and_acks_with_the_client_order_id() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let placement = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let cancellation = order_frame("0xorder1", "CANCELLATION", "0", "1792000009999");
    let ws = user_socket(vec![
        Script::frames(&[placement.as_str(), cancellation.as_str()]).after_first_frame(),
    ])
    .await;
    let clob = client_on(&server, &ws);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));
    // The order is placed before the stream is polled, so the ledger holds
    // its client id when the acks arrive.
    clob.place(request.clone()).await.unwrap();

    let mut events = clob.events();
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let TradingEvent::Ack(placed) = next_item(&mut events).await.unwrap() else {
        panic!("expected the placement's ack");
    };
    let TradingEvent::Ack(cancelled) = next_item(&mut events).await.unwrap() else {
        panic!("expected the cancellation's ack");
    };

    assert_eq!(placed.id.as_str(), "0xorder1");
    assert_eq!(placed.status, OrderStatus::Open);
    assert_eq!(placed.key, key());
    assert_eq!(placed.side, Side::Buy);
    assert_eq!(placed.price, d("0.52"));
    assert_eq!(placed.size, d("10"));
    assert_eq!(placed.created, Some(UnixMillis(1_792_000_000_123)));
    assert_eq!(placed.client_order_id, Some(ClientOrderId::new("mine-1")));
    assert_eq!(cancelled.status, OrderStatus::Cancelled);
    assert_eq!(cancelled.client_order_id, Some(ClientOrderId::new("mine-1")));
    // The socket was started with the account's credentials and on every
    // market: the subscribe frame carries the key and no market filter.
    ws.wait_for("the subscription", |ws| !ws.first_frames().is_empty())
        .await;
    let subscription = ws.first_frames()[0].clone();
    assert!(subscription.contains("test-key"), "{subscription}");
    assert!(!subscription.contains("\"markets\""), "{subscription}");
    // The cancelled ack dropped the ledger entry, so the id is free again.
    clob.place(request).await.unwrap();
    post.assert_async().await;
}

#[tokio::test]
async fn a_taker_fill_and_two_maker_fills_name_the_orders_of_the_account() {
    let server = Server::new_async().await;
    let one = trade_frame(
        "t1",
        "MATCHED",
        "test-key",
        "0xtaker",
        vec![
            maker_entry("0xm1", "0xdead000000000000000000000000000000000002", TOKEN, "0.52"),
            maker_entry("0xm2", &ours().to_uppercase().replacen("0X", "0x", 1), COMPLEMENT, "0.48"),
        ],
    );
    // Someone else took; the account's order rests on the trade's own token.
    let two = trade_frame(
        "t2",
        "MATCHED",
        "other-key",
        "0xother",
        vec![maker_entry("0xm3", &ours().to_lowercase(), TOKEN, "0.52")],
    );
    let ws = user_socket(vec![
        Script::frames(&[one.as_str(), two.as_str()]).after_first_frame(),
    ])
    .await;
    let mut events = client_on(&server, &ws).events();

    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let taker = fill_of(next_item(&mut events).await.unwrap());
    let complement = fill_of(next_item(&mut events).await.unwrap());
    let resting = fill_of(next_item(&mut events).await.unwrap());

    assert_eq!(taker.id.as_str(), "t1:0xtaker");
    assert_eq!(taker.liquidity, Some(Liquidity::Taker));
    assert_eq!(taker.side, Side::Buy);
    assert_eq!(taker.key, key());
    assert_eq!(taker.price, d("0.52"));
    assert_eq!(taker.size, d("4"));
    assert_eq!(taker.time, UnixMillis(1_792_000_000_123));
    assert_eq!(taker.trade_id.as_ref().map(|id| id.as_str()), Some("t1"));
    assert_eq!(complement.id.as_str(), "t1:0xm2");
    assert_eq!(complement.liquidity, Some(Liquidity::Maker));
    assert_eq!(complement.key, keys::clob(&TokenId::new(COMPLEMENT).unwrap()));
    assert_eq!(complement.side, Side::Buy);
    assert_eq!(complement.price, d("0.48"));
    assert_eq!(resting.id.as_str(), "t2:0xm3");
    assert_eq!(resting.liquidity, Some(Liquidity::Maker));
    assert_eq!(resting.side, Side::Sell);
}

#[tokio::test]
async fn a_failed_trade_is_a_resync_and_a_mined_one_is_silent() {
    let server = Server::new_async().await;
    let mined = trade_frame("t1", "MINED", "test-key", "0xtaker", vec![]);
    let failed = trade_frame("t1", "FAILED", "test-key", "0xtaker", vec![]);
    let ack = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let ws = user_socket(vec![
        Script::frames(&[mined.as_str(), failed.as_str(), ack.as_str()]).after_first_frame(),
    ])
    .await;
    let mut events = client_on(&server, &ws).events();

    // The leading one, then the failed trade's; the mined trade yields nothing.
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    assert!(matches!(
        next_item(&mut events).await.unwrap(),
        TradingEvent::Ack(_)
    ));
}

#[tokio::test]
async fn a_trade_naming_none_of_the_accounts_orders_is_an_error_item_and_the_stream_goes_on() {
    let server = Server::new_async().await;
    let stranger = trade_frame(
        "t1",
        "MATCHED",
        "other-key",
        "0xother",
        vec![maker_entry("0xm1", "0xdead000000000000000000000000000000000002", TOKEN, "0.52")],
    );
    let ack = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let ws = user_socket(vec![
        Script::frames(&[stranger.as_str(), ack.as_str()]).after_first_frame(),
    ])
    .await;
    let mut events = client_on(&server, &ws).events();

    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let err = next_item(&mut events).await.unwrap_err();
    assert_eq!(err.class, Class::Decode, "{err}");
    assert!(matches!(
        next_item(&mut events).await.unwrap(),
        TradingEvent::Ack(_)
    ));
}

#[tokio::test]
async fn every_fill_arrives_in_order_across_a_slow_consumer_and_a_server_cut() {
    let server = Server::new_async().await;
    let before: Vec<String> = (1..=3).map(|n| taker_trade(&format!("t{n}"))).collect();
    let before: Vec<&str> = before.iter().map(String::as_str).collect();
    let after = taker_trade("t4");
    let ws = user_socket(vec![
        // The first connection pushes three fills and is cut with no close
        // frame; the next one pushes the fourth.
        Script::frames(&before).after_first_frame().then_cut(),
        Script::frames(&[after.as_str()]).after_first_frame(),
    ])
    .await;
    let mut events = client_on(&server, &ws).events();

    let mut seen = Vec::new();
    let mut fills = 0;
    while fills < 4 {
        // The consumer is slower than the server; the socket's queue is
        // unbounded, so nothing is lost.
        tokio::time::sleep(Duration::from_millis(40)).await;
        match next_item(&mut events).await {
            Ok(TradingEvent::Fill(fill)) => {
                fills += 1;
                seen.push(fill.id.to_string());
            }
            Ok(TradingEvent::Resync) => seen.push("resync".to_owned()),
            Ok(other) => panic!("unexpected {other:?}"),
            // The cut is reported as an error item first.
            Err(_) => {}
        }
    }

    assert_eq!(
        seen,
        [
            "resync",
            "t1:0xtaker",
            "t2:0xtaker",
            "t3:0xtaker",
            "resync",
            "t4:0xtaker"
        ]
    );
}

#[tokio::test]
async fn events_without_an_account_yield_its_error_and_end() {
    let mut events = Clob::public().events();

    let err = events.next().await.unwrap().unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(err.to_string().contains("Account required"), "{err}");
    assert!(events.next().await.is_none());
}

/// The stream follows the maker address `place` trades from. A proxy client
/// without `gamma` cannot find its proxy wallet, so the stream's only item is
/// the native refusal and it opens no socket (the default socket is the CLOB's
/// live host, which a test must never reach); a stream that resolved the
/// account target's EOA would go on to connect.
#[cfg(not(feature = "gamma"))]
#[tokio::test]
async fn a_proxy_client_without_gamma_yields_the_refusal_and_ends() {
    let server = Server::new_async().await;
    let mut events = proxy_client(&server).events();

    let err = events.next().await.unwrap().unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(err.to_string().contains("requires the `gamma` feature"), "{err}");
    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn a_clob_runs_behind_an_arc_of_dyn_trading_and_its_stream_outlives_the_arc() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 1).await;
    let placement = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let ws = user_socket(vec![Script::frames(&[placement.as_str()]).after_first_frame()]).await;
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(client_on(&server, &ws));

    let order = venue
        .place(with_options(buy("0.52", "10"), declared(TickSize::Hundredth)))
        .await
        .unwrap();
    let mut events = venue.events();
    drop(venue);

    assert_eq!(order.id.as_str(), "0xorder1");
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let TradingEvent::Ack(acked) = next_item(&mut events).await.unwrap() else {
        panic!("expected the placement's ack");
    };
    assert_eq!(acked.id, order.id);
    post.assert_async().await;
}

// --- cancel, orders, positions and balances ---
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob
```

Expected: the library build fails with `cannot find type Item`, `cannot find function on_trade`, `translate` and `ack` in this scope (`fills.rs` holds only its tests, and `venue.rs` does not yet declare the module); the integration build fails with `no method named user_socket found for struct ClobBuilder`.

- [ ] **Step 3: Implement**

Give the client its user socket. In `polyoxide-polymarket/src/clob/client.rs`, Current:

```rust
    /// The client order ids of the orders this client placed, which the CLOB
    /// stores nowhere. Every clone shares it, and so does every events stream.
    #[cfg(all(feature = "clob-ws", feature = "data"))]
    pub(crate) ids: std::sync::Arc<std::sync::Mutex<crate::clob::venue::ClientIds>>,
}
```

Replace with:

```rust
    /// The client order ids of the orders this client placed, which the CLOB
    /// stores nowhere. Every clone shares it, and so does every events stream.
    #[cfg(all(feature = "clob-ws", feature = "data"))]
    pub(crate) ids: std::sync::Arc<std::sync::Mutex<crate::clob::venue::ClientIds>>,
    /// The user socket each `Trading::events` stream opens: the CLOB's own
    /// host and the default timing, unless `ClobBuilder::user_socket` set one.
    // Only `Trading::events` reads it, which `data` also builds.
    #[cfg(feature = "clob-ws")]
    #[cfg_attr(not(feature = "data"), allow(dead_code))]
    pub(crate) user_socket: crate::clob::ws::UserWsBuilder,
}
```

Current:

```rust
    #[cfg(feature = "data")]
    data: Option<DataApi>,
}
```

Replace with:

```rust
    #[cfg(feature = "data")]
    data: Option<DataApi>,
    #[cfg(feature = "clob-ws")]
    user_socket: crate::clob::ws::UserWsBuilder,
}
```

Current:

```rust
            #[cfg(feature = "data")]
            data: None,
        }
```

Replace with:

```rust
            #[cfg(feature = "data")]
            data: None,
            #[cfg(feature = "clob-ws")]
            user_socket: crate::clob::ws::UserWsBuilder::new(),
        }
```

Current:

```rust
    #[cfg(feature = "data")]
    pub fn data(mut self, data: DataApi) -> Self {
        self.data = Some(data);
        self
    }
```

Replace with:

```rust
    #[cfg(feature = "data")]
    pub fn data(mut self, data: DataApi) -> Self {
        self.data = Some(data);
        self
    }

    /// Set the user socket each `Trading::events` stream opens: its URL and
    /// its timing. The default is `UserWsBuilder::new()`, the CLOB's own host.
    ///
    /// The builder holds no credential: a stream builds them from the account
    /// when it opens its socket.
    #[cfg(feature = "clob-ws")]
    pub fn user_socket(mut self, socket: crate::clob::ws::UserWsBuilder) -> Self {
        self.user_socket = socket;
        self
    }
```

Current:

```rust
            #[cfg(all(feature = "clob-ws", feature = "data"))]
            ids: Default::default(),
        })
```

Replace with:

```rust
            #[cfg(all(feature = "clob-ws", feature = "data"))]
            ids: Default::default(),
            #[cfg(feature = "clob-ws")]
            user_socket: self.user_socket,
        })
```

List the new knob among clob's own in CLAUDE.md (AD-21). In the "**Builder pattern**" paragraph (`CLAUDE.md:209`), Current:

```markdown
`data` (built only with the `data` feature, for the `Trades` the CLOB reads through the Data API; `build` makes one with the builder's timeout and pool size when none is given, and the unified `Polymarket` client hands clob its own) and `with_account`, relay's
```

Replace with:

```markdown
`data` (built only with the `data` feature, for the `Trades` the CLOB reads through the Data API; `build` makes one with the builder's timeout and pool size when none is given, and the unified `Polymarket` client hands clob its own), `user_socket` (built only with the `clob-ws` feature: the URL and timing of the supervised user socket each `Trading::events` stream opens) and `with_account`, relay's
```

Declare the module. In `polyoxide-polymarket/src/clob/venue.rs`, Current:

```rust
#[cfg(all(feature = "clob-ws", feature = "data"))]
mod trading;
```

Replace with:

```rust
#[cfg(all(feature = "clob-ws", feature = "data"))]
mod fills;

#[cfg(all(feature = "clob-ws", feature = "data"))]
mod trading;
```

Point `events()` at the adapter. In `polyoxide-polymarket/src/clob/venue/trading.rs`, Current (if `cargo fmt` wrapped the stub differently, match it by its `// Temporary stub; Task 5.4 replaces it.` comment line):

```rust
    // Temporary stub; Task 5.4 replaces it.
    fn events(&self) -> TradingEvents {
        TradingEvents::new(futures_util::stream::empty::<
            Result<polyoxide_venue::TradingEvent, ClassifiedError>,
        >())
    }
```

Replace with:

```rust
    fn events(&self) -> TradingEvents {
        super::fills::events(self)
    }
```

Write the adapter. In `polyoxide-polymarket/src/clob/venue/fills.rs`, Current:

```rust
//! The events adapter: `Trading::events` for the CLOB, from the supervised
//! user socket's `UserEvent`s to the trait's `TradingEvent`s.

#[cfg(test)]
mod tests {
```

Replace with:

```rust
//! The events adapter: `Trading::events` for the CLOB, from the supervised
//! user socket's `UserEvent`s to the trait's `TradingEvent`s.
//!
//! A stream opens its own socket the first time it is polled. It resolves the
//! maker address the client trades from (its signature type's, as `place` and
//! `positions` do: a client built for a proxy wallet follows that wallet's
//! fills), starts the user socket on every market of the
//! account (`None`) with the account's credentials, and yields a leading
//! `Resync` once that first connect attempt has ended, whether the socket went
//! live or began in an outage, so a consumer reconciles before it trusts the
//! stream. It then maps each socket event in order:
//!
//! - an order message is an ack, with the client order id the ledger holds;
//! - a `MATCHED` trade is one fill per order of the account's in it, the
//!   taker's first (the trade's `owner` is the taker's API key) and then each
//!   maker entry at the account's maker address, ignoring ASCII case;
//! - a `FAILED` trade voids a fill already reported, and the trait has no
//!   event that takes a fill back, so it is a `Resync` and the consumer
//!   reconciles; `MINED`,
//!   `CONFIRMED` and `RETRYING` yield nothing, so a trade is reported once;
//! - a reconnect is a `Resync`, and a disconnect that carries an error yields
//!   that error;
//! - text that does not parse is an error item and its message is skipped.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Mutex;

use futures_util::StreamExt;
use polyoxide_venue::{
    ClassifiedError, Extensions, Fill, FillId, Liquidity, Order, OrderId, OrderStatus, Side,
    TradeId, TradingEvent, TradingEvents, UnixMillis,
};
use rust_decimal::Decimal;

use super::decode;
use super::trading::{decimal_text, key_of, lock, side_text, ClientIds, FillExt, OrderExt};
use crate::clob::error::ClobError;
use crate::clob::ws::{
    ApiCredentials, ClobWsError, DisconnectReason, OrderEventType, OrderMessage,
    SupervisedUserWs, TradeMessage, TradeStatus, UserEvent, UserMessage,
};
use crate::clob::Clob;

type Item = Result<TradingEvent, ClassifiedError>;

/// The stream `Trading::events` returns for `clob`.
pub(super) fn events(clob: &Clob) -> TradingEvents {
    TradingEvents::new(futures_util::stream::unfold(
        State {
            clob: clob.clone(),
            phase: Phase::Start,
            pending: VecDeque::new(),
        },
        step,
    ))
}

struct State {
    clob: Clob,
    phase: Phase,
    /// Items made from one socket event that the stream has not yet yielded.
    pending: VecDeque<Item>,
}

enum Phase {
    /// Not yet polled: the socket is not open.
    Start,
    Live(Live),
    /// The socket ended, or could not be opened.
    Done,
}

struct Live {
    socket: Pin<Box<SupervisedUserWs>>,
    /// The account's API key, which a trade's `owner` is when it took.
    api_key: String,
    /// The account's maker address, which a maker entry names when it rested.
    maker: String,
}

async fn step(mut state: State) -> Option<(Item, State)> {
    loop {
        if let Some(item) = state.pending.pop_front() {
            return Some((item, state));
        }
        match std::mem::replace(&mut state.phase, Phase::Done) {
            Phase::Done => return None,
            Phase::Start => match open(&state.clob).await {
                Ok(live) => {
                    state.pending.push_back(Ok(TradingEvent::Resync));
                    state.phase = Phase::Live(live);
                }
                // The only item, then the stream ends.
                Err(err) => state.pending.push_back(Err(err)),
            },
            Phase::Live(mut live) => {
                // The socket ended, as after an error recovery stopped on.
                let Some(item) = live.socket.next().await else {
                    continue;
                };
                translate(item, &live.api_key, &live.maker, &state.clob.ids, &mut state.pending);
                state.phase = Phase::Live(live);
            }
        }
    }
}

/// Resolves the maker address and starts the socket on every market.
async fn open(clob: &Clob) -> Result<Live, ClassifiedError> {
    let account = clob
        .account
        .as_ref()
        .ok_or_else(|| ClobError::validation("Account required for the events stream"))?;
    // The address `place` trades from: the client's own signature type's.
    let maker = clob
        .resolve_maker_address(None, clob.signature_type, account)
        .await?;
    let credentials = account.credentials();
    let socket = clob
        .user_socket
        .clone()
        .start(
            None,
            ApiCredentials::new(
                credentials.key.clone(),
                credentials.secret.clone(),
                credentials.passphrase.clone(),
            ),
        )
        .await?;
    Ok(Live {
        socket: Box::pin(socket),
        api_key: credentials.key.clone(),
        maker: maker.to_string(),
    })
}

/// Maps one socket event to the items it yields, in order.
fn translate(
    item: Result<UserEvent, ClobWsError>,
    api_key: &str,
    maker: &str,
    ids: &Mutex<ClientIds>,
    out: &mut VecDeque<Item>,
) {
    match item {
        Err(err) => out.push_back(Err(err.into())),
        Ok(UserEvent::Reconnected) => out.push_back(Ok(TradingEvent::Resync)),
        Ok(UserEvent::Disconnected {
            reason: DisconnectReason::Error(err),
        }) => out.push_back(Err(err.into())),
        // The `Resync` after the reconnect follows either way.
        Ok(UserEvent::Disconnected { .. }) => {}
        Ok(UserEvent::Message(UserMessage::Order(message))) => {
            out.push_back(ack(&message, ids).map(TradingEvent::Ack));
        }
        Ok(UserEvent::Message(UserMessage::Trade(trade))) => {
            on_trade(&trade, api_key, maker, ids, out);
        }
    }
}

fn on_trade(
    trade: &TradeMessage,
    api_key: &str,
    maker: &str,
    ids: &Mutex<ClientIds>,
    out: &mut VecDeque<Item>,
) {
    match trade.status {
        // A trade is reported once, when the CLOB matches it.
        TradeStatus::Matched => match fills(trade, api_key, maker, ids) {
            Ok(fills) => out.extend(fills.into_iter().map(|fill| Ok(TradingEvent::Fill(fill)))),
            Err(err) => out.push_back(Err(err)),
        },
        // Settlement gave up: the fill reported at `MATCHED` did not happen.
        TradeStatus::Failed => out.push_back(Ok(TradingEvent::Resync)),
        TradeStatus::Mined | TradeStatus::Confirmed | TradeStatus::Retrying => {}
    }
}

/// An order message as the ack it is. The message does not carry the time in
/// force, so a FAK remainder the venue cancelled is acked `Cancelled` though
/// `place` returned it `Killed`.
fn ack(message: &OrderMessage, ids: &Mutex<ClientIds>) -> Result<Order, ClassifiedError> {
    let size = decimal_text(&message.original_size, "an original size")?;
    let filled = decimal_text(&message.size_matched, "a matched size")?;
    let status = match message.order_type {
        OrderEventType::Placement => OrderStatus::Open,
        OrderEventType::Update if filled >= size => OrderStatus::Filled,
        OrderEventType::Update => OrderStatus::Open,
        OrderEventType::Cancellation => OrderStatus::Cancelled,
    };
    // Only a placement's time is the order's creation.
    let created = match message.order_type {
        OrderEventType::Placement => Some(millis(&message.timestamp)?),
        OrderEventType::Update | OrderEventType::Cancellation => None,
    };
    let id = OrderId::new(message.id.as_str());
    let mut order = Order::new(
        id.clone(),
        key_of(&message.asset_id)?,
        side_text(&message.side)?,
        decimal_text(&message.price, "a price")?,
        size,
        status,
    );
    order.filled = filled;
    order.created = created;
    order.client_order_id = {
        let mut ids = lock(ids);
        let client = ids.client_of(&id);
        // An order that ended has no more acks to carry its client id.
        if matches!(status, OrderStatus::Filled | OrderStatus::Cancelled) {
            ids.remove(&id);
        }
        client
    };
    order.extensions = Extensions::of(OrderExt {
        condition_id: Some(message.market.clone()),
        outcome: Some(message.outcome.clone()),
        ..OrderExt::default()
    });
    Ok(order)
}

/// The fills of a matched trade: the taker's, when the account took, then
/// each maker entry of the account's.
fn fills(
    trade: &TradeMessage,
    api_key: &str,
    maker: &str,
    ids: &Mutex<ClientIds>,
) -> Result<Vec<Fill>, ClassifiedError> {
    let side = side_text(&trade.side)?;
    let time = millis(&trade.timestamp)?;
    let mut out = Vec::new();
    if trade.owner.as_deref() == Some(api_key) {
        let order = OrderId::new(trade.taker_order_id.as_str());
        let mut fill = Fill::new(
            fill_id(&trade.id, &order),
            order,
            key_of(&trade.asset_id)?,
            side,
            decimal_text(&trade.price, "a price")?,
            decimal_text(&trade.size, "a size")?,
            time,
        );
        fill.liquidity = Some(Liquidity::Taker);
        fill.extensions = Extensions::of(FillExt {
            condition_id: Some(trade.market.clone()),
            outcome: Some(trade.outcome.clone()),
            taker_order_id: Some(trade.taker_order_id.clone()),
            transaction_hash: trade.transaction_hash.clone(),
            fee_rate_bps: None,
        });
        out.push(fill);
    }
    for entry in &trade.maker_orders {
        if !entry.maker_address.eq_ignore_ascii_case(maker) {
            continue;
        }
        let order = OrderId::new(entry.order_id.as_str());
        // On the trade's own token the maker is the other side; on the
        // complementary token (a mint or merge match) it is the same side.
        let side = if entry.asset_id == trade.asset_id {
            opposite(side)
        } else {
            side
        };
        let mut fill = Fill::new(
            fill_id(&trade.id, &order),
            order,
            key_of(&entry.asset_id)?,
            side,
            decimal_text(&entry.price, "a price")?,
            decimal_text(&entry.matched_amount, "a matched amount")?,
            time,
        );
        fill.liquidity = Some(Liquidity::Maker);
        fill.extensions = Extensions::of(FillExt {
            condition_id: Some(trade.market.clone()),
            outcome: Some(trade.outcome.clone()),
            taker_order_id: Some(trade.taker_order_id.clone()),
            transaction_hash: trade.transaction_hash.clone(),
            fee_rate_bps: entry
                .fee_rate_bps
                .as_deref()
                .and_then(|bps| bps.parse::<Decimal>().ok()),
        });
        out.push(fill);
    }
    if out.is_empty() {
        return Err(decode("a trade that names none of the account's orders"));
    }
    let trade_id = TradeId::new(trade.id.as_str());
    for fill in &mut out {
        fill.trade_id = Some(trade_id.clone());
        fill.client_order_id = lock(ids).client_of(&fill.order_id);
    }
    Ok(out)
}

/// A fill's id: the trade it is part of and the order it fills.
fn fill_id(trade: &str, order: &OrderId) -> FillId {
    FillId::new(format!("{trade}:{order}"))
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Buy => Side::Sell,
        Side::Sell => Side::Buy,
    }
}

/// A time the CLOB sent as text, in milliseconds.
fn millis(text: &str) -> Result<UnixMillis, ClassifiedError> {
    text.parse()
        .map(UnixMillis)
        .map_err(|_| decode("a timestamp that is not a number of milliseconds"))
}

#[cfg(test)]
mod tests {
```

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob
```

Expected: the library run prints `test result: ok. 40 passed; 0 failed` (the 35 of `clob::venue::trading` and `clob::venue::fills`, plus the five `clob::venue::tests` that already existed), and the integration run prints `test result: ok. 35 passed; 0 failed` (the `gamma` tests of a proxy client are not built here).

Then check the build with the user socket and without Data, where `Clob`'s `user_socket` has no reader, and that nothing warns:

```bash
nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws --all-targets -- -D warnings
```

Expected: the check finishes with no warning.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
nix develop --command cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws,data --all-targets -- -D warnings
git add -N polyoxide-polymarket/src/clob/venue/fills.rs
git commit -m "feat(clob): Trading::events on the supervised user socket (Story 7.4)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-polymarket/src/clob/venue.rs polyoxide-polymarket/src/clob/venue/trading.rs polyoxide-polymarket/src/clob/venue/fills.rs polyoxide-polymarket/src/clob/client.rs CLAUDE.md polyoxide-polymarket/tests/trading_clob.rs
```

### Task 5.5: The umbrella, the docs and the registry

Turns `Trading` on through the umbrella's `polymarket-clob-ws` and pins it there, writes the README line and the CLAUDE.md paragraph, declares the gate in the crate's `notes` and regenerates the lists it feeds, and runs the story's gates.

**Files:**
- Modify: `polyoxide/src/lib.rs:273-281`
- Modify: `polyoxide/Cargo.toml:24`
- Modify: `polyoxide-polymarket/Cargo.toml:523-528`
- Modify: `polyoxide-polymarket/src/clob/README.md:17`
- Modify: `CLAUDE.md:385` (and its regenerated crate list and umbrella-features region)
- Modify: `ci/dep-allowlists/*.txt` only if the pytest in Step 4 names a joined crate

- [ ] **Step 1: Write the failing pins**

In `polyoxide/src/lib.rs`, Current:

```rust
const _: fn(&crate::venue::DynMarketData<'static>) = |_| {};
```

Replace with:

```rust
const _: fn(&crate::venue::DynMarketData<'static>) = |_| {};
// `Trading` and its dyn wrapper come with the CLOB's sockets, and the CLOB
// implements the trait with Data as well.
#[cfg(feature = "polymarket-clob-ws")]
const _: fn(&crate::venue::DynTrading<'static>) = |_| {};
#[cfg(all(feature = "polymarket-clob-ws", feature = "polymarket-data"))]
const _: fn() = || {
    fn implements<T: crate::venue::Trading>() {}
    implements::<crate::polymarket::clob::Clob>();
};
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo check -p polyoxide --no-default-features --features polymarket-clob-ws,polymarket-data
```

Expected: the check passes, and no run can show these pins red. `polyoxide-polymarket/clob-ws` already turns on `polyoxide-venue/trading` (Task 5.1), and cargo builds the one `polyoxide-venue` with the union of the features its dependents ask for, so `DynTrading` is already in `polyoxide::venue`. Step 3 still states the feature in the umbrella's own list, as `polymarket-clob` does for `market`, so that the pin holds whatever the venue crate's own features do, and reviewers see the dependency where the pin sits.

- [ ] **Step 3: Implement**

Turn the feature on in the umbrella. In `polyoxide/Cargo.toml`, Current:

```toml
polymarket-clob-ws = ["polymarket-clob", "polyoxide-polymarket/clob-ws"]
```

Replace with:

```toml
polymarket-clob-ws = ["polymarket-clob", "polyoxide-polymarket/clob-ws", "polyoxide-venue/trading"]
```

Declare the gate in the crate's notes, which the generated crate list shows. It lands here, beside the regeneration, so that no earlier commit of the story leaves `gen_registry.py --check` red. Current (`polyoxide-polymarket/Cargo.toml:523-528`, the lines having moved down one with Task 5.1's `clob-ws` entry):

```toml
  "perps' auth and trading pending",
]
```

Replace with:

```toml
  "`Trading` for the CLOB (`clob::venue`) is built with `clob-ws` and `data` together: its events stream runs on the supervised user socket, which is `clob-ws`'s, and its positions read the Data API",
  "perps' auth and trading pending",
]
```

Add the README line, as prose with no code fence, since the module README is a doctest. In `polyoxide-polymarket/src/clob/README.md`, Current:

```markdown
- **WebSocket**: Real-time market and user channels (feature `clob-ws`). Live sports scores are in the [`sports`](../sports/README.md) module, feature `sports`.
```

Replace with:

```markdown
- **WebSocket**: Real-time market and user channels (feature `clob-ws`). Live sports scores are in the [`sports`](../sports/README.md) module, feature `sports`.
- **Trading**: With `clob-ws` and `data` together, `Clob` implements the venue-neutral `Trading` trait of `polyoxide-venue`: place and cancel orders, read open orders, positions and the pUSD balance, and follow acknowledgements and fills on an events stream that says when to reconcile.
```

Write the CLAUDE.md paragraph (AD-21), directly before the "**Wire enums**" paragraph. In `CLAUDE.md`, Current (the start of line 385):

```markdown
**Wire enums** — Story 3.9 declared the wire enums of gamma
```

Replace with:

```markdown
**The CLOB through `Trading`** — `clob::venue` (Story 7.4) implements `polyoxide_venue::Trading` on `Clob` with `clob-ws` and `data` together (amendment A7-3): `events()` runs on the supervised user socket, which is `clob-ws`'s, and `positions` reads the Data API. `clob-ws` turns on `polyoxide-venue/trading`, and so does the umbrella's `polymarket-clob-ws`, whose `const _` pins `DynTrading` and `Clob: Trading`; the gate is in the crate's `notes`, so the generated crate list shows it. Every method needs an `Account` and fails with the CLOB's own "Account required" error (`InvalidRequest`) without one. The address that trades is the client's signature type's (`ClobBuilder::signature_type`, by default the account target's): `place` signs as it, and `positions` and `events()` resolve their maker address from it with no funder, as `balances` reads with it, so a client built for a proxy wallet places, lists positions and follows fills as that wallet (found through Gamma; without the `gamma` feature, the native refusal of a proxy with no funder). `place` supports every account the native order routes sign for, a Deposit Wallet's (signature type 3) included: the order's id is its EIP-712 hash under its exchange's domain, computed before it is signed for every signature type, and a Deposit Wallet's order is signed as its ERC-7739 envelope. It refuses before it sends anything, as `InvalidRequest`: a key of another product, an extension other than `ClobOrderOptions` (`neg_risk`, `tick_size` and `expiration`; no fee terms, since a V2 order signs no fee rate, and not `Eq`, since `TickSize` is not), reduce-only, `post_only` or an expiration with anything but good-til-cancelled, a client order id an order still holds, a price off the tick size (the options', else `GET /tick-size`) or outside 0 to 1, and a size over two decimals or under 0.01. It then builds the order with the native `create_order`, signs it and posts it. A FAK or FOK kill, matched on the native `ClobError` variants before any conversion (AD-20), is `Ok` with `Killed { reason }`; a 200 whose `success` is false is a `VenueRefusal`; `live` and `delayed` are `Open`; `matched` is read back from `GET /data/order/{id}` for its `size_matched` (`Filled`, a short IOC `Killed { Unmatched }` with its `filled`, a short FOK `Killed { Unfillable }`, a short resting order `Open`), and a failed read-back leaves it `Open` with a WARN, since an `Err` would invite a duplicate. `cancel` is `Ok` only for an id among `canceled`, and `cancel_all(Some(key))` resolves the token's condition through `/markets-by-token` and sends both it and the token, which the route requires. `open_orders` pages `/data/orders` to the `LTE=` sentinel and refuses a repeated cursor; `positions` is Data v1 `/positions` for the maker address with `sizeThreshold=0`, in pages of 500 up to Data's offset of 10,000, past which it is a `VenueRefusal`; `balances` is one pUSD balance with six decimals and each allowance in `BalanceExt`, an unlimited approval being `Decimal::MAX`. The CLOB stores no client order id, so a `Clob` keeps an in-process ledger of the ones it placed orders with, by the order's id, shared by its clones and its events streams and bounded at 10,000 entries: `place` drops an entry for an order that ended at once or was refused and keeps it after a `Network` or `Unavailable` failure, `cancel`, `cancel_all` and an ack of a `Cancelled` or `Filled` order drop it, and it is lost when the process ends. `events()` opens its own socket when first polled (`ClobBuilder::user_socket` sets its URL and timing) and yields a leading `Resync`, a `Resync` after every reconnect and for a `FAILED` trade (the venue voided a fill the stream reported, and the trait has no event that takes one back), one `Fill` per order of the account's in a `MATCHED` trade (the taker's when the trade's `owner` is the account's API key, then each maker entry at its maker address, a maker on the trade's own token taking the opposite side), and an ack per order message; `MINED`, `CONFIRMED` and `RETRYING` yield nothing, an order message does not carry the time in force, and text that does not parse is an `Err` item that skips its message. `tests/trading_clob.rs` drives all of it on mock servers and the kit's scripted server.

**Wire enums** — Story 3.9 declared the wire enums of gamma
```

Regenerate the text the `notes` entry and the umbrella's new feature entry change. It is generated text between markers: never edit it by hand.

```bash
nix develop --command python3 scripts/gen_registry.py --write
nix develop --command python3 scripts/gen_registry.py --check
git status --short
```

Expected: `--write` rewrites two regions of `CLAUDE.md` and nothing else: the `polyoxide-polymarket` line of the crate list (it now ends with the `Trading` note) and the `claude-umbrella-features` region, where `polymarket-clob-ws` now reads `(polyoxide-polymarket/clob-ws, polyoxide-venue/trading)`; `README.md` does not change, since its crate list reads only each crate's `readme`. `--check` exits 0. `docs/ARCHITECTURE.md` is never edited by hand; if `git status` shows it changed, `--write` regenerated it and it goes into the commit.

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo check -p polyoxide --no-default-features --features polymarket-clob-ws,polymarket-data
nix develop --command cargo check -p polyoxide --no-default-features --features polymarket-clob-ws
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --lib clob::venue
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_module_readmes.py -q'
```

Expected: both checks finish, `trading_clob` prints `test result: ok. 35 passed; 0 failed`, the library run `ok. 40 passed; 0 failed`, and pytest passes. If `test_dep_allowlists.py` names a crate that joined `polyoxide-polymarket@clob-ws` or another allowlist (the `trading` feature builds `futures-core` there), regenerate the files and say why in the commit message:

```bash
nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'
```

Then run the story gates, in this order, and fix what they report:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy -p polyoxide-polymarket -p polyoxide -p polyoxide-cli -p polyoxide-py --all-targets --all-features -- -D warnings
nix develop --command cargo test -p polyoxide-polymarket -p polyoxide -p polyoxide-cli -p polyoxide-py --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket -p polyoxide -p polyoxide-cli -p polyoxide-py --all-features'
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
nix develop --command cargo hack check -p polyoxide-polymarket -p polyoxide --each-feature --no-dev-deps
```

`test_body_diff.py` reads only test items that moved or changed; the story adds new tests and edits none.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(clob): Trading through the umbrella, its docs and the registry (Story 7.4)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide/src/lib.rs polyoxide/Cargo.toml polyoxide-polymarket/Cargo.toml polyoxide-polymarket/src/clob/README.md CLAUDE.md Cargo.lock
```

If `git status` also shows regenerated `ci/dep-allowlists/*.txt` files or `docs/ARCHITECTURE.md`, add their paths to the pathspec and end the message with a line saying why (the `trading` feature joined `futures-core` to the `clob-ws` tree).

## Story 6: Live suites for the supervised sockets and `Trading`

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/tests/live_clob_ws.rs`, `polyoxide-polymarket/tests/live_clob_trading.rs`, `polyoxide-polymarket/Cargo.toml`, `scripts/live_unwraps.baseline.json`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- **Interfaces:** uses Stories 2, 3 and 5's public items (names contract) and the `"pUSD"` asset; exposes nothing
- **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma --test live_clob_trading && cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --test live_clob_ws && python3 scripts/live_unwraps.py && python3 scripts/gen_registry.py --check`
- **Review:** deep (live tests that place and cancel a real order)

Appends two `#[ignore]`d tests to `live_clob_ws.rs`: the supervised market socket receives a live book for a token the CLOB itself lists, and the supervised user socket stays connected for 15 s with the account's credentials and yields no error item. It adds the `live_clob_trading` target (spec §9), which reads through `Arc<DynTrading<'static>>` and then runs a post-only 0.01 bid of 5 shares: the stream yields its leading `Resync`, the bid rests, the stream acks it `Open`, and after `cancel` the stream acks it `Cancelled`. The bid sits under the best ask, so it moves no capital, and the test cancels it on every path it controls (a failed assertion is caught, the bid is cancelled, and the original failure is re-raised). Every failure goes through test-support's tags, so both files hold no unwrap, no `expect` and no bare failure site, and `scripts/live_unwraps.baseline.json` needs no entry (a file at zero is not in it). The new target is registered in the crate's `[package.metadata.polyoxide.live]` table; `gen_registry.py --write` then regenerates the nightly job, `SELF-HEALING.md`'s table and CLAUDE.md's nightly list. The new target declares the four secrets it reads and not `POLYMARKET_BUILDER_CODE`, because the registry's check requires a target's `secrets` to equal the names it passes to the loaders exactly; the job still carries `POLYMARKET_BUILDER_CODE` through `live_clob`. No code in this story holds a landmine marker.

### Task 6.1: The supervised market and user sockets, live

**Files:**
- Modify: `polyoxide-polymarket/tests/live_clob_ws.rs:233-236`
- Test: `polyoxide-polymarket/tests/live_clob_ws.rs`

- [ ] **Step 1: Write the failing test**

The file is append-only. Its new `use` lines go in the appended block, below the last existing test, so no existing line changes.

Current (`polyoxide-polymarket/tests/live_clob_ws.rs`):

```rust
        Ok(Some(Err(e))) => fail("server rejected the unfiltered subscription", &e),
        Ok(None) => transient("the server ended the connection on the unfiltered subscription"),
    }
}
```

Replace with:

```rust
        Ok(Some(Err(e))) => fail("server rejected the unfiltered subscription", &e),
        Ok(None) => transient("the server ended the connection on the unfiltered subscription"),
    }
}

// ── The supervised sockets ───────────────────────────────────────

use polyoxide_polymarket::clob::ws::{
    ApiCredentials, DisconnectReason, MarketEvent, MarketMessage, MarketWsBuilder, UserEvent,
    UserWsBuilder,
};
use polyoxide_polymarket::clob::Clob;
use polyoxide_test_support::environmental;

/// How long a subscribed token may take to deliver its first book. The server
/// answers a subscribe with the book at once, so 30 s is a stalled host.
const BOOK_WITHIN: Duration = Duration::from_secs(30);

/// How long the user socket must stay open without yielding an error item.
const STAY_CONNECTED: Duration = Duration::from_secs(15);

/// A token the CLOB lists as accepting orders, read from its own sampling
/// markets with the public client, so the token is one the market channel
/// serves. Fails as `environmental` when the CLOB lists none.
async fn a_listed_token() -> String {
    let listing = Clob::public()
        .markets()
        .sampling()
        .send()
        .await
        .or_fail("list the sampling markets");
    listing
        .data
        .iter()
        .filter(|market| market.accepting_orders == Some(true))
        .flat_map(|market| &market.tokens)
        .find_map(|token| token.token_id.clone())
        .unwrap_or_else(|| {
            environmental(
                "the CLOB lists no sampling market accepting orders; market conditions \
                 rather than a defect",
            )
        })
}

/// The supervised market socket delivers the `book` of a token it subscribed
/// to, within [`BOOK_WITHIN`], with no error item on the way.
#[tokio::test]
#[ignore]
async fn live_supervised_market_socket_receives_a_book() {
    let token = a_listed_token().await;
    let mut ws = MarketWsBuilder::new()
        .connect(vec![token.clone()])
        .await
        .or_fail("connect the supervised market socket");

    let deadline = tokio::time::Instant::now() + BOOK_WITHIN;
    let mut seen_book = false;
    while !seen_book {
        let item = match tokio::time::timeout_at(deadline, ws.next()).await {
            Err(_) => break,
            Ok(None) => transient("the supervised market socket ended before a book arrived"),
            Ok(Some(item)) => item,
        };
        if let MarketEvent::Message(MarketMessage::Book(book)) =
            item.or_fail("the supervised market socket yields an event")
        {
            assert_eq!(
                book.asset_id, token,
                "a book arrived for a token that was not subscribed"
            );
            seen_book = true;
        }
    }
    assert!(
        seen_book,
        "no book for token {token} within {BOOK_WITHIN:?}"
    );
    ws.close()
        .await
        .or_fail("close the supervised market socket");
}

/// The supervised user socket opens with the account's credentials and stays
/// connected for [`STAY_CONNECTED`]: the server refuses bad credentials about
/// a second after the subscribe with a 1008 close, which arrives as an error
/// item (`ClobWsError::Authentication`) and ends the stream, and any drop
/// inside the window fails the test.
#[tokio::test]
#[ignore]
async fn live_supervised_user_socket_stays_connected_with_the_accounts_credentials() {
    let (api_key, secret, passphrase) = derive_credentials().await;
    let mut ws = UserWsBuilder::new()
        .connect(None, ApiCredentials::new(api_key, secret, passphrase))
        .await
        .or_fail("connect the supervised user socket");

    let deadline = tokio::time::Instant::now() + STAY_CONNECTED;
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await {
            Err(_) => break,
            Ok(None) => transient("the supervised user socket ended inside the window"),
            Ok(Some(item)) => match item.or_fail("the supervised user socket yields an event") {
                UserEvent::Disconnected {
                    reason: DisconnectReason::Error(cause),
                } => fail("the supervised user socket lost its connection", &cause),
                UserEvent::Disconnected { reason } => transient(&format!(
                    "the supervised user socket lost its connection: {reason:?}"
                )),
                UserEvent::Reconnected => {
                    transient("the supervised user socket reconnected inside the window")
                }
                // An account event is not what this test asserts on.
                _ => {}
            },
        }
    }
    ws.close().await.or_fail("close the supervised user socket");
}
```

- [ ] **Step 2: Run it to see it fail**

Before the block above is in place, the two tests do not exist, so the listing holds neither.

Run: `nix develop --command bash -c "cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --test live_clob_ws -- --ignored --list | grep -c live_supervised"`

Expected: prints `0` and exits 1 (`grep -c` found no match).

- [ ] **Step 3: Implement**

Step 1's block is the whole implementation: it is the tests. Format it, and compile the target with the gate's lints.

Run: `nix develop --command cargo fmt --all`

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c "cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --test live_clob_ws -- --ignored --list | grep -c live_supervised"`

Expected: prints `2`.

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --test live_clob_ws`

Expected: the target compiles and ends `test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out` (the two existing tests and the two new ones, all `#[ignore]`d).

Run: `nix develop --command cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command python3 scripts/live_unwraps.py`

Expected: exit 0, and the file's counts unchanged (`live_clob_ws.rs` keeps its one opted-out site, and the new code adds none).

With network access and credentials, `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws --test live_clob_ws -- --ignored --nocapture live_supervised` runs the two tests against the live host. The gates do not need it.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(polymarket): live tests for the supervised market and user sockets (Epic 7, Story 6)" -- polyoxide-polymarket/tests/live_clob_ws.rs
```

### Task 6.2: The `live_clob_trading` target

**Files:**
- Create: `polyoxide-polymarket/tests/live_clob_trading.rs`
- Modify: `polyoxide-polymarket/Cargo.toml:316-318`
- Modify: `polyoxide-polymarket/Cargo.toml:540-544`
- Modify: `CLAUDE.md:654`
- Modify (generated): `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- Test: `.github/scripts/tests/test_live_registry.py`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-polymarket/tests/live_clob_trading.rs`. A test target cannot import another's helpers, so `find_token_the_bid_can_rest_on` is `live_clob.rs`'s `find_token_id_with_min_ask` (at `tests/live_clob.rs:138`), copied and given the one extra precondition this test needs: the book's tick and minimum order size must admit a 0.01 bid of 5 shares.

```rust
//! Live tests for the CLOB through the venue-neutral `Trading` trait. Ignored
//! by default; run with `cargo test -p polyoxide-polymarket
//! --no-default-features --features clob-ws,data,gamma --test live_clob_trading
//! -- --ignored --nocapture`.
//!
//! Every test reaches the CLOB as `Arc<DynTrading<'static>>`, the way a
//! consumer that picks its venue at run time does. The reads need an account.
//! The round trip places a post-only bid of 5 shares at 0.01 under the best
//! ask, so it rests and moves no capital, and cancels it on every path the
//! test controls. Without credentials each test fails as `auth-gated`.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{FutureExt, StreamExt};
use polyoxide_polymarket::clob::{
    Account, Clob, ClobBuilder, Credentials, OrderBook, SignatureType,
};
use polyoxide_polymarket::gamma::Gamma;
use polyoxide_polymarket::shared::keys::{self, TokenId};
#[cfg(feature = "keychain")]
use polyoxide_test_support::keychain;
use polyoxide_test_support::{
    environmental, fail, load_env, transient, ClassifiedResultExt, ResultExt,
};
use polyoxide_venue::{
    DynTrading, Order, OrderId, OrderRequest, OrderStatus, Side, Trading, TradingEvent,
    TradingEvents,
};
use rust_decimal::Decimal;

/// How long the events stream may take to yield its first item, which comes
/// once its socket's first connect attempt has ended.
const RESYNC_WITHIN: Duration = Duration::from_secs(30);

/// How long the user channel may take to ack a placement or a cancellation.
const ACK_WITHIN: Duration = Duration::from_secs(30);

/// How many gamma-listed candidates are confirmed against the CLOB book
/// before giving up. Each confirmation is one `/book` request.
const MAX_BOOK_PROBES: usize = 10;

/// The price the test's bid rests at: a valid multiple on every tick the CLOB
/// lists, and at the bottom of any book whose best ask is above it.
fn resting_price() -> Decimal {
    Decimal::new(1, 2)
}

/// The bid's size in shares, the CLOB's usual minimum order size.
fn bid_size() -> Decimal {
    Decimal::new(5, 0)
}

/// Load the account these tests trade as, from `POLYMARKET_*` and then the OS
/// keychain under the `polyoxide-clob` service when the `keychain` feature is
/// on. When neither source has all four, the loader fails the test as
/// `auth-gated`, which the nightly skips; an empty value counts as absent.
fn load_account() -> Account {
    let creds = load_env(&[
        "POLYMARKET_PRIVATE_KEY",
        "POLYMARKET_API_KEY",
        "POLYMARKET_API_SECRET",
        "POLYMARKET_API_PASSPHRASE",
    ]);
    #[cfg(feature = "keychain")]
    let creds = creds.or_else(|_| {
        keychain(
            "polyoxide-clob",
            &[
                ("POLYMARKET_PRIVATE_KEY", "private_key"),
                ("POLYMARKET_API_KEY", "api_key"),
                ("POLYMARKET_API_SECRET", "api_secret"),
                ("POLYMARKET_API_PASSPHRASE", "api_passphrase"),
            ],
        )
    });
    let creds = creds.unwrap_or_else(|missing| missing.or_auth_gated());
    Account::new(
        creds.get("POLYMARKET_PRIVATE_KEY"),
        Credentials {
            key: creds.get("POLYMARKET_API_KEY").to_owned(),
            secret: creds.get("POLYMARKET_API_SECRET").to_owned(),
            passphrase: creds.get("POLYMARKET_API_PASSPHRASE").to_owned(),
        },
    )
    .or_fail("build the account from POLYMARKET_*")
}

/// The account's CLOB client held as the venue-neutral trait object. The test
/// account is a Polymarket proxy wallet, so the client needs the POLY_PROXY
/// signature type to resolve the address its orders and positions belong to.
fn trading() -> Arc<DynTrading<'static>> {
    let clob = ClobBuilder::new()
        .with_account(load_account())
        .signature_type(SignatureType::PolyProxy)
        .build()
        .or_fail("authenticated clob client");
    DynTrading::new_arc(clob)
}

/// Whether the live book lets the test's bid rest unfilled and be accepted:
/// its best ask is above [`resting_price`], its tick divides it, and its
/// minimum order size is no more than [`bid_size`]. A book that omits its tick
/// or minimum size is not held against the token.
fn admits_the_bid(book: &OrderBook) -> bool {
    let best_ask = book.asks.iter().map(|level| level.price).min();
    let tick_fits = match book.tick_size.as_deref() {
        None => true,
        Some(tick) => tick
            .parse::<Decimal>()
            .is_ok_and(|tick| tick <= resting_price()),
    };
    let size_fits = match book.min_order_size.as_deref() {
        None => true,
        Some(least) => least
            .parse::<Decimal>()
            .is_ok_and(|least| least <= bid_size()),
    };
    best_ask.is_some_and(|ask| ask > resting_price()) && tick_fits && size_fits
}

/// Find an open market whose CLOB book admits the test's bid.
///
/// Selecting on the precondition, on the same `/book` route the check reads,
/// is what keeps a cheap first listing from failing the test. Narrowing is
/// free: gamma returns `best_ask` in the listing, and the chosen candidate is
/// then confirmed against the CLOB book, because gamma's figure is a cached
/// view.
///
/// Fails as `environmental` when nothing qualifies. A book request that errors
/// is not "nothing qualifies": when every probe errored, the last error fails
/// the test with its own tag, so an outage is not reported as market
/// conditions.
async fn find_token_the_bid_can_rest_on() -> String {
    let gamma = Gamma::builder().build().or_fail("gamma client");
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .limit(100)
        .send()
        .await
        .or_fail("gamma list markets");

    let clob = Clob::public();
    let mut probed = 0usize;
    let mut errored = 0usize;
    let mut last_error = None;

    for market in markets.iter() {
        let gamma_ask = market.best_ask.and_then(|ask| Decimal::try_from(ask).ok());
        if gamma_ask.is_none_or(|ask| ask <= resting_price()) {
            continue;
        }
        // clob_token_ids is a JSON-encoded array string: '["id1", "id2"]'
        let token_id = market.clob_token_ids.as_ref().and_then(|ids| {
            serde_json::from_str::<Vec<String>>(ids)
                .ok()
                .and_then(|v| v.into_iter().next())
        });
        let Some(token_id) = token_id else {
            continue;
        };

        probed += 1;
        match clob.markets().order_book(&token_id).send().await {
            Ok(book) => {
                if admits_the_bid(&book) {
                    return token_id;
                }
            }
            Err(err) => {
                errored += 1;
                last_error = Some(err);
            }
        }
        if probed >= MAX_BOOK_PROBES {
            break;
        }
    }

    if let Some(err) = last_error.filter(|_| errored == probed) {
        fail(&format!("every one of {probed} book probe(s) failed"), &err);
    }
    environmental(&format!(
        "no open market whose book admits a post-only bid of {} shares at {} among the \
         markets gamma lists ({probed} book(s) probed); market conditions rather than a \
         defect, so re-run before concluding otherwise",
        bid_size(),
        resting_price()
    ));
}

/// The first item of an events stream, which must be `Resync`.
async fn first_event(events: &mut TradingEvents) -> TradingEvent {
    match tokio::time::timeout(RESYNC_WITHIN, events.next()).await {
        Err(_) => transient("the events stream yielded nothing in its first 30 s"),
        Ok(None) => transient("the events stream ended before its first event"),
        Ok(Some(item)) => item.or_fail("the events stream's first item"),
    }
}

/// The next ack of order `id` whose status satisfies `wanted`, skipping every
/// other event, or `None` when [`ACK_WITHIN`] passes first.
async fn next_ack(
    events: &mut TradingEvents,
    id: &OrderId,
    wanted: fn(&OrderStatus) -> bool,
) -> Option<Order> {
    let deadline = tokio::time::Instant::now() + ACK_WITHIN;
    loop {
        match tokio::time::timeout_at(deadline, events.next()).await {
            Err(_) => return None,
            Ok(None) => transient("the events stream ended before the ack"),
            Ok(Some(item)) => match item.or_fail("the events stream yields an item") {
                TradingEvent::Ack(order) if order.id == *id && wanted(&order.status) => {
                    return Some(order);
                }
                _ => {}
            },
        }
    }
}

/// The account's open orders, positions and collateral balance read through
/// the trait object. The collateral is pUSD, and a balance is never negative.
#[tokio::test]
#[ignore]
async fn live_trading_reads_open_orders_positions_and_the_collateral_balance() {
    let trading = trading();

    let open = trading
        .open_orders(None)
        .await
        .or_fail("read the open orders");
    for order in &open {
        let _token = keys::token_id(&order.key)
            .or_fail("an open order is keyed by a CLOB outcome token");
    }

    let positions = trading.positions().await.or_fail("read the positions");
    for position in &positions {
        let _token = keys::token_id(&position.key)
            .or_fail("a position is keyed by a CLOB outcome token");
    }

    let balances = trading.balances().await.or_fail("read the balances");
    let collateral = balances.iter().find(|balance| balance.asset == "pUSD");
    assert!(
        collateral.is_some(),
        "no pUSD balance among {balances:?}"
    );
    assert!(
        collateral.is_some_and(|balance| balance.total.is_some_and(|total| total >= Decimal::ZERO)),
        "the pUSD balance has no non-negative total: {collateral:?}"
    );
}

/// A post-only bid rests, the events stream acks it after its leading
/// `Resync`, `cancel` succeeds, and the stream acks the cancellation. The bid
/// is cancelled on every path out of the test, including a failed assertion.
#[tokio::test]
#[ignore]
async fn live_trading_places_acks_and_cancels_a_resting_post_only_bid() {
    let trading = trading();
    let token_id = find_token_the_bid_can_rest_on().await;
    let key = keys::clob(&TokenId::new(token_id).or_fail("name the token"));

    // Open the stream before placing: it begins with `Resync`, and from then
    // on its socket delivers the account's order events.
    let mut events = trading.events();
    let first = first_event(&mut events).await;
    assert!(
        matches!(first, TradingEvent::Resync),
        "an events stream begins with Resync, not {first:?}"
    );

    let mut request = OrderRequest::new(key.clone(), Side::Buy, resting_price(), bid_size());
    // The venue refuses a post-only order that would take, so a mispriced bid
    // fails loudly instead of spending money.
    request.post_only = true;
    let order = trading
        .place(request)
        .await
        .or_fail("place the resting bid");

    // From here the bid rests on the venue. Whatever fails below, it is
    // cancelled, and the original failure is raised afterwards.
    let rested = AssertUnwindSafe(async {
        assert!(
            matches!(order.status, OrderStatus::Open),
            "the bid must rest: {order:?}"
        );
        assert_eq!(
            order.filled,
            Decimal::ZERO,
            "the bid must rest unfilled: {order:?}"
        );
        let resting = trading
            .open_orders(Some(&key))
            .await
            .or_fail("read the open orders of the token");
        assert!(
            resting.iter().any(|open| open.id == order.id),
            "order {} is not among the open orders {resting:?}",
            order.id
        );
        let acked = next_ack(&mut events, &order.id, |status| {
            matches!(status, OrderStatus::Open)
        })
        .await;
        assert!(
            acked.is_some(),
            "no Open ack of order {} within {ACK_WITHIN:?}",
            order.id
        );
    })
    .catch_unwind()
    .await;

    let cancelled = trading.cancel(&order.id).await;
    if let Err(failure) = rested {
        if let Err(cancel_error) = &cancelled {
            eprintln!(
                "the failed test also failed to cancel order {}: {cancel_error:?}",
                order.id
            );
        }
        std::panic::resume_unwind(failure);
    }
    cancelled.or_fail("cancel the resting bid");

    let acked = next_ack(&mut events, &order.id, |status| {
        matches!(status, OrderStatus::Cancelled)
    })
    .await;
    assert!(
        acked.is_some(),
        "no Cancelled ack of order {} within {ACK_WITHIN:?}",
        order.id
    );
}
```

Run: `nix develop --command cargo fmt --all`

- [ ] **Step 2: Run it to see it fail**

The file is a live target, and no `[package.metadata.polyoxide.live.live_clob_trading]` entry names it yet.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py -q'`

Expected: FAIL. `test_every_live_target_is_registered` prints `AssertionError: live test targets without a [package.metadata.polyoxide.live.<target>] entry: [('polyoxide-polymarket', 'live_clob_trading')]; entries naming no target: []`, and the parametrized `live_clob_trading` cases of the other tests print `... is not registered; see test_every_live_target_is_registered`.

- [ ] **Step 3: Implement**

Declare the target with its features, so a build without them does not compile it to an empty binary.

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
[[test]]
name = "live_clob_session_keys"
required-features = ["clob", "relay", "gamma"]
```

Replace with:

```toml
[[test]]
name = "live_clob_session_keys"
required-features = ["clob", "relay", "gamma"]

# `live_clob_trading` reaches the CLOB through `Trading`, which `clob-ws` and
# `data` build, and looks a proxy wallet up through gamma.
[[test]]
name = "live_clob_trading"
required-features = ["clob-ws", "data", "gamma"]
```

Register it in the nightly. It joins the `clob` suite, so it runs in the job `live_clob` and `live_clob_ws` already share. Its `secrets` are exactly the names the file passes to the loaders.

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
[package.metadata.polyoxide.live.live_clob_ws]
suite = "clob"
timeout = 15
features = ["clob-ws"]
secrets = ["POLYMARKET_PRIVATE_KEY"]
```

Replace with:

```toml
[package.metadata.polyoxide.live.live_clob_ws]
suite = "clob"
timeout = 15
features = ["clob-ws"]
secrets = ["POLYMARKET_PRIVATE_KEY"]

[package.metadata.polyoxide.live.live_clob_trading]
suite = "clob"
timeout = 15
features = ["clob-ws", "data", "gamma"]
secrets = [
  "POLYMARKET_API_KEY",
  "POLYMARKET_API_PASSPHRASE",
  "POLYMARKET_API_SECRET",
  "POLYMARKET_PRIVATE_KEY",
]
```

CLAUDE.md's Testing Conventions lists the venue crate's live targets by hand. The edit is to that one line; the nightly list below it is regenerated.

Current (`CLAUDE.md`, part of one long line):

```text
`live_clob`, `live_clob_ws`, `live_clob_session_keys`, `live_gamma`,
```

Replace with:

```text
`live_clob`, `live_clob_ws`, `live_clob_trading`, `live_clob_session_keys`, `live_gamma`,
```

Regenerate the nightly job and the two documents' lists (never by hand):

Run: `nix develop --command python3 scripts/gen_registry.py --write`

Expected rewrites, and only these three files: `.github/workflows/nightly-behavioral.yml` (the `live-polyoxide-polymarket-clob` job's `flags` line becomes `flags: "--no-default-features --features clob,clob-ws,data,gamma --test live_clob --test live_clob_trading --test live_clob_ws"`, with its `env:` secrets unchanged), `SELF-HEALING.md` (the `clob` cell of the `polyoxide-polymarket` row gains `` `live_clob_trading` (built with `--features clob-ws,data,gamma`) `` between `live_clob` and `live_clob_ws`) and `CLAUDE.md` (the generated nightly bullet reads `` - `live-polyoxide-polymarket-clob` (15 min): `live_clob` with `--features clob,gamma`, `live_clob_trading` with `--features clob-ws,data,gamma`, `live_clob_ws` with `--features clob-ws`; secrets `POLYMARKET_API_KEY`, `POLYMARKET_API_PASSPHRASE`, `POLYMARKET_API_SECRET`, `POLYMARKET_BUILDER_CODE`, `POLYMARKET_PRIVATE_KEY` ``). If the workflow edit is refused by the security hook on the first try, run the command again unchanged and check with `git diff --stat`.

`scripts/live_unwraps.baseline.json` stays as it is: the new file has no unwrap and no bare failure site, and a file at zero must not be in it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py -q'`

Expected: all pass (the parametrized cases now include `polyoxide-polymarket/live_clob_trading`; `test_scanned_env_names_equal_declared_secrets` finds the four names).

Run: `nix develop --command python3 scripts/gen_registry.py --check`

Expected: exit 0.

Run: `nix develop --command python3 scripts/live_unwraps.py`

Expected: exit 0 with the baseline unchanged.

Run: `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma --test live_clob_trading`

Expected: the target compiles and ends `test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out`.

Run: `nix develop --command cargo fmt --all -- --check`

Expected: no output.

Run: `nix develop --command cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features --no-run`

Expected: every target of the crate compiles with all features, the new one included.

With network access and credentials, `nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma --test live_clob_trading -- --ignored --nocapture` runs both tests against the live host. The round trip places and cancels a real post-only order, so run it only with an account the owner has named for it. The gates do not need it.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-polymarket/tests/live_clob_trading.rs
git commit -m "test(polymarket): live suite for Trading on the CLOB, and its nightly registration (Epic 7, Story 6)" -- polyoxide-polymarket/tests/live_clob_trading.rs polyoxide-polymarket/Cargo.toml CLAUDE.md SELF-HEALING.md .github/workflows/nightly-behavioral.yml
```

## Story 7: The Kalshi trading mapping, the amendments, the release note and the records

- **Developer:** junior-dev
- **Files:** `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md`, `docs/s3-migration.md`, `_bmad-output/implementation-artifacts/deferred-work.md`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`
- **Interfaces:** uses the names of Stories 1 to 5 in prose; exposes nothing
- **Verify:** `python3 scripts/gen_registry.py --check && git diff --check HEAD~1 HEAD`
- **Review:** light (planning documents and a release note; no code)

Ports Lane B's Kalshi mapping onto this epic's `Trading` trait, writing the whole document as a new file at its planning-companion path (spec §10, D9): the feature is `trading`, key construction follows `KeySpace`, the `events()` paragraph adds the leading `Resync` (D3) and the void rule (D4), and the release-note section points at `docs/s3-migration.md`. It keeps the tag's sources (Kalshi OpenAPI 3.34.0, AsyncAPI 2.0.0, read 2026-10-12), its deferrals and its eight demo-host probes. It writes `spine-amendments/epic-7.md` with A7-1 to A7-4 and "7.5: no amendment", in the format of `epic-6.md`. It adds Epic 7's consumer-impact section to `docs/s3-migration.md` (NFR11). It updates `deferred-work.md` (closes one entry, narrows one, adds two) and the clob rows of `divergences.md`. Documents only: no code, so no developer effort beyond copying. Every task's check is a `grep` that fails before the text exists and prints `ok` after.

### Task 7.1: The Kalshi trading mapping

**Files:**
- Create: `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`

- [ ] **Step 1: Write the failing test**

The check reads the finished document for what the port changes: the feature name, the `KeySpace` wording, the leading `Resync`, the `ensure_declared` refusal, the pointer to the release note, no leftover of the tag's feature name (`traits`) or its bundle, and the eight numbered probes.

```bash
nix develop --command bash -c 'f=_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md; grep -qF "feature \`trading\`" "$f" && grep -qF "KeySpace" "$f" && grep -qF "leading \`Resync\`" "$f" && grep -qF "Extensions::ensure_declared" "$f" && grep -qF "docs/s3-migration.md" "$f" && ! grep -qF "traits" "$f" && ! grep -qF "bundle N" "$f" && [ "$(grep -c "^[0-9]\. " "$f")" = 8 ] && echo ok'
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: `grep: _bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md: No such file or directory`, no `ok`, exit status 1.

- [ ] **Step 3: Implement**

Create `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`:

```markdown
# Kalshi trading mapping

Epic 7, Story 7.5. This maps every common Kalshi order field and fill field onto `polyoxide_venue::Trading` and its records (`polyoxide-venue/src/trading.rs`, feature `trading`). It shows the trait fits a second event-contract venue before Kalshi ships. Each field lands in one of three places: a trait field, a named field of a future Kalshi option or extension type, or a deferral to Kalshi integration.

This is a planning companion (spec D9 of Epic 7's design, `docs/superpowers/specs/2026-10-17-epic-7-supervised-clob-sockets-and-trading-design.md`: the owner's choice of 2026-10-12). Story 8.1 moves it to `docs/specs/kalshi/trading-mapping.md` when it adds Kalshi's mirror entry, and `deferred-work.md` holds that move.

**Sources, read on 2026-10-12:**

- Kalshi's OpenAPI `docs.kalshi.com/openapi.yaml`, `info.version` 3.34.0. The JSON pointers below point into it.
- The AsyncAPI `docs.kalshi.com/asyncapi.yaml`, `info.version` 2.0.0. Pointers into it are marked "AsyncAPI".
- These prose pages: `getting_started/order_direction.md`, `getting_started/fixed_point_migration.md`, `fix/order-entry.md`, and the changelog.

The legacy order-write routes under `/portfolio/orders` have been retired: a call returns "Please switch to the V2 endpoints" (changelog, 2026-06-18). So every write below uses the V2 routes under `/portfolio/events/orders`. Reads stay on `/portfolio/orders`.

## Keys and the YES-side conversion

A Kalshi key names one outcome: `kalshi.events:<TICKER>:yes` or `kalshi.events:<TICKER>:no` (AD-7). The ticker matches `^[A-Z0-9-]+$` (AsyncAPI `#/components/schemas/marketTicker`). Story 8.3 writes the product's `KeySpace`: its `parse_id` checks the ticker and the `:yes` or `:no` suffix and canonicalizes them, and its `id_text` renders them. A key is made only through it, since `MarketKey` has no other constructor.

A V2 order addresses the YES leg only. `side` is a `BookSide`, `bid` (buy YES) or `ask` (sell YES), and `price` is the YES price. Kalshi's own description of `BookSide` (`#/components/schemas/BookSide`) says: "Selling YES is economically equivalent to buying NO at `1 - price`". An `OrderRequest` therefore converts as follows, with `p` its price:

| Key's outcome | `OrderRequest.side` | Kalshi `side` | Kalshi `price` |
|---|---|---|---|
| `:yes` | `Buy` | `bid` | `p` |
| `:yes` | `Sell` | `ask` | `p` |
| `:no` | `Buy` | `ask` | `1 − p` |
| `:no` | `Sell` | `bid` | `1 − p` |

**Buying NO at p is a YES ask at 1 − p.**

Kalshi's reports keep only the net direction. On `Order` and `Fill`, `outcome_side` and `book_side` follow `bid ≡ yes, ask ≡ no`, "always" (`order_direction.md`). Kalshi itself makes buy-NO and sell-YES the same thing. A report therefore reads back in canonical form:

- `outcome_side: yes` becomes the `:yes` key, `Side::Buy`, at `yes_price_dollars`.
- `outcome_side: no` becomes the `:no` key, `Side::Buy`, at `no_price_dollars`. The WebSocket messages carry no `no_price_dollars`, so the price there is `1 − yes_price_dollars`.

So an order placed as "sell YES at 0.60" reads back as "buy NO at 0.40". It is the same order, converted deterministically as AD-7 requires. Every path reports this canonical form, so one order reads the same on `place`, `open_orders` and `events()`: `place` of `Sell` on `:yes` at 0.60 returns `Buy` on `:no` at 0.40.

**A NO position is a positive position in `kalshi.events:<TICKER>:no`.** `MarketPosition.position_fp` is signed: "Negative means NO contracts and positive means YES contracts" (`#/components/schemas/MarketPosition`). It maps to one `Position`:

- A positive value is a `Position` on `:yes` with `size = position_fp`.
- A negative value is a `Position` on `:no` with `size = −position_fp`.
- Zero yields no `Position`.

YES and NO in one market are netted, so the two keys never both hold a position.

## Order fields

### Request: `CreateOrderV2Request` (`#/components/schemas/CreateOrderV2Request`, `POST /portfolio/events/orders`)

| Kalshi field | Required | Maps to | Rule |
|---|---|---|---|
| `ticker` | yes | `OrderRequest.key` | The key's ticker part. A key of another venue or product is refused as `InvalidRequest` before anything is sent. |
| `side` | yes | `OrderRequest.side`, with the key's outcome | The conversion table above. |
| `count` | yes | `OrderRequest.size` | A `FixedPointCount` string of contracts (`SizeUnit::Contracts`). Requests accept 0–2 decimal places, and the minimum granularity is 0.01 (`#/components/schemas/FixedPointCount`). The size is normalized (trailing zeros dropped) before its scale is checked, so `10.000` passes. One with more than 2 decimals after that is refused as `InvalidRequest`, not rounded. It is sent with at least 2 decimals (`10.00`, `2.50`). |
| `price` | yes | `OrderRequest.price` | A `FixedPointDollars` YES price, with 2–4 decimals on requests (`#/components/schemas/FixedPointDollars`). It is `1 − price` for a `:no` key. The price is normalized before its scale is checked, so `0.5` and `0.5000` both pass. One with more than 4 decimals after that is refused as `InvalidRequest`. It is sent with at least 2 decimals (`0.50`). A price off the market's `price_ranges` step is Kalshi's to refuse, as a `VenueRefusal`. |
| `time_in_force` | yes | `OrderRequest.time_in_force` | `GoodTilCancelled` becomes `good_till_canceled`, `ImmediateOrCancel` becomes `immediate_or_cancel`, and `FillOrKill` becomes `fill_or_kill`. The three are the same set. |
| `client_order_id` | no | `OrderRequest.client_order_id` | Sent as is. A duplicate is a `409` (quick-start page), so a `VenueRefusal`. |
| `post_only` | no | `OrderRequest.post_only` | Sent as is. |
| `reduce_only` | no | `OrderRequest.reduce_only` | Sent as is. Kalshi rejects it unless the order is `immediate_or_cancel`. Integration decides whether to refuse that combination before sending. |
| `expiration_time` | no | `KalshiOrderOptions.expires_at: Option<UnixMillis>` | Sent as epoch seconds. Valid only with `good_till_canceled`, since `immediate_or_cancel` "cannot be combined with `expiration_time`". This is a venue option, as `ClobOrderOptions` carries the CLOB's expiration. |
| `cancel_order_on_pause` | no | `KalshiOrderOptions.cancel_order_on_pause: Option<bool>` | Sent when set. |
| `self_trade_prevention_type` | **yes** | Deferred | Kalshi requires it, so integration sends one fixed value it names until the option is designed. Under `taker_at_cross`, a fill-or-kill order that self-crosses keeps its partial fills (changelog, 2025-09-25). |
| `subaccount` | no | Deferred | Omitted, which means the primary account, 0. |
| `order_group_id` | no | Deferred | Omitted. |
| `exchange_index` | no | Not sent | Omitted, so Kalshi auto-routes by ticker. The shard comes back in `KalshiOrderExt.exchange_index`. |

`KalshiOrderOptions` is the one option type Kalshi's `place` declares, carried in `OrderRequest.extensions`. Any other extension on the request is refused as `InvalidRequest` by `Extensions::ensure_declared(&[TypeId::of::<KalshiOrderOptions>()])`, which returns `UndeclaredExtension`.

V2 has no `action`, no yes/no `side`, no `type` and no `buy_max_cost`. Market orders were withdrawn, and a marketable order is a limit priced through the book. Those fields exist only in the orphaned legacy schema, so nothing maps them.

### Reports: `CreateOrderV2Response`, `Order` and the `user_order` message

`place` gets back a `CreateOrderV2Response` (`#/components/schemas/CreateOrderV2Response`). It holds `order_id`, `client_order_id`, `fill_count`, `remaining_count`, `average_fill_price`, `average_fee_paid` and `ts_ms`, and no status, side or `Order`. The implementation builds the returned `Order` from the request plus those fields.

`open_orders` reads `Order` (`#/components/schemas/Order`), and `events()` reads the AsyncAPI `user_order` message (`#/components/schemas/userOrderPayload`). A `Kalshi*Ext` named below is the one value of that type in the record's `extensions`.

| Kalshi field | Maps to | Rule |
|---|---|---|
| `order_id` | `Order.id` | Kalshi's UUID. |
| `ticker`, `outcome_side` | `Order.key` | The canonical outcome, above. |
| `outcome_side`, `book_side` | `Order.side` | `Buy` on the canonical outcome. |
| `yes_price_dollars`, `no_price_dollars` | `Order.price` | Read by outcome. The WebSocket price for a NO order is `1 − yes_price_dollars`. |
| `initial_count_fp` (or the request's `count`) | `Order.size` | |
| `fill_count_fp`, `fill_count` | `Order.filled` | |
| `status` | `Order.status` | `resting` becomes `Open`, `executed` becomes `Filled`, and `canceled` becomes `Cancelled`, or `Killed` when the order's time in force ended it (below). `status` is `#/components/schemas/OrderStatus`, plus the WebSocket's `unknown`, which integration settles by re-reading the order. |
| `client_order_id` | `Order.client_order_id` | |
| `created_time`, `created_ts_ms`, `ts_ms` | `Order.created` | `created_time` is RFC 3339 on REST. The two `ts_ms` fields are epoch milliseconds. |
| (none) | `Order.time_in_force` | Kalshi does not report it, so the field is `None`, or `Some` for an order the implementation placed. |
| `remaining_count_fp`, `remaining_count` | `KalshiOrderExt.remaining` | |
| `average_fill_price`, `average_fee_paid` | `KalshiOrderExt.average_fill_price`, `.average_fee_paid` | Create response only. |
| `taker_fees_dollars`, `maker_fees_dollars`, `taker_fill_cost_dollars`, `maker_fill_cost_dollars` | `KalshiOrderExt.taker_fees`, `.maker_fees`, `.taker_fill_cost`, `.maker_fill_cost` | |
| `expiration_time`, `expiration_ts_ms` | `KalshiOrderExt.expires_at` | |
| `last_update_time`, `last_updated_ts_ms`, `last_update_reason` | `KalshiOrderExt.last_updated`, `.last_update_reason` | `last_update_reason` names why a `canceled` order ended: `SelfTradeCancel`, `PostOnlyCrossCancel`, `ReduceOnlyCancel`, `ExpiryCancel` and so on (WebSocket only). |
| `exchange_index`, `cancel_order_on_pause` | `KalshiOrderExt.exchange_index`, `.cancel_order_on_pause` | |
| `self_trade_prevention_type`, `order_group_id`, `subaccount_number` | Deferred | |
| `user_id` | Not mapped | The account's own id. |
| `side`, `action`, `is_yes` | Not read | Deprecated in favour of `outcome_side` and `book_side`. |

**Kills (AD-20).** Through the trait, a kill is `Ok` with `Killed { reason }`, never an error.

- **Immediate-or-cancel.** `remaining_count` "reflects the final state after unfilled contracts are canceled". So a create response with `remaining_count` 0 and `fill_count` below `count` is `Killed { reason: Unmatched }`, with `filled = fill_count`.
- **Fill-or-kill that cannot fill.** The REST outcome is not documented: either a `201` with `fill_count` 0, or a 4xx. FIX reports `FOK_INSUFFICIENT_VOLUME` as an execution `Canceled`, not a reject (`fix/order-entry.md`). Whichever it is, the trait returns `Ok` with `Killed { reason: Unfillable }`, and the probe below settles which signal the implementation reads.
- **Fill-or-kill that self-crosses.** It may end partly filled (changelog, "FoK orders that self-cross treated as IoC"). Kalshi cancelled it partway, so it is `Cancelled` with its partial `filled`, not `Killed`, as `TimeInForce::FillOrKill`'s contract says. The reason (`SelfTradeCancel`) goes to `KalshiOrderExt.last_update_reason`.
- **What Kalshi cannot tell the implementation.** Kalshi reports no time in force on `Order`, and `last_update_reason` has no value for an immediate-or-cancel remainder. So a kill is recognised only on an order whose time in force the implementation knows, which means one it placed. Any other order ends `Cancelled`, with `time_in_force: None`. `open_orders` never returns a killed order, so reconciliation is unaffected.

## Fill fields

The fills come from the AsyncAPI `fill` message (`#/components/schemas/fillPayload`, channel `fill`), which feeds `events()`. Trade history comes from the REST `Fill` (`#/components/schemas/Fill`, `GET /portfolio/fills`).

| Kalshi field | Maps to | Rule |
|---|---|---|
| `trade_id` | `Fill.id` (`FillId`) | "Unique identifier for fills". On REST, `trade_id` is the "legacy field name, same as fill_id". It is unique per fill, so it does not go to `Fill.trade_id`, which names one match. `Fill.trade_id` stays `None` until probe 8 shows both sides of a match share an id. |
| `fill_id` (REST) | `Fill.id` | The same value as `trade_id`. |
| `order_id` | `Fill.order_id` | |
| `market_ticker` (WebSocket), `ticker` (REST), `outcome_side` | `Fill.key` | The canonical outcome. |
| `outcome_side`, `book_side` | `Fill.side` | `Buy` on the canonical outcome. |
| `yes_price_dollars`, `no_price_dollars` (REST) | `Fill.price` | Read by outcome. The WebSocket message has no `no_price_dollars`, so its price is `1 − yes_price_dollars`. |
| `count_fp` | `Fill.size` | Contracts, to 2 decimals. |
| `fee_cost` | `Fill.fee` | Dollars, the quote currency, up to 6 decimals. |
| `is_taker` | `Fill.liquidity` | `true` becomes `Liquidity::Taker`, and `false` becomes `Maker`. |
| `ts_ms` (WebSocket), `created_time` (REST) | `Fill.time` | Epoch milliseconds, and RFC 3339 respectively. REST's `ts` states no unit and is not read. |
| `client_order_id` (WebSocket only) | `Fill.client_order_id` | REST fills dropped it on 2026-03-30. There it is `None`, or joined through `order_id`. |
| (none) | `Fill.notional` | `None`: Kalshi reports no quote amount. |
| `post_position_fp` (WebSocket) | `KalshiFillExt.post_position` | The signed net position after the fill. A negative value means NO contracts. |
| `exchange_index` | `KalshiFillExt.exchange_index` | |
| `subaccount`, `subaccount_number` | Deferred | |
| `side`, `action`, `purchased_side`, `ts` | Not read | Deprecated. `ts` is in seconds. |

## The trait's methods

- **`place`** is `POST /portfolio/events/orders`, with the request and report rules above.
- **`cancel(&OrderId)`** is `DELETE /portfolio/events/orders/{order_id}`. Kalshi warns that "an `order_id` alone cannot identify the exchange shard", and auto-routing needs `market_ticker`.
  - The implementation passes the ticker of an order it placed.
  - Otherwise it first reads `GET /portfolio/orders/{order_id}`, which takes no shard parameter and returns the order's `ticker` and `exchange_index`.
  - A `404` is a `VenueRefusal`.
- **`cancel_all(Option<&MarketKey>)`** lists the resting orders with `GET /portfolio/orders?status=resting`, adding `&ticker=` when a key is given. It cancels them with `DELETE /portfolio/events/orders/batched` and returns the ids whose items carry no `error`. The batch size limit scales with the tier, so a long list goes in several batches.
  - An item that errored inside the `200` stays open, and the consumer sees it in `open_orders`.
  - Kalshi's all-shards `DELETE /portfolio/events/orders` is not used. It answers `204` with no ids, and "newly placed orders may also be cancelled during the minute after the request".
- **`open_orders(Option<&MarketKey>)`** is `GET /portfolio/orders?status=resting`, adding `&ticker=` when a key is given, paged by `cursor`.
- **`positions`** reads `market_positions` from `GET /portfolio/positions`, with the sign rule above. Its subaccount defaults to 0.
  - `total_traded_dollars`, `market_exposure_dollars`, `realized_pnl_dollars`, `fees_paid_dollars` and `last_updated_ts` go to `KalshiPositionExt`.
  - `Position.average_price` is `None`, since Kalshi reports cost, not an average.
  - `event_positions` are per-event aggregates with no outcome key, and are not mapped.
- **`balances`** reads `GET /portfolio/balance` into one `Balance { asset: "USD", available: balance_dollars, total: None }`.
  - `portfolio_value` is integer cents with no `_dollars` twin, and goes to `KalshiBalanceExt.portfolio_value`, converted to dollars.
  - `balance_breakdown` (per shard) goes to `KalshiBalanceExt.per_shard`.
  - The integer-cent `balance` truncates sub-cent amounts and is not read.
- **`events()`** combines two WebSocket channels, both behind the signed handshake.
  - The `fill` channel feeds `TradingEvent::Fill`, and the `user_orders` channel feeds `TradingEvent::Ack`.
  - Both channels ride one connection, as subscriptions on Kalshi's one WebSocket endpoint. Neither envelope has a `seq`, and Kalshi replays nothing on resubscribe.
  - **The leading `Resync` (spec D3).** The stream yields `Resync` as its first item, once its connection's first connect attempt has ended, whether the connection went live or began in an outage. A fill between `events()` returning and the first subscription is therefore covered by a reconciliation.
  - **After every reconnect.** `Resync` comes before any later event, acknowledgement or fill (AD-24), so an outage of `user_orders` is flagged as surely as one of `fill`.
  - **The void rule (spec D4).** `Resync` also comes when the venue voids a fill it already reported, because the trait has no event that takes a fill back. Kalshi's `fill` message carries no status, and the pages read for this mapping describe no way for Kalshi to void a reported fill, so an implementation yields none today. If a probe finds one, it yields `Resync`, and `Resync` keeps its meaning: reconcile through `open_orders()` and the trade history, since the stream cannot vouch for what it reported or missed.

## Future Kalshi types named above

Kalshi integration defines these. Every field is an `Option`, so absent data stays absent.

- `KalshiOrderOptions`, the request option `place` declares:
  - `expires_at: Option<UnixMillis>`;
  - `cancel_order_on_pause: Option<bool>`.
- `KalshiOrderExt`:
  - `remaining`, `average_fill_price`, `average_fee_paid`, `taker_fees`, `maker_fees`, `taker_fill_cost` and `maker_fill_cost` (`Decimal`);
  - `expires_at` and `last_updated` (`UnixMillis`);
  - `last_update_reason` (an `open_enum!`);
  - `exchange_index` (`u32`);
  - `cancel_order_on_pause` (`bool`).
- `KalshiFillExt`:
  - `post_position` (`Decimal`);
  - `exchange_index` (`u32`).
- `KalshiPositionExt`:
  - `total_traded`, `market_exposure`, `realized_pnl` and `fees_paid` (`Decimal`);
  - `last_updated` (`UnixMillis`).
- `KalshiBalanceExt`:
  - `portfolio_value` (`Decimal`, dollars);
  - `per_shard` (each shard's index and balance).

## Deferred to Kalshi integration

- **Subaccounts.** These are `subaccount` on requests, and `subaccount_number` on REST `Order` and `Fill` and on the WebSocket `user_order` message. An omitted subaccount means every subaccount on orders, fills and cancel-all, and the primary account everywhere else.
- **Order groups.** These are `order_group_id`, the `/portfolio/order_groups/*` routes and the WebSocket `order_group_updates` channel.
- **RFQ and block trades.** These are the `/communications/*` routes and the WebSocket `communications` channel.
- **Self-trade prevention.** `self_trade_prevention_type` is required on every V2 create, so integration sends a fixed value until the option is designed.

These are also left to the native client and kept out of the trait: amend and decrease (`POST /portfolio/events/orders/{order_id}/amend` and `/decrease`), batch create, and explicit shard routing (`exchange_index`, which is auto-routed here).

## Misfits

None. Every common order field (ticker, side, count, price, time in force, client order id, post-only, reduce-only) maps to a trait field. So does every common fill field (trade and order ids, price, size, fee, taker flag). Each other Kalshi field maps to a named Kalshi option or extension field, or to a deferral. `spine-amendments/epic-7.md` records this as 7.5's "no amendment".

Two behaviours fit the trait without changing it. Every path reports an order and its fills in canonical outcome form (AD-7's deterministic conversion), so a `Sell` on `:yes` reads back as a `Buy` on `:no`. A kill is recognised only on an order whose time in force the implementation knows.

## To probe on the demo host before Kalshi's `Trading` implementation ships

The documentation does not settle these points:

1. The REST outcome of a fill-or-kill order that cannot fill: a `201` with `fill_count` 0, or a 4xx.
2. Whether an immediate-or-cancel order with no fill returns a `201` with `remaining_count` `"0.00"`. The schema's description implies it.
3. Whether a post-only order that would cross is refused (the legacy behaviour) or cancelled. This decides whether it surfaces as a `VenueRefusal` or as `Ok(Cancelled)`.
4. Whether V2 routes return the rejection codes of 2026-01-26 (`invalid_order`, `available_balance_too_low` and so on), which the error decode would key on.
5. The units of REST `Fill.ts`, `GetBalanceResponse.updated_ts` and the `min_ts`/`max_ts` filters. The mapping above reads none of them.
6. Whether `GET /portfolio/orders/{order_id}` finds an order on every shard, which `cancel`'s routing relies on.
7. Whether the bug of 2026-05-21, in which V2 cancel and amend responses described the wrong order, is fixed. `cancel_all` reads the batch-cancel items' `order_id`.
8. Whether both sides of one match carry the same `trade_id`. Kalshi documents it as unique per fill. If both sides share it, it can also fill `Fill.trade_id`; until then that field stays `None`.

## Release note

Epic 7's consumer-impact note is in `docs/s3-migration.md`, under "Epic 7: supervised CLOB sockets and normalized trading". It states that the epic is additive and names the `trading` feature and its items. Nothing public is removed or renamed.
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: `ok`.

Run: `git add -N _bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, then `nix develop --command git diff --check`

Expected: no output (no trailing whitespace; the intent-to-add entry makes the new file part of the diff).

- [ ] **Step 5: Commit**

```bash
git add -N _bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md
git commit -m "docs(epic-7): the Kalshi trading mapping, ported onto the Trading trait (Story 7)" -- _bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md
```

### Task 7.2: The spine amendments A7-1 to A7-4

**Files:**
- Create: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md`

- [ ] **Step 1: Write the failing test**

The check wants the four amendment headings and the "no amendment" heading, and nothing else at that level, and each amendment's amended rule named.

```bash
nix develop --command bash -c 'f=_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md; grep -q "^## A7-1: " "$f" && grep -q "^## A7-2: " "$f" && grep -q "^## A7-3: " "$f" && grep -q "^## A7-4: " "$f" && grep -q "^## 7.5: no amendment" "$f" && [ "$(grep -c "^## " "$f")" = 5 ] && grep -qF "amends AD-24" "$f" && grep -qF "amends AD-3" "$f" && grep -qF "amends AD-11" "$f" && grep -qF "amends AD-5" "$f" && echo ok'
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: `grep: _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md: No such file or directory`, no `ok`, exit status 1.

- [ ] **Step 3: Implement**

Create `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md`:

```markdown
# Proposed spine amendments: Epic 7

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A7-1: `Trading::events()` returns the `TradingEvents` newtype (amends AD-5's stream rule for dyn-held traits)

- **Proposed by:** the user, 2026-10-12, during bundle N's Story 7.3, as bundle N's decision N-D3, made when implementation found the conflict. Epic 7's design of 2026-10-17 adopts it (spec D2, in `docs/superpowers/specs/2026-10-17-epic-7-supervised-clob-sockets-and-trading-design.md`) and Story 4 ships it.
- **Current rule (AD-5):** "Streams are written `-> impl Stream<Item = …> + Send + Unpin + 'static` (futures-core). Implementations return `Box::pin(..)` or another `Unpin` stream." The rule applies to every dyn-held trait, each carrying `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`.
- **Proposed rule:**
  - **The return type.** A dyn-held trait's stream method returns a named, concrete stream type, not `impl Stream`. For `Trading` this is `pub struct TradingEvents(Pin<Box<dyn Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static>>)`.
  - **Its API.** It is built with `TradingEvents::new(stream)` and implements `Stream` by delegation. It is `Send`, `Unpin` and `'static`.
  - **Its documentation.** The type documents the replay-nothing and `Resync` contract (AD-24, as A7-2 amends it).
  - **Futures are unchanged.** Every future-returning method keeps `impl Future<Output = …> + Send`, which dynosaur handles.
- **Why:**
  - **dynosaur cannot keep `'static` on a returned `impl Trait`.** dynosaur 0.3.1, the newest release, removes every lifetime bound from a returned `impl Trait` and substitutes the method's own borrow lifetime (`dynosaur_derive` 0.3.1, `src/expand.rs`: `expand_sig_ret_ty` boxes the returned `impl Trait` as a `dyn` with the method's lifetime, and `expand_bounds`, at line 247, drops every lifetime bound). Through `Arc<DynTrading<'static>>`, `events()` would therefore return a stream that borrows the `Arc`. Calling it fails with E0597 ("argument requires that `x` is borrowed for `'static`"), so a consumer that picks its venue at run time could not move its event stream onto a task. Static dispatch was unaffected, which is why only a dyn test shows the problem.
  - **A concrete type passes through.** dynosaur leaves a return type that is not `impl Trait` as written, so the stream keeps `'static` on both paths. Implementations box the stream either way, as AD-5 already said.
  - **A newtype, not an alias.** The representation can change later without a break, and the type has a page of its own on which to document the stream's contract.
- **Follow-up in the code:** Story 4 ships it (`polyoxide-venue/src/trading.rs`, feature `trading`). `polyoxide-venue/tests/trading.rs` proves the claim rather than asserting it: it takes the stream from a temporary `Arc<DynTrading<'static>>` clone, drops every `Arc`, and drains the stream on a `std::thread::spawn`ed thread, which accepts only `Send + 'static`. A later dyn-held stream trait follows the same pattern, for example a streaming market-data trait, which Epic 6 deferred.

## A7-2: every events stream begins with `Resync`, and a voided fill yields `Resync` (amends AD-24)

- **Proposed by:** the user, 2026-10-17, during Epic 7's design (spec D3 and D4). Spec D3 is bundle N's review finding E15.
- **Current rule (AD-24):**
  - A `Trading` implementation feeds `events()` from an unbounded queue. Its fills channel's `Protocol` declares `Unbounded`.
  - After every `Reconnected` on a fills channel, it yields `TradingEvent::Resync` before any later fill. The consumer reconciles through `open_orders()` and trade history.
  - The trait documents that the venue replays nothing.
- **Proposed rule:**
  - **Unchanged:** the unbounded queue, and the documentation that the venue replays nothing.
  - **The stream begins with `Resync` (spec D3).** It yields `TradingEvent::Resync` as its first item, once its fills connection's first connect attempt has ended, whether the connection went live or began in an outage.
  - **After every reconnect.** `Resync` still comes after every reconnect of the fills connection, before any later event.
  - **A voided fill yields `Resync` (spec D4).** A trade yields a `Fill` when the venue reports it matched, and a trade the venue later reports as failed yields `Resync`, because the trait has no event that takes a fill back. The venue's intermediate states yield nothing, so a trade is reported once.
  - **The meaning of `Resync`.** It tells the consumer to reconcile through `open_orders()` and the trade history: the stream cannot vouch for what it reported or missed. It comes at the start, after every reconnect of the fills connection, and when the venue voids a fill.
- **Why:**
  - **The window before the first subscription.** A fill between `events()` returning and the first subscription is neither replayed nor preceded by a `Reconnected`, so AD-24's rule alone leaves it unflagged. A leading `Resync` covers it with one reconciliation.
  - **The trait cannot take a fill back.** A fill reported on a match and later failed on chain would leave a consumer's books wrong with no signal. `Resync` is the one signal the trait has.
- **Follow-up in the code:** Story 4 documents the contract on `Trading::events` and `TradingEvent::Resync`, and Story 5's events adapter implements it for the CLOB (`MATCHED` yields a `Fill`, `FAILED` a `Resync`). The Kalshi mapping applies the same two rules to Kalshi's streams.

## A7-3: the CLOB's `Trading` is gated `all(clob-ws, data)` (amends AD-3's last bullet)

- **Proposed by:** the user, 2026-10-17, during Epic 7's design (spec D6).
- **Current rule (AD-3, last bullet):** "Polymarket's `Trading::positions` reads the data API, so the clob `Trading` impl is gated `all(clob, data)` and declared in Cargo metadata."
- **Proposed rule:**
  - The clob `Trading` impl is gated `all(feature = "clob-ws", feature = "data")`, and the gate is declared in `polyoxide-polymarket`'s `[package.metadata.polyoxide] notes`.
  - `clob-ws` implies `clob` and turns on `polyoxide-venue/trading`; the umbrella's `polymarket-clob-ws` turns it on too.
- **Why:**
  - **`events()` needs the user socket.** The supervised user socket is `clob-ws`'s, so an impl gated on `clob` alone would have to omit `events()` or build without the socket it reads.
  - **`positions` needs the Data API.** That half of AD-3's reasoning stands, so `data` stays in the gate.
- **Follow-up in the code:** Story 5 adds the impl under that gate, the feature wiring in `polyoxide-polymarket/Cargo.toml` and `polyoxide/Cargo.toml`, and the `notes` entry that the generated crate lists in CLAUDE.md and README.md show.

## A7-4: a `Protocol` may dial its own connection (amends AD-11's "What a venue implements")

- **Proposed by:** the user, 2026-10-17, during Epic 7's design (spec D7).
- **Current rule (AD-11, "What a venue implements"):** only a `Protocol`: the handshake request per attempt; decode; the liveness and delivered predicates; the ping schedule; `P::Membership` with membership frames and paced `replay`; `wanted(&Membership)`; optionally `max_connection_age`; venue-specific recovery and protocol-originated events; whether it declares `Disconnected`; and its queue.
- **Proposed rule:** the list gains one item, "optionally how it dials: `Protocol::connect`, a provided method the `Supervisor` calls for every attempt. Its default builds the handshake's request and dials it with the kit's single-address `connect`, so a `Protocol` that does not override it behaves as before."
- **Why:**
  - **The CLOB dials every address.** It resolves each address of its host, interleaves the families and dials each in turn, bounding every step (`divergences.md` D20). The kit's `connect` dials only the first address, so a blackholed route, such as a broken IPv6 one, would spend the whole timeout there.
  - **Without the hook the supervised sockets lose that.** A `Supervisor` that always dials with the kit's `connect` would take the per-address behaviour away from the CLOB the moment it is supervised.
  - **Additive.** The method is provided, so perps' and Binance's protocols compile and behave unchanged.
- **Follow-up in the code:** Story 1 adds the method and the `Supervisor`'s call, and Stories 2 and 3 override it in `ClobMarketProtocol` and `ClobUserProtocol` with the CLOB's `dial`.

## 7.5: no amendment

Story 7.5's Kalshi mapping (`_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`) found no Kalshi order or fill field that fits neither the `Trading` trait nor a deferral, so it proposes no amendment. The same is true of bundle N's reading of the mapping: Epic 7's changes to the trait (the `trading` feature, `Extensions::ensure_declared`, and A7-2's two rules) moved no field of it.
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: `ok`.

Run: `git add -N _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md`, then `nix develop --command git diff --check`.

Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add -N _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md
git commit -m "docs(epic-7): spine amendments A7-1 to A7-4, and 7.5's no amendment (Story 7)" -- _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-7.md
```

### Task 7.3: Epic 7's consumer-impact section

**Files:**
- Modify: `docs/s3-migration.md:47-48`

- [ ] **Step 1: Write the failing test**

The check wants the section heading, and each item the release note must name (spec §10).

```bash
nix develop --command bash -c 'f=docs/s3-migration.md; grep -qF "## Epic 7: supervised CLOB sockets and normalized trading" "$f" && grep -qF "WebSocketWithPing" "$f" && grep -qF "Protocol::connect" "$f" && grep -qF "Extensions::ensure_declared" "$f" && grep -qF "UndeclaredExtension" "$f" && grep -qF "ClobBuilder::user_socket" "$f" && grep -qF "polyoxide-venue/trading" "$f" && grep -qF "ClobWsError::Authentication" "$f" && grep -qF "all(clob-ws, data)" "$f" && echo ok'
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: no `ok` (the first `grep` finds no Epic 7 heading), exit status 1.

- [ ] **Step 3: Implement**

The section is appended after Epic 6's last paragraph.

Current (`docs/s3-migration.md`):

```text
No dependency tree changes: `market` builds only rust_decimal, serde and dynosaur, which every
module that turns it on already builds, and the socket-only modules turn on neither feature.
```

Replace with:

```text
No dependency tree changes: `market` builds only rust_decimal, serde and dynosaur, which every
module that turns it on already builds, and the socket-only modules turn on neither feature.

## Epic 7: supervised CLOB sockets and normalized trading

**Consumer impact: additive.** Nothing is removed or renamed, and no signature changes; code
that builds on 0.40 builds unchanged.

- `polyoxide-ws`: `Protocol` gains a provided method, `Protocol::connect`, which the
  `Supervisor` calls for every attempt. Its default builds `handshake()`'s request and dials it
  with the kit's `connect`, so a `Protocol` written before it keeps its behaviour, and perps and
  Binance are untouched. The CLOB's two protocols override it with the CLOB's per-address dial.
- `polyoxide-polymarket`, with `clob-ws`: two supervised sockets beside the bare tier.
  `MarketWsBuilder` connects a `SupervisedMarketWs` (`ClobMarketProtocol`, `MarketEvent`,
  `MarketChange` and the `MarketMembership` trait), and `UserWsBuilder` a `SupervisedUserWs`
  (`ClobUserProtocol`, `UserEvent`, `UserChange` and the `UserMembership` trait). Both yield a
  `DisconnectReason` (the server's close code and reason, an error, a stall or a rotation) with
  each `Disconnected`. They reconnect after a close or a stall, replay their membership on every
  connection (the user socket its credentials too), and yield one `Disconnected` and one
  `Reconnected` per outage. They send a text `PING` every 10 s, count any frame as liveness and
  are stale after 30 s, back off from 500 ms to 60 s, and bound each address and step of a
  connect by 10 s. They take `wss://` URLs, or `ws://` to a loopback host, and at most 500 ids
  or markets per connection. The market queue holds 1024 events and the user queue is
  unbounded. The market socket yields every book of an array frame. `WebSocket`,
  `WebSocketBuilder` and `WebSocketWithPing` are unchanged, and `MarketMessage::from_json` still
  keeps only the first book of an array.
- `ClobWsError::Authentication` has its first use: the user socket decodes the server's 1008
  close as `ClobWsError::Authentication(<the close reason>)`, classed `Unauthorized`, and stops
  on it instead of reconnecting. The enum's variants are unchanged.
- `polyoxide-venue` gains a feature, `trading`, which turns on `market` and builds futures-core.
  It holds the trait `Trading` (`place`, `cancel`, `cancel_all`, `open_orders`, `positions`,
  `balances` and `events`) with its `DynTrading` wrapper; `TradingEvents`, the named stream
  `events()` returns, since dynosaur drops `'static` from a returned `impl Stream`;
  `TradingEvent` (`Ack`, `Fill` and `Resync`); the records `OrderRequest`, `Order`, `Fill`,
  `Position` and `Balance`; `OrderStatus`, `KillReason`, `TimeInForce` and `Liquidity`; and the
  opaque `OrderId`, `FillId` and `ClientOrderId`. A kill is `Ok` with `Killed { reason }`, never
  an error. Under `market`, `Extensions::ensure_declared` and `UndeclaredExtension` (an
  `InvalidRequest`) give a venue the refusal it runs on an option it does not declare.
- `polyoxide-polymarket`: `ClobBuilder::user_socket`, and, with `clob-ws` and `data`, `impl
  Trading for Clob` with `ClobOrderOptions` (`neg_risk`, `tick_size`, `expiration`) and the
  venue's own facts in `OrderExt`, `FillExt`, `PositionExt` and `BalanceExt`, all in
  `polyoxide_polymarket::clob::venue`.
  `clob-ws` now turns on `polyoxide-venue/trading`, and so does the umbrella's
  `polymarket-clob-ws`, whose crate root pins `DynTrading` and, with `polymarket-data`,
  `Clob: Trading`. The CLOB stores no client
  order id, so a `Clob` remembers the ones it placed in-process (at most 10,000, lost with the
  process). `positions` reads Data v1 with no dust filter, `balances` holds the pUSD collateral,
  and `events()` opens its own user socket for each call.
- No dependency tree gains a crate: `trading` builds futures-core, which every build that turns
  it on already has through futures-util.

Where this epic departs from its acceptance criteria:

- The CLOB's `Trading` is gated `all(clob-ws, data)`, not `all(clob, data)`, because `events()`
  needs the user socket, which is `clob-ws`'s.
- `ClobOrderOptions` carries no fee terms: a V2 order signs no fee rate, and a fill's `fee` is
  `None`, with the maker's `fee_rate_bps` in `FillExt`.
- The Kalshi trading mapping stays a planning document,
  `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, until Story 8.1
  registers Kalshi's mirror.
- Every events stream begins with `Resync`, and a fill the venue voids yields `Resync`, where the
  architecture asked for one only after a reconnect (amendment A7-2).
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: `ok`.

Run: `nix develop --command git diff --check`

Expected: no output.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(s3): Epic 7's consumer-impact note (Story 7)" -- docs/s3-migration.md
```

### Task 7.4: The deferred-work records

**Files:**
- Modify: `_bmad-output/implementation-artifacts/deferred-work.md:269-270`
- Modify: `_bmad-output/implementation-artifacts/deferred-work.md:336-337`
- Modify: `_bmad-output/implementation-artifacts/deferred-work.md:407`

- [ ] **Step 1: Write the failing test**

The check wants the close-code entry closed, the prune entry narrowed to `ConnectionClosed`, and the two new entries.

```bash
nix develop --command bash -c 'f=_bmad-output/implementation-artifacts/deferred-work.md; grep -qF "status: Done in Epic 7 (Stories 7.1 and 7.2): the supervised sockets surface the server" "$f" && grep -qF "prune \`ClobWsError::ConnectionClosed\`, which nothing constructs" "$f" && ! grep -qF "prune \`ClobWsError::{ConnectionClosed, Authentication}\`" "$f" && grep -qF "Move the Kalshi trading mapping" "$f" && grep -qF "Run the eight Kalshi demo-host probes" "$f" && echo ok'
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: no `ok`, exit status 1 (the first `grep` finds no Epic 7 status line).

- [ ] **Step 3: Implement**

Close the close-code entry. The entry keeps its text, and gains a `status:` line as the file's other closed entries have.

Current (`_bmad-output/implementation-artifacts/deferred-work.md`):

```text
  summary: For Epic 7's supervised clob sockets, surface the server's close code. clob's `WebSocket` ends with `None`, and `WebSocketWithPing::run` returns `Ok(())`, on every server close, so a 1008 policy close reads like a normal shutdown. `Connection::closed()` now holds the code.
  evidence: Review 1 finding B7 (low, as before the bundle). `polyoxide-clob/src/ws/client.rs` `poll_next` and `run` end on the connection's end without reading `closed()`.
```

Replace with:

```text
  summary: For Epic 7's supervised clob sockets, surface the server's close code. clob's `WebSocket` ends with `None`, and `WebSocketWithPing::run` returns `Ok(())`, on every server close, so a 1008 policy close reads like a normal shutdown. `Connection::closed()` now holds the code.
  evidence: Review 1 finding B7 (low, as before the bundle). `polyoxide-clob/src/ws/client.rs` `poll_next` and `run` end on the connection's end without reading `closed()`.
  status: Done in Epic 7 (Stories 7.1 and 7.2): the supervised sockets surface the server's close code and reason as `DisconnectReason::Closed { code, reason }` on every `Disconnected`, and the user socket decodes a 1008 close as `ClobWsError::Authentication` and stops on it (spec D10). The bare tier is unchanged: `WebSocket` still ends with `None` and `WebSocketWithPing::run` returns `Ok(())` on a server close.
```

Narrow the prune entry to the one variant nothing constructs.

Current (`_bmad-output/implementation-artifacts/deferred-work.md`):

```text
  summary: For S3: prune `ClobWsError::{ConnectionClosed, Authentication}`, which nothing constructs. S2 kept them, because removing them edits the protected unit test in `src/clob/ws/error.rs` that builds them. (low, review 1 B11)
  evidence: The 5.4 spec's Decisions; `git grep -n 'ConnectionClosed\|Authentication(' polyoxide-polymarket/src/clob` shows only the `class()` match and that test.
```

Replace with:

```text
  summary: For S3: prune `ClobWsError::ConnectionClosed`, which nothing constructs. S2 kept it, because removing it edits the protected unit test in `src/clob/ws/error.rs` that builds it. (low, review 1 B11)
  evidence: The 5.4 spec's Decisions; `git grep -n 'ConnectionClosed\|Authentication(' polyoxide-polymarket/src/clob` shows only the `class()` match and that test.
  status: Narrowed by Epic 7 (Story 7.2, spec D10): the supervised user socket constructs `ClobWsError::Authentication` on the server's 1008 close, so only `ConnectionClosed` is left to prune. Pruning it is a removal, so it stays out of Epic 7 (design §13).
```

Add the two entries for Story 8.1 and the Kalshi integration, after the file's last entry.

Current (`_bmad-output/implementation-artifacts/deferred-work.md`):

```text
  evidence: Polymarket retired `user-pnl-api` and `lb-api` on 2026-10-16; both answer `410 Gone` (nightly-behavioral run 38851911857 on the hub). The live tests now hold the 410, and fail once the hosts stop answering it.
```

Replace with:

```text
  evidence: Polymarket retired `user-pnl-api` and `lb-api` on 2026-10-16; both answer `410 Gone` (nightly-behavioral run 38851911857 on the hub). The live tests now hold the 410, and fail once the hosts stop answering it.

- source_spec: `docs/superpowers/specs/2026-10-17-epic-7-supervised-clob-sockets-and-trading-design.md`
  summary: Move the Kalshi trading mapping from `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md` to `docs/specs/kalshi/trading-mapping.md` in Story 8.1, in the commit that registers Kalshi (AD-13), and make `kalshi` a venue-gate term then. Update the citations of the old path in `spine-amendments/epic-7.md` ("7.5: no amendment") and `docs/s3-migration.md`.
  evidence: spec D9. `scripts/gen_registry.py:545-549` fails for a `docs/specs/` directory with no `[workspace.metadata.polyoxide.mirrors.<id>]` table, and that table is Story 8.1's registration, so the mapping stays a planning companion until then.

- source_spec: `docs/superpowers/specs/2026-10-17-epic-7-supervised-clob-sockets-and-trading-design.md`
  summary: Run the eight Kalshi demo-host probes before Kalshi's `Trading` implementation ships, and record each answer in the mapping or in `docs/specs/kalshi/OBSERVED.md`. They are: the REST outcome of a fill-or-kill that cannot fill (a `201` with `fill_count` 0, or a 4xx); whether an immediate-or-cancel with no fill returns a `201` with `remaining_count` `"0.00"`; whether a post-only order that would cross is refused or cancelled; whether V2 routes return the 2026-01-26 rejection codes; the units of REST `Fill.ts`, `GetBalanceResponse.updated_ts` and the `min_ts` and `max_ts` filters; whether `GET /portfolio/orders/{order_id}` finds an order on every shard; whether the 2026-05-21 bug in V2 cancel and amend responses is fixed; and whether both sides of one match carry the same `trade_id`.
  evidence: `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, "To probe on the demo host before Kalshi's `Trading` implementation ships". The documentation (OpenAPI 3.34.0, AsyncAPI 2.0.0, read 2026-10-12) does not settle any of the eight, and the mapping's kill, `cancel_all`, `cancel` and `Fill.trade_id` rules each depend on one.
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: `ok`.

Run: `nix develop --command git diff --check`

Expected: no output.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(epic-7): deferred-work closes the close-code entry, narrows the prune entry and adds the Kalshi moves (Story 7)" -- _bmad-output/implementation-artifacts/deferred-work.md
```

### Task 7.5: The clob rows of `divergences.md`

**Files:**
- Modify: `_bmad-output/specs/spec-venue-extensibility/divergences.md:20`
- Modify: `_bmad-output/specs/spec-venue-extensibility/divergences.md:28`
- Modify: `_bmad-output/specs/spec-venue-extensibility/divergences.md:35-36`
- Modify: `_bmad-output/specs/spec-venue-extensibility/divergences.md:55`

- [ ] **Step 1: Write the failing test**

The check wants each of the five rows to say what the supervised sockets do, and the old R11 sentence gone.

```bash
nix develop --command bash -c 'f=_bmad-output/specs/spec-venue-extensibility/divergences.md; grep -qF "which its supervised sockets send on the kit" "$f" && grep -qF "The supervised clob sockets keep it" "$f" && grep -qF "Binance, sports and the supervised clob sockets reconnect" "$f" && grep -qF "override \`Protocol::connect\` with it" "$f" && grep -qF "the supervised clob sockets (Epic 7) wait it out" "$f" && ! grep -qF "clob does not reconnect." "$f" && echo ok'
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: no `ok`, exit status 1.

- [ ] **Step 3: Implement**

Each edit changes one fragment of one table row, so the rest of the row stays as it is.

D4 (the ping row):

Current (`_bmad-output/specs/spec-venue-extensibility/divergences.md`):

```text
sports sends none; clob every 10 s |
```

Replace with:

```text
sports sends none; clob a text `PING` every 10 s, which its supervised sockets send on the kit's wall clock, the first one an interval after the connection opens, where the bare tier pings at once (Epic 7) |
```

D12 (Binary frames):

Current (`_bmad-output/specs/spec-venue-extensibility/divergences.md`):

```text
| D12 | Clob decodes Binary frames as text; others skip them | Protocol-specific. |
```

Replace with:

```text
| D12 | Clob decodes Binary frames as text; others skip them | Protocol-specific. The supervised clob sockets keep it: their decode reads a Binary frame as text, as the bare tier does (Epic 7). |
```

D19, its first cell and then its last sentence:

Current (`_bmad-output/specs/spec-venue-extensibility/divergences.md`):

```text
perps and rtds stop while Binance and sports reconnect |
```

Replace with:

```text
perps and rtds stop while Binance, sports and the supervised clob sockets reconnect |
```

Current (`_bmad-output/specs/spec-venue-extensibility/divergences.md`):

```text
since Story 4.10). |
```

Replace with:

```text
since Story 4.10). The supervised clob sockets reconnect after one as well: their `recover` reconnects exactly when `Classify::is_retriable` says so, and a use after close (`Kind::AlreadyClosed`) reconnects too, though its class stays `InvalidRequest` (Epic 7). |
```

D20 (the per-address connect):

Current (`_bmad-output/specs/spec-venue-extensibility/divergences.md`):

```text
connect_over` (Story 4.9). |
```

Replace with:

```text
connect_over` (Story 4.9). Under supervision the same dial runs on every attempt: `ClobMarketProtocol` and `ClobUserProtocol` override `Protocol::connect` with it (Epic 7), so each address is tried within `connect_timeout` per step there too. |
```

R11 (the `Retry-After` wait):

Current (`_bmad-output/specs/spec-venue-extensibility/divergences.md`):

```text
clob does not reconnect. |
```

Replace with:

```text
the supervised clob sockets (Epic 7) wait it out on the kit's `Supervisor` as every protocol does; the bare tier does not reconnect. |
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: `ok`.

Run: `nix develop --command python3 scripts/gen_registry.py --check`

Expected: exit 0 (no generated region changed).

Run: `nix develop --command git diff --check`

Expected: no output.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(epic-7): divergences.md's clob rows say what the supervised sockets do (Story 7)" -- _bmad-output/specs/spec-venue-extensibility/divergences.md
```
