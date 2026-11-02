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
- `polyoxide-binance/src/usdm/ws/mod.rs`, `client.rs`, `error.rs`, `protocol.rs`, `supervised.rs` (modify): onto `shared::ws`, their tests untouched.
- `.claude/rules/binance.md` (modify): the `Streams (usdm-ws)` sentences that name `src/usdm/ws/protocol.rs`.

**Story 3: spot stream names, payloads and errors.**

- `polyoxide-binance/Cargo.toml` (modify): the `spot-ws` feature, the `test-server` comment, one `[[test]]` stanza.
- `polyoxide-binance/src/lib.rs` (modify): the `Classify` assertion's spot socket rows.
- `polyoxide-binance/src/spot/mod.rs` (modify): `pub mod ws;` under `spot-ws`.
- `polyoxide-binance/src/spot/ws/mod.rs`, `stream.rs`, `event.rs`, `error.rs`, `fixtures.rs` (create).
- `polyoxide-binance/tests/wire_agreement_spot_ws.rs` (create).
- `.github/scripts/tests/test_dep_allowlists.py` (modify) and `ci/dep-allowlists/polyoxide-binance@spot-ws.txt` (by `--write`).

**Story 4: spot socket clients.**

- `polyoxide-binance/src/shared/mod.rs` (modify): `ws`'s `cfg` widened to `spot-ws`.
- `polyoxide-binance/src/spot/ws/mod.rs` (modify): the client modules, their re-exports and the module docs.
- `polyoxide-binance/src/spot/ws/protocol.rs`, `client.rs`, `supervised.rs` (create).
- `polyoxide-binance/Cargo.toml` (modify): docs.rs features, `identifiers`, the `live_spot_ws` live entry, two `[[test]]` stanzas.
- `polyoxide-binance/tests/supervision_spot.rs`, `live_spot_ws.rs` (create).
- `docs/MUTANTS.md` (modify): the socket-pace row.
- The files `gen_registry.py --write` rewrites; `Cargo.lock`.

**Story 5: the CLI and the umbrella.**

- `polyoxide-cli/Cargo.toml` (modify): `polyoxide-binance` with `spot-ws`, in dependencies and dev-dependencies.
- `polyoxide-cli/src/commands/binance/mod.rs` (modify), `polyoxide-cli/src/commands/binance/spot.rs` (create).
- `polyoxide-cli/tests/ws_binance.rs` (modify, append only).
- `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs` (modify): `binance-spot-ws`, `full`, the crate docs, the prelude and the passthrough checks.
- The files `gen_registry.py --write` rewrites; `Cargo.lock`.

**Story 6: docs.**

- `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `README.md`, `CLAUDE.md` (modify, outside generated regions).
- `.claude/rules/binance.md`, `.claude/rules/sockets.md`, `.claude/rules/errors.md`, `.claude/rules/cli.md`, `.claude/rules/umbrella.md` (modify).
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
- `src/shared/ws/protocol.rs`: `#[derive(Debug, Clone)] pub(crate) enum Change<N> { Subscribe(Vec<N>), Unsubscribe(Vec<N>) }`, and `#[derive(Debug)] pub(crate) struct BinanceProtocol<S: StreamSet>` with `pub(crate) fn new(base_url: String, path: S::Path) -> Self` and `impl<S: StreamSet> polyoxide_ws::Protocol for BinanceProtocol<S>` whose `Event = S::Event`, `Error = S::Error`, `Membership = Vec<S::Name>`, `Change = Change<S::Name>` and `Ack = Vec<S::Name>`: today's `UsdmProtocol`, its recovery matching on `Shape`, row for row.
- `src/shared/ws/supervised.rs`: `pub(crate) struct Settings<S: StreamSet> { pub(crate) base_url: String, pub(crate) timing: polyoxide_ws::Timing, pub(crate) streams: Vec<S::Name> }`; `pub(crate) async fn connect<S: StreamSet>(settings: Settings<S>) -> Result<Running<S>, S::Error>`, today's `UsdmWsBuilder::connect` body; `pub(crate) struct Running<S: StreamSet>` with `membership(&self) -> Handle<S>`, `async close(self) -> Result<(), S::Error>`, `is_running(&self) -> bool` and `poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<S::Event, S::Error>>>`; and `#[derive(Clone)] pub(crate) struct Handle<S: StreamSet>` with `async subscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error>` and `async unsubscribe(&self, streams: Vec<S::Name>) -> Result<(), S::Error>`. The router, `Command`, `Stop`, `Path`, `Ended`, `route` and `forward` are private to this file.
- In `usdm::ws`, after the story: `src/usdm/ws/protocol.rs` holds `pub(crate) struct UsdmStreams;` with `impl StreamSet for UsdmStreams`, `pub(crate) type UsdmProtocol = BinanceProtocol<UsdmStreams>;` and `pub(crate) type Change = shared::ws::protocol::Change<StreamName>;`; `src/usdm/ws/error.rs` holds `impl SocketError for UsdmWsError`; `UsdmWs`, `SupervisedUsdmWs` and `MembershipHandle` wrap `Bare<UsdmStreams>`, `Running<UsdmStreams>` and `Handle<UsdmStreams>`; `UsdmWsBuilder` keeps its fields `base_url`, `timing` and `streams`. Story 4's spot types take the same shape.

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

