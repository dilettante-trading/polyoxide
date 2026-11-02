# Binance Spot Streams Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** `polyoxide-binance` gains `spot-ws`: Binance spot's nine market streams on `stream.binance.com`, a bare socket and a supervised one that enforce spot's measured limits, built on socket plumbing shared with `usdm::ws`, with the CLI's `polyoxide binance spot stream` and the umbrella's `binance-spot-ws`.

**Architecture:** `usdm::ws`'s Binance-wide machinery (the request correlation, the bare connection, the kit `Protocol`, the router and its forwarding, the stop) moves into a crate-private `shared::ws`, generic over a `StreamSet` that each product implements on a marker type and a `SocketError` that each product's error implements. `usdm::ws` keeps every public type as its own struct or enum over the generic code, and every unit test in its file. `spot::ws` is then a second `StreamSet`: one path, `/stream`, at most 1024 names per request and one request per 500 ms, with its own stream names, payloads, errors, clients, captured frames and suites. The CLI and the umbrella expose it, and the docs follow.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), tokio, futures-util, `polyoxide-ws` (the `Supervisor`, `Connection`, the scripted test server), serde, rust_decimal, thiserror, clap (the CLI), and a Python capture script.

**Spec:** `docs/superpowers/specs/2026-11-02-binance-spot-design.md` (plan 2: §1.2, §3.2, §5.9, §5.10, §5.11, §5.12, §5.13, §7, §10), read with `.claude/rules/binance.md`, `.claude/rules/sockets.md` and `docs/specs/binance/OBSERVED.md`'s `## Spot` section from `### Stream limits` on, which holds the probe's measurements.

---

## Rules for every story

- **Run commands with the dev shell's prefix**, `nix develop --command`. A story's gates are the profile's; its **Verify** line is the developer's done-check.
- **No test changes body** (AD-12). A test is added after a file's last test, or in a file of its own. Story 2 moves no test: `usdm::ws`'s unit tests stay in their files (`client.rs`, `error.rs`, `event.rs`, `protocol.rs`, `stream.rs`, `supervised.rs`), byte for byte, and every name they use stays in scope in their module; an import only tests use goes under `#[cfg(test)]` at module level, never inside the tests module. `src/usdm/ws/scripted.rs` is not moved, copied or edited.
- **`usdm`'s public API does not change.** Every public path, name, variant, field and signature in `polyoxide_binance::usdm::ws` stays, and its docs keep their meaning. prader-rs pins `usdm::ws::StreamPath` exhaustive.
- **The production socket is shared.** Binance disconnects a connection that sends more than 5 messages a second and bans an address it disconnects repeatedly; the live suites and other sessions on this host share the address. No test, capture or probe against production sends faster than spot's client paces (one request per 500 ms), and none sends an over-limit request on purpose.
- **Fixtures are captured, never hand-written.** Story 1 captures the frames from production with no credentials; a later story that needs another one stops and asks.
- **Generated regions are regenerated, never edited**: `python3 scripts/gen_registry.py --write` after any `[package.metadata.polyoxide]` change or any change to a workspace member's dependencies. `docs/ARCHITECTURE.md` and `ci/dep-allowlists/*.txt` are never edited by hand.
- **README text that a doctest compiles**: a module README's `rust` fences compile under its feature, so every fence in `spot/README.md` builds with `spot` alone; socket examples go in `spot::ws`'s rustdoc, which compiles under `spot-ws`.
- **Live suites**: every test `#[ignore]`d; failures only through `polyoxide-test-support`'s tags (`or_fail`, `fail`, `environmental`, `transient`); no `.unwrap()`, `.expect(` or `panic!` (`python3 scripts/live_unwraps.py`).
- **A rule's text changes in the commit that changes the rule** (AD-21): Story 2, which moves `usdm::ws`'s protocol into `shared::ws`, rewrites the sentences of `.claude/rules/binance.md` that name its old file.
- **Nothing publishes.** No `cargo publish` without `--dry-run`, no `scripts/finish_release.sh`, no `gh workflow run Release`, no version bump.
- **Commit by pathspec**, conventional, with `Cargo.lock` when cargo changes it, never `--no-verify`; every message ends with `(binance spot ws, Story <n>)`.

## File structure

Paths are from the repo root.

**Story 1: frame captures.**

- `scripts/capture_binance_spot.py` (modify): a `ws` subcommand beside `rest`.
- `polyoxide-binance/tests/fixtures/spot/ws/*.json` (create, captured): the eleven files the names contract lists.
- `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md` (modify): `## Streams (ws/)` after `## REST (rest/)`.

**Story 2: shared socket plumbing under `usdm::ws`.**

- `polyoxide-binance/src/shared/mod.rs` (modify): `pub(crate) mod ws;` under `usdm-ws`.
- `polyoxide-binance/src/shared/ws/mod.rs`, `request.rs`, `bare.rs`, `protocol.rs`, `supervised.rs` (create).
- `polyoxide-binance/src/usdm/ws/client.rs`, `error.rs`, `protocol.rs`, `supervised.rs` (modify): onto `shared::ws`, their tests untouched; `usdm/ws/mod.rs` is unchanged.
- `.claude/rules/binance.md` (modify): the `Streams (usdm-ws)` sentences that name `src/usdm/ws/protocol.rs`.

**Story 3: spot stream names, payloads and errors.**

- `polyoxide-binance/Cargo.toml` (modify): the `spot-ws` feature, the `test-server` comment, one `[[test]]` stanza.
- `polyoxide-binance/src/lib.rs` (modify): the `Classify` assertion's spot socket rows.
- `polyoxide-binance/src/spot/mod.rs` (modify): `pub mod ws;` under `spot-ws`.
- `polyoxide-binance/src/spot/ws/mod.rs`, `stream.rs`, `event.rs`, `error.rs`, `fixtures.rs` (create).
- `polyoxide-binance/tests/wire_agreement_spot_ws.rs` (create).
- `.github/scripts/tests/test_dep_allowlists.py` (modify) and `ci/dep-allowlists/polyoxide-binance@spot-ws.txt` (by `--write`).
- `.github/scripts/tests/test_classify_coverage.py` (modify): the spot socket types in its two non-vacuity lists.
- `.claude/rules/ci-gates.md` (modify): the `polyoxide-binance` dependency rule, with Task 3.6 (AD-21); `.claude/rules/wire-enums.md` (modify): spot's hand-written `StreamPath`.

**Story 4: spot socket clients.**

- `polyoxide-binance/src/shared/mod.rs` (modify): `ws`'s `cfg` widened to `spot-ws`; `polyoxide-binance/src/shared/ws/mod.rs`, `bare.rs` (modify): the dead-code allows a build without `usdm-ws` needs.
- `Cargo.toml` (root, modify): `[workspace.metadata.polyoxide.mirrors.binance]`'s spot stream texts.
- `polyoxide-binance/src/spot/ws/mod.rs` (modify): the client modules, their re-exports and the module docs.
- `polyoxide-binance/src/spot/ws/protocol.rs`, `client.rs`, `supervised.rs` (create).
- `polyoxide-binance/Cargo.toml` (modify): docs.rs features and comment, `identifiers`, the `description` and `readme` texts, the `live_spot_ws` live entry, two `[[test]]` stanzas.
- `polyoxide-binance/tests/supervision_spot.rs`, `live_spot_ws.rs` (create).
- `docs/MUTANTS.md` (modify): the socket-pace row; `.github/scripts/tests/test_mutants_ledger.py` (modify): the row's cited-line snippets.
- `scripts/live_unwraps.baseline.json` (modify): `live_spot_ws.rs` under `opted_out`.
- The files `gen_registry.py --write` rewrites; `Cargo.lock`.

**Story 5: the CLI and the umbrella.**

- `polyoxide-cli/Cargo.toml` (modify): `polyoxide-binance` with `spot-ws`, in dependencies and dev-dependencies; the `description` and `readme` texts.
- `polyoxide-cli/src/main.rs` (modify): the `about` text and the `Binance` command's doc.
- `polyoxide-cli/src/commands/binance/mod.rs` (modify), `polyoxide-cli/src/commands/binance/spot.rs` (create).
- `polyoxide-cli/tests/ws_binance.rs` (modify, append only).
- `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs` (modify): `binance-spot-ws`, `full`, the crate docs, the prelude and the passthrough checks.
- The files `gen_registry.py --write` rewrites; `Cargo.lock`.

**Story 6: docs.**

- `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `polyoxide-cli/README.md`, `README.md`, `CLAUDE.md` (modify, outside generated regions).
- `.claude/rules/binance.md`, `.claude/rules/sockets.md`, `.claude/rules/errors.md`, `.claude/rules/cli.md`, `.claude/rules/umbrella.md`, `.claude/rules/live-tests.md` (modify).
- `docs/specs/binance/probes/` is not touched: the probe ran before this plan.

## Names contract

Every name one story defines for another, with its type, signature and visibility, and every text a later story quotes. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Paths under `src/` are `polyoxide-binance/src/` unless they name a crate.

**Story 1 → Stories 3 to 5.**

- **Fixtures**, under `polyoxide-binance/tests/fixtures/spot/ws/`, each one combined-stream envelope (`{"stream": …, "data": …}`) read from `wss://stream.binance.com:9443/stream` after one `SUBSCRIBE` of every name below, with no credentials, every key kept, written with `indent=1`:
  - `stream_btcusdt_aggTrade.json`: `btcusdt@aggTrade`;
  - `stream_btcusdt_trade.json`: `btcusdt@trade`;
  - `stream_btcusdt_kline_1m.json`: `btcusdt@kline_1m`;
  - `stream_btcusdt_miniTicker.json`: `btcusdt@miniTicker`;
  - `stream_all_miniTicker_arr.json`: `!miniTicker@arr`, its array trimmed to its first two rows;
  - `stream_btcusdt_ticker.json`: `btcusdt@ticker`;
  - `stream_btcusdt_bookTicker.json`: `btcusdt@bookTicker`;
  - `stream_btcusdt_depth5.json`: `btcusdt@depth5`, the bare (1000 ms) spelling, untrimmed;
  - `stream_btcusdt_depth20_100ms.json`: `btcusdt@depth20@100ms`, `bids` and `asks` trimmed to three levels each;
  - `stream_btcusdt_avgPrice.json`: `btcusdt@avgPrice`;
  - `stream_non_ascii_miniTicker.json`: `<s>@miniTicker` for the first `TRADING` symbol in `GET /api/v3/exchangeInfo` with a non-ASCII character, its ASCII letters lowercased (`币安人生usdt@miniTicker` on 2026-11-02); the capture fails when no such symbol is listed.
- **`scripts/capture_binance_spot.py ws [OUT_DIR]`**, `OUT_DIR` defaulting to `polyoxide-binance/tests/fixtures/spot`, writes those files under `OUT_DIR/ws/` and nothing else, nothing until every stream has delivered (within 30 s), opens one connection through `capture_common.ws_session` and sends one `SUBSCRIBE`, and prints the `## Streams (ws/)` section to stdout. `rest` is unchanged.
- **`polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`** gains, after `## REST (rest/)`, a section headed `## Streams (ws/)`: the capture date, host and command, the one `SUBSCRIBE`, then one table row per file (file, stream, trimmed to), and the non-ASCII symbol it picked.

**Story 2 → Story 4.** In `polyoxide-binance`, all `pub(crate)`:

- `src/shared/mod.rs` gains `#[cfg(feature = "usdm-ws")] pub(crate) mod ws;`. Story 4 widens it to `#[cfg(any(feature = "usdm-ws", feature = "spot-ws"))]`.
- `src/shared/ws/mod.rs` declares `pub(crate) mod bare; pub(crate) mod protocol; pub(crate) mod request; pub(crate) mod supervised;` and holds:
  - `pub(crate) const MAX_STREAMS_PER_CONNECTION: usize = 1024;`
  - `pub(crate) const DEFAULT_MAX_CONNECTION_AGE: Duration = Duration::from_secs(23 * 3600 + 50 * 60);`
  - `pub(crate) const EVENT_BUFFER: usize = 1024;`
  - the stream set:

        pub(crate) trait StreamSet: Sized + Send + Sync + 'static {
            type Name: Clone + Eq + std::hash::Hash + fmt::Display + fmt::Debug + Send + Sync + 'static;
            type Path: Copy + Eq + std::hash::Hash + fmt::Display + fmt::Debug + Send + Sync + 'static;
            type Update: Send + 'static;
            type Error: SocketError<Path = Self::Path>;
            type Event: Send + 'static;
            /// Every path, in the order a first connect opens them.
            const PATHS: &'static [Self::Path];
            const MAX_NAMES_PER_REQUEST: usize;
            const MIN_REQUEST_INTERVAL: Duration;
            fn path(name: &Self::Name) -> Self::Path;
            /// The combined-stream URL of `path` on `base`.
            fn url(base: &str, path: Self::Path) -> String;
            /// One text frame as an update: the product's `Update::from_json`.
            fn decode(text: &str) -> Result<Self::Update, Self::Error>;
            /// `Event::Update(Box::new(update))`.
            fn update(update: Self::Update) -> Self::Event;
            /// `Event::Disconnected { path, reason }`, the reason read from the kit's outage.
            fn disconnected(path: Self::Path, outage: polyoxide_ws::Outage<Self::Error>) -> Self::Event;
            /// `Event::Reconnected { path }`.
            fn reconnected(path: Self::Path) -> Self::Event;
        }

  - the error's view:

        pub(crate) enum Shape<'a> {
            Connect(&'a polyoxide_ws::WsError),
            ConnectTimeout, Closed, Refused, NoAnswer, Response, TooManyStreams, WrongPath, Frame, Stopped,
        }

        pub(crate) trait SocketError:
            From<polyoxide_ws::WsError> + polyoxide_venue::Classify + fmt::Display + fmt::Debug + Send + Sync + 'static
        {
            type Path;
            /// The product's `StopCause`.
            type Cause: Clone + fmt::Debug + Send + Sync + 'static;
            fn refused(code: i64, msg: String) -> Self;
            fn no_answer(id: u64, timeout: Duration) -> Self;
            fn response(id: u64, raw: String) -> Self;
            fn too_many_streams(path: Self::Path, limit: usize) -> Self;
            fn stopped(cause: Option<Self::Cause>) -> Self;
            /// `StopCause::of(self)`.
            fn cause(&self) -> Self::Cause;
            fn shape(&self) -> Shape<'_>;
        }

- `src/shared/ws/request.rs`: `pub(crate) enum Answer { Result { id: Option<u64>, result: serde_json::Value }, Error { id: Option<u64>, code: i64, msg: String } }` with `pub(crate) fn parse(text: &str) -> Option<Self>`, moved from `usdm/ws/client.rs`; and `pub(crate) async fn request<N: fmt::Display, E: SocketError>(connection: &mut polyoxide_ws::Connection, method: &str, params: Option<&[N]>) -> Result<serde_json::Value, E>`, today's `request` with `E::refused` and `E::no_answer`.
- `src/shared/ws/bare.rs`: `pub(crate) struct Bare<S: StreamSet>`, one connection on one path, with `pub(crate) async fn open(base_url: &str, path: S::Path, connect_timeout: Duration, answer_timeout: Duration) -> Result<Self, S::Error>` (paced at `S::MIN_REQUEST_INTERVAL`); `subscribe(&mut self, streams: &[S::Name])` (the cap first, then batches of `S::MAX_NAMES_PER_REQUEST`, each recorded once answered; it does not check paths: `UsdmWs::subscribe` checks `WrongPath` before calling it); `unsubscribe(&mut self, streams: &[S::Name])`; `list_subscriptions(&mut self) -> Result<Vec<String>, S::Error>`; `ping(&mut self) -> Result<Duration, S::Error>`; `close(&mut self) -> Result<(), S::Error>`; `streams(&self) -> &[S::Name]`; `path(&self) -> S::Path`; `connection(&self) -> &polyoxide_ws::Connection` (for the products' `Debug`); `#[cfg(test)] close_frame(&self) -> Option<(Option<u16>, String)>`; and `impl<S: StreamSet> futures_util::Stream for Bare<S>` with `Item = Result<S::Update, S::Error>`, skipping an answer nobody awaits and a Binary frame.
- `src/shared/ws/protocol.rs`: `#[derive(Debug, Clone)] pub(crate) enum Change<N> { Subscribe(Vec<N>), Unsubscribe(Vec<N>) }`, and `pub(crate) struct BinanceProtocol<S: StreamSet>` (its `Debug` written by hand, with no bound on `S`) with `pub(crate) fn new(base_url: String, path: S::Path) -> Self` and `impl<S: StreamSet> polyoxide_ws::Protocol for BinanceProtocol<S>` whose `Event = S::Event`, `Error = S::Error`, `Membership = Vec<S::Name>`, `Change = Change<S::Name>` and `Ack = Vec<S::Name>`: today's `UsdmProtocol`, its recovery matching on `Shape`, row for row.
- `src/shared/ws/supervised.rs`: `pub(crate) struct Settings<S: StreamSet> { pub(crate) base_url: String, pub(crate) timing: polyoxide_ws::Timing, pub(crate) streams: Vec<S::Name> }`; `pub(crate) async fn connect<S: StreamSet>(settings: Settings<S>) -> Result<Running<S>, S::Error>`, today's `UsdmWsBuilder::connect` body; `pub(crate) struct Running<S: StreamSet>` with `membership(&self) -> Handle<S>`, `async close(self) -> Result<(), S::Error>`, `is_running(&self) -> bool` and `poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<S::Event, S::Error>>>`; and `pub(crate) struct Handle<S: StreamSet>`, its `Clone` and `Debug` written by hand with no bound on `S`, with `async subscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error>` and `async unsubscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error>`. The router, `Command`, `Stop`, `Path`, `Ended`, `route` and `forward` are private to this file.
- `Bare<S>` and `Running<S>` are `Unpin` for every `S: StreamSet` (an explicit `impl Unpin` where auto-inference cannot prove it), so a product's wrapper polls them with `poll_next_unpin` and `poll_event`. A marker type needs no derive.
- `SocketError::cause` shares its name with the deprecated `std::error::Error::cause`, so every call is written `<E as SocketError>::cause(err)` (`<S::Error as SocketError>::cause(err)` in generic code), never `err.cause()`.
- `Answer` moves to `shared::ws::request` and `usdm::ws::client` does not re-export it: no `usdm` test names it.
- Texts Story 4 quotes from Story 2, which no Story 2 fixer changes: `src/shared/mod.rs`'s two lines `#[cfg(feature = "usdm-ws")]` and `pub(crate) mod ws;`; `Shape`'s `WrongPath` variant in `src/shared/ws/mod.rs`; `Bare::path`'s signature line in `src/shared/ws/bare.rs`. Story 4 puts `#[cfg_attr(not(feature = "usdm-ws"), allow(dead_code))]` on that variant and that method, which only `usdm` builds or calls, so a `spot-ws` build without `usdm-ws` is warning-free.
- In `usdm::ws`, after the story: `src/usdm/ws/protocol.rs` holds `pub(crate) struct UsdmStreams;` with `impl StreamSet for UsdmStreams`, `#[cfg(test)] pub(crate) type UsdmProtocol = BinanceProtocol<UsdmStreams>;` and `#[cfg(test)] pub(crate) type Change = shared::ws::protocol::Change<StreamName>;` (only the unit tests name them); `src/usdm/ws/error.rs` holds `impl SocketError for UsdmWsError`; `UsdmWs`, `SupervisedUsdmWs` and `MembershipHandle` wrap `Bare<UsdmStreams>`, `Running<UsdmStreams>` and `Handle<UsdmStreams>`; `UsdmWsBuilder` keeps its fields `base_url`, `timing` and `streams`. Story 4's spot types take the same shape.

**Story 3 → Stories 4 and 5.** In `polyoxide-binance`:

- `Cargo.toml`: `spot-ws = ["spot", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]`; the `test-server` comment names `spot::ws::fixtures` with `spot-ws` beside `usdm::ws::fixtures` with `usdm-ws`.
- `src/spot/mod.rs` gains `#[cfg(feature = "spot-ws")] pub mod ws;`.
- `src/spot/ws/mod.rs` (`pub`):
  - `pub mod error; pub mod event; #[cfg(any(test, feature = "test-server"))] #[doc(hidden)] pub mod fixtures; pub mod stream;`
  - `pub use error::{SpotWsError, StopCause};`, `pub use event::{AggTradeEvent, AvgPriceEvent, BookTickerEvent, KlineBar, KlineEvent, MiniTickerEvent, PartialDepthEvent, Payload, TickerEvent, TradeEvent, Update};`, `pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};`
  - `pub const SPOT_WS_BASE: &str = "wss://stream.binance.com:9443";`
  - `pub const MAX_STREAMS_PER_CONNECTION: usize = 1024;`, `pub const MAX_NAMES_PER_REQUEST: usize = 1024;`, `pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(500);`, each documented with its measurement from `OBSERVED.md`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum StreamPath { Stream }`, exhaustive, with `pub const ALL: &'static [Self] = &[Self::Stream];`, `pub fn as_str(self) -> &'static str` (`"stream"`), `pub(crate) fn url(self, base: &str) -> String` (`format!("{}/stream", base.trim_end_matches('/'))`) and `Display` as `as_str`.
- `src/spot/ws/stream.rs`:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum DepthLevels { Five, Ten, Twenty }` with `pub const ALL`;
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum DepthSpeed { Ms100, Ms1000 }` with `pub const ALL`: `Ms100` spells `@100ms`, `Ms1000` nothing (the bare name);
  - `#[derive(Debug, Clone, PartialEq, Eq, Hash)] #[non_exhaustive] pub enum StreamName { AggTrade(Symbol), Trade(Symbol), Kline(Symbol, Interval), MiniTicker(Symbol), AllMiniTickers, Ticker(Symbol), BookTicker(Symbol), PartialDepth(Symbol, DepthLevels, DepthSpeed), AvgPrice(Symbol) }`, `Symbol` and `Interval` being `crate::spot::types`'; `pub fn path(&self) -> StreamPath`, `pub fn symbol(&self) -> Option<&Symbol>`; `Display` the wire name (§5.9's table, the symbol's ASCII letters lowercased); `FromStr<Err = InvalidStreamName>` the inverse, refusing an ASCII uppercase letter in the symbol;
  - `#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] pub struct InvalidStreamName(pub String);`, `Classify` `InvalidRequest`.
- `src/spot/ws/event.rs`, every struct `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] #[non_exhaustive]` with `pub` fields, money `Decimal` through `rust_decimal::serde::str`, each field renamed to its wire key:
  - `AggTradeEvent { event_type: String "e", event_time: u64 "E", symbol: String "s", id: u64 "a", price: Decimal "p", quantity: Decimal "q", first_trade_id: u64 "f", last_trade_id: u64 "l", trade_time: u64 "T", is_buyer_maker: bool "m" }`;
  - `TradeEvent { event_type "e", event_time "E", symbol "s", id: u64 "t", price "p", quantity "q", trade_time "T", is_buyer_maker "m" }`;
  - `KlineEvent { event_type "e", event_time "E", symbol "s", kline: KlineBar "k" }`; `KlineBar { open_time: u64 "t", close_time: u64 "T", symbol: String "s", interval: Interval "i", first_trade_id: i64 "f", last_trade_id: i64 "L", open "o", close "c", high "h", low "l", volume "v", trade_count: u64 "n", is_closed: bool "x", quote_volume "q", taker_buy_base_volume "V", taker_buy_quote_volume "Q" }`;
  - `MiniTickerEvent { event_type "e", event_time "E", symbol "s", close_price "c", open_price "o", high_price "h", low_price "l", volume "v", quote_volume "q" }`;
  - `TickerEvent { event_type "e", event_time "E", symbol "s", price_change "p", price_change_percent "P", weighted_avg_price "w", prev_close_price "x", last_price "c", last_quantity "Q", bid_price "b", bid_quantity "B", ask_price "a", ask_quantity "A", open_price "o", high_price "h", low_price "l", volume "v", quote_volume "q", open_time: u64 "O", close_time: u64 "C", first_trade_id: i64 "F", last_trade_id: i64 "L", trade_count: u64 "n" }`;
  - `BookTickerEvent { update_id: u64 "u", symbol: String "s", bid_price "b", bid_quantity "B", ask_price "a", ask_quantity "A" }`;
  - `PartialDepthEvent { symbol: String, last_update_id: u64 "lastUpdateId", bids: Vec<Level>, asks: Vec<Level> }`, `Level` being `crate::spot::types::Level`, `symbol` the envelope's symbol uppercased as `Symbol` renders it, `#[serde(skip)]`;
  - `AvgPriceEvent { event_type "e", event_time "E", symbol "s", interval: String "i", price: Decimal "w", last_trade_time: u64 "T" }`;
  - `#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] pub enum Payload { AggTrade(AggTradeEvent), Trade(TradeEvent), Kline(KlineEvent), MiniTicker(MiniTickerEvent), MiniTickers(Vec<MiniTickerEvent>), Ticker(TickerEvent), BookTicker(BookTickerEvent), PartialDepth(PartialDepthEvent), AvgPrice(AvgPriceEvent), Unknown { event_type: String, raw: String } }`, serialised as the wire's `data`;
  - `#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] pub struct Update { pub stream: StreamName, pub payload: Payload }` with `pub fn from_json(text: &str) -> Result<Self, SpotWsError>` and `Serialize` as `{"stream": <name>, "data": <payload>}`.
- `src/spot/ws/error.rs`: `#[derive(thiserror::Error, Debug)] #[non_exhaustive] pub enum SpotWsError { Connect(#[source] polyoxide_ws::WsError), ConnectTimeout(Duration), Closed { code: Option<u16>, reason: String }, Refused { code: i64, msg: String }, NoAnswer { id: u64, timeout: Duration }, Response { id: u64, raw: String }, TooManyStreams { path: StreamPath, limit: usize }, Frame { stream: String, raw: String, reason: String }, Stopped { cause: Option<StopCause> } }`, with `UsdmWsError`'s messages; `#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] pub struct StopCause { pub class: Class, pub is_fault: bool, pub retry_after: Option<Duration>, pub message: String }` with `pub fn of(err: &SpotWsError) -> Self`; `From<WsError>`; `Classify` with `UsdmWsError`'s rows, its handshake `403` → `Restricted` included, the file expanding `polyoxide_venue::impl_ws_classification!()`.
- `src/spot/ws/fixtures.rs`: `pub const` `AGG_TRADE`, `TRADE`, `KLINE`, `MINI_TICKER`, `ALL_MINI_TICKERS`, `TICKER`, `BOOK_TICKER`, `PARTIAL_DEPTH` (`stream_btcusdt_depth5.json`), `PARTIAL_DEPTH_100MS` (`stream_btcusdt_depth20_100ms.json`), `AVG_PRICE` and `NON_ASCII_MINI_TICKER`, each `include_str!` of Story 1's file, and `pub const ALL: &[(&str, &str)]`, each with its fixture's file stem.
- `src/lib.rs`'s `const _` `Classify` assertion gains `spot::ws::SpotWsError` and `spot::ws::InvalidStreamName` under `#[cfg(feature = "spot-ws")]`.
- `tests/wire_agreement_spot_ws.rs` (`required-features = ["spot-ws", "test-server"]`) holds every fixture to its type by `wire_agreement_usdm_ws.rs`'s rules, its `IGNORED` naming `/data/M` on the trade and aggregate-trade fixtures and `/data/k/B` on the kline.
- Story 3's rules text: `.claude/rules/ci-gates.md`'s `polyoxide-binance` dependency rule says what Task 3.6's `test_dep_allowlists.py` rule says (the socket stack only with `usdm-ws` or `spot-ws`, the `@spot-ws` file, what it must hold); `.claude/rules/wire-enums.md`'s `paths:` gain `polyoxide-binance/src/spot/ws/mod.rs` and its hand-written list reads "Binance's `StreamPath` (`usdm/ws/mod.rs`, `spot/ws/mod.rs`)". `test_dep_allowlists.py`'s `SOCKETS` message changes with the rule's.
- Texts Story 4 quotes from Story 3, which no Story 3 fixer changes: in `src/spot/ws/mod.rs`, `pub mod error;`, `pub mod stream;`, `pub use error::{SpotWsError, StopCause};`, the `pub use stream::{…};` line, and `StreamPath::url`'s comment, `cfg_attr` and `fn` lines; in `Cargo.toml`, the `spot-ws` feature line and the `wire_agreement_spot_ws` `[[test]]` stanza. Story 4 replaces Story 3's `//!` module docs whole, by shape (every leading `//!` line), not by quote.
- `StreamPath::url` is `pub(crate)` and nothing calls it before Story 4, so Story 3 puts `#[cfg_attr(not(test), allow(dead_code))]` on it; Story 4's `SpotStreams::url` calls it, and Story 4 removes that attribute.

**Story 4 → Stories 5 and 6.** In `polyoxide-binance`'s `spot::ws`:

- `src/spot/ws/protocol.rs` (private): `pub(crate) struct SpotStreams;` with `impl StreamSet for SpotStreams` (`PATHS = StreamPath::ALL`, `MAX_NAMES_PER_REQUEST`, `MIN_REQUEST_INTERVAL`, `StreamPath::url`, `Update::from_json`), `#[cfg(test)] pub(crate) type SpotProtocol = BinanceProtocol<SpotStreams>;` only if a spot unit test names it (never under `allow(dead_code)`), and `impl SocketError for SpotWsError` (in `error.rs` or here; its `shape` never returns `WrongPath`).
- `src/spot/ws/client.rs`: `pub const CONNECT_TIMEOUT: Duration` and `pub const ANSWER_TIMEOUT: Duration`, 10 s each; `pub struct SpotWs` over `Bare<SpotStreams>`, with `pub async fn connect(streams: impl IntoIterator<Item = StreamName>) -> Result<Self, SpotWsError>`, `pub async fn connect_to(base_url: &str, streams: impl IntoIterator<Item = StreamName>) -> Result<Self, SpotWsError>`, `subscribe(&mut self, streams: &[StreamName])`, `unsubscribe(&mut self, streams: &[StreamName])`, `list_subscriptions(&mut self) -> Result<Vec<String>, SpotWsError>`, `ping(&mut self) -> Result<Duration, SpotWsError>`, `close(&mut self) -> Result<(), SpotWsError>`, `streams(&self) -> &[StreamName]`, and `Stream<Item = Result<Update, SpotWsError>>`.
- `src/spot/ws/supervised.rs`: `#[derive(Debug)] #[non_exhaustive] pub enum Event { Update(Box<Update>), Disconnected { path: StreamPath, reason: DisconnectReason }, Reconnected { path: StreamPath } }`; `#[derive(Debug)] #[non_exhaustive] pub enum DisconnectReason { Closed { code: Option<u16>, reason: String }, Error(SpotWsError), Stale, Rotation }` with `usdm`'s `Display` texts; `#[derive(Debug, Clone)] pub struct SpotWsBuilder { base_url: String, timing: Timing, streams: Vec<StreamName> }` with `new()` (§5.9's defaults), `Default`, `base_url(url)`, `polyoxide_ws::timing_setters!`, `streams(streams)` and `async connect(self) -> Result<SupervisedSpotWs, SpotWsError>`; `pub struct SupervisedSpotWs` with `membership(&self) -> MembershipHandle`, `async close(self) -> Result<(), SpotWsError>` and `Stream<Item = Result<Event, SpotWsError>>`; `#[derive(Debug, Clone)] pub struct MembershipHandle` with `async subscribe(&self, streams: impl IntoIterator<Item = StreamName>) -> Result<(), SpotWsError>` and `async unsubscribe` likewise.
- `src/spot/ws/mod.rs` gains `pub mod client; mod protocol; pub mod supervised;`, `#[cfg(test)] #[path = "../../usdm/ws/scripted.rs"] mod scripted;`, `pub use client::SpotWs;` and `pub use supervised::{DisconnectReason, Event, MembershipHandle, SpotWsBuilder, SupervisedSpotWs};`.
- `Cargo.toml`: `[package.metadata.polyoxide.live.live_spot_ws]` with `suite = "spot"`, `timeout = 15`, `features = ["spot-ws"]`, `secrets = []`; `identifiers` gains `stream.binance.com`; docs.rs `features` gains `spot-ws`; `[[test]]` `supervision_spot` (`["spot-ws", "test-server"]`) and `live_spot_ws` (`["spot-ws"]`).
- `tests/supervision_spot.rs` and `tests/live_spot_ws.rs` (whose drift test is `live_frames_carry_no_unmodelled_keys`) include `../src/usdm/ws/scripted.rs` with `#[path]` where they script a server.
- `docs/MUTANTS.md`'s socket-pace row: the line of `spot::ws::MIN_REQUEST_INTERVAL`, the mutation `Duration::from_millis(250)`, and the tests that fail under it.
- The root `Cargo.toml`'s `[workspace.metadata.polyoxide.mirrors.binance]`, which feeds `docs/specs/INDEX.md` and CLAUDE.md's regions: `base_urls = ["https://fapi.binance.com", "https://api.binance.com", "wss://fstream.binance.com", "wss://stream.binance.com:9443"]`, `description = "Futures and spot public market data, and the futures and spot market streams. No published spec, so not a mirror"`, `crate_note = "streams behind `usdm-ws` and `spot-ws`"`; `name`, `section` and `exclude` unchanged. `[workspace.metadata.polyoxide] stage` is not touched.
- `polyoxide-binance/Cargo.toml`'s texts, which `gen_registry.py --write` carries into the generated regions: `description = "Rust client library for Binance USDⓈ-M futures and spot market data and streams"`; `[package.metadata.polyoxide]`'s `readme = "Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`) and `spot` (spot market data, with its streams under `spot-ws`); re-exported by the umbrella as `polyoxide::binance`"`; and the docs.rs comment above `[package.metadata.docs.rs]` names both modules' streams.

**Story 5 → Story 6.**

- `polyoxide_cli::commands::binance::spot`: `#[derive(Subcommand)] pub enum SpotCommand { Stream { #[command(flatten)] args: SpotArgs } }` with `pub async fn run(self) -> Result<()>`; `#[derive(Args, Debug)] pub struct SpotArgs { pub symbol: Vec<String>, pub kind: Vec<String>, pub all_mini_tickers: bool, pub format: OutputFormat, pub count: Option<u64>, pub timeout: Option<Duration> }` with `usdm::BinanceArgs`'s flag spellings and parsers, `--all-mini-tickers` for `!miniTicker@arr`, and the kinds of spec §5.12; `pub async fn run(args: SpotArgs) -> Result<()>`; `pub async fn run_with<S>(args: SpotArgs, events: S, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> where S: Stream<Item = Result<Event, SpotWsError>> + Unpin`.
- `BinanceCommand` gains `/// Spot market data` `Spot { #[command(subcommand)] command: spot::SpotCommand }`, so the command is `polyoxide binance spot stream`.
- The umbrella: `binance-spot-ws = ["binance-spot", "polyoxide-binance/spot-ws"]`, `full` gaining it; the prelude names and passthrough checks of spec §5.11.
- `polyoxide-cli`'s texts: `Cargo.toml`'s `description = "Command-line client for the Polymarket APIs and streams, and Binance USDⓈ-M and spot market data"` and `[package.metadata.polyoxide]`'s `readme = "CLI tool for querying Polymarket APIs and Binance USDⓈ-M and spot market data"`; `src/main.rs`'s `about = "CLI tool for Polymarket's APIs and streams, and Binance USDⓈ-M and spot market data"` and the `Binance` command's doc `/// Binance: USDⓈ-M futures and spot market data`; `commands/binance/mod.rs`'s `Spot` doc `/// Spot market data`. Story 6 brings `polyoxide-cli/README.md` into line: its opening sentence, a `binance spot` row in the command table beside `binance usdm`, and a spot stream section after the USDⓈ-M one.

**Design stage:** /flow-draft session 605d5e20-f300-4879-bb6c-61d238166557, 2026-11-02T13:16:06Z to 2026-11-02T13:46:27Z UTC: $18.29 logged, $18.40 with estimated thinking; 10464 plan lines, $1.75 per 1,000 lines.

## Stories

- **Story 1** — Frame captures (spec §3.2, §5.9, §5.13).

  Adds a `ws` subcommand to `scripts/capture_binance_spot.py` that opens one connection to `wss://stream.binance.com:9443/stream` through `capture_common.ws_session`, sends one `SUBSCRIBE` of every stream the names contract lists (the non-ASCII symbol found through `exchangeInfo`), keeps the first envelope of each, trims as listed, writes nothing until all have arrived, and prints `PROVENANCE.md`'s `## Streams (ws/)` section. Runs it and appends that section. One request on one connection: far under spot's 5 messages a second. No Rust.

  - **Developer:** intermediate-dev
  - **Files:** `scripts/capture_binance_spot.py`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_aggTrade.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_trade.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_kline_1m.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_miniTicker.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_all_miniTicker_arr.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_ticker.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_bookTicker.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_depth5.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_depth20_100ms.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_avgPrice.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_non_ascii_miniTicker.json`, `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`
  - **Interfaces:** exposes the fixtures, the `ws` capture command and the `## Streams (ws/)` section under "Story 1 → Stories 3 to 5"; uses nothing from earlier stories
  - **Verify:** `python3 -c "import glob, json; files = glob.glob('polyoxide-binance/tests/fixtures/spot/ws/*.json'); assert len(files) == 11, files; [json.load(open(f, encoding='utf-8')) for f in files]" && grep -q '^## Streams (ws/)' polyoxide-binance/tests/fixtures/spot/PROVENANCE.md && python3 scripts/capture_binance_spot.py ws --help`
  - **Review:** light (a capture script and its captures; no library code)
  - **Reading:** spec §3.2, §5.9 (the stream table), §5.13; `scripts/capture_binance_spot.py`, `scripts/capture_binance_usdm.py:73-175`, `scripts/capture_common.py` (`ws_session` and `write_json`), `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`, `polyoxide-binance/tests/fixtures/usdm/PROVENANCE.md:21-32`; `docs/specs/binance/OBSERVED.md` (`### Frames`)

- **Story 2** — Shared socket plumbing under `usdm::ws` (spec §5.2, §5.9).

  Moves `usdm::ws`'s Binance-wide machinery into `src/shared/ws/`, generic over `StreamSet` and `SocketError` as the names contract gives them, and rebuilds `usdm::ws` on it: `UsdmStreams` and `impl SocketError for UsdmWsError` describe USDⓈ-M, and `UsdmWs`, `SupervisedUsdmWs`, `MembershipHandle` and `UsdmWsBuilder` wrap the generic types, with every public doc, path, name and signature as it was. Behaviour is unchanged line for line: the recovery rows, the record-before-send, the batched and paced replay, the refused batch undone, the router's stop cause, the forwarding queue, the rotation constant. No test file and no test module changes; `UsdmProtocol` and `Change` (as `#[cfg(test)]` aliases), `UsdmWs::open`, `UsdmWs::close_frame` and the builder's fields stay where the unit tests reach them; `Answer`, which no test names, moves to `shared::ws::request`. `supervision_usdm`, `supervision_usdm_edges`, `supervision_usdm_kit` and `wire_agreement_usdm_ws` pass unchanged. `.claude/rules/binance.md`'s sentences naming `src/usdm/ws/protocol.rs` name `src/shared/ws/protocol.rs` in the same commit.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/ws/mod.rs`, `polyoxide-binance/src/shared/ws/request.rs`, `polyoxide-binance/src/shared/ws/bare.rs`, `polyoxide-binance/src/shared/ws/protocol.rs`, `polyoxide-binance/src/shared/ws/supervised.rs`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `.claude/rules/binance.md`
  - **Interfaces:** exposes "Story 2 → Story 4"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_claude_rules.py -q'`
  - **Review:** deep (a refactor under a public socket API that prader-rs folds outages on)
  - **Reading:** spec §2 (the moved-tests row), §5.2, §5.9 (from "`shared::ws` holds"); `polyoxide-binance/src/usdm/ws/mod.rs`, `polyoxide-binance/src/usdm/ws/client.rs:1-352`, `polyoxide-binance/src/usdm/ws/protocol.rs:1-333`, `polyoxide-binance/src/usdm/ws/supervised.rs:1-749`, `polyoxide-binance/src/usdm/ws/error.rs:1-214`, and the `use` lines of each file's tests module; `polyoxide-ws/src/supervisor/mod.rs:100-330` (the `Protocol` trait); `polyoxide-binance/src/shared/mod.rs`; `scripts/test_body_diff.py:1-90`; `.claude/rules/binance.md` (`## Streams (usdm-ws)`)

- **Story 3** — Spot stream names, payloads and errors (spec §3.2, §5.9, §5.10).

  Adds the `spot-ws` feature and `spot::ws`'s data half: `StreamPath`, the constants, `StreamName` with its `Display` and `FromStr` round trip, `DepthLevels` and `DepthSpeed`, the payload types and `Update::from_json` (a partial-depth frame's symbol from the envelope; `Payload::Unknown` for a single-symbol object whose `e` is not its stream's), `SpotWsError` with `StopCause`, `From<WsError>` and its `Classify` rows, and `fixtures`. Unit tests beside each type cover the name round trip (the non-ASCII fixture's echoed name included), every refused spelling (an uppercase symbol, `@250ms`, `!ticker@arr`), the envelope symbol, `Unknown`, and every error's class. `wire_agreement_spot_ws` holds each fixture to its type. The dep allowlists gain `polyoxide-binance@spot-ws.txt`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/src/spot/ws/stream.rs`, `polyoxide-binance/src/spot/ws/event.rs`, `polyoxide-binance/src/spot/ws/error.rs`, `polyoxide-binance/src/spot/ws/fixtures.rs`, `polyoxide-binance/tests/wire_agreement_spot_ws.rs`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide-binance@spot-ws.txt`, `.github/scripts/tests/test_classify_coverage.py`, `.claude/rules/ci-gates.md`, `.claude/rules/wire-enums.md`, `Cargo.lock`
  - **Interfaces:** exposes "Story 3 → Stories 4 and 5"; uses Story 1's fixtures and plan 1's `spot::types::{Symbol, Interval, Level}`
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py -q'`
  - **Review:** deep (public payload types every consumer decodes into, and an error's classes)
  - **Reading:** spec §3.2, §5.9, §5.10; `docs/specs/binance/OBSERVED.md` (`### Stream answers`, `### Frames`); `polyoxide-binance/src/usdm/ws/stream.rs:1-191`, `polyoxide-binance/src/usdm/ws/event.rs:1-487`, `polyoxide-binance/src/usdm/ws/error.rs:1-214`, `polyoxide-binance/src/usdm/ws/fixtures.rs`, `polyoxide-binance/tests/wire_agreement_usdm_ws.rs`, `polyoxide-binance/src/spot/types.rs:300-430`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/Cargo.toml`, `.github/scripts/tests/test_dep_allowlists.py:100-160,275-290`

- **Story 4** — Spot socket clients, registration and the live suite (spec §5.9, §5.10, §7).

  Widens `shared::ws`'s `cfg` and adds `SpotStreams`, `SpotWs`, `SpotWsBuilder`, `SupervisedSpotWs`, `MembershipHandle`, `Event` and `DisconnectReason` on the generic code, with module docs and examples that compile under `spot-ws`. Unit tests in `client.rs` and `supervised.rs` and the `supervision_spot` suite run on the scripted server (`src/usdm/ws/scripted.rs` by `#[path]`): one path, `/stream`; a request carrying all 1024 names; requests at least 500 ms apart; the 1025th stream refused with nothing sent; `Disconnected` then `Reconnected`; rotation; `Stopped { cause }` after a refused replay; a refused handshake's `Retry-After`. `live_spot_ws` subscribes to every kind on production at the client's pace and runs `live_frames_carry_no_unmodelled_keys`. Registers the live entry and identifier, runs `gen_registry.py --write`, and adds the socket-pace row to `docs/MUTANTS.md`, proved by applying the mutation and seeing its named tests fail.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/ws/mod.rs`, `polyoxide-binance/src/shared/ws/bare.rs`, `Cargo.toml`, `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/src/spot/ws/protocol.rs`, `polyoxide-binance/src/spot/ws/client.rs`, `polyoxide-binance/src/spot/ws/supervised.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/supervision_spot.rs`, `polyoxide-binance/tests/live_spot_ws.rs`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `scripts/live_unwraps.baseline.json`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`, `Cargo.lock`
  - **Interfaces:** exposes "Story 4 → Stories 5 and 6"; uses Story 2's `shared::ws` items and Story 3's `spot::ws` types and fixtures
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && cargo test -p polyoxide-binance --no-default-features --features spot-ws --test live_spot_ws -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py`
  - **Review:** deep (a supervised socket that must never pass spot's message limit on a shared address)
  - **Reading:** spec §5.9, §5.10, §7; `docs/specs/binance/OBSERVED.md` (`### Stream limits`, `### Pace at 500 ms`); the `shared::ws` files Story 2 wrote; `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs:1-120` (as Story 2 left them); `polyoxide-binance/src/usdm/ws/scripted.rs`; `polyoxide-binance/tests/supervision_usdm.rs:1-140`, `polyoxide-binance/tests/live_usdm_ws.rs`; `polyoxide-binance/Cargo.toml` (the `[package.metadata.polyoxide]` tables); `docs/MUTANTS.md` (row (l)'s format and the paragraphs on proving)

- **Story 5** — `polyoxide binance spot stream` and the umbrella's `binance-spot-ws` (spec §5.11, §5.12).

  Adds `commands/binance/spot.rs` in `usdm.rs`'s shape: `SpotCommand`, `SpotArgs` with spec §5.12's kinds and `--all-mini-tickers`, `run` on the supervised tier, `run_with` over any event stream, and a `Feed` whose `pretty` prints a line per update and per `!miniTicker@arr` row and whose `json` prints each frame's envelope; outage markers and a skipped frame go to stderr. `polyoxide-cli` builds `polyoxide-binance` with `usdm-ws` and `spot-ws` (and `test-server` in dev-dependencies). New tests appended to `tests/ws_binance.rs` drive every flag on `spot::ws::fixtures`. The umbrella gains `binance-spot-ws`, `full` gaining it, the crate docs' feature list, and the prelude names and passthrough checks of spec §5.11. `gen_registry.py --write` regenerates the regions that list crate features and needs.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-cli/Cargo.toml`, `polyoxide-cli/src/main.rs`, `polyoxide-cli/src/commands/binance/mod.rs`, `polyoxide-cli/src/commands/binance/spot.rs`, `polyoxide-cli/tests/ws_binance.rs`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `CLAUDE.md`, `README.md`, `docs/ARCHITECTURE.md`, `Cargo.lock`
  - **Interfaces:** exposes "Story 5 → Story 6"; uses Story 3's types and fixtures and Story 4's `SpotWsBuilder`, `Event` and `SpotWsError`
  - **Verify:** `cargo test -p polyoxide-cli --all-features && cargo test -p polyoxide --all-features && cargo hack check -p polyoxide -p polyoxide-cli --each-feature --no-dev-deps && python3 scripts/gen_registry.py --check`
  - **Review:** deep (a new CLI command and the umbrella's public surface)
  - **Reading:** spec §5.11, §5.12; `polyoxide-cli/src/commands/binance/mod.rs`, `polyoxide-cli/src/commands/binance/usdm.rs`, `polyoxide-cli/src/commands/stream.rs:1-120`, `polyoxide-cli/tests/ws_binance.rs`, `polyoxide-cli/Cargo.toml`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs:1-60,120-140,260-360`

- **Story 6** — Docs (spec §5.13).

  Adds spot's streams to `docs/specs/binance/INDEX.md` (the host row and a `## Spot streams covered` table, with `live_spot_ws`'s drift detector), a streams paragraph to `src/spot/README.md` (prose only: its fences compile under `spot`), `spot-ws` to the crate README and `src/lib.rs`'s crate docs, `binance-spot-ws` to the umbrella and root READMEs, the CLI's spot command to `polyoxide-cli/README.md` and the hand-written lines of `CLAUDE.md`. In the rules: `.claude/rules/binance.md` gains `shared::ws` in its `shared` section and a `## Streams (spot-ws)` section (one path, 1024 names a request, 500 ms, the scripted file both products include); `sockets.md` says Binance's protocol is `shared::ws`'s, for both products; `errors.md` lists `SpotWsError::Connect`; `cli.md` gains `binance spot stream`; `umbrella.md` gains `binance-spot-ws` and its prelude names; `live-tests.md`'s wrap-first constructors gain `SpotWsError::from(WsError::from(..))`.

  - **Developer:** intermediate-dev
  - **Files:** `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `polyoxide-cli/README.md`, `README.md`, `CLAUDE.md`, `.claude/rules/binance.md`, `.claude/rules/sockets.md`, `.claude/rules/errors.md`, `.claude/rules/cli.md`, `.claude/rules/umbrella.md`, `.claude/rules/live-tests.md`
  - **Interfaces:** uses every earlier story's names; exposes none
  - **Verify:** `cargo test --doc -p polyoxide-binance --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q' && bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features'`
  - **Review:** light (docs and rules; no library code)
  - **Reading:** spec §5.13; `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/spot/README.md`, `.claude/rules/binance.md`, `.claude/rules/sockets.md`, `.claude/rules/errors.md` (`## Socket errors`), `.claude/rules/cli.md`, `.claude/rules/umbrella.md`

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->

## Story 1: Frame captures

- **Developer:** intermediate-dev
- **Files:** `scripts/capture_binance_spot.py`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_aggTrade.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_trade.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_kline_1m.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_miniTicker.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_all_miniTicker_arr.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_ticker.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_bookTicker.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_depth5.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_depth20_100ms.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_avgPrice.json`, `polyoxide-binance/tests/fixtures/spot/ws/stream_non_ascii_miniTicker.json`, `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`
- **Interfaces:** exposes the fixtures, the `ws` capture command and the `## Streams (ws/)` section under "Story 1 → Stories 3 to 5"; uses nothing from earlier stories
- **Verify:** `python3 -c "import glob, json; files = glob.glob('polyoxide-binance/tests/fixtures/spot/ws/*.json'); assert len(files) == 11, files; [json.load(open(f, encoding='utf-8')) for f in files]" && grep -q '^## Streams (ws/)' polyoxide-binance/tests/fixtures/spot/PROVENANCE.md && python3 scripts/capture_binance_spot.py ws --help`
- **Review:** light (a capture script and its captures; no library code)

This story adds a `ws` subcommand to `scripts/capture_binance_spot.py`, beside `rest`, and runs it once against production to capture the eleven spot stream frames that Stories 3 to 5 decode and replay. The command opens one connection to `wss://stream.binance.com:9443/stream` through `capture_common.ws_session`, sends one `SUBSCRIBE` of every stream the names contract lists, keeps the first envelope of each, trims as listed, writes nothing until all eleven have arrived (within 30 s), and prints `PROVENANCE.md`'s `## Streams (ws/)` section. The non-ASCII stream is found through `GET /api/v3/exchangeInfo`: the first `TRADING` symbol with a non-ASCII character, its ASCII letters lowercased. The capture sends one request on one connection, far under spot's 5 messages a second, and one `exchangeInfo` (weight 20). No Rust and none of the profile's landmine markers: the script uses Python's `websockets`, not the kit.

### Task 1.1: The `ws` capture command

**Files:**
- Modify: `scripts/capture_binance_spot.py` (the header and imports at the top, the constants after `INVALID`, new functions before `main`, and `main`)

- [ ] **Step 1: Write the failing check**

The script has no test harness (the Rust wire-agreement suite of Story 3 is what holds the captured frames to their types), so the check is a throwaway one over the helpers' pure parts, typed at the shell and not committed. It holds the stream list to the names contract, and the failure paths: no `TRADING` symbol with a non-ASCII character, and an array stream with too few rows to trim.

```bash
PYTHONPATH=scripts python3 - <<'EOF'
import capture_binance_spot as c

assert c.wire_symbol("币安人生USDT") == "币安人生usdt"
assert c.wire_symbol("BTCUSDT") == "btcusdt"

info = {"symbols": [
    {"symbol": "币安人生OLD", "status": "BREAK"},
    {"symbol": "BTCUSDT", "status": "TRADING"},
    {"symbol": "币安人生USDT", "status": "TRADING"},
]}
assert c.trading_non_ascii(info) == "币安人生USDT"
try:
    c.trading_non_ascii({"symbols": info["symbols"][:2]})
except SystemExit as err:
    assert "non-ASCII" in str(err), err
else:
    raise AssertionError("a listing with no TRADING non-ASCII symbol must end the run")

streams = c.stream_list("币安人生USDT")
assert [f for f, _, _ in streams] == [
    "stream_btcusdt_aggTrade.json",
    "stream_btcusdt_trade.json",
    "stream_btcusdt_kline_1m.json",
    "stream_btcusdt_miniTicker.json",
    "stream_all_miniTicker_arr.json",
    "stream_btcusdt_ticker.json",
    "stream_btcusdt_bookTicker.json",
    "stream_btcusdt_depth5.json",
    "stream_btcusdt_depth20_100ms.json",
    "stream_btcusdt_avgPrice.json",
    "stream_non_ascii_miniTicker.json",
]
assert [n for _, n, _ in streams] == [
    "btcusdt@aggTrade",
    "btcusdt@trade",
    "btcusdt@kline_1m",
    "btcusdt@miniTicker",
    "!miniTicker@arr",
    "btcusdt@ticker",
    "btcusdt@bookTicker",
    "btcusdt@depth5",
    "btcusdt@depth20@100ms",
    "btcusdt@avgPrice",
    "币安人生usdt@miniTicker",
]

rows = [{"s": "A"}, {"s": "B"}, {"s": "C"}]
assert c.trim("rows", rows, "x") == rows[:2]
for short in ([], rows[:1]):
    try:
        c.trim("rows", short, "x")
    except SystemExit:
        pass
    else:
        raise AssertionError("an array of fewer than two rows must end the run")
depth = {"lastUpdateId": 1, "bids": [["1", "1"]] * 5, "asks": [["2", "2"]] * 5}
cut = c.trim("levels", depth, "x")
assert cut["bids"] == depth["bids"][:3] and cut["asks"] == depth["asks"][:3]
assert list(cut) == list(depth), "trimming keeps the key order"
assert c.trim(None, depth, "x") is depth
print("ok")
EOF
```

- [ ] **Step 2: Run it to see it fail**

Run the block above. Expected: it stops at its first line that touches a new helper,

```
AttributeError: module 'capture_binance_spot' has no attribute 'wire_symbol'
```

and the command itself is unknown:

```bash
python3 scripts/capture_binance_spot.py ws --help
```

prints `usage: capture_binance_spot.py [-h] {rest} ...` and `error: argument command: invalid choice: 'ws'`, with status 2.

- [ ] **Step 3: Implement**

Edit `scripts/capture_binance_spot.py`. Run `ws` with `uv run`, which reads the inline dependencies; `rest` and `ws --help` still run under plain `python3`, because `websockets` is imported inside `read_streams`, as `capture_common.ws_session` imports it, and not at load time.

(a) The header. Replace

```python
# dependencies = ["certifi"]
# ///
"""Capture Binance spot REST responses as test fixtures for polyoxide-binance's `spot` module.

Usage: python3 scripts/capture_binance_spot.py rest [OUT_DIR]
       (or `uv run scripts/capture_binance_spot.py rest [OUT_DIR]`)

OUT_DIR defaults to polyoxide-binance/tests/fixtures/spot. Writes one JSON file per route
under OUT_DIR/rest/ and nothing else; it does not write PROVENANCE.md. It prints
PROVENANCE.md's `## REST (rest/)` section to stdout, and progress to stderr. Nothing is
written until every capture has succeeded, so a failed run leaves the fixtures as they
were. Every key is kept; only list lengths are trimmed, so a wire-agreement test sees each
field the server sends. No credentials. Costs about 180 request weight, well inside the
6000 per minute. Review the diff before committing.

Run it as a file, never `python3 -I`, which keeps scripts/ off sys.path.
"""
```

with

```python
# dependencies = ["websockets>=13", "certifi"]
# ///
"""Capture Binance spot REST responses and stream frames as test fixtures for
polyoxide-binance's `spot` module.

Usage: python3 scripts/capture_binance_spot.py rest [OUT_DIR]
       uv run scripts/capture_binance_spot.py ws [OUT_DIR]
       (`rest` needs only the standard library, so `python3` runs it; `ws` imports
       `websockets`, so run it with `uv run`)

OUT_DIR defaults to polyoxide-binance/tests/fixtures/spot. `rest` writes one JSON file per
route under OUT_DIR/rest/, and `ws` one combined-stream envelope per stream under OUT_DIR/ws/;
neither writes anything else, and neither writes PROVENANCE.md. Each prints PROVENANCE.md's
section for what it captured (`## REST (rest/)`, `## Streams (ws/)`) to stdout, and progress
to stderr. Nothing is written until every capture has succeeded, so a failed run leaves the
fixtures as they were. Every key is kept; only list lengths are trimmed, so a wire-agreement
test sees each field the server sends. No credentials. `rest` costs about 180 request weight,
well inside the 6000 per minute. `ws` costs 20 (one `exchangeInfo`) and one WebSocket
connection that sends one request, far under spot's 5 messages a second. Review the diff
before committing.

Run it as a file, never `python3 -I`, which keeps scripts/ off sys.path.
"""
```

(b) The imports. Replace

```python
import argparse
import sys
import urllib.parse
from pathlib import Path

import capture_common
```

with

```python
import argparse
import asyncio
import json
import sys
import time
import urllib.parse
from pathlib import Path

import capture_common
```

(c) The constants. After the line

```python
INVALID = ("error_invalid_symbol.json", "/api/v3/exchangeInfo", {"symbol": "NOPEUSDT"})
```

add

```python

WS_BASE = "wss://stream.binance.com:9443"
# How long `ws` waits for the first frame of every stream, in seconds.
WS_WAIT = 30
# (file, wire name, how the frame is trimmed), in the order PROVENANCE.md lists them. The
# last name takes the non-ASCII symbol `exchangeInfo` lists, through `stream_list`.
STREAMS = [
    ("stream_btcusdt_aggTrade.json", "btcusdt@aggTrade", None),
    ("stream_btcusdt_trade.json", "btcusdt@trade", None),
    ("stream_btcusdt_kline_1m.json", "btcusdt@kline_1m", None),
    ("stream_btcusdt_miniTicker.json", "btcusdt@miniTicker", None),
    ("stream_all_miniTicker_arr.json", "!miniTicker@arr", "rows"),
    ("stream_btcusdt_ticker.json", "btcusdt@ticker", None),
    ("stream_btcusdt_bookTicker.json", "btcusdt@bookTicker", None),
    ("stream_btcusdt_depth5.json", "btcusdt@depth5", None),
    ("stream_btcusdt_depth20_100ms.json", "btcusdt@depth20@100ms", "levels"),
    ("stream_btcusdt_avgPrice.json", "btcusdt@avgPrice", None),
    ("stream_non_ascii_miniTicker.json", "{non_ascii}@miniTicker", None),
]
# What PROVENANCE.md's "Trimmed to" column says of each way a frame is trimmed.
TRIMMED = {
    None: "as fetched",
    "rows": "`data`: the first two rows",
    "levels": "`bids` and `asks`: the first three levels each",
}
```

(d) The stream helpers, before `def main`. Add

```python
def wire_symbol(symbol):
    """A symbol as a stream name spells it: ASCII letters lowercased, every other character
    as it is (`str.lower` would also rewrite some non-ASCII ones)."""
    return "".join(c.lower() if c.isascii() else c for c in symbol)


def trading_non_ascii(info):
    """The first `TRADING` symbol of an `exchangeInfo` answer with a non-ASCII character."""
    row = first(
        info["symbols"],
        lambda s: s["status"] == "TRADING" and not s["symbol"].isascii(),
        "TRADING symbol with a non-ASCII character",
    )
    return row["symbol"]


def stream_list(non_ascii):
    """`STREAMS` with the non-ASCII stream's name filled in from the symbol."""
    return [(file, name.format(non_ascii=wire_symbol(non_ascii)), how) for file, name, how in STREAMS]


def trim(how, data, name):
    """`data` trimmed the way `how` says; a frame too short to trim ends the run."""
    if how == "rows":
        if not isinstance(data, list) or len(data) < 2:
            sys.exit(f"{name}: expected an array of at least two rows to keep two of")
        return data[:2]
    if how == "levels":
        return {**data, "bids": data["bids"][:3], "asks": data["asks"][:3]}
    return data


async def read_streams(names):
    """Subscribes to `names` on one connection and reads until the first envelope of each has
    arrived or `WS_WAIT` seconds have passed. Returns the envelopes by stream name, the names
    still missing, every stream name seen, and the text of the answer to the `SUBSCRIBE`."""
    from websockets.exceptions import ConnectionClosed, InvalidHandshake

    url = f"{WS_BASE}/stream"
    found, wanted, seen, answer = {}, set(names), set(), None
    try:
        async with capture_common.ws_session(url, max_size=2**22, ping_interval=None) as ws:
            await ws.send(json.dumps({"method": "SUBSCRIBE", "params": names, "id": 1}))
            deadline = time.monotonic() + WS_WAIT
            while wanted:
                left = deadline - time.monotonic()
                if left <= 0:
                    break
                try:
                    message = await asyncio.wait_for(ws.recv(), left)
                except TimeoutError:
                    break
                if not isinstance(message, str):
                    continue
                frame = json.loads(message)
                if "stream" not in frame:
                    if "error" in frame or "code" in frame:
                        sys.exit(f"the SUBSCRIBE was refused: {message}")
                    if frame.get("id") == 1:
                        answer = message
                    continue
                name = frame["stream"]
                seen.add(name)
                if name in wanted:
                    wanted.discard(name)
                    found[name] = frame
    except InvalidHandshake as err:
        sys.exit(f"{url}: handshake refused: {err}")
    except ConnectionClosed as err:
        sys.exit(f"{url}: the server closed the connection: {err}")
    return found, wanted, seen, answer


def capture_ws():
    """Every fixture by file name, PROVENANCE.md's table rows, the non-ASCII symbol the
    non-ASCII stream is for, and the answer to the `SUBSCRIBE`."""
    symbol = trading_non_ascii(get("/api/v3/exchangeInfo"))
    log(f"  GET /api/v3/exchangeInfo -> {symbol}")
    streams = stream_list(symbol)
    names = [name for _, name, _ in streams]
    envelopes, missing, seen, answer = asyncio.run(read_streams(names))
    if missing:
        sys.exit(f"no frame within {WS_WAIT} s on {sorted(missing)}; frames came on "
                 f"{sorted(seen)}; nothing written, run again")
    files, rows = {}, []
    for file, name, how in streams:
        envelope = envelopes[name]
        envelope["data"] = trim(how, envelope["data"], name)
        files[file] = envelope
        rows.append([f"`{file}`", f"`{name}`", TRIMMED[how]])
        log(f"  {name}")
    return files, rows, symbol, answer or "(no answer read)"


def streams_section(today, rows, symbol, answer):
    """PROVENANCE.md's `## Streams (ws/)` section, from table rows of cells."""
    lines = [
        "## Streams (ws/)",
        "",
        f"Captured {today} from `{WS_BASE}/stream` by `scripts/capture_binance_spot.py ws`. "
        "No credentials. One connection and one `SUBSCRIBE` (`id` 1) of the "
        f"{len(rows)} names below, answered `{answer}`; the first combined-stream envelope "
        f"(`{{\"stream\", \"data\"}}`) of each, within {WS_WAIT} s, every key kept.",
        "",
    ]
    lines += capture_common.provenance_table(["File", "Stream", "Trimmed to"], rows, short_rule=True)
    lines += [
        "",
        f"The non-ASCII stream is for `{symbol}`, the first `TRADING` symbol in "
        "`GET /api/v3/exchangeInfo` with a non-ASCII character, its ASCII letters lowercased "
        f"in the stream name (`{wire_symbol(symbol)}@miniTicker`).",
    ]
    return "\n".join(lines) + "\n"


```

(e) `main`. Replace

```python
def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    rest = commands.add_parser("rest", help="capture the REST fixtures under OUT_DIR/rest/")
    rest.add_argument("out", nargs="?", type=Path, default=DEFAULT_OUT,
                      help="the fixtures directory (default: polyoxide-binance/tests/fixtures/spot)")
    args = parser.parse_args(argv)

    log(f"capturing from {BASE}")
    files, rows = capture_rest()
    for name, value in files.items():
        capture_common.write_json(args.out / "rest" / name, value, indent=1)
    log(f"wrote {len(files)} files under {args.out / 'rest'}")
    sys.stdout.reconfigure(encoding="utf-8")
    print(provenance_section(capture_common.stamp("%Y-%m-%d"), rows), end="")
```

with

```python
def run_rest(out):
    log(f"capturing from {BASE}")
    files, rows = capture_rest()
    for name, value in files.items():
        capture_common.write_json(out / "rest" / name, value, indent=1)
    log(f"wrote {len(files)} files under {out / 'rest'}")
    print(provenance_section(capture_common.stamp("%Y-%m-%d"), rows), end="")


def run_ws(out):
    log(f"capturing from {WS_BASE}/stream")
    files, rows, symbol, answer = capture_ws()
    for name, value in files.items():
        capture_common.write_json(out / "ws" / name, value, indent=1)
    log(f"wrote {len(files)} files under {out / 'ws'}")
    print(streams_section(capture_common.stamp("%Y-%m-%d"), rows, symbol, answer), end="")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    rest = commands.add_parser("rest", help="capture the REST fixtures under OUT_DIR/rest/")
    ws = commands.add_parser("ws", help="capture the stream fixtures under OUT_DIR/ws/")
    for command in (rest, ws):
        command.add_argument("out", nargs="?", type=Path, default=DEFAULT_OUT,
                             help="the fixtures directory (default: polyoxide-binance/tests/fixtures/spot)")
    args = parser.parse_args(argv)

    sys.stdout.reconfigure(encoding="utf-8")
    if args.command == "rest":
        run_rest(args.out)
    else:
        run_ws(args.out)
```

`rest` is otherwise unchanged: its constants, `capture_rest` and `provenance_section` are not touched.

- [ ] **Step 4: Run it to see it pass**

Run the block from Step 1 again. Expected output: `ok`.

```bash
python3 scripts/capture_binance_spot.py ws --help
```

Expected: the usage line `usage: capture_binance_spot.py ws [-h] [out]` and the `out` argument's help, status 0 (this needs no `websockets`). `python3 scripts/capture_binance_spot.py rest --help` prints the same shape with `rest`.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(binance): scripts/capture_binance_spot.py ws captures spot's stream frames (binance spot ws, Story 1)" -- scripts/capture_binance_spot.py
```

### Task 1.2: Capture the frames and write their provenance

**Files:**
- Create: `polyoxide-binance/tests/fixtures/spot/ws/stream_btcusdt_aggTrade.json`, `stream_btcusdt_trade.json`, `stream_btcusdt_kline_1m.json`, `stream_btcusdt_miniTicker.json`, `stream_all_miniTicker_arr.json`, `stream_btcusdt_ticker.json`, `stream_btcusdt_bookTicker.json`, `stream_btcusdt_depth5.json`, `stream_btcusdt_depth20_100ms.json`, `stream_btcusdt_avgPrice.json`, `stream_non_ascii_miniTicker.json` (all under `polyoxide-binance/tests/fixtures/spot/ws/`)
- Modify: `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md` (append the section after `## REST (rest/)`)

The fixtures are captured, never hand-written. The capture opens one connection to the production socket, which this host's address shares with the live suites and other sessions: run it once. If it fails with a refused handshake or a close (a 1008 "Too many requests" included), stop and report; do not loop. A run that only misses a stream within 30 s wrote nothing and may be run once more.

- [ ] **Step 1: Write the failing check**

The check is the story's **Verify** done-check, plus a shape check over what the capture must hold. Save the second as a throwaway, uncommitted block:

```bash
python3 - <<'EOF'
import glob, json

d = "polyoxide-binance/tests/fixtures/spot/ws/"


def load(name):
    with open(d + name, encoding="utf-8") as f:
        return json.load(f)


assert len(glob.glob(d + "*.json")) == 11
expected = {
    "stream_btcusdt_aggTrade.json": "btcusdt@aggTrade",
    "stream_btcusdt_trade.json": "btcusdt@trade",
    "stream_btcusdt_kline_1m.json": "btcusdt@kline_1m",
    "stream_btcusdt_miniTicker.json": "btcusdt@miniTicker",
    "stream_all_miniTicker_arr.json": "!miniTicker@arr",
    "stream_btcusdt_ticker.json": "btcusdt@ticker",
    "stream_btcusdt_bookTicker.json": "btcusdt@bookTicker",
    "stream_btcusdt_depth5.json": "btcusdt@depth5",
    "stream_btcusdt_depth20_100ms.json": "btcusdt@depth20@100ms",
    "stream_btcusdt_avgPrice.json": "btcusdt@avgPrice",
}
for file, stream in expected.items():
    envelope = load(file)
    assert list(envelope) == ["stream", "data"], (file, list(envelope))
    assert envelope["stream"] == stream, (file, envelope["stream"])
assert len(load("stream_all_miniTicker_arr.json")["data"]) == 2
depth5 = load("stream_btcusdt_depth5.json")["data"]
assert len(depth5["bids"]) == 5 and len(depth5["asks"]) == 5, "depth5 is untrimmed"
depth20 = load("stream_btcusdt_depth20_100ms.json")["data"]
assert len(depth20["bids"]) == 3 and len(depth20["asks"]) == 3
assert "M" in load("stream_btcusdt_trade.json")["data"], "every key is kept"
assert "M" in load("stream_btcusdt_aggTrade.json")["data"]
assert "B" in load("stream_btcusdt_kline_1m.json")["data"]["k"]
assert "e" not in load("stream_btcusdt_bookTicker.json")["data"], "bookTicker has no event type"
odd = load("stream_non_ascii_miniTicker.json")
name = odd["stream"]
assert name.endswith("@miniTicker") and not name.isascii(), name
assert not any(c.isascii() and c.isupper() for c in name), name
assert odd["data"]["e"] == "24hrMiniTicker", odd["data"]["e"]
print("ok")
EOF
```

- [ ] **Step 2: Run it to see it fail**

Run the **Verify** assertion alone:

```bash
python3 -c "import glob, json; files = glob.glob('polyoxide-binance/tests/fixtures/spot/ws/*.json'); assert len(files) == 11, files; [json.load(open(f, encoding='utf-8')) for f in files]"
```

Expected: `AssertionError: []`, and the shape check stops at its first `assert len(glob.glob(d + "*.json")) == 11` with a bare `AssertionError`. `grep -q '^## Streams (ws/)' polyoxide-binance/tests/fixtures/spot/PROVENANCE.md` exits 1.

- [ ] **Step 3: Implement**

Capture once and append the section it prints. The file is only written when all eleven streams have delivered, and the section only when the files are written, so a failed run changes nothing:

```bash
bash -c 'set -e; section="$(mktemp)"; nix develop --command uv run scripts/capture_binance_spot.py ws > "$section"; { printf "\n"; cat "$section"; } >> polyoxide-binance/tests/fixtures/spot/PROVENANCE.md; rm "$section"'
```

Progress goes to stderr: the `exchangeInfo` symbol, the eleven stream names, then `wrote 11 files under polyoxide-binance/tests/fixtures/spot/ws`. Read the diff before committing: `git diff polyoxide-binance/tests/fixtures/spot/PROVENANCE.md` shows only the appended `## Streams (ws/)` section (the capture date, `wss://stream.binance.com:9443/stream`, the `ws` command, the answer `{"result":null,"id":1}`, eleven table rows and the non-ASCII symbol), and `git status --short polyoxide-binance/tests/fixtures/spot` shows the eleven new files and `PROVENANCE.md` and nothing else (`rest/` is untouched).

- [ ] **Step 4: Run it to see it pass**

```bash
python3 -c "import glob, json; files = glob.glob('polyoxide-binance/tests/fixtures/spot/ws/*.json'); assert len(files) == 11, files; [json.load(open(f, encoding='utf-8')) for f in files]" && grep -q '^## Streams (ws/)' polyoxide-binance/tests/fixtures/spot/PROVENANCE.md && python3 scripts/capture_binance_spot.py ws --help
```

Expected: no output from the first two commands, then the `ws` usage text. Then run the shape check from Step 1 again; expected output: `ok`. If the capture did not return `24hrMiniTicker` for the non-ASCII stream, or an assertion about a kept key fails, do not edit the fixture: report what the wire sent.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/tests/fixtures/spot/ws
git commit -m "test(binance): spot's stream frames, captured from production (binance spot ws, Story 1)" -- polyoxide-binance/tests/fixtures/spot/ws polyoxide-binance/tests/fixtures/spot/PROVENANCE.md
```

## Story 2: Shared socket plumbing under `usdm::ws`

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/ws/mod.rs`, `polyoxide-binance/src/shared/ws/request.rs`, `polyoxide-binance/src/shared/ws/bare.rs`, `polyoxide-binance/src/shared/ws/protocol.rs`, `polyoxide-binance/src/shared/ws/supervised.rs`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `.claude/rules/binance.md`
- **Interfaces:** exposes "Story 2 → Story 4"; uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_claude_rules.py -q'`
- **Review:** deep (a refactor under a public socket API that prader-rs folds outages on)

Moves `usdm::ws`'s Binance-wide machinery into `src/shared/ws/`, generic over `StreamSet` and `SocketError` as the names contract gives them, and rebuilds `usdm::ws` on it: `UsdmStreams` and `impl SocketError for UsdmWsError` describe USDⓈ-M, and `UsdmWs`, `SupervisedUsdmWs`, `MembershipHandle` and `UsdmWsBuilder` wrap the generic types, with every public doc, path, name and signature as it was. Behaviour is unchanged line for line: the recovery rows (now matched on `Shape`), the record-before-send, the batched and paced replay, the refused batch undone, the router's stop cause, the forwarding queue, the rotation constant. No test file and no test module changes: Task 2.2 replaces only the code above each file's `mod tests`, and its Step 4 diffs every `mod tests` against `HEAD`. `UsdmProtocol` and `Change` (as `#[cfg(test)]` aliases), `UsdmWs::open`, `UsdmWs::close_frame` and the builder's fields stay where the unit tests reach them; `Answer`, which no test names, moves to `shared::ws::request`. An import only a tests module uses is a module-level `#[cfg(test)] use` (`StreamExt`, `Instant`, `MIN_REQUEST_INTERVAL` in `client.rs`; `Duration` in `supervised.rs`; `Message`, `Decoded`, `Offline`, `Protocol`, `Recovery`, `WsError` in `protocol.rs`), never one inside the tests module. Task 2.1 builds `shared::ws` with two tests of its own on a one-path probe stream set (three names a request, 60 ms apart), which `usdm`'s tests cannot reach: they pin that the batch size and pace come from the `StreamSet` and not from USDⓈ-M's numbers. Two deliberate departures from the contract's wording, with the same effect: `BinanceProtocol`'s `Debug` and `Handle`'s `Clone` and `Debug` are written by hand, because `#[derive]` on a type generic over `S` would require `S: Debug`/`S: Clone` of the marker; and `Bare<S>` has an explicit `impl Unpin`, because `S::Path` is not provably `Unpin` and the `Stream` impl needs `Pin<&mut Self>` to be `DerefMut`. No landmine marker appears in this story's code. `.claude/rules/binance.md` names `src/shared/ws/protocol.rs` for the protocol in the same commit as the move (Task 2.2). `src/usdm/ws/mod.rs` needs no edit: `StreamPath::ALL`, `StreamPath::url` and the three constants are already what `UsdmStreams` reads.

### Task 2.1: `shared::ws`, generic over a `StreamSet`

**Files:**
- Modify: `polyoxide-binance/src/shared/mod.rs`
- Create: `polyoxide-binance/src/shared/ws/mod.rs`, `polyoxide-binance/src/shared/ws/request.rs`, `polyoxide-binance/src/shared/ws/bare.rs`, `polyoxide-binance/src/shared/ws/protocol.rs`, `polyoxide-binance/src/shared/ws/supervised.rs`
- Test: `polyoxide-binance/src/shared/ws/mod.rs` (its `mod tests`)

- [ ] **Step 1: Write the failing test**

In `polyoxide-binance/src/shared/mod.rs`, the module list at the end of the file is:

```rust
pub(crate) mod error;
pub(crate) mod policy;
pub(crate) mod request;
pub(crate) mod symbol;
pub(crate) mod weight;
```

Replace it with (`ws` sorts last):

```rust
pub(crate) mod error;
pub(crate) mod policy;
pub(crate) mod request;
pub(crate) mod symbol;
pub(crate) mod weight;
#[cfg(feature = "usdm-ws")]
pub(crate) mod ws;
```

Create `polyoxide-binance/src/shared/ws/mod.rs` with the module declarations and the tests only (Step 3 adds the items between the declarations and the tests):

```rust
//! Binance's socket plumbing, shared by every product's `ws` module.
//!
//! What `usdm::ws` has that is not USDⓈ-M's: the request correlation, the
//! bare connection, the kit `Protocol` with Binance's recovery rows, the
//! router with its forwarding queue, and the stop. It is generic over a
//! product's [`StreamSet`] (its stream names, paths, frame decode and limits)
//! and [`SocketError`] (the constructors it needs of the product's error, and a
//! [`Shape`] view of its variants), so a second product's streams are a second
//! marker type and not a second copy.

use std::{fmt, time::Duration};

pub(crate) mod bare;
pub(crate) mod protocol;
pub(crate) mod request;
pub(crate) mod supervised;

#[cfg(test)]
#[path = "../../usdm/ws/scripted.rs"]
mod scripted;

#[cfg(test)]
mod tests {
    use polyoxide_venue::{Class, Classify};
    use polyoxide_ws::{Outage, WsError};
    use tokio::time::Instant;

    use super::{
        bare::Bare,
        scripted::{Script, ScriptedServer},
        *,
    };

    /// A stream set that is nothing like USDⓈ-M's: one path, three names a
    /// request, a request every 60 ms.
    struct Probe;

    #[derive(Debug, thiserror::Error)]
    enum ProbeError {
        #[error("transport: {0}")]
        Connect(#[source] WsError),
        #[error("refused: {code} {msg}")]
        Refused { code: i64, msg: String },
        #[error("no answer to {id} within {timeout:?}")]
        NoAnswer { id: u64, timeout: Duration },
        #[error("malformed answer to {id}: {raw}")]
        Response { id: u64, raw: String },
        #[error("{path} carries at most {limit}")]
        TooMany { path: &'static str, limit: usize },
        #[error("stopped")]
        Stopped(Option<String>),
    }

    impl From<WsError> for ProbeError {
        fn from(err: WsError) -> Self {
            Self::Connect(err)
        }
    }

    impl Classify for ProbeError {
        fn class(&self) -> Class {
            Class::Decode
        }
    }

    impl SocketError for ProbeError {
        type Path = &'static str;
        type Cause = String;

        fn refused(code: i64, msg: String) -> Self {
            Self::Refused { code, msg }
        }

        fn no_answer(id: u64, timeout: Duration) -> Self {
            Self::NoAnswer { id, timeout }
        }

        fn response(id: u64, raw: String) -> Self {
            Self::Response { id, raw }
        }

        fn too_many_streams(path: &'static str, limit: usize) -> Self {
            Self::TooMany { path, limit }
        }

        fn stopped(cause: Option<String>) -> Self {
            Self::Stopped(cause)
        }

        fn cause(&self) -> String {
            match self {
                Self::Stopped(cause) => cause.clone().unwrap_or_default(),
                other => other.to_string(),
            }
        }

        fn shape(&self) -> Shape<'_> {
            match self {
                Self::Connect(err) => Shape::Connect(err),
                Self::Refused { .. } => Shape::Refused,
                Self::NoAnswer { .. } => Shape::NoAnswer,
                Self::Response { .. } => Shape::Response,
                Self::TooMany { .. } => Shape::TooManyStreams,
                Self::Stopped(_) => Shape::Stopped,
            }
        }
    }

    impl StreamSet for Probe {
        type Name = String;
        type Path = &'static str;
        type Update = String;
        type Error = ProbeError;
        type Event = String;

        const PATHS: &'static [&'static str] = &["probe"];
        const MAX_NAMES_PER_REQUEST: usize = 3;
        const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(60);

        fn path(_name: &String) -> &'static str {
            "probe"
        }

        fn url(base: &str, path: &'static str) -> String {
            format!("{}/{path}", base.trim_end_matches('/'))
        }

        fn decode(text: &str) -> Result<String, ProbeError> {
            Ok(text.to_owned())
        }

        fn update(update: String) -> String {
            update
        }

        fn disconnected(path: &'static str, outage: Outage<ProbeError>) -> String {
            let _ = outage;
            format!("{path} down")
        }

        fn reconnected(path: &'static str) -> String {
            format!("{path} back")
        }
    }

    fn names(count: usize) -> Vec<String> {
        (0..count).map(|i| format!("n{i}@probe")).collect()
    }

    fn sizes(server: &ScriptedServer) -> Vec<usize> {
        server
            .requests()
            .iter()
            .map(|request| request["params"].as_array().unwrap().len())
            .collect()
    }

    #[tokio::test]
    async fn requests_follow_the_stream_sets_batch_size_and_pace() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = Bare::<Probe>::open(
            &server.url,
            "probe",
            Duration::from_secs(5),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        let wanted = names(7);
        let start = Instant::now();
        ws.subscribe(&wanted).await.unwrap();
        // Three names a request, not USDⓈ-M's 200, and each request at least
        // the set's interval after the last, not USDⓈ-M's 200 ms.
        assert_eq!(sizes(&server), [3, 3, 1]);
        assert!(
            start.elapsed() >= Probe::MIN_REQUEST_INTERVAL * 2,
            "three requests in {:?}",
            start.elapsed()
        );
        assert_eq!(server.paths(), ["/probe"]);
        assert_eq!(ws.streams(), wanted.as_slice());
    }

    #[tokio::test]
    async fn unsubscribe_batches_follow_the_stream_set_too() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = Bare::<Probe>::open(
            &server.url,
            "probe",
            Duration::from_secs(5),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        let wanted = names(4);
        ws.subscribe(&wanted).await.unwrap();
        ws.unsubscribe(&wanted).await.unwrap();
        assert_eq!(sizes(&server), [3, 1, 3, 1]);
        assert!(ws.streams().is_empty());
        assert_eq!(ws.path(), "probe");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --all-features --lib shared::ws::tests`

Expected: the build fails, because the four child modules have no files yet and the items the tests name do not exist:

```
error[E0583]: file not found for module `bare`
error[E0583]: file not found for module `protocol`
error[E0583]: file not found for module `request`
error[E0583]: file not found for module `supervised`
error: could not compile `polyoxide-binance` (lib) due to 4 previous errors
```

- [ ] **Step 3: Implement**

**3a. The items in `shared/ws/mod.rs`.** In `polyoxide-binance/src/shared/ws/mod.rs`, add `#![allow(dead_code)]` as the first inner attribute, below the `//!` doc lines and above `use std::{fmt, time::Duration};`:

```rust
// TEMPORARY, Task 2.1 only: nothing uses `shared::ws` until Task 2.2 moves
// `usdm::ws` onto it, and Task 2.2 deletes this line.
#![allow(dead_code)]
```

Then replace the line `pub(crate) mod supervised;` (it is unique; the `#[cfg(test)]` block that follows it stays) with:

```rust
pub(crate) mod supervised;

/// Streams one connection may carry. Both products' hosts close a connection
/// at its 1025th stream.
pub(crate) const MAX_STREAMS_PER_CONNECTION: usize = 1024;

/// Ten minutes under Binance's documented 24-hour connection lifetime.
pub(crate) const DEFAULT_MAX_CONNECTION_AGE: Duration = Duration::from_secs(23 * 3600 + 50 * 60);

/// Events the consumer may leave unread, across every path, before the paths
/// stop reading.
pub(crate) const EVENT_BUFFER: usize = 1024;

/// What one product says about its streams: the types it names them and their
/// frames with, and the limits its host enforces.
///
/// A product implements it on a marker type. The shared code is generic over
/// that marker, so nothing in it names a product.
pub(crate) trait StreamSet: Sized + Send + Sync + 'static {
    /// A stream's name, the only way to name one.
    type Name: Clone + Eq + std::hash::Hash + fmt::Display + fmt::Debug + Send + Sync + 'static;
    /// Which routed path a connection is on.
    type Path: Copy + Eq + std::hash::Hash + fmt::Display + fmt::Debug + Send + Sync + 'static;
    /// One decoded frame.
    type Update: Send + 'static;
    /// The product's socket error.
    type Error: SocketError<Path = Self::Path>;
    /// What the supervised stream yields.
    type Event: Send + 'static;

    /// Every path, in the order a first connect opens them.
    const PATHS: &'static [Self::Path];
    /// Stream names in one `SUBSCRIBE` or `UNSUBSCRIBE`.
    const MAX_NAMES_PER_REQUEST: usize;
    /// The shortest gap between two requests on one connection.
    const MIN_REQUEST_INTERVAL: Duration;

    /// The path a stream rides.
    fn path(name: &Self::Name) -> Self::Path;
    /// The combined-stream URL of `path` on `base`.
    fn url(base: &str, path: Self::Path) -> String;
    /// One text frame as an update: the product's `Update::from_json`.
    fn decode(text: &str) -> Result<Self::Update, Self::Error>;
    /// `Event::Update(Box::new(update))`.
    fn update(update: Self::Update) -> Self::Event;
    /// `Event::Disconnected { path, reason }`, the reason read from the kit's outage.
    fn disconnected(path: Self::Path, outage: polyoxide_ws::Outage<Self::Error>) -> Self::Event;
    /// `Event::Reconnected { path }`.
    fn reconnected(path: Self::Path) -> Self::Event;
}

/// A product's socket error, as the shared code reads it: which variant it is,
/// without the variant's fields, so recovery and the router match on a view and
/// not on a product's enum.
pub(crate) enum Shape<'a> {
    /// The kit's transport error, which recovery reads through its `Kind`.
    Connect(&'a polyoxide_ws::WsError),
    /// The connection did not open in time.
    ConnectTimeout,
    /// The server closed the connection.
    Closed,
    /// The server answered a request with an error.
    Refused,
    /// No answer to a request.
    NoAnswer,
    /// An answer that was not the documented shape.
    Response,
    /// A subscribe past the connection's cap, refused before sending.
    TooManyStreams,
    /// A stream for the other path, refused before sending.
    WrongPath,
    /// A frame that did not decode.
    Frame,
    /// The supervised tier has stopped.
    Stopped,
}

/// What the shared code needs of a product's error: constructors for the errors
/// it makes itself, and a [`Shape`] of the variant.
pub(crate) trait SocketError:
    From<polyoxide_ws::WsError> + polyoxide_venue::Classify + fmt::Display + fmt::Debug + Send + Sync + 'static
{
    /// The product's path type.
    type Path;
    /// The product's `StopCause`.
    type Cause: Clone + fmt::Debug + Send + Sync + 'static;

    /// The server answered a request with `{"error": {"code", "msg"}}`.
    fn refused(code: i64, msg: String) -> Self;
    /// No answer to request `id` within `timeout`; id 0 is a ping.
    fn no_answer(id: u64, timeout: Duration) -> Self;
    /// An answer to request `id` that was not the documented shape.
    fn response(id: u64, raw: String) -> Self;
    /// A subscribe that would take `path`'s connection past `limit` streams.
    fn too_many_streams(path: Self::Path, limit: usize) -> Self;
    /// The supervised tier has stopped, on `cause` when it is known.
    fn stopped(cause: Option<Self::Cause>) -> Self;
    /// `StopCause::of(self)`.
    fn cause(&self) -> Self::Cause;
    /// Which variant this is.
    fn shape(&self) -> Shape<'_>;
}
```

(`SocketError::cause` shares its name with the deprecated `std::error::Error::cause`, which every `Classify` type has as a supertrait method. The shared code therefore always calls it as `<S::Error as SocketError>::cause(err)`, never `err.cause()`.)

**3b. `polyoxide-binance/src/shared/ws/request.rs`:**

```rust
//! Request correlation: the frames Binance answers a request with, and the
//! request itself, sent on the kit's `Connection` and matched by id.

use std::fmt;

use polyoxide_ws::{tungstenite::Message, Connection, Correlated, Kind, WsError};
use serde_json::{json, Value};

use super::SocketError;

/// What a text frame that is not an update is.
pub(crate) enum Answer {
    /// `{"result": .., "id": n}`.
    Result { id: Option<u64>, result: Value },
    /// `{"error": {"code", "msg"}, "id": n}`.
    Error {
        id: Option<u64>,
        code: i64,
        msg: String,
    },
}

impl Answer {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        if object.contains_key("stream") {
            return None;
        }
        let id = object.get("id").and_then(Value::as_u64);
        if let Some(error) = object.get("error") {
            return Some(Self::Error {
                id,
                code: error
                    .get("code")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                msg: error
                    .get("msg")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
        if object.contains_key("result") || object.contains_key("id") {
            return Some(Self::Result {
                id,
                result: object.get("result").cloned().unwrap_or(Value::Null),
            });
        }
        None
    }

    fn id(&self) -> Option<u64> {
        match self {
            Self::Result { id, .. } | Self::Error { id, .. } => *id,
        }
    }
}

/// The request `method` with `params`, for request `id`.
fn request_frame<N: fmt::Display>(method: &str, params: Option<&[N]>, id: u64) -> Message {
    let mut body = json!({ "method": method, "id": id });
    if let Some(params) = params {
        let names: Vec<String> = params.iter().map(ToString::to_string).collect();
        body["params"] = json!(names);
    }
    Message::Text(body.to_string().into())
}

/// What a frame read while request `id` awaits its answer is: the answer, an
/// answer to another request, or anything else, which the stream yields
/// afterwards.
fn correlate(id: u64, message: &Message) -> Correlated<Answer> {
    let Message::Text(text) = message else {
        return Correlated::Push;
    };
    match Answer::parse(text) {
        Some(answer) if answer.id() == Some(id) => Correlated::Answer(answer),
        Some(other) => {
            tracing::debug!(id = ?other.id(), "uncorrelated answer while awaiting {id}");
            Correlated::Uncorrelated
        }
        None => Correlated::Push,
    }
}

/// Send request `method` with `params` on `connection`, paced as the
/// connection is, and wait for its answer's `result`. A request left
/// unanswered for the connection's answer timeout is the error's `no_answer`
/// with its id.
pub(crate) async fn request<N: fmt::Display, E: SocketError>(
    connection: &mut Connection,
    method: &str,
    params: Option<&[N]>,
) -> Result<Value, E> {
    let id = connection.next_id();
    let answer = connection
        .request(|id| request_frame(method, params, id), correlate)
        .await
        .map_err(|err| no_answer::<E>(err, id))?;
    match answer {
        Answer::Result { result, .. } => Ok(result),
        Answer::Error { code, msg, .. } => Err(E::refused(code, msg)),
    }
}

/// The kit's `NoAnswer` as the product's, with the id of what went
/// unanswered; every other failure as the socket error maps it.
pub(crate) fn no_answer<E: SocketError>(err: WsError, id: u64) -> E {
    match err.kind() {
        Kind::NoAnswer { after } => E::no_answer(id, after),
        _ => E::from(err),
    }
}
```

**3c. `polyoxide-binance/src/shared/ws/bare.rs`:**

```rust
//! The bare tier: one connection on one path, control methods, and a `Stream`
//! of updates.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_ws::{connect, tungstenite::Message, Connection, Correlated, Mode};
use tokio::time::Instant;

use super::{
    request::{no_answer, request, Answer},
    SocketError, StreamSet, MAX_STREAMS_PER_CONNECTION,
};

/// One connection on one path of a product's streams.
///
/// Requests are batched at most `S::MAX_NAMES_PER_REQUEST` names each and sent
/// no faster than one per `S::MIN_REQUEST_INTERVAL`; each waits for the answer
/// carrying its id, and updates read meanwhile are kept and yielded by the
/// stream afterwards. A subscribe that would take the connection past
/// [`MAX_STREAMS_PER_CONNECTION`] is refused before anything is sent. It does
/// not check paths: the product's wrapper does, because only it knows what a
/// stream on the wrong path is. The control methods are not cancel-safe, and
/// after a `NoAnswer` the server's membership is unknown: replace the
/// connection rather than reuse it. The server's protocol pings are answered
/// while the caller polls, and its close is answered with the close reply. The
/// stream ends after the server's close, or after the first transport error it
/// yields.
pub(crate) struct Bare<S: StreamSet> {
    connection: Connection,
    path: S::Path,
    streams: Vec<S::Name>,
}

// Nothing in it is structurally pinned, but `S::Path` is not provably `Unpin`,
// and the `Stream` impl below needs `Pin<&mut Self>` to be `DerefMut`.
impl<S: StreamSet> Unpin for Bare<S> {}

impl<S: StreamSet> Bare<S> {
    /// Open a connection with no streams, paced at `S::MIN_REQUEST_INTERVAL`.
    pub(crate) async fn open(
        base_url: &str,
        path: S::Path,
        connect_timeout: Duration,
        answer_timeout: Duration,
    ) -> Result<Self, S::Error> {
        let socket = connect(S::url(base_url, path), connect_timeout).await?;
        let connection = Connection::new(socket, Mode::Data)
            .min_request_interval(S::MIN_REQUEST_INTERVAL)
            .answer_timeout(answer_timeout);
        Ok(Self {
            connection,
            path,
            streams: Vec::new(),
        })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with `too_many_streams` before sending anything, and with
    /// `refused` when the server answers with an error; batches answered
    /// before it stay subscribed, and [`streams`](Self::streams) says which.
    pub(crate) async fn subscribe(&mut self, streams: &[S::Name]) -> Result<(), S::Error> {
        let mut new: Vec<S::Name> = Vec::new();
        for stream in streams {
            if !self.streams.contains(stream) && !new.contains(stream) {
                new.push(stream.clone());
            }
        }
        if self.streams.len() + new.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(<S::Error as SocketError>::too_many_streams(
                self.path,
                MAX_STREAMS_PER_CONNECTION,
            ));
        }
        for batch in new.chunks(S::MAX_NAMES_PER_REQUEST) {
            request::<S::Name, S::Error>(&mut self.connection, "SUBSCRIBE", Some(batch)).await?;
            self.streams.extend(batch.iter().cloned());
        }
        Ok(())
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub(crate) async fn unsubscribe(&mut self, streams: &[S::Name]) -> Result<(), S::Error> {
        let mut gone: Vec<S::Name> = Vec::new();
        for stream in streams {
            if self.streams.contains(stream) && !gone.contains(stream) {
                gone.push(stream.clone());
            }
        }
        for batch in gone.chunks(S::MAX_NAMES_PER_REQUEST) {
            request::<S::Name, S::Error>(&mut self.connection, "UNSUBSCRIBE", Some(batch))
                .await?;
            self.streams.retain(|s| !batch.contains(s));
        }
        Ok(())
    }

    /// The stream names the server says this connection carries.
    pub(crate) async fn list_subscriptions(&mut self) -> Result<Vec<String>, S::Error> {
        let id = self.connection.next_id();
        let result =
            request::<S::Name, S::Error>(&mut self.connection, "LIST_SUBSCRIPTIONS", None).await?;
        serde_json::from_value(result.clone())
            .map_err(|_| <S::Error as SocketError>::response(id, result.to_string()))
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub(crate) async fn ping(&mut self) -> Result<Duration, S::Error> {
        let start = Instant::now();
        self.send_ping().await?;
        self.connection
            .await_answer(|message| match message {
                Message::Pong(_) => Correlated::Answer(()),
                _ => Correlated::Push,
            })
            .await
            .map_err(|err| no_answer::<S::Error>(err, 0))?;
        Ok(start.elapsed())
    }

    /// Close the connection.
    pub(crate) async fn close(&mut self) -> Result<(), S::Error> {
        self.connection.close().await?;
        Ok(())
    }

    /// The streams this connection carries.
    pub(crate) fn streams(&self) -> &[S::Name] {
        &self.streams
    }

    /// The path this connection is on.
    pub(crate) fn path(&self) -> S::Path {
        self.path
    }

    /// The connection, for a product's `Debug`.
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Send a protocol ping without waiting for its pong.
    async fn send_ping(&mut self) -> Result<(), S::Error> {
        self.connection
            .send(Message::Ping(Vec::new().into()))
            .await?;
        Ok(())
    }

    /// The server's close code and reason, once it has closed the connection
    /// or the stream has ended without a close frame.
    #[cfg(test)]
    pub(crate) fn close_frame(&self) -> Option<(Option<u16>, String)> {
        let closed = self.connection.closed()?;
        Some(match closed.kind() {
            polyoxide_ws::Kind::Closed { code, reason } => (code, reason.to_owned()),
            _ => (None, String::new()),
        })
    }
}

impl<S: StreamSet> Stream for Bare<S> {
    type Item = Result<S::Update, S::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let message = match self.connection.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            };
            // The connection yields Text and Binary only; a Binary frame is
            // not part of Binance's protocol (D12).
            let Message::Text(text) = message else {
                continue;
            };
            match S::decode(&text) {
                Ok(update) => return Poll::Ready(Some(Ok(update))),
                // An answer nobody is waiting for, such as one to a request
                // whose caller gave up.
                Err(_) if Answer::parse(&text).is_some() => continue,
                Err(err) => return Poll::Ready(Some(Err(err))),
            }
        }
    }
}
```

**3d. `polyoxide-binance/src/shared/ws/protocol.rs`:**

```rust
//! Binance's socket protocol for one routed path, as the kit's `Supervisor`
//! drives it. The supervised tier runs one supervisor per path.

use std::{fmt, time::Duration};

use polyoxide_venue::class_for_handshake_status;
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    Connection, Decoded, Kind, Offline, Outage, Protocol, Queue, Recovery, WsError,
};

use super::{
    request::{request, Answer},
    Shape, SocketError, StreamSet, MAX_STREAMS_PER_CONNECTION,
};

/// One change to a path's streams.
#[derive(Debug, Clone)]
pub(crate) enum Change<N> {
    /// Subscribe to these streams; one already subscribed is skipped.
    Subscribe(Vec<N>),
    /// Unsubscribe from these streams; one not subscribed is skipped.
    Unsubscribe(Vec<N>),
}

/// Binance's protocol on one routed path of a product's streams.
///
/// Its membership is the path's streams, replayed on every fresh connection at
/// most `S::MAX_NAMES_PER_REQUEST` names a request, one request per
/// `S::MIN_REQUEST_INTERVAL`, with the connection's cap checked first (D9).
/// Its divergence rows:
///
/// - **D4, the ping:** an empty protocol `Ping` on the wall clock.
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
pub(crate) struct BinanceProtocol<S: StreamSet> {
    base_url: String,
    path: S::Path,
}

// By hand: a derive would require `S: Debug` of the marker type.
impl<S: StreamSet> fmt::Debug for BinanceProtocol<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BinanceProtocol")
            .field("base_url", &self.base_url)
            .field("path", &self.path)
            .finish()
    }
}

impl<S: StreamSet> BinanceProtocol<S> {
    pub(crate) fn new(base_url: String, path: S::Path) -> Self {
        Self { base_url, path }
    }

    fn too_many(&self) -> S::Error {
        <S::Error as SocketError>::too_many_streams(self.path, MAX_STREAMS_PER_CONNECTION)
    }

    /// The streams of `add` not already in `streams`, once each, refused
    /// before anything is recorded or sent if they would pass the cap.
    fn new_streams(
        &self,
        streams: &[S::Name],
        add: Vec<S::Name>,
    ) -> Result<Vec<S::Name>, S::Error> {
        let mut new: Vec<S::Name> = Vec::new();
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
fn gone_streams<N: Clone + Eq>(streams: &[N], remove: Vec<N>) -> Vec<N> {
    let mut gone: Vec<N> = Vec::new();
    for stream in remove {
        if streams.contains(&stream) && !gone.contains(&stream) {
            gone.push(stream);
        }
    }
    gone
}

impl<S: StreamSet> Protocol for BinanceProtocol<S> {
    type Event = S::Event;
    type Error = S::Error;
    type Membership = Vec<S::Name>;
    type Change = Change<S::Name>;
    /// The path's streams after the change.
    type Ack = Vec<S::Name>;

    fn handshake(&mut self) -> Result<Handshake, S::Error> {
        Ok(S::url(&self.base_url, self.path)
            .into_client_request()
            .map_err(WsError::from)?)
    }

    /// Requests no faster than one per `S::MIN_REQUEST_INTERVAL`. No answer
    /// timeout: the supervisor bounds each replay and change by `stale_after`
    /// (L2-D1).
    fn configure(&self, connection: Connection) -> Connection {
        connection.min_request_interval(S::MIN_REQUEST_INTERVAL)
    }

    /// Every stream, `S::MAX_NAMES_PER_REQUEST` names a request; the cap is
    /// checked first. A stream refused on a fresh connection stops the path,
    /// as retrying replays it.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        streams: &Vec<S::Name>,
    ) -> Result<(), S::Error> {
        if streams.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(self.too_many());
        }
        for batch in streams.chunks(S::MAX_NAMES_PER_REQUEST) {
            request::<S::Name, S::Error>(connection, "SUBSCRIBE", Some(batch)).await?;
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
        streams: &mut Vec<S::Name>,
        change: Change<S::Name>,
    ) -> Result<Vec<S::Name>, S::Error> {
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
        for (n, batch) in names.chunks(S::MAX_NAMES_PER_REQUEST).enumerate() {
            match request::<S::Name, S::Error>(connection, method, Some(batch)).await {
                Ok(_) => {}
                Err(err) if matches!(err.shape(), Shape::Refused) => {
                    let answered = &names[..n * S::MAX_NAMES_PER_REQUEST];
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
        streams: &mut Vec<S::Name>,
        change: Change<S::Name>,
    ) -> Offline<Change<S::Name>, Vec<S::Name>, S::Error> {
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

    fn decode(&mut self, frame: &Message) -> Decoded<S::Event, S::Error> {
        // A Binary frame is not part of the protocol (D12), and the kit
        // answers the server's Ping and its Close.
        let Message::Text(text) = frame else {
            return Decoded::Nothing;
        };
        match S::decode(text) {
            Ok(update) => Decoded::Event(S::update(update)),
            // An answer nobody awaits, such as one to a request that
            // outlived the staleness window.
            Err(_) if Answer::parse(text).is_some() => Decoded::Nothing,
            Err(err) => Decoded::Error(err),
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<S::Event, S::Error>) -> bool {
        true
    }

    /// `decode` yields an event only for an update (the outage markers come
    /// from `disconnected` and `reconnected`), so any event is one.
    fn delivered(&self, _frame: &Message, decoded: &Decoded<S::Event, S::Error>) -> bool {
        matches!(decoded, Decoded::Event(_))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Ping(Vec::new().into()))
    }

    fn wanted(streams: &Vec<S::Name>) -> bool {
        !streams.is_empty()
    }

    fn is_retriable(err: &S::Error) -> bool {
        err.is_retriable()
    }

    /// Binance's own recovery, not the class's: a server close of any code,
    /// a lost or timed out connection, an unanswered replay and a malformed
    /// answer reconnect; a frame that does not decode is skipped; a refusal
    /// and the client's own refusals stop.
    fn recover(&self, err: &S::Error) -> Recovery {
        match err.shape() {
            Shape::Connect(err) => match err.kind() {
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
            Shape::ConnectTimeout | Shape::Closed | Shape::NoAnswer | Shape::Response => {
                Recovery::Reconnect
            }
            Shape::Frame => Recovery::Skip,
            Shape::Refused | Shape::TooManyStreams | Shape::WrongPath | Shape::Stopped => {
                Recovery::Stop
            }
        }
    }

    /// A refused handshake's `Retry-After`, as Binance's class reads it,
    /// which the reconnect waits out up to the backoff's ceiling (DRIFT R11).
    fn retry_after(&self, err: &S::Error) -> Option<Duration> {
        err.retry_after()
    }

    fn disconnected(&mut self, outage: Outage<S::Error>) -> Option<S::Event> {
        Some(S::disconnected(self.path, outage))
    }

    fn reconnected(&mut self) -> S::Event {
        S::reconnected(self.path)
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(1)
    }

    /// A replay or a change the server left unanswered for the staleness
    /// window. The supervised tier answers a change so left `Ok`, as it is
    /// recorded and the replay applies it.
    fn stalled(silent: Duration) -> S::Error {
        <S::Error as SocketError>::no_answer(0, silent)
    }

    /// The kit's task stopped; the router adds the cause it saw.
    fn stopped() -> S::Error {
        <S::Error as SocketError>::stopped(None)
    }
}
```

**3e. `polyoxide-binance/src/shared/ws/supervised.rs`:**

```rust
//! The supervised tier: one kit `Supervisor` per routed path, with keep-alive
//! on the wall clock, staleness detection, reconnect with a paced replay of
//! the path's streams, rotation before Binance's 24-hour cutoff and outage
//! markers, behind a router that sends each membership change to its path's
//! connection, opens a path for its first stream, and merges every path's
//! events into one stream.

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    task::{Context, Poll},
};

use futures_util::StreamExt;
use polyoxide_ws::{MembershipHandle as PathHandle, Supervised, Supervisor, Timing};
use tokio::sync::{mpsc, oneshot, watch};

use super::{
    protocol::{BinanceProtocol, Change},
    Shape, SocketError, StreamSet, EVENT_BUFFER, MAX_STREAMS_PER_CONNECTION,
};

/// What a product's builder hands the shared code.
pub(crate) struct Settings<S: StreamSet> {
    /// The host the paths are on.
    pub(crate) base_url: String,
    /// The kit's timing, rotation age included.
    pub(crate) timing: Timing,
    /// The streams to open with.
    pub(crate) streams: Vec<S::Name>,
}

impl<S: StreamSet> Settings<S> {
    /// The supervisor for one path's connection.
    fn supervisor(&self, path: S::Path) -> Supervisor<BinanceProtocol<S>> {
        Supervisor::new(
            BinanceProtocol::new(self.base_url.clone(), path),
            self.timing,
        )
    }
}

type Cause<S> = <<S as StreamSet>::Error as SocketError>::Cause;
type Item<S> = Result<<S as StreamSet>::Event, <S as StreamSet>::Error>;
type Events<S> = mpsc::Sender<Item<S>>;
type Reply<S> = oneshot::Sender<Result<(), <S as StreamSet>::Error>>;

/// Open a connection for each path the streams need, subscribe, and start
/// supervising.
///
/// Fails if any first connect fails, closing the others: a setup that is
/// wrong is reported, not retried. With no streams it opens nothing until the
/// first [`Handle::subscribe`].
pub(crate) async fn connect<S: StreamSet>(
    settings: Settings<S>,
) -> Result<Running<S>, S::Error> {
    let mut membership: HashMap<S::Path, Vec<S::Name>> = HashMap::new();
    for stream in settings.streams.iter().cloned() {
        let wanted = membership.entry(S::path(&stream)).or_default();
        if !wanted.contains(&stream) {
            wanted.push(stream);
        }
    }
    for (path, wanted) in &membership {
        if wanted.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(<S::Error as SocketError>::too_many_streams(
                *path,
                MAX_STREAMS_PER_CONNECTION,
            ));
        }
    }
    let mut opened: Vec<(S::Path, Supervised<BinanceProtocol<S>>)> = Vec::new();
    for path in S::PATHS {
        let Some(wanted) = membership.get(path) else {
            continue;
        };
        match settings.supervisor(*path).connect(wanted.clone()).await {
            Ok(supervised) => opened.push((*path, supervised)),
            Err(err) => {
                for (_, supervised) in opened {
                    let _ = supervised.close().await;
                }
                return Err(err);
            }
        }
    }
    let (events, events_rx) = mpsc::channel(EVENT_BUFFER);
    let (commands, commands_rx) = mpsc::channel(16);
    let (ended, ended_rx) = mpsc::unbounded_channel();
    let (closing, _) = watch::channel(false);
    let stop = Stop::<S>::new();
    let mut router = Router {
        config: settings,
        paths: HashMap::new(),
        membership,
        events,
        closing,
        ended,
        forwarders: Vec::new(),
        generation: 0,
        stop: stop.clone(),
    };
    for (path, supervised) in opened {
        router.adopt(path, supervised);
    }
    let task = tokio::spawn(route(router, commands_rx, ended_rx));
    Ok(Running {
        events: events_rx,
        commands,
        task,
        stop,
    })
}

enum Command<S: StreamSet> {
    Subscribe(Vec<S::Name>, Reply<S>),
    Unsubscribe(Vec<S::Name>, Reply<S>),
    /// Close every socket and end. Sent by [`Running::close`]; a dropped
    /// sender is not enough, because every [`Handle`] holds a clone.
    Close,
}

/// Changes the streams of a [`Running`] client.
///
/// Each stream is routed to its path's connection, opening it if this is the
/// path's first stream and closing it when its last leaves. The membership is
/// a set: subscribing a stream twice holds it once, and counting references is
/// the caller's job. A change is recorded before it is sent (see the product's
/// handle for the rules a caller reads).
pub(crate) struct Handle<S: StreamSet> {
    commands: mpsc::Sender<Command<S>>,
    stop: Stop<S>,
}

// By hand: a derive would require `S: Clone` and `S: Debug` of the marker type.
impl<S: StreamSet> Clone for Handle<S> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            stop: self.stop.clone(),
        }
    }
}

impl<S: StreamSet> std::fmt::Debug for Handle<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").finish_non_exhaustive()
    }
}

impl<S: StreamSet> Handle<S> {
    /// Subscribe to more streams.
    pub(crate) async fn subscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        self.send(|reply| Command::Subscribe(streams, reply)).await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub(crate) async fn unsubscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        self.send(|reply| Command::Unsubscribe(streams, reply))
            .await
    }

    async fn send(&self, make: impl FnOnce(Reply<S>) -> Command<S>) -> Result<(), S::Error> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.commands
            .send(make(reply_tx))
            .await
            .map_err(|_| self.stop.error())?;
        reply_rx.await.map_err(|_| self.stop.error())?
    }
}

/// The error a path stopped the client on, once the router has seen it:
/// shared by the router, the stream and every handle, so `close` and every
/// later membership call carry it (M-D8). A stop's cause is set once.
struct Stop<S: StreamSet>(Arc<OnceLock<Cause<S>>>);

impl<S: StreamSet> Clone for Stop<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<S: StreamSet> Stop<S> {
    fn new() -> Self {
        Self(Arc::new(OnceLock::new()))
    }

    fn set(&self, cause: Cause<S>) {
        let _ = self.0.set(cause);
    }

    /// The product's `Stopped` error, with the cause when there is one.
    fn error(&self) -> S::Error {
        <S::Error as SocketError>::stopped(self.0.get().cloned())
    }
}

/// A supervised set of connections: the events of every path, merged, that
/// survives drops, stalls and Binance's 24-hour cutoff.
///
/// A fatal error on any path, such as the server refusing a replay after a
/// reconnect, is yielded as `Err` and then the stream ends, every path closed.
pub(crate) struct Running<S: StreamSet> {
    events: mpsc::Receiver<Item<S>>,
    commands: mpsc::Sender<Command<S>>,
    task: tokio::task::JoinHandle<Result<(), S::Error>>,
    stop: Stop<S>,
}

impl<S: StreamSet> Running<S> {
    /// A handle for changing streams while the stream runs.
    pub(crate) fn membership(&self) -> Handle<S> {
        Handle {
            commands: self.commands.clone(),
            stop: self.stop.clone(),
        }
    }

    /// Whether the router's task is still running.
    pub(crate) fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    /// The next event or error, for the product's `Stream` impl.
    pub(crate) fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<Item<S>>> {
        self.events.poll_recv(cx)
    }

    /// Close every connection and stop. Every [`Handle`] answers the product's
    /// `Stopped` afterwards.
    ///
    /// Fails with `Stopped` if a path had stopped the client on an error,
    /// which the stream yielded: its cause is that error. That holds once the
    /// router has the path's stop: a `close()` made right after the stream
    /// yielded the stopping error can return `Ok` or `Stopped { cause: None }`.
    pub(crate) async fn close(self) -> Result<(), S::Error> {
        let Self {
            events,
            commands,
            task,
            stop,
        } = self;
        // Dropping the receiver first unblocks a path parked behind a full
        // queue, which would otherwise never see the command.
        drop(events);
        let _ = commands.send(Command::Close).await;
        drop(commands);
        match task.await {
            Ok(result) => result,
            Err(_) => Err(stop.error()),
        }
    }
}

/// A path's supervised connection, as the router holds it.
struct Path<S: StreamSet> {
    /// Which of the path's supervisors this is; a path closed for want of
    /// streams may still be draining when the next opens.
    generation: u64,
    handle: PathHandle<BinanceProtocol<S>>,
}

/// A path's stream that ended.
struct Ended<S: StreamSet> {
    path: S::Path,
    generation: u64,
    /// Whether it stopped on an error, rather than because its streams left.
    stopped: bool,
    /// The error it stopped on: the last item it yielded, when that was an
    /// error.
    cause: Option<Cause<S>>,
}

/// The router: routes membership changes to the paths, opens a path for its
/// first stream, and lets it close when its last leaves.
struct Router<S: StreamSet> {
    config: Settings<S>,
    paths: HashMap<S::Path, Path<S>>,
    /// Each path's streams, as its supervisor last reported them.
    membership: HashMap<S::Path, Vec<S::Name>>,
    events: Events<S>,
    /// Set when the router closes, which closes every path.
    closing: watch::Sender<bool>,
    ended: mpsc::UnboundedSender<Ended<S>>,
    forwarders: Vec<tokio::task::JoinHandle<()>>,
    generation: u64,
    /// The cause the client stopped on, shared with the stream and handles.
    stop: Stop<S>,
}

impl<S: StreamSet> Router<S> {
    /// Forward a path's events into the consumer's queue, and route its
    /// changes to it from now on.
    fn adopt(&mut self, path: S::Path, supervised: Supervised<BinanceProtocol<S>>) {
        self.generation += 1;
        let handle = supervised.membership();
        self.forwarders.retain(|forwarder| !forwarder.is_finished());
        self.forwarders.push(tokio::spawn(forward::<S>(
            path,
            self.generation,
            supervised,
            self.events.clone(),
            self.closing.subscribe(),
            self.ended.clone(),
        )));
        self.paths.insert(
            path,
            Path {
                generation: self.generation,
                handle,
            },
        );
    }

    /// A path's stream ended. One that stopped on an error stops the client;
    /// one whose streams left is forgotten, unless a newer one holds its path.
    fn ended(&mut self, ended: Ended<S>) -> Result<(), S::Error> {
        if ended.stopped {
            if let Some(cause) = ended.cause {
                self.stop.set(cause);
            }
            return Err(self.stop.error());
        }
        if self
            .paths
            .get(&ended.path)
            .is_some_and(|path| path.generation == ended.generation)
        {
            self.paths.remove(&ended.path);
            self.membership.remove(&ended.path);
        }
        Ok(())
    }

    async fn subscribe(&mut self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        let by_path = new_by_path::<S>(&self.membership, streams);
        // Refuse before sending anything: the server answers the 1025th stream
        // with an error and then closes the connection, losing all 1024.
        for (path, add) in &by_path {
            let held = self.membership.get(path).map_or(0, Vec::len);
            if held + add.len() > MAX_STREAMS_PER_CONNECTION {
                return Err(<S::Error as SocketError>::too_many_streams(
                    *path,
                    MAX_STREAMS_PER_CONNECTION,
                ));
            }
        }
        for path in S::PATHS {
            let Some(add) = by_path.get(path) else {
                continue;
            };
            match self.paths.get(path) {
                Some(slot) => {
                    let handle = slot.handle.clone();
                    self.change(*path, &handle, Change::Subscribe(add.clone()))
                        .await?;
                }
                None => self.open(*path, add.clone()).await?,
            }
        }
        Ok(())
    }

    async fn unsubscribe(&mut self, streams: Vec<S::Name>) -> Result<(), S::Error> {
        let mut by_path: HashMap<S::Path, Vec<S::Name>> = HashMap::new();
        for stream in streams {
            let path = S::path(&stream);
            if self
                .membership
                .get(&path)
                .is_some_and(|m| m.contains(&stream))
            {
                let batch = by_path.entry(path).or_default();
                if !batch.contains(&stream) {
                    batch.push(stream);
                }
            }
        }
        for path in S::PATHS {
            let (Some(remove), Some(slot)) = (by_path.get(path), self.paths.get(path)) else {
                continue;
            };
            let handle = slot.handle.clone();
            self.change(*path, &handle, Change::Unsubscribe(remove.clone()))
                .await?;
        }
        Ok(())
    }

    /// Open a path for its first streams: its connection, or an outage when
    /// the first attempt fails in a way retrying can fix. One that fails for
    /// good opens nothing.
    async fn open(&mut self, path: S::Path, streams: Vec<S::Name>) -> Result<(), S::Error> {
        let supervised = self.config.supervisor(path).start(streams.clone()).await?;
        self.adopt(path, supervised);
        self.membership.insert(path, streams);
        Ok(())
    }

    /// Make one change on a path, and note the streams it leaves there.
    async fn change(
        &mut self,
        path: S::Path,
        handle: &PathHandle<BinanceProtocol<S>>,
        change: Change<S::Name>,
    ) -> Result<(), S::Error> {
        match handle.apply(change.clone()).await {
            Ok(streams) => {
                self.settle(path, streams);
                Ok(())
            }
            // `BinanceProtocol::stalled`: the change outlived the staleness
            // window. It was recorded before it was sent, the connection is
            // being replaced, and the replay applies it, so it stands.
            Err(err) if matches!(err.shape(), Shape::NoAnswer) => {
                let mut streams = self.membership.get(&path).cloned().unwrap_or_default();
                match change {
                    Change::Subscribe(add) => streams.extend(add),
                    Change::Unsubscribe(remove) => streams.retain(|s| !remove.contains(s)),
                }
                self.settle(path, streams);
                Ok(())
            }
            Err(err) if matches!(err.shape(), Shape::Refused) => {
                // The batches answered before the refusal stay; an empty
                // change reads what the path now carries.
                if let Ok(streams) = handle.apply(Change::Subscribe(Vec::new())).await {
                    self.settle(path, streams);
                }
                Err(err)
            }
            Err(err) => Err(err),
        }
    }

    /// Note a path's streams after a change. A path left with none closes
    /// itself and is forgotten now, without waiting for it, so a full queue
    /// cannot hold the change's caller.
    fn settle(&mut self, path: S::Path, streams: Vec<S::Name>) {
        if streams.is_empty() {
            self.membership.remove(&path);
            self.paths.remove(&path);
        } else {
            self.membership.insert(path, streams);
        }
    }

    /// Close every path, and wait for each to go.
    async fn close(self) {
        let _ = self.closing.send(true);
        for forwarder in self.forwarders {
            let _ = forwarder.await;
        }
    }
}

/// Group `streams` by path, dropping any already wanted and duplicates.
fn new_by_path<S: StreamSet>(
    membership: &HashMap<S::Path, Vec<S::Name>>,
    streams: Vec<S::Name>,
) -> HashMap<S::Path, Vec<S::Name>> {
    let mut by_path: HashMap<S::Path, Vec<S::Name>> = HashMap::new();
    for stream in streams {
        let path = S::path(&stream);
        let known = membership.get(&path).is_some_and(|m| m.contains(&stream));
        let batch = by_path.entry(path).or_default();
        if !known && !batch.contains(&stream) {
            batch.push(stream);
        }
    }
    by_path.retain(|_, batch| !batch.is_empty());
    by_path
}

/// Serve membership changes one at a time, and stop when the consumer goes,
/// the client is closed, or a path stops on an error.
async fn route<S: StreamSet>(
    mut router: Router<S>,
    mut commands: mpsc::Receiver<Command<S>>,
    mut ended: mpsc::UnboundedReceiver<Ended<S>>,
) -> Result<(), S::Error> {
    let events = router.events.clone();
    let result = loop {
        tokio::select! {
            biased;
            Some(path) = ended.recv() => {
                if let Err(err) = router.ended(path) {
                    break Err(err);
                }
            }
            () = events.closed() => break Ok(()),
            command = commands.recv() => match command {
                None | Some(Command::Close) => break Ok(()),
                Some(Command::Subscribe(streams, reply)) => {
                    let _ = reply.send(router.subscribe(streams).await);
                }
                Some(Command::Unsubscribe(streams, reply)) => {
                    let _ = reply.send(router.unsubscribe(streams).await);
                }
            },
        }
    };
    drop(events);
    router.close().await;
    result
}

/// Move one path's events into the consumer's queue until the path ends, the
/// consumer goes, or the router closes. A path that ends by itself says so,
/// and whether it stopped on an error, which is the last item it yielded.
async fn forward<S: StreamSet>(
    path: S::Path,
    generation: u64,
    mut supervised: Supervised<BinanceProtocol<S>>,
    events: Events<S>,
    mut closing: watch::Receiver<bool>,
    ended: mpsc::UnboundedSender<Ended<S>>,
) {
    // The last item, when it was an error: a path that stops yields the
    // error it stopped on last, before its stream ends.
    let mut last_error: Option<Cause<S>> = None;
    loop {
        let next = tokio::select! {
            biased;
            _ = closing.wait_for(|closing| *closing) => break,
            // The consumer is gone: close the path politely before going,
            // so `Running::close` returns after it has.
            () = events.closed() => break,
            next = supervised.next() => next,
        };
        let Some(item) = next else {
            let stopped = supervised.close().await.is_err();
            let _ = ended.send(Ended {
                path,
                generation,
                stopped,
                cause: last_error.filter(|_| stopped),
            });
            return;
        };
        last_error = item.as_ref().err().map(<S::Error as SocketError>::cause);
        let slot = tokio::select! {
            biased;
            _ = closing.wait_for(|closing| *closing) => break,
            slot = events.reserve() => slot,
        };
        match slot {
            Ok(slot) => slot.send(item),
            Err(_) => break,
        }
    }
    let _ = supervised.close().await;
}
```

Then run `nix develop --command cargo fmt --all` so rustfmt settles the line breaks.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --all-features --lib shared::ws::tests`

Expected:

```
running 2 tests
test shared::ws::tests::requests_follow_the_stream_sets_batch_size_and_pace ... ok
test shared::ws::tests::unsubscribe_batches_follow_the_stream_set_too ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured
```

Then the tree gates for the package: `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings`, `nix develop --command cargo test -p polyoxide-binance --all-features` (the `usdm` suites still pass: nothing in them changed), `nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps` and `python3 scripts/test_body_diff.py --baseline v0.40.0`. The temporary `#![allow(dead_code)]` keeps clippy quiet about the unused generic code; Task 2.2 removes it. If the compiler rejects a generic bound, fix it in the shared file (the names contract fixes the signatures; the bodies are the part to adjust).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/shared/ws/mod.rs polyoxide-binance/src/shared/ws/request.rs polyoxide-binance/src/shared/ws/bare.rs polyoxide-binance/src/shared/ws/protocol.rs polyoxide-binance/src/shared/ws/supervised.rs
git commit -m "refactor(binance): shared::ws, the socket plumbing generic over a StreamSet (binance spot ws, Story 2)" -- polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/shared/ws/mod.rs polyoxide-binance/src/shared/ws/request.rs polyoxide-binance/src/shared/ws/bare.rs polyoxide-binance/src/shared/ws/protocol.rs polyoxide-binance/src/shared/ws/supervised.rs
```

### Task 2.2: Rebuild `usdm::ws` on `shared::ws`

**Files:**
- Modify: `polyoxide-binance/src/shared/ws/mod.rs` (delete the temporary `allow`)
- Modify: `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`
- Modify: `.claude/rules/binance.md`
- Test: the existing `mod tests` of the four `usdm/ws` files above, unchanged, with `supervision_usdm`, `supervision_usdm_edges`, `supervision_usdm_kit` and `wire_agreement_usdm_ws`

- [ ] **Step 1: The guard is the existing tests; write no new one**

This is a refactor under AD-12: no test is added, moved or edited, and the unit tests in `client.rs`, `error.rs`, `protocol.rs` and `supervised.rs` are the regression guard with the four integration suites. Nothing below touches a `mod tests` block.

- [ ] **Step 2: Run the guard on the tree as it stands**

Run: `nix develop --command cargo test -p polyoxide-binance --all-features`

Expected: every test passes, the `usdm` unit tests and `supervision_usdm*` among them (this is the baseline Step 4 must reproduce, test for test).

- [ ] **Step 3: Implement**

Each edit below replaces the code **above** a file's `mod tests`; leave the `#[cfg(test)] mod tests { ... }` block byte for byte as it is.

**3a. `polyoxide-binance/src/shared/ws/mod.rs`:** delete these three lines (the comment and the attribute):

```rust
// TEMPORARY, Task 2.1 only: nothing uses `shared::ws` until Task 2.2 moves
// `usdm::ws` onto it, and Task 2.2 deletes this line.
#![allow(dead_code)]
```

**3b. `polyoxide-binance/src/usdm/ws/error.rs`.** Replace the import line

```rust
use crate::usdm::ws::{stream::StreamName, StreamPath};
```

with

```rust
use crate::{
    shared::ws::{Shape, SocketError},
    usdm::ws::{stream::StreamName, StreamPath},
};
```

Then replace the end of the `Classify` impl, exactly

```rust
    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Connect(err) => ws_retry_after(err),
            Self::Stopped { cause: Some(cause) } => cause.retry_after,
            _ => self.class().retry_after(),
        }
    }
}
```

with itself followed by the new impl (everything after it, from `#[cfg(test)]`, stays):

```rust
    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Connect(err) => ws_retry_after(err),
            Self::Stopped { cause: Some(cause) } => cause.retry_after,
            _ => self.class().retry_after(),
        }
    }
}

/// What the shared socket code reads of this error: the errors it makes
/// itself, and which variant this is.
impl SocketError for UsdmWsError {
    type Path = StreamPath;
    type Cause = StopCause;

    fn refused(code: i64, msg: String) -> Self {
        Self::Refused { code, msg }
    }

    fn no_answer(id: u64, timeout: Duration) -> Self {
        Self::NoAnswer { id, timeout }
    }

    fn response(id: u64, raw: String) -> Self {
        Self::Response { id, raw }
    }

    fn too_many_streams(path: StreamPath, limit: usize) -> Self {
        Self::TooManyStreams { path, limit }
    }

    fn stopped(cause: Option<StopCause>) -> Self {
        Self::Stopped { cause }
    }

    fn cause(&self) -> StopCause {
        StopCause::of(self)
    }

    fn shape(&self) -> Shape<'_> {
        match self {
            Self::Connect(err) => Shape::Connect(err),
            Self::ConnectTimeout(_) => Shape::ConnectTimeout,
            Self::Closed { .. } => Shape::Closed,
            Self::Refused { .. } => Shape::Refused,
            Self::NoAnswer { .. } => Shape::NoAnswer,
            Self::Response { .. } => Shape::Response,
            Self::TooManyStreams { .. } => Shape::TooManyStreams,
            Self::WrongPath { .. } => Shape::WrongPath,
            Self::Frame { .. } => Shape::Frame,
            Self::Stopped { .. } => Shape::Stopped,
        }
    }
}
```

**3c. `polyoxide-binance/src/usdm/ws/protocol.rs`.** Replace lines 1 through 332 (everything above the blank line that precedes `#[cfg(test)]` and `mod tests {`; that blank line stays) with:

```rust
//! USDⓈ-M's stream set for Binance's socket protocol, which lives in
//! `shared::ws` and is generic over it. The supervised tier runs one
//! supervisor per path.

use std::time::Duration;

use polyoxide_ws::Outage;
#[cfg(test)]
use polyoxide_ws::{tungstenite::Message, Decoded, Offline, Protocol, Recovery, WsError};

use crate::{
    shared::ws::StreamSet,
    usdm::ws::{
        error::UsdmWsError,
        event::Update,
        stream::StreamName,
        supervised::{DisconnectReason, Event},
        StreamPath, MAX_NAMES_PER_REQUEST, MIN_REQUEST_INTERVAL,
    },
};

/// USDⓈ-M's streams: two paths, `/market` and `/public`, 200 names a request
/// and one request per 200 ms.
pub(crate) struct UsdmStreams;

impl StreamSet for UsdmStreams {
    type Name = StreamName;
    type Path = StreamPath;
    type Update = Update;
    type Error = UsdmWsError;
    type Event = Event;

    const PATHS: &'static [StreamPath] = StreamPath::ALL;
    const MAX_NAMES_PER_REQUEST: usize = MAX_NAMES_PER_REQUEST;
    const MIN_REQUEST_INTERVAL: Duration = MIN_REQUEST_INTERVAL;

    fn path(name: &StreamName) -> StreamPath {
        name.path()
    }

    fn url(base: &str, path: StreamPath) -> String {
        path.url(base)
    }

    fn decode(text: &str) -> Result<Update, UsdmWsError> {
        Update::from_json(text)
    }

    fn update(update: Update) -> Event {
        Event::Update(Box::new(update))
    }

    fn disconnected(path: StreamPath, outage: Outage<UsdmWsError>) -> Event {
        let reason = match outage {
            Outage::Closed { code, reason } => DisconnectReason::Closed { code, reason },
            Outage::Error(err) => DisconnectReason::Error(err),
            Outage::Rotation => DisconnectReason::Rotation,
            // `Stale`, and any outage a later kit adds.
            _ => DisconnectReason::Stale,
        };
        Event::Disconnected { path, reason }
    }

    fn reconnected(path: StreamPath) -> Event {
        Event::Reconnected { path }
    }
}

/// Binance's protocol on one routed path of USDⓈ-M's streams. Only the unit
/// tests name it.
#[cfg(test)]
pub(crate) type UsdmProtocol = crate::shared::ws::protocol::BinanceProtocol<UsdmStreams>;

/// One change to a path's streams. Only the unit tests name it.
#[cfg(test)]
pub(crate) type Change = crate::shared::ws::protocol::Change<StreamName>;
```

**3d. `polyoxide-binance/src/usdm/ws/client.rs`.** Replace lines 1 through 351 (everything above the blank line that precedes `#[cfg(test)]` and `mod tests {`; that blank line stays) with:

```rust
//! The bare tier: one connection on one path, control methods, and a `Stream`
//! of updates.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
#[cfg(test)]
use futures_util::StreamExt;
#[cfg(test)]
use tokio::time::Instant;

use crate::{
    shared::ws::bare::Bare,
    usdm::ws::{
        error::UsdmWsError, event::Update, protocol::UsdmStreams, stream::StreamName, StreamPath,
        USDM_WS_BASE,
    },
};

#[cfg(test)]
use crate::usdm::ws::MIN_REQUEST_INTERVAL;

/// How long the bare tier waits for a connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the bare tier waits for a request's answer.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// A bare connection to one path's combined stream.
///
/// Requests are batched at most 200 names each and sent no faster than one
/// per 200 ms; each waits for the answer carrying its id, and updates read
/// meanwhile are kept and yielded by the stream afterwards. A stream for the
/// other path, or one that would take the connection past 1024 streams, is
/// refused before anything is sent. The control methods are not cancel-safe,
/// and after a `NoAnswer` the server's membership is unknown: replace the
/// connection rather than reuse it, as the supervised tier does. The server's
/// protocol pings are answered while the caller polls, and its close is
/// answered with the close reply. The stream ends after the server's close,
/// or after the first transport error it yields; a control call after that
/// fails, with the close as [`UsdmWsError::Closed`] or, after an error, as
/// [`UsdmWsError::Connect`] whose kind is `AlreadyClosed`, and sends nothing.
/// Binance closes a connection after 24 hours;
/// the supervised tier ([`UsdmWsBuilder`](crate::usdm::ws::UsdmWsBuilder))
/// replaces it before then, and this tier does not.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::usdm::{types::Symbol, ws::{StreamName, StreamPath, UsdmWs}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::AggTrade(btc)]).await?;
/// while let Some(update) = ws.next().await {
///     println!("{}", serde_json::to_string(&update?)?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct UsdmWs {
    inner: Bare<UsdmStreams>,
}

impl std::fmt::Debug for UsdmWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsdmWs")
            .field("path", &self.inner.path())
            .field("streams", &self.inner.streams().len())
            .field("connection", self.inner.connection())
            .finish_non_exhaustive()
    }
}

impl UsdmWs {
    /// Connect to `path` on the production host and subscribe.
    pub async fn connect(
        path: StreamPath,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, UsdmWsError> {
        Self::connect_to(USDM_WS_BASE, path, streams).await
    }

    /// Connect to `path` on another host, such as a local test server, and
    /// subscribe.
    pub async fn connect_to(
        base_url: &str,
        path: StreamPath,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, UsdmWsError> {
        let streams: Vec<StreamName> = streams.into_iter().collect();
        let mut ws = Self::open(base_url, path, CONNECT_TIMEOUT, ANSWER_TIMEOUT).await?;
        ws.subscribe(&streams).await?;
        Ok(ws)
    }

    /// Open a connection with no streams.
    pub(crate) async fn open(
        base_url: &str,
        path: StreamPath,
        connect_timeout: Duration,
        answer_timeout: Duration,
    ) -> Result<Self, UsdmWsError> {
        Ok(Self {
            inner: Bare::open(base_url, path, connect_timeout, answer_timeout).await?,
        })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with [`UsdmWsError::WrongPath`] or [`UsdmWsError::TooManyStreams`]
    /// before sending anything, and with [`UsdmWsError::Refused`] when the
    /// server answers with an error; batches answered before it stay
    /// subscribed, and [`streams`](Self::streams) says which.
    pub async fn subscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        let path = self.inner.path();
        if let Some(stream) = streams.iter().find(|s| s.path() != path) {
            return Err(UsdmWsError::WrongPath {
                stream: stream.clone(),
                path,
            });
        }
        self.inner.subscribe(streams).await
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub async fn unsubscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        self.inner.unsubscribe(streams).await
    }

    /// The stream names the server says this connection carries.
    pub async fn list_subscriptions(&mut self) -> Result<Vec<String>, UsdmWsError> {
        self.inner.list_subscriptions().await
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub async fn ping(&mut self) -> Result<Duration, UsdmWsError> {
        self.inner.ping().await
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), UsdmWsError> {
        self.inner.close().await
    }

    /// The streams this connection carries.
    pub fn streams(&self) -> &[StreamName] {
        self.inner.streams()
    }

    /// The path this connection is on.
    pub fn path(&self) -> StreamPath {
        self.inner.path()
    }

    /// The server's close code and reason, once it has closed the connection
    /// or the stream has ended without a close frame.
    #[cfg(test)]
    fn close_frame(&self) -> Option<(Option<u16>, String)> {
        self.inner.close_frame()
    }
}

impl Stream for UsdmWs {
    type Item = Result<Update, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}
```

**3e. `polyoxide-binance/src/usdm/ws/supervised.rs`.** Replace lines 1 through 748 (everything above the blank line that precedes `#[cfg(test)]` and `mod tests {`; that blank line stays) with the following. The doc comments are the old ones, word for word; the moved bodies are the new code.

```rust
//! The supervised tier: one kit `Supervisor` per routed path, with keep-alive
//! on the wall clock, staleness detection, reconnect with a paced replay of
//! the path's streams, rotation before Binance's 24-hour cutoff and outage
//! markers, behind a router that sends each membership change to its path's
//! connection, opens a path for its first stream, and merges every path's
//! events into one stream.

use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
#[cfg(test)]
use std::time::Duration;

use futures_util::Stream;
use polyoxide_ws::Timing;

use crate::{
    shared::ws::{
        supervised::{self as shared_supervised, Handle, Running, Settings},
        DEFAULT_MAX_CONNECTION_AGE,
    },
    usdm::ws::{
        error::UsdmWsError, event::Update, protocol::UsdmStreams, stream::StreamName, StreamPath,
        USDM_WS_BASE,
    },
};

/// What the supervised stream yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// One frame of one stream, boxed, as the other variants are small.
    Update(Box<Update>),
    /// A path's connection was lost. Every update on that path is stale until
    /// the matching [`Event::Reconnected`], which always follows while the
    /// client runs.
    Disconnected {
        /// The path that went down.
        path: StreamPath,
        /// Why.
        reason: DisconnectReason,
    },
    /// The path is current again: a fresh connection has replayed its
    /// streams, or the path's last stream left during the outage, so nothing
    /// on it is stale. Sent once per outage, however many attempts it took.
    /// Drop book state for the path and resync.
    Reconnected {
        /// The path that came back.
        path: StreamPath,
    },
}

/// Why a path's connection went down.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed it, with the code and reason it sent, or ended the
    /// stream with neither.
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },
    /// A transport error, a replay left unanswered for the staleness window,
    /// or a failed first connect of a path opened by a membership change.
    Error(UsdmWsError),
    /// Nothing arrived, pongs included, for the staleness window, or a
    /// membership change went unanswered for it.
    Stale,
    /// Replaced on purpose before Binance's 24-hour cutoff.
    Rotation,
}

impl fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed { code, reason } => match code {
                Some(code) => write!(f, "closed by the server ({code} {reason})"),
                None => f.write_str("closed by the server"),
            },
            Self::Error(err) => write!(f, "{err}"),
            Self::Stale => f.write_str("no frame within the staleness window"),
            Self::Rotation => f.write_str("rotated before the 24-hour cutoff"),
        }
    }
}

/// Builder for a supervised connection.
///
/// Each routed path's connection runs on its own task, on the socket kit's
/// `Supervisor`, and logs under the `polyoxide_ws` target.
///
/// While the consumer leaves 1024 events unread, the paths stop reading but
/// keep pinging, and that wait is not silence: a slow consumer is never
/// reported [`Stale`](DisconnectReason::Stale), and no event is dropped. The
/// server's own pings go unanswered meanwhile. Binance documents that it
/// closes a connection that has not answered them for 10 minutes; that close
/// is reconnected like any other, and the events already queued are kept.
#[derive(Debug, Clone)]
pub struct UsdmWsBuilder {
    base_url: String,
    timing: Timing,
    streams: Vec<StreamName>,
}

impl Default for UsdmWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdmWsBuilder {
    /// Defaults: the production host, a ping every 20 s, stale after 30 s of
    /// silence, reconnect backoff from 500 ms doubling to 60 s, a 10 s connect
    /// timeout, rotation after 23 h 50 min, and no streams.
    pub fn new() -> Self {
        let mut timing = Timing::default();
        timing.max_connection_age = Some(DEFAULT_MAX_CONNECTION_AGE);
        Self {
            base_url: USDM_WS_BASE.to_owned(),
            timing,
            streams: Vec::new(),
        }
    }

    /// Connect to another host, such as a local test server.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    polyoxide_ws::timing_setters! {
        /// How often each connection sends a protocol ping, whatever its
        /// traffic, and also while the consumer leaves 1024 events unread.
        /// None goes out while a membership change is in flight, which
        /// `stale_after` bounds. Binance closes a connection that sends more
        /// than 10 messages a second.
        ping_interval;
        /// Silence after which a connection is presumed dead and replaced.
        /// Pongs and the server's pings count, so a quiet connection that
        /// answers stays up, and time the consumer leaves 1024 events unread
        /// does not count.
        ///
        /// Also bounds each replay and each membership change as a whole, its
        /// 200 ms pacing included: one the server leaves unanswered that long
        /// replaces the connection. A replay of 1024 streams is six requests,
        /// about a second.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max`, with no cap
        /// on attempts, and back to `initial` after a connection that
        /// delivered an update. Both are raised to at least 1 ms, and an
        /// `initial` above `max` is lowered to `max`. A reconnect refused with
        /// a `Retry-After` waits at least that long, up to `max` (DRIFT R11).
        backoff;
        /// How long one connection attempt may take to open.
        connect_timeout;
        /// How long a connection may live before it is replaced, without
        /// backoff, ahead of Binance's 24-hour cutoff. Each replacement is a
        /// new connection, and Binance allows 300 connection attempts per 5
        /// minutes per IP.
        max_connection_age;
    }

    /// The streams to open with.
    pub fn streams(mut self, streams: impl IntoIterator<Item = StreamName>) -> Self {
        self.streams = streams.into_iter().collect();
        self
    }

    /// Open a connection for each path the streams need, subscribe, and start
    /// supervising.
    ///
    /// Fails if any first connect fails, closing the others: a setup that is
    /// wrong is reported, not retried. With no streams it opens nothing until
    /// the first [`MembershipHandle::subscribe`].
    pub async fn connect(self) -> Result<SupervisedUsdmWs, UsdmWsError> {
        let inner = shared_supervised::connect::<UsdmStreams>(Settings {
            base_url: self.base_url,
            timing: self.timing,
            streams: self.streams,
        })
        .await?;
        Ok(SupervisedUsdmWs { inner })
    }
}

/// Changes the streams of a running [`SupervisedUsdmWs`].
///
/// Each stream is routed to its path's connection, opening it if this is the
/// path's first stream and closing it when its last leaves. The membership is
/// a set: subscribing a stream twice holds it once, and counting references is
/// the caller's job.
///
/// A change is recorded before it is sent. During a path's outage it is
/// answered `Ok` at once, and the replay applies it; so is a change whose
/// connection fails under it, or one the server leaves unanswered for
/// `stale_after`, which replaces the connection. A change that arrives while
/// a connect or a replay is in flight waits for that attempt. Calls are served
/// one at a time across paths, so a change for a healthy path can wait behind
/// another path's attempt.
///
/// A call fails when the server refuses it ([`UsdmWsError::Refused`]), when it
/// would pass 1024 streams on a path ([`UsdmWsError::TooManyStreams`], nothing
/// sent), when the first connect of a path it opens fails in a way retrying
/// cannot fix ([`UsdmWsError::Connect`]), or when the client has stopped
/// ([`UsdmWsError::Stopped`]). Its `cause` is the error a path stopped on once
/// the router has that path's stop; a call in flight when the path stops can
/// get `Stopped { cause: None }`. A subscription of more than 200 names is sent
/// in batches, so one refused part-way leaves the earlier batches applied.
///
/// A call waits for the path's task, which takes no change while the consumer
/// leaves 1024 events unread. Make membership changes from a task other than
/// the one draining the stream, or keep draining while they run.
#[derive(Debug, Clone)]
pub struct MembershipHandle {
    inner: Handle<UsdmStreams>,
}

impl MembershipHandle {
    /// Subscribe to more streams.
    pub async fn subscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.inner.subscribe(streams.into_iter().collect()).await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub async fn unsubscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.inner.unsubscribe(streams.into_iter().collect()).await
    }
}

/// A supervised set of connections: a `Stream` of [`Event`]s that survives
/// drops, stalls and Binance's 24-hour cutoff.
///
/// Every [`Event::Disconnected`] is followed by an [`Event::Reconnected`] for
/// the same path while the client runs. A path that closes because its last
/// stream left, while it is up, yields neither.
///
/// A fatal error on any path, such as the server refusing a replay after a
/// reconnect, is yielded as `Err` and then the stream ends, every path
/// closed. An `Err(UsdmWsError::Frame { .. })`, a frame that did not decode,
/// does not end it.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::usdm::{types::Symbol, ws::{Event, StreamName, UsdmWsBuilder, UsdmWsError}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut feed = UsdmWsBuilder::new()
///     .streams([StreamName::MarkPrice(btc.clone()), StreamName::BookTicker(btc)])
///     .connect()
///     .await?;
/// let membership = feed.membership();
/// while let Some(event) = feed.next().await {
///     let event = match event {
///         Ok(event) => event,
///         // A frame that did not decode is skipped; anything else ends the stream.
///         Err(UsdmWsError::Frame { .. }) => continue,
///         Err(err) => return Err(err.into()),
///     };
///     match event {
///         Event::Update(update) => println!("{}", serde_json::to_string(&update)?),
///         Event::Disconnected { path, reason } => eprintln!("{path} down: {reason}"),
///         Event::Reconnected { path } => eprintln!("{path} back"),
///         _ => {}
///     }
/// }
/// # membership.subscribe([StreamName::AllMarkPrices]).await?;
/// # Ok(())
/// # }
/// ```
pub struct SupervisedUsdmWs {
    inner: Running<UsdmStreams>,
}

impl fmt::Debug for SupervisedUsdmWs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupervisedUsdmWs")
            .field("running", &self.inner.is_running())
            .finish_non_exhaustive()
    }
}

impl SupervisedUsdmWs {
    /// A handle for changing streams while the stream runs.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            inner: self.inner.membership(),
        }
    }

    /// Close every connection and stop. Every [`MembershipHandle`] answers
    /// [`UsdmWsError::Stopped`] afterwards.
    ///
    /// Fails with [`UsdmWsError::Stopped`] if a path had stopped the client
    /// on an error, which the stream yielded: its `cause` is that error, as
    /// [`Classify`](polyoxide_venue::Classify) read it, and so is the cause
    /// every later membership call carries. That holds once the router has the
    /// path's stop: a `close()` made right after the stream yielded the
    /// stopping error can return `Ok` or `Stopped { cause: None }`. Read the
    /// stream to its end first to be sure of the cause.
    pub async fn close(self) -> Result<(), UsdmWsError> {
        self.inner.close().await
    }
}

impl Stream for SupervisedUsdmWs {
    type Item = Result<Event, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_event(cx)
    }
}
```

**3f. `.claude/rules/binance.md`** (the `## Streams (`usdm-ws`)` section). Replace the line

```
- The private `UsdmProtocol` (`src/usdm/ws/protocol.rs`) keeps Binance's rows: pings on the wall
```

with the two lines (the next line, `  clock, also while the queue is full, ...`, stays)

```
- The crate-private `BinanceProtocol<S>` (`src/shared/ws/protocol.rs`, generic over the product's
  `StreamSet`; `UsdmProtocol` is its alias on `UsdmStreams`) keeps Binance's rows: pings on the wall
```

and replace the first bullet's last line

```
  stream, and forwards both paths into one queue of 1024.
```

with

```
  stream, and forwards both paths into one queue of 1024. The router, the request correlation and
  the bare connection are `shared::ws`'s (`src/shared/ws/`), generic over `StreamSet`, and
  `usdm::ws`'s public types wrap them.
```

- [ ] **Step 4: Run it to see it pass**

Run, in order:

```bash
nix develop --command cargo fmt --all
nix develop --command cargo test -p polyoxide-binance --all-features
bash -c 'for f in client error protocol supervised; do diff <(git show HEAD:polyoxide-binance/src/usdm/ws/$f.rs | sed -n "/^mod tests {\$/,\$p") <(sed -n "/^mod tests {\$/,\$p" polyoxide-binance/src/usdm/ws/$f.rs) && echo "$f: mod tests unchanged"; done'
```

Expected: the same tests pass as in Step 2, plus the two `shared::ws::tests`; `supervision_usdm`, `supervision_usdm_edges`, `supervision_usdm_kit` and `wire_agreement_usdm_ws` among them; and the loop prints `client: mod tests unchanged`, `error: mod tests unchanged`, `protocol: mod tests unchanged`, `supervised: mod tests unchanged` with no diff output. (`cargo fmt` runs over the new code only; the tests were already formatted.)

Then the story's gates, in order:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-targets --all-features -- -D warnings
nix develop --command cargo test -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features'
nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps
python3 scripts/gen_registry.py --check
python3 scripts/venue_gate.py
python3 scripts/live_unwraps.py
python3 scripts/test_body_diff.py --baseline v0.40.0
python3 scripts/api_removals.py check --baseline v0.40.0
bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_claude_rules.py -q'
```

Expected: all pass; `test_body_diff.py` and `api_removals.py` report nothing new. If clippy names an unused import in a `usdm/ws` file, the cause is an import that only a tests module uses and that is not under `#[cfg(test)]`; fix it at module level, never inside `mod tests`. If rustc rejects a generic bound in `shared::ws` (an `Unpin`, `Send` or ambiguous-method error), fix it in the shared file: the names contract fixes the signatures, not the bodies.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(binance): usdm::ws on shared::ws, its unit tests untouched (binance spot ws, Story 2)" -- polyoxide-binance/src/shared/ws/mod.rs polyoxide-binance/src/usdm/ws/error.rs polyoxide-binance/src/usdm/ws/protocol.rs polyoxide-binance/src/usdm/ws/client.rs polyoxide-binance/src/usdm/ws/supervised.rs .claude/rules/binance.md
```

## Story 3: Spot stream names, payloads and errors

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/src/spot/ws/stream.rs`, `polyoxide-binance/src/spot/ws/event.rs`, `polyoxide-binance/src/spot/ws/error.rs`, `polyoxide-binance/src/spot/ws/fixtures.rs`, `polyoxide-binance/tests/wire_agreement_spot_ws.rs`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide-binance@spot-ws.txt`, `.github/scripts/tests/test_classify_coverage.py`, `.claude/rules/ci-gates.md`, `.claude/rules/wire-enums.md`, `Cargo.lock`
- **Interfaces:** exposes "Story 3 → Stories 4 and 5"; uses Story 1's fixtures and plan 1's `spot::types::{Symbol, Interval, Level}`
- **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py -q'`
- **Review:** deep (public payload types every consumer decodes into, and an error's classes)

Adds the `spot-ws` feature and `spot::ws`'s data half, all of it decoding and no socket: the module root with `StreamPath` and the three limit constants, `error` (`SpotWsError`, `StopCause`, `From<WsError>`, the `Classify` rows of `UsdmWsError` less `WrongPath`), `stream` (`StreamName`, `DepthLevels`, `DepthSpeed`, `InvalidStreamName`), `fixtures` (Story 1's frames), and `event` (the nine payload types, `Payload`, `Update::from_json`, which takes a partial-depth frame's symbol from the envelope and keeps a single-symbol object whose `e` is not its stream's as `Payload::Unknown`). `tests/wire_agreement_spot_ws.rs` holds each fixture to its type, and the dep allowlists gain `polyoxide-binance@spot-ws.txt`, with the rules text that says what they hold (`.claude/rules/ci-gates.md`, `.claude/rules/wire-enums.md`) and the classify sweep's two non-vacuity lists naming the new error. None of the code holds a landmine marker. Nothing here opens a connection; Story 4 builds the clients on these types. One temporary `allow(dead_code)` sits on `StreamPath::url`, which nothing calls until Story 4's `SpotStreams::url`; that story removes it. `Cargo.lock` does not change (a feature adds no dependency), so a commit names it only if `git status` shows it modified.

### Task 3.1: The `spot-ws` feature, the module root and the error

**Files:**
- Modify: `polyoxide-binance/Cargo.toml` (the `[features]` table and the `polyoxide-ws` comment)
- Modify: `polyoxide-binance/src/spot/mod.rs:25-32` (the module list)
- Modify: `polyoxide-binance/src/lib.rs:52-58` (the spot `Classify` assertion)
- Modify: `.claude/rules/wire-enums.md:8` (the `paths:` list) and `:41` (the hand-written list)
- Create: `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/src/spot/ws/error.rs`
- Test: `polyoxide-binance/src/spot/ws/mod.rs` and `polyoxide-binance/src/spot/ws/error.rs` (their `tests` modules)

- [ ] **Step 1: Write the failing tests**

In `polyoxide-binance/Cargo.toml`, add the feature after `usdm-ws` and rewrite the `test-server` comment. The current lines:

```toml
# The USDⓈ-M market streams, `usdm::ws`, on the socket kit.
usdm-ws = ["usdm", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]
# Exposes the captured frames, `usdm::ws::fixtures`, for downstream tests, with
# `usdm-ws`; it turns no module on. Not for consumers.
test-server = []
```

become:

```toml
# The USDⓈ-M market streams, `usdm::ws`, on the socket kit.
usdm-ws = ["usdm", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]
# The spot market streams, `spot::ws`, on the same kit.
spot-ws = ["spot", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]
# Exposes the captured frames, `usdm::ws::fixtures` with `usdm-ws` and
# `spot::ws::fixtures` with `spot-ws`, for downstream tests; it turns no module
# on. Not for consumers.
test-server = []
```

and the dependency comment

```toml
# The socket kit (`usdm-ws`): connect, the bare tier, the socket error and the
```

becomes

```toml
# The socket kit (`usdm-ws`, `spot-ws`): connect, the bare tier, the socket error and the
```

In `polyoxide-binance/src/spot/mod.rs`, the module list currently ends

```rust
pub mod types;
pub mod venue;
pub mod weight;
```

Add after `pub mod weight;`:

```rust
#[cfg(feature = "spot-ws")]
pub mod ws;
```

Create `polyoxide-binance/src/spot/ws/mod.rs` with only the module declaration for now:

```rust
//! Spot market streams on `stream.binance.com`.

pub mod error;
```

Create `polyoxide-binance/src/spot/ws/error.rs` holding only the tests (Step 3 adds the code above them):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_ws::tungstenite::{self, http};

    fn http_error(status: u16) -> SpotWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> SpotWsError {
        SpotWsError::from(WsError::from(err))
    }

    #[test]
    fn a_server_close_and_a_connect_timeout_keep_their_own_variants() {
        let closed = SpotWsError::from(WsError::closed(Some(1008), "Too many requests"));
        assert!(
            matches!(&closed, SpotWsError::Closed { code: Some(1008), reason } if reason == "Too many requests"),
            "{closed:?}"
        );
        let after = Duration::from_secs(10);
        assert!(matches!(
            SpotWsError::from(WsError::connect_timeout(after)),
            SpotWsError::ConnectTimeout(timeout) if timeout == after
        ));
        // The bare tier maps an unanswered request itself, with its id.
        assert!(matches!(
            SpotWsError::from(WsError::no_answer(after)),
            SpotWsError::Connect(_)
        ));
        assert!(matches!(
            transport(tungstenite::Error::ConnectionClosed),
            SpotWsError::Connect(_)
        ));
    }

    #[test]
    fn a_refused_429_surfaces_its_retry_after() {
        let err = transport(tungstenite::Error::Http(
            http::Response::builder()
                .status(429)
                .header("Retry-After", "7")
                .body(None)
                .unwrap(),
        ));
        assert_eq!(
            err.class(),
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(7))
            }
        );
        assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
        assert!(err.is_retriable());
    }

    #[test]
    fn a_refused_503_surfaces_its_retry_after() {
        // Every refusal's `Retry-After` is the error's wait; only a 429's class
        // carries it.
        let err = transport(tungstenite::Error::Http(
            http::Response::builder()
                .status(503)
                .header("Retry-After", "30")
                .body(None)
                .unwrap(),
        ));
        assert_eq!(err.class(), Class::Unavailable { code: None });
        assert_eq!(err.retry_after(), Some(Duration::from_secs(30)));
    }

    #[test]
    fn a_stop_follows_its_cause() {
        // A stop on a 451 refusal is that refusal's class and not a fault, a
        // stop on a 429 keeps its wait, and one with no cause is the client's
        // own `InvalidRequest`.
        let region = SpotWsError::Stopped {
            cause: Some(StopCause::of(&http_error(451))),
        };
        assert_eq!(region.class(), Class::Restricted);
        assert!(!region.is_fault());
        assert_eq!(
            region.to_string(),
            "the supervised connection has stopped: WebSocket transport error: HTTP error: 451 \
             Unavailable For Legal Reasons"
        );
        let throttled = SpotWsError::Stopped {
            cause: Some(StopCause::of(&transport(tungstenite::Error::Http(
                http::Response::builder()
                    .status(429)
                    .header("Retry-After", "30")
                    .body(None)
                    .unwrap(),
            )))),
        };
        assert_eq!(throttled.retry_after(), Some(Duration::from_secs(30)));
        let bare = SpotWsError::Stopped { cause: None };
        assert_eq!(bare.class(), Class::InvalidRequest);
        assert!(bare.is_fault());
        assert_eq!(bare.to_string(), "the supervised connection has stopped");
    }

    #[test]
    fn spot_ws_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SpotWsError>();
    }

    #[test]
    fn the_transport_follows_the_socket_table() {
        let bad_header = http::header::HeaderName::from_bytes(b"in valid").unwrap_err();
        let rows = [
            (tungstenite::Error::ConnectionClosed, Class::Network),
            (
                tungstenite::Error::Io(std::io::ErrorKind::ConnectionReset.into()),
                Class::Network,
            ),
            (
                tungstenite::Error::Protocol(
                    tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                ),
                Class::Network,
            ),
            (
                tungstenite::Error::WriteBufferFull(tungstenite::Message::Close(None)),
                Class::Network,
            ),
            (tungstenite::Error::Utf8, Class::Network),
            (tungstenite::Error::AlreadyClosed, Class::InvalidRequest),
            (
                tungstenite::Error::Url(tungstenite::error::UrlError::NoHostName),
                Class::InvalidRequest,
            ),
            (
                tungstenite::Error::HttpFormat(bad_header.into()),
                Class::InvalidRequest,
            ),
            (
                tungstenite::Error::Tls(tungstenite::error::TlsError::InvalidDnsName),
                Class::InvalidRequest,
            ),
            (tungstenite::Error::AttackAttempt, Class::InvalidRequest),
            (
                tungstenite::Error::Capacity(tungstenite::error::CapacityError::TooManyHeaders),
                Class::InvalidRequest,
            ),
        ];
        for (err, class) in rows {
            let err = transport(err);
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
        }
        // (status, class, is_fault)
        for (status, class, fault) in [
            (401, Class::Unauthorized, true),
            // Binance's firewall, as on REST.
            (403, Class::Restricted, true),
            (404, Class::VenueRefusal { code: None }, true),
            // A ban the client earned by overspending.
            (418, Class::Restricted, true),
            (429, Class::RateLimited { retry_after: None }, true),
            // A region block, as `SpotError::RegionBlocked` is on REST.
            (451, Class::Restricted, false),
            (503, Class::Unavailable { code: None }, true),
            (200, Class::Decode, true),
        ] {
            assert_eq!(http_error(status).class(), class, "{status}");
            assert_eq!(http_error(status).is_fault(), fault, "{status}");
        }
    }

    #[test]
    fn every_variant_classifies() {
        let rows = [
            (http_error(503), Class::Unavailable { code: None }),
            (
                SpotWsError::ConnectTimeout(Duration::from_secs(10)),
                Class::Network,
            ),
            (
                SpotWsError::Closed {
                    code: Some(1008),
                    reason: "Too many requests".into(),
                },
                Class::VenueRefusal {
                    code: Some("1008".into()),
                },
            ),
            (
                SpotWsError::Closed {
                    code: Some(1001),
                    reason: String::new(),
                },
                Class::Network,
            ),
            (
                SpotWsError::Closed {
                    code: None,
                    reason: String::new(),
                },
                Class::Network,
            ),
            (
                SpotWsError::Refused {
                    code: 2,
                    msg: "Invalid request".into(),
                },
                Class::VenueRefusal {
                    code: Some("2".into()),
                },
            ),
            (
                SpotWsError::NoAnswer {
                    id: 3,
                    timeout: Duration::from_secs(10),
                },
                Class::Network,
            ),
            (
                SpotWsError::Response {
                    id: 4,
                    raw: "{}".into(),
                },
                Class::Decode,
            ),
            (
                SpotWsError::TooManyStreams {
                    path: StreamPath::Stream,
                    limit: 1024,
                },
                Class::InvalidRequest,
            ),
            (
                SpotWsError::Frame {
                    stream: "btcusdt@aggTrade".into(),
                    raw: "{}".into(),
                    reason: "missing field".into(),
                },
                Class::Decode,
            ),
            (SpotWsError::Stopped { cause: None }, Class::InvalidRequest),
        ];
        for (err, class) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
```

Add the root's tests to `polyoxide-binance/src/spot/ws/mod.rs`, below the `pub mod error;` line:

```rust

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_has_one_path_and_it_is_stream() {
        assert_eq!(StreamPath::ALL, &[StreamPath::Stream]);
        assert_eq!(StreamPath::Stream.as_str(), "stream");
        assert_eq!(StreamPath::Stream.to_string(), "stream");
    }

    #[test]
    fn the_url_appends_the_one_path_to_the_base() {
        assert_eq!(
            StreamPath::Stream.url(SPOT_WS_BASE),
            "wss://stream.binance.com:9443/stream"
        );
        // A scripted server's base, with or without a trailing slash.
        assert_eq!(
            StreamPath::Stream.url("ws://127.0.0.1:4000"),
            "ws://127.0.0.1:4000/stream"
        );
        assert_eq!(
            StreamPath::Stream.url("ws://127.0.0.1:4000/"),
            "ws://127.0.0.1:4000/stream"
        );
    }

    #[test]
    fn the_limits_are_the_measured_ones() {
        // OBSERVED.md, `## Spot`: 1024 names in one request held; a request every
        // 250 ms was closed with 1008, every 500 ms held for 45 s.
        assert_eq!(MAX_STREAMS_PER_CONNECTION, 1024);
        assert_eq!(MAX_NAMES_PER_REQUEST, 1024);
        assert_eq!(MIN_REQUEST_INTERVAL, Duration::from_millis(500));
    }
}
```

In `polyoxide-binance/src/lib.rs` the spot assertion currently reads:

```rust
// The spot module's error types, in the same assertion `usdm`'s are in.
#[cfg(feature = "spot")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<spot::SpotError>();
    is::<spot::types::InvalidSymbol>();
};
```

Change it to:

```rust
// The spot module's error types, in the same assertion `usdm`'s are in.
#[cfg(feature = "spot")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<spot::SpotError>();
    is::<spot::types::InvalidSymbol>();
    #[cfg(feature = "spot-ws")]
    is::<spot::ws::SpotWsError>();
};
```

In `.claude/rules/wire-enums.md` (AD-21: a commit that adds a rule's subject edits the rule's text; `test_claude_rules.py` fails on a `paths:` glob that matches no file, so the glob lands in this commit, which creates `spot/ws/mod.rs`), the `paths:` list currently has, at line 8:

```yaml
  - "polyoxide-binance/src/usdm/ws/mod.rs"
```

Add below it:

```yaml
  - "polyoxide-binance/src/spot/ws/mod.rs"
```

and line 41 currently reads

```
relay's `WalletType`; Binance's `StreamPath` (`usdm/ws/mod.rs`); gamma's `ParentEntityType`; and
```

Change it to

```
relay's `WalletType`; Binance's `StreamPath` (`usdm/ws/mod.rs`, `spot/ws/mod.rs`); gamma's `ParentEntityType`; and
```

(`DepthLevels` and `DepthSpeed` are not listed: usdm's are not.) Keep the paragraph's wrapping otherwise; if the longer line exceeds the file's wrap, rewrap that paragraph only.

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws`
Expected: the build fails with `error[E0412]: cannot find type `SpotWsError` in this scope`, `cannot find type `StreamPath``, `cannot find value `MAX_STREAMS_PER_CONNECTION`` and `cannot find type `StopCause``, and no test runs.

- [ ] **Step 3: Implement**

Replace the whole of `polyoxide-binance/src/spot/ws/mod.rs` (the stub and the tests of Step 1) with the root below; keep the `tests` module of Step 1 verbatim at the end of the file, after the `impl` blocks:

```rust
//! Spot market streams on `stream.binance.com`.
//!
//! Spot has one combined-stream path, `/stream`, on `SPOT_WS_BASE`. Binance
//! acknowledges every `SUBSCRIBE`, including a stream type that does not exist,
//! an uppercase symbol and an unknown symbol, and none of those deliver
//! anything, so a stream is named only through `StreamName`, which builds the
//! spellings Binance serves. A frame is an `Update`: the stream it arrived on
//! and its `Payload`.
//!
//! Binance enforces its connection rules by closing the connection, so the
//! clients hold them first: at most `MAX_STREAMS_PER_CONNECTION` streams,
//! `MAX_NAMES_PER_REQUEST` names per request and one request per
//! `MIN_REQUEST_INTERVAL`. The measurements are in
//! `docs/specs/binance/OBSERVED.md`, under `## Spot`.

pub mod error;

use std::{fmt, time::Duration};

pub use error::{SpotWsError, StopCause};

/// The production stream host. The client appends `/stream`.
pub const SPOT_WS_BASE: &str = "wss://stream.binance.com:9443";

/// Streams one connection may carry. Binance documents 1024 and 1024 were
/// measured held (2026-11-02); the client never sends a 1025th.
pub const MAX_STREAMS_PER_CONNECTION: usize = 1024;

/// Stream names per `SUBSCRIBE` or `UNSUBSCRIBE`. Binance documents no limit;
/// one request of 1024 names (23,598 bytes) was acknowledged and held, as were
/// 200 and 500 (2026-11-02), so a request carries as many names as a
/// connection holds.
pub const MAX_NAMES_PER_REQUEST: usize = 1024;

/// The shortest gap between two requests on one connection. Binance allows 5
/// incoming messages a second, pings, pongs and requests counted. A request
/// every 250 ms, with a client ping and a pong, was closed about 16 s in with
/// `1008 "Too many requests"` (at most 6 messages in a second); a request every
/// 500 ms held for 45 s with two pings and two pongs (at most 4 in a second).
pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(500);

/// Which routed path a connection is on. Spot has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamPath {
    /// `/stream`: every spot market stream.
    Stream,
}

impl StreamPath {
    /// Every path.
    pub const ALL: &'static [Self] = &[Self::Stream];

    /// The path segment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stream => "stream",
        }
    }

    /// The combined-stream URL on `base`.
    // Story 4's `SpotStreams::url` is the first caller; it removes this allow.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn url(self, base: &str) -> String {
        format!("{}/{}", base.trim_end_matches('/'), self.as_str())
    }
}

impl fmt::Display for StreamPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
```

The module doc names its items in plain backticks, not intra-doc links, because `StreamName`, `Update` and `Payload` arrive in the next tasks.

Put the implementation above the `tests` module of `polyoxide-binance/src/spot/ws/error.rs`:

```rust
//! Errors of the spot socket tiers, and their classes.

use std::time::Duration;

use polyoxide_venue::{class_for_close_code, Class, Classify};
use polyoxide_ws::{Kind, WsError};
use thiserror::Error;

use crate::spot::ws::StreamPath;

polyoxide_venue::impl_ws_classification!();

/// Error type for the spot socket tiers.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SpotWsError {
    /// The connection could not be opened, or failed mid-stream: the kit's
    /// one socket error, read through its [`Kind`]. A
    /// server close and a connect timeout have variants of their own.
    #[error("WebSocket transport error: {0}")]
    Connect(#[source] WsError),

    /// The connection did not open within the connect timeout.
    #[error("no connection within {0:?}")]
    ConnectTimeout(Duration),

    /// The server closed the connection, with its close code and reason when
    /// it sent them: `1008 "Too many requests"` after more than 5 incoming
    /// messages in a second.
    #[error("the server closed the connection ({code:?}: {reason})")]
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },

    /// The server answered a request with `{"error": {"code", "msg"}}`.
    #[error("the server refused the request: {code} {msg}")]
    Refused {
        /// Binance's code.
        code: i64,
        /// Binance's message.
        msg: String,
    },

    /// No answer to a request, or no pong to a ping (id 0), within the answer
    /// timeout.
    #[error("no answer to request {id} within {timeout:?}")]
    NoAnswer {
        /// The request id.
        id: u64,
        /// How long the client waited.
        timeout: Duration,
    },

    /// An answer that was not the documented shape.
    #[error("malformed answer to request {id}: {raw}")]
    Response {
        /// The request id.
        id: u64,
        /// The answer as sent.
        raw: String,
    },

    /// A subscribe that would take a connection past Binance's cap. Nothing
    /// was sent: a 1025th stream is not one the client probes, since the
    /// server's answer to it closes the connection and loses all 1024.
    #[error("a {path} connection carries at most {limit} streams")]
    TooManyStreams {
        /// The path whose connection is full.
        path: StreamPath,
        /// The cap.
        limit: usize,
    },

    /// A text frame that did not decode as an update.
    #[error("a frame on {stream:?} did not decode: {reason}")]
    Frame {
        /// The envelope's stream name, or empty when there was none.
        stream: String,
        /// The frame as sent.
        raw: String,
        /// Why it did not decode.
        reason: String,
    },

    /// The supervised tier has stopped; its handles can no longer be used.
    ///
    /// `cause` is the error a path stopped on, as [`Classify`] read it, so a
    /// stream stopped by a reconnect refused with 451 is classed and tagged as
    /// that refusal is. `None` when the client stopped for another reason,
    /// such as a task that panicked, or before the stop's cause was known.
    #[error("the supervised connection has stopped{}", describe_stop(cause.as_ref()))]
    Stopped {
        /// The error a path stopped on.
        cause: Option<StopCause>,
    },
}

/// The error a supervised path stopped on, kept for
/// [`SpotWsError::Stopped`] after the error itself was yielded: its class,
/// whether it was a fault, the wait it asked for, and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StopCause {
    /// The error's class.
    pub class: Class,
    /// Whether the error was a fault.
    pub is_fault: bool,
    /// The wait the error asked for.
    pub retry_after: Option<Duration>,
    /// The error's text.
    pub message: String,
}

impl StopCause {
    /// The cause of a stop on `err`.
    pub fn of(err: &SpotWsError) -> Self {
        Self {
            class: err.class(),
            is_fault: err.is_fault(),
            retry_after: err.retry_after(),
            message: err.to_string(),
        }
    }
}

/// The tail of a stop's message: its cause's text, when it has one.
fn describe_stop(cause: Option<&StopCause>) -> String {
    cause.map_or_else(String::new, |cause| format!(": {}", cause.message))
}

/// A server close is [`SpotWsError::Closed`], a connect timeout
/// [`SpotWsError::ConnectTimeout`], and every other socket failure
/// [`SpotWsError::Connect`].
impl From<WsError> for SpotWsError {
    fn from(err: WsError) -> Self {
        match err.kind() {
            Kind::Closed { code, reason } => Self::Closed {
                code,
                reason: reason.to_owned(),
            },
            Kind::ConnectTimeout { after } => Self::ConnectTimeout(after),
            _ => Self::Connect(err),
        }
    }
}

/// The transport by the one socket table
/// (`polyoxide_venue::impl_ws_classification!`), a server close by its close
/// code, a refused request a `VenueRefusal` with Binance's code in decimal,
/// an answer or frame that did not parse a `Decode`, and the client's own
/// refusals an `InvalidRequest`.
///
/// One row is Binance's own, ahead of the table (D14): a handshake refused
/// with `403` is `Restricted`, as on REST. Binance's firewall answers it, and
/// it is not a credential failure, since the streams need none.
///
/// Every error is a fault except a handshake refused with `451`: Binance does
/// not serve the caller's region, a defined outcome, as
/// [`SpotError::RegionBlocked`](crate::spot::SpotError::RegionBlocked) is on
/// REST. A `418` ban and a `403` from the firewall stay faults. A refused
/// handshake's `Retry-After`, of any status, is the error's wait.
///
/// [`Stopped`](SpotWsError::Stopped) follows its cause, so a stop on a 451
/// refusal is `Restricted` and not a fault; one with no cause is an
/// `InvalidRequest` and a fault.
impl Classify for SpotWsError {
    fn class(&self) -> Class {
        match self {
            Self::Connect(err) => match err.kind() {
                Kind::Refused { status: 403, .. } => Class::Restricted,
                _ => ws_class(err),
            },
            Self::ConnectTimeout(_) | Self::NoAnswer { .. } => Class::Network,
            Self::Closed { code, .. } => class_for_close_code(*code),
            Self::Refused { code, .. } => Class::VenueRefusal {
                code: Some(code.to_string().into()),
            },
            Self::Response { .. } | Self::Frame { .. } => Class::Decode,
            Self::Stopped { cause: Some(cause) } => cause.class.clone(),
            Self::TooManyStreams { .. } | Self::Stopped { cause: None } => Class::InvalidRequest,
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Connect(err) => ws_is_fault(err),
            Self::Stopped { cause: Some(cause) } => cause.is_fault,
            _ => true,
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Connect(err) => ws_retry_after(err),
            Self::Stopped { cause: Some(cause) } => cause.retry_after,
            _ => self.class().retry_after(),
        }
    }
}

```

(`#[cfg(test)] mod tests { … }` of Step 1 follows, unchanged.)

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws`
Expected: `test result: ok. 10 passed` (seven in `spot::ws::error::tests` and three in `spot::ws::tests`; `a_server_close_and_a_connect_timeout_keep_their_own_variants`, `a_stop_follows_its_cause`, `the_transport_follows_the_socket_table`, `every_variant_classifies`, `spot_has_one_path_and_it_is_stream`, `the_limits_are_the_measured_ones` among them).

Then the gates this task touches: `nix develop --command cargo fmt --all -- --check` prints nothing (if it does, run `nix develop --command cargo fmt --all`), `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` and `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'` print no warning, and `nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps` passes. The rules file's globs match the new file: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'` passes.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/ws/mod.rs polyoxide-binance/src/spot/ws/error.rs
git commit -m "feat(binance): the spot-ws feature, its module root and SpotWsError (binance spot ws, Story 3)" -- polyoxide-binance/Cargo.toml polyoxide-binance/src/lib.rs polyoxide-binance/src/spot/mod.rs polyoxide-binance/src/spot/ws/mod.rs polyoxide-binance/src/spot/ws/error.rs .claude/rules/wire-enums.md
```

### Task 3.2: Stream names

**Files:**
- Create: `polyoxide-binance/src/spot/ws/stream.rs`
- Modify: `polyoxide-binance/src/spot/ws/mod.rs` (the module list and the re-exports)
- Modify: `polyoxide-binance/src/lib.rs` (the spot `Classify` assertion)
- Test: `polyoxide-binance/src/spot/ws/stream.rs` (its `tests` module)

- [ ] **Step 1: Write the failing tests**

In `polyoxide-binance/src/spot/ws/mod.rs`, add the module after `pub mod error;`:

```rust
pub mod error;
pub mod stream;
```

and the re-export after `pub use error::{SpotWsError, StopCause};`:

```rust
pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};
```

Create `polyoxide-binance/src/spot/ws/stream.rs` holding only the tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn every_kind(symbol: &str) -> Vec<StreamName> {
        let s = Symbol::new(symbol).unwrap();
        let mut names = vec![
            StreamName::AllMiniTickers,
            StreamName::AggTrade(s.clone()),
            StreamName::Trade(s.clone()),
            StreamName::MiniTicker(s.clone()),
            StreamName::Ticker(s.clone()),
            StreamName::BookTicker(s.clone()),
            StreamName::AvgPrice(s.clone()),
        ];
        names.extend(
            Interval::ALL
                .iter()
                .map(|i| StreamName::Kline(s.clone(), *i)),
        );
        for levels in DepthLevels::ALL {
            for speed in DepthSpeed::ALL {
                names.push(StreamName::PartialDepth(s.clone(), *levels, *speed));
            }
        }
        names
    }

    #[test]
    fn every_name_round_trips_through_its_wire_spelling() {
        for symbol in [
            "BTCUSDT",
            "币安人生USDT",
            "1000SATSUSDT",
            "ÉTÉUSDT",
            "BTCUSDT_261225",
        ] {
            for name in every_kind(symbol) {
                let wire = name.to_string();
                assert_eq!(wire.parse::<StreamName>(), Ok(name.clone()), "{wire}");
            }
        }
    }

    #[test]
    fn names_are_spelled_as_the_wire_spells_them() {
        let btc = Symbol::new("BTCUSDT").unwrap();
        let chinese = Symbol::new("币安人生USDT").unwrap();
        let cases = [
            (StreamName::AggTrade(btc.clone()), "btcusdt@aggTrade"),
            (StreamName::Trade(btc.clone()), "btcusdt@trade"),
            (
                StreamName::Kline(btc.clone(), Interval::Month1),
                "btcusdt@kline_1M",
            ),
            (
                StreamName::Kline(btc.clone(), Interval::Minute1),
                "btcusdt@kline_1m",
            ),
            (
                StreamName::Kline(btc.clone(), Interval::Second1),
                "btcusdt@kline_1s",
            ),
            (StreamName::MiniTicker(chinese), "币安人生usdt@miniTicker"),
            (StreamName::AllMiniTickers, "!miniTicker@arr"),
            (StreamName::Ticker(btc.clone()), "btcusdt@ticker"),
            (StreamName::BookTicker(btc.clone()), "btcusdt@bookTicker"),
            (
                StreamName::PartialDepth(btc.clone(), DepthLevels::Twenty, DepthSpeed::Ms100),
                "btcusdt@depth20@100ms",
            ),
            (StreamName::AvgPrice(btc), "btcusdt@avgPrice"),
        ];
        for (name, wire) in cases {
            assert_eq!(name.to_string(), wire);
        }
    }

    #[test]
    fn every_depth_name_is_one_binance_delivers() {
        // Measured 2026-11-02 (OBSERVED.md, `### Frames`): each of these
        // delivered frames at its cadence, the bare name every 1000 ms and the
        // `@100ms` name every 100 ms.
        let btc = Symbol::new("BTCUSDT").unwrap();
        for (levels, speed, wire) in [
            (DepthLevels::Five, DepthSpeed::Ms100, "btcusdt@depth5@100ms"),
            (DepthLevels::Five, DepthSpeed::Ms1000, "btcusdt@depth5"),
            (DepthLevels::Ten, DepthSpeed::Ms100, "btcusdt@depth10@100ms"),
            (DepthLevels::Ten, DepthSpeed::Ms1000, "btcusdt@depth10"),
            (
                DepthLevels::Twenty,
                DepthSpeed::Ms100,
                "btcusdt@depth20@100ms",
            ),
            (DepthLevels::Twenty, DepthSpeed::Ms1000, "btcusdt@depth20"),
        ] {
            let name = StreamName::PartialDepth(btc.clone(), levels, speed);
            assert_eq!(name.to_string(), wire);
            assert_eq!(wire.parse::<StreamName>(), Ok(name));
        }
        assert_eq!(DepthLevels::ALL.len() * DepthSpeed::ALL.len(), 6);
    }

    #[test]
    fn every_stream_rides_the_one_path() {
        for name in every_kind("BTCUSDT") {
            assert_eq!(name.path(), StreamPath::Stream, "{name}");
        }
    }

    #[test]
    fn only_the_array_stream_has_no_symbol() {
        for name in every_kind("BTCUSDT") {
            match name {
                StreamName::AllMiniTickers => assert_eq!(name.symbol(), None),
                _ => assert_eq!(name.symbol().map(Symbol::as_str), Some("BTCUSDT"), "{name}"),
            }
        }
    }

    #[test]
    fn a_name_this_crate_does_not_build_does_not_parse() {
        for bad in [
            // Uppercase: acknowledged by Binance and delivers nothing.
            "BTCUSDT@aggTrade",
            "BTCUSDT@trade",
            "btcUSDT@trade",
            // A stream type that does not exist.
            "btcusdt@nonsense",
            // Depth: five levels, a speed spot does not serve, a level count it
            // does not have.
            "btcusdt@depth7@100ms",
            "btcusdt@depth5@250ms",
            "btcusdt@depth5@500ms",
            "btcusdt@depth50",
            "btcusdt@depth",
            // An interval spot does not have.
            "btcusdt@kline_2s",
            "btcusdt@kline_",
            // Streams of the other product and a retired one.
            "btcusdt@markPrice@1s",
            "!ticker@arr",
            "!miniTicker@arr@1s",
            "!bookTicker",
            // Not a stream at all.
            "btcusdt",
            "",
            "@trade",
            "btc usdt@ticker",
        ] {
            assert_eq!(
                bad.parse::<StreamName>(),
                Err(InvalidStreamName(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_name_that_does_not_parse_is_an_invalid_request() {
        use polyoxide_venue::{Class, Classify};

        let err = "btcusdt".parse::<StreamName>().unwrap_err();
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(err.is_fault());
        assert_eq!(err.retry_after(), None);
        assert!(!err.is_retriable());
    }
}
```

In `polyoxide-binance/src/lib.rs`, add the name to the assertion Task 3.1 extended:

```rust
    #[cfg(feature = "spot-ws")]
    is::<spot::ws::SpotWsError>();
```

becomes

```rust
    #[cfg(feature = "spot-ws")]
    is::<spot::ws::SpotWsError>();
    #[cfg(feature = "spot-ws")]
    is::<spot::ws::InvalidStreamName>();
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::stream`
Expected: the build fails with `error[E0432]: unresolved import `stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName}`` (the file holds no such items) and `cannot find type `Symbol``.

- [ ] **Step 3: Implement**

Put above the `tests` module in `polyoxide-binance/src/spot/ws/stream.rs`:

```rust
//! Stream names: the only way to name a spot market stream.
//!
//! Binance acknowledges every `SUBSCRIBE`, including a stream type that does
//! not exist, an uppercase symbol, a spelling it does not serve (`@250ms`) and
//! an unknown symbol, and none of those deliver anything. A name that will
//! never deliver is indistinguishable from a quiet one, so names are built
//! here, by construction, and never accepted as caller strings. The symbol is
//! the one part this cannot check: `exchange().exchange_info()` lists the
//! symbols that trade.

use std::{fmt, str::FromStr};

use crate::spot::{
    types::{Interval, Symbol},
    ws::StreamPath,
};

/// Levels per side of a partial depth stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthLevels {
    /// `depth5`.
    Five,
    /// `depth10`.
    Ten,
    /// `depth20`.
    Twenty,
}

impl DepthLevels {
    /// Every value, in order.
    pub const ALL: &'static [Self] = &[Self::Five, Self::Ten, Self::Twenty];

    fn as_str(self) -> &'static str {
        match self {
            Self::Five => "5",
            Self::Ten => "10",
            Self::Twenty => "20",
        }
    }
}

/// How often a partial depth stream pushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthSpeed {
    /// `@100ms`.
    Ms100,
    /// No suffix: every 1000 ms, Binance's default. An explicit `@250ms` or
    /// `@500ms` is not a spot spelling.
    Ms1000,
}

impl DepthSpeed {
    /// Every value, in order.
    pub const ALL: &'static [Self] = &[Self::Ms100, Self::Ms1000];

    /// What follows the levels in a stream name.
    fn suffix(self) -> &'static str {
        match self {
            Self::Ms100 => "@100ms",
            Self::Ms1000 => "",
        }
    }
}

/// A spot market stream.
///
/// `Display` renders the wire name, with the symbol's ASCII letters
/// lowercased (`btcusdt@aggTrade`, `币安人生usdt@miniTicker`), and `FromStr`
/// parses an echoed name back; [`Symbol::new`] uppercases ASCII, so the two
/// round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StreamName {
    /// `<s>@aggTrade`.
    AggTrade(Symbol),
    /// `<s>@trade`.
    Trade(Symbol),
    /// `<s>@kline_<interval>`.
    Kline(Symbol, Interval),
    /// `<s>@miniTicker`.
    MiniTicker(Symbol),
    /// `!miniTicker@arr`: the mini ticker of every symbol that changed.
    AllMiniTickers,
    /// `<s>@ticker`.
    Ticker(Symbol),
    /// `<s>@bookTicker`.
    BookTicker(Symbol),
    /// `<s>@depth<levels>`, or `<s>@depth<levels>@100ms`.
    PartialDepth(Symbol, DepthLevels, DepthSpeed),
    /// `<s>@avgPrice`.
    AvgPrice(Symbol),
}

impl StreamName {
    /// The path whose connection carries this stream: spot has one.
    pub fn path(&self) -> StreamPath {
        StreamPath::Stream
    }

    /// The symbol, for a single-symbol stream.
    pub fn symbol(&self) -> Option<&Symbol> {
        match self {
            Self::AllMiniTickers => None,
            Self::AggTrade(s)
            | Self::Trade(s)
            | Self::Kline(s, _)
            | Self::MiniTicker(s)
            | Self::Ticker(s)
            | Self::BookTicker(s)
            | Self::PartialDepth(s, ..)
            | Self::AvgPrice(s) => Some(s),
        }
    }
}

impl fmt::Display for StreamName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lower = |s: &Symbol| s.as_str().to_ascii_lowercase();
        match self {
            Self::AggTrade(s) => write!(f, "{}@aggTrade", lower(s)),
            Self::Trade(s) => write!(f, "{}@trade", lower(s)),
            Self::Kline(s, interval) => write!(f, "{}@kline_{interval}", lower(s)),
            Self::MiniTicker(s) => write!(f, "{}@miniTicker", lower(s)),
            Self::AllMiniTickers => f.write_str("!miniTicker@arr"),
            Self::Ticker(s) => write!(f, "{}@ticker", lower(s)),
            Self::BookTicker(s) => write!(f, "{}@bookTicker", lower(s)),
            Self::PartialDepth(s, levels, speed) => {
                write!(f, "{}@depth{}{}", lower(s), levels.as_str(), speed.suffix())
            }
            Self::AvgPrice(s) => write!(f, "{}@avgPrice", lower(s)),
        }
    }
}

/// A string that is not a stream name this crate builds.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a spot stream name this crate supports: StreamName builds every one, with its symbol in lowercase")]
pub struct InvalidStreamName(pub String);

/// An `InvalidRequest`: the caller named a stream this crate does not build.
impl polyoxide_venue::Classify for InvalidStreamName {
    fn class(&self) -> polyoxide_venue::Class {
        polyoxide_venue::Class::InvalidRequest
    }
}

impl FromStr for StreamName {
    type Err = InvalidStreamName;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidStreamName(name.to_owned());
        if name == "!miniTicker@arr" {
            return Ok(Self::AllMiniTickers);
        }
        let (symbol, kind) = name.split_once('@').ok_or_else(invalid)?;
        // A stream name spells its symbol in lowercase; one that does not was
        // not built here and, on the wire, delivers nothing.
        if symbol.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(invalid());
        }
        let symbol = Symbol::new(symbol).map_err(|_| invalid())?;
        match kind {
            "aggTrade" => Ok(Self::AggTrade(symbol)),
            "trade" => Ok(Self::Trade(symbol)),
            "miniTicker" => Ok(Self::MiniTicker(symbol)),
            "ticker" => Ok(Self::Ticker(symbol)),
            "bookTicker" => Ok(Self::BookTicker(symbol)),
            "avgPrice" => Ok(Self::AvgPrice(symbol)),
            _ => {
                if let Some(interval) = kind.strip_prefix("kline_") {
                    let interval = interval.parse().map_err(|_| invalid())?;
                    return Ok(Self::Kline(symbol, interval));
                }
                let depth = kind.strip_prefix("depth").ok_or_else(invalid)?;
                let (levels, speed) = DepthLevels::ALL
                    .iter()
                    .flat_map(|l| DepthSpeed::ALL.iter().map(move |s| (*l, *s)))
                    .find(|(l, s)| depth.strip_prefix(l.as_str()) == Some(s.suffix()))
                    .ok_or_else(invalid)?;
                Ok(Self::PartialDepth(symbol, levels, speed))
            }
        }
    }
}

```

(The `tests` module of Step 1 follows, unchanged.)

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::stream`
Expected: `test result: ok. 7 passed`.

Then `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` and `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'` print nothing.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/ws/stream.rs
git commit -m "feat(binance): spot's StreamName, DepthLevels, DepthSpeed and InvalidStreamName (binance spot ws, Story 3)" -- polyoxide-binance/src/lib.rs polyoxide-binance/src/spot/ws/mod.rs polyoxide-binance/src/spot/ws/stream.rs
```

### Task 3.3: The captured frames

**Files:**
- Create: `polyoxide-binance/src/spot/ws/fixtures.rs`
- Modify: `polyoxide-binance/src/spot/ws/mod.rs` (the module list)
- Test: `polyoxide-binance/src/spot/ws/stream.rs` (one test after the last)

This task needs Story 1's eleven files under `polyoxide-binance/tests/fixtures/spot/ws/`; if one is missing, stop: fixtures are captured, never written by hand.

- [ ] **Step 1: Write the failing test**

In `polyoxide-binance/src/spot/ws/mod.rs`, add the module between `error` and `stream`:

```rust
pub mod error;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;
pub mod stream;
```

Append this test to the `tests` module of `polyoxide-binance/src/spot/ws/stream.rs`, after `a_name_that_does_not_parse_is_an_invalid_request`:

```rust

    #[test]
    fn every_captured_envelope_names_a_stream_this_crate_builds_and_echoes_it() {
        use crate::spot::ws::fixtures;

        let btc = Symbol::new("BTCUSDT").unwrap();
        // Each fixture, with the name its capture subscribed to.
        let expected = [
            (fixtures::AGG_TRADE, "btcusdt@aggTrade"),
            (fixtures::TRADE, "btcusdt@trade"),
            (fixtures::KLINE, "btcusdt@kline_1m"),
            (fixtures::MINI_TICKER, "btcusdt@miniTicker"),
            (fixtures::ALL_MINI_TICKERS, "!miniTicker@arr"),
            (fixtures::TICKER, "btcusdt@ticker"),
            (fixtures::BOOK_TICKER, "btcusdt@bookTicker"),
            (fixtures::PARTIAL_DEPTH, "btcusdt@depth5"),
            (fixtures::PARTIAL_DEPTH_100MS, "btcusdt@depth20@100ms"),
            (fixtures::AVG_PRICE, "btcusdt@avgPrice"),
        ];
        for (frame, wire) in expected {
            let envelope: serde_json::Value = serde_json::from_str(frame).unwrap();
            assert_eq!(envelope["stream"], wire);
            let name: StreamName = wire.parse().unwrap();
            assert_eq!(name.to_string(), wire);
        }
        let parsed: StreamName = "btcusdt@depth20@100ms".parse().unwrap();
        assert_eq!(
            parsed,
            StreamName::PartialDepth(btc, DepthLevels::Twenty, DepthSpeed::Ms100)
        );

        // The non-ASCII symbol's name is echoed as sent, and parses back to the
        // name that renders it.
        let envelope: serde_json::Value =
            serde_json::from_str(fixtures::NON_ASCII_MINI_TICKER).unwrap();
        let echoed = envelope["stream"].as_str().unwrap();
        assert!(!echoed.is_ascii(), "{echoed}");
        let name: StreamName = echoed.parse().unwrap();
        assert!(matches!(&name, StreamName::MiniTicker(s) if !s.as_str().is_ascii()));
        assert_eq!(name.to_string(), echoed);

        // And every file in `ALL` is an envelope whose name parses.
        assert_eq!(fixtures::ALL.len(), 11);
        for (stem, frame) in fixtures::ALL {
            let envelope: serde_json::Value = serde_json::from_str(frame).unwrap();
            let echoed = envelope["stream"].as_str().unwrap();
            let name: StreamName = echoed.parse().unwrap_or_else(|e| panic!("{stem}: {e}"));
            assert_eq!(name.to_string(), echoed, "{stem}");
        }
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::stream`
Expected: the build fails with `error[E0583]: file not found for module `fixtures``.

- [ ] **Step 3: Implement**

Create `polyoxide-binance/src/spot/ws/fixtures.rs`:

```rust
//! The captured stream frames, one combined-stream envelope per stream kind,
//! for tests here and downstream. Provenance is in `tests/fixtures/spot/PROVENANCE.md`.

/// `btcusdt@aggTrade`.
pub const AGG_TRADE: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_aggTrade.json");
/// `btcusdt@trade`.
pub const TRADE: &str = include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_trade.json");
/// `btcusdt@kline_1m`.
pub const KLINE: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_kline_1m.json");
/// `btcusdt@miniTicker`.
pub const MINI_TICKER: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_miniTicker.json");
/// `!miniTicker@arr`, trimmed to two rows.
pub const ALL_MINI_TICKERS: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_all_miniTicker_arr.json");
/// `btcusdt@ticker`.
pub const TICKER: &str = include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_ticker.json");
/// `btcusdt@bookTicker`.
pub const BOOK_TICKER: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_bookTicker.json");
/// `btcusdt@depth5`, the bare (1000 ms) spelling.
pub const PARTIAL_DEPTH: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_depth5.json");
/// `btcusdt@depth20@100ms`, sides trimmed to three levels.
pub const PARTIAL_DEPTH_100MS: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_depth20_100ms.json");
/// `btcusdt@avgPrice`.
pub const AVG_PRICE: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_avgPrice.json");
/// `<s>@miniTicker` for a listed symbol with a non-ASCII character in it.
pub const NON_ASCII_MINI_TICKER: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_non_ascii_miniTicker.json");

/// Every frame, with its fixture's file stem.
pub const ALL: &[(&str, &str)] = &[
    ("stream_btcusdt_aggTrade", AGG_TRADE),
    ("stream_btcusdt_trade", TRADE),
    ("stream_btcusdt_kline_1m", KLINE),
    ("stream_btcusdt_miniTicker", MINI_TICKER),
    ("stream_all_miniTicker_arr", ALL_MINI_TICKERS),
    ("stream_btcusdt_ticker", TICKER),
    ("stream_btcusdt_bookTicker", BOOK_TICKER),
    ("stream_btcusdt_depth5", PARTIAL_DEPTH),
    ("stream_btcusdt_depth20_100ms", PARTIAL_DEPTH_100MS),
    ("stream_btcusdt_avgPrice", AVG_PRICE),
    ("stream_non_ascii_miniTicker", NON_ASCII_MINI_TICKER),
];
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::stream`
Expected: `test result: ok. 8 passed`.

Then `nix develop --command cargo fmt --all -- --check` (run `cargo fmt --all` if it prints a diff, since rustfmt may rewrap the `include_str!` lines) and `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` print nothing.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/ws/fixtures.rs
git commit -m "feat(binance): spot::ws::fixtures, the captured frames (binance spot ws, Story 3)" -- polyoxide-binance/src/spot/ws/fixtures.rs polyoxide-binance/src/spot/ws/mod.rs polyoxide-binance/src/spot/ws/stream.rs
```

### Task 3.4: Payloads and `Update::from_json`

**Files:**
- Create: `polyoxide-binance/src/spot/ws/event.rs`
- Modify: `polyoxide-binance/src/spot/ws/mod.rs` (the module list and the re-exports)
- Test: `polyoxide-binance/src/spot/ws/event.rs` (its `tests` module)

- [ ] **Step 1: Write the failing tests**

In `polyoxide-binance/src/spot/ws/mod.rs`, add the module after `pub mod error;`:

```rust
pub mod error;
pub mod event;
```

and the re-export after `pub use error::{SpotWsError, StopCause};`:

```rust
pub use event::{
    AggTradeEvent, AvgPriceEvent, BookTickerEvent, KlineBar, KlineEvent, MiniTickerEvent,
    PartialDepthEvent, Payload, TickerEvent, TradeEvent, Update,
};
```

Create `polyoxide-binance/src/spot/ws/event.rs` holding only the tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spot::ws::fixtures;
    use serde_json::json;

    fn wire(frame: &str) -> Value {
        serde_json::from_str(frame).unwrap()
    }

    fn data(frame: &str) -> Value {
        wire(frame)["data"].clone()
    }

    fn dec(row: &Value, key: &str) -> Decimal {
        row[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} is not a string"))
            .parse()
            .unwrap()
    }

    fn int(row: &Value, key: &str) -> u64 {
        row[key]
            .as_u64()
            .unwrap_or_else(|| panic!("{key} is not a u64"))
    }

    fn signed(row: &Value, key: &str) -> i64 {
        row[key]
            .as_i64()
            .unwrap_or_else(|| panic!("{key} is not an i64"))
    }

    fn text(row: &Value, key: &str) -> String {
        row[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} is not a string"))
            .to_owned()
    }

    fn payload(frame: &str) -> Payload {
        Update::from_json(frame).unwrap().payload
    }

    /// A captured frame with its envelope's stream name replaced.
    fn on_stream(frame: &str, stream: &str) -> String {
        let mut frame = wire(frame);
        frame["stream"] = json!(stream);
        frame.to_string()
    }

    #[test]
    fn every_captured_frame_decodes_as_its_stream() {
        for (name, frame) in fixtures::ALL {
            let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                !matches!(update.payload, Payload::Unknown { .. }),
                "{name} decoded as Unknown"
            );
        }
    }

    #[test]
    fn each_frame_lands_in_the_payload_its_stream_names() {
        assert!(matches!(payload(fixtures::AGG_TRADE), Payload::AggTrade(_)));
        assert!(matches!(payload(fixtures::TRADE), Payload::Trade(_)));
        assert!(matches!(payload(fixtures::KLINE), Payload::Kline(_)));
        assert!(matches!(
            payload(fixtures::MINI_TICKER),
            Payload::MiniTicker(_)
        ));
        assert!(matches!(
            payload(fixtures::ALL_MINI_TICKERS),
            Payload::MiniTickers(_)
        ));
        assert!(matches!(payload(fixtures::TICKER), Payload::Ticker(_)));
        assert!(matches!(
            payload(fixtures::BOOK_TICKER),
            Payload::BookTicker(_)
        ));
        assert!(matches!(
            payload(fixtures::PARTIAL_DEPTH),
            Payload::PartialDepth(_)
        ));
        assert!(matches!(
            payload(fixtures::PARTIAL_DEPTH_100MS),
            Payload::PartialDepth(_)
        ));
        assert!(matches!(
            payload(fixtures::AVG_PRICE),
            Payload::AvgPrice(_)
        ));
        assert!(matches!(
            payload(fixtures::NON_ASCII_MINI_TICKER),
            Payload::MiniTicker(_)
        ));
    }

    /// The round trip proves the key set, not which field a key lands in: two
    /// fields of one type swapped would pass it. Each field is compared here
    /// with its own key in the raw frame, as Binance's pages define the keys,
    /// so this holds for any capture. Where two keys carry equal values on
    /// every frame, a swap of those two stays invisible.
    #[test]
    fn each_field_holds_the_value_of_its_own_key() {
        fn mini(e: &MiniTickerEvent, row: &Value) {
            assert_eq!(
                [
                    e.close_price,
                    e.open_price,
                    e.high_price,
                    e.low_price,
                    e.volume,
                    e.quote_volume,
                ],
                ["c", "o", "h", "l", "v", "q"].map(|k| dec(row, k)),
                "{}",
                e.symbol
            );
            assert_eq!(e.event_time, int(row, "E"), "{}", e.symbol);
            assert_eq!(e.symbol, text(row, "s"));
            assert_eq!(e.event_type, text(row, "e"));
        }

        let Payload::MiniTickers(rows) = payload(fixtures::ALL_MINI_TICKERS) else {
            panic!("not MiniTickers");
        };
        assert_eq!(rows.len(), 2);
        for (e, row) in rows
            .iter()
            .zip(data(fixtures::ALL_MINI_TICKERS).as_array().unwrap())
        {
            mini(e, row);
        }
        for frame in [fixtures::MINI_TICKER, fixtures::NON_ASCII_MINI_TICKER] {
            let Payload::MiniTicker(e) = payload(frame) else {
                panic!("not MiniTicker");
            };
            mini(&e, &data(frame));
        }

        let row = data(fixtures::TICKER);
        let Payload::Ticker(e) = payload(fixtures::TICKER) else {
            panic!("not Ticker");
        };
        assert_eq!(
            [
                e.price_change,
                e.price_change_percent,
                e.weighted_avg_price,
                e.prev_close_price,
                e.last_price,
                e.last_quantity,
                e.bid_price,
                e.bid_quantity,
                e.ask_price,
                e.ask_quantity,
                e.open_price,
                e.high_price,
                e.low_price,
                e.volume,
                e.quote_volume,
            ],
            [
                "p", "P", "w", "x", "c", "Q", "b", "B", "a", "A", "o", "h", "l", "v", "q"
            ]
            .map(|k| dec(&row, k))
        );
        assert_eq!(
            [e.event_time, e.open_time, e.close_time, e.trade_count],
            ["E", "O", "C", "n"].map(|k| int(&row, k))
        );
        assert_eq!(
            [e.first_trade_id, e.last_trade_id],
            ["F", "L"].map(|k| signed(&row, k))
        );

        let row = data(fixtures::AGG_TRADE);
        let Payload::AggTrade(e) = payload(fixtures::AGG_TRADE) else {
            panic!("not AggTrade");
        };
        assert_eq!([e.price, e.quantity], ["p", "q"].map(|k| dec(&row, k)));
        assert_eq!(
            [
                e.event_time,
                e.id,
                e.first_trade_id,
                e.last_trade_id,
                e.trade_time
            ],
            ["E", "a", "f", "l", "T"].map(|k| int(&row, k))
        );
        assert_eq!(e.is_buyer_maker, row["m"].as_bool().unwrap());
        assert_eq!(e.symbol, text(&row, "s"));

        let row = data(fixtures::TRADE);
        let Payload::Trade(e) = payload(fixtures::TRADE) else {
            panic!("not Trade");
        };
        assert_eq!([e.price, e.quantity], ["p", "q"].map(|k| dec(&row, k)));
        assert_eq!(
            [e.event_time, e.id, e.trade_time],
            ["E", "t", "T"].map(|k| int(&row, k))
        );
        assert_eq!(e.is_buyer_maker, row["m"].as_bool().unwrap());

        let row = data(fixtures::KLINE)["k"].clone();
        let Payload::Kline(e) = payload(fixtures::KLINE) else {
            panic!("not Kline");
        };
        let k = &e.kline;
        assert_eq!(
            [
                k.open,
                k.close,
                k.high,
                k.low,
                k.volume,
                k.quote_volume,
                k.taker_buy_base_volume,
                k.taker_buy_quote_volume,
            ],
            ["o", "c", "h", "l", "v", "q", "V", "Q"].map(|key| dec(&row, key))
        );
        assert_eq!(
            [k.open_time, k.close_time, k.trade_count],
            ["t", "T", "n"].map(|key| int(&row, key))
        );
        assert_eq!(
            [k.first_trade_id, k.last_trade_id],
            ["f", "L"].map(|key| signed(&row, key))
        );
        assert_eq!(k.interval.as_str(), text(&row, "i"));
        assert_eq!(k.is_closed, row["x"].as_bool().unwrap());

        let row = data(fixtures::BOOK_TICKER);
        let Payload::BookTicker(e) = payload(fixtures::BOOK_TICKER) else {
            panic!("not BookTicker");
        };
        assert_eq!(
            [e.bid_price, e.bid_quantity, e.ask_price, e.ask_quantity],
            ["b", "B", "a", "A"].map(|k| dec(&row, k))
        );
        assert_eq!(e.update_id, int(&row, "u"));
        assert_eq!(e.symbol, text(&row, "s"));

        let row = data(fixtures::AVG_PRICE);
        let Payload::AvgPrice(e) = payload(fixtures::AVG_PRICE) else {
            panic!("not AvgPrice");
        };
        assert_eq!(e.price, dec(&row, "w"));
        assert_eq!(
            [e.event_time, e.last_trade_time],
            ["E", "T"].map(|k| int(&row, k))
        );
        assert_eq!(e.interval, text(&row, "i"));
    }

    #[test]
    fn a_partial_depth_frame_takes_its_symbol_from_the_envelope() {
        // The frame carries `lastUpdateId`, `bids` and `asks` and nothing else.
        for (frame, bids) in [
            (fixtures::PARTIAL_DEPTH, 5),
            (fixtures::PARTIAL_DEPTH_100MS, 3),
        ] {
            let row = data(frame);
            assert_eq!(row.as_object().unwrap().len(), 3);
            let update = Update::from_json(frame).unwrap();
            let Payload::PartialDepth(e) = update.payload else {
                panic!("not PartialDepth");
            };
            assert_eq!(e.symbol, "BTCUSDT");
            assert_eq!(e.last_update_id, int(&row, "lastUpdateId"));
            assert_eq!(e.bids.len(), bids);
            assert_eq!(e.asks.len(), bids);
            for (side, key) in [(&e.bids, "bids"), (&e.asks, "asks")] {
                let sent: Vec<(Decimal, Decimal)> = row[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|level| {
                        (
                            level[0].as_str().unwrap().parse().unwrap(),
                            level[1].as_str().unwrap().parse().unwrap(),
                        )
                    })
                    .collect();
                let decoded: Vec<(Decimal, Decimal)> =
                    side.iter().map(|l| (l.price, l.quantity)).collect();
                assert_eq!(decoded, sent, "{key}");
            }
        }

        // Another stream's name gives another symbol: it is the envelope's, not
        // the frame's.
        let ethereum = on_stream(fixtures::PARTIAL_DEPTH, "ethusdt@depth5");
        let Payload::PartialDepth(e) = Update::from_json(&ethereum).unwrap().payload else {
            panic!("not PartialDepth");
        };
        assert_eq!(e.symbol, "ETHUSDT");
        let chinese = on_stream(fixtures::PARTIAL_DEPTH, "币安人生usdt@depth5");
        let Payload::PartialDepth(e) = Update::from_json(&chinese).unwrap().payload else {
            panic!("not PartialDepth");
        };
        assert_eq!(e.symbol, "币安人生USDT");
    }

    #[test]
    fn a_book_ticker_frame_has_no_event_type_and_decodes() {
        assert!(data(fixtures::BOOK_TICKER).get("e").is_none());
        assert!(matches!(
            payload(fixtures::BOOK_TICKER),
            Payload::BookTicker(_)
        ));
    }

    #[test]
    fn the_non_ascii_symbol_keeps_its_characters_in_the_name_and_the_payload() {
        let update = Update::from_json(fixtures::NON_ASCII_MINI_TICKER).unwrap();
        let StreamName::MiniTicker(symbol) = &update.stream else {
            panic!("not MiniTicker: {:?}", update.stream);
        };
        assert!(!symbol.as_str().is_ascii());
        let Payload::MiniTicker(e) = &update.payload else {
            panic!("not MiniTicker");
        };
        assert_eq!(
            e.symbol.to_ascii_lowercase(),
            symbol.as_str().to_ascii_lowercase()
        );
        assert_eq!(
            wire(fixtures::NON_ASCII_MINI_TICKER)["stream"],
            update.stream.to_string()
        );
    }

    #[test]
    fn an_object_whose_event_type_is_not_its_streams_is_kept_whole() {
        // An aggregate trade's data on a trade stream: `e` is `aggTrade`, not
        // `trade`.
        let crossed = on_stream(fixtures::AGG_TRADE, "btcusdt@trade");
        let update = Update::from_json(&crossed).unwrap();
        assert_eq!(update.stream.to_string(), "btcusdt@trade");
        let Payload::Unknown { event_type, raw } = &update.payload else {
            panic!("not Unknown: {:?}", update.payload);
        };
        assert_eq!(event_type, "aggTrade");
        assert_eq!(serde_json::from_str::<Value>(raw).unwrap(), data(fixtures::AGG_TRADE));

        // A renamed event, and an object with none.
        for (e, expected) in [(json!("aggTradeV2"), "aggTradeV2"), (Value::Null, "")] {
            let mut frame = wire(fixtures::AGG_TRADE);
            match e {
                Value::Null => {
                    frame["data"].as_object_mut().unwrap().remove("e");
                }
                e => frame["data"]["e"] = e,
            }
            let update = Update::from_json(&frame.to_string()).unwrap();
            assert!(
                matches!(&update.payload, Payload::Unknown { event_type, .. } if event_type == expected),
                "{:?}",
                update.payload
            );
            // It serialises back as it came.
            assert_eq!(serde_json::to_value(&update).unwrap(), frame);
        }
    }

    #[test]
    fn a_broken_frame_is_a_frame_error_that_names_its_stream() {
        let mut broken = wire(fixtures::AGG_TRADE);
        broken["data"]["p"] = json!("x");
        let err = Update::from_json(&broken.to_string()).unwrap_err();
        assert!(
            matches!(&err, SpotWsError::Frame { stream, .. } if stream == "btcusdt@aggTrade"),
            "{err:?}"
        );

        // An array stream whose data is an object, and a book ticker with no
        // `u`.
        let mut object = wire(fixtures::ALL_MINI_TICKERS);
        object["data"] = json!({});
        assert!(matches!(
            Update::from_json(&object.to_string()),
            Err(SpotWsError::Frame { .. })
        ));
        let mut thin = wire(fixtures::BOOK_TICKER);
        thin["data"].as_object_mut().unwrap().remove("u");
        assert!(matches!(
            Update::from_json(&thin.to_string()),
            Err(SpotWsError::Frame { .. })
        ));
        let mut empty = wire(fixtures::PARTIAL_DEPTH);
        empty["data"] = json!({});
        assert!(matches!(
            Update::from_json(&empty.to_string()),
            Err(SpotWsError::Frame { .. })
        ));
    }

    #[test]
    fn a_stream_this_crate_does_not_build_is_a_frame_error() {
        for stream in [
            "btcusdt@forceOrder",
            "BTCUSDT@trade",
            "!ticker@arr",
            "btcusdt@depth5@250ms",
        ] {
            let err = Update::from_json(&on_stream(fixtures::TRADE, stream)).unwrap_err();
            assert!(
                matches!(&err, SpotWsError::Frame { stream: s, .. } if s == stream),
                "{stream}: {err:?}"
            );
        }
        assert!(matches!(
            Update::from_json("not json"),
            Err(SpotWsError::Frame { stream, .. }) if stream.is_empty()
        ));
        assert!(matches!(
            Update::from_json(r#"{"result":null,"id":1}"#),
            Err(SpotWsError::Frame { .. })
        ));
    }

    #[test]
    fn an_update_serialises_back_to_its_envelope() {
        // Frames whose every key the types model come back whole. The partial
        // depth's symbol is not serialised: it was never in the frame.
        for frame in [
            fixtures::BOOK_TICKER,
            fixtures::PARTIAL_DEPTH,
            fixtures::PARTIAL_DEPTH_100MS,
            fixtures::MINI_TICKER,
            fixtures::ALL_MINI_TICKERS,
            fixtures::TICKER,
            fixtures::AVG_PRICE,
        ] {
            let update = Update::from_json(frame).unwrap();
            assert_eq!(serde_json::to_value(&update).unwrap(), wire(frame));
        }
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::event`
Expected: the build fails with `error[E0432]: unresolved import `event::{AggTradeEvent, …}`` and `cannot find type `Update``, `cannot find type `Payload``, `cannot find type `Decimal``, `cannot find type `Value``.

- [ ] **Step 3: Implement**

Put above the `tests` module in `polyoxide-binance/src/spot/ws/event.rs`:

```rust
//! Stream payloads, and the update that carries one.
//!
//! The socket uses one-letter keys and different fields from REST, so each
//! stream has its own payload type, with long field names over the wire's keys.
//! Symbols are `String`, as sent. The keys Binance documents as "Ignore", `M`
//! on trades and aggregate trades and `B` on the kline, are not modelled.

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::spot::{
    types::{Interval, Level},
    ws::{error::SpotWsError, stream::StreamName},
};

/// `<s>@aggTrade`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AggTradeEvent {
    /// `aggTrade`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Aggregate trade id.
    #[serde(rename = "a")]
    pub id: u64,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// First trade id merged.
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    /// Last trade id merged.
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    /// Trade time.
    #[serde(rename = "T")]
    pub trade_time: u64,
    /// Whether the buyer was the maker, so the taker sold.
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `<s>@trade`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TradeEvent {
    /// `trade`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Trade id.
    #[serde(rename = "t")]
    pub id: u64,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Trade time.
    #[serde(rename = "T")]
    pub trade_time: u64,
    /// Whether the buyer was the maker, so the taker sold.
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `<s>@kline_<interval>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KlineEvent {
    /// `kline`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The candle so far.
    #[serde(rename = "k")]
    pub kline: KlineBar,
}

/// The candle in a [`KlineEvent`]. The wire's `B`, documented as "Ignore", is
/// dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KlineBar {
    /// Candle start.
    #[serde(rename = "t")]
    pub open_time: u64,
    /// Candle end, inclusive.
    #[serde(rename = "T")]
    pub close_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The candle's width.
    #[serde(rename = "i")]
    pub interval: Interval,
    /// First trade id; `-1` when the candle holds no trade.
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    /// Last trade id; `-1` when the candle holds no trade.
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    /// Open price.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open: Decimal,
    /// Close price so far.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub close: Decimal,
    /// High price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high: Decimal,
    /// Low price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Trades.
    #[serde(rename = "n")]
    pub trade_count: u64,
    /// Whether the candle is closed.
    #[serde(rename = "x")]
    pub is_closed: bool,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Base-asset volume bought by takers.
    #[serde(rename = "V", with = "rust_decimal::serde::str")]
    pub taker_buy_base_volume: Decimal,
    /// Quote-asset volume bought by takers.
    #[serde(rename = "Q", with = "rust_decimal::serde::str")]
    pub taker_buy_quote_volume: Decimal,
}

/// `<s>@miniTicker` and each row of `!miniTicker@arr`: the 24-hour window's
/// close, open, high, low and volumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MiniTickerEvent {
    /// `24hrMiniTicker`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Close price: the last price.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub close_price: Decimal,
    /// Price 24 hours ago.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
}

/// `<s>@ticker`: rolling 24-hour statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TickerEvent {
    /// `24hrTicker`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Last price less the open price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price_change: Decimal,
    /// The change as a percentage.
    #[serde(rename = "P", with = "rust_decimal::serde::str")]
    pub price_change_percent: Decimal,
    /// Volume-weighted average price.
    #[serde(rename = "w", with = "rust_decimal::serde::str")]
    pub weighted_avg_price: Decimal,
    /// The price before the window opened.
    #[serde(rename = "x", with = "rust_decimal::serde::str")]
    pub prev_close_price: Decimal,
    /// Last price.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Last quantity.
    #[serde(rename = "Q", with = "rust_decimal::serde::str")]
    pub last_quantity: Decimal,
    /// Best bid price.
    #[serde(rename = "b", with = "rust_decimal::serde::str")]
    pub bid_price: Decimal,
    /// Best bid quantity.
    #[serde(rename = "B", with = "rust_decimal::serde::str")]
    pub bid_quantity: Decimal,
    /// Best ask price.
    #[serde(rename = "a", with = "rust_decimal::serde::str")]
    pub ask_price: Decimal,
    /// Best ask quantity.
    #[serde(rename = "A", with = "rust_decimal::serde::str")]
    pub ask_quantity: Decimal,
    /// Price 24 hours ago.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Window start.
    #[serde(rename = "O")]
    pub open_time: u64,
    /// Window end.
    #[serde(rename = "C")]
    pub close_time: u64,
    /// First trade id in the window.
    #[serde(rename = "F")]
    pub first_trade_id: i64,
    /// Last trade id in the window.
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    /// Trades in the window.
    #[serde(rename = "n")]
    pub trade_count: u64,
}

/// `<s>@bookTicker`: the best bid and ask. The frame carries no event type
/// and no time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BookTickerEvent {
    /// Book update id.
    #[serde(rename = "u")]
    pub update_id: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Best bid price.
    #[serde(rename = "b", with = "rust_decimal::serde::str")]
    pub bid_price: Decimal,
    /// Best bid quantity.
    #[serde(rename = "B", with = "rust_decimal::serde::str")]
    pub bid_quantity: Decimal,
    /// Best ask price.
    #[serde(rename = "a", with = "rust_decimal::serde::str")]
    pub ask_price: Decimal,
    /// Best ask quantity.
    #[serde(rename = "A", with = "rust_decimal::serde::str")]
    pub ask_quantity: Decimal,
}

/// `<s>@depth<levels>` and `<s>@depth<levels>@100ms`: the top of the book.
///
/// The frame carries `lastUpdateId`, `bids` and `asks` and no symbol, so
/// `symbol` is the envelope's, uppercased as [`Symbol`](crate::spot::types::Symbol)
/// renders it, and it is not serialised back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PartialDepthEvent {
    /// The symbol, from the envelope's stream name.
    #[serde(skip)]
    pub symbol: String,
    /// Book update id of the snapshot.
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    /// Bids, best first.
    pub bids: Vec<Level>,
    /// Asks, best first.
    pub asks: Vec<Level>,
}

/// `<s>@avgPrice`: the average price over `interval`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvgPriceEvent {
    /// `avgPrice`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The window the average covers, such as `5m`.
    #[serde(rename = "i")]
    pub interval: String,
    /// The average price.
    #[serde(rename = "w", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Time of the last trade.
    #[serde(rename = "T")]
    pub last_trade_time: u64,
}

/// What a stream carries, by stream kind.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Payload {
    /// `<s>@aggTrade`.
    AggTrade(AggTradeEvent),
    /// `<s>@trade`.
    Trade(TradeEvent),
    /// `<s>@kline_<interval>`.
    Kline(KlineEvent),
    /// `<s>@miniTicker`.
    MiniTicker(MiniTickerEvent),
    /// `!miniTicker@arr`: only the symbols that changed. A row's `e` is not
    /// checked, and one row that does not decode fails the whole frame.
    MiniTickers(Vec<MiniTickerEvent>),
    /// `<s>@ticker`.
    Ticker(TickerEvent),
    /// `<s>@bookTicker`.
    BookTicker(BookTickerEvent),
    /// `<s>@depth<levels>` and `<s>@depth<levels>@100ms`.
    PartialDepth(PartialDepthEvent),
    /// `<s>@avgPrice`.
    AvgPrice(AvgPriceEvent),
    /// A single-symbol stream's object whose event type (`e`) is not the one
    /// its stream carries, kept whole rather than failing the frame. The live
    /// suite fails on one, since it means Binance renamed an event.
    Unknown {
        /// The payload's `e`, or empty when it has none.
        event_type: String,
        /// The payload, re-serialised: its values as sent, its keys sorted.
        raw: String,
    },
}

impl Serialize for Payload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::AggTrade(event) => event.serialize(serializer),
            Self::Trade(event) => event.serialize(serializer),
            Self::Kline(event) => event.serialize(serializer),
            Self::MiniTicker(event) => event.serialize(serializer),
            Self::MiniTickers(rows) => rows.serialize(serializer),
            Self::Ticker(event) => event.serialize(serializer),
            Self::BookTicker(event) => event.serialize(serializer),
            Self::PartialDepth(event) => event.serialize(serializer),
            Self::AvgPrice(event) => event.serialize(serializer),
            Self::Unknown { raw, .. } => serde_json::from_str::<Value>(raw)
                .map_err(serde::ser::Error::custom)?
                .serialize(serializer),
        }
    }
}

/// One frame of a combined stream: `{"stream": <name>, "data": <payload>}`.
///
/// It serialises back to that envelope, so `serde_json::to_string(&update)`
/// is the frame as the wire sent it, less the fields this crate drops.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Update {
    /// The stream the frame arrived on.
    pub stream: StreamName,
    /// What it carries.
    pub payload: Payload,
}

impl Serialize for Update {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("stream", &self.stream.to_string())?;
        map.serialize_entry("data", &self.payload)?;
        map.end()
    }
}

#[derive(Deserialize)]
struct Envelope {
    stream: String,
    data: Value,
}

impl Update {
    /// Decodes one combined-stream frame.
    ///
    /// Fails with [`SpotWsError::Frame`] when the text is not an envelope, its
    /// stream is not one this crate builds, or its payload does not decode as
    /// the stream's type.
    pub fn from_json(text: &str) -> Result<Self, SpotWsError> {
        let envelope: Envelope =
            serde_json::from_str(text).map_err(|err| frame_error("", text, err))?;
        let stream: StreamName = envelope
            .stream
            .parse()
            .map_err(|err| frame_error(&envelope.stream, text, err))?;
        let payload = decode(&stream, envelope.data)
            .map_err(|err| frame_error(&envelope.stream, text, err))?;
        Ok(Self { stream, payload })
    }
}

fn frame_error(stream: &str, raw: &str, reason: impl fmt::Display) -> SpotWsError {
    SpotWsError::Frame {
        stream: stream.to_owned(),
        raw: raw.to_owned(),
        reason: reason.to_string(),
    }
}

fn decode(stream: &StreamName, data: Value) -> Result<Payload, serde_json::Error> {
    // The streams whose frames carry no event type, and the array stream, decode
    // as they are. A partial-depth frame carries no symbol either: the
    // envelope's stream name supplies it.
    let expected = match stream {
        StreamName::AllMiniTickers => {
            return serde_json::from_value(data).map(Payload::MiniTickers)
        }
        StreamName::BookTicker(_) => return serde_json::from_value(data).map(Payload::BookTicker),
        StreamName::PartialDepth(symbol, ..) => {
            let mut event: PartialDepthEvent = serde_json::from_value(data)?;
            event.symbol = symbol.to_string();
            return Ok(Payload::PartialDepth(event));
        }
        StreamName::AggTrade(_) => "aggTrade",
        StreamName::Trade(_) => "trade",
        StreamName::Kline(..) => "kline",
        StreamName::MiniTicker(_) => "24hrMiniTicker",
        StreamName::Ticker(_) => "24hrTicker",
        StreamName::AvgPrice(_) => "avgPrice",
    };
    let event_type = data.get("e").and_then(Value::as_str).unwrap_or_default();
    if event_type != expected {
        return Ok(Payload::Unknown {
            event_type: event_type.to_owned(),
            raw: data.to_string(),
        });
    }
    Ok(match stream {
        StreamName::AggTrade(_) => Payload::AggTrade(serde_json::from_value(data)?),
        StreamName::Trade(_) => Payload::Trade(serde_json::from_value(data)?),
        StreamName::Kline(..) => Payload::Kline(serde_json::from_value(data)?),
        StreamName::MiniTicker(_) => Payload::MiniTicker(serde_json::from_value(data)?),
        StreamName::Ticker(_) => Payload::Ticker(serde_json::from_value(data)?),
        StreamName::AvgPrice(_) => Payload::AvgPrice(serde_json::from_value(data)?),
        StreamName::AllMiniTickers | StreamName::BookTicker(_) | StreamName::PartialDepth(..) => {
            unreachable!("returned above")
        }
    })
}

```

(The `tests` module of Step 1 follows, unchanged.)

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::event`
Expected: `test result: ok. 10 passed`. If `every_captured_frame_decodes_as_its_stream` names a fixture as `Unknown`, the event type this code expects for that stream (`24hrMiniTicker`, `24hrTicker`, `avgPrice`, `trade`, `aggTrade`, `kline`) is not what the capture carries: print `wire(frame)["data"]["e"]`, and change the expected string in `decode` to the captured one, never the fixture.

Then `nix develop --command cargo fmt --all -- --check` (run `cargo fmt --all` if it prints a diff), `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` and `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'` print nothing.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/ws/event.rs
git commit -m "feat(binance): spot's stream payloads and Update::from_json (binance spot ws, Story 3)" -- polyoxide-binance/src/spot/ws/event.rs polyoxide-binance/src/spot/ws/mod.rs
```

### Task 3.5: The wire agreement suite

**Files:**
- Create: `polyoxide-binance/tests/wire_agreement_spot_ws.rs`
- Modify: `polyoxide-binance/Cargo.toml` (one `[[test]]` stanza)
- Test: `polyoxide-binance/tests/wire_agreement_spot_ws.rs`

- [ ] **Step 1: Write the failing test**

In `polyoxide-binance/Cargo.toml`, the stanzas currently end

```toml
[[test]]
name = "wire_agreement_spot"
required-features = ["spot"]

[[test]]
name = "wire_agreement_usdm"
required-features = ["usdm"]
```

Insert between them:

```toml
[[test]]
name = "wire_agreement_spot_ws"
required-features = ["spot-ws", "test-server"]
```

Create `polyoxide-binance/tests/wire_agreement_spot_ws.rs`:

```rust
//! Agreement between the spot stream payload types and the captured frames in
//! `tests/fixtures/spot/ws/`, by the rules of `wire_agreement_usdm_ws.rs`:
//! nothing unmodelled, nothing invented, nothing altered.

use polyoxide_test_support::agreement as common;

use polyoxide_binance::spot::ws::{fixtures, Payload, StreamName, Update};
use polyoxide_test_support::agreement::Ledger;
use serde_json::Value;

/// `(fixture, path, reason)` the types deliberately drop.
pub const IGNORED: &[(&str, &str, &str)] = &[
    (
        "stream_btcusdt_trade",
        "/data/M",
        "Binance documents the trade's `M` as \"Ignore\"",
    ),
    (
        "stream_btcusdt_aggTrade",
        "/data/M",
        "Binance documents the aggregate trade's `M` as \"Ignore\"",
    ),
    (
        "stream_btcusdt_kline_1m",
        "/data/k/B",
        "Binance documents the kline's `B` as \"Ignore\"",
    ),
];

#[test]
fn every_stream_fixture_agrees_with_its_type() {
    let mut ledger = Ledger::new(&[IGNORED]);
    let mut used = Vec::new();
    for (fixture, frame) in fixtures::ALL {
        let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        assert!(
            !matches!(update.payload, Payload::Unknown { .. }),
            "{fixture}: decoded as Unknown"
        );
        let wire: Value = serde_json::from_str(frame).unwrap();
        let emitted = serde_json::to_value(&update).unwrap();
        let diff = common::compare_values(fixture, &wire, &emitted);
        for path in diff.unmodelled {
            match ledger.excuse(IGNORED, fixture, &path) {
                Some(entry) => used.push(entry),
                None => panic!("{fixture}: the server sent {path}, which the type does not model"),
            }
        }
        assert!(
            diff.invented.is_empty(),
            "{fixture}: the type emits {:?}, which the server did not send",
            diff.invented
        );
    }
    assert_eq!(
        used.len(),
        IGNORED.len(),
        "an IGNORED entry no fixture needs"
    );
}

#[test]
fn the_fixture_set_covers_every_stream_kind() {
    // A set that lost a kind would leave its payload type untested.
    let mut kinds = Vec::new();
    for (fixture, frame) in fixtures::ALL {
        let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        let kind = match (&update.stream, &update.payload) {
            (StreamName::AggTrade(_), Payload::AggTrade(_)) => "aggTrade",
            (StreamName::Trade(_), Payload::Trade(_)) => "trade",
            (StreamName::Kline(..), Payload::Kline(_)) => "kline",
            (StreamName::MiniTicker(_), Payload::MiniTicker(_)) => "miniTicker",
            (StreamName::AllMiniTickers, Payload::MiniTickers(_)) => "miniTicker@arr",
            (StreamName::Ticker(_), Payload::Ticker(_)) => "ticker",
            (StreamName::BookTicker(_), Payload::BookTicker(_)) => "bookTicker",
            (StreamName::PartialDepth(..), Payload::PartialDepth(_)) => "depth",
            (StreamName::AvgPrice(_), Payload::AvgPrice(_)) => "avgPrice",
            other => panic!("{fixture}: {other:?}"),
        };
        kinds.push(kind);
    }
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(
        kinds,
        [
            "aggTrade",
            "avgPrice",
            "bookTicker",
            "depth",
            "kline",
            "miniTicker",
            "miniTicker@arr",
            "ticker",
            "trade",
        ]
    );
}

#[test]
fn the_array_fixture_keeps_two_rows_and_the_depth_fixtures_their_levels() {
    let Payload::MiniTickers(rows) = Update::from_json(fixtures::ALL_MINI_TICKERS)
        .unwrap()
        .payload
    else {
        panic!("not MiniTickers");
    };
    assert_eq!(rows.len(), 2);

    // The bare name's frame is untrimmed (five levels a side) and the 100 ms
    // frame is trimmed to three.
    for (frame, levels) in [
        (fixtures::PARTIAL_DEPTH, 5),
        (fixtures::PARTIAL_DEPTH_100MS, 3),
    ] {
        let Payload::PartialDepth(e) = Update::from_json(frame).unwrap().payload else {
            panic!("not PartialDepth");
        };
        assert_eq!((e.bids.len(), e.asks.len()), (levels, levels));
    }
}

#[test]
fn the_non_ascii_fixture_is_a_non_ascii_symbol() {
    // The capture picks the first trading symbol with a non-ASCII character;
    // a fixture that lost it would leave the lowercasing rule untested.
    let update = Update::from_json(fixtures::NON_ASCII_MINI_TICKER).unwrap();
    let StreamName::MiniTicker(symbol) = update.stream else {
        panic!("not MiniTicker");
    };
    assert!(!symbol.as_str().is_ascii(), "{symbol}");
}
```

- [ ] **Step 2: Run it to see it fail**

The suite has no production code to wait for (Tasks 3.1 to 3.4 wrote the types), so prove it can fail: temporarily delete the `"stream_btcusdt_kline_1m"` entry from `IGNORED` and run `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --test wire_agreement_spot_ws`.
Expected: `every_stream_fixture_agrees_with_its_type` fails with `stream_btcusdt_kline_1m: the server sent /data/k/B, which the type does not model`.

- [ ] **Step 3: Implement**

Restore the deleted `IGNORED` entry. The suite and the stanza of Step 1 are the whole task: there is no production code to add.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --test wire_agreement_spot_ws`
Expected: `test result: ok. 4 passed`. If `every_stream_fixture_agrees_with_its_type` names an unmodelled key other than the three in `IGNORED`, the type is missing a field the capture carries: add it to the type (and its contract entry is then wrong; stop and report), never to `IGNORED`.

Then `nix develop --command cargo fmt --all -- --check` and `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` print nothing.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/tests/wire_agreement_spot_ws.rs
git commit -m "test(binance): wire agreement of spot's stream fixtures with their types (binance spot ws, Story 3)" -- polyoxide-binance/Cargo.toml polyoxide-binance/tests/wire_agreement_spot_ws.rs
```

### Task 3.6: The `spot-ws` dependency allowlist

**Files:**
- Modify: `.github/scripts/tests/test_dep_allowlists.py` (the binance rule, `MUST_BUILD`, the existence test, `SOCKETS` and two prose passages)
- Modify: `.github/scripts/tests/test_classify_coverage.py:236` and `:276` (the two non-vacuity lists)
- Modify: `.claude/rules/ci-gates.md:80-83` (the `polyoxide-binance` bullet)
- Create: `ci/dep-allowlists/polyoxide-binance@spot-ws.txt` (written by `--write`)
- Test: `.github/scripts/tests/test_dep_allowlists.py`, `.github/scripts/tests/test_classify_coverage.py`

- [ ] **Step 1: Write the failing test**

In `.github/scripts/tests/test_dep_allowlists.py`, the Binance rule currently reads:

```python
    "polyoxide-binance": (
        Rule(frozenset({"polyoxide-ws", "tokio-tungstenite", "tungstenite"}), (),
             frozenset({"usdm-ws"}),
             "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws"),
    ),
```

Change it to:

```python
    "polyoxide-binance": (
        Rule(frozenset({"polyoxide-ws", "tokio-tungstenite", "tungstenite"}), (),
             frozenset({"usdm-ws", "spot-ws"}),
             "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws or spot-ws"),
    ),
```

`MUST_BUILD` currently has the line `    "polyoxide-binance@usdm-ws.txt": "polyoxide-ws",`; add below it:

```python
    "polyoxide-binance@spot-ws.txt": "polyoxide-ws",
```

The existence test currently has:

```python
    assert {f"polyoxide-binance@{m}.txt" for m in ("usdm", "usdm-ws")} <= set(names), names
```

Change it to:

```python
    assert {f"polyoxide-binance@{m}.txt"
            for m in ("usdm", "usdm-ws", "spot", "spot-ws")} <= set(names), names
```

In the module docstring, change

```
`polyoxide-binance` builds `polyoxide-ws`, `tokio-tungstenite` and
`tungstenite`, the socket stack, only with `usdm-ws`, so its `@usdm` file is the
REST client alone.
```

to

```
`polyoxide-binance` builds `polyoxide-ws`, `tokio-tungstenite` and
`tungstenite`, the socket stack, only with `usdm-ws` or `spot-ws`, so its
`@usdm` and `@spot` files are the REST clients alone.
```

and `Binance's `@usdm-ws` file `polyoxide-ws`` to `Binance's `@usdm-ws` and `@spot-ws` files `polyoxide-ws``. (Read the surrounding lines first and keep the paragraph's wrapping.)

The `SOCKETS` message that `test_binances_rest_module_may_not_build_the_socket_stack` compares against the rule's message currently reads:

```python
SOCKETS = "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws"
```

Change it to the new rule message:

```python
SOCKETS = "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws or spot-ws"
```

Leave `BINANCE_TABLE` alone: that test still holds against it, since the rule refuses the stack to `usdm`, `test-server` and no feature, and admits it to `usdm-ws`.

In `.github/scripts/tests/test_classify_coverage.py`, `test_the_sweep_finds_the_known_types` has the row

```python
    ("polyoxide-binance", {"UsdmError", "InvalidSymbol", "UsdmWsError", "InvalidStreamName"}),
```

Change it to (`InvalidStreamName` is already in the set, by usdm's type of the same name, which spot's `stream::InvalidStreamName` shares; a set holds it once):

```python
    ("polyoxide-binance", {"UsdmError", "InvalidSymbol", "UsdmWsError", "InvalidStreamName",
                           "SpotWsError"}),
```

and `test_the_ws_sweep_finds_every_socket_error` has the rows

```python
    ("polyoxide-binance", "UsdmWsError"),
    ("polyoxide-polymarket", "RtdsError"),
    ("polyoxide-polymarket", "SportsError"),
])
```

Change them to:

```python
    ("polyoxide-binance", "UsdmWsError"),
    ("polyoxide-polymarket", "RtdsError"),
    ("polyoxide-polymarket", "SportsError"),
    ("polyoxide-binance", "SpotWsError"),
])
```

These two additions pass at once, since Task 3.1 added `SpotWsError` and its `lib.rs` assertion; they pin that the sweeps keep finding it.

In `.claude/rules/ci-gates.md` (AD-21: this commit changes the rule, so it edits the rule's text), the `polyoxide-binance` bullet currently reads:

```
  - `polyoxide-binance`: `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with
    `usdm-ws` (files `@usdm`, `@usdm-ws` and `@spot`; `@usdm-ws` must hold `polyoxide-ws`, and
    `@spot` must hold `polyoxide-core` and `reqwest`). `rustls` is not on the
    rule, since core's HTTP client builds it for TLS.
```

Change it to:

```
  - `polyoxide-binance`: `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with
    `usdm-ws` or `spot-ws` (files `@usdm`, `@usdm-ws`, `@spot` and `@spot-ws`; `@usdm-ws` and
    `@spot-ws` must each hold `polyoxide-ws`, and `@spot` must hold `polyoxide-core` and
    `reqwest`). `rustls` is not on the
    rule, since core's HTTP client builds it for TLS.
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q'`
Expected: failures: `test_the_allowlists_exist` (`polyoxide-binance@spot-ws.txt` is not in the names) and `test_each_per_feature_rule_admits_what_it_guards[polyoxide-binance@spot-ws.txt]` (the file does not exist).

- [ ] **Step 3: Implement**

`--write` rewrites only files that already exist, so create the file first with one comment line:

```bash
printf '# placeholder: regenerated by test_dep_allowlists --write\n' > ci/dep-allowlists/polyoxide-binance@spot-ws.txt
nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'
```

Then check what the run changed: `git status --short ci/dep-allowlists` lists only `polyoxide-binance@spot-ws.txt` (untracked); every other allowlist is unchanged. If another file changed, the tree moved for another reason: stop and report. The new file's header names `cargo tree -p polyoxide-binance -e normal --no-default-features --target x86_64-unknown-linux-gnu --features spot-ws`, and it lists `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` among the crates `polyoxide-binance@spot.txt` does not.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_claude_rules.py -q'`
Expected: all pass; `test_classify_coverage.py` finds `SpotWsError` and `InvalidStreamName` in `lib.rs`'s assertion and `error.rs` expanding the socket table and calling `ws_class`, `ws_is_fault` and `ws_retry_after`.

Then the story's whole Verify line from the repo root, in order: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server`, `nix develop --command cargo test -p polyoxide-binance --all-features`, `nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps`, and the pytest command. Also the profile's gates for `polyoxide-binance` and its dependents (`polyoxide`, `polyoxide-cli`): `cargo fmt --all -- --check`, `cargo clippy -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-targets --all-features -- -D warnings`, `cargo test -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features`, the `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` build over the same packages, `python3 scripts/gen_registry.py --check`, `python3 scripts/venue_gate.py`, `python3 scripts/live_unwraps.py` and `python3 scripts/test_body_diff.py --baseline v0.40.0`, each under `nix develop --command`.

- [ ] **Step 5: Commit**

```bash
git add -N ci/dep-allowlists/polyoxide-binance@spot-ws.txt
git commit -m "test(ci): pin polyoxide-binance's spot-ws dependency tree

The new spot-ws feature builds the socket kit (polyoxide-ws, tokio-tungstenite,
tungstenite) as usdm-ws does, so the allowlist gains its file and the binance
rule admits the socket stack with either feature. (binance spot ws, Story 3)" -- .github/scripts/tests/test_dep_allowlists.py .github/scripts/tests/test_classify_coverage.py .claude/rules/ci-gates.md ci/dep-allowlists/polyoxide-binance@spot-ws.txt
```

## Story 4: Spot socket clients, registration and the live suite

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/ws/mod.rs`, `polyoxide-binance/src/shared/ws/bare.rs`, `Cargo.toml`, `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/src/spot/ws/protocol.rs`, `polyoxide-binance/src/spot/ws/client.rs`, `polyoxide-binance/src/spot/ws/supervised.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/supervision_spot.rs`, `polyoxide-binance/tests/live_spot_ws.rs`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `scripts/live_unwraps.baseline.json`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`, `Cargo.lock`
- **Interfaces:** exposes "Story 4 → Stories 5 and 6"; uses Story 2's `shared::ws` items and Story 3's `spot::ws` types and fixtures
- **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && cargo test -p polyoxide-binance --no-default-features --features spot-ws --test live_spot_ws -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py`
- **Review:** deep (a supervised socket that must never pass spot's message limit on a shared address)

This story puts spot's sockets on the generic code Story 2 left in `shared::ws`. It widens that module's `cfg` to `spot-ws`, adds `SpotStreams` (the `StreamSet` marker) and `impl SocketError for SpotWsError` in `spot/ws/protocol.rs`, and writes the public types over `Bare<SpotStreams>`, `Running<SpotStreams>` and `Handle<SpotStreams>`: `SpotWs` in `client.rs`, and `Event`, `DisconnectReason`, `SpotWsBuilder`, `SupervisedSpotWs` and `MembershipHandle` in `supervised.rs`. Spot has one path, `/stream`, takes up to 1024 names in a request (the connection's cap, so a request is never split) and is paced at one request per 500 ms, because a request every 250 ms was closed with 1008 "Too many requests" where one every 500 ms held for 45 s (`OBSERVED.md`, `### Pace at 250 ms` and `### Pace at 500 ms`). Every pace and size assertion compares with the literal measured number, never with the constant under test. Unit tests in `client.rs`, `supervised.rs` and `protocol.rs` and the `supervision_spot` suite run on the scripted server (`src/usdm/ws/scripted.rs` by `#[path]`, not moved or copied). `live_spot_ws` runs against production at the client's pace and holds `live_frames_carry_no_unmodelled_keys`. The last two tasks register the live entry and the `stream.binance.com` identifier, regenerate the registry's files, and add row (m), the socket pace, to `docs/MUTANTS.md`, proved by applying the mutation. No code here holds a landmine marker, so no task needs `effort: xhigh`.

`impl SocketError for SpotWsError` lives in `protocol.rs`, so the error file keeps Story 3's text and its `Classify` coverage. `.github/scripts/tests/test_mutants_ledger.py` and `scripts/live_unwraps.baseline.json` are edited in Tasks 4.4 and 4.3, because their gates fail otherwise.

### Task 4.1: The stream set, the events and the bare tier

**Files:**
- Modify: `polyoxide-binance/src/shared/mod.rs`
- Modify: `polyoxide-binance/src/spot/ws/mod.rs`
- Create: `polyoxide-binance/src/spot/ws/protocol.rs`
- Create: `polyoxide-binance/src/spot/ws/client.rs`
- Create: `polyoxide-binance/src/spot/ws/supervised.rs`
- Test: the `tests` modules of the three new files

- [ ] **Step 1: Write the failing tests**

Wire the modules into `polyoxide-binance/src/spot/ws/mod.rs`. Story 3 left these lines in it; three edits.

Replace

```rust
pub mod error;
```

with

```rust
pub mod client;
pub mod error;
```

Replace

```rust
pub mod stream;
```

with

```rust
mod protocol;
#[cfg(test)]
#[path = "../../usdm/ws/scripted.rs"]
mod scripted;
pub mod stream;
pub mod supervised;
```

(The scripted server's file names nothing of either product. `usdm::ws` includes it with `mod scripted;` under `cfg(test)` and this module includes the same file by path, so there is one copy.)

Replace

```rust
pub use error::{SpotWsError, StopCause};
```

with

```rust
pub use client::SpotWs;
pub use error::{SpotWsError, StopCause};
```

and put this line after the file's `pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};` line:

```rust
pub use supervised::{DisconnectReason, Event};
```

(Task 4.2 widens that re-export to the builder, stream and handle.)

Create `polyoxide-binance/src/spot/ws/protocol.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use polyoxide_venue::Classify;
    use polyoxide_ws::{tungstenite, Outage, Protocol, WsError};

    use super::*;
    use crate::spot::{types::Symbol, ws::fixtures};

    fn btc() -> Symbol {
        Symbol::new("BTCUSDT").unwrap()
    }

    #[test]
    fn the_stream_set_is_one_path_with_the_measured_limits() {
        assert_eq!(<SpotStreams as StreamSet>::PATHS, [StreamPath::Stream]);
        assert_eq!(
            <SpotStreams as StreamSet>::MAX_NAMES_PER_REQUEST,
            MAX_NAMES_PER_REQUEST
        );
        assert_eq!(
            <SpotStreams as StreamSet>::MIN_REQUEST_INTERVAL,
            MIN_REQUEST_INTERVAL
        );
    }

    #[test]
    fn every_name_is_on_the_one_path() {
        for name in [
            StreamName::AggTrade(btc()),
            StreamName::AllMiniTickers,
            StreamName::BookTicker(btc()),
        ] {
            assert_eq!(SpotStreams::path(&name), StreamPath::Stream, "{name}");
        }
    }

    #[test]
    fn the_url_appends_the_one_path_to_the_base() {
        assert_eq!(
            SpotStreams::url("ws://127.0.0.1:9", StreamPath::Stream),
            "ws://127.0.0.1:9/stream"
        );
        assert_eq!(
            SpotStreams::url("ws://127.0.0.1:9/", StreamPath::Stream),
            "ws://127.0.0.1:9/stream"
        );
    }

    #[test]
    fn the_protocol_dials_the_stream_url() {
        let mut protocol = SpotProtocol::new("ws://127.0.0.1:9".to_owned(), StreamPath::Stream);
        let handshake = protocol.handshake().unwrap();
        assert_eq!(handshake.uri().to_string(), "ws://127.0.0.1:9/stream");
    }

    #[test]
    fn a_frame_decodes_as_an_update_and_a_bad_one_is_a_frame_error() {
        let update = SpotStreams::decode(fixtures::AGG_TRADE).unwrap();
        assert_eq!(update.stream.to_string(), "btcusdt@aggTrade");
        let err = SpotStreams::decode("not json").unwrap_err();
        assert!(matches!(err, SpotWsError::Frame { .. }), "{err:?}");
    }

    #[test]
    fn an_update_is_boxed_into_the_event() {
        let update = SpotStreams::decode(fixtures::AGG_TRADE).unwrap();
        let Event::Update(boxed) = SpotStreams::update(update) else {
            panic!("expected Event::Update");
        };
        assert_eq!(boxed.stream.to_string(), "btcusdt@aggTrade");
    }

    #[test]
    fn each_outage_reads_as_its_disconnect_reason() {
        let reason_of = |outage: Outage<SpotWsError>| {
            let Event::Disconnected { path, reason } =
                SpotStreams::disconnected(StreamPath::Stream, outage)
            else {
                panic!("expected Event::Disconnected");
            };
            assert_eq!(path, StreamPath::Stream);
            reason
        };
        assert!(matches!(
            reason_of(Outage::Closed {
                code: Some(1008),
                reason: "Too many requests".to_owned()
            }),
            DisconnectReason::Closed { code: Some(1008), reason } if reason == "Too many requests"
        ));
        assert!(matches!(
            reason_of(Outage::Error(SpotWsError::Refused {
                code: 2,
                msg: "Invalid request".to_owned()
            })),
            DisconnectReason::Error(SpotWsError::Refused { code: 2, .. })
        ));
        assert!(matches!(
            reason_of(Outage::Stale {
                silent: Duration::from_secs(30)
            }),
            DisconnectReason::Stale
        ));
        assert!(matches!(
            reason_of(Outage::Rotation),
            DisconnectReason::Rotation
        ));
    }

    #[test]
    fn a_return_names_its_path() {
        assert!(matches!(
            SpotStreams::reconnected(StreamPath::Stream),
            Event::Reconnected {
                path: StreamPath::Stream
            }
        ));
    }

    #[test]
    fn the_error_constructors_build_their_variants_and_the_shape_reads_them_back() {
        let connect = SpotWsError::from(WsError::from(tungstenite::Error::AlreadyClosed));
        assert!(matches!(connect.shape(), Shape::Connect(_)), "{connect:?}");
        assert!(matches!(
            SpotWsError::ConnectTimeout(Duration::from_secs(10)).shape(),
            Shape::ConnectTimeout
        ));
        assert!(matches!(
            SpotWsError::Closed {
                code: Some(1008),
                reason: String::new()
            }
            .shape(),
            Shape::Closed
        ));
        let refused = SpotWsError::refused(2, "Invalid request".to_owned());
        assert!(matches!(refused, SpotWsError::Refused { code: 2, .. }));
        assert!(matches!(refused.shape(), Shape::Refused));
        let no_answer = SpotWsError::no_answer(7, Duration::from_secs(10));
        assert!(matches!(no_answer, SpotWsError::NoAnswer { id: 7, .. }));
        assert!(matches!(no_answer.shape(), Shape::NoAnswer));
        let response = SpotWsError::response(8, "[]".to_owned());
        assert!(matches!(response, SpotWsError::Response { id: 8, .. }));
        assert!(matches!(response.shape(), Shape::Response));
        let many = SpotWsError::too_many_streams(StreamPath::Stream, 1024);
        assert!(matches!(
            many,
            SpotWsError::TooManyStreams {
                path: StreamPath::Stream,
                limit: 1024
            }
        ));
        assert!(matches!(many.shape(), Shape::TooManyStreams));
        let frame = SpotWsError::Frame {
            stream: String::new(),
            raw: String::new(),
            reason: String::new(),
        };
        assert!(matches!(frame.shape(), Shape::Frame));
    }

    #[test]
    fn a_stop_carries_the_cause_it_was_given_and_the_cause_reads_the_error() {
        let refused = SpotWsError::refused(2, "Invalid request".to_owned());
        let cause = <SpotWsError as SocketError>::cause(&refused);
        assert_eq!(cause, StopCause::of(&refused));
        assert_eq!(cause.message, refused.to_string());
        let stopped = SpotWsError::stopped(Some(cause.clone()));
        assert!(matches!(stopped.shape(), Shape::Stopped));
        assert_eq!(stopped.class(), refused.class());
        assert!(matches!(
            SpotWsError::stopped(None),
            SpotWsError::Stopped { cause: None }
        ));
    }
}
```

Create `polyoxide-binance/src/spot/ws/supervised.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disconnect_reason_reads_as_the_texts_usdm_gives() {
        assert_eq!(
            DisconnectReason::Closed {
                code: Some(1008),
                reason: "Too many requests".to_owned()
            }
            .to_string(),
            "closed by the server (1008 Too many requests)"
        );
        assert_eq!(
            DisconnectReason::Closed {
                code: None,
                reason: String::new()
            }
            .to_string(),
            "closed by the server"
        );
        let refused = SpotWsError::Refused {
            code: 2,
            msg: "Invalid request".to_owned(),
        };
        let text = refused.to_string();
        assert_eq!(DisconnectReason::Error(refused).to_string(), text);
        assert_eq!(
            DisconnectReason::Stale.to_string(),
            "no frame within the staleness window"
        );
        assert_eq!(
            DisconnectReason::Rotation.to_string(),
            "rotated before the 24-hour cutoff"
        );
    }
}
```

Create `polyoxide-binance/src/spot/ws/client.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spot::{
        types::Symbol,
        ws::{
            event::Payload,
            fixtures,
            scripted::{Script, ScriptedServer},
        },
    };

    fn agg(symbol: &str) -> StreamName {
        StreamName::AggTrade(Symbol::new(symbol).unwrap())
    }

    fn many(n: usize) -> Vec<StreamName> {
        (0..n).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect()
    }

    #[test]
    fn the_documented_limits_are_the_measured_ones() {
        // docs/specs/binance/OBSERVED.md, `## Spot`: 1024 names in one request
        // held, 1024 streams per connection, one request every 500 ms held for
        // 45 s where one every 250 ms was closed with 1008.
        assert_eq!(MAX_NAMES_PER_REQUEST, 1024);
        assert_eq!(MAX_STREAMS_PER_CONNECTION, 1024);
        assert_eq!(MIN_REQUEST_INTERVAL, Duration::from_millis(500));
        assert_eq!(SPOT_WS_BASE, "wss://stream.binance.com:9443");
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(ANSWER_TIMEOUT, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn connect_subscribes_on_the_one_path_and_yields_updates() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![fixtures::AGG_TRADE.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        let update = ws.next().await.unwrap().unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
        assert_eq!(server.paths(), ["/stream"]);
        assert_eq!(
            server.requests()[0]["params"],
            serde_json::json!(["btcusdt@aggTrade"])
        );
    }

    #[tokio::test]
    async fn a_request_carries_all_1024_names() {
        // Bug: batching at USDⓈ-M's 200 names, which sends six requests where
        // one is measured to hold, and replays slowly at 500 ms apiece.
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let ws = SpotWs::connect_to(&server.url, many(1024)).await.unwrap();
        assert_eq!(ws.streams().len(), 1024);
        let sizes: Vec<usize> = server
            .requests()
            .iter()
            .map(|r| r["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [1024]);
    }

    #[tokio::test]
    async fn requests_are_paced_at_500_ms() {
        // Bug: a pace of 250 ms, which the host closed with 1008 "Too many
        // requests" once the pings and pongs were counted. The gap is read off
        // the server's clock against the measured 500 ms, not the constant.
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("AAAUSDT")])
            .await
            .unwrap();
        ws.subscribe(&[agg("BBBUSDT")]).await.unwrap();
        ws.subscribe(&[agg("CCCUSDT")]).await.unwrap();
        let received = server.received();
        assert_eq!(received.len(), 3);
        for pair in received.windows(2) {
            let gap = pair[1].at - pair[0].at;
            assert!(
                gap >= Duration::from_millis(490),
                "requests {gap:?} apart"
            );
        }
    }

    #[tokio::test]
    async fn the_1025th_stream_is_refused_and_nothing_is_sent() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = SpotWs::connect_to(&server.url, many(1024)).await.unwrap();
        let sent = server.requests().len();
        let err = ws.subscribe(&[agg("BTCUSDT")]).await.unwrap_err();
        assert!(
            matches!(
                err,
                SpotWsError::TooManyStreams {
                    path: StreamPath::Stream,
                    limit: 1024
                }
            ),
            "{err:?}"
        );
        assert_eq!(server.requests().len(), sent);
        // Already-subscribed names do not count twice.
        ws.subscribe(&[agg("ZZ0000USDT")]).await.unwrap();
    }

    #[tokio::test]
    async fn connecting_with_1025_names_sends_nothing() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let err = SpotWs::connect_to(&server.url, many(1025))
            .await
            .unwrap_err();
        assert!(
            matches!(err, SpotWsError::TooManyStreams { limit: 1024, .. }),
            "{err:?}"
        );
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn an_error_answer_is_refused_and_the_connection_kept() {
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
            ..Default::default()
        }])
        .await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, SpotWsError::Refused { code: 2, .. }),
            "{err:?}"
        );
        assert_eq!(ws.streams(), &[agg("BTCUSDT")]);
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["btcusdt@aggTrade"]);
    }

    #[tokio::test]
    async fn a_ping_is_answered_and_unsubscribe_leaves_the_rest() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT"), agg("ETHUSDT")])
            .await
            .unwrap();
        assert!(ws.ping().await.unwrap() < Duration::from_secs(1));
        ws.unsubscribe(&[agg("BTCUSDT")]).await.unwrap();
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["ethusdt@aggTrade"]);
        // The supervised tier replays from this list after a reconnect.
        assert_eq!(ws.streams(), &[agg("ETHUSDT")]);
    }

    #[tokio::test]
    async fn the_server_s_close_code_is_reported() {
        let server = ScriptedServer::start(vec![Script {
            close_after: true,
            close_code: Some((1008, "Too many requests".into())),
            ..Default::default()
        }])
        .await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        assert!(ws.next().await.is_none());
        assert_eq!(
            ws.close_frame(),
            Some((Some(1008), "Too many requests".to_owned()))
        );
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, SpotWsError::Connect(_) | SpotWsError::Closed { .. }),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn an_answer_nobody_awaits_is_not_yielded() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![
                r#"{"result":null,"id":99}"#.into(),
                fixtures::AGG_TRADE.into(),
            ],
            ..Default::default()
        }])
        .await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        let update = ws.next().await.unwrap().unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::`

Expected: the build fails. The errors name `crate::shared::ws` (unresolved: the module is not built under `spot-ws` yet), `SpotStreams`, `SpotProtocol`, `Event`, `DisconnectReason`, `SpotWs`, `CONNECT_TIMEOUT` and `ANSWER_TIMEOUT` as not found in scope (`error[E0432]`, `error[E0412]`, `error[E0433]`), and `error: could not compile polyoxide-binance` ends the output.

- [ ] **Step 3: Implement**

In `polyoxide-binance/src/shared/mod.rs`, Story 2 left the module declaration

```rust
#[cfg(feature = "usdm-ws")]
pub(crate) mod ws;
```

Replace the `cfg` line (leave `pub(crate) mod ws;` where it is) so that the declaration reads

```rust
#[cfg(any(feature = "usdm-ws", feature = "spot-ws"))]
pub(crate) mod ws;
```

In `polyoxide-binance/src/spot/ws/mod.rs`, `StreamPath::url` gains its first caller (`SpotStreams::url` below), so Story 3's temporary allow goes. Replace

```rust
    /// The combined-stream URL on `base`.
    // Story 4's `SpotStreams::url` is the first caller; it removes this allow.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn url(self, base: &str) -> String {
```

with

```rust
    /// The combined-stream URL on `base`.
    pub(crate) fn url(self, base: &str) -> String {
```

Put this block at the top of `polyoxide-binance/src/spot/ws/protocol.rs`, above the `#[cfg(test)] mod tests` Step 1 wrote:

```rust
//! Spot's side of the shared socket plumbing (`shared::ws`): the stream set
//! that the generic connection, protocol and router are written over, and the
//! error's view they match on. Spot has one path, so the error never has a
//! wrong-path shape.

use std::time::Duration;

use polyoxide_ws::Outage;

use crate::{
    shared::ws::{Shape, SocketError, StreamSet},
    spot::ws::{
        error::{SpotWsError, StopCause},
        event::Update,
        stream::StreamName,
        supervised::{DisconnectReason, Event},
        StreamPath, MAX_NAMES_PER_REQUEST, MIN_REQUEST_INTERVAL,
    },
};

/// Spot's stream set: one path, `/stream`, up to 1024 names in a request and
/// one request per 500 ms.
pub(crate) struct SpotStreams;

/// Binance's kit `Protocol` on spot's one path. Only the unit tests name it:
/// the shared `connect` builds the protocol for the stream set.
#[cfg(test)]
pub(crate) type SpotProtocol = crate::shared::ws::protocol::BinanceProtocol<SpotStreams>;

impl StreamSet for SpotStreams {
    type Name = StreamName;
    type Path = StreamPath;
    type Update = Update;
    type Error = SpotWsError;
    type Event = Event;

    const PATHS: &'static [StreamPath] = StreamPath::ALL;
    const MAX_NAMES_PER_REQUEST: usize = MAX_NAMES_PER_REQUEST;
    const MIN_REQUEST_INTERVAL: Duration = MIN_REQUEST_INTERVAL;

    fn path(name: &StreamName) -> StreamPath {
        name.path()
    }

    fn url(base: &str, path: StreamPath) -> String {
        path.url(base)
    }

    fn decode(text: &str) -> Result<Update, SpotWsError> {
        Update::from_json(text)
    }

    fn update(update: Update) -> Event {
        Event::Update(Box::new(update))
    }

    fn disconnected(path: StreamPath, outage: Outage<SpotWsError>) -> Event {
        let reason = match outage {
            Outage::Closed { code, reason } => DisconnectReason::Closed { code, reason },
            Outage::Error(err) => DisconnectReason::Error(err),
            Outage::Rotation => DisconnectReason::Rotation,
            // `Stale`, and any outage a later kit adds.
            _ => DisconnectReason::Stale,
        };
        Event::Disconnected { path, reason }
    }

    fn reconnected(path: StreamPath) -> Event {
        Event::Reconnected { path }
    }
}

impl SocketError for SpotWsError {
    type Path = StreamPath;
    type Cause = StopCause;

    fn refused(code: i64, msg: String) -> Self {
        Self::Refused { code, msg }
    }

    fn no_answer(id: u64, timeout: Duration) -> Self {
        Self::NoAnswer { id, timeout }
    }

    fn response(id: u64, raw: String) -> Self {
        Self::Response { id, raw }
    }

    fn too_many_streams(path: StreamPath, limit: usize) -> Self {
        Self::TooManyStreams { path, limit }
    }

    fn stopped(cause: Option<StopCause>) -> Self {
        Self::Stopped { cause }
    }

    fn cause(&self) -> StopCause {
        StopCause::of(self)
    }

    fn shape(&self) -> Shape<'_> {
        match self {
            Self::Connect(err) => Shape::Connect(err),
            Self::ConnectTimeout(_) => Shape::ConnectTimeout,
            Self::Closed { .. } => Shape::Closed,
            Self::Refused { .. } => Shape::Refused,
            Self::NoAnswer { .. } => Shape::NoAnswer,
            Self::Response { .. } => Shape::Response,
            Self::TooManyStreams { .. } => Shape::TooManyStreams,
            Self::Frame { .. } => Shape::Frame,
            Self::Stopped { .. } => Shape::Stopped,
        }
    }
}
```

Put this block at the top of `polyoxide-binance/src/spot/ws/supervised.rs`, above its `#[cfg(test)] mod tests`:

```rust
//! The supervised tier: the Binance protocol on the kit's `Supervisor`, with
//! keep-alive on the wall clock, staleness detection, reconnect with a paced
//! replay of the streams, rotation before Binance's 24-hour cutoff and outage
//! markers. The protocol, the router and the forwarding are `shared::ws`'s.

use std::fmt;

use crate::spot::ws::{error::SpotWsError, event::Update, StreamPath};

/// What the supervised stream yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// One frame of one stream, boxed, as the other variants are small.
    Update(Box<Update>),
    /// The connection was lost. Every update is stale until the matching
    /// [`Event::Reconnected`], which always follows while the client runs.
    Disconnected {
        /// The path that went down: spot has one, [`StreamPath::Stream`].
        path: StreamPath,
        /// Why.
        reason: DisconnectReason,
    },
    /// The path is current again: a fresh connection has replayed its
    /// streams, or its last stream left during the outage, so nothing on it is
    /// stale. Sent once per outage, however many attempts it took. Drop book
    /// state and resync.
    Reconnected {
        /// The path that came back.
        path: StreamPath,
    },
}

/// Why the connection went down.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed it, with the code and reason it sent, or ended the
    /// stream with neither.
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },
    /// A transport error, a replay left unanswered for the staleness window,
    /// or a failed first connect of a path opened by a membership change.
    Error(SpotWsError),
    /// Nothing arrived, pongs included, for the staleness window, or a
    /// membership change went unanswered for it.
    Stale,
    /// Replaced on purpose before Binance's 24-hour cutoff.
    Rotation,
}

impl fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed { code, reason } => match code {
                Some(code) => write!(f, "closed by the server ({code} {reason})"),
                None => f.write_str("closed by the server"),
            },
            Self::Error(err) => write!(f, "{err}"),
            Self::Stale => f.write_str("no frame within the staleness window"),
            Self::Rotation => f.write_str("rotated before the 24-hour cutoff"),
        }
    }
}
```

Put this block at the top of `polyoxide-binance/src/spot/ws/client.rs`, above its `#[cfg(test)] mod tests`:

```rust
//! The bare tier: one connection on spot's one path, control methods, and a
//! `Stream` of updates.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};

use crate::{
    shared::ws::bare::Bare,
    spot::ws::{
        error::SpotWsError, event::Update, protocol::SpotStreams, stream::StreamName, StreamPath,
        MAX_NAMES_PER_REQUEST, MAX_STREAMS_PER_CONNECTION, MIN_REQUEST_INTERVAL, SPOT_WS_BASE,
    },
};

/// How the bare tier waits for a connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the bare tier waits for a request's answer.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// A bare connection to spot's combined stream.
///
/// Requests carry at most 1024 names, the connection's cap, and are sent no
/// faster than one per 500 ms; each waits for the answer carrying its id, and
/// updates read meanwhile are kept and yielded by the stream afterwards. A
/// stream that would take the connection past 1024 streams is refused before
/// anything is sent. The control methods are not cancel-safe, and after a
/// `NoAnswer` the server's membership is unknown: replace the connection
/// rather than reuse it, as the supervised tier does. The server's protocol
/// pings are answered while the caller polls, and its close is answered with
/// the close reply. The stream ends after the server's close, or after the
/// first transport error it yields; a control call after that fails, with the
/// close as [`SpotWsError::Closed`] or, after an error, as
/// [`SpotWsError::Connect`] whose kind is `AlreadyClosed`, and sends nothing.
/// Binance closes a connection after 24 hours; the supervised tier
/// (`SpotWsBuilder`) replaces it before then, and this tier does not.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::spot::{types::Symbol, ws::{SpotWs, StreamName}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut ws = SpotWs::connect([StreamName::AggTrade(btc)]).await?;
/// while let Some(update) = ws.next().await {
///     println!("{}", serde_json::to_string(&update?)?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct SpotWs {
    bare: Bare<SpotStreams>,
}

impl std::fmt::Debug for SpotWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpotWs")
            .field("streams", &self.bare.streams().len())
            .field("connection", &self.bare.connection())
            .finish_non_exhaustive()
    }
}

impl SpotWs {
    /// Connect to the production host and subscribe.
    pub async fn connect(
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, SpotWsError> {
        Self::connect_to(SPOT_WS_BASE, streams).await
    }

    /// Connect to another host, such as a local test server, and subscribe.
    /// The client appends `/stream` to `base_url`.
    pub async fn connect_to(
        base_url: &str,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, SpotWsError> {
        let streams: Vec<StreamName> = streams.into_iter().collect();
        let mut ws = Self::open(base_url, CONNECT_TIMEOUT, ANSWER_TIMEOUT).await?;
        ws.subscribe(&streams).await?;
        Ok(ws)
    }

    /// Open a connection with no streams.
    pub(crate) async fn open(
        base_url: &str,
        connect_timeout: Duration,
        answer_timeout: Duration,
    ) -> Result<Self, SpotWsError> {
        let bare = Bare::open(base_url, StreamPath::Stream, connect_timeout, answer_timeout)
            .await?;
        Ok(Self { bare })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with [`SpotWsError::TooManyStreams`] before sending anything, and
    /// with [`SpotWsError::Refused`] when the server answers with an error;
    /// batches answered before it stay subscribed, and
    /// [`streams`](Self::streams) says which.
    pub async fn subscribe(&mut self, streams: &[StreamName]) -> Result<(), SpotWsError> {
        self.bare.subscribe(streams).await
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub async fn unsubscribe(&mut self, streams: &[StreamName]) -> Result<(), SpotWsError> {
        self.bare.unsubscribe(streams).await
    }

    /// The stream names the server says this connection carries.
    pub async fn list_subscriptions(&mut self) -> Result<Vec<String>, SpotWsError> {
        self.bare.list_subscriptions().await
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub async fn ping(&mut self) -> Result<Duration, SpotWsError> {
        self.bare.ping().await
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), SpotWsError> {
        self.bare.close().await
    }

    /// The streams this connection carries.
    pub fn streams(&self) -> &[StreamName] {
        self.bare.streams()
    }

    /// The server's close code and reason, once it has closed the connection
    /// or the stream has ended without a close frame.
    #[cfg(test)]
    fn close_frame(&self) -> Option<(Option<u16>, String)> {
        self.bare.close_frame()
    }
}

impl Stream for SpotWs {
    type Item = Result<Update, SpotWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.bare.poll_next_unpin(cx)
    }
}
```

`MAX_NAMES_PER_REQUEST`, `MAX_STREAMS_PER_CONNECTION` and `MIN_REQUEST_INTERVAL` are imported for the limits test, so they are unused outside `cfg(test)`. Make that import test-only: replace

```rust
        error::SpotWsError, event::Update, protocol::SpotStreams, stream::StreamName, StreamPath,
        MAX_NAMES_PER_REQUEST, MAX_STREAMS_PER_CONNECTION, MIN_REQUEST_INTERVAL, SPOT_WS_BASE,
    },
};
```

with

```rust
        error::SpotWsError, event::Update, protocol::SpotStreams, stream::StreamName, StreamPath,
        SPOT_WS_BASE,
    },
};

#[cfg(test)]
use crate::spot::ws::{MAX_NAMES_PER_REQUEST, MAX_STREAMS_PER_CONNECTION, MIN_REQUEST_INTERVAL};
```

(An import only tests use is a module-level `#[cfg(test)] use`, never inside the tests module.) Then run `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib spot::ws::`

Expected: `test result: ok.` with the new `spot::ws::protocol::tests::*` (10), `spot::ws::supervised::tests::a_disconnect_reason_reads_as_the_texts_usdm_gives` and `spot::ws::client::tests::*` (10) among them, and Story 3's tests unchanged.

Then the task's gates, each passing:

```
nix develop --command cargo fmt --all -- --check
nix develop --command cargo check -p polyoxide-binance --no-default-features --features spot-ws,test-server --all-targets
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'
```

(No spot-only `clippy -D warnings` here: under `spot-ws` alone `shared::ws::protocol`, `supervised` and `EVENT_BUFFER` have no caller until Task 4.2 builds the supervised tier, so it would report them as dead code. Task 4.2 runs it.)

- [ ] **Step 5: Commit**

```
git add -N polyoxide-binance/src/spot/ws/protocol.rs polyoxide-binance/src/spot/ws/client.rs polyoxide-binance/src/spot/ws/supervised.rs
git commit -m "feat(binance): spot's stream set and bare socket on shared::ws (binance spot ws, Story 4)" -- polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/spot/ws/mod.rs polyoxide-binance/src/spot/ws/protocol.rs polyoxide-binance/src/spot/ws/client.rs polyoxide-binance/src/spot/ws/supervised.rs
```

### Task 4.2: The supervised tier and its suite

**Files:**
- Modify: `polyoxide-binance/src/spot/ws/supervised.rs`
- Modify: `polyoxide-binance/src/spot/ws/client.rs`
- Modify: `polyoxide-binance/src/spot/ws/mod.rs`
- Modify: `polyoxide-binance/src/shared/ws/mod.rs`
- Modify: `polyoxide-binance/src/shared/ws/bare.rs`
- Modify: `polyoxide-binance/Cargo.toml`
- Create: `polyoxide-binance/tests/supervision_spot.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-binance/Cargo.toml`, add the suite's stanza before `supervision_usdm`'s. Replace

```toml
[[test]]
name = "supervision_usdm"
required-features = ["usdm-ws", "test-server"]
```

with

```toml
[[test]]
name = "supervision_spot"
required-features = ["spot-ws", "test-server"]

[[test]]
name = "supervision_usdm"
required-features = ["usdm-ws", "test-server"]
```

Create `polyoxide-binance/tests/supervision_spot.rs`:

```rust
//! The supervised tier against the scripted server, with limits in hundreds of
//! milliseconds. Each test names the bug it exists to catch.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_binance::spot::{
    types::Symbol,
    ws::{
        fixtures, DepthLevels, DepthSpeed, DisconnectReason, Event, SpotWsBuilder, SpotWsError,
        StreamName, StreamPath, SupervisedSpotWs,
    },
};
use polyoxide_venue::{Class, Classify};

// Binance's answer on the shared scripted server, as the unit tests use it.
#[path = "../src/usdm/ws/scripted.rs"]
mod scripted;

use scripted::{Script, ScriptedServer};

fn symbol(s: &str) -> Symbol {
    Symbol::new(s).unwrap()
}

fn agg(s: &str) -> StreamName {
    StreamName::AggTrade(symbol(s))
}

fn btc_mini() -> StreamName {
    StreamName::MiniTicker(symbol("BTCUSDT"))
}

/// `n` aggregate-trade streams of unlisted symbols, `ZZ0000USDT` on.
fn many(n: usize) -> Vec<StreamName> {
    (0..n).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect()
}

fn fast(server: &ScriptedServer) -> SpotWsBuilder {
    SpotWsBuilder::new()
        .base_url(&server.url)
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
}

/// One push, then a close.
fn flap(frame: &str) -> Script {
    Script {
        pushes: vec![frame.into()],
        close_after: true,
        ..Default::default()
    }
}

/// One push, then steady traffic.
fn steady(frame: &str) -> Script {
    Script {
        pushes: vec![frame.into()],
        push_every: Some(Duration::from_millis(50)),
        ..Default::default()
    }
}

/// An upgrade refused with `status`, and a `Retry-After` when one is given.
fn refuse_upgrade(status: u16, retry_after: Option<&str>) -> Script {
    let headers = retry_after
        .map(|value| vec![("Retry-After".to_owned(), value.to_owned())])
        .unwrap_or_default();
    Script {
        refuse_upgrade: Some((status, headers)),
        ..Default::default()
    }
}

async fn next(ws: &mut SupervisedSpotWs) -> Result<Event, SpotWsError> {
    tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("an event within 3 s")
        .expect("the stream is open")
}

async fn next_event(ws: &mut SupervisedSpotWs) -> Event {
    next(ws).await.expect("an event, not an error")
}

#[tokio::test]
async fn every_kind_shares_the_one_connection_on_the_one_path() {
    // Bug: routing streams by kind as USDⓈ-M does, which opens a second
    // connection spot does not serve.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let depth = StreamName::PartialDepth(symbol("BTCUSDT"), DepthLevels::Twenty, DepthSpeed::Ms100);
    let ws = fast(&server)
        .streams([
            agg("BTCUSDT"),
            depth,
            StreamName::BookTicker(symbol("BTCUSDT")),
            StreamName::AllMiniTickers,
        ])
        .connect()
        .await
        .unwrap();
    assert_eq!(server.paths(), ["/stream"]);
    assert_eq!(server.connection_count(), 1);
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "SUBSCRIBE");
    assert_eq!(
        requests[0]["params"],
        serde_json::json!([
            "btcusdt@aggTrade",
            "btcusdt@depth20@100ms",
            "btcusdt@bookTicker",
            "!miniTicker@arr"
        ])
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_request_carries_all_1024_names() {
    // Bug: batching at USDⓈ-M's 200 names, six requests at 500 ms apiece where
    // one request of 1024 names was measured to hold.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(many(1024))
        .connect()
        .await
        .unwrap();
    let sizes: Vec<usize> = server
        .requests()
        .iter()
        .map(|r| r["params"].as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, [1024]);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn the_1025th_stream_is_refused_and_nothing_reaches_the_server() {
    // Bug: sending it, which makes the server close the connection and lose
    // all 1024.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(many(1024))
        .connect()
        .await
        .unwrap();
    let sent = server.received().len();
    let err = ws
        .membership()
        .subscribe([agg("BTCUSDT")])
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            SpotWsError::TooManyStreams {
                path: StreamPath::Stream,
                limit: 1024
            }
        ),
        "{err:?}"
    );
    assert_eq!(server.received().len(), sent);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn connecting_with_1025_names_is_an_error_and_sends_nothing() {
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let err = fast(&server)
        .streams(many(1025))
        .connect()
        .await
        .unwrap_err();
    assert!(
        matches!(err, SpotWsError::TooManyStreams { limit: 1024, .. }),
        "{err:?}"
    );
    assert!(server.received().is_empty());
}

#[tokio::test]
async fn a_membership_change_waits_500_ms_for_the_one_before() {
    // Bug: a pace of 250 ms, which the host closed with 1008 "Too many
    // requests" once pings and pongs were counted (OBSERVED.md, `### Pace at
    // 250 ms`). Gaps are read off the server's clock against the measured
    // 500 ms, not the constant.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .stale_after(Duration::from_secs(5))
        .streams([agg("AAAUSDT")])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    membership.subscribe([agg("BBBUSDT")]).await.unwrap();
    membership.unsubscribe([agg("AAAUSDT")]).await.unwrap();
    let received = server.received();
    assert_eq!(received.len(), 3);
    for pair in received.windows(2) {
        let gap = pair[1].at - pair[0].at;
        assert!(
            gap >= Duration::from_millis(490),
            "requests {gap:?} apart"
        );
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_replay_is_one_request_of_every_name() {
    // Bug: replaying in batches of 200, which is five requests and two
    // seconds at spot's pace where one request carries them all.
    let server = ScriptedServer::start(vec![
        Script {
            drop_after: Some(Duration::from_millis(400)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(many(1000))
        .connect()
        .await
        .unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    let received = server.received();
    for connection in 0..2 {
        let sizes: Vec<usize> = received
            .iter()
            .filter(|r| r.connection == connection)
            .map(|r| r.request["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [1000], "connection {connection}");
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_closed_connection_is_reported_replaced_and_resubscribed() {
    // Bug: missing resubscribe after a reconnect.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MINI_TICKER.into()],
            close_after: true,
            close_code: Some((1008, "Too many requests".into())),
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MINI_TICKER.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server)
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let Event::Disconnected { path, reason } = next_event(&mut ws).await else {
        panic!("expected Disconnected");
    };
    assert_eq!(path, StreamPath::Stream);
    assert!(
        matches!(&reason, DisconnectReason::Closed { code: Some(1008), reason } if reason == "Too many requests"),
        "{reason:?}"
    );
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Stream
        }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let replayed = server
        .received()
        .into_iter()
        .find(|r| r.connection == 1)
        .unwrap();
    assert_eq!(replayed.request["params"][0], "btcusdt@miniTicker");
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_old_connection_is_rotated_without_backoff() {
    // Bug: running into Binance's 24-hour cutoff unannounced.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .backoff(Duration::from_secs(10), Duration::from_secs(10))
        .max_connection_age(Duration::from_millis(200))
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    let started = tokio::time::Instant::now();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Stream,
            reason: DisconnectReason::Rotation
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "rotation waited out the backoff: {:?}",
        started.elapsed()
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_refused_replay_stops_the_client_and_its_cause_is_kept() {
    // Bug: a stop that forgets why it stopped, so `close()` and the handle
    // answer a bare `Stopped` and a refusal reads as an invalid request.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MINI_TICKER),
        Script {
            refuse: vec![("btcusdt@miniTicker".into(), 2, "Invalid request".into())],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server)
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Stream,
            ..
        }
    ));
    let refusal = next(&mut ws).await.expect_err("the refusal");
    assert!(
        matches!(refusal, SpotWsError::Refused { code: 2, .. }),
        "{refusal:?}"
    );
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");

    let stopped = ws.close().await.expect_err("a stop on an error");
    let SpotWsError::Stopped { cause: Some(cause) } = &stopped else {
        panic!("expected a stop with its cause, got {stopped:?}");
    };
    assert_eq!(cause.message, refusal.to_string());
    assert_eq!(
        cause.class,
        Class::VenueRefusal {
            code: Some("2".into())
        }
    );
    assert!(cause.is_fault);
    assert_eq!(stopped.class(), cause.class);

    let later = membership
        .subscribe([agg("ETHUSDT")])
        .await
        .expect_err("the client has stopped");
    assert!(
        matches!(&later, SpotWsError::Stopped { cause: Some(_) }),
        "{later:?}"
    );
    assert_eq!(later.class(), cause.class);
}

#[tokio::test]
async fn a_region_block_on_reconnect_stops_the_client_restricted_with_its_wait() {
    // A reconnect refused with 451 and a `Retry-After` stops the client.
    // `Stopped` carries the refusal's class, fault and wait, so a region block
    // is `Restricted`, not a fault, and tags environmental rather than real.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MINI_TICKER),
        refuse_upgrade(451, Some("7")),
    ])
    .await;
    let mut ws = fast(&server)
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    let refusal = next(&mut ws).await.expect_err("the refusal");
    assert!(matches!(refusal, SpotWsError::Connect(_)), "{refusal:?}");
    assert_eq!(refusal.retry_after(), Some(Duration::from_secs(7)));
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");

    let stopped = ws.close().await.expect_err("a stop on an error");
    let SpotWsError::Stopped { cause: Some(cause) } = &stopped else {
        panic!("expected a stop with its cause, got {stopped:?}");
    };
    assert_eq!(cause.class, Class::Restricted);
    assert!(!cause.is_fault);
    assert_eq!(cause.retry_after, Some(Duration::from_secs(7)));
    assert_eq!(stopped.class(), Class::Restricted);
    assert!(!stopped.is_fault(), "{stopped:?}");
}

#[tokio::test]
async fn a_refused_handshake_s_retry_after_is_waited_out() {
    // A 10 ms step and a 400 ms wait: the attempt after the refusal waits the
    // 400 ms, where it retried inside the server's window.
    let server = ScriptedServer::start(vec![
        flap(fixtures::MINI_TICKER),
        refuse_upgrade(503, Some("0.4")),
        steady(fixtures::MINI_TICKER),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(10), Duration::from_secs(5))
        .streams([btc_mini()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Stream,
            ..
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Stream
        }
    ));
    let accepted = server.accepted_at();
    let waited = accepted[2] - accepted[1];
    assert!(waited >= Duration::from_millis(400), "{waited:?}");
    ws.close().await.unwrap();
}
```

Add the builder's, the stream's and the handle's unit tests to `polyoxide-binance/src/spot/ws/supervised.rs`: extend the Step 1 tests module of Task 4.1. Replace

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disconnect_reason_reads_as_the_texts_usdm_gives() {
```

with

```rust
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_defaults_match_the_measured_cadences() {
        // Spot's server pings every 20 s (OBSERVED.md, `### Pings`); the rest
        // are usdm's.
        let b = SpotWsBuilder::new();
        assert_eq!(b.base_url, "wss://stream.binance.com:9443");
        assert_eq!(b.timing.ping_interval, Some(Duration::from_secs(20)));
        assert_eq!(b.timing.stale_after, Duration::from_secs(30));
        assert_eq!(b.timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(b.timing.max_backoff, Duration::from_secs(60));
        assert_eq!(b.timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(
            b.timing.max_connection_age,
            Some(Duration::from_secs(23 * 3600 + 50 * 60))
        );
        assert!(b.timing.max_connection_age < Some(Duration::from_secs(24 * 3600)));
        assert!(b.streams.is_empty());
    }

    #[test]
    fn the_setters_replace_their_field_and_default_is_new() {
        let b = SpotWsBuilder::default()
            .base_url("ws://127.0.0.1:9")
            .streams([StreamName::AllMiniTickers]);
        assert_eq!(b.base_url, "ws://127.0.0.1:9");
        assert_eq!(b.streams, [StreamName::AllMiniTickers]);
        assert_eq!(
            SpotWsBuilder::default().timing.stale_after,
            SpotWsBuilder::new().timing.stale_after
        );
    }

    #[test]
    fn a_disconnect_reason_reads_as_the_texts_usdm_gives() {
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --test supervision_spot`

Expected: the build fails: `error[E0432]: unresolved imports polyoxide_binance::spot::ws::SpotWsBuilder, polyoxide_binance::spot::ws::SupervisedSpotWs` (not exported yet), and `error: could not compile polyoxide-binance (test "supervision_spot")`.

- [ ] **Step 3: Implement**

In `polyoxide-binance/src/spot/ws/supervised.rs`, replace the import block Task 4.1 wrote

```rust
use std::fmt;

use crate::spot::ws::{error::SpotWsError, event::Update, StreamPath};
```

with

```rust
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use polyoxide_ws::Timing;

use crate::{
    shared::ws::{
        supervised::{connect as connect_running, Handle, Running, Settings},
        DEFAULT_MAX_CONNECTION_AGE,
    },
    spot::ws::{
        error::SpotWsError, event::Update, protocol::SpotStreams, stream::StreamName, StreamPath,
        SPOT_WS_BASE,
    },
};
```

and put this block after the `impl fmt::Display for DisconnectReason` block and before the `#[cfg(test)] mod tests`:

```rust
/// Builder for a supervised connection.
///
/// The connection runs on its own task, on the socket kit's `Supervisor`, and
/// logs under the `polyoxide_ws` target.
///
/// While the consumer leaves 1024 events unread, the connection stops reading
/// but keeps pinging, and that wait is not silence: a slow consumer is never
/// reported [`Stale`](DisconnectReason::Stale), and no event is dropped. The
/// server's own pings go unanswered meanwhile. Binance closes a connection
/// that has not answered them for a minute; that close is reconnected like any
/// other, and the events already queued are kept.
#[derive(Debug, Clone)]
pub struct SpotWsBuilder {
    base_url: String,
    timing: Timing,
    streams: Vec<StreamName>,
}

impl Default for SpotWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotWsBuilder {
    /// Defaults: the production host, a ping every 20 s (the server's own
    /// cadence), stale after 30 s of silence, reconnect backoff from 500 ms
    /// doubling to 60 s, a 10 s connect timeout, rotation after 23 h 50 min,
    /// and no streams.
    pub fn new() -> Self {
        let mut timing = Timing::default();
        timing.max_connection_age = Some(DEFAULT_MAX_CONNECTION_AGE);
        Self {
            base_url: SPOT_WS_BASE.to_owned(),
            timing,
            streams: Vec::new(),
        }
    }

    /// Connect to another host, such as a local test server. The client
    /// appends `/stream`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    polyoxide_ws::timing_setters! {
        /// How often the connection sends a protocol ping, whatever its
        /// traffic, and also while the consumer leaves 1024 events unread.
        /// None goes out while a membership change is in flight, which
        /// `stale_after` bounds. Binance closes a connection that receives
        /// more than 5 messages a second, requests, pings and pongs counted.
        ping_interval;
        /// Silence after which the connection is presumed dead and replaced.
        /// Pongs and the server's pings count, so a quiet connection that
        /// answers stays up, and time the consumer leaves 1024 events unread
        /// does not count.
        ///
        /// Also bounds each replay and each membership change as a whole, its
        /// 500 ms pacing included: one the server leaves unanswered that long
        /// replaces the connection. A replay is one request, whatever the
        /// number of streams, since a request carries up to 1024 names.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max`, with no cap
        /// on attempts, and back to `initial` after a connection that
        /// delivered an update. Both are raised to at least 1 ms, and an
        /// `initial` above `max` is lowered to `max`. A reconnect refused with
        /// a `Retry-After` waits at least that long, up to `max` (DRIFT R11).
        backoff;
        /// How long one connection attempt may take to open.
        connect_timeout;
        /// How long a connection may live before it is replaced, without
        /// backoff, ahead of Binance's 24-hour cutoff. Each replacement is a
        /// new connection, and Binance allows 300 connection attempts per 5
        /// minutes per IP.
        max_connection_age;
    }

    /// The streams to open with.
    pub fn streams(mut self, streams: impl IntoIterator<Item = StreamName>) -> Self {
        self.streams = streams.into_iter().collect();
        self
    }

    /// Open the connection, subscribe, and start supervising.
    ///
    /// Fails if the first connect fails, or if the streams pass 1024: a setup
    /// that is wrong is reported, not retried. With no streams it opens
    /// nothing until the first [`MembershipHandle::subscribe`].
    pub async fn connect(self) -> Result<SupervisedSpotWs, SpotWsError> {
        let running = connect_running::<SpotStreams>(Settings {
            base_url: self.base_url,
            timing: self.timing,
            streams: self.streams,
        })
        .await?;
        Ok(SupervisedSpotWs { running })
    }
}

/// Changes the streams of a running [`SupervisedSpotWs`].
///
/// The connection is opened for the first stream and closed when the last
/// leaves. The membership is a set: subscribing a stream twice holds it once,
/// and counting references is the caller's job.
///
/// A change is recorded before it is sent. During an outage it is answered
/// `Ok` at once, and the replay applies it; so is a change whose connection
/// fails under it, or one the server leaves unanswered for `stale_after`,
/// which replaces the connection. A change that arrives while a connect or a
/// replay is in flight waits for that attempt. Changes are sent no faster than
/// one per 500 ms.
///
/// A call fails when the server refuses it ([`SpotWsError::Refused`]), when it
/// would pass 1024 streams ([`SpotWsError::TooManyStreams`], nothing sent),
/// when the first connect of a connection it opens fails in a way retrying
/// cannot fix ([`SpotWsError::Connect`]), or when the client has stopped
/// ([`SpotWsError::Stopped`]). Its `cause` is the error the client stopped on
/// once the router has that stop; a call in flight when it stops can get
/// `Stopped { cause: None }`. A request carries up to 1024 names, so a
/// subscription is never split.
///
/// A call waits for the connection's task, which takes no change while the
/// consumer leaves 1024 events unread. Make membership changes from a task
/// other than the one draining the stream, or keep draining while they run.
#[derive(Debug, Clone)]
pub struct MembershipHandle {
    handle: Handle<SpotStreams>,
}

impl MembershipHandle {
    /// Subscribe to more streams.
    pub async fn subscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), SpotWsError> {
        self.handle.subscribe(streams.into_iter().collect()).await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub async fn unsubscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), SpotWsError> {
        self.handle.unsubscribe(streams.into_iter().collect()).await
    }
}

/// A supervised connection: a `Stream` of [`Event`]s that survives drops,
/// stalls and Binance's 24-hour cutoff.
///
/// Every [`Event::Disconnected`] is followed by an [`Event::Reconnected`] for
/// the same path while the client runs. A connection that closes because its
/// last stream left, while it is up, yields neither.
///
/// A fatal error, such as the server refusing a replay after a reconnect, is
/// yielded as `Err` and then the stream ends. An `Err(SpotWsError::Frame { .. })`,
/// a frame that did not decode, does not end it.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::spot::{types::Symbol, ws::{Event, SpotWsBuilder, SpotWsError, StreamName}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut feed = SpotWsBuilder::new()
///     .streams([StreamName::MiniTicker(btc.clone()), StreamName::BookTicker(btc)])
///     .connect()
///     .await?;
/// let membership = feed.membership();
/// while let Some(event) = feed.next().await {
///     let event = match event {
///         Ok(event) => event,
///         // A frame that did not decode is skipped; anything else ends the stream.
///         Err(SpotWsError::Frame { .. }) => continue,
///         Err(err) => return Err(err.into()),
///     };
///     match event {
///         Event::Update(update) => println!("{}", serde_json::to_string(&update)?),
///         Event::Disconnected { path, reason } => eprintln!("{path} down: {reason}"),
///         Event::Reconnected { path } => eprintln!("{path} back"),
///         _ => {}
///     }
/// }
/// # membership.subscribe([StreamName::AllMiniTickers]).await?;
/// # Ok(())
/// # }
/// ```
pub struct SupervisedSpotWs {
    running: Running<SpotStreams>,
}

impl fmt::Debug for SupervisedSpotWs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupervisedSpotWs")
            .field("running", &self.running.is_running())
            .finish_non_exhaustive()
    }
}

impl SupervisedSpotWs {
    /// A handle for changing streams while the stream runs.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            handle: self.running.membership(),
        }
    }

    /// Close the connection and stop. Every [`MembershipHandle`] answers
    /// [`SpotWsError::Stopped`] afterwards.
    ///
    /// Fails with [`SpotWsError::Stopped`] if the client had stopped on an
    /// error, which the stream yielded: its `cause` is that error, as
    /// [`Classify`](polyoxide_venue::Classify) read it, and so is the cause
    /// every later membership call carries. That holds once the router has the
    /// stop: a `close()` made right after the stream yielded the stopping
    /// error can return `Ok` or `Stopped { cause: None }`. Read the stream to
    /// its end first to be sure of the cause.
    pub async fn close(self) -> Result<(), SpotWsError> {
        self.running.close().await
    }
}

impl Stream for SupervisedSpotWs {
    type Item = Result<Event, SpotWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.running.poll_event(cx)
    }
}
```

In `polyoxide-binance/src/spot/ws/mod.rs`, replace the re-export Task 4.1 added

```rust
pub use supervised::{DisconnectReason, Event};
```

with

```rust
pub use supervised::{DisconnectReason, Event, MembershipHandle, SpotWsBuilder, SupervisedSpotWs};
```

In `polyoxide-binance/src/spot/ws/client.rs`, `SpotWsBuilder` now exists, so the `SpotWs` doc links it. Replace

```rust
/// (`SpotWsBuilder`) replaces it before then, and this tier does not.
```

with

```rust
/// ([`SpotWsBuilder`](crate::spot::ws::SpotWsBuilder)) replaces it before then,
/// and this tier does not.
```

Spot-only builds read two items that only the USDⓈ-M wrapper uses, so under `spot-ws` without `usdm-ws` the compiler calls them dead and the spot-only clippy below fails. Mark both, quoting Story 2's lines.

In `polyoxide-binance/src/shared/ws/mod.rs`, replace

```rust
    /// A stream for the other path, refused before sending.
    WrongPath,
```

with

```rust
    /// A stream for the other path, refused before sending.
    #[cfg_attr(not(feature = "usdm-ws"), allow(dead_code))]
    WrongPath,
```

In `polyoxide-binance/src/shared/ws/bare.rs`, replace

```rust
    /// The path this connection is on.
    pub(crate) fn path(&self) -> S::Path {
```

with

```rust
    /// The path this connection is on.
    #[cfg_attr(not(feature = "usdm-ws"), allow(dead_code))]
    pub(crate) fn path(&self) -> S::Path {
```

If the spot-only clippy below names another item as never used, put the same `#[cfg_attr(not(feature = "usdm-ws"), allow(dead_code))]` on it and say so in the commit message body.

Then run `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server --lib --test supervision_spot`

Expected: `test result: ok.` for both targets; `supervision_spot` reports 11 passed (it takes about 3 s: the pace test waits 1 s), and the lib's `spot::ws::supervised::tests` reports its three tests among Story 3's.

Then the task's gates, each passing:

```
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy -p polyoxide-binance --no-default-features --features spot-ws,test-server --all-targets -- -D warnings
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command cargo test --doc -p polyoxide-binance --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'
nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
```

- [ ] **Step 5: Commit**

```
git add -N polyoxide-binance/tests/supervision_spot.rs
git commit -m "feat(binance): spot's supervised socket, builder and membership handle (binance spot ws, Story 4)" -- polyoxide-binance/src/spot/ws/supervised.rs polyoxide-binance/src/spot/ws/client.rs polyoxide-binance/src/spot/ws/mod.rs polyoxide-binance/src/shared/ws/mod.rs polyoxide-binance/src/shared/ws/bare.rs polyoxide-binance/Cargo.toml polyoxide-binance/tests/supervision_spot.rs
```

### Task 4.3: The module docs, the live suite and the registration

**Files:**
- Modify: `Cargo.toml`
- Modify: `polyoxide-binance/src/spot/ws/mod.rs`
- Modify: `polyoxide-binance/Cargo.toml`
- Create: `polyoxide-binance/tests/live_spot_ws.rs`
- Modify: `scripts/live_unwraps.baseline.json`
- Modify: the files `python3 scripts/gen_registry.py --write` rewrites (`CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`), and `Cargo.lock` if cargo changes it

- [ ] **Step 1: Write the failing test**

Create `polyoxide-binance/tests/live_spot_ws.rs`. It reaches production, so it paces as the client does: the supervised test sends one `SUBSCRIBE`, the drift test sends none (its names ride the URL's `streams=` query), and the ping test sends a subscribe, a ping and a list at least 500 ms apart.

```rust
//! Live tests against `stream.binance.com`, `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --features spot-ws --test live_spot_ws -- --ignored
//! ```
//!
//! `live_frames_carry_no_unmodelled_keys` is the streams' drift detector. It
//! sees keys and the length of a book level this version does not model, and
//! fails when a stream sends nothing to check. It does not see an array row
//! whose `e` changed, nor a new value of any other enum, which decodes as
//! `Other`. Binance documents `M` on trades and aggregate trades and `B` on the
//! kline as "Ignore", and the types do not model them.
//!
//! Each failure prints the tag the nightly classifier reads, taken from its
//! error's class: a connect timeout, a 5xx or a reset is transient, a 451
//! environmental, and a 418, a 403 or a malformed frame real. A stream that
//! ends without saying why is transient. A kind that delivered nothing is
//! real, since ten BTCUSDT streams are never all quiet for a minute, and so
//! is an unanswered ping, which is what `live_a_client_ping_is_answered` tests.
//!
//! The address is shared with other sessions and the host closes a connection
//! that sends more than 5 messages a second, so nothing here sends faster than
//! the client paces: one `SUBSCRIBE` for the supervised feed, none for the
//! drift check, and requests at least 500 ms apart for the bare tier.

use polyoxide_test_support::agreement as common;

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_binance::spot::{
    types::{Interval, Symbol},
    ws::{
        client::CONNECT_TIMEOUT, DepthLevels, DepthSpeed, Event, Payload, SpotWs, SpotWsBuilder,
        SpotWsError, StreamName, StreamPath, Update, SPOT_WS_BASE,
    },
};
use polyoxide_test_support::{fail, transient, ResultExt};
use polyoxide_ws::{tungstenite::Message, WsError};
use serde_json::Value;

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").or_fail("BTCUSDT")
}

fn every_kind() -> Vec<StreamName> {
    vec![
        StreamName::AggTrade(btc()),
        StreamName::Trade(btc()),
        StreamName::Kline(btc(), Interval::Minute1),
        StreamName::MiniTicker(btc()),
        StreamName::AllMiniTickers,
        StreamName::Ticker(btc()),
        StreamName::BookTicker(btc()),
        StreamName::PartialDepth(btc(), DepthLevels::Twenty, DepthSpeed::Ms100),
        StreamName::PartialDepth(btc(), DepthLevels::Five, DepthSpeed::Ms1000),
        StreamName::AvgPrice(btc()),
    ]
}

/// A book level is `[price, quantity]`.
const LEVEL_VALUES: usize = 2;

const KINDS: [&str; 10] = [
    "aggTrade",
    "trade",
    "kline",
    "miniTicker",
    "all miniTickers",
    "ticker",
    "book ticker",
    "partial depth",
    "partial depth, bare",
    "avgPrice",
];

fn kind(update: &Update) -> &'static str {
    match (&update.payload, update.stream.to_string().ends_with("@100ms")) {
        (Payload::AggTrade(_), _) => "aggTrade",
        (Payload::Trade(_), _) => "trade",
        (Payload::Kline(_), _) => "kline",
        (Payload::MiniTicker(_), _) => "miniTicker",
        (Payload::MiniTickers(_), _) => "all miniTickers",
        (Payload::Ticker(_), _) => "ticker",
        (Payload::BookTicker(_), _) => "book ticker",
        (Payload::PartialDepth(_), true) => "partial depth",
        (Payload::PartialDepth(_), false) => "partial depth, bare",
        (Payload::AvgPrice(_), _) => "avgPrice",
        _ => "unknown",
    }
}

#[tokio::test]
#[ignore]
async fn live_every_stream_kind_delivers() {
    // One SUBSCRIBE of ten names on one connection.
    let mut feed = SpotWsBuilder::new()
        .streams(every_kind())
        .connect()
        .await
        .or_fail("connect");
    let mut seen = HashSet::new();
    let mut last_outage = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while seen.len() < KINDS.len() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            // Ten BTCUSDT streams are never all quiet for a minute: a kind
            // missing here stopped delivering, or the connection never came
            // back.
            let missing: Vec<&str> = KINDS.into_iter().filter(|k| !seen.contains(k)).collect();
            let finding = format!(
                "in 60 s these kinds delivered nothing: {missing:?}; last outage: {last_outage:?}"
            );
            panic!("{finding}"); // live-unwraps: a kind that delivered nothing is the fault under test
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the connection"))
            .or_fail("the supervised feed");
        match event {
            Event::Update(update) => {
                assert!(
                    !matches!(update.payload, Payload::Unknown { .. }),
                    "{update:?}"
                );
                seen.insert(kind(&update));
            }
            Event::Disconnected { path, reason } => {
                last_outage = Some(format!("{path}: {reason:?}"));
            }
            _ => {}
        }
    }
    feed.close().await.or_fail("close");
}

#[tokio::test]
#[ignore]
async fn live_frames_carry_no_unmodelled_keys() {
    let names: Vec<String> = every_kind().into_iter().map(|s| s.to_string()).collect();
    // The names ride the URL, so this connection sends no request at all.
    let url = format!(
        "{SPOT_WS_BASE}/{}?streams={}",
        StreamPath::Stream.as_str(),
        names.join("/")
    );
    let mut unmodelled = Vec::new();
    // A raw socket through the kit's connect, which installs the TLS provider
    // (DRIFT R9) and fails a timeout as `ConnectTimeout`.
    let mut socket = polyoxide_ws::connect(url, CONNECT_TIMEOUT)
        .await
        .map_err(SpotWsError::from)
        .or_fail("connect");
    let mut checked = HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while let Ok(next) = tokio::time::timeout_at(deadline, socket.next()).await {
        let Some(message) = next else {
            transient("the server ended the connection");
        };
        let message = message
            .map_err(|err| SpotWsError::from(WsError::from(err)))
            .or_fail("read");
        let Message::Text(text) = message else {
            continue;
        };
        let update = Update::from_json(&text).or_fail(&text);
        assert!(!matches!(update.payload, Payload::Unknown { .. }), "{text}");
        let stream = update.stream.to_string();
        checked.insert(stream.clone());
        let wire: Value = serde_json::from_str(&text).expect("a JSON frame"); // live-unwraps: the frame decoded above
        let emitted = serde_json::to_value(&update).expect("an update serialises"); // live-unwraps: serialising test data
        let diff = common::compare_values(&stream, &wire, &emitted);
        for key in diff.unmodelled {
            // Binance documents these as "Ignore": the kline's `B`, and the
            // `M` of a trade and an aggregate trade.
            let ignored = key == "/data/k/B"
                || (key == "/data/M"
                    && matches!(update.payload, Payload::AggTrade(_) | Payload::Trade(_)));
            if !ignored {
                unmodelled.push(format!("{stream}: {key}"));
            }
        }
        // A book level has no keys, so a value appended to one is seen only
        // by its length.
        if matches!(update.payload, Payload::PartialDepth(_)) {
            for side in ["bids", "asks"] {
                let levels = wire["data"][side].as_array().expect("a book side"); // live-unwraps: an assertion on the frame
                for level in levels {
                    let n = level.as_array().expect("a book level").len(); // live-unwraps: an assertion on the frame
                    if n != LEVEL_VALUES {
                        unmodelled.push(format!(
                            "{stream}: a {side} level of {n} values, not {LEVEL_VALUES}"
                        ));
                    }
                }
            }
        }
    }
    let missing: Vec<&String> = names.iter().filter(|n| !checked.contains(*n)).collect();
    assert!(
        missing.is_empty(),
        "in 10 s these streams sent no frame to check: {missing:?}"
    );
    unmodelled.sort();
    unmodelled.dedup();
    assert!(
        unmodelled.is_empty(),
        "the host sent what the stream types do not model; model it and record it in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}

#[tokio::test]
#[ignore]
async fn live_a_client_ping_is_answered() {
    // A SUBSCRIBE, a ping, then a LIST_SUBSCRIPTIONS 500 ms after the
    // SUBSCRIBE: three messages in a second at most.
    let mut ws = SpotWs::connect([StreamName::MiniTicker(btc())])
        .await
        .or_fail("connect");
    // An unanswered ping is the fault under test, so it files whatever its
    // class; any other error, a server restart among them, fails by its class.
    let rtt = match ws.ping().await {
        Ok(rtt) => rtt,
        Err(err @ SpotWsError::NoAnswer { .. }) => panic!("pong: {err:?}"), // live-unwraps: the property under test
        Err(err) => fail("ping", &err),
    };
    assert!(rtt < Duration::from_secs(5), "{rtt:?}");
    assert_eq!(
        ws.list_subscriptions().await.or_fail("list"),
        ["btcusdt@miniTicker"]
    );
    ws.close().await.or_fail("close");
}
```

Registering the target comes first. In `polyoxide-binance/Cargo.toml`, replace

```toml
[[test]]
name = "live_spot"
required-features = ["spot"]
```

with

```toml
[[test]]
name = "live_spot"
required-features = ["spot"]

[[test]]
name = "live_spot_ws"
required-features = ["spot-ws"]
```

In `scripts/live_unwraps.baseline.json`, the opted-out sites of the new file are six (the `panic!("{finding}")`, the two `.expect(` on the frame, the two on a book side and a level, and the `panic!("pong: …")`), as in `live_usdm_ws.rs`. Replace

```json
    "polyoxide-binance/tests/live_spot.rs": 6,
```

with

```json
    "polyoxide-binance/tests/live_spot.rs": 6,
    "polyoxide-binance/tests/live_spot_ws.rs": 6,
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command python3 scripts/gen_registry.py --check`

Expected: it fails: the `live_spot_ws` target has a test file and a `[[test]]` stanza but no `[package.metadata.polyoxide.live.live_spot_ws]` entry, so the registry reports the target as unregistered (`live_spot_ws` named in the message). If your tree prints a different wording, the failure is still that the live target is not registered.

- [ ] **Step 3: Implement**

In `polyoxide-binance/Cargo.toml`:

Replace

```toml
features = ["usdm", "usdm-ws", "spot"]
```

with

```toml
features = ["usdm", "usdm-ws", "spot", "spot-ws"]
```

and replace its comment

```toml
# docs.rs documents both modules and the USDⓈ-M streams; `test-server` stays out
# of the docs.
```

with

```toml
# docs.rs documents both modules and the streams of each, `usdm-ws` and
# `spot-ws`; `test-server` stays out of the docs.
```

Replace the package `description`

```toml
description = "Rust client library for Binance USDⓈ-M futures and spot market data, and USDⓈ-M streams"
```

with

```toml
description = "Rust client library for Binance USDⓈ-M futures and spot market data and streams"
```

and, in `[package.metadata.polyoxide]`, replace

```toml
readme = "Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`) and `spot` (spot market data); re-exported by the umbrella as `polyoxide::binance`"
```

with

```toml
readme = "Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`) and `spot` (spot market data, with its streams under `spot-ws`); re-exported by the umbrella as `polyoxide::binance`"
```

These two texts are what `gen_registry.py --write` carries into the generated regions of `CLAUDE.md` and `README.md`, so they change in this commit, before the `--write` below: the regions then name `spot-ws`.

Replace

```toml
identifiers = ["fapi", "fstream", "X-MBX", "api/v3"]
```

with

```toml
identifiers = ["fapi", "fstream", "X-MBX", "api/v3", "stream.binance.com"]
```

Replace

```toml
[package.metadata.polyoxide.live.live_spot]
suite = "spot"
timeout = 15
features = ["spot"]
secrets = []
```

with

```toml
[package.metadata.polyoxide.live.live_spot]
suite = "spot"
timeout = 15
features = ["spot"]
secrets = []

[package.metadata.polyoxide.live.live_spot_ws]
suite = "spot"
timeout = 15
features = ["spot-ws"]
secrets = []
```

Replace the module docs of `polyoxide-binance/src/spot/ws/mod.rs`: its leading block of `//!` lines (from line 1 through the last consecutive line that begins with `//!`; if the file has none, prepend this) with

```rust
//! Spot market streams on `stream.binance.com`.
//!
//! Binance serves spot's streams on one combined path, `/stream`: every
//! [`StreamName`] is a name on it, the bare [`SpotWs`] holds one connection,
//! and the supervised tier ([`SpotWsBuilder`]) keeps that connection up,
//! reconnecting, replaying its streams and replacing it before Binance's
//! 24-hour cutoff.
//!
//! Binance acknowledges every `SUBSCRIBE` and enforces its connection rules by
//! closing the connection, so both tiers enforce them before sending: at most
//! [`MAX_STREAMS_PER_CONNECTION`] streams, [`MAX_NAMES_PER_REQUEST`] names per
//! request, and one request per [`MIN_REQUEST_INTERVAL`]. The host closes a
//! connection that receives more than five messages a second, requests, pings
//! and pongs counted, and bans an address it disconnects repeatedly, so the
//! pace is not a knob: a request every 250 ms was closed with 1008 "Too many
//! requests" where one every 500 ms held. The measurements are in
//! `docs/specs/binance/OBSERVED.md`.
//!
//! ```no_run
//! use futures_util::StreamExt;
//! use polyoxide_binance::spot::{types::Symbol, ws::{Event, SpotWsBuilder, StreamName}};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let btc = Symbol::new("BTCUSDT")?;
//! let mut feed = SpotWsBuilder::new()
//!     .streams([StreamName::AggTrade(btc.clone()), StreamName::BookTicker(btc)])
//!     .connect()
//!     .await?;
//! while let Some(event) = feed.next().await {
//!     if let Event::Update(update) = event? {
//!         println!("{}", update.stream);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
```

In the root `Cargo.toml`, before regenerating, the Binance mirror entry names the streams of both products. In `[workspace.metadata.polyoxide.mirrors.binance]` (`Cargo.toml:265-271`) replace

```toml
base_urls = ["https://fapi.binance.com", "https://api.binance.com", "wss://fstream.binance.com"]
description = "Futures and spot public market data, and the futures market streams. No published spec, so not a mirror"
crate_note = "streams behind `usdm-ws`"
```

with

```toml
base_urls = ["https://fapi.binance.com", "https://api.binance.com", "wss://fstream.binance.com", "wss://stream.binance.com:9443"]
description = "Futures and spot public market data, and the futures and spot market streams. No published spec, so not a mirror"
crate_note = "streams behind `usdm-ws` and `spot-ws`"
```

Leave `name`, `section` and `exclude` as they are, and do not touch `[workspace.metadata.polyoxide] stage`.

Then regenerate and look at what changed:

```
nix develop --command cargo fmt --all
nix develop --command python3 scripts/gen_registry.py --write
git status --short
```

Expected changes: `Cargo.toml`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/tests/live_spot_ws.rs`, `scripts/live_unwraps.baseline.json`, and the generated regions of `CLAUDE.md` (the nightly list gains `live_spot_ws` under `live-polyoxide-binance-spot`, the `polyoxide-binance` rows carry the new `readme` text and name `spot-ws`, and the schema-exclusions region carries the Binance mirror's new description), `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md` (the Binance row's base URLs, description and crate note), `docs/ARCHITECTURE.md` and the two nightly workflows. Do not edit any of those by hand. Then check the live-unwraps count:

```
nix develop --command python3 scripts/live_unwraps.py
```

If it reports a count for `polyoxide-binance/tests/live_spot_ws.rs` other than 6, set the baseline's number to the one it reports and say nothing else; it must report no unwrap and no bare site.

- [ ] **Step 4: Run it to see it pass**

Run, in order:

```
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command cargo test --doc -p polyoxide-binance --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'
nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
```

Each exits 0; the doctests include the three new `no_run` examples (`SpotWs`, `SupervisedSpotWs`, the module docs).

Then the live suite against production, once (it sends one `SUBSCRIBE` in the first test, none in the second, and three requests at least 500 ms apart in the third; the three connections are well under 300 per 5 minutes):

```
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws --test live_spot_ws -- --ignored
```

Expected: `test result: ok. 3 passed`. A failure prints a `polyoxide-class=<tag>` line: `transient` or `environmental` is the host, not the code; `real` with a key list from `live_frames_carry_no_unmodelled_keys` means the host sent a key the types do not model: stop and report it, since modelling it is Story 3's types and a captured fixture is Story 1's.

- [ ] **Step 5: Commit**

```
git add -N polyoxide-binance/tests/live_spot_ws.rs
git commit -m "feat(binance): spot's live stream suite, the live entry and the stream.binance.com identifier (binance spot ws, Story 4)" -- Cargo.toml polyoxide-binance/Cargo.toml polyoxide-binance/src/spot/ws/mod.rs polyoxide-binance/tests/live_spot_ws.rs scripts/live_unwraps.baseline.json CLAUDE.md README.md SELF-HEALING.md docs/specs/INDEX.md docs/ARCHITECTURE.md .github/workflows/nightly-behavioral.yml .github/workflows/nightly-schema.yml Cargo.lock
```

(`Cargo.lock` and any generated file that did not change are harmless in the pathspec; a pathspec that matches no tracked file is the only error, and each of these is tracked.)

### Task 4.4: The socket-pace row of `docs/MUTANTS.md`

**Files:**
- Modify: `docs/MUTANTS.md`
- Modify: `.github/scripts/tests/test_mutants_ledger.py`

The row cites lines, and a row that cites a line the code has moved from fails the ledger's test, so this task runs after the code is committed and computes the lines from the tree.

- [ ] **Step 1: Write the failing test**

Insert the row and its ledger snippets with one script. It reads the lines from the committed files, so it needs no editing; run it from the repo root:

```
python3 - <<'PY'
import subprocess
from datetime import date
from pathlib import Path

MOD = "polyoxide-binance/src/spot/ws/mod.rs"
CLIENT = "polyoxide-binance/src/spot/ws/client.rs"
SUITE = "polyoxide-binance/tests/supervision_spot.rs"
CONST = "pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(500);"
LIMITS = "fn the_documented_limits_are_the_measured_ones("
PACED = "async fn requests_are_paced_at_500_ms("
GAP = "async fn a_membership_change_waits_500_ms_for_the_one_before("


def line_of(path, needle):
    for number, text in enumerate(Path(path).read_text().splitlines(), 1):
        if needle in text:
            return number
    raise SystemExit(f"{needle!r} is not in {path}")


const_line = line_of(MOD, CONST)
limits_line = line_of(CLIENT, LIMITS)
paced_line = line_of(CLIENT, PACED)
gap_line = line_of(SUITE, GAP)
head = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True).strip()

row = (
    "| (m) Spot's socket requests go no faster than one per 500 ms, the pace its probe held for 45 s "
    "where one per 250 ms was closed with 1008 \"Too many requests\" "
    f"| `{MOD}:{const_line}`, `MIN_REQUEST_INTERVAL`, which `SpotStreams` hands to the shared bare "
    "connection and to the kit's `configure` "
    "| `Duration::from_millis(250)` "
    f"| `{CLIENT}:{limits_line}` `the_documented_limits_are_the_measured_ones`; "
    f"`{CLIENT}:{paced_line}` `requests_are_paced_at_500_ms`; "
    f"`{SUITE}:{gap_line}` `a_membership_change_waits_500_ms_for_the_one_before` |"
)
paragraph = (
    f"Row (m), spot's socket pace, was proved on {date.today().isoformat()} on top of `{head}`, on "
    "Rust 1.99.0 with `-j 4`: its three tests passed, then failed with the mutant in place, then passed "
    "again once the line was restored, with `git diff --exit-code` clean, through the commands in "
    "Story 4 of the Binance spot streams plan (`cargo test -p polyoxide-binance --no-default-features "
    "--features spot-ws,test-server -j 4 --lib -- the_documented_limits_are_the_measured_ones "
    "requests_are_paced_at_500_ms` and `--test supervision_spot -- "
    "a_membership_change_waits_500_ms_for_the_one_before`)."
)

ledger = Path("docs/MUTANTS.md")
lines = ledger.read_text().split("\n")
at = next(i for i, text in enumerate(lines) if text.startswith("| (l) "))
lines.insert(at + 1, row)
rules = lines.index("## The rules")
lines[rules:rules] = [paragraph, ""]
ledger.write_text("\n".join(lines))

test = Path(".github/scripts/tests/test_mutants_ledger.py")
text = test.read_text()
anchor = '    ("polyoxide-binance/src/spot/api/market.rs", 167): ".route(Route::Depth { limit: Some(limit) })",\n'
assert anchor in text, "row (l)'s snippet moved"
text = text.replace(
    anchor,
    anchor
    + f'    ("{MOD}", {const_line}): "{CONST}",\n'
    + f'    ("{CLIENT}", {limits_line}): "{LIMITS}",\n'
    + f'    ("{CLIENT}", {paced_line}): "{PACED}",\n'
    + f'    ("{SUITE}", {gap_line}): "{GAP}",\n',
)
test.write_text(text)
PY
```

Then run the ledger's test to see the rows hold: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`. It passes (every citation has a snippet, every snippet a citation, each line holds its code). The row's claim is not proved yet.

- [ ] **Step 2: Run it to see it fail**

Apply the mutation, with the file otherwise untouched, and run the row's three tests:

```
L=$(grep -n 'pub const MIN_REQUEST_INTERVAL' polyoxide-binance/src/spot/ws/mod.rs | cut -d: -f1)
sed -i "${L}s/from_millis(500)/from_millis(250)/" polyoxide-binance/src/spot/ws/mod.rs
git diff --stat -- polyoxide-binance/src/spot/ws/mod.rs
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server -j 4 --lib -- the_documented_limits_are_the_measured_ones requests_are_paced_at_500_ms
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server -j 4 --test supervision_spot -- a_membership_change_waits_500_ms_for_the_one_before
```

Expected: the `git diff --stat` shows one line changed in `mod.rs`; the lib run prints `the_documented_limits_are_the_measured_ones ... FAILED` (left `250ms`, right `500ms`) and `requests_are_paced_at_500_ms ... FAILED` (`requests 2xxms apart`); the suite run prints `a_membership_change_waits_500_ms_for_the_one_before ... FAILED`. If any of the three passes under the mutant, the row is wrong: stop and report which one.

- [ ] **Step 3: Implement**

Restore the line, from git, and check that nothing else of the file moved:

```
git checkout -- polyoxide-binance/src/spot/ws/mod.rs
git diff --exit-code -- polyoxide-binance/src/spot/ws/mod.rs
```

(`git checkout -- <file>` is safe here: `mod.rs` was committed in Task 4.3 and holds no uncommitted change; the row and the ledger snippets live in other files.)

- [ ] **Step 4: Run it to see it pass**

```
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server -j 4 --lib -- the_documented_limits_are_the_measured_ones requests_are_paced_at_500_ms
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server -j 4 --test supervision_spot -- a_membership_change_waits_500_ms_for_the_one_before
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
nix develop --command cargo fmt --all -- --check
```

Expected: the three tests pass, the ledger's test passes, and every gate exits 0.

- [ ] **Step 5: Commit**

```
git commit -m "docs(mutants): row (m), spot's socket pace, proved by its mutant (binance spot ws, Story 4)" -- docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
```

## Story 5: `polyoxide binance spot stream` and the umbrella's `binance-spot-ws`

- **Developer:** intermediate-dev
- **Files:** `polyoxide-cli/Cargo.toml`, `polyoxide-cli/src/main.rs`, `polyoxide-cli/src/commands/binance/mod.rs`, `polyoxide-cli/src/commands/binance/spot.rs`, `polyoxide-cli/tests/ws_binance.rs`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `CLAUDE.md`, `README.md`, `docs/ARCHITECTURE.md`, `Cargo.lock`
- **Interfaces:** exposes "Story 5 → Story 6"; uses Story 3's types and fixtures and Story 4's `SpotWsBuilder`, `Event` and `SpotWsError`
- **Verify:** `cargo test -p polyoxide-cli --all-features && cargo test -p polyoxide --all-features && cargo hack check -p polyoxide -p polyoxide-cli --each-feature --no-dev-deps && python3 scripts/gen_registry.py --check`
- **Review:** deep (a new CLI command and the umbrella's public surface)

Adds `polyoxide binance spot stream`, the counterpart of `binance usdm stream`, in `commands/binance/spot.rs`, and the umbrella's `binance-spot-ws`. The command is `usdm.rs`'s shape on spot's types: `SpotCommand`, `SpotArgs` with spec §5.12's kinds (`agg-trade`, `trade`, `kline-<interval>`, `mini-ticker`, `ticker`, `book-ticker`, `avg-price`, `depth5`, `depth10`, `depth20`, the depths each every 100 ms) and `--all-mini-tickers` for `!miniTicker@arr`, `run` on the supervised tier, `run_with` over any event stream, and a `Feed` whose `pretty` prints a line per update and per `!miniTicker@arr` row and whose `json` prints each frame's envelope; outage markers and a skipped frame go to stderr. Task 5.1 builds it, driven first by tests appended to `tests/ws_binance.rs` (in a module of their own, so no existing line of the file changes) over `spot::ws::fixtures`. Task 5.2 adds the umbrella's feature, `full` gaining it, the crate docs' feature list, the prelude names and the passthrough checks of spec §5.11. Each task runs `gen_registry.py --write`, because the regions of `CLAUDE.md`, `README.md` and `docs/ARCHITECTURE.md` that list crate features and needs change with the manifests, and the story gate `gen_registry.py --check` must pass after every task. No code here holds a landmine marker. `Cargo.lock` changes only if cargo says so: `spot-ws` adds no crate `usdm-ws` did not.

### Task 5.1: `polyoxide binance spot stream`

**Files:**
- Modify: `polyoxide-cli/Cargo.toml`
- Modify: `polyoxide-cli/src/main.rs`
- Modify: `polyoxide-cli/src/commands/binance/mod.rs`
- Create: `polyoxide-cli/src/commands/binance/spot.rs`
- Test: `polyoxide-cli/tests/ws_binance.rs` (append only), and the unit tests in `spot.rs`

- [ ] **Step 1: Write the failing test**

Append this module to the end of `polyoxide-cli/tests/ws_binance.rs`, after `a_trade_is_a_sell_when_the_buyer_was_the_maker`. Touch no existing line (AD-12): the module has its own imports and helpers, and reuses only the file's `HANG`, `Run`, `str_at`, `assert_run` and `frame` through `super`.

```rust

/// `polyoxide binance spot stream`, over the same scripted event streams built
/// from spot's captured frames.
mod spot_ws {
    use std::io;

    use clap::Parser;
    use futures_util::{stream, Stream, StreamExt};
    use polyoxide_binance::spot::ws::{
        fixtures, DisconnectReason, Event, SpotWsError, StreamPath, Update,
    };
    use polyoxide_cli::commands::binance::spot::{run_with, SpotArgs};
    use serde_json::Value;

    use super::{assert_run, frame, str_at, Run, HANG};

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: SpotArgs,
    }

    type Item = Result<Event, SpotWsError>;

    fn update(frame: &str) -> Item {
        Ok(Event::Update(Box::new(Update::from_json(frame).unwrap())))
    }

    /// Run over any stream, with `--all-mini-tickers` in front of `argv`.
    async fn run_stream<S>(argv: &[&str], events: S) -> Run
    where
        S: Stream<Item = Item> + Unpin,
    {
        let mut full = vec!["binance", "--all-mini-tickers"];
        full.extend_from_slice(argv);
        let cli = Cli::try_parse_from(full).unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        tokio::time::timeout(HANG, run_with(cli.args, events, &mut out, &mut err))
            .await
            .expect("the run ended")
            .unwrap();
        Run {
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    async fn run(argv: &[&str], events: Vec<Item>) -> Run {
        run_stream(argv, stream::iter(events)).await
    }

    /// The pretty lines one frame prints.
    async fn pretty(frame: &str) -> Vec<String> {
        run(&[], vec![update(frame)])
            .await
            .out
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn parse(argv: &[&str]) -> SpotArgs {
        Cli::try_parse_from(argv).unwrap().args
    }

    /// The wire frame as the client prints it: Binance documents `M` on trades
    /// and aggregate trades and `B` on the kline as "Ignore", and the client
    /// does not model them.
    fn modelled(raw: &str) -> Value {
        let mut wire = frame(raw);
        if let Some(data) = wire.get_mut("data").and_then(Value::as_object_mut) {
            data.remove("M");
        }
        if let Some(k) = wire.pointer_mut("/data/k").and_then(Value::as_object_mut) {
            k.remove("B");
        }
        wire
    }

    #[tokio::test]
    async fn json_lines_are_the_frames_envelopes() {
        // The eleven captured frames, so a loop over a shorter list cannot pass.
        assert_eq!(fixtures::ALL.len(), 11);
        let events: Vec<Item> = fixtures::ALL
            .iter()
            .map(|(_, frame)| update(frame))
            .collect();
        let run = run(&["--format", "json"], events).await;
        let lines: Vec<&str> = run.out.lines().collect();
        assert_eq!(lines.len(), fixtures::ALL.len());
        for (line, (name, frame)) in lines.iter().zip(fixtures::ALL) {
            let printed: Value = serde_json::from_str(line).unwrap();
            assert_eq!(printed, modelled(frame), "{name}");
        }
        assert!(run.err.contains("The feed ended"), "{}", run.err);
    }

    #[tokio::test]
    async fn count_stops_after_n_updates_and_markers_go_to_stderr() {
        let events = vec![
            update(fixtures::MINI_TICKER),
            Ok(Event::Disconnected {
                path: StreamPath::Stream,
                reason: DisconnectReason::Stale,
            }),
            Ok(Event::Reconnected {
                path: StreamPath::Stream,
            }),
            update(fixtures::MINI_TICKER),
            update(fixtures::MINI_TICKER),
        ];
        let run = run(&["--format", "json", "-n", "2"], events).await;
        assert_eq!(run.out.lines().count(), 2);
        assert!(
            !run.out.contains('#'),
            "markers leaked into stdout: {}",
            run.out
        );
        assert!(run.err.contains("# stream disconnected"), "{}", run.err);
        assert!(run.err.contains("# stream reconnected"), "{}", run.err);
        assert!(run.err.contains("Reached 2 update(s)"), "{}", run.err);
    }

    #[tokio::test]
    async fn pretty_prints_a_line_per_update_and_per_array_row() {
        let run = run(
            &[],
            vec![
                update(fixtures::ALL_MINI_TICKERS),
                update(fixtures::AGG_TRADE),
                update(fixtures::BOOK_TICKER),
            ],
        )
        .await;
        let lines: Vec<&str> = run.out.lines().collect();
        // The array fixture holds two rows.
        assert_eq!(lines.len(), 4, "{}", run.out);
        assert!(
            lines[0].contains("mini") && lines[1].contains("mini"),
            "{}",
            run.out
        );
        assert!(
            lines[2].starts_with("BTCUSDT") && lines[2].contains("aggtrade"),
            "{}",
            run.out
        );
        assert!(lines[3].contains("book"), "{}", run.out);
    }

    #[tokio::test]
    async fn a_bad_frame_is_skipped_on_stderr_and_a_fatal_error_ends_the_run() {
        let bad = Update::from_json(r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade"}}"#)
            .unwrap_err();
        let events = vec![Err(bad), update(fixtures::MINI_TICKER)];
        let run = run(&["--format", "json"], events).await;
        assert_eq!(run.out.lines().count(), 1);
        assert!(
            run.err
                .contains("# skipped a frame on \"btcusdt@aggTrade\""),
            "{}",
            run.err
        );

        let cli = Cli::try_parse_from(["binance", "--all-mini-tickers"]).unwrap();
        let events: Vec<Item> = vec![Err(SpotWsError::Refused {
            code: 2,
            msg: "Invalid request".into(),
        })];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let result = run_with(cli.args, stream::iter(events), &mut out, &mut err).await;
        assert!(result.unwrap_err().to_string().contains("Invalid request"));
    }

    #[tokio::test]
    async fn a_closed_reader_ends_the_run_quietly() {
        struct Closed;
        impl io::Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let cli = Cli::try_parse_from(["binance", "--all-mini-tickers"]).unwrap();
        // The feed stays open, so only the broken pipe can end this run.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]).chain(stream::pending());
        let mut err = Vec::new();
        tokio::time::timeout(HANG, run_with(cli.args, events, &mut Closed, &mut err))
            .await
            .expect("the run ended when the reader went away")
            .expect("a closed reader is not an error");
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    }

    #[tokio::test]
    async fn the_timeout_ends_a_quiet_feed() {
        // One update, then silence: only the deadline can end this run.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]).chain(stream::pending());
        let run = run_stream(&["--format", "json", "-t", "100ms"], events).await;
        assert_eq!(run.out.lines().count(), 1, "{}", run.out);
        assert!(run.err.contains("Timeout reached"), "{}", run.err);
    }

    #[tokio::test]
    async fn a_timeout_too_large_to_add_means_no_deadline() {
        // Adding this to `Instant::now()` overflows; it must mean "no
        // deadline", not a panic.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]);
        let run = run_stream(&["--format", "json", "-t", "18446744073709551615s"], events).await;
        assert_eq!(run.out.lines().count(), 1, "{}", run.out);
    }

    #[tokio::test]
    async fn count_stops_without_waiting_for_another_frame() {
        // A live feed may go quiet after the last wanted update. Reaching `-n`
        // must end the run then, not on the next frame.
        let events = stream::iter(vec![update(fixtures::MINI_TICKER)]).chain(stream::pending());
        let run = run_stream(&["--format", "json", "-n", "1"], events).await;
        assert_eq!(run.out.lines().count(), 1, "{}", run.out);
        assert!(run.err.contains("Reached 1 update(s)"), "{}", run.err);
    }

    #[tokio::test]
    async fn a_server_close_marker_names_its_code() {
        // The nightly classifier reads `closed by the server (1011 ...` in this
        // marker as a restart, so the text is pinned here.
        let run = run(
            &[],
            vec![Ok(Event::Disconnected {
                path: StreamPath::Stream,
                reason: DisconnectReason::Closed {
                    code: Some(1011),
                    reason: "Internal error".into(),
                },
            })],
        )
        .await;
        assert!(
            run.err.contains(
                "# stream disconnected: closed by the server (1011 Internal error). \
                 Its streams are stale until it reconnects."
            ),
            "{}",
            run.err
        );
    }

    #[test]
    fn a_symbol_needs_a_kind_even_with_all_mini_tickers() {
        // `--all-mini-tickers` makes the stream list non-empty, so only the
        // pairing check can notice that the symbol would be silently dropped.
        let err = parse(&["binance", "--symbol", "BTCUSDT", "--all-mini-tickers"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("go together"), "{err}");
        let err = parse(&["binance", "--kind", "ticker", "--all-mini-tickers"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("go together"), "{err}");
    }

    #[test]
    fn an_empty_list_entry_is_ignored() {
        let streams = parse(&[
            "binance",
            "--symbol",
            "BTCUSDT,,ETHUSDT",
            "--kind",
            "ticker",
        ])
        .streams()
        .unwrap();
        assert_eq!(streams.len(), 2, "{streams:?}");
        let streams = parse(&[
            "binance",
            "--symbol",
            "BTCUSDT",
            "--kind",
            "ticker,,agg-trade",
        ])
        .streams()
        .unwrap();
        assert_eq!(streams.len(), 2, "{streams:?}");
    }

    /// A trade line. `m: true` means the buyer was the maker, so the taker sold.
    #[track_caller]
    fn assert_trade(line: &str, label: &str, trade: &Value) {
        let side = if trade["m"].as_bool().unwrap() {
            "sell"
        } else {
            "buy"
        };
        assert_run(
            line,
            &[
                str_at(trade, "/s"),
                label,
                str_at(trade, "/q"),
                "@",
                str_at(trade, "/p"),
                side,
            ],
        );
    }

    /// A mini-ticker or 24-hour ticker row, single or from the array stream:
    /// `label` and the word before the close price differ, the rest is shared.
    #[track_caller]
    fn assert_summary(line: &str, label: &str, close: &str, row: &Value) {
        assert_run(
            line,
            &[
                str_at(row, "/s"),
                label,
                close,
                str_at(row, "/c"),
                "open",
                str_at(row, "/o"),
                "quote",
                "volume",
                str_at(row, "/q"),
            ],
        );
    }

    /// A partial-depth line. The frame carries no symbol, so it is the
    /// envelope's, uppercased.
    #[track_caller]
    fn assert_depth(line: &str, wire: &Value) {
        let symbol = str_at(wire, "/stream")
            .split('@')
            .next()
            .unwrap()
            .to_ascii_uppercase();
        let book = &wire["data"];
        assert_run(
            line,
            &[
                &symbol,
                "depth",
                "bid",
                str_at(book, "/bids/0/0"),
                "x",
                str_at(book, "/bids/0/1"),
                "ask",
                str_at(book, "/asks/0/0"),
                "x",
                str_at(book, "/asks/0/1"),
            ],
        );
        let levels = book["bids"]
            .as_array()
            .unwrap()
            .len()
            .max(book["asks"].as_array().unwrap().len());
        assert!(line.ends_with(&format!("({levels} levels)")), "{line}");
    }

    #[tokio::test]
    async fn pretty_prints_every_payload_kind() {
        // Every value is read from the fixture, so a re-capture does not break
        // this; the labels and the order of the fields are what is pinned.

        let wire = frame(fixtures::AGG_TRADE);
        let lines = pretty(fixtures::AGG_TRADE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_trade(&lines[0], "aggtrade", &wire["data"]);

        let wire = frame(fixtures::TRADE);
        let lines = pretty(fixtures::TRADE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_trade(&lines[0], "trade", &wire["data"]);

        let wire = frame(fixtures::KLINE);
        let lines = pretty(fixtures::KLINE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let k = &wire["data"]["k"];
        assert_run(
            &lines[0],
            &[
                str_at(&wire, "/data/s"),
                "kline",
                str_at(k, "/i"),
                "o",
                str_at(k, "/o"),
                "h",
                str_at(k, "/h"),
                "l",
                str_at(k, "/l"),
                "c",
                str_at(k, "/c"),
                "v",
                str_at(k, "/v"),
            ],
        );
        assert_eq!(
            lines[0].split_whitespace().last() == Some("closed"),
            k["x"].as_bool().unwrap(),
            "{}",
            lines[0]
        );

        let wire = frame(fixtures::MINI_TICKER);
        let lines = pretty(fixtures::MINI_TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_summary(&lines[0], "mini", "close", &wire["data"]);

        let wire = frame(fixtures::TICKER);
        let lines = pretty(fixtures::TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_summary(&lines[0], "ticker", "last", &wire["data"]);

        let wire = frame(fixtures::BOOK_TICKER);
        let lines = pretty(fixtures::BOOK_TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let book = &wire["data"];
        assert_run(
            &lines[0],
            &[
                str_at(book, "/s"),
                "book",
                str_at(book, "/b"),
                "x",
                str_at(book, "/B"),
                "/",
                str_at(book, "/a"),
                "x",
                str_at(book, "/A"),
            ],
        );

        // Both spellings of a partial depth: the bare (1000 ms) one and `@100ms`.
        for raw in [fixtures::PARTIAL_DEPTH, fixtures::PARTIAL_DEPTH_100MS] {
            let wire = frame(raw);
            let lines = pretty(raw).await;
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert_depth(&lines[0], &wire);
        }

        let wire = frame(fixtures::AVG_PRICE);
        let lines = pretty(fixtures::AVG_PRICE).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        let avg = &wire["data"];
        assert_run(
            &lines[0],
            &[
                str_at(avg, "/s"),
                "avg",
                str_at(avg, "/w"),
                "over",
                str_at(avg, "/i"),
            ],
        );

        // A symbol with non-ASCII characters prints as the frame sent it.
        let wire = frame(fixtures::NON_ASCII_MINI_TICKER);
        let lines = pretty(fixtures::NON_ASCII_MINI_TICKER).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(!str_at(&wire, "/data/s").is_ascii(), "{wire}");
        assert_summary(&lines[0], "mini", "close", &wire["data"]);

        // The array stream prints a line per row, each as its single-symbol kind.
        let wire = frame(fixtures::ALL_MINI_TICKERS);
        let rows = wire["data"].as_array().unwrap();
        let lines = pretty(fixtures::ALL_MINI_TICKERS).await;
        assert!(rows.len() >= 2, "{wire}");
        assert_eq!(lines.len(), rows.len(), "{lines:?}");
        for (line, row) in lines.iter().zip(rows) {
            assert_summary(line, "mini", "close", row);
        }
    }

    #[tokio::test]
    async fn a_trade_is_a_sell_when_the_buyer_was_the_maker() {
        // The fixtures hold one side; the other is the same frame with `m`
        // flipped.
        for (raw, label) in [
            (fixtures::AGG_TRADE, "aggtrade"),
            (fixtures::TRADE, "trade"),
        ] {
            for buyer_is_maker in [false, true] {
                let mut wire = frame(raw);
                wire["data"]["m"] = Value::Bool(buyer_is_maker);
                let lines = pretty(&wire.to_string()).await;
                assert_eq!(lines.len(), 1, "{lines:?}");
                assert_trade(&lines[0], label, &wire["data"]);
                let sold = lines[0].split_whitespace().last() == Some("sell");
                assert_eq!(sold, buyer_is_maker, "{}", lines[0]);
            }
        }
    }

    #[tokio::test]
    async fn an_unknown_event_prints_its_type() {
        // A single-symbol object whose `e` is not its stream's is
        // `Payload::Unknown`, and prints as such rather than failing the run.
        let lines =
            pretty(r#"{"stream":"btcusdt@trade","data":{"e":"surprise","s":"BTCUSDT"}}"#).await;
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("btcusdt@trade"), "{}", lines[0]);
        assert!(
            lines[0].contains("unknown event \"surprise\""),
            "{}",
            lines[0]
        );
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-cli --all-features --test ws_binance spot_ws`

Expected: the test target does not compile. `polyoxide-cli` builds `polyoxide-binance` with `usdm-ws` only and has no `commands::binance::spot`, so it prints

```
error[E0432]: unresolved import `polyoxide_binance::spot::ws`
error[E0432]: unresolved import `polyoxide_cli::commands::binance::spot`
```

and `error: could not compile `polyoxide-cli` (test "ws_binance")`.

- [ ] **Step 3: Implement**

(a) `polyoxide-cli/Cargo.toml`: turn on `spot-ws` for the binary and for the tests, and bring the CLI's description and registry readme to cover spot.

Replace

```toml
polyoxide-binance = { workspace = true, features = ["usdm-ws"] }
```

with

```toml
polyoxide-binance = { workspace = true, features = ["usdm-ws", "spot-ws"] }
```

and replace

```toml
# The captured frames, for the `binance usdm stream` tests.
polyoxide-binance = { workspace = true, features = ["usdm-ws", "test-server"] }
```

with

```toml
# The captured frames, for the `binance usdm stream` and `binance spot stream` tests.
polyoxide-binance = { workspace = true, features = ["usdm-ws", "spot-ws", "test-server"] }
```

Replace (line 9)

```toml
description = "Command-line client for the Polymarket APIs and streams, and Binance USDⓈ-M market data"
```

with

```toml
description = "Command-line client for the Polymarket APIs and streams, and Binance USDⓈ-M and spot market data"
```

and replace (in `[package.metadata.polyoxide]`, line 69)

```toml
readme = "CLI tool for querying Polymarket APIs and Binance USDⓈ-M market data"
```

with

```toml
readme = "CLI tool for querying Polymarket APIs and Binance USDⓈ-M and spot market data"
```

(b) `polyoxide-cli/src/main.rs`: the CLI's `about` and the `Binance` command's doc comment. Replace (lines 11-15)

```rust
#[command(
    version,
    about = "CLI tool for Polymarket's APIs and streams, and Binance USDⓈ-M market data",
    long_about = None
)]
```

with

```rust
#[command(
    version,
    about = "CLI tool for Polymarket's APIs and streams, and Binance USDⓈ-M and spot market data",
    long_about = None
)]
```

and replace (line 30)

```rust
    /// Binance: USDⓈ-M futures market data
```

with

```rust
    /// Binance: USDⓈ-M futures and spot market data
```

(c) `polyoxide-cli/src/commands/binance/mod.rs`: replace the whole file with

```rust
//! `polyoxide binance`: Binance's APIs, one module per product.

pub mod spot;
pub mod usdm;

use clap::Subcommand;
use color_eyre::eyre::Result;

#[derive(Subcommand)]
pub enum BinanceCommand {
    /// USDⓈ-M futures market data
    Usdm {
        #[command(subcommand)]
        command: usdm::UsdmCommand,
    },
    /// Spot market data
    Spot {
        #[command(subcommand)]
        command: spot::SpotCommand,
    },
}

impl BinanceCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Usdm { command } => command.run().await,
            Self::Spot { command } => command.run().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(subcommand)]
        command: BinanceCommand,
    }

    #[test]
    fn each_product_has_its_stream_verb() {
        let cli = Wrapper::try_parse_from([
            "test", "spot", "stream", "--symbol", "BTCUSDT", "--kind", "ticker",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            BinanceCommand::Spot {
                command: spot::SpotCommand::Stream { .. }
            }
        ));
        let cli = Wrapper::try_parse_from(["test", "usdm", "stream", "--all-tickers"]).unwrap();
        assert!(matches!(
            cli.command,
            BinanceCommand::Usdm {
                command: usdm::UsdmCommand::Stream { .. }
            }
        ));
    }
}
```

(d) Create `polyoxide-cli/src/commands/binance/spot.rs`:

```rust
//! `polyoxide binance spot stream`: stream Binance spot market data.

use std::{
    io::{self, Write},
    time::Duration,
};

use clap::{Args, Subcommand};
use color_eyre::eyre::{bail, Result};
use futures_util::Stream;
use polyoxide_binance::spot::{
    types::{Interval, Level, Symbol},
    ws::{
        AggTradeEvent, AvgPriceEvent, BookTickerEvent, DepthLevels, DepthSpeed, Event, KlineEvent,
        MiniTickerEvent, PartialDepthEvent, Payload, SpotWsBuilder, SpotWsError, StreamName,
        TickerEvent, TradeEvent, Update,
    },
};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};
use crate::commands::stream::{self, excerpt, Feed, Handled, Limits, OutputFormat};

#[derive(Subcommand)]
pub enum SpotCommand {
    /// Stream spot market data: trades, klines, tickers, depth, book tickers
    /// and average prices
    Stream {
        #[command(flatten)]
        args: SpotArgs,
    },
}

impl SpotCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Stream { args } => run(args).await,
        }
    }
}

/// The kinds a symbol can be streamed as.
const KINDS: &str = "agg-trade, avg-price, book-ticker, depth5, depth10, depth20, \
                     kline-<interval>, mini-ticker, ticker, trade";

#[derive(Args, Debug)]
pub struct SpotArgs {
    /// Symbols, comma-separated, e.g. BTCUSDT,ETHUSDT. Each `--kind` is
    /// streamed for each symbol. Letters are matched case-blind.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub symbol: Vec<String>,

    /// Kinds to stream for each symbol, comma-separated: agg-trade,
    /// avg-price, book-ticker, depth5, depth10 and depth20 (each every
    /// 100 ms), `kline-<interval>` (kline-1s, kline-1m, kline-1h, kline-1d,
    /// ...; `1M` is a month, `1m` a minute), mini-ticker, ticker, trade.
    /// Kinds are case-sensitive.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub kind: Vec<String>,

    /// Stream the mini ticker of every symbol that changed
    /// (`!miniTicker@arr`).
    #[arg(long)]
    pub all_mini_tickers: bool,

    /// Output format: `pretty` prints one line per update, and one per row of
    /// an array stream; `json` prints each frame's `{"stream", "data"}`
    /// envelope
    #[arg(short, long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,

    /// Exit after printing N updates
    #[arg(short = 'n', long)]
    pub count: Option<u64>,

    /// Exit after the given duration (e.g. "30s", "5m")
    #[arg(short, long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
}

impl SpotArgs {
    /// The streams the arguments name.
    pub fn streams(&self) -> Result<Vec<StreamName>> {
        let symbols = self
            .symbol
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| Symbol::new(s.as_str()).map_err(|e| color_eyre::eyre::eyre!("{e}")))
            .collect::<Result<Vec<_>>>()?;
        let kinds: Vec<&str> = self
            .kind
            .iter()
            .map(String::as_str)
            .filter(|k| !k.is_empty())
            .collect();
        if symbols.is_empty() != kinds.is_empty() {
            bail!("--symbol and --kind go together: each kind is streamed for each symbol");
        }
        let mut streams = Vec::new();
        if self.all_mini_tickers {
            streams.push(StreamName::AllMiniTickers);
        }
        for symbol in &symbols {
            for kind in &kinds {
                streams.push(stream(symbol.clone(), kind)?);
            }
        }
        if streams.is_empty() {
            bail!("nothing to stream: pass --symbol with --kind, or --all-mini-tickers");
        }
        Ok(streams)
    }
}

/// One `--kind` for one symbol.
fn stream(symbol: Symbol, kind: &str) -> Result<StreamName> {
    Ok(match kind {
        "agg-trade" => StreamName::AggTrade(symbol),
        "avg-price" => StreamName::AvgPrice(symbol),
        "book-ticker" => StreamName::BookTicker(symbol),
        "depth5" => StreamName::PartialDepth(symbol, DepthLevels::Five, DepthSpeed::Ms100),
        "depth10" => StreamName::PartialDepth(symbol, DepthLevels::Ten, DepthSpeed::Ms100),
        "depth20" => StreamName::PartialDepth(symbol, DepthLevels::Twenty, DepthSpeed::Ms100),
        "mini-ticker" => StreamName::MiniTicker(symbol),
        "ticker" => StreamName::Ticker(symbol),
        "trade" => StreamName::Trade(symbol),
        _ => match kind
            .strip_prefix("kline-")
            .and_then(|i| i.parse::<Interval>().ok())
        {
            Some(interval) => StreamName::Kline(symbol, interval),
            // In a block, as a statement: Rust 1.99 rejects `bail!`, which
            // expands to `return Err(..);`, as a match arm's value.
            None => {
                bail!(
                    "unknown kind {kind:?}; expected one of {KINDS}, where <interval> is one of {}",
                    Interval::ALL
                        .iter()
                        .map(|i| i.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
        },
    })
}

/// Connect to the production host and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(args: SpotArgs) -> Result<()> {
    let streams = args.streams()?;
    let names: Vec<String> = streams.iter().map(ToString::to_string).collect();
    let listed = if names.len() <= 10 {
        names.join(", ")
    } else {
        format!("{} streams", names.len())
    };
    eprintln!("Connecting to Binance spot streams: {listed}");
    let mut feed = SpotWsBuilder::new().streams(streams).connect().await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
    // As in `binance usdm stream`, there is no Ctrl+C handler: the process
    // ends with the signal. On `-n`, `-t` or the end of the feed the
    // connection is closed with a handshake.
    let result = run_with(
        args,
        &mut feed,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await;
    let _ = feed.close().await;
    result
}

/// Print events from any stream until `-n` or `-t` is reached or the stream
/// ends.
///
/// Takes the stream rather than connecting, so tests can drive every flag with
/// a scripted list of events. Updates go to `out`; outage markers and skipped
/// frames go to `err`, so JSON output stays clean JSONL.
pub async fn run_with<S>(
    args: SpotArgs,
    events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<Event, SpotWsError>> + Unpin,
{
    let limits = Limits {
        count: args.count,
        timeout: args.timeout,
    };
    stream::run(limits, events, &mut Spot(args.format), out, err).await
}

/// The streams' half of a run: what each event means, printed in a format.
struct Spot(OutputFormat);

impl Feed for Spot {
    type Event = Event;
    type Error = SpotWsError;

    fn event(&mut self, event: Event, out: &mut dyn Write, err: &mut dyn Write) -> Result<Handled> {
        match event {
            Event::Update(update) => {
                print_update(&update, self.0, out)?;
                return Ok(Handled::Printed);
            }
            Event::Disconnected { path, reason } => writeln!(
                err,
                "# {path} disconnected: {reason}. Its streams are stale until it reconnects."
            )?,
            Event::Reconnected { path } => writeln!(
                err,
                "# {path} reconnected. Rebuild anything built from its streams."
            )?,
            // `Event` is #[non_exhaustive]; a future variant is not a fault.
            _ => {}
        }
        Ok(Handled::Skipped)
    }

    fn error(&mut self, error: SpotWsError, err: &mut dyn Write) -> Result<()> {
        match error {
            SpotWsError::Frame {
                stream,
                raw,
                reason,
            } => {
                writeln!(
                    err,
                    "# skipped a frame on {stream:?} that did not decode ({reason}): {}",
                    excerpt(&raw)
                )?;
                Ok(())
            }
            error => Err(error.into()),
        }
    }
}

/// Print one update and flush it.
fn print_update(update: &Update, format: OutputFormat, out: &mut dyn Write) -> io::Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(update)?)?,
        OutputFormat::Pretty => match &update.payload {
            Payload::MiniTickers(rows) => {
                for row in rows {
                    writeln!(out, "{}", mini_ticker(row))?;
                }
            }
            Payload::AggTrade(event) => writeln!(out, "{}", agg_trade(event))?,
            Payload::Trade(event) => writeln!(out, "{}", trade(event))?,
            Payload::Kline(event) => writeln!(out, "{}", kline(event))?,
            Payload::MiniTicker(event) => writeln!(out, "{}", mini_ticker(event))?,
            Payload::Ticker(event) => writeln!(out, "{}", ticker(event))?,
            Payload::BookTicker(event) => writeln!(out, "{}", book_ticker(event))?,
            Payload::PartialDepth(event) => writeln!(out, "{}", depth(event))?,
            Payload::AvgPrice(event) => writeln!(out, "{}", avg_price(event))?,
            Payload::Unknown { event_type, .. } => writeln!(
                out,
                "{:<16} unknown event {event_type:?}",
                update.stream.to_string()
            )?,
            _ => writeln!(out, "{}", update.stream)?,
        },
    }
    out.flush()
}

fn agg_trade(t: &AggTradeEvent) -> String {
    let side = if t.is_buyer_maker { "sell" } else { "buy" };
    format!(
        "{:<16} aggtrade {} @ {} {side}",
        t.symbol, t.quantity, t.price
    )
}

fn trade(t: &TradeEvent) -> String {
    let side = if t.is_buyer_maker { "sell" } else { "buy" };
    format!(
        "{:<16} trade   {} @ {} {side}",
        t.symbol, t.quantity, t.price
    )
}

fn kline(k: &KlineEvent) -> String {
    let bar = &k.kline;
    let closed = if bar.is_closed { " closed" } else { "" };
    format!(
        "{:<16} kline   {} o {} h {} l {} c {} v {}{closed}",
        k.symbol, bar.interval, bar.open, bar.high, bar.low, bar.close, bar.volume
    )
}

fn mini_ticker(t: &MiniTickerEvent) -> String {
    format!(
        "{:<16} mini    close {} open {} quote volume {}",
        t.symbol, t.close_price, t.open_price, t.quote_volume
    )
}

fn ticker(t: &TickerEvent) -> String {
    format!(
        "{:<16} ticker  last {} open {} quote volume {}",
        t.symbol, t.last_price, t.open_price, t.quote_volume
    )
}

fn book_ticker(b: &BookTickerEvent) -> String {
    format!(
        "{:<16} book    {} x {} / {} x {}",
        b.symbol, b.bid_price, b.bid_quantity, b.ask_price, b.ask_quantity
    )
}

fn depth(d: &PartialDepthEvent) -> String {
    let level = |side: &[Level]| match side.first() {
        Some(l) => format!("{} x {}", l.price, l.quantity),
        None => "-".to_owned(),
    };
    format!(
        "{:<16} depth   bid {} ask {} ({} levels)",
        d.symbol,
        level(&d.bids),
        level(&d.asks),
        d.bids.len().max(d.asks.len())
    )
}

fn avg_price(a: &AvgPriceEvent) -> String {
    format!("{:<16} avg     {} over {}", a.symbol, a.price, a.interval)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: SpotArgs,
    }

    fn parse(argv: &[&str]) -> SpotArgs {
        Wrapper::try_parse_from(argv).unwrap().args
    }

    fn names(args: &SpotArgs) -> Vec<String> {
        args.streams()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[derive(Parser)]
    struct Tree {
        #[command(subcommand)]
        command: SpotCommand,
    }

    #[test]
    fn stream_is_spots_one_verb() {
        let tree = Tree::try_parse_from(["test", "stream", "--all-mini-tickers", "-n", "1"])
            .unwrap();
        let SpotCommand::Stream { args } = tree.command;
        assert!(args.all_mini_tickers);
        assert_eq!(args.count, Some(1));
        assert!(Tree::try_parse_from(["test", "trades"]).is_err());
    }

    #[test]
    fn symbols_and_kinds_split_on_commas_and_multiply() {
        let args = parse(&[
            "test",
            "--symbol",
            "btcusdt, 币安人生USDT",
            "--kind",
            "agg-trade,kline-1h",
        ]);
        assert_eq!(
            names(&args),
            [
                "btcusdt@aggTrade",
                "btcusdt@kline_1h",
                "币安人生usdt@aggTrade",
                "币安人生usdt@kline_1h"
            ]
        );
    }

    #[test]
    fn every_kind_parses_and_depth_streams_at_100ms() {
        let args = parse(&[
            "test",
            "--symbol",
            "BTCUSDT",
            "--kind",
            "agg-trade,avg-price,book-ticker,depth5,depth10,depth20,kline-1s,kline-1M,\
             mini-ticker,ticker,trade",
            "--all-mini-tickers",
        ]);
        assert_eq!(
            names(&args),
            [
                "!miniTicker@arr",
                "btcusdt@aggTrade",
                "btcusdt@avgPrice",
                "btcusdt@bookTicker",
                "btcusdt@depth5@100ms",
                "btcusdt@depth10@100ms",
                "btcusdt@depth20@100ms",
                "btcusdt@kline_1s",
                "btcusdt@kline_1M",
                "btcusdt@miniTicker",
                "btcusdt@ticker",
                "btcusdt@trade",
            ]
        );
    }

    #[test]
    fn a_bad_kind_names_the_valid_ones() {
        let err = parse(&["test", "--symbol", "BTCUSDT", "--kind", "trades"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("\"trades\"") && err.contains("avg-price") && err.contains("1M"),
            "{err}"
        );
        // Spot's interval list is its own: `kline-2m` is no interval of it.
        let err = parse(&["test", "--symbol", "BTCUSDT", "--kind", "kline-2m"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("kline-2m"), "{err}");
        // The USDⓈ-M kinds spot has no stream for.
        for kind in ["mark-price", "funding"] {
            assert!(
                parse(&["test", "--symbol", "BTCUSDT", "--kind", kind])
                    .streams()
                    .is_err(),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_symbol_needs_a_kind_and_something_must_be_streamed() {
        assert!(parse(&["test", "--symbol", "BTCUSDT"]).streams().is_err());
        assert!(parse(&["test", "--kind", "ticker"]).streams().is_err());
        assert!(parse(&["test"])
            .streams()
            .unwrap_err()
            .to_string()
            .contains("nothing to stream"));
        assert!(parse(&["test", "--symbol", "BTC USDT", "--kind", "ticker"])
            .streams()
            .is_err());
    }

    #[test]
    fn count_timeout_and_format_parse() {
        let args = parse(&[
            "test",
            "--all-mini-tickers",
            "-n",
            "3",
            "-t",
            "5m",
            "--format",
            "json",
        ]);
        assert_eq!(args.count, Some(3));
        assert_eq!(args.timeout, Some(Duration::from_secs(300)));
        assert_eq!(args.format, OutputFormat::Json);
    }
}
```

(e) Regenerate the files that list the CLI's needs and texts (after the manifest edits of (a)): `nix develop --command python3 scripts/gen_registry.py --write`, then `git status --short`. Expect `CLAUDE.md` (the CLI's `needs` line now reads `polyoxide-binance` with `usdm-ws` and `spot-ws`) and, if the tool says so, `README.md` and `docs/ARCHITECTURE.md`; none is edited by hand. Expect the same regions to carry the new `readme` text. `polyoxide-cli/README.md` is Story 6's; do not touch it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-cli --all-features --test ws_binance`

Expected: `test result: ok. 27 passed; 0 failed` (the 13 existing usdm tests and the 14 `spot_ws::` ones).

Run: `nix develop --command cargo test -p polyoxide-cli --all-features`

Expected: every suite `ok`, including `commands::binance::spot::tests` (7 tests) and `commands::binance::tests::each_product_has_its_stream_verb`.

Run: `nix develop --command cargo fmt --all -- --check`, then `nix develop --command cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings`, then `nix develop --command python3 scripts/gen_registry.py --check`

Expected: all three exit 0 with no output beyond cargo's progress.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-cli/src/commands/binance/spot.rs
git commit -m "feat(cli): polyoxide binance spot stream (binance spot ws, Story 5)" -- polyoxide-cli/Cargo.toml polyoxide-cli/src/main.rs polyoxide-cli/src/commands/binance/mod.rs polyoxide-cli/src/commands/binance/spot.rs polyoxide-cli/tests/ws_binance.rs CLAUDE.md Cargo.lock
```

Add to the pathspec every other file `git status --short` listed after the regeneration in Step 3 (`README.md`, `docs/ARCHITECTURE.md`), and leave out `Cargo.lock` if cargo did not change it.

### Task 5.2: The umbrella's `binance-spot-ws`

**Files:**
- Modify: `polyoxide/Cargo.toml`
- Modify: `polyoxide/src/lib.rs`

- [ ] **Step 1: Write the failing test**

The umbrella's checks are `const _` items outside `cfg(test)`, which `cargo hack --each-feature` compiles with one feature alone. In `polyoxide/src/lib.rs`, add two after the `binance-spot` pair of each block.

After

```rust
#[cfg(feature = "binance-spot")]
const _: fn(crate::binance::spot::Spot) = drop;
```

add

```rust
#[cfg(feature = "binance-spot-ws")]
const _: fn(crate::binance::spot::ws::SpotWs) = drop;
```

and after

```rust
#[cfg(feature = "binance-spot")]
const _: fn(crate::prelude::Spot) = drop;
```

add

```rust
#[cfg(feature = "binance-spot-ws")]
const _: fn(crate::prelude::SpotWs) = drop;
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo check -p polyoxide --no-default-features --features binance-spot-ws`

Expected: `error: the package 'polyoxide' does not contain this feature: binance-spot-ws`.

- [ ] **Step 3: Implement**

(a) `polyoxide/Cargo.toml`: after

```toml
binance-spot = ["dep:polyoxide-binance", "polyoxide-binance/spot", "polyoxide-venue/market"]
```

add

```toml
binance-spot-ws = ["binance-spot", "polyoxide-binance/spot-ws"]
```

and in `full`, after `"binance-spot",` add `"binance-spot-ws",`:

```toml
    "binance-usdm-ws",
    "binance-spot",
    "binance-spot-ws",
    "kalshi",
```

(b) `polyoxide/src/lib.rs`, the crate docs. Replace

```rust
//! - `polyoxide::binance` is `polyoxide-binance`, under `binance` (its
//!   defaults: `usdm` and `spot`), `binance-usdm`, `binance-usdm-ws` or
//!   `binance-spot`.
```

with

```rust
//! - `polyoxide::binance` is `polyoxide-binance`, under `binance` (its
//!   defaults: `usdm` and `spot`), `binance-usdm`, `binance-usdm-ws`,
//!   `binance-spot` or `binance-spot-ws`.
```

(c) The re-export's `cfg`. Replace

```rust
#[cfg(any(
    feature = "binance",
    feature = "binance-usdm",
    feature = "binance-usdm-ws",
    feature = "binance-spot"
))]
pub use polyoxide_binance as binance;
```

with

```rust
#[cfg(any(
    feature = "binance",
    feature = "binance-usdm",
    feature = "binance-usdm-ws",
    feature = "binance-spot",
    feature = "binance-spot-ws"
))]
pub use polyoxide_binance as binance;
```

(d) The prelude. After

```rust
    // Aliased: usdm's `WeightBudget` keeps the bare name.
    #[cfg(feature = "binance-spot")]
    pub use polyoxide_binance::spot::{
        Spot, SpotBuilder, SpotError, WeightBudget as SpotWeightBudget,
    };
```

add

```rust

    // Aliased as usdm's and the other venues' streams are: usdm's `StreamName`
    // keeps the bare name, and a bare `Event` names no venue.
    #[cfg(feature = "binance-spot-ws")]
    pub use polyoxide_binance::spot::ws::{
        Event as SpotEvent, SpotWs, SpotWsBuilder, SpotWsError, StreamName as SpotStreamName,
    };
```

(e) Regenerate: `nix develop --command python3 scripts/gen_registry.py --write`, then `git status --short`. Expect `CLAUDE.md`'s umbrella feature list to gain `binance-spot-ws` (`polyoxide-binance/spot-ws`) and `full` to read as before; no other hand edit.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo check -p polyoxide --no-default-features --features binance-spot-ws`

Expected: `Finished` with no error: the two `const _` items resolve `crate::binance::spot::ws::SpotWs` and `crate::prelude::SpotWs`.

Run: `nix develop --command cargo hack check -p polyoxide -p polyoxide-cli --each-feature --no-dev-deps`

Expected: every feature set checks, `binance-spot-ws` alone included.

Run: `nix develop --command cargo test -p polyoxide --all-features`

Expected: `test result: ok` for every suite and doctest.

Run: `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy -p polyoxide --all-targets --all-features -- -D warnings`, `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide -p polyoxide-cli --all-features'` and `nix develop --command python3 scripts/gen_registry.py --check`

Expected: all exit 0.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polyoxide): binance-spot-ws, with its prelude names and passthrough checks (binance spot ws, Story 5)" -- polyoxide/Cargo.toml polyoxide/src/lib.rs CLAUDE.md Cargo.lock
```

Add to the pathspec every other file `git status --short` listed after the regeneration in Step 3, and leave out `Cargo.lock` if cargo did not change it.

## Story 6: Docs

- **Developer:** intermediate-dev
- **Files:** `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `polyoxide-cli/README.md`, `README.md`, `CLAUDE.md`, `.claude/rules/binance.md`, `.claude/rules/sockets.md`, `.claude/rules/errors.md`, `.claude/rules/cli.md`, `.claude/rules/umbrella.md`, `.claude/rules/live-tests.md`
- **Interfaces:** uses every earlier story's names; exposes none
- **Verify:** `cargo test --doc -p polyoxide-binance --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q' && bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features'`
- **Review:** light (docs and rules; no library code)

This story writes down what Stories 1 to 5 built, and edits no code but one crate-doc comment. Task 6.1 does the reader-facing docs: `docs/specs/binance/INDEX.md` gains spot's streams (the host row, a `## Spot streams covered` table and `live_spot_ws`'s drift detector), the module README gains a prose-only streams section (its `rust` fences compile under `spot` alone, so no fence is added), and `spot-ws` and `binance-spot-ws` reach the crate README, the crate docs, the umbrella README and the root README, and the CLI's spot command reaches `polyoxide-cli/README.md` (its opening sentence, a command-table row and a spot stream section). Task 6.2 does the rules: `.claude/rules/binance.md` gains `shared::ws` in its `shared` section and a Streams (`spot-ws`) section (its `paths:` globs already cover the spot socket and CLI files); `sockets.md` says Binance's protocol is `shared::ws`'s for both products; `errors.md` lists `SpotWsError::Connect`; `cli.md` gains `binance spot stream`; `umbrella.md` gains `binance-spot-ws` and its prelude names; `live-tests.md`'s wrap-first constructors gain `SpotWsError::from(WsError::from(..))`, and `CLAUDE.md`'s hand-written lines say the same (the generated regions are regenerated by Stories 4 and 5, never edited here). No landmine markers are in this story's text.

Every edit below quotes the text it replaces, and each task's check is a script that fails until the new text is there. The commands that the rules and READMEs name (`binance spot stream`, `--all-mini-tickers`, `-n`) and the names (`SpotWs`, `SpotWsBuilder`, `SupervisedSpotWs`, `SpotWsError`, `StreamName`, `StreamPath`, `StopCause`, `MAX_STREAMS_PER_CONNECTION`, `MAX_NAMES_PER_REQUEST`, `MIN_REQUEST_INTERVAL`, `SPOT_WS_BASE`, `shared::ws`, `BinanceProtocol`) are the names contract's. Run from the repo root.

### Task 6.1: Spec index, READMEs and crate docs

**Files:**
- Modify: `docs/specs/binance/INDEX.md`
- Modify: `polyoxide-binance/README.md`
- Modify: `polyoxide-binance/src/spot/README.md`
- Modify: `polyoxide-binance/src/lib.rs` (the crate docs at the top)
- Modify: `polyoxide/README.md`
- Modify: `polyoxide-cli/README.md`
- Modify: `README.md`

- [ ] **Step 1: Write the failing check**

```bash
python3 - <<'EOF'
import pathlib

checks = [
    ("docs/specs/binance/INDEX.md", "## Spot streams covered", True),
    ("docs/specs/binance/INDEX.md", "wss://stream.binance.com:9443/stream", True),
    ("docs/specs/binance/INDEX.md", "`live_spot_ws.rs::live_frames_carry_no_unmodelled_keys`", True),
    ("docs/specs/binance/INDEX.md", "scripts/capture_binance_spot.py ws", True),
    ("docs/specs/binance/INDEX.md", "`binance-spot-ws`", True),
    ("docs/specs/binance/INDEX.md", "Spot has no streams yet.", False),
    ("polyoxide-binance/README.md", "`binance-spot-ws`", True),
    ("polyoxide-binance/README.md", "`spot::ws::fixtures`", True),
    ("polyoxide-binance/src/spot/README.md", "## Streams", True),
    ("polyoxide-binance/src/spot/README.md", "`spot-ws`", True),
    ("polyoxide-binance/src/lib.rs", "with `spot-ws` its", True),
    ("polyoxide/README.md", "| `binance-spot-ws` |", True),
    ("polyoxide/README.md", "--features binance-spot-ws", True),
    ("polyoxide-cli/README.md", "and Binance USDⓈ-M and spot market data.", True),
    ("polyoxide-cli/README.md", "| `binance spot`", True),
    ("polyoxide-cli/README.md", "#### `binance spot stream`", True),
    ("polyoxide-cli/README.md", "--all-mini-tickers", True),
    ("polyoxide-cli/README.md", "and Binance USDⓈ-M market data.", False),
    ("README.md", "--features binance-spot-ws", True),
    ("README.md", "polyoxide binance spot stream", True),
]
bad = []
for path, needle, present in checks:
    if (needle in pathlib.Path(path).read_text(encoding="utf-8")) != present:
        bad.append((path, needle, "missing" if present else "still there"))
for item in bad:
    print(item)
raise SystemExit(1 if bad else 0)
EOF
```

- [ ] **Step 2: Run it to see it fail**

Run the block. Expected: it prints all twenty tuples (eighteen `missing`, and `("docs/specs/binance/INDEX.md", "Spot has no streams yet.", 'still there')` and `("polyoxide-cli/README.md", "and Binance USDⓈ-M market data.", 'still there')`) and exits 1.

- [ ] **Step 3: Implement**

(a) `docs/specs/binance/INDEX.md`. Replace

```
`polyoxide::binance`, under its `binance-usdm`, `binance-usdm-ws` and `binance-spot` features
and `full`.
```

with

```
`polyoxide::binance`, under its `binance-usdm`, `binance-usdm-ws`, `binance-spot` and
`binance-spot-ws` features and `full`.
```

Replace

```
| Spot REST, public market data | `https://api.binance.com` | `polyoxide-binance` with the `spot` feature (`spot::Spot`) |

Spot has no streams yet.
```

with

```
| Spot REST, public market data | `https://api.binance.com` | `polyoxide-binance` with the `spot` feature (`spot::Spot`) |
| Spot market streams | `wss://stream.binance.com:9443/stream` | `polyoxide-binance` with the `spot-ws` feature (`spot::ws::SpotWs`, `spot::ws::SpotWsBuilder`) |
```

Replace

```
`live_usdm_ws.rs::live_frames_carry_no_unmodelled_keys` and
`live_spot.rs::live_responses_carry_no_unmodelled_keys` fail on any key the types do not model.
```

with

```
`live_usdm_ws.rs::live_frames_carry_no_unmodelled_keys`,
`live_spot.rs::live_responses_carry_no_unmodelled_keys` and
`live_spot_ws.rs::live_frames_carry_no_unmodelled_keys` fail on any key the types do not model.
```

Insert, before the line `## Fixtures and probes`:

```
## Spot streams covered

All nine are combined streams on the one path, `wss://stream.binance.com:9443/stream`, and a
symbol in a stream's name is lowercase.

| Stream | `StreamName` | `Payload` |
|---|---|---|
| `<s>@aggTrade` | `AggTrade(symbol)` | `AggTrade(AggTradeEvent)` |
| `<s>@trade` | `Trade(symbol)` | `Trade(TradeEvent)` |
| `<s>@kline_<interval>` | `Kline(symbol, interval)` | `Kline(KlineEvent)` |
| `<s>@miniTicker` | `MiniTicker(symbol)` | `MiniTicker(MiniTickerEvent)` |
| `!miniTicker@arr` | `AllMiniTickers` | `MiniTickers(Vec<MiniTickerEvent>)` |
| `<s>@ticker` | `Ticker(symbol)` | `Ticker(TickerEvent)` |
| `<s>@bookTicker` | `BookTicker(symbol)` | `BookTicker(BookTickerEvent)` |
| `<s>@depth<5\|10\|20>`, bare (1000 ms) or `@100ms` | `PartialDepth(symbol, levels, speed)` | `PartialDepth(PartialDepthEvent)` |
| `<s>@avgPrice` | `AvgPrice(symbol)` | `AvgPrice(AvgPriceEvent)` |

The client holds the limits `OBSERVED.md`'s `## Spot` measured, from `### Stream limits` on: at
most 1024 streams per connection, 1024 names in one request and one request per 500 ms (a request
every 250 ms was closed with 1008 "Too many requests"). Binance acknowledges every `SUBSCRIBE`,
an unknown symbol and an uppercase one included, so `StreamName` is the only way to name a
stream. `!ticker@arr` is acknowledged and delivers nothing, so it has no `StreamName`. A
partial-depth frame and a `bookTicker` frame carry no event type, and partial depth carries no
symbol, so both take their stream from the combined envelope. The keys the pages document as
"Ignore", `M` on `trade` and `aggTrade` and `B` in the kline, are not modelled.

```

Replace

```
  `python3 scripts/capture_binance_spot.py rest`, with each file's request and trimming in
  `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`.
```

with

```
  `python3 scripts/capture_binance_spot.py rest`, with each file's request and trimming in
  `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`.
- `polyoxide-binance/tests/fixtures/spot/ws/`: stream envelopes, refreshed by
  `uv run scripts/capture_binance_spot.py ws` (one connection, one `SUBSCRIBE`), with each file's
  stream and trimming in the same `PROVENANCE.md`, and compiled into
  `polyoxide_binance::spot::ws::fixtures` under `spot-ws` and `test-server`.
```

Replace

```
  `probes/probe_rest.py --product {usdm,spot}` measures the REST weights and limits of either
  product (`usdm` is the default).
```

with

```
  `probes/probe_rest.py --product {usdm,spot}` measures the REST weights and limits of either
  product (`usdm` is the default).
- `probes/probe_ws.py <wsprobe_dir> --product spot` measured spot's stream acknowledgements,
  limits, pings, frames and pace, and `OBSERVED.md`'s `## Spot` holds the results from
  `### Stream limits` on. Its `pace` section at 250 ms is disconnected by the server: never run it
  again.
```

(b) `polyoxide-binance/README.md`. Replace

```
re-exports it as `polyoxide::binance` under its `binance`, `binance-usdm`, `binance-usdm-ws` and
`binance-spot` features.
```

with

```
re-exports it as `polyoxide::binance` under its `binance`, `binance-usdm`, `binance-usdm-ws`,
`binance-spot` and `binance-spot-ws` features.
```

Replace

```
| [`spot`](src/spot/README.md) | `spot` | Spot public market data on `api.binance.com`; read-only, no credentials |
```

with

```
| [`spot`](src/spot/README.md) | `spot`; its market streams, `spot::ws`, under `spot-ws` | Spot public market data on `api.binance.com`, and its market streams on `stream.binance.com`; read-only, no credentials |
```

Replace

```
`usdm` and `spot` are on by default, and `usdm-ws` turns `usdm` on. `test-server` exposes the
captured stream frames, `usdm::ws::fixtures`, to integration tests with `usdm-ws`; it is not for
consumers.
```

with

```
`usdm` and `spot` are on by default, and `usdm-ws` turns `usdm` on as `spot-ws` turns `spot` on.
`test-server` exposes the captured stream frames, `usdm::ws::fixtures` with `usdm-ws` and
`spot::ws::fixtures` with `spot-ws`, to integration tests; it is not for consumers.
```

(c) `polyoxide-binance/src/spot/README.md`. Add a section at the end of the file, after the last line ``The `spot::venue` module's documentation lists each method's route and weight.``:

```

## Streams

With the `spot-ws` feature, which turns `spot` on, `spot::ws` streams spot's nine market streams
from `stream.binance.com`: aggregate trades, trades, klines, the mini ticker of one symbol and of
every symbol, the ticker, the book ticker, partial depth (5, 10 or 20 levels, every second or
every 100 ms) and the average price. `SpotWs` is one connection, and `SpotWsBuilder` builds a
`SupervisedSpotWs`, which reconnects, replays its streams and rotates before Binance's 24-hour
cutoff. `StreamName` is the only way to name a stream, because Binance acknowledges every
subscription, including a name that will never deliver. The client holds Binance's limits itself,
since the venue enforces them by closing the connection: at most 1024 streams per connection and
one request every 500 ms. Its documentation carries the examples, because this README compiles
under `spot` alone.
```

(d) `polyoxide-binance/src/lib.rs`. Replace

```
//! `usdm` (feature `usdm`, on by default) reads USDⓈ-M futures public market
//! data (`fapi.binance.com`), and with `usdm-ws` its market streams
//! (`fstream.binance.com`). `spot` (feature `spot`, on by default) reads spot
//! public market data (`api.binance.com`). No credentials are needed. Each
//! module's own docs carry its example.
```

with

```
//! `usdm` (feature `usdm`, on by default) reads USDⓈ-M futures public market
//! data (`fapi.binance.com`), and with `usdm-ws` its market streams
//! (`fstream.binance.com`). `spot` (feature `spot`, on by default) reads spot
//! public market data (`api.binance.com`), and with `spot-ws` its market
//! streams (`stream.binance.com`). No credentials are needed. Each module's
//! own docs carry its example.
```

(e) `polyoxide/README.md`. Replace

```
| `polyoxide::binance` | `polyoxide-binance`: Binance's USDⓈ-M futures market data and streams, and its spot market data | `binance` and `binance-<module>` |
```

with

```
| `polyoxide::binance` | `polyoxide-binance`: Binance's USDⓈ-M futures and spot market data, and the market streams of both | `binance` and `binance-<module>` |
```

Replace

```
# Binance spot market data
cargo add polyoxide --no-default-features --features binance-spot
```

with

```
# Binance spot market data
cargo add polyoxide --no-default-features --features binance-spot

# Binance spot market data and its streams
cargo add polyoxide --no-default-features --features binance-spot-ws
```

Replace

```
| `binance-spot` | no | Binance spot market data via `polyoxide-binance`'s `spot` module |
```

with

```
| `binance-spot` | no | Binance spot market data via `polyoxide-binance`'s `spot` module |
| `binance-spot-ws` | no | Binance spot market streams via `polyoxide-binance`'s `spot-ws` feature |
```

(f) `polyoxide-cli/README.md`. Replace (line 3)

```
CLI tool for Polymarket's APIs and streams, and Binance USDⓈ-M market data. Provides read-only access to market data (Gamma), user and trade data (Data), the CLOB's price history, real-time WebSocket streams, and credentials in the OS keychain.
```

with

```
CLI tool for Polymarket's APIs and streams, and Binance USDⓈ-M and spot market data. Provides read-only access to market data (Gamma), user and trade data (Data), the CLOB's price history, real-time WebSocket streams, and credentials in the OS keychain.
```

Replace (line 41, in the command table)

```
| `binance usdm`                    | Stream Binance USDⓈ-M futures market data                         |
```

with

```
| `binance usdm`                    | Stream Binance USDⓈ-M futures market data                         |
| `binance spot`                    | Stream Binance spot market data                                   |
```

Replace (the end of the `binance usdm stream` section, from line 385)

```
Kinds: `agg-trade`, `book-ticker`, `depth5`, `depth10`, `depth20`, `kline-<interval>`
(`1m` to `1M`), `mark-price`, `ticker`.

---
```

with

```
Kinds: `agg-trade`, `book-ticker`, `depth5`, `depth10`, `depth20`, `kline-<interval>`
(`1m` to `1M`), `mark-price`, `ticker`.

#### `binance spot stream`

Binance spot market data, from `stream.binance.com`. No credentials needed. It takes `usdm`'s
flags: each `--kind` is streamed for each `--symbol`; connection notices go to stderr, and
`--format json` prints each frame's `{"stream", "data"}` envelope. `--all-mini-tickers` streams
every symbol's mini ticker (`!miniTicker@arr`) in the place of `usdm`'s two array flags.

```bash
# Every symbol's mini ticker, each second
polyoxide binance spot stream --all-mini-tickers

# Trades and hourly klines for two symbols
polyoxide binance spot stream --symbol BTCUSDT,ETHUSDT --kind trade,kline-1h

# Top 20 book levels every 100 ms, as JSON envelopes, stop after 10 updates
polyoxide binance spot stream --symbol BTCUSDT --kind depth20 --format json -n 10
```

Kinds: `agg-trade`, `trade`, `kline-<interval>` (`1m` to `1M`), `mini-ticker`, `ticker`,
`book-ticker`, `avg-price`, `depth5`, `depth10`, `depth20` (each depth every 100 ms).

---
```

(g) `README.md` (the root; outside its generated `readme-crates` region). Replace

```
# Binance spot market data
cargo add polyoxide --no-default-features --features binance-spot
```

with

```
# Binance spot market data
cargo add polyoxide --no-default-features --features binance-spot

# Binance spot market data and its streams
cargo add polyoxide --no-default-features --features binance-spot-ws
```

Replace

```
polyoxide binance usdm stream --all-mark-prices -n 10
```

with

```
polyoxide binance usdm stream --all-mark-prices -n 10
polyoxide binance spot stream --all-mini-tickers -n 10
```

- [ ] **Step 4: Run it to see it pass**

Run the check from Step 1 again. Expected: no output, exit 0. Then the story's doc gates for these files:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo test --doc -p polyoxide-binance --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'
python3 scripts/gen_registry.py --check
bash -c 'cd .github/scripts && uv run pytest tests/test_module_readmes.py -q'
```

Expected: the doctests pass (the module READMEs are doctests, and `spot/README.md` gained no fence), the doc build has no warning, `gen_registry.py --check` reports nothing to rewrite (no edit above touches a generated region), and `test_module_readmes.py` passes.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(binance): spot's streams in the spec index, the READMEs and the crate docs (binance spot ws, Story 6)" -- docs/specs/binance/INDEX.md polyoxide-binance/README.md polyoxide-binance/src/spot/README.md polyoxide-binance/src/lib.rs polyoxide/README.md polyoxide-cli/README.md README.md
```

### Task 6.2: The rules and `CLAUDE.md`

**Files:**
- Modify: `.claude/rules/binance.md`
- Modify: `.claude/rules/sockets.md`
- Modify: `.claude/rules/errors.md`
- Modify: `.claude/rules/cli.md`
- Modify: `.claude/rules/umbrella.md`
- Modify: `.claude/rules/live-tests.md`
- Modify: `CLAUDE.md` (lines 7 and 40 only, outside every generated region)

- [ ] **Step 1: Write the failing check**

```bash
python3 - <<'EOF'
import pathlib

checks = [
    (".claude/rules/binance.md", "- `shared::ws`, built under", True),
    (".claude/rules/binance.md", "## Streams (`spot-ws`)", True),
    (".claude/rules/binance.md", "`binance-spot-ws` and `full` features", True),
    (".claude/rules/sockets.md", "`shared::ws`'s `BinanceProtocol`", True),
    (".claude/rules/errors.md", "`SpotWsError::Connect`", True),
    (".claude/rules/cli.md", "`binance spot stream`", True),
    (".claude/rules/cli.md", "`--all-mini-tickers`", True),
    (".claude/rules/umbrella.md", "`binance-spot-ws`", True),
    (".claude/rules/umbrella.md", "`SpotEvent`", True),
    (".claude/rules/live-tests.md", "`SpotWsError::from(WsError::from(..))`", True),
    ("CLAUDE.md", "`polyoxide binance spot stream`", True),
    ("CLAUDE.md", "and USDⓈ-M's streams |", False),
]
bad = []
for path, needle, present in checks:
    if (needle in pathlib.Path(path).read_text(encoding="utf-8")) != present:
        bad.append((path, needle, "missing" if present else "still there"))
for item in bad:
    print(item)
raise SystemExit(1 if bad else 0)
EOF
```

- [ ] **Step 2: Run it to see it fail**

Run the block. Expected: it prints the twelve tuples (eleven `missing`, and `("CLAUDE.md", "and USDⓈ-M's streams |", 'still there')`) and exits 1.

- [ ] **Step 3: Implement**

(a) `.claude/rules/binance.md`. Its front matter needs no edit: `polyoxide-binance/**` and `polyoxide-cli/src/commands/binance/**` already cover the spot socket and the CLI's spot command. Replace

```
`binance-usdm-ws`, `binance-spot` and `full` features, none of which its default, `polymarket`,
```

with

```
`binance-usdm-ws`, `binance-spot`, `binance-spot-ws` and `full` features, none of which its default,
`polymarket`,
```

Replace (the `shared` section's `shared::symbol` bullet; the new bullet goes before it)

```
- `shared::symbol`: `Symbol` and `InvalidSymbol`. Nothing else is shared: response types,
```

with

```
- `shared::ws`, built under `any(feature = "usdm-ws", feature = "spot-ws")` and so compiling under
  either alone: what both products' sockets do the same way, generic over a product's `StreamSet`
  (its stream name, path, update, error and event types, its paths and URL, its frame decode, its
  pacing interval and its cap on names per request) and its error's `SocketError` (constructors for
  the errors the shared code makes, and a `Shape` view the recovery and the router match on): the
  request correlation (`request`), the bare connection (`Bare`), the kit `Protocol`
  (`BinanceProtocol<S>`), and the router with its forwarding queue of 1024 and its stop
  (`supervised`). Each product's public socket types stay its own structs and enums over it.
- `shared::symbol`: `Symbol` and `InvalidSymbol`. Nothing else is shared: response types,
```

Replace

```
- `tests/venue_spot.rs` holds each method to the native client's values on the captured fixtures;
  the fixtures are captured, never written by hand, by `scripts/capture_binance_spot.py rest`.
```

with

```
- `tests/venue_spot.rs` holds each method to the native client's values on the captured fixtures;
  the fixtures are captured, never written by hand, by `scripts/capture_binance_spot.py rest`
  (the stream frames by its `ws`, described in the Streams (`spot-ws`) section below).
```

Append at the end of the file, after the last line ``  build that wants the frames names both features.``:

```

## Streams (`spot-ws`)

Nine spot market streams on `stream.binance.com`: `SpotWs` (one connection) and
`SpotWsBuilder`/`SupervisedSpotWs`, on the same `shared::ws` code as `usdm::ws`, with `SpotStreams`
as spot's `StreamSet`. `spot-ws` turns `spot` on.

- **One path.** Every stream is on `/stream` of `SPOT_WS_BASE`, `wss://stream.binance.com:9443`:
  `base_url` sets another host and the client appends `/stream`, so a test server's base works as it
  does for `usdm`. `StreamPath` has the one variant, `Stream`, exhaustive, and `Event::Disconnected`
  and `Reconnected` keep their `{ path }` field, so a consumer that folds outages treats both
  products alike; every `Disconnected` is followed by a `Reconnected` while the client runs, from
  the shared code.
- **The limits are measured**, not copied (`OBSERVED.md`, `## Spot`, from `### Stream limits`): at
  most 1024 streams per connection (`MAX_STREAMS_PER_CONNECTION`), 1024 names in one request
  (`MAX_NAMES_PER_REQUEST`: one `SUBSCRIBE` of 1024 names, 23,598 bytes, was acknowledged and held,
  where `usdm`'s is 200) and one request per 500 ms (`MIN_REQUEST_INTERVAL`). Binance counts at
  most 5 incoming messages a second on a connection, pings, pongs and requests included, and
  closes one past it with 1008 "Too many requests": a request every 250 ms was closed after 16 s,
  and one every 500 ms held for 45 s. So the client enforces all three first (the 1025th stream is
  refused with nothing sent), and `docs/MUTANTS.md`'s socket-pace row holds the interval.
- **`StreamName` is the only way to name a stream**, because Binance acknowledges every `SUBSCRIBE`,
  an unknown symbol, an unknown stream type and an uppercase symbol included, and an uppercase
  symbol delivers nothing. It refuses an uppercase ASCII letter in a symbol and an explicit
  `@250ms` depth, and spells a symbol's ASCII letters in lowercase and every other character as it
  is (`币安人生usdt@miniTicker`, held by `stream_non_ascii_miniTicker.json`). `!ticker@arr` is
  acknowledged and delivers nothing, so it is not a `StreamName`; `!miniTicker@arr` is.
- **Two frames carry less than the rest.** A partial-depth frame and a `bookTicker` frame have no
  event type, and partial depth has no symbol: both take their stream from the combined envelope,
  so `PartialDepthEvent::symbol` is read from the envelope's `stream` and not serialised back. A
  single-symbol frame whose `e` is not its stream's is `Payload::Unknown`. The keys Binance
  documents as "Ignore", `M` on `trade` and `aggTrade` and `B` in the kline, are not modelled:
  `wire_agreement_spot_ws`'s `IGNORED` names them, and
  `tests/live_spot_ws.rs::live_frames_carry_no_unmodelled_keys` is the drift detector.
- **The error** `SpotWsError` has `UsdmWsError`'s variants, docs and `Classify` rows but
  `WrongPath`, which one path cannot produce, and `spot::ws::StopCause` is its own. A path that
  stops on an error ends the stream after yielding it, and `close()` and later membership calls
  answer `SpotWsError::Stopped { cause }`, as `usdm`'s do (above).
- **The scripted file is shared.** `src/usdm/ws/scripted.rs` names nothing of either product, so
  `spot::ws`'s unit tests and `tests/supervision_spot.rs` include it with `#[path]`: moving it
  would lose a test-context file and copying it would keep two. The `test-server` feature, with
  `spot-ws`, exposes the captured frames as `spot::ws::fixtures`, which the CLI's tests import.
  The frames are captured, never written by hand, by `scripts/capture_binance_spot.py ws`: one
  connection and one `SUBSCRIBE`.
- **The production socket is a shared address.** Binance bans an address it disconnects
  repeatedly, and the live suites and other sessions on a host share the address. No test,
  capture or probe against production sends faster than the client paces, and none sends an
  over-limit request on purpose.
- The CLI's `binance spot stream` (`polyoxide-cli/src/commands/binance/spot.rs`, `cli.md`) streams
  through the supervised tier.
```

(b) `.claude/rules/sockets.md`. Replace

```
  connection, such as Binance's 200 ms request pacing;
```

with

```
  connection, such as Binance's request pacing (200 ms on USDⓈ-M, 500 ms on spot);
```

Replace

```
- On the `Supervisor`: perps (`PerpsProtocol`), Binance (one supervisor per routed path, behind a
  router), Kalshi (`PredictionsProtocol`), and the CLOB's supervised market and user sockets.
```

with

```
- On the `Supervisor`: perps (`PerpsProtocol`), Binance (`shared::ws`'s `BinanceProtocol`, one for
  both products: one supervisor per routed path, behind a router; spot has the one path),
  Kalshi (`PredictionsProtocol`), and the CLOB's supervised market and user sockets.
```

(c) `.claude/rules/errors.md`. Replace

```
- The wrappers: `PerpsWsError::Connection`, `UsdmWsError::Connect`,
  `PredictionsWsError::Connection`, `RtdsError::Connection`, `ClobWsError::Connection` and
```

with

```
- The wrappers: `PerpsWsError::Connection`, `UsdmWsError::Connect`, `SpotWsError::Connect`,
  `PredictionsWsError::Connection`, `RtdsError::Connection`, `ClobWsError::Connection` and
```

(d) `.claude/rules/cli.md`. Replace

```
It queries Polymarket's APIs and Binance USDⓈ-M market data, and depends on the component crates
```

with

```
It queries Polymarket's APIs and Binance's USDⓈ-M and spot market data, and depends on the component crates
```

Replace

```
`rtds`, `sports` and, with `keychain`, `credentials`; `polyoxide binance` holds `usdm`;
```

with

```
`rtds`, `sports` and, with `keychain`, `credentials`; `polyoxide binance` holds `usdm` and `spot`;
```

Replace

```
`polymarket clob stream market` and `… user`, `polymarket rtds stream`, `polymarket sports stream`
and `binance usdm stream` each describe their socket as a `commands::stream::Feed` (what each
```

with

```
`polymarket clob stream market` and `… user`, `polymarket rtds stream`, `polymarket sports stream`,
`binance usdm stream` and `binance spot stream` each describe their socket as a
`commands::stream::Feed` (what each
```

Replace

```
  crate's captured frames (`polyoxide_binance::usdm::ws::fixtures`, features `usdm-ws` and
  `test-server`).
```

with

```
  crate's captured frames (`polyoxide_binance::usdm::ws::fixtures`, features `usdm-ws` and
  `test-server`).
- `binance spot stream` streams through the supervised tier of `polyoxide-binance`'s `spot::ws`,
  with `usdm`'s flags and parsers: `--symbol` and `--kind` are comma-separated and each kind is
  streamed for each symbol. Its kinds are `agg-trade`, `trade`, `kline-<interval>`, `mini-ticker`,
  `ticker`, `book-ticker`, `avg-price` and `depth5`, `depth10` and `depth20` (each every 100 ms);
  `--all-mini-tickers` streams `!miniTicker@arr` in the place of `usdm`'s two array flags.
  `pretty` prints a line per update and per array row, outage markers and skipped frames go to
  stderr, and `--format json` prints each frame's envelope. `tests/ws_binance.rs` drives it with
  `polyoxide_binance::spot::ws::fixtures` (features `spot-ws` and `test-server`).
```

(e) `.claude/rules/umbrella.md`. Replace

```
  `SportsEvent`, `UsdmEvent`, `PredictionsEvent`). One `const _` per prelude block pins it to its
```

with

```
  `SportsEvent`, `UsdmEvent`, `SpotEvent`, `PredictionsEvent`). One `const _` per prelude block pins it to its
```

Replace

```
  `crate::binance::spot::Spot`. `SpotMarket` is at `polyoxide::venue` beside the other bundles.
```

with

```
  `crate::binance::spot::Spot`. `SpotMarket` is at `polyoxide::venue` beside the other bundles.
- `binance-spot-ws` is `["binance-spot", "polyoxide-binance/spot-ws"]`, and `full` turns it on.
  Under it the prelude also names `Event` as `SpotEvent`, `StreamName` as `SpotStreamName`
  (`USDⓈ-M`'s keeps the bare `StreamName`), `SpotWs`, `SpotWsBuilder` and `SpotWsError`, and a
  passthrough `const _` names `SpotWs` at `crate::binance::spot::ws`, with another at the prelude.
```

(f) `.claude/rules/live-tests.md` (line 44, in the wrap-first bullet). Replace

```
  `UsdmWsError::from(WsError::from(..))`, `RtdsError::from(WsError::from(..))`,
  `ClobWsError::from(WsError::from(..))`, `SportsError::Transport { source: WsError::from(..) }`),
```

with

```
  `UsdmWsError::from(WsError::from(..))`, `SpotWsError::from(WsError::from(..))`,
  `RtdsError::from(WsError::from(..))`, `ClobWsError::from(WsError::from(..))`,
  `SportsError::Transport { source: WsError::from(..) }`),
```

(g) `CLAUDE.md`. Replace, in the "Project Overview" paragraph (line 7),

```
`polyoxide-binance` for Binance's USDⓈ-M futures market data and streams and its spot market data,
```

with

```
`polyoxide-binance` for Binance's USDⓈ-M futures and spot market data and the market streams of both,
```

and, in the same line,

```
and a standalone CLI for Polymarket and Binance. Hard fork
```

with

```
and a standalone CLI for Polymarket and for Binance's streams (`polyoxide binance usdm stream`, `polyoxide binance spot stream`). Hard fork
```

Replace, in the rules table (line 40),

```
and USDⓈ-M's streams |
```

with

```
and the streams of both products |
```

Do not touch the lines between `<!-- generated:begin … -->` and `<!-- generated:end … -->` markers (the crate list, the CLI's dependencies and the umbrella's features name `spot-ws` and `binance-spot-ws` once Stories 4 and 5 ran `gen_registry.py --write`).

- [ ] **Step 4: Run it to see it pass**

Run the check from Step 1 again. Expected: no output, exit 0. Then:

```bash
python3 scripts/gen_registry.py --check
bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q'
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features'
```

Expected: `gen_registry.py --check` reports nothing to rewrite, the pytest run passes (`test_claude_rules.py` finds every `paths:` glob matching a file, and the table in `CLAUDE.md` agreeing with the directory), and the doc build has no warning.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(rules): shared::ws and spot's streams in the Binance, socket, error, CLI and umbrella rules (binance spot ws, Story 6)" -- .claude/rules/binance.md .claude/rules/sockets.md .claude/rules/errors.md .claude/rules/cli.md .claude/rules/umbrella.md .claude/rules/live-tests.md CLAUDE.md
```