**Story 4 → Stories 5 and 6.** In `polyoxide-binance`'s `spot::ws`:

- `src/spot/ws/protocol.rs` (private): `pub(crate) struct SpotStreams;` with `impl StreamSet for SpotStreams` (`PATHS = StreamPath::ALL`, `MAX_NAMES_PER_REQUEST`, `MIN_REQUEST_INTERVAL`, `StreamPath::url`, `Update::from_json`), `pub(crate) type SpotProtocol = BinanceProtocol<SpotStreams>;` and `impl SocketError for SpotWsError` (in `error.rs` or here; its `shape` never returns `WrongPath`).
- `src/spot/ws/client.rs`: `pub const CONNECT_TIMEOUT: Duration` and `pub const ANSWER_TIMEOUT: Duration`, 10 s each; `pub struct SpotWs` over `Bare<SpotStreams>`, with `pub async fn connect(streams: impl IntoIterator<Item = StreamName>) -> Result<Self, SpotWsError>`, `pub async fn connect_to(base_url: &str, streams: impl IntoIterator<Item = StreamName>) -> Result<Self, SpotWsError>`, `subscribe(&mut self, streams: &[StreamName])`, `unsubscribe(&mut self, streams: &[StreamName])`, `list_subscriptions(&mut self) -> Result<Vec<String>, SpotWsError>`, `ping(&mut self) -> Result<Duration, SpotWsError>`, `close(&mut self) -> Result<(), SpotWsError>`, `streams(&self) -> &[StreamName]`, and `Stream<Item = Result<Update, SpotWsError>>`.
- `src/spot/ws/supervised.rs`: `#[derive(Debug)] #[non_exhaustive] pub enum Event { Update(Box<Update>), Disconnected { path: StreamPath, reason: DisconnectReason }, Reconnected { path: StreamPath } }`; `#[derive(Debug)] #[non_exhaustive] pub enum DisconnectReason { Closed { code: Option<u16>, reason: String }, Error(SpotWsError), Stale, Rotation }` with `usdm`'s `Display` texts; `#[derive(Debug, Clone)] pub struct SpotWsBuilder { base_url: String, timing: Timing, streams: Vec<StreamName> }` with `new()` (§5.9's defaults), `Default`, `base_url(url)`, `polyoxide_ws::timing_setters!`, `streams(streams)` and `async connect(self) -> Result<SupervisedSpotWs, SpotWsError>`; `pub struct SupervisedSpotWs` with `membership(&self) -> MembershipHandle`, `async close(self) -> Result<(), SpotWsError>` and `Stream<Item = Result<Event, SpotWsError>>`; `#[derive(Debug, Clone)] pub struct MembershipHandle` with `async subscribe(&self, streams: impl IntoIterator<Item = StreamName>) -> Result<(), SpotWsError>` and `async unsubscribe` likewise.
- `src/spot/ws/mod.rs` gains `pub mod client; mod protocol; pub mod supervised;`, `#[cfg(test)] #[path = "../../usdm/ws/scripted.rs"] mod scripted;`, `pub use client::SpotWs;` and `pub use supervised::{DisconnectReason, Event, MembershipHandle, SpotWsBuilder, SupervisedSpotWs};`.
- `Cargo.toml`: `[package.metadata.polyoxide.live.live_spot_ws]` with `suite = "spot"`, `timeout = 15`, `features = ["spot-ws"]`, `secrets = []`; `identifiers` gains `stream.binance.com`; docs.rs `features` gains `spot-ws`; `[[test]]` `supervision_spot` (`["spot-ws", "test-server"]`) and `live_spot_ws` (`["spot-ws"]`).
- `tests/supervision_spot.rs` and `tests/live_spot_ws.rs` (whose drift test is `live_frames_carry_no_unmodelled_keys`) include `../src/usdm/ws/scripted.rs` with `#[path]` where they script a server.
- `docs/MUTANTS.md`'s socket-pace row: the line of `spot::ws::MIN_REQUEST_INTERVAL`, the mutation `Duration::from_millis(250)`, and the tests that fail under it.

**Story 5 → Story 6.**

- `polyoxide_cli::commands::binance::spot`: `#[derive(Subcommand)] pub enum SpotCommand { Stream { #[command(flatten)] args: SpotArgs } }` with `pub async fn run(self) -> Result<()>`; `#[derive(Args, Debug)] pub struct SpotArgs { pub symbol: Vec<String>, pub kind: Vec<String>, pub all_mini_tickers: bool, pub format: OutputFormat, pub count: Option<u64>, pub timeout: Option<Duration> }` with `usdm::BinanceArgs`'s flag spellings and parsers, `--all-mini-tickers` for `!miniTicker@arr`, and the kinds of spec §5.12; `pub async fn run(args: SpotArgs) -> Result<()>`; `pub async fn run_with<S>(args: SpotArgs, events: S, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> where S: Stream<Item = Result<Event, SpotWsError>> + Unpin`.
- `BinanceCommand` gains `/// Spot market data` `Spot { #[command(subcommand)] command: spot::SpotCommand }`, so the command is `polyoxide binance spot stream`.
- The umbrella: `binance-spot-ws = ["binance-spot", "polyoxide-binance/spot-ws"]`, `full` gaining it; the prelude names and passthrough checks of spec §5.11.

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

  Moves `usdm::ws`'s Binance-wide machinery into `src/shared/ws/`, generic over `StreamSet` and `SocketError` as the names contract gives them, and rebuilds `usdm::ws` on it: `UsdmStreams` and `impl SocketError for UsdmWsError` describe USDⓈ-M, and `UsdmWs`, `SupervisedUsdmWs`, `MembershipHandle` and `UsdmWsBuilder` wrap the generic types, with every public doc, path, name and signature as it was. Behaviour is unchanged line for line: the recovery rows, the record-before-send, the batched and paced replay, the refused batch undone, the router's stop cause, the forwarding queue, the rotation constant. No test file and no test module changes; `UsdmProtocol`, `Change`, `UsdmWs::open`, `UsdmWs::close_frame`, `Answer` and the builder's fields stay where the unit tests reach them. `supervision_usdm`, `supervision_usdm_edges`, `supervision_usdm_kit` and `wire_agreement_usdm_ws` pass unchanged. `.claude/rules/binance.md`'s sentences naming `src/usdm/ws/protocol.rs` name `src/shared/ws/protocol.rs` in the same commit.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/ws/mod.rs`, `polyoxide-binance/src/shared/ws/request.rs`, `polyoxide-binance/src/shared/ws/bare.rs`, `polyoxide-binance/src/shared/ws/protocol.rs`, `polyoxide-binance/src/shared/ws/supervised.rs`, `polyoxide-binance/src/usdm/ws/mod.rs`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `.claude/rules/binance.md`
  - **Interfaces:** exposes "Story 2 → Story 4"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_claude_rules.py -q'`
  - **Review:** deep (a refactor under a public socket API that prader-rs folds outages on)
  - **Reading:** spec §2 (the moved-tests row), §5.2, §5.9 (from "`shared::ws` holds"); `polyoxide-binance/src/usdm/ws/mod.rs`, `polyoxide-binance/src/usdm/ws/client.rs:1-352`, `polyoxide-binance/src/usdm/ws/protocol.rs:1-333`, `polyoxide-binance/src/usdm/ws/supervised.rs:1-749`, `polyoxide-binance/src/usdm/ws/error.rs:1-214`, and the `use` lines of each file's tests module; `polyoxide-ws/src/supervisor/mod.rs:100-330` (the `Protocol` trait); `polyoxide-binance/src/shared/mod.rs`; `scripts/test_body_diff.py:1-90`; `.claude/rules/binance.md` (`## Streams (usdm-ws)`)

- **Story 3** — Spot stream names, payloads and errors (spec §3.2, §5.9, §5.10).

  Adds the `spot-ws` feature and `spot::ws`'s data half: `StreamPath`, the constants, `StreamName` with its `Display` and `FromStr` round trip, `DepthLevels` and `DepthSpeed`, the payload types and `Update::from_json` (a partial-depth frame's symbol from the envelope; `Payload::Unknown` for a single-symbol object whose `e` is not its stream's), `SpotWsError` with `StopCause`, `From<WsError>` and its `Classify` rows, and `fixtures`. Unit tests beside each type cover the name round trip (the non-ASCII fixture's echoed name included), every refused spelling (an uppercase symbol, `@250ms`, `!ticker@arr`), the envelope symbol, `Unknown`, and every error's class. `wire_agreement_spot_ws` holds each fixture to its type. The dep allowlists gain `polyoxide-binance@spot-ws.txt`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/src/spot/ws/stream.rs`, `polyoxide-binance/src/spot/ws/event.rs`, `polyoxide-binance/src/spot/ws/error.rs`, `polyoxide-binance/src/spot/ws/fixtures.rs`, `polyoxide-binance/tests/wire_agreement_spot_ws.rs`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide-binance@spot-ws.txt`, `Cargo.lock`
  - **Interfaces:** exposes "Story 3 → Stories 4 and 5"; uses Story 1's fixtures and plan 1's `spot::types::{Symbol, Interval, Level}`
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py -q'`
  - **Review:** deep (public payload types every consumer decodes into, and an error's classes)
  - **Reading:** spec §3.2, §5.9, §5.10; `docs/specs/binance/OBSERVED.md` (`### Stream answers`, `### Frames`); `polyoxide-binance/src/usdm/ws/stream.rs:1-191`, `polyoxide-binance/src/usdm/ws/event.rs:1-487`, `polyoxide-binance/src/usdm/ws/error.rs:1-214`, `polyoxide-binance/src/usdm/ws/fixtures.rs`, `polyoxide-binance/tests/wire_agreement_usdm_ws.rs`, `polyoxide-binance/src/spot/types.rs:300-430`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/Cargo.toml`, `.github/scripts/tests/test_dep_allowlists.py:100-160,275-290`

- **Story 4** — Spot socket clients, registration and the live suite (spec §5.9, §5.10, §7).

  Widens `shared::ws`'s `cfg` and adds `SpotStreams`, `SpotWs`, `SpotWsBuilder`, `SupervisedSpotWs`, `MembershipHandle`, `Event` and `DisconnectReason` on the generic code, with module docs and examples that compile under `spot-ws`. Unit tests in `client.rs` and `supervised.rs` and the `supervision_spot` suite run on the scripted server (`src/usdm/ws/scripted.rs` by `#[path]`): one path, `/stream`; a request carrying all 1024 names; requests at least 500 ms apart; the 1025th stream refused with nothing sent; `Disconnected` then `Reconnected`; rotation; `Stopped { cause }` after a refused replay; a refused handshake's `Retry-After`. `live_spot_ws` subscribes to every kind on production at the client's pace and runs `live_frames_carry_no_unmodelled_keys`. Registers the live entry and identifier, runs `gen_registry.py --write`, and adds the socket-pace row to `docs/MUTANTS.md`, proved by applying the mutation and seeing its named tests fail.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/spot/ws/mod.rs`, `polyoxide-binance/src/spot/ws/protocol.rs`, `polyoxide-binance/src/spot/ws/client.rs`, `polyoxide-binance/src/spot/ws/supervised.rs`, `polyoxide-binance/src/spot/ws/error.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/supervision_spot.rs`, `polyoxide-binance/tests/live_spot_ws.rs`, `docs/MUTANTS.md`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`, `Cargo.lock`
  - **Interfaces:** exposes "Story 4 → Stories 5 and 6"; uses Story 2's `shared::ws` items and Story 3's `spot::ws` types and fixtures
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && cargo test -p polyoxide-binance --no-default-features --features spot-ws --test live_spot_ws -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py`
  - **Review:** deep (a supervised socket that must never pass spot's message limit on a shared address)
  - **Reading:** spec §5.9, §5.10, §7; `docs/specs/binance/OBSERVED.md` (`### Stream limits`, `### Pace at 500 ms`); the `shared::ws` files Story 2 wrote; `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs:1-120` (as Story 2 left them); `polyoxide-binance/src/usdm/ws/scripted.rs`; `polyoxide-binance/tests/supervision_usdm.rs:1-140`, `polyoxide-binance/tests/live_usdm_ws.rs`; `polyoxide-binance/Cargo.toml` (the `[package.metadata.polyoxide]` tables); `docs/MUTANTS.md` (row (l)'s format and the paragraphs on proving)

- **Story 5** — `polyoxide binance spot stream` and the umbrella's `binance-spot-ws` (spec §5.11, §5.12).

  Adds `commands/binance/spot.rs` in `usdm.rs`'s shape: `SpotCommand`, `SpotArgs` with spec §5.12's kinds and `--all-mini-tickers`, `run` on the supervised tier, `run_with` over any event stream, and a `Feed` whose `pretty` prints a line per update and per `!miniTicker@arr` row and whose `json` prints each frame's envelope; outage markers and a skipped frame go to stderr. `polyoxide-cli` builds `polyoxide-binance` with `usdm-ws` and `spot-ws` (and `test-server` in dev-dependencies). New tests appended to `tests/ws_binance.rs` drive every flag on `spot::ws::fixtures`. The umbrella gains `binance-spot-ws`, `full` gaining it, the crate docs' feature list, and the prelude names and passthrough checks of spec §5.11. `gen_registry.py --write` regenerates the regions that list crate features and needs.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-cli/Cargo.toml`, `polyoxide-cli/src/commands/binance/mod.rs`, `polyoxide-cli/src/commands/binance/spot.rs`, `polyoxide-cli/tests/ws_binance.rs`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `CLAUDE.md`, `README.md`, `docs/ARCHITECTURE.md`, `Cargo.lock`
  - **Interfaces:** exposes "Story 5 → Story 6"; uses Story 3's types and fixtures and Story 4's `SpotWsBuilder`, `Event` and `SpotWsError`
  - **Verify:** `cargo test -p polyoxide-cli --all-features && cargo test -p polyoxide --all-features && cargo hack check -p polyoxide -p polyoxide-cli --each-feature --no-dev-deps && python3 scripts/gen_registry.py --check`
  - **Review:** deep (a new CLI command and the umbrella's public surface)
  - **Reading:** spec §5.11, §5.12; `polyoxide-cli/src/commands/binance/mod.rs`, `polyoxide-cli/src/commands/binance/usdm.rs`, `polyoxide-cli/src/commands/stream.rs:1-120`, `polyoxide-cli/tests/ws_binance.rs`, `polyoxide-cli/Cargo.toml`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs:1-60,120-140,260-360`

- **Story 6** — Docs (spec §5.13).

  Adds spot's streams to `docs/specs/binance/INDEX.md` (the host row and a `## Spot streams covered` table, with `live_spot_ws`'s drift detector), a streams paragraph to `src/spot/README.md` (prose only: its fences compile under `spot`), `spot-ws` to the crate README and `src/lib.rs`'s crate docs, `binance-spot-ws` to the umbrella and root READMEs, and the CLI's spot command to the hand-written lines of `CLAUDE.md`. In the rules: `.claude/rules/binance.md` gains `shared::ws` in its `shared` section and a `## Streams (spot-ws)` section (one path, 1024 names a request, 500 ms, the scripted file both products include), and its `paths:` cover `polyoxide-binance/src/spot/ws/**` and `polyoxide-cli/src/commands/binance/spot.rs`; `sockets.md` says Binance's protocol is `shared::ws`'s, for both products; `errors.md` lists `SpotWsError::Connect`; `cli.md` gains `binance spot stream`; `umbrella.md` gains `binance-spot-ws` and its prelude names.

  - **Developer:** junior-dev
  - **Files:** `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `README.md`, `CLAUDE.md`, `.claude/rules/binance.md`, `.claude/rules/sockets.md`, `.claude/rules/errors.md`, `.claude/rules/cli.md`, `.claude/rules/umbrella.md`
  - **Interfaces:** uses every earlier story's names; exposes none
  - **Verify:** `cargo test --doc -p polyoxide-binance --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q' && bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features'`
  - **Review:** light (docs and rules; no library code)
  - **Reading:** spec §5.13; `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/spot/README.md`, `.claude/rules/binance.md`, `.claude/rules/sockets.md`, `.claude/rules/errors.md` (`## Socket errors`), `.claude/rules/cli.md`, `.claude/rules/umbrella.md`

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->
