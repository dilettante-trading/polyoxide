# Epic 9: Kalshi Published Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** `polyoxide-kalshi` becomes a complete, publishable market-data venue: `Candles` at Kalshi's three widths through a provided `Candles::intervals()`, the socket's `ticker` and `trade` channels, a local order book, an in-place re-snapshot on a sequence gap, measured read limits, and the crate published and re-exported by the umbrella as `polyoxide::kalshi`.

**Architecture:** Two additive foundation edits come first: `polyoxide-venue`'s `Candles` gains a provided `intervals()` and the `UnservedInterval` refusal, and `polyoxide-ws`'s `Protocol` gains a provided `follow_up()` that lets a protocol send a frame after `decode`. Kalshi's `predictions` module then implements `Candles` over the batch candlestick route, its socket protocol gains per-channel `sid`s and the `get_snapshot` recovery, and a pure `ws::book` keeps ladders from the events. Last, the crate drops `publish = false` and the umbrella gains `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws`. The release commit is an owner session after the run.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), tokio, ring 0.17, rust_decimal, serde, dynosaur 0.3.1, mockito, the kit's scripted WebSocket server, test-support's soak helpers, and Python capture scripts on `scripts/capture_common.py`.

**Spec:** `docs/superpowers/specs/2026-10-19-epic-9-kalshi-published-design.md`, read with Epic 8's spec (`docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`), whose decisions stand unless this one replaces them, and the architecture spine with its amendments through `spine-amendments/epic-8.md`.

---

## Rules for every story

- **Run commands with the dev shell's prefix**, `nix develop --command`. A story's gates are the profile's (spec §13); its **Verify** line is the developer's done-check, and it ends with the touch-set gate.
- **The touch-set gate.** `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh` (Story 1 writes it; names contract) prints every path changed, staged, unstaged or untracked since `git merge-base main HEAD` that `epic-9-touch-set.txt` does not admit, and fails when it prints one. An edit outside the allowlist stops the story and goes to the owner.
- **No story quotes code another story writes** (spec §1.5). Shared files grow by anchors the names contract gives verbatim, or by replacing a whole block the contract gives for the story before.
- **Add tests; never change an existing test's body** (AD-12). New tests are appended after a file's last test, or go in a file of their own.
- **The foundation crates name no venue** (`python3 scripts/venue_gate.py`). Stories 2 and 4 use made-up words in docs and tests, never "kalshi", "predictions", "clob", "Polymarket", "Binance" or "perps".
- **Generated regions are regenerated, never edited**: `python3 scripts/gen_registry.py --write` after any `[package.metadata.polyoxide]` change. `docs/ARCHITECTURE.md` is never edited.
- **README text is prose with no code fence**, since the crate and module READMEs are doctests.
- **Mirrors stay byte for byte**; findings go in `docs/specs/kalshi/OBSERVED.md` above its Epic 9 anchor.
- **Live suites**: credentials only through `polyoxide-test-support`'s loaders with literal env names; failures through `or_fail`, `ClassifiedResultExt::or_fail`, `environmental` and `transient`; no `.unwrap()` or `.expect(`; every test `#[ignore]`d.
- **The demo host needs the owner's key.** Stories 1, 5 and 7 use `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`. When either is absent the story stops at that step and asks; no fixture is ever written by hand.
- **A bare `D<n>` in code, docs and CLAUDE.md names a row of `divergences.md`.** Code and docs state this spec's decisions in words; planning records may cite "spec D<n>".
- **Nothing publishes.** No `cargo publish` without `--dry-run`, no `finish_release.sh`, no `gh workflow run Release`, no version bump.
- **Commit by pathspec**, conventional, naming the story, with `Cargo.lock` when cargo changes it, never `--no-verify`.

## File structure

Paths under `polyoxide-kalshi/` are from the crate root.

**Story 1: probes and captures.**

- `scripts/capture_kalshi_predictions.py` (modify): the candlestick captures.
- `scripts/capture_kalshi_predictions_ws.py` (modify): the multi-channel, multi-`sid`, `get_snapshot` and empty-subscribe captures.
- `tests/fixtures/predictions/candlesticks/` (create, captured): the files the names contract lists, and `PROVENANCE.md`.
- `tests/fixtures/predictions/ws/` (modify, captured): the new frames the names contract lists, and `PROVENANCE.md`'s rows.
- `docs/specs/kalshi/OBSERVED.md` (modify): the `## Epic 9` section, spec §1.6 and Story 1's findings, ending with the anchor.
- `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt` and `epic-9-touch-set.sh` (create).

**Story 2: `Candles::intervals()`.**

- `polyoxide-venue/src/market.rs` (modify): the provided method and the refusal sentence.
- `polyoxide-venue/src/records.rs` (modify): `CandleQuery::ensure_served` and `UnservedInterval`.
- `polyoxide-venue/src/lib.rs` (modify): the re-export and the `Classify` assertion.
- `polyoxide-venue/tests/candles_intervals.rs` (create).

**Story 3: Kalshi's `Candles`.**

- `src/predictions/request.rs` (modify): the two routes in `routes` and `routes::ALL`.
- `src/predictions/api/markets.rs` (modify): `candlesticks` and `series_candlesticks` and their builders.
- `src/predictions/types/markets.rs` (modify): `PeriodInterval` and the candlestick types.
- `src/predictions/types/mod.rs` (modify): their re-export.
- `src/predictions/venue.rs` (modify): `impl Candles`, `CandleExt` and `Ohlc`, and the module docs.
- `tests/mock_predictions_routes.rs`, `tests/query_setters_predictions.rs`, `tests/spec_agreement_predictions.rs`, `tests/wire_agreement_predictions.rs`, `tests/venue_predictions.rs`, `tests/live_predictions_venue.rs` (modify, append only), except the owner's exception (spec §13): the four eight-route tests, in `request.rs`, `mock_predictions_routes.rs`, `spec_agreement_predictions.rs` and `tests/live_predictions.rs` (modify), go to ten routes.
- `docs/specs/kalshi/OBSERVED.md` (modify, above the anchor, only for a finding of its own).

**Story 4: `Protocol::follow_up`.**

- `polyoxide-ws/src/supervisor/mod.rs` (modify): the provided method.
- `polyoxide-ws/src/supervisor/task.rs` (modify): the call after `decode` and the bounded send.
- `polyoxide-ws/src/supervisor/tests.rs` (modify, append only).

**Story 5: the socket protocol.**

- `src/predictions/ws/builder.rs`, `ws/protocol.rs`, `ws/event.rs`, `ws/error.rs` (modify).
- `src/predictions/ws/scripted.rs` (modify): the answerer learns several channels and `get_snapshot`.
- `src/predictions/ws/mod.rs` (modify): the module block the names contract gives for Story 5.
- `tests/supervision_predictions.rs`, `tests/wire_agreement_predictions_ws.rs`, `tests/live_predictions_ws.rs` (modify, append only), except the owner's exception (spec §13): two gap tests in `supervision_predictions.rs` rewritten to D9, and `ws/protocol.rs`'s unit tests moved to the multi-channel shapes.
- `docs/specs/kalshi/OBSERVED.md` (modify, above the anchor, only for a finding of its own).

**Story 6: the local book.**

- `src/predictions/ws/book.rs` (create): `Books`, `OrderBook` and their unit tests.
- `src/predictions/ws/mod.rs` (modify): replaces Story 5's block whole with Story 6's.

**Story 7: limits.**

- `polyoxide-kalshi/Cargo.toml` (modify, by its anchors): the `read_soak` example and test-support's `soak` feature.
- `examples/read_soak.rs` (create).
- `src/predictions/throttle.rs` (modify): the unsigned sizing, and a keyed client's public-read bucket if the probe says so.
- `tests/mock_predictions_signed.rs` (modify, append only), except the owner's third exception (spec §13): if public reads move, its two signed-bucket tests charge a signed account route.
- `docs/specs/kalshi/OBSERVED.md` (modify, above the anchor): the `### Rate limits` runs; and Epic 8's `## Rate limits` bullet saying the soak is deferred, rewritten to point at them.

**Story 8: publish wiring.**

- `Cargo.toml` (modify): the `polyoxide-kalshi` workspace pin, and the Kalshi mirrors' `description` and `covers` strings.
- `polyoxide-kalshi/Cargo.toml` (modify, by its anchors): `publish`, `description`, the `polyoxide` dev-dependency's features, the metadata `readme`.
- `polyoxide-kalshi/README.md`, `src/predictions/README.md`, `src/lib.rs`, `src/predictions/mod.rs` (modify): the crate docs drop "walking skeleton".
- `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`, `polyoxide/examples/venue_neutral.rs` (modify).
- `polyoxide/tests/venue_neutral.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs` (modify, append only).
- `ci/dep-allowlists/polyoxide@kalshi-predictions.txt`, `polyoxide@kalshi-predictions-ws.txt` (create, by `--write`).
- `.github/scripts/tests/test_dep_allowlists.py`, `.github/scripts/tests/test_ci_workflow.py` (modify).
- `.github/workflows/ci.yml` (modify): only the comment above the `features` job's umbrella header pins that says Kalshi is `publish = false` (spec §13).
- The files `gen_registry.py --write` rewrites; `Cargo.lock`.

**Story 9: docs and records.**

- `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `.env.example`, `docs/specs/INDEX.md` (modify, outside generated regions).
- `docs/s3-migration.md` (modify).
- `.github/scripts/tests/test_classify_coverage.py` (modify): a Kalshi vacuity guard.
- `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md` (create).
- `_bmad-output/implementation-artifacts/deferred-work.md` (modify).

## Names contract

Every name one story defines for another, with its type, signature and visibility, and every text a later story quotes. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Paths are from `polyoxide-kalshi/src/` unless they start with a crate or a top-level directory. Rust blocks are in the form `cargo fmt` leaves.

**Story 1 → Stories 2 to 9.**

- **The touch-set gate.** `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh` is Epic 8's `epic-8-touch-set.sh` with `epic-8` replaced by `epic-9`. `epic-9-touch-set.txt` holds one extended regex per line, anchored with `^`, admitting exactly spec §13's paths, plus `^docs/plans/2026-10-19-epic-9-kalshi-published\.md$` and `^\.flow/`.
- **The `OBSERVED.md` anchor.** Story 1 appends a section headed `## Epic 9` whose last line is, verbatim, `<!-- epic 9 findings go above this line -->`. Stories 3, 5 and 7 insert their findings directly above that line, each under a `###` heading of its own (Story 7's is `### Rate limits`), so one story's findings never land under another's heading.
- **Fixtures** (under `tests/fixtures/predictions/`), each captured, never hand-written:
  - `candlesticks/batch_1.json`, `candlesticks/batch_60.json`, `candlesticks/batch_1440.json`: `GET /markets/candlesticks` for one market at each width, over a span holding at least one traded period, one untraded period carrying `previous_dollars`, and, where the market has one, a period before its first trade (`price: {}`);
  - `candlesticks/series_60.json`: the series route at 60 for the same market and span;
  - `candlesticks/error_width.json`: the series route's `400` for `period_interval=5`;
  - `candlesticks/PROVENANCE.md`: one row per file (host, ticker, span, time);
  - `ws/subscribed_ticker.json`, `ws/subscribed_trade.json`: the `subscribed` answers of a three-channel `subscribe`;
  - `ws/ticker.json`, `ws/trade.json`: one frame of each channel;
  - `ws/snapshot_answer.json`: the `orderbook_snapshot` answering a `get_snapshot`, with its `id`, `sid` and `seq`, and `ws/deltas_after_snapshot.json`: the array of deltas on that `sid` that followed it;
  - `ws/error_snapshot_outside.json`: the error answering `get_snapshot` for a ticker outside the subscription;
  - `ws/multi_sid_answer.json`: the answer (or answers, as an array) to one `update_subscription` naming two `sids`.

**Story 2 → Stories 3, 8 and 9.** In `polyoxide-venue`, feature `market`, re-exported at the crate root beside `CandleQuery`:

- in `trait Candles`: `fn intervals(&self) -> &'static [CandleInterval] { CandleInterval::ALL }`, forwarded by `DynCandles`;
- `impl CandleQuery { pub fn ensure_served(&self, served: &'static [CandleInterval]) -> Result<(), UnservedInterval> }`;
- `#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] pub struct UnservedInterval { pub interval: CandleInterval, pub served: &'static [CandleInterval] }`, with `Display`, `std::error::Error` and `Classify` (`InvalidRequest`, a fault, no `retry_after`); `ClassifiedError: From<UnservedInterval>` through the existing blanket impl. No public constructor: only `ensure_served` builds it. Its `Display` is, for example, `` `Minute5` candles are not served; this venue serves Hour1, Day1 `` (the widths' `Debug` names, `, `-joined, `none` for an empty list).

**Story 3 → Stories 8 and 9.**

- `impl polyoxide_venue::Candles for Predictions`, whose `intervals()` returns `&[CandleInterval::Minute1, CandleInterval::Hour1, CandleInterval::Day1]`.
- `predictions::venue::CandleExt { pub yes_bid: Ohlc, pub yes_ask: Ohlc, pub open_interest: Decimal, pub mean: Option<Decimal> }` and `predictions::venue::Ohlc { pub open: Decimal, pub high: Decimal, pub low: Decimal, pub close: Decimal }`, each `#[derive(Debug, Clone, PartialEq)] #[non_exhaustive]`.
- `MarketsApi::candlesticks(&self, ticker: &Ticker, period: PeriodInterval, start_ts: i64, end_ts: i64) -> GetCandlesticks` and `MarketsApi::series_candlesticks(&self, series_ticker: &str, ticker: &Ticker, period: PeriodInterval, start_ts: i64, end_ts: i64) -> GetSeriesCandlesticks`; `predictions::types::PeriodInterval { Minute1, Hour1, Day1 }`.

**Story 4 → Story 5.** In `polyoxide_ws::Protocol`: `fn follow_up(&mut self) -> Option<Message> { None }`, `Message` being the type `Protocol::ping` returns. The task calls it once after each `decode` that leaves the connection up (not after a decode error that reconnects or stops, nor on a connection already finished, such as after the server's close frame) and sends what it returns, one frame per call, bounded by `stale_after`, a failed or timed-out send ending the connection as a failed ping does. A frame the protocol still holds when the connection ends stays with it, so the protocol clears its queued follow-ups in `reset`.

**Story 5 → Stories 6, 8 and 9.** In `predictions::ws`:

- **The module block of `ws/mod.rs`** that Story 5 writes (its items after the module docs):

      mod builder;
      mod error;
      mod event;
      mod protocol;
      #[cfg(test)]
      mod scripted;

      pub use builder::PredictionsWsBuilder;
      pub use error::PredictionsWsError;
      #[doc(hidden)]
      pub use event::frames;
      pub use event::{
          DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot, PublicTrade, TickerUpdate,
      };
      #[doc(hidden)]
      pub use protocol::signed_request;
      pub use protocol::{Change, Channel, PredictionsMembership, PredictionsProtocol};

  Story 6 replaces it whole with the same block plus `pub mod book;` directly before `mod builder;` and `pub use book::{Books, OrderBook};` directly before `pub use builder::PredictionsWsBuilder;`, the places rustfmt's sorting leaves them. The `SupervisedPredictionsWs` alias and the two URL consts below the block stay as they are.
- `pub enum Channel { OrderbookDelta, Ticker, Trade }` from `polyoxide_venue::wire_enum!` (`"orderbook_delta"`, `"ticker"`, `"trade"`), also `Ord`, in that order.
- `PredictionsWsBuilder::channels(self, channels: impl IntoIterator<Item = Channel>) -> Self`.
- `Event` gains `Ticker(TickerUpdate)`, `Trade(PublicTrade)` and `Resyncing { tickers: Vec<String> }`; `Snapshot(OrderbookSnapshot)`, `Delta(OrderbookDelta)`, `Disconnected { reason }` and `Reconnected` stay.
- `OrderbookSnapshot { pub market_ticker: String, pub market_id: String, pub bids: Vec<Level>, pub asks: Vec<Level> }` and `OrderbookDelta { pub market_ticker: String, pub side: BookSide, pub price: Decimal, pub delta: Decimal, pub ts: Option<UnixMillis>, pub client_order_id: Option<String> }` stay as Epic 8 left them, in YES pricing (`BookSide::Bid` a YES bid, `BookSide::Ask` a NO bid priced as a YES ask).
- `TickerUpdate` and `PublicTrade`, `#[derive(Debug, Clone, PartialEq)] #[non_exhaustive]`, with spec D8's fields, all `pub`; prices and sizes `Decimal`, `TickerUpdate`'s `dollar_volume` and `dollar_open_interest` `i64` (whole dollars, as the mirror types them), `ts: UnixMillis`, `taker_outcome_side: OutcomeSide`, `taker_book_side: BookSide` (both from `crate::predictions::types`).
- `Change` gains `Snapshot(Vec<Ticker>)`; `PredictionsMembership` gains `fn snapshot(&self, tickers: impl IntoIterator<Item = Ticker>) -> impl Future<Output = Result<(), PredictionsWsError>> + Send`.
- `PredictionsWsError` gains `SnapshotTimeout { pub sid: u64, pub waited: Duration }`, classed `Unavailable`, with recovery `Reconnect`.
- `PredictionsWsError` gains `InvalidRequest { reason: String }`, classed `InvalidRequest`, not retriable: an empty channel set at `connect`, or a `snapshot()` of a ticker outside the membership or without the order-book channel.

**Story 6 → Stories 8 and 9.** In `predictions::ws::book`, re-exported at `predictions::ws`:

- `#[derive(Debug, Default, Clone)] pub struct Books`, with `pub fn new() -> Self`, `pub fn apply(&mut self, event: &Event) -> Option<&OrderBook>`, `pub fn get(&self, market_ticker: &str) -> Option<&OrderBook>`.
- `#[derive(Debug, Clone, PartialEq)] pub struct OrderBook`, with `pub fn market_ticker(&self) -> &str`, `pub fn best_bid(&self) -> Option<Level>`, `pub fn best_ask(&self) -> Option<Level>`, `pub fn bids(&self) -> Vec<Level>` (highest first), `pub fn asks(&self) -> Vec<Level>` (lowest first), `pub fn levels(&self, outcome: Outcome) -> (Vec<Level>, Vec<Level>)` (that outcome's bids highest first and asks lowest first; `Outcome` from `crate::predictions::key`).

**Story 7 → Story 9.** `polyoxide-kalshi/examples/read_soak.rs`, run as `cargo run -p polyoxide-kalshi --example read_soak --features predictions -- --stages <stages> --routes <routes>`; the `### Rate limits` subsection of `OBSERVED.md`'s Epic 9 section (Epic 8's `## Rate limits` stays as it is, but for its bullet saying the anonymous soak is deferred, which Story 7 rewrites to point at the Epic 9 record), which ends with the line `<!-- rate limits end -->`.

**Story 8 → Story 9.** The umbrella's features `kalshi`, `kalshi-predictions`, `kalshi-predictions-ws`; `polyoxide::kalshi`; the prelude names of spec D13 (`Predictions`, `PredictionsBuilder`, `PredictionsError`, `KalshiCredentials`, `PredictionsEvent`, `PredictionsWsBuilder`, `SupervisedPredictionsWs`, `PredictionsWsError`, `PredictionsBooks`).

**Commit messages.** Every story's commit message ends with `(Epic 9, Story <n>)`, which Story 9's Task 9.6 greps for to find each story's commit.

**Who edits the shared docs.** Story 8 changes `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md` and `docs/ARCHITECTURE.md` only through `python3 scripts/gen_registry.py --write`, inside generated regions; every hand-written line of those files is Story 9's, so Story 9's edits target today's hand-written text with no "if not already rewritten" condition. The root `Cargo.toml`'s `[workspace.metadata.polyoxide.mirrors.kalshi]` `description` and `covers` strings are Story 8's, and Story 9 does not edit the root `Cargo.toml`. The "walking skeleton" sentence in `polyoxide-kalshi/src/predictions/mod.rs`'s module docs is Story 8's, with the crate's other "walking skeleton" wording.

**Anchors in `polyoxide-kalshi/Cargo.toml`** (existing lines, quoted verbatim; a story inserts directly before or after one and never quotes another story's text):

- `publish = false` (Story 8 deletes it);
- the `description = "…"` line (Story 8 replaces it);
- `[package.metadata.polyoxide]` (Story 7 inserts its `[[example]]` stanza directly before the comment line above it, `# Registration, read by scripts/gen_registry.py; run it with --write after an edit.`);
- the `polyoxide = { path = "../polyoxide", … }` dev-dependency line (Story 8 replaces it);
- the `readme = "…"` line under `[package.metadata.polyoxide]` (Story 8 replaces it).

**Design stage:** /flow-draft session ca16c18d-518d-45fe-ab26-1a23defc316e, 2026-10-21T07:40:13Z to 2026-10-21T08:51:44Z UTC: $40.35 logged, $47.18 with estimated thinking; 16010 plan lines, $2.52 per 1,000 lines.

## Stories

- **Story 1** — Probes and captures for the gap fill (spec §3, §1.6).

  Extends the two capture scripts and runs them: the candlestick fixtures from production, unsigned, and the socket frames from the demo host, signed (a three-channel subscribe, a two-`sid` update, `get_snapshot` inside and outside the subscription, an empty `ticker` subscribe stopped after a few frames). Writes the `## Epic 9` section of `OBSERVED.md`, with spec §1.6's probe and every finding, ending with the anchor, and the touch-set gate every later story ends with. No Rust. A finding that contradicts D6 or D9 goes to the owner before Stories 3 and 5.

  - **Developer:** intermediate-dev
  - **Files:** `scripts/capture_kalshi_predictions.py`, `scripts/capture_kalshi_predictions_ws.py`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/batch_1.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/batch_60.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/batch_1440.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/series_60.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/error_width.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/PROVENANCE.md`, `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed_ticker.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed_trade.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/ticker.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/trade.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/snapshot_answer.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/deltas_after_snapshot.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/error_snapshot_outside.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/multi_sid_answer.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/PROVENANCE.md`, `docs/specs/kalshi/OBSERVED.md`, `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt`, `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Interfaces:** exposes the touch-set gate, the `OBSERVED.md` anchor and the fixtures under "Story 1 → Stories 2 to 9"; uses nothing from earlier stories
  - **Verify:** `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-kalshi/tests/fixtures/predictions/**/*.json', recursive=True)]" && grep -qx '<!-- epic 9 findings go above this line -->' docs/specs/kalshi/OBSERVED.md && cargo test -p polyoxide-kalshi --all-features && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** light (scripts, captures and records; no library code)
  - **Reading:** spec §1.4, §1.6, §3, §13 (the touch set), §15; `scripts/capture_kalshi_predictions.py`, `scripts/capture_kalshi_predictions_ws.py`, `scripts/capture_common.py`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt`, `polyoxide-kalshi/tests/fixtures/predictions/PROVENANCE.md`, `polyoxide-kalshi/tests/fixtures/predictions/ws/PROVENANCE.md`; `docs/specs/kalshi/OBSERVED.md` (its headings and its last 40 lines); `docs/specs/kalshi/asyncapi.yaml` (the `ticker` and `trade` channels, `tickerPayload`, `tradePayload`, `updateSubscriptionCommandPayload`)

- **Story 2** — `Candles::intervals()` and the `UnservedInterval` refusal in `polyoxide-venue` (spec D2, D3).

  Adds the provided method, the trait docs' refusal sentence, `CandleQuery::ensure_served` and `UnservedInterval` with its `Classify` impl and its place in the crate's assertion. No other venue changes: the CLOB, perps and USDⓈ-M keep the default. The tests prove the default, an override seen through `Arc<DynCandles<'static>>`, the refusal and the accepted width, and the error's class, fault and message. If dynosaur cannot forward the provided method, the story stops and asks. The docs and tests name no venue.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/records.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/tests/candles_intervals.rs`
  - **Interfaces:** exposes "Story 2 → Stories 3, 8 and 9"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-venue --all-features && cargo test -p polyoxide-venue --features market --test candles_intervals && python3 scripts/venue_gate.py && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** deep (a foundation crate's public trait)
  - **Reading:** spec §2 (D2, D3), §4, §13; `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/records.rs:220-360`, `polyoxide-venue/src/class.rs:150-230`, `polyoxide-venue/src/key.rs:360-430` (`KeyError`'s `Classify` as the model), `polyoxide-venue/src/lib.rs`, `polyoxide-venue/tests/trading.rs:1-80`

- **Story 3** — Kalshi's `Candles` over the batch candlestick route (spec D4 to D6, §5).

  Adds both candle routes to the route table with their costs, the native builders and query setters, `PeriodInterval` and the candlestick types (every `price` field and `previous_dollars` an `Option`, since the venue omits them), and `impl Candles for Predictions` with `CandleExt` and `Ohlc`: the span shifted by one width because Kalshi selects by a candle's end, the trade-price OHLC, a `no` key mirrored with high and low swapped, an untraded period closed at `previous`, a period with no `previous` skipped, and an unserved width refused before any request. The suites cover both routes on a mock server, the setters' exact pairs, spec and wire agreement on Story 1's fixtures, each D6 rule in `venue_predictions`, and the three widths live through the trait.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs`, `polyoxide-kalshi/src/predictions/types/markets.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/venue.rs`, `polyoxide-kalshi/tests/mock_predictions_routes.rs`, `polyoxide-kalshi/tests/query_setters_predictions.rs`, `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions.rs`, `polyoxide-kalshi/tests/venue_predictions.rs`, `polyoxide-kalshi/tests/live_predictions_venue.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `docs/specs/kalshi/OBSERVED.md`
  - **Interfaces:** exposes "Story 3 → Stories 8 and 9"; uses Story 1's candlestick fixtures and anchor and Story 2's `intervals`, `ensure_served` and `UnservedInterval`
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored && python3 scripts/live_unwraps.py && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** deep (the price mapping every candle consumer reads)
  - **Reading:** spec §1.6, §2 (D4 to D6), §5; `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs`, `polyoxide-kalshi/src/predictions/types/markets.rs`, `polyoxide-kalshi/src/predictions/venue.rs`, `polyoxide-kalshi/tests/venue_predictions.rs:1-140`, `polyoxide-kalshi/tests/mock_predictions_routes.rs:1-80`, `polyoxide-kalshi/tests/query_setters_predictions.rs:1-60`, `polyoxide-kalshi/tests/spec_agreement_predictions.rs:1-80`, `polyoxide-kalshi/tests/wire_agreement_predictions.rs:1-80`, `polyoxide-kalshi/tests/live_predictions_venue.rs`; `polyoxide-polymarket/src/clob/venue.rs` (its `candles`), `polyoxide-binance/src/usdm/venue.rs` (its `candles` and `CandleExt`); `docs/specs/kalshi/openapi.yaml` (`/markets/candlesticks`, `/series/{series_ticker}/markets/{ticker}/candlesticks`, `MarketCandlestick`, `BidAskDistribution`, `PriceDistribution`, `MarketCandlesticksResponse`, `BatchGetMarketCandlesticksResponse`)

- **Story 4** — `Protocol::follow_up` in the socket kit (spec D7, §6).

  Adds the provided method to `Protocol` and, in the supervisor task, the call after every `decode`, sending what it returns on the same connection bounded by `stale_after`, a failed or timed-out send handled as a failed ping. The tests, appended to `src/supervisor/tests.rs`, pin that a returned frame is sent after the decode that prompted it, that `None` sends nothing, and that a send outliving `stale_after` reconnects. The docs and tests name no venue.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/supervisor/tests.rs`
  - **Interfaces:** exposes "Story 4 → Story 5"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-ws --all-features --lib supervisor && cargo test -p polyoxide-ws --all-features && python3 scripts/venue_gate.py && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** deep (a foundation crate's supervision loop)
  - **Reading:** spec §2 (D7), §6; `polyoxide-ws/src/supervisor/mod.rs:150-340`, `polyoxide-ws/src/supervisor/task.rs:200-260,480-620`, `polyoxide-ws/src/supervisor/tests.rs` (its first 120 and last 120 lines)

- **Story 5** — The socket protocol: the `ticker` and `trade` channels, `get_snapshot` and the gap re-snapshot (spec D8, D9, §7).

  Adds `Channel` and `PredictionsWsBuilder::channels`, a `sid` per channel from each `subscribed` answer, one replayed `subscribe` naming every chosen channel, one `update_subscription` per `sid` for a change, and the empty-set rule for every channel. Decodes `TickerUpdate` and `PublicTrade`. A gap on the `trade` `sid` reconnects. A gap on the order-book `sid` yields `Resyncing`, sends `get_snapshot` for the membership through Story 4's `follow_up`, drops that `sid`'s deltas per ticker until each snapshot arrives, re-anchors on each answering snapshot's `seq`, and reconnects on `SnapshotTimeout`, on an error answering the follow-up, or on a second gap. Adds `Change::Snapshot` and `PredictionsMembership::snapshot`. Every edit to `ws/protocol.rs`, `event.rs`, `error.rs` and `builder.rs` in this epic is this story's.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/event.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `docs/specs/kalshi/OBSERVED.md`
  - **Interfaces:** exposes "Story 5 → Stories 6, 8 and 9"; uses Story 1's ws fixtures and anchor and Story 4's `Protocol::follow_up`
  - **Verify:** `cargo test -p polyoxide-kalshi --all-features && cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws -- --ignored && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** deep (sequence recovery and the empty-subscription rule)
  - **Reading:** spec §2 (D8, D9), §7; Epic 8's spec §8 (D13 to D16); `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/event.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs:1-160`, `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `polyoxide-ws/src/supervisor/mod.rs:150-340`; `docs/specs/kalshi/asyncapi.yaml` (`tickerPayload`, `tradePayload`, `updateSubscriptionCommandPayload`, the `orderbook_delta` channel's description)

- **Story 6** — The local order book, `ws::book` (spec D10, §8).

  Adds `Books` and `OrderBook`, a pure value the consumer feeds Story 5's events into: a snapshot replaces a ticker's YES-priced ladder, a delta adds its signed size and removes a level at zero or below, a delta for a ticker with no snapshot is ignored, `Resyncing` drops its tickers' books and `Reconnected` drops all. `levels(outcome)` gives one outcome's view, a NO bid at *q* being a YES ask at 1 − *q*. No I/O and no clock. Unit tests, in the file, replay Epic 8's captured `ws/snapshot.json` and `ws/deltas.json` and assert literal prices and sizes. Replaces `ws/mod.rs`'s module block with Story 6's.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/src/predictions/ws/book.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`
  - **Interfaces:** exposes "Story 6 → Stories 8 and 9"; uses Story 5's `Event`, `OrderbookSnapshot`, `OrderbookDelta` and module block, Epic 8's `ws/snapshot.json` and `ws/deltas.json`, and `crate::predictions::key::Outcome` and `crate::predictions::types::BookSide`
  - **Verify:** `cargo test -p polyoxide-kalshi --all-features --lib book && cargo test -p polyoxide-kalshi --all-features && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** light (a pure data structure with literal-value tests)
  - **Reading:** spec §2 (D10), §8; `polyoxide-kalshi/src/predictions/venue.rs:1-130` (the YES/NO conversion), `polyoxide-kalshi/src/predictions/ws/event.rs:1-160`, `polyoxide-kalshi/src/predictions/key.rs:110-160`, `polyoxide-kalshi/tests/fixtures/predictions/ws/snapshot.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/deltas.json`

- **Story 7** — Measured read limits: the unsigned soak and the signed-bucket probe (spec D11, §9).

  Writes `examples/read_soak.rs` on test-support's soak helpers over distinct URLs, each path probed first, and runs it on production up to and past Basic's 200/s; then runs the signed probe on the demo host (stop and ask without `KALSHI_DEMO_*`). Records every run in `OBSERVED.md`'s `## Rate limits`. Sizes the unsigned client from the measured clean rate with a tenth reserved (`RESERVED_FRACTION`), or confirms Basic's sizing, and moves a keyed client's public reads only if the probe shows they do not draw from the key's bucket, each change pinned by an appended test.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/tests/mock_predictions_signed.rs`, `docs/specs/kalshi/OBSERVED.md`, `Cargo.lock`
  - **Interfaces:** exposes "Story 7 → Story 9"; uses Story 1's anchor and the `[package.metadata.polyoxide]` anchor
  - **Verify:** `cargo build -p polyoxide-kalshi --features predictions --example read_soak && cargo test -p polyoxide-kalshi --no-default-features --features predictions && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** deep (RESERVED_FRACTION: a rate-limit sizing measured against the live host)
  - **Reading:** spec §2 (D11), §9; Epic 8's spec D3 and D5; `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/tests/mock_predictions_signed.rs:1-120`, `polyoxide-polymarket/examples/info_soak.rs`, `polyoxide-polymarket/examples/v2_soak/main.rs`, `polyoxide-test-support/src/soak/mod.rs`, `polyoxide-test-support/src/soak/observe.rs`, `polyoxide-core/src/rate_limit.rs` (`RESERVED_FRACTION`), `polyoxide-core/src/capacity.rs`; `docs/specs/kalshi/OBSERVED.md` (its rate-limit notes); CLAUDE.md's "A bucket's depth and its refill rate" and "The published count is reachable as a burst" paragraphs

- **Story 8** — Publish `polyoxide-kalshi` and re-export it from the umbrella (spec D12 to D14, §10).

  Drops `publish = false` and the "walking skeleton" wording, gives the workspace pin a version, and adds the umbrella's `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws` features (all in `full`), `polyoxide::kalshi`, `parse_key`'s Kalshi arm, the prelude block with D13's aliases, and the `const _` pins. The venue-neutral example gains a Kalshi arm in `main` with no `cfg`, so Kalshi's own `#[path]` include keeps compiling, and Kalshi's `polyoxide` dev-dependency gains `kalshi-predictions`. Adds the umbrella's Kalshi allowlists and rules, drops the real-tree Kalshi exemption in `test_ci_workflow.py`, regenerates the registry, and runs the package, cargo-hack and full header-pin steps.

  - **Developer:** intermediate-dev
  - **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/README.md`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/README.md`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`, `polyoxide/examples/venue_neutral.rs`, `polyoxide/tests/venue_neutral.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`, `ci/dep-allowlists/polyoxide@kalshi-predictions.txt`, `ci/dep-allowlists/polyoxide@kalshi-predictions-ws.txt`, `.github/scripts/tests/test_dep_allowlists.py`, `.github/scripts/tests/test_ci_workflow.py`, `.github/workflows/ci.yml`, `README.md`, `CLAUDE.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml`
  - **Interfaces:** exposes "Story 8 → Story 9"; uses Story 3's `Candles` impl, Story 5's and Story 6's public names for the prelude, and the Cargo.toml anchors
  - **Verify:** `python3 scripts/publish_order.py check-manifests && cargo publish --workspace --dry-run --no-verify --locked && cargo test -p polyoxide --features full && cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions && cargo hack check -p polyoxide -p polyoxide-kalshi --each-feature --no-dev-deps && cargo test -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_ci_workflow.py tests/test_gen_registry.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** deep (publishes a crate and changes CI gates)
  - **Reading:** spec §2 (D12 to D14), §10; `Cargo.toml:1-40`, `polyoxide-kalshi/Cargo.toml`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/examples/venue_neutral.rs`, `polyoxide/tests/venue_neutral.rs:1-130`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`, `polyoxide-kalshi/tests/venue_predictions.rs:1-30`, `.github/scripts/tests/test_dep_allowlists.py:1-160`, `.github/scripts/tests/test_ci_workflow.py:270-420`, `scripts/publish_order.py` (`check-manifests`), `.github/workflows/ci.yml:106-166`

- **Story 9** — Docs, records and amendments (spec D15, §11).

  Brings the hand-written docs up to a published Kalshi: CLAUDE.md's venue lists and paragraphs (D15's list, `Candles::intervals()`, the socket's channels, book and re-snapshot, "Today's order"), the README, `docs/specs/INDEX.md` and `SELF-HEALING.md` lead-ins, `.env.example`, a Kalshi vacuity guard in `test_classify_coverage.py`, and `docs/s3-migration.md`'s notes for the release. Writes `spine-amendments/epic-9.md` (A9-1 to A9-4 and any gap the run met) and closes the `deferred-work.md` entries this epic settled, each with its commit. Generated regions only through `gen_registry.py --write`.

  - **Developer:** intermediate-dev
  - **Files:** `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `.env.example`, `docs/specs/INDEX.md`, `docs/s3-migration.md`, `.github/scripts/tests/test_classify_coverage.py`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md`, `_bmad-output/implementation-artifacts/deferred-work.md`
  - **Interfaces:** uses every public name above for the docs; exposes nothing
  - **Verify:** `python3 scripts/gen_registry.py --check && cargo test --doc -p polyoxide-kalshi -p polyoxide --all-features && bash -c 'cd .github/scripts && uv run pytest tests/ -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
  - **Review:** light (docs and records)
  - **Reading:** spec §2 (D15), §11, §14; `_bmad-output/implementation-artifacts/deferred-work.md` (the Kalshi entries), `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md`, `docs/s3-migration.md`, `.github/scripts/tests/test_classify_coverage.py`, `.env.example`; CLAUDE.md's paragraphs D15 names

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->

## Story 1: Probes and captures for the gap fill (spec §3, §1.6)

- **Developer:** intermediate-dev
- **Files:** `scripts/capture_kalshi_predictions.py`, `scripts/capture_kalshi_predictions_ws.py`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/batch_1.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/batch_60.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/batch_1440.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/series_60.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/error_width.json`, `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/PROVENANCE.md`, `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed_ticker.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed_trade.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/ticker.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/trade.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/snapshot_answer.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/deltas_after_snapshot.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/error_snapshot_outside.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/multi_sid_answer.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/PROVENANCE.md`, `docs/specs/kalshi/OBSERVED.md`, `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt`, `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Interfaces:** exposes the touch-set gate, the `OBSERVED.md` anchor and the fixtures under "Story 1 → Stories 2 to 9"; uses nothing from earlier stories
- **Verify:** `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-kalshi/tests/fixtures/predictions/**/*.json', recursive=True)]" && grep -qx '<!-- epic 9 findings go above this line -->' docs/specs/kalshi/OBSERVED.md && cargo test -p polyoxide-kalshi --all-features && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** light (scripts, captures and records; no library code)

Extends the two capture scripts and runs them: the candlestick fixtures from production, unsigned, and the socket frames from the demo host, signed (a three-channel subscribe, a two-`sid` update, `get_snapshot` inside and outside the subscription, an empty `ticker` and `trade` subscribe stopped after a few frames). Writes the `## Epic 9` section of `OBSERVED.md`, with spec §1.6's probe and every finding, ending with the anchor, and the touch-set gate every later story ends with. No Rust. Each capture mode has a `--check…` twin that re-reads the files on disk and fails on what is wrong, which is the task's test. A finding that contradicts D6 or D9 goes to the owner before Stories 3 and 5; the socket half stops and asks the owner when `KALSHI_DEMO_API_KEY_ID` or `KALSHI_DEMO_PRIVATE_KEY` is absent, and no fixture is ever written by hand.

### Task 1.1: The touch-set gate

Every story of the epic ends with this gate, so it lands first. It prints each path the branch has changed since `git merge-base main HEAD`, staged, unstaged or untracked, that the allowlist does not admit, and it fails when it prints one. The shell script is Epic 8's with `epic-8` replaced by `epic-9`; the allowlist admits exactly spec §13's paths, the plan and `.flow/`.

**Files:**
- Create: `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt`
- Create: `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`

- [ ] **Step 1: Write the failing check.** The check is the gate itself, run three ways: on the branch as it stands (it must pass), with an untracked file outside the allowlist (it must print that file and fail), and with the allowlist missing (it must fail closed). There is no file to write for it.

- [ ] **Step 2: Run it to see it fail.**

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: `bash: _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh: No such file or directory`, then `exit 127`.

- [ ] **Step 3: Implement.** Create the allowlist, one extended regular expression per line, these and no others.

`_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt`:

```text
^Cargo\.toml$
^Cargo\.lock$
^polyoxide-kalshi/
^polyoxide-venue/src/
^polyoxide-venue/tests/
^polyoxide-ws/src/supervisor/
^polyoxide/
^docs/specs/kalshi/
^scripts/capture_kalshi_[a-z_]+\.py$
^ci/dep-allowlists/polyoxide-kalshi(@[a-z0-9,-]+)?\.txt$
^ci/dep-allowlists/polyoxide@kalshi[a-z0-9,-]*\.txt$
^\.github/scripts/tests/test_dep_allowlists\.py$
^\.github/scripts/tests/test_ci_workflow\.py$
^\.github/scripts/tests/test_classify_coverage\.py$
^CLAUDE\.md$
^README\.md$
^SELF-HEALING\.md$
^\.env\.example$
^docs/specs/INDEX\.md$
^docs/ARCHITECTURE\.md$
^\.github/workflows/nightly-behavioral\.yml$
^\.github/workflows/nightly-schema\.yml$
^\.github/workflows/ci\.yml$
^docs/s3-migration\.md$
^docs/superpowers/specs/2026-10-19-epic-9-kalshi-published-design\.md$
^docs/plans/2026-10-19-epic-9-kalshi-published\.md$
^_bmad-output/specs/
^_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9\.md$
^_bmad-output/implementation-artifacts/deferred-work\.md$
^\.flow/
```

Create the gate. It does not set `pipefail`, so `test` reads `grep`'s status: `grep -v` exits 0 when it printed a path the allowlist does not admit, 1 when it printed none, and 2 when it could not run, and only 1 passes.

`_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`:

```bash
#!/usr/bin/env bash
cd "$(git rev-parse --show-toplevel)" || exit 2
{ git diff --no-renames --name-only "$(git merge-base main HEAD)"; git ls-files --others --exclude-standard; } | sort -u | grep -v -E -f "$(dirname "$0")/epic-9-touch-set.txt"; test $? -eq 1
```

- [ ] **Step 4: Run the check three ways.**

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`. The branch's changes since the merge base are the spec, the plan, `.flow/` and the two new files, all admitted.

Run: `touch touch-set-control.txt && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"; rm touch-set-control.txt`
Expected: `touch-set-control.txt`, then `exit 1`.

Run: `mv _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt.aside; bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"; mv _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt.aside _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt`
Expected: a `grep: …/epic-9-touch-set.txt: No such file or directory` line, then `exit 1`: a missing allowlist fails the gate. `git status --short` shows the two new files (and `.flow/`, if untracked) and nothing else.

- [ ] **Step 5: Commit.**

```bash
git add -N _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh
git commit -m "chore(epic-9): the touch-set gate every story ends with (Epic 9, Story 1)" -- _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh
```

### Task 1.2: The `## Epic 9` section and its anchor

`docs/specs/kalshi/OBSERVED.md` ends with Epic 8's anchor, `<!-- capture findings go above this line -->`, which the socket script still writes above. This task appends the Epic 9 section below it: spec §1.6's probe as the design session made it, and the anchor line every later story writes directly above. The two capture scripts insert their generated blocks above that anchor in the next tasks.

**Files:**
- Modify: `docs/specs/kalshi/OBSERVED.md` (append after the last line)

- [ ] **Step 1: Write the failing check.** The anchor test is the story's own **Verify** clause.

- [ ] **Step 2: Run it to see it fail.**

Run: `grep -qx '<!-- epic 9 findings go above this line -->' docs/specs/kalshi/OBSERVED.md; echo "exit $?"`
Expected: `exit 1`.

- [ ] **Step 3: Implement.** Replace the file's last line, quoted exactly, with itself followed by the new section. The old text is:

```markdown
<!-- capture findings go above this line -->
```

The new text (the first line is the old line, kept; the section ends with the new anchor, which is the file's last line, followed by one newline):

```markdown
<!-- capture findings go above this line -->

## Epic 9

Findings of Epic 9's Story 1 and of the stories after it. Each is dated and names its host. A block between `kalshi-…-capture:begin` and `…:end` markers is written by the capture script that names it, and the script's next run replaces it. A later story inserts its findings directly above the last line of this section.

### The design session's probe of the candlestick routes (production, unsigned, 2026-10-19)

`GET /trade-api/v2/markets/candlesticks` for `KXMLBGAME-26OCT192008CLETB-TB` at `period_interval` 1, 60 and 1440 answered `200` with `{markets: [{market_ticker, candlesticks}]}`. In a traded period `price` held `open_dollars`, `high_dollars`, `low_dollars`, `close_dollars`, `mean_dollars` and `previous_dollars`; the first traded period had no `previous_dollars`. An untraded period after a trade held only `previous_dollars`, and a period before any trade held `price: {}`. `min_dollars` and `max_dollars` never appeared. `period_interval=5` answered `200` with no rows on the batch route and `400` (a validation `msg`) on `GET /series/{series_ticker}/markets/{ticker}/candlesticks`. Hourly candles end on the hour; the daily candle ended at 04:00 UTC. The captures below re-measure each of these.

<!-- epic 9 findings go above this line -->
```

- [ ] **Step 4: Run it to see it pass.**

Run: `grep -qx '<!-- epic 9 findings go above this line -->' docs/specs/kalshi/OBSERVED.md; echo "exit $?"; tail -n 1 docs/specs/kalshi/OBSERVED.md`
Expected: `exit 0`, then the anchor line itself.

- [ ] **Step 5: Commit.**

```bash
git commit -m "docs(kalshi): the Epic 9 section of OBSERVED.md and its anchor (Epic 9, Story 1)" -- docs/specs/kalshi/OBSERVED.md
```

### Task 1.3: The candlestick fixtures

`scripts/capture_kalshi_predictions.py` gains `--candlesticks OUT_DIR [--ticker T]`, which reads production unsigned, picks an open market whose answer holds every kind of period and writes `OUT_DIR/candlesticks/`, and `--check-candlesticks OUT_DIR`, which re-reads what is on disk. The default mode (`OUT_DIR` alone) is untouched, so Epic 8's fixtures are never recaptured by accident. A period is one of three kinds: **traded** (`price.close_dollars` present), **untraded** (no close, `price.previous_dollars` present) and **empty** (neither). The picked market has traded and untraded periods at all three widths and, when one of the first 60 candidates has it, an empty period before its first trade. The run also probes what no fixture holds, a width outside the enum on both routes and a batch past 10,000 rows, and writes every finding into `OBSERVED.md` above the Epic 9 anchor, between its own markers.

**Files:**
- Modify: `scripts/capture_kalshi_predictions.py`
- Create (captured): `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/batch_1.json`, `batch_60.json`, `batch_1440.json`, `series_60.json`, `error_width.json`, `PROVENANCE.md`
- Modify: `docs/specs/kalshi/OBSERVED.md` (written by the script, above the anchor)

- [ ] **Step 1: Write the failing check.** Three edits to the script, none of which captures anything yet.

First, extend the module docstring. Replace

```python
sees each field the host sends. Review the diff before committing.
"""
import sys
from pathlib import Path

import capture_common
```

with

```python
sees each field the host sends. Review the diff before committing.

Epic 9 adds two modes, each with an option first:

    uv run scripts/capture_kalshi_predictions.py --candlesticks polyoxide-kalshi/tests/fixtures/predictions [--ticker T]
    uv run scripts/capture_kalshi_predictions.py --check-candlesticks polyoxide-kalshi/tests/fixtures/predictions

`--candlesticks` reads `GET /markets/candlesticks` at periods 1, 60 and 1440 for one open market, the
series route at 60, the series route at 5 (its 400), and the batch route at 5 and over 10,000 rows
(probes, kept only in OBSERVED.md), and writes OUT_DIR/candlesticks/ and a block of OBSERVED.md above
its Epic 9 anchor. `--check-candlesticks` makes no request: it re-reads the files and exits 1 naming
what is wrong. Neither mode touches the other fixtures.
"""
import itertools
import json
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path

import capture_common
```

Second, add the constants. Directly after

```python
RATE_LIMITED_BODY = {"error": "too many requests"}
```

insert

```python

OBSERVED = Path(__file__).resolve().parent.parent / "docs" / "specs" / "kalshi" / "OBSERVED.md"
EPIC9_ANCHOR = "<!-- epic 9 findings go above this line -->"
CANDLES_BEGIN = "<!-- kalshi-candlesticks-capture:begin -->"
CANDLES_END = "<!-- kalshi-candlesticks-capture:end -->"
# The batch route's period widths in minutes, and the file each is written to.
WIDTHS = (1, 60, 1440)
BATCH_FILES = {"batch_1.json": 1, "batch_60.json": 60, "batch_1440.json": 1440}
CANDLE_FILES = (*BATCH_FILES, "series_60.json", "error_width.json")
# How far back from now each width is read, in seconds, never before the market's open.
SPAN_SECONDS = {1: 2 * 3600, 60: 7 * 86400, 1440: 120 * 86400}
# A width the enum lacks, sent to both routes.
WRONG_WIDTH = 5
# Candidates to try before giving up, tickers in the over-limit probe, and its span.
CANDLE_CANDIDATES = 60
BATCH_TICKERS = 100
OVER_LIMIT_SECONDS = 3 * 3600
BID_ASK_KEYS = {"open_dollars", "low_dollars", "high_dollars", "close_dollars"}
```

Third, insert the check, and the dispatch it needs, directly above `def main():`. The check is the story's test: it reads the five files and `PROVENANCE.md`, and `OBSERVED.md` for the capture's block.

```python
def kind(row):
    """`traded` when the period has a close, `untraded` when it has only `previous_dollars`, else
    `empty`."""
    price = row.get("price") or {}
    if price.get("close_dollars") is not None:
        return "traded"
    if price.get("previous_dollars") is not None:
        return "untraded"
    return "empty"


def check_candlesticks(root):
    """Re-reads root/candlesticks/ and exits 1 naming each thing that is wrong."""
    folder = root / "candlesticks"
    problems = []

    def load(name):
        try:
            return json.loads((folder / name).read_text(encoding="utf-8"))
        except (OSError, ValueError) as err:
            problems.append(f"{name}: {err}")
            return None

    tickers = set()
    for name, width in BATCH_FILES.items():
        body = load(name)
        if body is None:
            continue
        markets = body.get("markets") if isinstance(body, dict) else None
        if not isinstance(markets, list) or len(markets) != 1:
            problems.append(f"{name}: expected exactly one entry in `markets`")
            continue
        tickers.add(markets[0].get("market_ticker"))
        rows = markets[0].get("candlesticks")
        if not isinstance(rows, list) or not all(
            isinstance(row.get("end_period_ts"), int) and isinstance(row.get("price"), dict) for row in rows
        ):
            problems.append(f"{name}: `candlesticks` is not a list of rows with an integer `end_period_ts` and a `price` object")
            continue
        kinds = Counter(kind(row) for row in rows)
        if not kinds["traded"]:
            problems.append(f"{name}: no traded period")
        if not kinds["untraded"]:
            problems.append(f"{name}: no untraded period carrying `previous_dollars`")
        modulus = 86400 if width == 1440 else width * 60
        offsets = {row["end_period_ts"] % modulus for row in rows}
        if len(offsets) > 1:
            problems.append(f"{name}: periods end at different offsets of the width: {sorted(offsets)}")
    if len(tickers) > 1:
        problems.append(f"the batch files name different markets: {sorted(map(str, tickers))}")

    series = load("series_60.json")
    if series is not None:
        if not isinstance(series, dict) or set(series) != {"candlesticks", "ticker"}:
            problems.append("series_60.json: expected exactly the keys `candlesticks` and `ticker`")
        elif tickers and series["ticker"] not in tickers:
            problems.append(f"series_60.json: ticker {series['ticker']!r} is not the batch files' {sorted(map(str, tickers))}")
    refusal = load("error_width.json")
    if refusal is not None and not (isinstance(refusal, dict) and refusal):
        problems.append("error_width.json: expected a non-empty JSON object")

    try:
        provenance = (folder / "PROVENANCE.md").read_text(encoding="utf-8")
    except OSError as err:
        problems.append(f"PROVENANCE.md: {err}")
    else:
        absent = [name for name in CANDLE_FILES if name not in provenance]
        if absent:
            problems.append("PROVENANCE.md does not mention " + ", ".join(absent))
    try:
        observed = OBSERVED.read_text(encoding="utf-8")
    except OSError as err:
        problems.append(f"{OBSERVED}: {err}")
    else:
        if CANDLES_BEGIN not in observed or CANDLES_END not in observed:
            problems.append(f"{OBSERVED} holds no block between {CANDLES_BEGIN} and {CANDLES_END}")

    for problem in problems:
        print(f"  {problem}")
    if problems:
        raise SystemExit(f"{len(problems)} problem(s) in {folder}")
    print(f"ok: {len(CANDLE_FILES)} candlestick fixtures in {folder}")


def epic9_command(args):
    """Runs one of Epic 9's two modes; `args` is `sys.argv[1:]`, whose first item starts with `--`."""
    if len(args) == 2 and args[0] == "--check-candlesticks":
        check_candlesticks(Path(args[1]))
        return
    sys.exit(__doc__)


```

Fourth, make `main` call it. Replace

```python
def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
```

with

```python
def main():
    args = sys.argv[1:]
    if args and args[0].startswith("--"):
        epic9_command(args)
        return
    if len(args) != 1:
        sys.exit(__doc__)
    out = Path(args[0])
```

- [ ] **Step 2: Run it to see it fail.**

Run: `nix develop --command uv run scripts/capture_kalshi_predictions.py --check-candlesticks polyoxide-kalshi/tests/fixtures/predictions; echo "exit $?"`
Expected: one indented line per missing file (`batch_1.json: [Errno 2] No such file or directory: …`, likewise `batch_60.json`, `batch_1440.json`, `series_60.json`, `error_width.json`, and `PROVENANCE.md: …`), then `OBSERVED.md` holds no block (so seven lines), then `7 problem(s) in polyoxide-kalshi/tests/fixtures/predictions/candlesticks` and `exit 1`.

- [ ] **Step 3: Implement.** Insert the capture directly above `def kind(row):`, and replace `epic9_command` with the two-mode version.

The capture. `fetch`, `fetch_ok`, `refused`, `is_two_sided` and `listing_rows` are the script's own, above.

```python
def opened(market):
    """The market's `open_time` as Unix seconds."""
    return int(datetime.fromisoformat(market["open_time"].replace("Z", "+00:00")).timestamp())


def fmt_ts(ts):
    return datetime.fromtimestamp(ts, timezone.utc).strftime("%Y-%m-%d %H:%MZ")


def rows_of(body):
    """The candlesticks of a batch answer's first market, or an empty list."""
    markets = body.get("markets") if isinstance(body, dict) else None
    return (markets[0].get("candlesticks") or []) if markets else []


def span(width, now, open_ts):
    """The seconds read at `width`: the last SPAN_SECONDS, but not before the market opened."""
    return max(now - SPAN_SECONDS[width], open_ts), now


def fetch_batch(tickers, width, start, end):
    return fetch("/markets/candlesticks", market_tickers=tickers, start_ts=start, end_ts=end, period_interval=width)


def candidates(wanted):
    """Open markets that quote both sides: the one asked for, else the filtered listing's rows."""
    if wanted:
        yield fetch_ok(f"/markets/{wanted}")["market"]
        return
    for row in listing_rows():
        if is_two_sided(row):
            yield row


def read_widths(market, now):
    """`(bodies, "")` with each width's `(start, end, body)` when the market has a traded and an
    untraded period at every width, else `(None, why)`."""
    bodies = {}
    for width in (60, 1, 1440):
        start, end = span(width, now, opened(market))
        body = capture_common.require_ok(
            fetch_batch(market["ticker"], width, start, end), f"GET /markets/candlesticks at {width}"
        )
        kinds = Counter(kind(row) for row in rows_of(body))
        if not (kinds["traded"] and kinds["untraded"]):
            return None, f"width {width} ({dict(kinds)})"
        bodies[width] = (start, end, body)
    return bodies, ""


def pick_candle_market(wanted, now):
    """The first candidate whose answers hold every kind of period, preferring one that also holds an
    empty period (a period before its first trade); else the first without one."""
    fallback = None
    for tried, market in enumerate(candidates(wanted), start=1):
        if tried > CANDLE_CANDIDATES:
            break
        bodies, why = read_widths(market, now)
        if bodies is None:
            if wanted:
                sys.exit(f"{wanted} lacks a traded or an untraded period at {why}: pass another --ticker")
            continue
        picked = (market, bodies)
        if any(kind(row) == "empty" for _s, _e, body in bodies.values() for row in rows_of(body)):
            return picked
        fallback = fallback or picked
    if fallback:
        return fallback
    sys.exit(
        f"none of the first {CANDLE_CANDIDATES} quoted open markets has a traded and an untraded period at "
        "every width; run again, or pass --ticker"
    )


def over_limit_probe(now):
    """One batch for up to BATCH_TICKERS open markets at width 1 over OVER_LIMIT_SECONDS, which is
    more than the route's 10,000 rows when the markets all have rows; what the host does with it."""
    tickers = [row["ticker"] for row in itertools.islice(listing_rows(), BATCH_TICKERS)]
    reply = fetch_batch(",".join(tickers), 1, now - OVER_LIMIT_SECONDS, now)
    info = {"tickers": len(tickers), "status": reply.status}
    if reply.is_json and isinstance(reply.body, dict) and isinstance(reply.body.get("markets"), list):
        counts = [len(entry.get("candlesticks") or []) for entry in reply.body["markets"]]
        info.update(
            keys=sorted(reply.body),
            markets=len(counts),
            rows=sum(counts),
            fewest=min(counts, default=0),
            most=max(counts, default=0),
        )
    else:
        info["body"] = str(reply.body)[:300]
    return info


def d6_problems(rows):
    """What in `rows` would contradict spec D6's reading of the wire: a distribution whose keys are
    not the four, a value that is not a decimal string, an `end_period_ts` that is not an integer."""
    found = []

    def decimal(value, where):
        if not isinstance(value, str):
            found.append(f"{where} is {value!r}, not a string")
            return
        try:
            Decimal(value)
        except InvalidOperation:
            found.append(f"{where} is {value!r}, not a decimal")

    for row in rows:
        if not isinstance(row.get("end_period_ts"), int):
            found.append(f"end_period_ts is {row.get('end_period_ts')!r}")
        for side in ("yes_bid", "yes_ask"):
            dist = row.get(side)
            if not isinstance(dist, dict) or set(dist) != BID_ASK_KEYS:
                found.append(f"{side} is {dist!r}")
                continue
            for key, value in dist.items():
                decimal(value, f"{side}.{key}")
        for key in ("volume_fp", "open_interest_fp"):
            decimal(row.get(key), key)
        for key, value in (row.get("price") or {}).items():
            if value is not None:
                decimal(value, f"price.{key}")
    return sorted(set(found))[:5]


def describe(width, start, end, rows):
    kinds = Counter(kind(row) for row in rows)
    ends = [row["end_period_ts"] for row in rows]
    if ends == sorted(ends):
        order = "ascending"
    elif ends == sorted(ends, reverse=True):
        order = "descending"
    else:
        order = "unordered"
    modulus = 86400 if width == 1440 else width * 60
    offsets = sorted({ts % modulus for ts in ends})
    shown = ", ".join(str(offset) for offset in offsets)
    if width == 1440:
        clock = ", ".join(f"{o // 3600:02d}:{o % 3600 // 60:02d} UTC" for o in offsets)
        shown = f"{shown} ({clock})"
    price_keys = sorted({key for row in rows for key in (row.get("price") or {})})
    return (
        str(width),
        f"{fmt_ts(start)} to {fmt_ts(end)}",
        str(len(rows)),
        f"{kinds['traded']} / {kinds['untraded']} / {kinds['empty']}",
        order,
        shown,
        ", ".join(f"`{key}`" for key in price_keys) or "none",
    )


def section(info, stamp):
    """The OBSERVED.md block for one run."""
    rows = info["rows"]
    every = [row for width in WIDTHS for row in rows[width]]
    table = [describe(width, *info["spans"][width], rows[width]) for width in WIDTHS]
    prices = [row.get("price") or {} for row in every]
    min_max = any("min_dollars" in price or "max_dollars" in price for price in prices)
    nulls = any(value is None for price in prices for value in price.values())
    extra = sorted({key for row in every if kind(row) == "untraded" for key in row["price"] if key != "previous_dollars"})
    empty = next((row for row in every if kind(row) == "empty"), None)
    firsts = []
    for width in WIDTHS:
        traded = sorted((r for r in rows[width] if kind(r) == "traded"), key=lambda r: r["end_period_ts"])
        has = "has" if "previous_dollars" in traded[0]["price"] else "lacks"
        start = info["spans"][width][0]
        where = "the span starts at the market's open" if start == info["open_ts"] else "the span starts after the market opened"
        firsts.append(f"{width}: the earliest traded period {has} `previous_dollars` ({where})")
    problems = d6_problems(every)
    if empty is None:
        empty_text = "No period before the market's first trade was in any span: this market has none to show."
    else:
        empty_text = (
            "A period before the first trade held `price` = "
            f"`{json.dumps(empty['price'])}`, `yes_bid` = `{json.dumps(empty['yes_bid'])}`, "
            f"`yes_ask` = `{json.dumps(empty['yes_ask'])}`, `volume_fp` = `{empty['volume_fp']}`."
        )
    series_same = info["series"]["candlesticks"] == rows[60]
    series_text = (
        "identical to the batch route's rows"
        if series_same
        else f"different from the batch route's rows ({len(info['series']['candlesticks'])} against {len(rows[60])})"
    )
    wrong = info["wrong"]
    over = info["over"]
    if "rows" in over:
        over_text = (
            f"`{over['status']}` with keys {', '.join('`' + k + '`' for k in over['keys'])}: {over['markets']} markets, "
            f"{over['rows']} rows in all ({over['fewest']} to {over['most']} each). "
            + ("That is within the route's 10,000." if over["rows"] <= 10000 else "That is past the route's 10,000: the host did not refuse it.")
        )
    else:
        over_text = f"`{over['status']}` with no market list: {over['body']}"
    lines = [
        "### Candlesticks, production, unsigned",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions.py --candlesticks` from `{BASE}` on `{info['ticker']}` "
        f"(series `{info['series_ticker']}`, opened {fmt_ts(info['open_ts'])}). Fixtures: "
        "`polyoxide-kalshi/tests/fixtures/predictions/candlesticks/`.",
        "",
        *capture_common.provenance_table(
            (
                "Width (min)",
                "Span (UTC)",
                "Rows",
                "Traded / untraded / empty",
                "Wire order by `end_period_ts`",
                "`end_period_ts` mod the width (s)",
                "`price` keys seen",
            ),
            table,
        ),
        "",
        f"- `min_dollars` or `max_dollars` in any `price`: {'yes' if min_max else 'never'}. A `null` in any `price`: {'yes' if nulls else 'never, the keys are omitted'}.",
        f"- An untraded period's `price` holds a key besides `previous_dollars`: {', '.join('`' + k + '`' for k in extra) if extra else 'no'}.",
        f"- {empty_text}",
        "- First traded period: " + "; ".join(firsts) + ".",
        f"- The series route at 60 answered `{sorted(info['series'])}`, {series_text}.",
        f"- Width {WRONG_WIDTH} on the batch route: `{wrong['status']}`, "
        + (f"{wrong['rows']} rows." if wrong["rows"] is not None else f"body `{wrong['body']}`."),
        f"- Width {WRONG_WIDTH} on the series route: `400`, body `{json.dumps(info['error_width'])}`.",
        f"- One batch for {over['tickers']} open markets at width 1 over {OVER_LIMIT_SECONDS // 3600} hours: {over_text}",
        "- Spec D6's reading of the wire (every `*_dollars` and `*_fp` a decimal string, `yes_bid` and `yes_ask` carrying exactly the four distribution keys, `end_period_ts` an integer): "
        + ("holds in every captured row." if not problems else "**DOES NOT HOLD: " + "; ".join(problems) + ". Report this to the owner before Story 3 runs.**"),
    ]
    return "\n".join(lines)


def record_candle_findings(text):
    """Writes `text` into OBSERVED.md between the capture's markers, replacing an earlier run's, or
    directly above the Epic 9 anchor when there is none."""
    current = OBSERVED.read_text(encoding="utf-8")
    if EPIC9_ANCHOR not in current:
        sys.exit(f"{OBSERVED} has no anchor line {EPIC9_ANCHOR!r}: Task 1.2 writes it")
    block = f"{CANDLES_BEGIN}\n{text.rstrip()}\n{CANDLES_END}"
    if CANDLES_BEGIN in current and CANDLES_END in current:
        head, rest = current.split(CANDLES_BEGIN, 1)
        _, tail = rest.split(CANDLES_END, 1)
        current = head + block + tail
    else:
        current = current.replace(EPIC9_ANCHOR, f"{block}\n\n{EPIC9_ANCHOR}", 1)
    capture_common.write_raw(OBSERVED, current)


def provenance_candles(info, stamp):
    ticker, series = info["ticker"], info["series_ticker"]
    host = "external-api.kalshi.com"
    rows = []
    for name, width in BATCH_FILES.items():
        start, end = info["spans"][width]
        request = f"GET /markets/candlesticks?market_tickers={ticker}&period_interval={width}&start_ts={start}&end_ts={end}"
        rows.append((f"`{name}`", host, f"`{request}`", f"{fmt_ts(start)} to {fmt_ts(end)}", stamp))
    start, end = info["spans"][60]
    path = f"/series/{series}/markets/{ticker}/candlesticks"
    rows.append(("`series_60.json`", host, f"`GET {path}?period_interval=60&start_ts={start}&end_ts={end}`", f"{fmt_ts(start)} to {fmt_ts(end)}", stamp))
    rows.append(("`error_width.json`", host, f"`GET {path}?period_interval={WRONG_WIDTH}&start_ts={start}&end_ts={end}` (the 400 body)", f"{fmt_ts(start)} to {fmt_ts(end)}", stamp))
    return [
        "# Provenance",
        "",
        f"Captured {stamp} from `https://{host}/trade-api/v2` by `scripts/capture_kalshi_predictions.py --candlesticks`.",
        "No credentials. Every key is kept and no list is trimmed.",
        "",
        f"The market is `{ticker}` (series `{series}`), picked from `markets?status=open&mve_filter=exclude` because",
        "it quoted both sides and its answers held a traded and an untraded period at every width. Each span starts",
        "at the later of the market's open and the last 2 hours (width 1), 7 days (width 60) or 120 days (width 1440).",
        "",
        *capture_common.provenance_table(("File", "Host", "Request", "Span (UTC)", "Captured"), rows),
    ]


def capture_candlesticks(out, wanted):
    now = int(time.time())
    market, bodies = pick_candle_market(wanted, now)
    ticker = market["ticker"]
    series_ticker = fetch_ok(f"/events/{market['event_ticker']}")["event"]["series_ticker"]
    start, end, _body = bodies[60]
    path = f"/series/{series_ticker}/markets/{ticker}/candlesticks"
    series = fetch_ok(path, start_ts=start, end_ts=end, period_interval=60)
    error_width = refused(path, 400, start_ts=start, end_ts=end, period_interval=WRONG_WIDTH)
    wrong = fetch_batch(ticker, WRONG_WIDTH, start, end)
    wrong_rows = rows_of(wrong.body) if wrong.is_json else None
    over = over_limit_probe(now)
    stamp = capture_common.stamp()
    info = {
        "ticker": ticker,
        "series_ticker": series_ticker,
        "open_ts": opened(market),
        "spans": {width: (s, e) for width, (s, e, _b) in bodies.items()},
        "rows": {width: rows_of(body) for width, (_s, _e, body) in bodies.items()},
        "series": series,
        "error_width": error_width,
        "wrong": {"status": wrong.status, "rows": None if wrong_rows is None else len(wrong_rows), "body": str(wrong.body)[:200]},
        "over": over,
    }
    folder = out / "candlesticks"
    files = {name: bodies[width][2] for name, width in BATCH_FILES.items()}
    files["series_60.json"] = series
    files["error_width.json"] = error_width
    for name, value in files.items():
        capture_common.write_json(folder / name, value)
    capture_common.write_provenance(folder, provenance_candles(info, stamp))
    record_candle_findings(section(info, stamp))
    print(f"wrote {len(files)} fixtures and PROVENANCE.md to {folder} (market {ticker})")
    print(f"recorded the findings in {OBSERVED}")
```

Replace the `epic9_command` of Step 1, quoted:

```python
def epic9_command(args):
    """Runs one of Epic 9's two modes; `args` is `sys.argv[1:]`, whose first item starts with `--`."""
    if len(args) == 2 and args[0] == "--check-candlesticks":
        check_candlesticks(Path(args[1]))
        return
    sys.exit(__doc__)
```

with

```python
def epic9_command(args):
    """Runs one of Epic 9's two modes; `args` is `sys.argv[1:]`, whose first item starts with `--`."""
    if len(args) == 2 and args[0] == "--check-candlesticks":
        check_candlesticks(Path(args[1]))
        return
    if args[0] == "--candlesticks":
        rest = args[1:]
        wanted = None
        if len(rest) == 3 and rest[1] == "--ticker":
            wanted = rest[2]
            rest = rest[:1]
        if len(rest) == 1:
            capture_candlesticks(Path(rest[0]), wanted)
            return
    sys.exit(__doc__)
```

Then run the capture, which needs only the network:

Run: `nix develop --command uv run scripts/capture_kalshi_predictions.py --candlesticks polyoxide-kalshi/tests/fixtures/predictions`
Expected: `wrote 5 fixtures and PROVENANCE.md to polyoxide-kalshi/tests/fixtures/predictions/candlesticks (market <ticker>)` and `recorded the findings in …/docs/specs/kalshi/OBSERVED.md`. If it exits with "none of the first 60 quoted open markets…", run it again later or pass `--ticker` of a market that trades through the day. Review `git diff docs/specs/kalshi/OBSERVED.md` and the new files; no secret appears in them (the run is unsigned).

- [ ] **Step 4: Run it to see it pass, and read the findings against D6.**

Run: `nix develop --command uv run scripts/capture_kalshi_predictions.py --check-candlesticks polyoxide-kalshi/tests/fixtures/predictions; echo "exit $?"`
Expected: `ok: 5 candlestick fixtures in polyoxide-kalshi/tests/fixtures/predictions/candlesticks`, then `exit 0`.

Run: `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-kalshi/tests/fixtures/predictions/candlesticks/*.json')]" && grep -n 'DOES NOT HOLD' docs/specs/kalshi/OBSERVED.md; echo "grep exit $?"`
Expected: no line from `grep`, then `grep exit 1`. A `DOES NOT HOLD` line means a finding contradicts D6: stop the story and report it to the owner before Story 3 runs. Do not edit the block by hand; it is the script's.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-kalshi/tests/fixtures/predictions/candlesticks
git commit -m "feat(kalshi): the candlestick fixtures and their findings (Epic 9, Story 1)" -- scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions/candlesticks docs/specs/kalshi/OBSERVED.md
```

### Task 1.4: The multi-channel socket fixtures

`scripts/capture_kalshi_predictions_ws.py` gains `--epic9`, which signs one demo session through `capture_kalshi_account.signed_headers` and walks the questions of spec §3 on four quoted open demo markets (`first` and `second` subscribed, `third` added, `outside` never subscribed), and `--check-epic9`, which makes no connection and re-reads the files. The default mode, which writes Epic 8's six fixtures and `PROVENANCE.md`, is untouched, and a later default run would rewrite `PROVENANCE.md` without Epic 9's block, so it is not rerun. The session, in order: one `subscribe` for the three channels on `first` and `second` (the `subscribed` answers, one `ticker` frame and one `trade` frame); `get_snapshot` on the order-book `sid` for `first` (the answer and the deltas that follow it on the `sid`); one `update_subscription` naming the order-book and `ticker` `sid`s together to add `third`; `get_snapshot` for `outside` on the order-book `sid` (an error); `get_snapshot` on the `ticker` and `trade` `sid`s; a `subscribe` for `ticker` and `trade` naming no market, stopped after a few frames; an `unsubscribe` of every `sid`. Nothing is written until every step has succeeded. The script stops before it connects when `KALSHI_DEMO_API_KEY_ID` or `KALSHI_DEMO_PRIVATE_KEY` is absent: the story stops there and asks the owner, and no fixture is ever written by hand.

**Files:**
- Modify: `scripts/capture_kalshi_predictions_ws.py`
- Create (captured): `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed_ticker.json`, `subscribed_trade.json`, `ticker.json`, `trade.json`, `snapshot_answer.json`, `deltas_after_snapshot.json`, `error_snapshot_outside.json`, `multi_sid_answer.json`
- Modify: `polyoxide-kalshi/tests/fixtures/predictions/ws/PROVENANCE.md` (the script appends a block), `docs/specs/kalshi/OBSERVED.md` (the script writes a block above the Epic 9 anchor)

- [ ] **Step 1: Write the failing check.** Four edits to the script; none opens a connection.

First, extend the module docstring. Replace

```python
one an earlier run wrote. Review the diff before committing.
"""
```

with

```python
one an earlier run wrote. Review the diff before committing.

Epic 9 adds two options, each its own mode:

    uv run scripts/capture_kalshi_predictions_ws.py --epic9 [--ticker TICKER] [--seconds 60]
    uv run scripts/capture_kalshi_predictions_ws.py --check-epic9

`--epic9` signs one demo session and writes eight more files under the same ws/ directory, a block
of PROVENANCE.md between its epic9 markers, and a block of OBSERVED.md above its Epic 9 anchor
(`<!-- epic 9 findings go above this line -->`). --seconds is how long it waits for a `ticker`
frame and a `trade` frame. `--check-epic9` makes no connection: it re-reads the files and exits 1
naming what is wrong.
"""
```

Second, add the constants. Directly after

```python
END = "<!-- kalshi-ws-capture:end -->"
```

insert

```python

EPIC9_ANCHOR = "<!-- epic 9 findings go above this line -->"
EPIC9_BEGIN = "<!-- kalshi-ws-epic9:begin -->"
EPIC9_END = "<!-- kalshi-ws-epic9:end -->"
PROV_BEGIN = "<!-- epic9:begin -->"
PROV_END = "<!-- epic9:end -->"
CHANNELS = ("orderbook_delta", "ticker", "trade")
EPIC9_FILES = (
    "subscribed_ticker.json",
    "subscribed_trade.json",
    "ticker.json",
    "trade.json",
    "snapshot_answer.json",
    "deltas_after_snapshot.json",
    "error_snapshot_outside.json",
    "multi_sid_answer.json",
)
# A command's answer is complete when this long passes with no further frame carrying its id.
SETTLE_SECONDS = 2.0
# Pause before each command: Kalshi refuses a subscription that sends too fast (code 27).
PACE_SECONDS = 0.3
# How long the capture reads after get_snapshot, and after the subscribe with no market.
SNAPSHOT_SECONDS = 20.0
EMPTY_SECONDS = 15.0
EMPTY_FRAMES = 20
# `tickerPayload.msg` and `tradePayload.msg` required keys in docs/specs/kalshi/asyncapi.yaml.
TICKER_REQUIRED = (
    "market_id", "market_ticker", "price_dollars", "yes_bid_dollars", "yes_ask_dollars", "volume_fp",
    "open_interest_fp", "dollar_volume", "dollar_open_interest", "yes_bid_size_fp", "yes_ask_size_fp",
    "last_trade_size_fp", "ts", "ts_ms", "time",
)
TRADE_REQUIRED = (
    "trade_id", "market_ticker", "yes_price_dollars", "no_price_dollars", "count_fp", "taker_side",
    "taker_outcome_side", "taker_book_side", "is_block_trade", "ts", "ts_ms",
)
```

Third, add the check directly above `def main() -> None:`.

```python
def check_epic9() -> None:
    """Re-reads Epic 9's eight fixtures, PROVENANCE.md and OBSERVED.md, and exits 1 naming each
    thing that is wrong."""
    problems: list[str] = []

    def load(name: str) -> Any:
        try:
            return json.loads((OUT / name).read_text(encoding="utf-8"))
        except (OSError, ValueError) as err:
            problems.append(f"{name}: {err}")
            return None

    def is_frame(value: Any, kind: str, name: str) -> bool:
        if not isinstance(value, dict) or value.get("type") != kind:
            problems.append(f"{name}: expected a `{kind}` frame")
            return False
        return True

    sids: dict[str, Any] = {}
    for channel in ("ticker", "trade"):
        name = f"subscribed_{channel}.json"
        frame = load(name)
        if frame is not None and is_frame(frame, "subscribed", name):
            msg = frame.get("msg") or {}
            if msg.get("channel") != channel or not isinstance(msg.get("sid"), int):
                problems.append(f"{name}: expected `msg.channel` {channel!r} and an integer `msg.sid`")
            if not isinstance(frame.get("id"), int):
                problems.append(f"{name}: expected an integer `id` on the frame")
            sids[channel] = msg.get("sid")
    if len(sids) == 2 and sids["ticker"] == sids["trade"]:
        problems.append("subscribed_ticker.json and subscribed_trade.json carry the same `sid`")

    for kind in ("ticker", "trade"):
        name = f"{kind}.json"
        frame = load(name)
        if frame is not None and is_frame(frame, kind, name):
            if not isinstance(frame.get("sid"), int) or not isinstance(frame.get("msg"), dict):
                problems.append(f"{name}: expected an integer `sid` and a `msg` object")
            elif sids.get(kind) is not None and frame["sid"] != sids[kind]:
                problems.append(f"{name}: `sid` {frame['sid']} is not the {kind} answer's {sids[kind]}")
            if kind == "ticker" and "seq" in frame:
                problems.append(f"{name}: the `ticker` channel carries no `seq`, but this frame has one")
            if kind == "trade" and not isinstance(frame.get("seq"), int):
                problems.append(f"{name}: expected an integer `seq` on the `trade` frame")

    answer = load("snapshot_answer.json")
    answer_sid = answer_seq = None
    if answer is not None and is_frame(answer, "orderbook_snapshot", "snapshot_answer.json"):
        answer_sid, answer_seq = answer.get("sid"), answer.get("seq")
        for key in ("id", "sid", "seq"):
            if not isinstance(answer.get(key), int):
                problems.append(f"snapshot_answer.json: expected an integer `{key}`")
    deltas = load("deltas_after_snapshot.json")
    if deltas is not None:
        if not isinstance(deltas, list) or not deltas:
            problems.append("deltas_after_snapshot.json: expected a non-empty array")
        else:
            seqs = [d.get("seq") if isinstance(d, dict) else None for d in deltas]
            if not all(isinstance(d, dict) and d.get("type") == "orderbook_delta" for d in deltas):
                problems.append("deltas_after_snapshot.json: every item must be an `orderbook_delta` frame")
            elif answer_sid is not None and any(d.get("sid") != answer_sid for d in deltas):
                problems.append("deltas_after_snapshot.json: a delta is not on the snapshot answer's `sid`")
            elif not all(isinstance(s, int) for s in seqs) or seqs != sorted(set(seqs)):
                problems.append("deltas_after_snapshot.json: `seq` must be integers in strictly increasing order")
            elif isinstance(answer_seq, int) and seqs[0] <= answer_seq:
                problems.append("deltas_after_snapshot.json: the first delta's `seq` does not come after the answer's")
    refusal = load("error_snapshot_outside.json")
    if refusal is not None and is_frame(refusal, "error", "error_snapshot_outside.json"):
        if not isinstance((refusal.get("msg") or {}).get("code"), int):
            problems.append("error_snapshot_outside.json: expected an integer `msg.code`")
        if not isinstance(refusal.get("id"), int):
            problems.append("error_snapshot_outside.json: expected an integer `id` on the frame")
    multi = load("multi_sid_answer.json")
    if multi is not None and not (
        (isinstance(multi, dict) and multi) or (isinstance(multi, list) and multi and all(isinstance(f, dict) for f in multi))
    ):
        problems.append("multi_sid_answer.json: expected a frame or a non-empty array of frames")

    try:
        provenance = (OUT / "PROVENANCE.md").read_text(encoding="utf-8")
    except OSError as err:
        problems.append(f"PROVENANCE.md: {err}")
    else:
        absent = [name for name in EPIC9_FILES if name not in provenance]
        if absent:
            problems.append("PROVENANCE.md does not mention " + ", ".join(absent))
    observed = OBSERVED.read_text(encoding="utf-8")
    if EPIC9_BEGIN not in observed or EPIC9_END not in observed:
        problems.append(f"{OBSERVED} holds no block between {EPIC9_BEGIN} and {EPIC9_END}")

    for problem in problems:
        print(f"  {problem}")
    if problems:
        raise SystemExit(f"{len(problems)} problem(s) in {OUT}")
    print(f"ok: {len(EPIC9_FILES)} Epic 9 socket fixtures in {OUT}")
```

Fourth, wire `--check-epic9` into `main`. Replace

```python
    parser.add_argument("--seconds", type=float, default=60.0, help="how long to read deltas (default 60)")
    args = parser.parse_args()
    require_key()
```

with

```python
    parser.add_argument("--seconds", type=float, default=60.0, help="how long to read deltas (default 60)")
    parser.add_argument("--check-epic9", action="store_true", help="re-read Epic 9's fixtures and exit 1 on a fault; no connection")
    args = parser.parse_args()
    if args.check_epic9:
        check_epic9()
        return
    require_key()
```

- [ ] **Step 2: Run it to see it fail.**

Run: `nix develop --command uv run scripts/capture_kalshi_predictions_ws.py --check-epic9; echo "exit $?"`
Expected: eight indented `…: [Errno 2] No such file or directory: …` lines (one for each file of `EPIC9_FILES`), then `PROVENANCE.md does not mention subscribed_ticker.json, …` and `OBSERVED.md holds no block between …`, then `10 problem(s) in …/polyoxide-kalshi/tests/fixtures/predictions/ws` and `exit 1`.

- [ ] **Step 3: Implement.** First check the key; without it stop here and ask the owner.

Run: `bash -c 'test -n "$KALSHI_DEMO_API_KEY_ID" && test -n "$KALSHI_DEMO_PRIVATE_KEY"; echo "exit $?"'`
Expected: `exit 0`. On `exit 1` stop the story at this step and ask the owner for the demo key (the plan's rules), then resume here. Do not write any fixture by hand.

Insert the capture directly above `def check_epic9() -> None:`.

```python
def choose_four(wanted: str | None) -> tuple[str, str, str, str]:
    """`first` (the market asked for, else the first quoted open one with a two-sided book), then
    three more quoted open markets: `second`, `third` and `outside`."""
    listing = fetch_ok("/trade-api/v2/markets", status="open", mve_filter="exclude", limit=1000)
    tickers = [market["ticker"] for market in listing["markets"] if quoted(market)]
    first = wanted or next((t for t in tickers[:CANDIDATES] if two_sided(t)), None)
    if first is None:
        raise SystemExit(
            f"none of the first {CANDIDATES} quoted open demo markets has a two-sided book: pass --ticker"
        )
    others = [t for t in tickers if t != first]
    if len(others) < 3:
        raise SystemExit("the demo host lists fewer than four quoted open markets")
    return first, others[0], others[1], others[2]


async def exchange(tape: Tape, command: dict[str, Any]) -> list[Seen]:
    """Sends `command` and returns every frame that carries its id, read until SETTLE_SECONDS pass
    after the last one. Exits when none arrives in ANSWER_SECONDS. Frames with other ids stay on the
    tape."""
    await asyncio.sleep(PACE_SECONDS)
    await tape.ws.send(json.dumps(command))
    got: list[Seen] = []
    give_up = time.monotonic() + ANSWER_SECONDS
    settle: float | None = None
    while True:
        remaining = (give_up if settle is None else settle) - time.monotonic()
        if remaining <= 0:
            break
        seen = await tape.read(remaining)
        if seen is None:
            break
        if seen.frame.get("id") == command["id"]:
            got.append(seen)
            settle = time.monotonic() + SETTLE_SECONDS
    if not got:
        raise SystemExit(f"no frame answered {command['cmd']} (id {command['id']}) in {ANSWER_SECONDS:.0f} s")
    return got


def outcome(frames: list[Seen]) -> str:
    """The frames' types, and an error's code and message."""
    parts = []
    for seen in frames:
        msg = seen.frame.get("msg") or {}
        if seen.frame.get("type") == "error":
            parts.append(f"`error` code {msg.get('code')}: {msg.get('msg')}")
        else:
            parts.append(f"`{seen.frame.get('type')}`")
    return ", ".join(parts)


def is_snapshot_of(seen: Seen, ticker: str) -> bool:
    return seen.frame.get("type") == "orderbook_snapshot" and (seen.frame.get("msg") or {}).get("market_ticker") == ticker


@dataclass
class Epic9:
    tickers: tuple[str, str, str, str]
    sids: dict[str, int]
    subscribed: dict[str, Seen]
    ticker: Seen
    trade: Seen
    answer_frames: list[Seen]
    answer: Seen
    has_id: bool
    between: int
    deltas_after: list[Seen]
    multi: list[Seen]
    third_snapshot: bool
    outside: list[Seen]
    on_ticker_sid: list[Seen]
    on_trade_sid: list[Seen]
    empty_answers: list[Seen]
    empty_tickers: list[str]
    empty_count: int
    final: list[Seen]
    too_fast: int


async def run_epic9(tickers: tuple[str, str, str, str], seconds: float) -> Epic9:
    first, second, third, outside = tickers
    ids = iter(range(1, 1000))
    async with capture_common.ws_session(WS, headers=signed) as ws:
        tape = Tape(ws)

        subscribe = {
            "id": next(ids),
            "cmd": "subscribe",
            "params": {"channels": list(CHANNELS), "market_tickers": [first, second], "use_yes_price": True},
        }
        answers = await exchange(tape, subscribe)
        subscribed: dict[str, Seen] = {}
        for seen in answers:
            expect(seen, "subscribed", "the three-channel subscribe")
            channel = (seen.frame.get("msg") or {}).get("channel")
            if channel is None:
                raise SystemExit(f"a `subscribed` answer names no channel: {seen.text}")
            subscribed[channel] = seen
        if sorted(subscribed) != sorted(CHANNELS):
            raise SystemExit(f"the three-channel subscribe was answered for {sorted(subscribed)}: {[s.text for s in answers]}")
        sids = {channel: seen.frame["msg"]["sid"] for channel, seen in subscribed.items()}

        def logged(kind: str) -> Seen | None:
            return next((s for s in tape.log if s.frame.get("type") == kind), None)

        deadline = time.monotonic() + seconds
        while (logged("ticker") is None or logged("trade") is None) and time.monotonic() < deadline:
            if await tape.read(deadline - time.monotonic()) is None:
                break
        ticker, trade = logged("ticker"), logged("trade")
        if ticker is None or trade is None:
            missing = [name for name, seen in (("ticker", ticker), ("trade", trade)) if seen is None]
            raise SystemExit(
                f"no {' and no '.join(missing)} frame for {first} or {second} in {seconds:.0f} s: "
                "rerun with a larger --seconds, or --ticker of a busier market"
            )

        book = sids["orderbook_delta"]
        mark = len(tape.log)
        command = {
            "id": next(ids),
            "cmd": "update_subscription",
            "params": {"sid": book, "action": "get_snapshot", "market_tickers": [first]},
        }
        answer_frames = await exchange(tape, command)
        await tape.read_for(SNAPSHOT_SECONDS)
        window = tape.log[mark:]
        index = next((i for i, seen in enumerate(window) if is_snapshot_of(seen, first)), None)
        if index is None:
            raise SystemExit(
                f"get_snapshot for {first} was answered with {outcome(answer_frames)}, and no snapshot of it arrived"
            )
        answer = window[index]
        between = sum(
            1 for seen in window[:index] if seen.frame.get("type") == "orderbook_delta" and seen.frame.get("sid") == book
        )
        deltas_after = [
            seen for seen in window[index + 1 :] if seen.frame.get("type") == "orderbook_delta" and seen.frame.get("sid") == book
        ][:MAX_DELTAS]
        if not deltas_after:
            raise SystemExit(
                f"no orderbook_delta followed the snapshot in {SNAPSHOT_SECONDS:.0f} s: rerun with --ticker of a busier market"
            )

        mark = len(tape.log)
        command = {
            "id": next(ids),
            "cmd": "update_subscription",
            "params": {"sids": [book, sids["ticker"]], "action": "add_markets", "market_tickers": [third]},
        }
        multi = await exchange(tape, command)
        await tape.read_for(ADDED_SECONDS)
        third_snapshot = any(is_snapshot_of(seen, third) for seen in tape.log[mark:])

        command = {
            "id": next(ids),
            "cmd": "update_subscription",
            "params": {"sid": book, "action": "get_snapshot", "market_tickers": [outside]},
        }
        refused_frames = await exchange(tape, command)
        if not any(seen.frame.get("type") == "error" for seen in refused_frames):
            raise SystemExit(
                f"get_snapshot for {outside}, outside the subscription, was answered with {outcome(refused_frames)}, "
                "not an error: report this to the owner, since the plan expects an error"
            )

        on_ticker_sid = await exchange(
            tape,
            {
                "id": next(ids),
                "cmd": "update_subscription",
                "params": {"sid": sids["ticker"], "action": "get_snapshot", "market_tickers": [first]},
            },
        )
        on_trade_sid = await exchange(
            tape,
            {
                "id": next(ids),
                "cmd": "update_subscription",
                "params": {"sid": sids["trade"], "action": "get_snapshot", "market_tickers": [first]},
            },
        )

        mark = len(tape.log)
        empty_answers = await exchange(
            tape, {"id": next(ids), "cmd": "subscribe", "params": {"channels": ["ticker", "trade"]}}
        )
        fresh = [
            seen.frame["msg"]["sid"]
            for seen in empty_answers
            if seen.frame.get("type") == "subscribed" and isinstance((seen.frame.get("msg") or {}).get("sid"), int)
        ]

        def fresh_frames() -> list[Seen]:
            return [
                seen
                for seen in tape.log[mark:]
                if seen.frame.get("type") in ("ticker", "trade") and seen.frame.get("sid") in fresh
            ]

        deadline = time.monotonic() + EMPTY_SECONDS
        while fresh and len(fresh_frames()) < EMPTY_FRAMES and time.monotonic() < deadline:
            if await tape.read(deadline - time.monotonic()) is None:
                break
        seen_on_fresh = fresh_frames()
        empty_tickers = sorted({(seen.frame.get("msg") or {}).get("market_ticker") for seen in seen_on_fresh} - {None})

        final = await exchange(
            tape, {"id": next(ids), "cmd": "unsubscribe", "params": {"sids": [*sids.values(), *fresh]}}
        )
        too_fast = sum(
            1
            for seen in tape.log
            if seen.frame.get("type") == "error" and (seen.frame.get("msg") or {}).get("code") == 27
        )
        return Epic9(
            tickers, sids, subscribed, ticker, trade, answer_frames, answer,
            any(seen is answer for seen in answer_frames), between, deltas_after, multi,
            third_snapshot, refused_frames, on_ticker_sid, on_trade_sid, empty_answers, empty_tickers,
            len(seen_on_fresh), final, too_fast,
        )
```

Now the writers and the section. Insert them directly above `def check_epic9() -> None:`, below `run_epic9`.

```python
def shape(frame: dict[str, Any], required: tuple[str, ...]) -> str:
    msg = frame.get("msg") or {}
    missing = [key for key in required if key not in msg]
    extra = sorted(key for key in msg if key not in required)
    absent = ", ".join(f"`{key}`" for key in missing) or "none"
    more = ", ".join(f"`{key}`" for key in extra) or "none"
    return f"required keys absent: {absent}; keys the mirror lacks: {more}"


def section9(c: Epic9, stamp: str) -> str:
    first, second, third, outside = c.tickers
    book = c.sids["orderbook_delta"]
    seq, previous = c.answer.frame.get("seq"), c.answer.previous
    first_delta = c.deltas_after[0].frame.get("seq")
    on_numbering = isinstance(seq, int) and previous is not None and seq == previous + 1
    continues = isinstance(seq, int) and first_delta == seq + 1
    elsewhere = sum(
        1 for seen in c.deltas_after if (seen.frame.get("msg") or {}).get("market_ticker") != first
    )
    d9 = "holds" if continues else "**DOES NOT HOLD. Report this to the owner before Story 5 runs.**"
    beyond = [t for t in c.empty_tickers if t not in c.tickers]
    rows = [
        (
            "One `subscribe` for the three channels",
            f"answered by {len(c.subscribed)} `subscribed` frames, one for each channel, each with its own `sid`: "
            + ", ".join(f"`{channel}` {sid}" for channel, sid in c.sids.items()),
        ),
        (
            "A `ticker` frame",
            f"on `sid` {c.ticker.frame.get('sid')} (the answered {c.sids['ticker']}); carries `seq`: "
            f"{'yes' if 'seq' in c.ticker.frame else 'no'}; {shape(c.ticker.frame, TICKER_REQUIRED)}",
        ),
        (
            "A `trade` frame",
            f"on `sid` {c.trade.frame.get('sid')} (the answered {c.sids['trade']}); carries `seq`: "
            f"{'yes' if 'seq' in c.trade.frame else 'no'}; {shape(c.trade.frame, TRADE_REQUIRED)}",
        ),
        ("`get_snapshot` for `first` on the order-book `sid`: the frames carrying its id", outcome(c.answer_frames)),
        (
            "The snapshot answering it",
            f"carries the command's `id`: {'yes' if c.has_id else 'no'}; `sid` {c.answer.frame.get('sid')} "
            f"(the order-book `sid` is {book}); `seq` {seq}; the last `seq` on that `sid` before it was {previous}; "
            f"{c.between} deltas on the `sid` came between the command and the snapshot",
        ),
        (
            "The snapshot's `seq` is the next number of the `sid`'s numbering",
            "yes" if on_numbering else f"no: {previous} before it, {seq} on it",
        ),
        (
            "The first delta after the snapshot continues from its `seq`",
            f"{'yes' if continues else 'no'}: `seq` {first_delta} after {seq}; {len(c.deltas_after)} deltas captured, "
            f"{elsewhere} of them for a market other than `first`",
        ),
        ("Spec D9's premise (the answer is on the `sid`'s numbering and the next delta follows it)", d9),
        (
            "One `update_subscription` naming two `sids` (`add_markets` for `third`)",
            f"{outcome(c.multi)}; a snapshot of `third` followed on the order-book `sid`: {'yes' if c.third_snapshot else 'no'}",
        ),
        ("`get_snapshot` for a ticker outside the subscription", outcome(c.outside)),
        ("`get_snapshot` on the `ticker` channel's `sid`", outcome(c.on_ticker_sid)),
        ("`get_snapshot` on the `trade` channel's `sid`", outcome(c.on_trade_sid)),
        (
            "A `subscribe` for `ticker` and `trade` naming no market",
            f"{outcome(c.empty_answers)}; {c.empty_count} frames in {EMPTY_SECONDS:.0f} s on its `sid`s from "
            f"{len(c.empty_tickers)} markets, {len(beyond)} of them outside the four chosen: "
            + ("it means every market" if beyond else "no market beyond the four was seen, so this does not show it means every market"),
        ),
        ("The final `unsubscribe` of every `sid`", outcome(c.final)),
        ("Code 27 (too many requests) met", str(c.too_fast)),
    ]
    lines = [
        "### The multi-channel socket, as the demo host answers it",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py --epic9` from `{WS}`. `first` `{first}` and "
        f"`second` `{second}` were subscribed, `third` `{third}` added by the two-`sid` update, and `outside` `{outside}` "
        "never subscribed. Fixtures: `polyoxide-kalshi/tests/fixtures/predictions/ws/`.",
        "",
        *capture_common.provenance_table(("Question", "Observed"), rows),
    ]
    return "\n".join(lines)


def record_block(path: Path, begin: str, end: str, anchor: str | None, text: str) -> None:
    """Writes `text` into `path` between `begin` and `end`, replacing an earlier run's block, or
    directly above `anchor` (the end of the file when `anchor` is None) when there is none."""
    current = path.read_text(encoding="utf-8")
    block = f"{begin}\n{text.rstrip()}\n{end}"
    if begin in current and end in current:
        head, rest = current.split(begin, 1)
        _, tail = rest.split(end, 1)
        current = head + block + tail
    elif anchor is None:
        current = current.rstrip("\n") + f"\n\n{block}\n"
    else:
        current = current.replace(anchor, f"{block}\n\n{anchor}", 1)
    capture_common.write_raw(path, current)


def write_epic9(c: Epic9, stamp: str, seconds: float) -> None:
    first, second, third, outside = c.tickers
    multi = c.multi[0].frame if len(c.multi) == 1 else [seen.frame for seen in c.multi]
    files = [
        ("subscribed_ticker.json", c.subscribed["ticker"].frame, "the `subscribed` answer for the `ticker` channel, from the three-channel `subscribe`"),
        ("subscribed_trade.json", c.subscribed["trade"].frame, "the `subscribed` answer for the `trade` channel, from the same `subscribe`"),
        ("ticker.json", c.ticker.frame, "the first `ticker` frame of the session"),
        ("trade.json", c.trade.frame, "the first `trade` frame of the session"),
        ("snapshot_answer.json", c.answer.frame, "the `orderbook_snapshot` answering `get_snapshot` for `first`, with its `id`, `sid` and `seq`"),
        (
            "deltas_after_snapshot.json",
            [seen.frame for seen in c.deltas_after],
            f"{len(c.deltas_after)} `orderbook_delta` frames on that `sid` after it, as an array, in wire order (every market of the `sid`, not only `first`)",
        ),
        ("error_snapshot_outside.json", next(s for s in c.outside if s.frame.get("type") == "error").frame, f"the `error` answering `get_snapshot` for `{outside}`, a ticker outside the subscription"),
        ("multi_sid_answer.json", multi, "the answer to one `update_subscription` naming two `sids` (an array when there is more than one frame)"),
    ]
    for name, value, _what in files:
        capture_common.write_json(OUT / name, value, ensure_ascii=True)
    lines = [
        "## Epic 9 captures",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py --epic9` from `{WS}` (the demo host), the "
        "handshake signed as above. A `subscribe` for `orderbook_delta`, `ticker` and `trade` on "
        f"`{first}` and `{second}` (`use_yes_price: true`), up to {seconds:.0f} s for a `ticker` and a `trade` "
        f"frame, then the commands of spec §3 in order. `{third}` was added by the two-`sid` update and `{outside}` "
        "was never subscribed.",
        "",
        *capture_common.provenance_table(
            ("Fixture", "What it is"), [(f"`{name}`", what) for name, _value, what in files]
        ),
    ]
    record_block(OUT / "PROVENANCE.md", PROV_BEGIN, PROV_END, None, "\n".join(lines))


def capture_epic9(args: argparse.Namespace) -> None:
    require_key()
    if EPIC9_ANCHOR not in OBSERVED.read_text(encoding="utf-8"):
        raise SystemExit(f"{OBSERVED} has no anchor line {EPIC9_ANCHOR!r}: Task 1.2 writes it")
    tickers = choose_four(args.ticker)
    stamp = capture_common.stamp()
    try:
        capture = asyncio.run(run_epic9(tickers, args.seconds))
    except InvalidHandshake as err:
        raise SystemExit(
            f"the demo host refused the signed handshake ({err}): check the key id, and that the key is the demo environment's"
        ) from err
    except ConnectionClosed as err:
        raise SystemExit(f"the demo host closed the connection: {err}") from err
    write_epic9(capture, stamp, args.seconds)
    record_block(OBSERVED, EPIC9_BEGIN, EPIC9_END, EPIC9_ANCHOR, section9(capture, stamp))
    print(f"captured {len(EPIC9_FILES)} fixtures for {tickers[0]} into {OUT}")
    print(f"recorded the findings in {OBSERVED}")
```

Then add the `--epic9` option and its dispatch to `main`. Replace

```python
    parser.add_argument("--check-epic9", action="store_true", help="re-read Epic 9's fixtures and exit 1 on a fault; no connection")
    args = parser.parse_args()
    if args.check_epic9:
        check_epic9()
        return
    require_key()
```

with

```python
    parser.add_argument("--check-epic9", action="store_true", help="re-read Epic 9's fixtures and exit 1 on a fault; no connection")
    parser.add_argument("--epic9", action="store_true", help="capture Epic 9's multi-channel frames (--seconds is the wait for a ticker and a trade frame)")
    args = parser.parse_args()
    if args.check_epic9:
        check_epic9()
        return
    if args.epic9:
        capture_epic9(args)
        return
    require_key()
```

Run the capture:

Run: `nix develop --command uv run scripts/capture_kalshi_predictions_ws.py --epic9 --seconds 120`
Expected: `captured 8 fixtures for <ticker> into …/ws` and `recorded the findings in …/OBSERVED.md`. A `SystemExit` that names a missing `ticker` or `trade` frame, or no delta after the snapshot, is a quiet demo market: rerun with a larger `--seconds` or `--ticker` of a busier one. A `SystemExit` that says `get_snapshot` for a ticker outside the subscription was not an error, or that a `subscribed` answer names no channel, contradicts D8 or D9: stop and report it to the owner; the script has written nothing. Review the diff: the files are frames, not handshakes, so no key appears in them.

- [ ] **Step 4: Run it to see it pass, and read the findings against D9.**

Run: `nix develop --command uv run scripts/capture_kalshi_predictions_ws.py --check-epic9; echo "exit $?"`
Expected: `ok: 8 Epic 9 socket fixtures in …/ws`, then `exit 0`.

Run: `grep -n 'DOES NOT HOLD' docs/specs/kalshi/OBSERVED.md; echo "grep exit $?"`
Expected: no line, then `grep exit 1`. A `DOES NOT HOLD` line means the snapshot answer is not on the `sid`'s numbering, or the next delta does not follow it, which contradicts D9: stop the story and report it to the owner before Story 5 runs. The block is the script's; do not edit it by hand.

Run the story's **Verify** command from the repository root, now that every task's files exist, as three runs:

Run: `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-kalshi/tests/fixtures/predictions/**/*.json', recursive=True)]" && grep -qx '<!-- epic 9 findings go above this line -->' docs/specs/kalshi/OBSERVED.md; echo "exit $?"`
Expected: `exit 0`: every fixture parses, and the anchor is the line `OBSERVED.md` ends with.

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: the Kalshi suites pass as they did before; Epic 8's tests read their own fixtures, which are untouched, and the new ones are read by no test yet.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-kalshi/tests/fixtures/predictions/ws
git commit -m "feat(kalshi): the multi-channel socket fixtures and their findings (Epic 9, Story 1)" -- scripts/capture_kalshi_predictions_ws.py polyoxide-kalshi/tests/fixtures/predictions/ws docs/specs/kalshi/OBSERVED.md
```

## Story 2: `Candles::intervals()` and the `UnservedInterval` refusal in `polyoxide-venue` (spec D2, D3)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/records.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/tests/candles_intervals.rs`
- **Interfaces:** exposes "Story 2 → Stories 3, 8 and 9"; uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-venue --all-features && cargo test -p polyoxide-venue --features market --test candles_intervals && python3 scripts/venue_gate.py && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** deep (a foundation crate's public trait)

Some venues serve only some of the candle widths `CandleInterval` names, and a query for an unserved width must be refused before anything is sent. This story adds the provided method `Candles::intervals()` (default: every width, forwarded by `DynCandles`), `CandleQuery::ensure_served` and the `UnservedInterval` error with its `Display`, `Error` and `Classify` (`InvalidRequest`, a fault, no wait), re-exports it at the crate root and lists it in the crate's `Classify` assertion. No other venue changes: the CLOB, perps and USDⓈ-M keep the default. The new suite is a file of its own, `tests/candles_intervals.rs`; the docs and tests name no venue, only the made-up `acme.spot`. dynosaur 0.3.1 builds the `Dyn` wrapper's `impl Candles` from every dyn-compatible item of the trait, a provided method included (its erased trait drops the default body and the wrapper delegates), so `DynCandles` forwards `intervals()`; Task 2.2's dyn test proves it, and if it fails to compile or reports the default through an override, stop and ask rather than work around it. The story holds none of the profile's landmine markers.

### Task 2.1: `UnservedInterval` and `CandleQuery::ensure_served`

**Files:**
- Create: `polyoxide-venue/tests/candles_intervals.rs`
- Modify: `polyoxide-venue/src/records.rs:227-230` (the `CandleInterval` docs) and `polyoxide-venue/src/records.rs:298-303` (the end of `impl CandleQuery`)
- Modify: `polyoxide-venue/src/lib.rs:99-103` (the `records` re-export) and `polyoxide-venue/src/lib.rs:119-130` (the `Classify` assertion)

- [ ] **Step 1: Write the failing test**

Create `polyoxide-venue/tests/candles_intervals.rs`. The file holds Task 2.1's tests now; Task 2.2 replaces its imports and appends the venue tests.

```rust
#![cfg(feature = "market")]
//! `CandleQuery::ensure_served` and `UnservedInterval`: a width a venue does
//! not serve is refused with the widths it does, as an `InvalidRequest` that
//! no wait cures.

use polyoxide_venue::{
    CandleInterval, CandleQuery, Class, ClassifiedError, Classify, UnixMillis, UnservedInterval,
};

/// The widths the made-up venue serves.
const HOURLY_AND_DAILY: &[CandleInterval] = &[CandleInterval::Hour1, CandleInterval::Day1];

fn query(interval: CandleInterval) -> CandleQuery {
    CandleQuery::new(interval, UnixMillis(7))
}

#[test]
fn a_served_width_is_accepted() {
    for width in HOURLY_AND_DAILY {
        assert_eq!(query(*width).ensure_served(HOURLY_AND_DAILY), Ok(()));
    }
}

#[test]
fn every_other_width_is_refused_with_the_widths_that_are_served() {
    let unserved = [
        CandleInterval::Minute1,
        CandleInterval::Minute5,
        CandleInterval::Minute15,
        CandleInterval::Hour4,
        CandleInterval::Week1,
    ];
    for width in unserved {
        let refusal = query(width).ensure_served(HOURLY_AND_DAILY).unwrap_err();
        assert_eq!(refusal.interval, width);
        assert_eq!(refusal.served, HOURLY_AND_DAILY);
    }
    // The two lists together are every width, so none was left unchecked.
    assert_eq!(
        unserved.len() + HOURLY_AND_DAILY.len(),
        CandleInterval::ALL.len()
    );
}

#[test]
fn a_venue_that_serves_nothing_refuses_every_width() {
    for width in CandleInterval::ALL {
        let refusal = query(*width).ensure_served(&[]).unwrap_err();
        assert_eq!(refusal.interval, *width);
        assert!(refusal.served.is_empty());
    }
}

#[test]
fn the_message_names_the_width_and_the_widths_served() {
    let refusal = query(CandleInterval::Minute5)
        .ensure_served(HOURLY_AND_DAILY)
        .unwrap_err();
    assert_eq!(
        refusal.to_string(),
        "`Minute5` candles are not served; this venue serves Hour1, Day1"
    );

    let one = query(CandleInterval::Minute1)
        .ensure_served(&[CandleInterval::Day1])
        .unwrap_err();
    assert_eq!(
        one.to_string(),
        "`Minute1` candles are not served; this venue serves Day1"
    );

    let none = query(CandleInterval::Week1).ensure_served(&[]).unwrap_err();
    assert_eq!(
        none.to_string(),
        "`Week1` candles are not served; this venue serves none"
    );
}

#[test]
fn the_refusal_is_an_invalid_request_fault_that_no_wait_cures() {
    let refusal = query(CandleInterval::Minute5)
        .ensure_served(HOURLY_AND_DAILY)
        .unwrap_err();
    assert_eq!(refusal.class(), Class::InvalidRequest);
    assert!(refusal.is_fault());
    assert!(!refusal.is_retriable());
    assert_eq!(refusal.retry_after(), None);

    // `?` turns it into the one error a venue-neutral caller handles, with
    // the same class, fault and wait, and the refusal reachable by downcast.
    let classified: ClassifiedError = refusal.clone().into();
    assert_eq!(classified.class, Class::InvalidRequest);
    assert!(classified.is_fault());
    assert!(!classified.is_retriable());
    assert_eq!(classified.retry_after(), None);
    assert_eq!(
        classified.source.downcast_ref::<UnservedInterval>(),
        Some(&refusal)
    );
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features market --test candles_intervals`

Expected: the build fails before any test runs, with `error[E0432]: unresolved import `polyoxide_venue::UnservedInterval`` and `error[E0599]: no method named `ensure_served` found for struct `CandleQuery``.

- [ ] **Step 3: Implement**

In `polyoxide-venue/src/records.rs`, the `CandleInterval` docs say every venue serves every width, which `Candles::intervals` makes untrue. Replace

```rust
/// How long a candle spans. Each venue that serves candles serves every
/// width here, so a venue maps every variant and the enum is exhaustive: a
/// new width is a breaking change that fails to compile at a venue that does
/// not map it.
```

with

```rust
/// How long a candle spans. A venue serves some or all of these widths, the
/// ones its [`Candles::intervals`](crate::Candles::intervals) lists, and
/// handles every variant, since the enum is exhaustive: a new width is a
/// breaking change that fails to compile at a venue that does not handle it.
```

Then replace the end of `impl CandleQuery`, quoted exactly from the file,

```rust
    /// Ends the span at `end`.
    pub fn end(mut self, end: UnixMillis) -> Self {
        self.end = Some(end);
        self
    }
}
```

with

````rust
    /// Ends the span at `end`.
    pub fn end(mut self, end: UnixMillis) -> Self {
        self.end = Some(end);
        self
    }

    /// Checks that the query's width is one of `served`, which a venue takes
    /// from its [`Candles::intervals`](crate::Candles::intervals). Call it
    /// before the first request, and convert the refusal with `?`.
    pub fn ensure_served(&self, served: &'static [CandleInterval]) -> Result<(), UnservedInterval> {
        if served.contains(&self.interval) {
            Ok(())
        } else {
            Err(UnservedInterval {
                interval: self.interval,
                served,
            })
        }
    }
}

/// A query asked for a candle width the venue does not serve
/// ([`CandleQuery::ensure_served`]). An
/// [`InvalidRequest`](Class::InvalidRequest): the caller's misuse, refused
/// before anything is sent, and no wait or retry cures it.
///
/// Only [`CandleQuery::ensure_served`] builds one, since a field may be added
/// later and a struct literal is refused when the program is built:
///
/// ```compile_fail
/// use polyoxide_venue::{CandleInterval, UnservedInterval};
///
/// fn refuse() -> UnservedInterval {
///     UnservedInterval {
///         interval: CandleInterval::Minute5,
///         served: &[],
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UnservedInterval {
    /// The width asked for.
    pub interval: CandleInterval,
    /// The widths the venue serves, as its
    /// [`Candles::intervals`](crate::Candles::intervals) lists them.
    pub served: &'static [CandleInterval],
}

impl fmt::Display for UnservedInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{:?}` candles are not served; this venue serves ",
            self.interval
        )?;
        if self.served.is_empty() {
            return f.write_str("none");
        }
        for (index, width) in self.served.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{width:?}")?;
        }
        Ok(())
    }
}

impl Error for UnservedInterval {}

impl Classify for UnservedInterval {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}
````

`is_fault` stays the trait's default (true), and so does `retry_after`, which reads the class's own wait: none for `InvalidRequest`. The new test pins both.

In `polyoxide-venue/src/lib.rs`, add the re-export. Replace

```rust
pub use records::{
    Book, Candle, CandleInterval, CandleQuery, EventGroup, EventOutcome, FundingRate, Instrument,
    Level, NotListed, Quote, Side, SizeUnit, Trade, TradeId,
};
```

with

```rust
pub use records::{
    Book, Candle, CandleInterval, CandleQuery, EventGroup, EventOutcome, FundingRate, Instrument,
    Level, NotListed, Quote, Side, SizeUnit, Trade, TradeId, UnservedInterval,
};
```

and list the type in the assertion. Replace

```rust
    #[cfg(feature = "market")]
    is::<UndeclaredExtension>();
};
```

with

```rust
    #[cfg(feature = "market")]
    is::<UndeclaredExtension>();
    #[cfg(feature = "market")]
    is::<UnservedInterval>();
};
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --features market --test candles_intervals`

Expected: `test result: ok. 5 passed; 0 failed`.

Run: `nix develop --command cargo test -p polyoxide-venue --all-features`

Expected: every suite `ok`, the new `compile_fail` doctest on `UnservedInterval` among the doctests; nothing else changes.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-venue --all-targets --all-features -- -D warnings`

Expected: no output from fmt, and clippy ends `Finished` with no warnings.

Run: `bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'`

Expected: all pass (the sweep finds `UnservedInterval`'s `impl Error` and its `is::<UnservedInterval>()` line).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-venue/tests/candles_intervals.rs
git commit -m "feat(venue): UnservedInterval and CandleQuery::ensure_served (Epic 9, Story 2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_01FijMBSPGBERJ4VNP5AgG6U" -- polyoxide-venue/src/records.rs polyoxide-venue/src/lib.rs polyoxide-venue/tests/candles_intervals.rs
```

### Task 2.2: `Candles::intervals()`, forwarded by `DynCandles`

**Files:**
- Modify: `polyoxide-venue/src/market.rs:107-123` (the `Candles` trait)
- Test: `polyoxide-venue/tests/candles_intervals.rs` (replace the imports at the top, append after the last test)

- [ ] **Step 1: Write the failing test**

In `polyoxide-venue/tests/candles_intervals.rs`, replace the imports Task 2.1 wrote,

```rust
use polyoxide_venue::{
    CandleInterval, CandleQuery, Class, ClassifiedError, Classify, UnixMillis, UnservedInterval,
};
```

with

```rust
use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use futures_util::FutureExt;
use polyoxide_venue::{
    Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Classify, DynCandles,
    KeySpace, MarketKey, ProductId, UnixMillis, UnservedInterval, VenueId,
};
use rust_decimal::Decimal;
```

and append, after the file's last test (`the_refusal_is_an_invalid_request_fault_that_no_wait_cures`), the made-up venue and its tests. The venues answer at once, so `now_or_never` drives every call without a runtime.

```rust
struct Spot;

impl KeySpace for Spot {
    const VENUE: VenueId = VenueId::from_static("acme");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

fn abc() -> MarketKey {
    Spot::key(&"ABC".to_owned())
}

/// Overrides nothing, so it serves every width: the trait's default.
struct Everything;

impl Candles for Everything {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        query.ensure_served(self.intervals())?;
        Ok(vec![Candle::new(
            key.clone(),
            query.interval,
            query.start,
            Decimal::ONE,
        )])
    }
}

/// Serves hourly and daily candles only, and counts the reads it answered.
struct Sparse {
    reads: Arc<AtomicUsize>,
}

impl Candles for Sparse {
    fn intervals(&self) -> &'static [CandleInterval] {
        HOURLY_AND_DAILY
    }

    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        query.ensure_served(self.intervals())?;
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Candle::new(
            key.clone(),
            query.interval,
            query.start,
            Decimal::ONE,
        )])
    }
}

fn sparse_venue() -> (Arc<DynCandles<'static>>, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let venue = DynCandles::new_arc(Sparse {
        reads: Arc::clone(&reads),
    });
    (venue, reads)
}

#[test]
fn a_venue_that_does_not_override_serves_every_width() {
    assert_eq!(Everything.intervals(), CandleInterval::ALL);
    assert_eq!(Everything.intervals().len(), 7);
}

#[test]
fn an_override_is_seen_through_the_dyn_wrapper_and_so_is_the_default() {
    let (sparse, _) = sparse_venue();
    let everything: Arc<DynCandles<'static>> = DynCandles::new_arc(Everything);
    assert_eq!(
        sparse.intervals(),
        &[CandleInterval::Hour1, CandleInterval::Day1]
    );
    assert_eq!(everything.intervals(), CandleInterval::ALL);
}

#[test]
fn an_unserved_width_is_refused_before_the_venue_is_asked_and_a_served_one_is_not() {
    let (sparse, reads) = sparse_venue();
    let refused = sparse
        .candles(&abc(), &query(CandleInterval::Minute5))
        .now_or_never()
        .expect("the venue answers at once")
        .unwrap_err();
    assert_eq!(refused.class, Class::InvalidRequest);
    assert!(refused.is_fault());
    assert!(!refused.is_retriable());
    let refusal = refused
        .source
        .downcast_ref::<UnservedInterval>()
        .expect("the source is the UnservedInterval");
    assert_eq!(refusal.interval, CandleInterval::Minute5);
    assert_eq!(refusal.served, HOURLY_AND_DAILY);
    assert_eq!(reads.load(Ordering::SeqCst), 0);

    let candles = sparse
        .candles(&abc(), &query(CandleInterval::Hour1))
        .now_or_never()
        .expect("the venue answers at once")
        .expect("a served width is answered");
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].interval, CandleInterval::Hour1);
    assert_eq!(candles[0].open_time, UnixMillis(7));
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn the_default_refuses_no_width() {
    let everything: Arc<DynCandles<'static>> = DynCandles::new_arc(Everything);
    for width in CandleInterval::ALL {
        let candles = everything
            .candles(&abc(), &query(*width))
            .now_or_never()
            .expect("the venue answers at once")
            .expect("every width is served by default");
        assert_eq!(candles[0].interval, *width);
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features market --test candles_intervals`

Expected: the build fails with `error[E0407]: method `intervals` is not a member of trait `Candles`` (the `Sparse` impl) and `error[E0599]: no method named `intervals` found` for `Everything` and for `Arc<DynCandles<'static>>`.

- [ ] **Step 3: Implement**

In `polyoxide-venue/src/market.rs`, replace the `Candles` trait, quoted exactly,

```rust
/// Candles.
#[dynosaur::dynosaur(pub DynCandles = dyn(box) Candles)]
pub trait Candles: Send + Sync {
    /// Candles opening between `query.start` and `query.end`, oldest first:
    /// as many as the venue returns in one request. Read on from the last
    /// candle's open time for more.
    ///
    /// A venue may also bound the span one request covers (its module's docs
    /// give the bound), so an empty answer before `query.end` does not mean
    /// there are none later: read on from the end of the span the venue
    /// covered.
    fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> impl Future<Output = Result<Vec<Candle>, ClassifiedError>> + Send;
}
```

with

```rust
/// Candles.
#[dynosaur::dynosaur(pub DynCandles = dyn(box) Candles)]
pub trait Candles: Send + Sync {
    /// Candles opening between `query.start` and `query.end`, oldest first:
    /// as many as the venue returns in one request. Read on from the last
    /// candle's open time for more.
    ///
    /// A venue may also bound the span one request covers (its module's docs
    /// give the bound), so an empty answer before `query.end` does not mean
    /// there are none later: read on from the end of the span the venue
    /// covered.
    ///
    /// A query whose `interval` is not in [`intervals`](Self::intervals) is
    /// refused as an [`InvalidRequest`](crate::Class::InvalidRequest) with
    /// nothing sent. An implementation calls
    /// [`CandleQuery::ensure_served`](crate::CandleQuery::ensure_served) with
    /// `self.intervals()` before its first request and converts the
    /// [`UnservedInterval`](crate::UnservedInterval) with `?`.
    fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> impl Future<Output = Result<Vec<Candle>, ClassifiedError>> + Send;

    /// The candle widths this product serves, shortest first. Every width
    /// ([`CandleInterval::ALL`](crate::CandleInterval::ALL)) unless the
    /// implementation says otherwise; a product that serves only some
    /// overrides it, and its module's docs list them.
    fn intervals(&self) -> &'static [crate::CandleInterval] {
        crate::CandleInterval::ALL
    }
}
```

The new method is provided, so no implementation changes: the CLOB, perps and USDⓈ-M keep the default, since each serves every width.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --features market --test candles_intervals`

Expected: `test result: ok. 9 passed; 0 failed` (Task 2.1's five and these four).

Run: `nix develop --command cargo test -p polyoxide-venue --all-features`

Expected: every suite `ok`; `market.rs`'s own tests still compile, since their `impl Candles for Acme` keeps the default.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-venue --all-targets --all-features -- -D warnings && bash -c 'RUSTDOCFLAGS="-D warnings" nix develop --command cargo doc --no-deps -p polyoxide-venue --all-features'`

Expected: fmt prints nothing, clippy and doc end `Finished` with no warnings (every intra-doc link in the new text names a public item by its crate path).

Run: `python3 scripts/venue_gate.py`

Expected: it passes: the new docs and tests say only `acme`, `spot` and `Everything`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(venue): Candles::intervals, forwarded by DynCandles (Epic 9, Story 2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_01FijMBSPGBERJ4VNP5AgG6U" -- polyoxide-venue/src/market.rs polyoxide-venue/tests/candles_intervals.rs
```

## Story 3: Kalshi's `Candles` over the batch candlestick route (spec D4 to D6, §5)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs`, `polyoxide-kalshi/src/predictions/types/markets.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/venue.rs`, `polyoxide-kalshi/tests/mock_predictions_routes.rs`, `polyoxide-kalshi/tests/query_setters_predictions.rs`, `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions.rs`, `polyoxide-kalshi/tests/venue_predictions.rs`, `polyoxide-kalshi/tests/live_predictions_venue.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `docs/specs/kalshi/OBSERVED.md`
- **Interfaces:** exposes "Story 3 → Stories 8 and 9"; uses Story 1's candlestick fixtures and anchor and Story 2's `intervals`, `ensure_served` and `UnservedInterval`
- **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored && python3 scripts/live_unwraps.py && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** deep (the price mapping every candle consumer reads)

Gives `Predictions` the `Candles` trait over Kalshi's batch candlestick route, and the native client both candle routes. Task 3.1 is the owner's one exception (spec §13): it moves the four tests that pin Epic 8's eight routes to ten, and adds the two route constants. Task 3.2 adds `PeriodInterval` and the candlestick types, every `price` field and `previous_dollars` an `Option` (the venue omits them, never `null`), with spec and wire agreement on Story 1's captures. Task 3.3 adds `markets().candlesticks(..)` and `markets().series_candlesticks(..)` with their setters, pinned in the mock, query-setter and spec-agreement suites. Task 3.4 adds `impl Candles for Predictions`, `CandleExt` and `Ohlc`: the span shifted forward by one width because Kalshi selects a candlestick by its end and the trait by its open, the trade-price OHLC, a `no` key mirrored with high and low swapped, an untraded period closed at `previous`, a period with no `previous` skipped, and an unserved width refused before any request. Task 3.5 runs the three widths live through the trait. Story 1's fixtures are captured, so no test here quotes a literal from them: the fixture-driven tests read the fixture and compare field by field with what the client returns, and the literal-value tests serve inline bodies. No code of this story holds a landmine marker.

### Task 3.1: The route table holds ten routes (the owner's exception)

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/request.rs:81-105,238-254`
- Modify: `polyoxide-kalshi/tests/mock_predictions_routes.rs:498-513`
- Modify: `polyoxide-kalshi/tests/spec_agreement_predictions.rs:580`
- Modify: `polyoxide-kalshi/tests/live_predictions.rs:219`

- [ ] **Step 1: Move the four route-table tests to ten routes**

In `polyoxide-kalshi/src/predictions/request.rs`, replace the test (current lines 238-254):

```rust
    #[test]
    fn the_route_table_is_the_epics_eight_routes() {
        let templates: Vec<(&str, &str)> = route_templates().collect();
        assert_eq!(
            templates,
            [
                ("GET", "/trade-api/v2/exchange/status"),
                ("GET", "/trade-api/v2/markets"),
                ("GET", "/trade-api/v2/markets/:ticker"),
                ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
                ("GET", "/trade-api/v2/markets/trades"),
                ("GET", "/trade-api/v2/events/:event_ticker"),
                ("GET", "/trade-api/v2/account/endpoint_costs"),
                ("GET", "/trade-api/v2/account/limits"),
            ]
        );
    }
```

with:

```rust
    #[test]
    fn the_route_table_is_the_epics_ten_routes() {
        let templates: Vec<(&str, &str)> = route_templates().collect();
        assert_eq!(
            templates,
            [
                ("GET", "/trade-api/v2/exchange/status"),
                ("GET", "/trade-api/v2/markets"),
                ("GET", "/trade-api/v2/markets/:ticker"),
                ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
                ("GET", "/trade-api/v2/markets/trades"),
                ("GET", "/trade-api/v2/events/:event_ticker"),
                ("GET", "/trade-api/v2/account/endpoint_costs"),
                ("GET", "/trade-api/v2/account/limits"),
                ("GET", "/trade-api/v2/markets/candlesticks"),
                (
                    "GET",
                    "/trade-api/v2/series/:series_ticker/markets/:ticker/candlesticks"
                ),
            ]
        );
    }
```

In `polyoxide-kalshi/tests/mock_predictions_routes.rs`, replace the test (current lines 498-519):

```rust
#[test]
fn the_route_table_holds_the_eight_routes_in_endpoint_costs_spelling() {
    let mut routes: Vec<(String, &str)> = route_templates()
        .map(|(method, template)| (method.to_ascii_uppercase(), template))
        .collect();
    routes.sort();
    let expected = [
        ("GET", "/trade-api/v2/account/endpoint_costs"),
        ("GET", "/trade-api/v2/account/limits"),
        ("GET", "/trade-api/v2/events/:event_ticker"),
        ("GET", "/trade-api/v2/exchange/status"),
        ("GET", "/trade-api/v2/markets"),
        ("GET", "/trade-api/v2/markets/:ticker"),
        ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
        ("GET", "/trade-api/v2/markets/trades"),
    ];
```

with:

```rust
#[test]
fn the_route_table_holds_the_ten_routes_in_endpoint_costs_spelling() {
    let mut routes: Vec<(String, &str)> = route_templates()
        .map(|(method, template)| (method.to_ascii_uppercase(), template))
        .collect();
    routes.sort();
    let expected = [
        ("GET", "/trade-api/v2/account/endpoint_costs"),
        ("GET", "/trade-api/v2/account/limits"),
        ("GET", "/trade-api/v2/events/:event_ticker"),
        ("GET", "/trade-api/v2/exchange/status"),
        ("GET", "/trade-api/v2/markets"),
        ("GET", "/trade-api/v2/markets/:ticker"),
        ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
        ("GET", "/trade-api/v2/markets/candlesticks"),
        ("GET", "/trade-api/v2/markets/trades"),
        (
            "GET",
            "/trade-api/v2/series/:series_ticker/markets/:ticker/candlesticks",
        ),
    ];
```

(the lines after `];` of that test stay as they are).

In `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, replace the last line of `every_route_the_crate_holds_is_a_documented_operation`:

```rust
    assert_eq!(count, 8, "the route table holds eight routes");
```

with:

```rust
    assert_eq!(count, 10, "the route table holds ten routes");
```

In `polyoxide-kalshi/tests/live_predictions.rs`, replace the last line of `live_the_route_table_agrees_with_the_endpoint_costs`:

```rust
    assert_eq!(checked, 8, "the route table holds eight routes");
```

with:

```rust
    assert_eq!(checked, 10, "the route table holds ten routes");
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib the_route_table_is_the_epics_ten_routes
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_routes the_route_table_holds_the_ten_routes
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions every_route_the_crate_holds_is_a_documented_operation
```

Expected: each fails on its comparison, the table holding eight routes (`left: [..8 entries..]`, `right: [..10 entries..]` for the first two; `assertion left == right failed: the route table holds ten routes`, `left: 8`, `right: 10` for the third).

- [ ] **Step 3: Add the two routes**

In `polyoxide-kalshi/src/predictions/request.rs`, replace the head of the `routes` module (current lines 81-83):

```rust
/// The eight routes of this module, by name. No later story adds one.
pub(crate) mod routes {
    use super::Route;
```

with:

```rust
/// The ten routes of this module, by name.
pub(crate) mod routes {
    use super::Route;
```

and replace the end of that module (current lines 92-105):

```rust
    pub(crate) const ACCOUNT_LIMITS: Route = Route::get("/trade-api/v2/account/limits");

    /// Every route above.
    pub(crate) const ALL: &[Route] = &[
        EXCHANGE_STATUS,
        MARKETS,
        MARKET,
        ORDERBOOK,
        TRADES,
        EVENT,
        ENDPOINT_COSTS,
        ACCOUNT_LIMITS,
    ];
}
```

with:

```rust
    pub(crate) const ACCOUNT_LIMITS: Route = Route::get("/trade-api/v2/account/limits");
    /// The batch route: candlesticks for up to 100 markets, grouped by market.
    pub(crate) const CANDLESTICKS: Route = Route::get("/trade-api/v2/markets/candlesticks");
    /// One market's candlesticks, under its series.
    pub(crate) const SERIES_CANDLESTICKS: Route =
        Route::get("/trade-api/v2/series/:series_ticker/markets/:ticker/candlesticks");

    /// Every route above.
    pub(crate) const ALL: &[Route] = &[
        EXCHANGE_STATUS,
        MARKETS,
        MARKET,
        ORDERBOOK,
        TRADES,
        EVENT,
        ENDPOINT_COSTS,
        ACCOUNT_LIMITS,
        CANDLESTICKS,
        SERIES_CANDLESTICKS,
    ];
}
```

Both constants are used by `ALL`, so the tree has no dead code before Task 3.3 builds on them. Their cost is the default, 10, until the first-use confirmation replaces it with what `GET /account/endpoint_costs` lists for them (`read_costs` matches by method and template).

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib request::
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_routes
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions --no-run
```

Expected: `test result: ok.` for the first three (the unit test `our_routes_find_themselves_and_not_each_other` now covers ten routes: a literal segment never matches a parameter, so `/markets/candlesticks` and `/markets/:ticker` stay apart), and the live target compiles.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git commit -m "feat(kalshi): the route table holds the two candlestick routes (Epic 9, Story 3)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -- polyoxide-kalshi/src/predictions/request.rs polyoxide-kalshi/tests/mock_predictions_routes.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/tests/live_predictions.rs
```

### Task 3.2: `PeriodInterval` and the candlestick types

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/types/markets.rs:1,368-369,404-413`
- Modify: `polyoxide-kalshi/src/predictions/types/mod.rs:10-13`
- Test: `polyoxide-kalshi/tests/spec_agreement_predictions.rs`
- Test: `polyoxide-kalshi/tests/wire_agreement_predictions.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, add a `use` directly above the line `use polyoxide_test_support::{fixtures, openapi, query};`:

```rust
use polyoxide_kalshi::predictions::types::{
    BatchCandlesticksResponse, BidAskDistribution, MarketCandlestick, MarketCandlesticks,
    PeriodInterval, PriceDistribution, SeriesCandlesticksResponse,
};
```

and append after the file's last test (`every_route_the_crate_holds_is_a_documented_operation`):

```rust
// ── Candlesticks ────────────────────────────────────────────────────

#[test]
fn every_candlestick_schema_agrees_with_the_spec() {
    let schemas = mirror_schemas();
    check_schema::<BidAskDistribution>(&schemas, "BidAskDistribution");
    check_schema::<PriceDistribution>(&schemas, "PriceDistribution");
    check_schema::<MarketCandlestick>(&schemas, "MarketCandlestick");
    check_schema::<MarketCandlesticks>(&schemas, "MarketCandlesticksResponse");
    check_schema::<BatchCandlesticksResponse>(&schemas, "BatchGetMarketCandlesticksResponse");
    check_schema::<SeriesCandlesticksResponse>(&schemas, "GetMarketCandlesticksResponse");
}

#[test]
fn the_period_intervals_are_the_widths_the_mirror_documents() {
    let spec = mirror();
    let parameters = spec["paths"]["/series/{series_ticker}/markets/{ticker}/candlesticks"]["get"]
        ["parameters"]
        .as_array()
        .expect("the series route documents query parameters");
    let period = parameters
        .iter()
        .find(|parameter| parameter["name"] == "period_interval")
        .expect("the series route documents period_interval");
    let documented: Vec<String> = period["schema"]["enum"]
        .as_array()
        .expect("period_interval is an enum")
        .iter()
        .map(|width| width.to_string())
        .collect();
    let ours: Vec<String> = PeriodInterval::ALL
        .iter()
        .map(|width| width.as_str().to_owned())
        .collect();
    assert_eq!(ours, documented);
    // A width outside the set does not parse, so the native client cannot send it.
    assert!("5".parse::<PeriodInterval>().is_err());
}
```

In `polyoxide-kalshi/tests/wire_agreement_predictions.rs`, add a `use` directly above the line `use polyoxide_test_support::{`:

```rust
use polyoxide_kalshi::predictions::types::{
    BatchCandlesticksResponse, MarketCandlestick, SeriesCandlesticksResponse,
};
```

and append after the file's last test (`the_endpoint_costs_default_is_the_compiled_default`):

```rust
// ── candlesticks ────────────────────────────────────────────────

/// The batch captures by width: the file, and the number of seconds every candle's end is a
/// multiple of (a minute for 1-minute candles, an hour for the hourly ones, and for the daily
/// ones too, since the daily candle ends at 04:00 UTC).
const BATCH_FIXTURES: &[(&str, i64)] = &[("batch_1", 60), ("batch_60", 3_600), ("batch_1440", 3_600)];

fn hold_candles_to_fixture<T: DeserializeOwned + Serialize>(fixture: &str) {
    let name = format!("candlesticks/{fixture}");
    let text = fixtures!("predictions").text(&name);
    let diff = agreement::compare::<T>(&name, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{name}: the host sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{name}: the type emits {:?}, which the host did not send",
        diff.invented
    );
}

#[test]
fn every_candlestick_fixture_agrees_with_its_type() {
    for (fixture, _) in BATCH_FIXTURES {
        hold_candles_to_fixture::<BatchCandlesticksResponse>(fixture);
    }
    hold_candles_to_fixture::<SeriesCandlesticksResponse>("series_60");
}

/// A traded period and an untraded one that carries `previous_dollars`: the two cases the
/// mapping to candles is built on.
fn candles_cover(what: &str, rows: &[MarketCandlestick]) {
    assert!(
        rows.iter().any(|row| row.price.close_dollars.is_some()),
        "{what}: no traded period"
    );
    assert!(
        rows.iter()
            .any(|row| row.price.close_dollars.is_none() && row.price.previous_dollars.is_some()),
        "{what}: no untraded period that carries previous_dollars"
    );
}

#[test]
fn the_candlestick_fixtures_cover_the_cases_the_types_exist_for() {
    for (fixture, multiple) in BATCH_FIXTURES {
        let batch: BatchCandlesticksResponse = read(&format!("candlesticks/{fixture}"));
        assert_eq!(batch.markets.len(), 1, "{fixture}: one market was asked for");
        let rows = &batch.markets[0].candlesticks;
        candles_cover(fixture, rows);
        for row in rows {
            assert_eq!(
                row.end_period_ts % multiple,
                0,
                "{fixture}: a candle ends off its boundary"
            );
        }
    }

    // The series route answers the same rows as the batch route for the same market. The latest
    // shared period may still have been open at one capture and not at the other, so it is the
    // one left out of the comparison.
    let series: SeriesCandlesticksResponse = read("candlesticks/series_60");
    candles_cover("series_60", &series.candlesticks);
    let hourly: BatchCandlesticksResponse = read("candlesticks/batch_60");
    assert_eq!(series.ticker, hourly.markets[0].market_ticker);
    let mut shared: Vec<(&MarketCandlestick, &MarketCandlestick)> = series
        .candlesticks
        .iter()
        .filter_map(|row| {
            hourly.markets[0]
                .candlesticks
                .iter()
                .find(|other| other.end_period_ts == row.end_period_ts)
                .map(|other| (row, other))
        })
        .collect();
    assert!(!shared.is_empty(), "series_60 and batch_60 share no period");
    shared.sort_by_key(|(row, _)| row.end_period_ts);
    shared.pop();
    for (from_series, from_batch) in shared {
        assert_eq!(from_series, from_batch);
    }
}
```

`read` is the file's existing helper (`read::<T>(fixture)`), which takes a fixture name relative to `tests/fixtures/predictions/`.

Append to the `mod tests` of `polyoxide-kalshi/src/predictions/types/markets.rs`, after its last test `a_state_the_enum_does_not_name_is_kept_verbatim`; replace

```rust
        assert_eq!(MarketStatus::from_wire("active"), MarketStatus::Active);
    }
}
```

with:

```rust
        assert_eq!(MarketStatus::from_wire("active"), MarketStatus::Active);
    }

    #[test]
    fn a_price_distribution_keeps_every_digit_and_leaves_out_what_the_host_leaves_out() {
        let price: PriceDistribution =
            serde_json::from_str(r#"{"previous_dollars":"0.4200"}"#).unwrap();
        assert_eq!(
            price.previous_dollars.map(|p| p.to_string()),
            Some("0.4200".to_owned())
        );
        assert!(price.open_dollars.is_none() && price.close_dollars.is_none());
        assert_eq!(
            serde_json::to_string(&price).unwrap(),
            r#"{"previous_dollars":"0.4200"}"#
        );

        let none: PriceDistribution = serde_json::from_str("{}").unwrap();
        assert_eq!(serde_json::to_string(&none).unwrap(), "{}");
    }

    #[test]
    fn a_period_interval_is_its_width_in_minutes_and_nothing_else_parses() {
        assert_eq!(PeriodInterval::Minute1.to_string(), "1");
        assert_eq!(PeriodInterval::Hour1.to_string(), "60");
        assert_eq!(PeriodInterval::Day1.to_string(), "1440");
        assert_eq!(
            "1440".parse::<PeriodInterval>().ok(),
            Some(PeriodInterval::Day1)
        );
        for other in ["0", "5", "15", "240", "10080", "hour"] {
            assert!(other.parse::<PeriodInterval>().is_err(), "{other}");
        }
    }
}
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test wire_agreement_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib types::markets
```

Expected: each fails to compile, `error[E0432]: unresolved imports polyoxide_kalshi::predictions::types::BatchCandlesticksResponse, ...` for the two integration tests and `error[E0412]: cannot find type PriceDistribution in this scope` for the unit tests.

- [ ] **Step 3: Implement**

In `polyoxide-kalshi/src/predictions/types/markets.rs`, replace the first line:

```rust
//! Markets, their order books and their public trades.
```

with:

```rust
//! Markets, their order books, their public trades and their candlesticks.
```

and insert the candlestick types directly above the test module: replace

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_a_positional_pair_of_strings_that_keeps_every_digit() {
```

with:

```rust
polyoxide_venue::wire_enum! {
    /// The `period_interval` of the candlestick routes: a candlestick's width in minutes. Kalshi
    /// serves these three. The batch route answers any other number with `200` and no rows and
    /// the series route with a `400`, so the closed set is what keeps the native client from
    /// sending one.
    #[non_exhaustive]
    pub enum PeriodInterval {
        /// One minute: wire value `1`.
        Minute1 => "1",
        /// One hour: wire value `60`.
        Hour1 => "60",
        /// One day: wire value `1440`.
        Day1 => "1440",
    }
}

/// Offers on one side of the market over a candlestick's period (`BidAskDistribution`): the
/// first, lowest, highest and last price, in dollars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidAskDistribution {
    /// The price at the start of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_dollars: Decimal,
    /// The lowest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub low_dollars: Decimal,
    /// The highest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub high_dollars: Decimal,
    /// The price at the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub close_dollars: Decimal,
}

/// Trade prices over a candlestick's period (`PriceDistribution`), in dollars. Kalshi leaves a
/// field out when it has no value: a period with no trade has only `previous_dollars`, and a
/// period before the market's first trade has none of them (`{}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceDistribution {
    /// The first YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub open_dollars: Option<Decimal>,
    /// The lowest YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub low_dollars: Option<Decimal>,
    /// The highest YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub high_dollars: Option<Decimal>,
    /// The last YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub close_dollars: Option<Decimal>,
    /// The mean YES trade price of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub mean_dollars: Option<Decimal>,
    /// The last YES trade before the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub previous_dollars: Option<Decimal>,
    /// The mirror's `min_dollars`, which Kalshi has not been seen to send.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub min_dollars: Option<Decimal>,
    /// The mirror's `max_dollars`, which Kalshi has not been seen to send.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub max_dollars: Option<Decimal>,
}

/// One candlestick (`MarketCandlestick`). Kalshi names it by when its period ends, inclusive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketCandlestick {
    /// The end of the period, in Unix seconds.
    pub end_period_ts: i64,
    /// Offers to buy YES.
    pub yes_bid: BidAskDistribution,
    /// Offers to sell YES.
    pub yes_ask: BidAskDistribution,
    /// Trades.
    pub price: PriceDistribution,
    /// Contracts traded in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_fp: Decimal,
    /// Contracts bought, without netting, by the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest_fp: Decimal,
}

/// One market's candlesticks in the batch answer (`MarketCandlesticksResponse`), oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketCandlesticks {
    /// The market's ticker.
    pub market_ticker: String,
    /// Its candlesticks.
    pub candlesticks: Vec<MarketCandlestick>,
}

/// `GET /markets/candlesticks` (`BatchGetMarketCandlesticksResponse`): one entry per market.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchCandlesticksResponse {
    /// The markets asked for.
    pub markets: Vec<MarketCandlesticks>,
}

/// `GET /series/{series_ticker}/markets/{ticker}/candlesticks`
/// (`GetMarketCandlesticksResponse`): one market's candlesticks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesCandlesticksResponse {
    /// The market's ticker.
    pub ticker: String,
    /// Its candlesticks.
    pub candlesticks: Vec<MarketCandlestick>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_a_positional_pair_of_strings_that_keeps_every_digit() {
```

In `polyoxide-kalshi/src/predictions/types/mod.rs`, replace

```rust
pub use markets::{
    BookSide, Market, MarketResponse, MarketStatus, MarketStatusFilter, MarketsPage, MveFilter,
    OrderbookFp, OrderbookResponse, OutcomeSide, PriceLevel, PriceRange, Trade, TradesPage,
};
```

with:

```rust
pub use markets::{
    BatchCandlesticksResponse, BidAskDistribution, BookSide, Market, MarketCandlestick,
    MarketCandlesticks, MarketResponse, MarketStatus, MarketStatusFilter, MarketsPage, MveFilter,
    OrderbookFp, OrderbookResponse, OutcomeSide, PeriodInterval, PriceDistribution, PriceLevel,
    PriceRange, SeriesCandlesticksResponse, Trade, TradesPage,
};
```

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib types::markets
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test wire_agreement_predictions
```

Expected: `test result: ok.` for all three. If `every_candlestick_fixture_agrees_with_its_type` reports a key the host sent that the types do not model, or a row that does not decode (a `null` where a type is required, a missing `yes_bid` key), the capture contradicts the mirror: stop and report it to the owner (Story 1's findings decide the type), do not loosen a type to make the test pass.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git commit -m "feat(kalshi): PeriodInterval and the candlestick types (Epic 9, Story 3)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -- polyoxide-kalshi/src/predictions/types/markets.rs polyoxide-kalshi/src/predictions/types/mod.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/tests/wire_agreement_predictions.rs
```

### Task 3.3: The native candlestick builders

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/api/markets.rs:5-12,48-53,137-138`
- Test: `polyoxide-kalshi/tests/mock_predictions_routes.rs`
- Test: `polyoxide-kalshi/tests/query_setters_predictions.rs`
- Test: `polyoxide-kalshi/tests/spec_agreement_predictions.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-kalshi/tests/mock_predictions_routes.rs`, replace the import line

```rust
    types::{MarketStatus, MarketStatusFilter, MveFilter, OutcomeSide},
```

with:

```rust
    types::{
        MarketCandlestick, MarketStatus, MarketStatusFilter, MveFilter, OutcomeSide, PeriodInterval,
        PriceDistribution,
    },
```

and replace `use polyoxide_test_support::fixtures;` with:

```rust
use polyoxide_test_support::fixtures;
use rust_decimal::Decimal;
```

Append after the file's last test (`templates_match_segment_by_segment_and_a_star_takes_the_rest`):

```rust
// ── candlesticks ────────────────────────────────────────────────

/// A candlestick's `price` field by its wire key.
fn price_of(price: &PriceDistribution, key: &str) -> Option<Decimal> {
    match key {
        "open_dollars" => price.open_dollars,
        "low_dollars" => price.low_dollars,
        "high_dollars" => price.high_dollars,
        "close_dollars" => price.close_dollars,
        "mean_dollars" => price.mean_dollars,
        "previous_dollars" => price.previous_dollars,
        "min_dollars" => price.min_dollars,
        "max_dollars" => price.max_dollars,
        other => panic!("{other} is not a price field"),
    }
}

/// Every decoded candlestick holds the digits and the presence of the captured row it came from.
fn rows_agree(candles: &[MarketCandlestick], rows: &[Value], what: &str) {
    assert_eq!(candles.len(), rows.len(), "{what}");
    assert!(!rows.is_empty(), "{what}: the capture holds no candlestick");
    for (candle, row) in candles.iter().zip(rows) {
        assert_eq!(
            candle.end_period_ts,
            row["end_period_ts"].as_i64().unwrap(),
            "{what}"
        );
        assert_eq!(
            candle.volume_fp.to_string(),
            row["volume_fp"].as_str().unwrap(),
            "{what}"
        );
        assert_eq!(
            candle.open_interest_fp.to_string(),
            row["open_interest_fp"].as_str().unwrap(),
            "{what}"
        );
        for (side, offers) in [("yes_bid", &candle.yes_bid), ("yes_ask", &candle.yes_ask)] {
            for (key, value) in [
                ("open_dollars", offers.open_dollars),
                ("low_dollars", offers.low_dollars),
                ("high_dollars", offers.high_dollars),
                ("close_dollars", offers.close_dollars),
            ] {
                assert_eq!(
                    value.to_string(),
                    row[side][key].as_str().unwrap(),
                    "{what} {side}.{key}"
                );
            }
        }
        for key in [
            "open_dollars",
            "low_dollars",
            "high_dollars",
            "close_dollars",
            "mean_dollars",
            "previous_dollars",
            "min_dollars",
            "max_dollars",
        ] {
            assert_eq!(
                price_of(&candle.price, key).map(|price| price.to_string()),
                row["price"][key].as_str().map(str::to_owned),
                "{what} price.{key}"
            );
        }
    }
}

#[tokio::test]
async fn the_batch_candlesticks_send_their_query_and_decode_each_captured_width() {
    for (file, period, minutes) in [
        ("batch_1", PeriodInterval::Minute1, "1"),
        ("batch_60", PeriodInterval::Hour1, "60"),
        ("batch_1440", PeriodInterval::Day1, "1440"),
    ] {
        let name = format!("candlesticks/{file}");
        let sent = wire(&name);
        let market = sent["markets"][0]["market_ticker"].as_str().unwrap();
        let mut server = Server::new_async().await;
        let mock = route(
            &mut server,
            "/trade-api/v2/markets/candlesticks",
            &format!(
                "market_tickers={market}&start_ts=100&end_ts=200&period_interval={minutes}&include_latest_before_start=true"
            ),
            &fixture(&name),
        )
        .await;
        let read = client(&server)
            .markets()
            .candlesticks(&ticker(market), period, 100, 200)
            .include_latest_before_start(true)
            .send()
            .await
            .unwrap();
        mock.assert_async().await;

        assert_eq!(
            read.markets.len(),
            sent["markets"].as_array().unwrap().len(),
            "{file}"
        );
        assert_eq!(read.markets[0].market_ticker, market, "{file}");
        rows_agree(
            &read.markets[0].candlesticks,
            sent["markets"][0]["candlesticks"].as_array().unwrap(),
            file,
        );
    }
}

#[tokio::test]
async fn the_series_candlesticks_send_their_path_and_query_and_decode_the_captured_body() {
    let sent = wire("candlesticks/series_60");
    let market = sent["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/series/KXSER-1/markets/{market}/candlesticks"),
        "start_ts=100&end_ts=200&period_interval=60",
        &fixture("candlesticks/series_60"),
    )
    .await;
    let read = client(&server)
        .markets()
        .series_candlesticks("KXSER-1", &ticker(market), PeriodInterval::Hour1, 100, 200)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.ticker, market);
    rows_agree(
        &read.candlesticks,
        sent["candlesticks"].as_array().unwrap(),
        "series_60",
    );
}

#[tokio::test]
async fn a_width_the_series_route_refuses_is_the_venues_400() {
    let body = wire("candlesticks/error_width");
    let mut server = Server::new_async().await;
    let _mock = server
        .mock(
            "GET",
            "/trade-api/v2/series/KXSER-1/markets/KXTEST-1/candlesticks",
        )
        .match_query(Matcher::Any)
        .with_status(400)
        .with_body(fixture("candlesticks/error_width"))
        .create_async()
        .await;
    let err = client_with_retries(&server, NO_RETRIES)
        .markets()
        .series_candlesticks("KXSER-1", &ticker("KXTEST-1"), PeriodInterval::Hour1, 100, 200)
        .send()
        .await
        .unwrap_err();
    let PredictionsError::Venue(venue) = &err else {
        panic!("not a venue error: {err:?}");
    };
    // The body's own message, whichever of the three shapes the host used.
    let message = body["msg"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .or_else(|| body["error"]["message"].as_str())
        .unwrap();
    assert_eq!(usize::from(venue.status), 400);
    assert_eq!(venue.message, message);
    assert!(
        matches!(err.class(), Class::VenueRefusal { .. }),
        "{:?}",
        err.class()
    );
}
```

In `polyoxide-kalshi/tests/query_setters_predictions.rs`, replace

```rust
    types::{MarketStatusFilter, MveFilter},
```

with:

```rust
    types::{MarketStatusFilter, MveFilter, PeriodInterval},
```

and append after the file's last test (`every_setter_sends_its_key_and_value_in_call_order`):

```rust
/// The candlestick builders: the required arguments first, in the order the route documents
/// them, then `include_latest_before_start`. Each width sends its number of minutes.
const CANDLE_CASES: &[Case] = &[
    Case {
        builder: "GetCandlesticks",
        path: "/trade-api/v2/markets/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .candlesticks(&ticker(), PeriodInterval::Hour1, 100, 200)
                    .include_latest_before_start(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("market_tickers", "KXTEST-1"),
            ("start_ts", "100"),
            ("end_ts", "200"),
            ("period_interval", "60"),
            ("include_latest_before_start", "true"),
        ],
    },
    Case {
        builder: "GetCandlesticks, one minute",
        path: "/trade-api/v2/markets/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .candlesticks(&ticker(), PeriodInterval::Minute1, 1, 2)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("market_tickers", "KXTEST-1"),
            ("start_ts", "1"),
            ("end_ts", "2"),
            ("period_interval", "1"),
        ],
    },
    Case {
        builder: "GetCandlesticks, one day, a setter called twice",
        path: "/trade-api/v2/markets/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .candlesticks(&ticker(), PeriodInterval::Day1, 3, 4)
                    .include_latest_before_start(true)
                    .include_latest_before_start(false)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("market_tickers", "KXTEST-1"),
            ("start_ts", "3"),
            ("end_ts", "4"),
            ("period_interval", "1440"),
            ("include_latest_before_start", "false"),
        ],
    },
    Case {
        builder: "GetSeriesCandlesticks",
        path: "/trade-api/v2/series/KXSER-1/markets/KXTEST-1/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .series_candlesticks("KXSER-1", &ticker(), PeriodInterval::Hour1, 100, 200)
                    .include_latest_before_start(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("start_ts", "100"),
            ("end_ts", "200"),
            ("period_interval", "60"),
            ("include_latest_before_start", "true"),
        ],
    },
];

#[tokio::test]
async fn every_candle_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(CANDLE_CASES).await;
}
```

In `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, append after `the_period_intervals_are_the_widths_the_mirror_documents`:

```rust
#[tokio::test]
async fn the_candle_builders_send_exactly_the_documented_query_keys() {
    let spec = mirror();
    let batch = keys_of("/markets/candlesticks", "candlesticks/batch_60", |p| {
        Box::pin(async move {
            let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
            p.markets()
                .candlesticks(&ticker, PeriodInterval::Hour1, 1, 2)
                .include_latest_before_start(true)
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    assert_eq!(
        Some(batch),
        query::documented_parameters(&spec, "/markets/candlesticks")
    );

    let series = keys_of(
        "/series/KXSER-1/markets/KXTEST-1/candlesticks",
        "candlesticks/series_60",
        |p| {
            Box::pin(async move {
                let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
                p.markets()
                    .series_candlesticks("KXSER-1", &ticker, PeriodInterval::Hour1, 1, 2)
                    .include_latest_before_start(true)
                    .send()
                    .await
                    .map(|_| ())
            })
        },
    )
    .await;
    assert_eq!(
        Some(series),
        query::documented_parameters(&spec, "/series/{series_ticker}/markets/{ticker}/candlesticks")
    );
}
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_routes
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test query_setters_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions
```

Expected: each fails to compile, `error[E0599]: no method named candlesticks found for struct MarketsApi in the current scope` (and the same for `series_candlesticks`).

- [ ] **Step 3: Implement**

In `polyoxide-kalshi/src/predictions/api/markets.rs`, replace the type imports (current lines 5-12):

```rust
use crate::predictions::{
    key::Ticker,
    request::{routes, Routed},
    types::{
        MarketResponse, MarketStatusFilter, MarketsPage, MveFilter, OrderbookResponse, TradesPage,
    },
    Inner, PredictionsError,
};
```

with:

```rust
use crate::predictions::{
    key::Ticker,
    request::{routes, Routed},
    types::{
        BatchCandlesticksResponse, MarketResponse, MarketStatusFilter, MarketsPage, MveFilter,
        OrderbookResponse, PeriodInterval, SeriesCandlesticksResponse, TradesPage,
    },
    Inner, PredictionsError,
};
```

Add the two routes to `MarketsApi`: replace

```rust
    pub fn trades(&self) -> ListTrades {
        ListTrades {
            request: Routed::new(&self.inner, routes::TRADES, &[]),
        }
    }
}
```

with:

```rust
    pub fn trades(&self) -> ListTrades {
        ListTrades {
            request: Routed::new(&self.inner, routes::TRADES, &[]),
        }
    }

    /// `GET /trade-api/v2/markets/candlesticks` for one market: its candlesticks of one width
    /// that end between `start_ts` and `end_ts`, both inclusive and in Unix seconds.
    ///
    /// Kalshi selects a candlestick by when it ends, so a span of open times is shifted forward
    /// by one width. The route answers up to 10,000 candlesticks across the markets asked for.
    /// A market that settled before Kalshi's historical cutoff has candlesticks only on
    /// `GET /historical/markets/{ticker}/candlesticks`, which this client does not call.
    pub fn candlesticks(
        &self,
        ticker: &Ticker,
        period: PeriodInterval,
        start_ts: i64,
        end_ts: i64,
    ) -> GetCandlesticks {
        GetCandlesticks {
            request: Routed::new(&self.inner, routes::CANDLESTICKS, &[])
                .query("market_tickers", ticker.as_str())
                .query("start_ts", start_ts)
                .query("end_ts", end_ts)
                .query("period_interval", period),
        }
    }

    /// `GET /trade-api/v2/series/{series_ticker}/markets/{ticker}/candlesticks`: the same
    /// candlesticks as [`candlesticks`](Self::candlesticks), for the market under its series.
    pub fn series_candlesticks(
        &self,
        series_ticker: &str,
        ticker: &Ticker,
        period: PeriodInterval,
        start_ts: i64,
        end_ts: i64,
    ) -> GetSeriesCandlesticks {
        GetSeriesCandlesticks {
            request: Routed::new(
                &self.inner,
                routes::SERIES_CANDLESTICKS,
                &[series_ticker, ticker.as_str()],
            )
            .query("start_ts", start_ts)
            .query("end_ts", end_ts)
            .query("period_interval", period),
        }
    }
}
```

and add the builders at the end of the file: replace

```rust
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<TradesPage, PredictionsError> {
        self.request.send().await
    }
}
```

with:

```rust
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<TradesPage, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/candlesticks`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetCandlesticks {
    pub(crate) request: Routed<BatchCandlesticksResponse>,
}

impl GetCandlesticks {
    polyoxide_core::query_setters! {
        /// Also prepend the latest candlestick before `start_ts`, projected to the first period
        /// boundary with every price null and `previous_dollars` set to its close.
        include_latest_before_start: bool => "include_latest_before_start",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<BatchCandlesticksResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/series/{series_ticker}/markets/{ticker}/candlesticks`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetSeriesCandlesticks {
    pub(crate) request: Routed<SeriesCandlesticksResponse>,
}

impl GetSeriesCandlesticks {
    polyoxide_core::query_setters! {
        /// Also prepend the latest candlestick before `start_ts`, projected to the first period
        /// boundary with every price null and `previous_dollars` set to its close.
        include_latest_before_start: bool => "include_latest_before_start",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<SeriesCandlesticksResponse, PredictionsError> {
        self.request.send().await
    }
}
```

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_routes
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test query_setters_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions
```

Expected: `test result: ok.` for all three. Without the `.query("start_ts", ..)` order or the `include_latest_before_start` setter, `every_candle_setter_sends_its_key_and_value_in_call_order` fails on the pairs it names.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git commit -m "feat(kalshi): markets().candlesticks and series_candlesticks (Epic 9, Story 3)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -- polyoxide-kalshi/src/predictions/api/markets.rs polyoxide-kalshi/tests/mock_predictions_routes.rs polyoxide-kalshi/tests/query_setters_predictions.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs
```

### Task 3.4: `Candles` on `Predictions`

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/types/markets.rs`
- Modify: `polyoxide-kalshi/src/predictions/venue.rs:3-4,22-24,30,36-45,272-273`
- Test: `polyoxide-kalshi/tests/venue_predictions.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-kalshi/tests/venue_predictions.rs`, replace the import line

```rust
    venue::{EventGroupExt, InstrumentExt, TradeExt},
```

with:

```rust
    venue::{CandleExt, EventGroupExt, InstrumentExt, TradeExt},
```

replace `use polyoxide_test_support::fixtures;` with:

```rust
use polyoxide_test_support::{fixtures, query::pairs_sent};
```

and replace `use polyoxide_venue::{Side, TradeId, Trades};` with:

```rust
use polyoxide_venue::{Candle, CandleInterval, CandleQuery, Candles, DynCandles};
use polyoxide_venue::{Side, TradeId, Trades};
```

Append after the file's last test (`a_key_of_another_venue_is_refused_by_event_group_with_nothing_sent`):

```rust
// ── candles ─────────────────────────────────────────────────────

const CANDLES: &str = "/trade-api/v2/markets/candlesticks";

/// A whole number of days after the epoch, so the candles of every served width open on a
/// boundary of their own width.
const T0: i64 = 1_799_971_200;

/// Unix seconds as milliseconds.
fn ms(seconds: i64) -> u64 {
    u64::try_from(seconds).unwrap() * 1000
}

fn decimal(text: &str) -> Decimal {
    text.parse().unwrap()
}

/// The batch route's query, in the order the client writes it.
fn candles_query(ticker: &Ticker, start_ts: i64, end_ts: i64, minutes: u32) -> Matcher {
    Matcher::Exact(format!(
        "market_tickers={ticker}&start_ts={start_ts}&end_ts={end_ts}&period_interval={minutes}"
    ))
}

/// One candlestick as Kalshi sends it, ending at `end` Unix seconds with the given trade prices.
fn row(end: i64, price: Value) -> Value {
    json!({
        "end_period_ts": end,
        "yes_bid": {
            "open_dollars": "0.3800", "low_dollars": "0.3500",
            "high_dollars": "0.4400", "close_dollars": "0.4000"
        },
        "yes_ask": {
            "open_dollars": "0.4200", "low_dollars": "0.4000",
            "high_dollars": "0.4800", "close_dollars": "0.4500"
        },
        "price": price,
        "volume_fp": "12.00",
        "open_interest_fp": "345.00",
    })
}

/// A period with trades: YES open 0.40, low 0.38, high 0.45, close 0.42, mean 0.41.
fn traded() -> Value {
    json!({
        "open_dollars": "0.4000", "low_dollars": "0.3800", "high_dollars": "0.4500",
        "close_dollars": "0.4200", "mean_dollars": "0.4100", "previous_dollars": "0.3900"
    })
}

fn batch_body(ticker: &Ticker, rows: Vec<Value>) -> String {
    json!({"markets": [{"market_ticker": ticker.as_str(), "candlesticks": rows}]}).to_string()
}

/// `candles` on a mock server that serves `body` once, for the test ticker's `outcome`, and
/// requires the request to be the one the span and width name.
async fn candles_of(
    body: String,
    outcome: Outcome,
    query: CandleQuery,
    (start_ts, end_ts, minutes): (i64, i64, u32),
) -> Result<Vec<Candle>, ClassifiedError> {
    let ticker = test_ticker();
    let mut server = start().await;
    let mock = route(
        &mut server,
        CANDLES,
        candles_query(&ticker, start_ts, end_ts, minutes),
        &body,
        1,
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let candles = venue.candles(&key::key(&ticker, outcome), &query).await;
    mock.assert_async().await;
    candles
}

#[tokio::test]
async fn kalshi_serves_candles_of_a_minute_an_hour_and_a_day() {
    let server = start().await;
    let served = [
        CandleInterval::Minute1,
        CandleInterval::Hour1,
        CandleInterval::Day1,
    ];
    assert_eq!(client(&server).intervals(), served);
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    assert_eq!(venue.intervals(), served);
}

#[tokio::test]
async fn a_width_kalshi_does_not_serve_is_refused_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let key = key::key(&test_ticker(), Outcome::Yes);
    for width in [
        CandleInterval::Minute5,
        CandleInterval::Minute15,
        CandleInterval::Hour4,
        CandleInterval::Week1,
    ] {
        let err = venue
            .candles(&key, &CandleQuery::new(width, UnixMillis(0)))
            .await
            .unwrap_err();
        assert_eq!(err.class, Class::InvalidRequest, "{width:?}");
        assert_eq!(
            err.to_string(),
            format!("`{width:?}` candles are not served; this venue serves Minute1, Hour1, Day1")
        );
    }
    nothing.assert_async().await;
}

#[tokio::test]
async fn a_key_of_another_venue_is_refused_by_candles_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let err = venue
        .candles(
            &foreign_key(),
            &CandleQuery::new(CandleInterval::Hour1, UnixMillis(0)),
        )
        .await
        .unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    nothing.assert_async().await;
}

#[tokio::test]
async fn the_span_is_the_query_rounded_inward_to_the_second_and_moved_forward_by_one_width() {
    let ticker = test_ticker();
    let key = key::key(&ticker, Outcome::Yes);
    for (interval, width, minutes) in [
        (CandleInterval::Minute1, 60, 1),
        (CandleInterval::Hour1, 3_600, 60),
        (CandleInterval::Day1, 86_400, 1440),
    ] {
        // The start is 1 ms into second T0 and rounds up to T0 + 1; the end is 999 ms into
        // T0 + 3600 and rounds down to T0 + 3600. Kalshi selects by a candle's end, so both
        // move forward by the width.
        let mut server = start().await;
        let mock = route(
            &mut server,
            CANDLES,
            candles_query(&ticker, T0 + 1 + width, T0 + 3_600 + width, minutes),
            &batch_body(&ticker, vec![]),
            1,
        )
        .await;
        let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
        let query =
            CandleQuery::new(interval, UnixMillis(ms(T0) + 1)).end(UnixMillis(ms(T0 + 3_600) + 999));
        let candles = venue.candles(&key, &query).await.unwrap();
        mock.assert_async().await;
        assert!(candles.is_empty(), "{interval:?}");
    }
}

#[tokio::test]
async fn a_query_with_no_end_reads_up_to_now_and_sends_no_latest_before_start() {
    let key = key::key(&test_ticker(), Outcome::Yes);
    let before = UnixMillis::now().0 / 1000;
    let pairs = pairs_sent(CANDLES, |url| {
        let key = key.clone();
        async move {
            let predictions = Predictions::builder()
                .base_url(url)
                .with_retry_config(once())
                .build()
                .unwrap();
            let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(predictions);
            let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(ms(T0)));
            // The body served is `{}`, which does not decode; only the request matters.
            let _ = venue.candles(&key, &query).await;
        }
    })
    .await;
    let after = UnixMillis::now().0 / 1000;

    let keys: Vec<&str> = pairs.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        ["market_tickers", "start_ts", "end_ts", "period_interval"]
    );
    let value = |name: &str| -> u64 {
        pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.parse().unwrap())
            .unwrap()
    };
    assert_eq!(value("start_ts"), u64::try_from(T0 + 3_600).unwrap());
    assert!(
        (before + 3_600..=after + 3_600).contains(&value("end_ts")),
        "{pairs:?}"
    );
    assert_eq!(value("period_interval"), 60);
}

#[tokio::test]
async fn a_traded_period_maps_its_trade_prices_and_an_untraded_one_closes_at_its_previous() {
    let ticker = test_ticker();
    // YES prices, then what a NO key sees: each price p is 1 - p, and high and low swap.
    for (outcome, (open, high, low, close, mean, carried)) in [
        (Outcome::Yes, ("0.40", "0.45", "0.38", "0.42", "0.41", "0.42")),
        (Outcome::No, ("0.60", "0.62", "0.55", "0.58", "0.59", "0.58")),
    ] {
        let body = batch_body(
            &ticker,
            vec![
                // Opens at T0, traded.
                row(T0 + 3_600, traded()),
                // Opens at T0 + 3600, no trade: carried by the price before it.
                row(T0 + 7_200, json!({"previous_dollars": "0.4200"})),
                // Opens at T0 + 7200, nothing traded before it either: left out.
                row(T0 + 10_800, json!({})),
                // Opens at T0 + 10800, traded.
                row(T0 + 14_400, traded()),
            ],
        );
        let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(ms(T0)))
            .end(UnixMillis(ms(T0 + 10_800)));
        let candles = candles_of(body, outcome, query, (T0 + 3_600, T0 + 14_400, 60))
            .await
            .unwrap();

        let opens: Vec<u64> = candles.iter().map(|candle| candle.open_time.0).collect();
        assert_eq!(opens, [ms(T0), ms(T0 + 3_600), ms(T0 + 10_800)], "{outcome:?}");
        let expected_key = key::key(&ticker, outcome);
        for candle in &candles {
            assert_eq!(candle.key, expected_key);
            assert_eq!(candle.interval, CandleInterval::Hour1);
            assert_eq!(candle.volume, Some(decimal("12")), "{outcome:?}");
            assert_eq!(candle.notional, None);
            assert_eq!(candle.trades, None);
        }

        let first = &candles[0];
        assert_eq!(
            (first.open, first.high, first.low, first.close),
            (
                Some(decimal(open)),
                Some(decimal(high)),
                Some(decimal(low)),
                decimal(close)
            ),
            "{outcome:?}"
        );
        let ext = first.extensions.get::<CandleExt>().unwrap();
        assert_eq!(ext.mean, Some(decimal(mean)), "{outcome:?}");
        // The bid and ask distributions stay in YES terms for either key.
        assert_eq!(
            [ext.yes_bid.open, ext.yes_bid.high, ext.yes_bid.low, ext.yes_bid.close],
            [decimal("0.38"), decimal("0.44"), decimal("0.35"), decimal("0.40")]
        );
        assert_eq!(
            [ext.yes_ask.open, ext.yes_ask.high, ext.yes_ask.low, ext.yes_ask.close],
            [decimal("0.42"), decimal("0.48"), decimal("0.40"), decimal("0.45")]
        );
        assert_eq!(ext.open_interest, decimal("345"));

        // An untraded period: a close and nothing else, and no mean.
        let carried_candle = &candles[1];
        assert_eq!(
            (
                carried_candle.open,
                carried_candle.high,
                carried_candle.low,
                carried_candle.close
            ),
            (None, None, None, decimal(carried)),
            "{outcome:?}"
        );
        assert_eq!(carried_candle.volume, Some(decimal("12")));
        let carried_ext = carried_candle.extensions.get::<CandleExt>().unwrap();
        assert_eq!(carried_ext.mean, None);
        assert_eq!(carried_ext.open_interest, decimal("345"));

        assert_eq!(candles[2].close, decimal(close), "{outcome:?}");
    }
}

#[tokio::test]
async fn a_candle_opening_outside_the_query_is_dropped_and_the_rest_come_oldest_first() {
    let ticker = test_ticker();
    // Served newest first. Their open times are T0 + 7200 (after the query's end), T0 + 3600,
    // T0, and T0 - 3600 (before its start); the span Kalshi selects by is sloppier than the
    // query, so the client holds the line itself, ends inclusive.
    let body = batch_body(
        &ticker,
        vec![
            row(T0 + 10_800, traded()),
            row(T0 + 7_200, traded()),
            row(T0 + 3_600, traded()),
            row(T0, traded()),
        ],
    );
    let query =
        CandleQuery::new(CandleInterval::Hour1, UnixMillis(ms(T0))).end(UnixMillis(ms(T0 + 3_600)));
    let candles = candles_of(body, Outcome::Yes, query, (T0 + 3_600, T0 + 7_200, 60))
        .await
        .unwrap();
    let opens: Vec<u64> = candles.iter().map(|candle| candle.open_time.0).collect();
    assert_eq!(opens, [ms(T0), ms(T0 + 3_600)]);
}

#[tokio::test]
async fn a_market_the_answer_does_not_list_has_no_candles() {
    let query = || {
        CandleQuery::new(CandleInterval::Hour1, UnixMillis(ms(T0))).end(UnixMillis(ms(T0 + 3_600)))
    };
    let span = (T0 + 3_600, T0 + 7_200, 60);
    let none = candles_of(
        json!({"markets": []}).to_string(),
        Outcome::Yes,
        query(),
        span,
    )
    .await
    .unwrap();
    assert!(none.is_empty());

    let other = json!({"markets": [{"market_ticker": "KXOTHER-1", "candlesticks": [
        row(T0 + 3_600, traded())
    ]}]})
    .to_string();
    let none = candles_of(other, Outcome::Yes, query(), span).await.unwrap();
    assert!(none.is_empty(), "another market's candles are not this one's");
}

#[tokio::test]
async fn a_candle_that_ends_before_one_width_after_the_epoch_is_a_decode_failure() {
    let ticker = test_ticker();
    let body = batch_body(&ticker, vec![row(60, traded())]);
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(0)).end(UnixMillis(ms(3_600)));
    let err = candles_of(body, Outcome::Yes, query, (3_600, 7_200, 60))
        .await
        .unwrap_err();
    assert_eq!(err.class, Class::Decode);
    assert!(err.to_string().contains("end_period_ts"), "{err}");
}

#[tokio::test]
async fn the_venues_refusal_passes_through_as_its_own() {
    let mut server = start().await;
    let _mock = server
        .mock("GET", CANDLES)
        .match_query(Matcher::Any)
        .with_status(400)
        .with_body(fixture("error_validation"))
        .create_async()
        .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let key = key::key(&test_ticker(), Outcome::Yes);
    let err = venue
        .candles(&key, &CandleQuery::new(CandleInterval::Hour1, UnixMillis(0)))
        .await
        .unwrap_err();
    assert!(
        matches!(err.class, Class::VenueRefusal { .. }),
        "{:?}",
        err.class
    );
}

#[tokio::test]
async fn each_captured_width_maps_row_by_row_for_both_outcomes() {
    for (file, interval, width, minutes) in [
        ("batch_1", CandleInterval::Minute1, 60, 1),
        ("batch_60", CandleInterval::Hour1, 3_600, 60),
        ("batch_1440", CandleInterval::Day1, 86_400, 1440),
    ] {
        let name = format!("candlesticks/{file}");
        let sent = fixture_json(&name);
        let ticker = Ticker::new(sent["markets"][0]["market_ticker"].as_str().unwrap()).unwrap();
        let mut rows: Vec<&Value> = sent["markets"][0]["candlesticks"]
            .as_array()
            .unwrap()
            .iter()
            .collect();
        rows.sort_by_key(|row| row["end_period_ts"].as_i64().unwrap());
        let first = rows[0]["end_period_ts"].as_i64().unwrap();
        let last = rows[rows.len() - 1]["end_period_ts"].as_i64().unwrap();
        // Half a second before the first open and just under one after the last one: rounded
        // inward, they select exactly the captured span.
        let query = CandleQuery::new(interval, UnixMillis(ms(first - width) - 500))
            .end(UnixMillis(ms(last - width) + 999));

        for outcome in [Outcome::Yes, Outcome::No] {
            let candles = candles_of(
                fixture(&name),
                outcome,
                query,
                (first, last, minutes),
            )
            .await
            .unwrap();
            let flip = |price: Decimal| match outcome {
                Outcome::Yes => price,
                Outcome::No => Decimal::ONE - price,
            };

            let mut mapped = candles.iter();
            let (mut traded_rows, mut carried_rows) = (0, 0);
            for row in &rows {
                let price = &row["price"];
                let read = |field: &str| price[field].as_str().map(decimal);
                let (open, high, low, close) = match (read("close_dollars"), read("previous_dollars")) {
                    (Some(close), _) => {
                        traded_rows += 1;
                        let (high, low) = match outcome {
                            Outcome::Yes => (read("high_dollars"), read("low_dollars")),
                            Outcome::No => (read("low_dollars"), read("high_dollars")),
                        };
                        (
                            read("open_dollars").map(flip),
                            high.map(flip),
                            low.map(flip),
                            flip(close),
                        )
                    }
                    (None, Some(previous)) => {
                        carried_rows += 1;
                        (None, None, None, flip(previous))
                    }
                    (None, None) => continue,
                };
                let end = row["end_period_ts"].as_i64().unwrap();
                let Some(candle) = mapped.next() else {
                    panic!("{file}: no candle for the row ending {end}");
                };
                assert_eq!(candle.open_time, UnixMillis(ms(end - width)), "{file}");
                assert_eq!(candle.interval, interval, "{file}");
                assert_eq!(
                    (candle.open, candle.high, candle.low, candle.close),
                    (open, high, low, close),
                    "{file} {outcome:?} ending {end}"
                );
                assert_eq!(
                    candle.volume,
                    Some(decimal(row["volume_fp"].as_str().unwrap())),
                    "{file} ending {end}"
                );
                let ext = candle.extensions.get::<CandleExt>().unwrap();
                assert_eq!(
                    ext.open_interest,
                    decimal(row["open_interest_fp"].as_str().unwrap()),
                    "{file} ending {end}"
                );
                assert_eq!(
                    ext.mean,
                    read("mean_dollars").map(flip),
                    "{file} {outcome:?} ending {end}"
                );
                for (side, offers) in [("yes_bid", &ext.yes_bid), ("yes_ask", &ext.yes_ask)] {
                    for (field, value) in [
                        ("open_dollars", offers.open),
                        ("low_dollars", offers.low),
                        ("high_dollars", offers.high),
                        ("close_dollars", offers.close),
                    ] {
                        assert_eq!(
                            value,
                            decimal(row[side][field].as_str().unwrap()),
                            "{file} {side}.{field} ending {end}"
                        );
                    }
                }
            }
            assert!(mapped.next().is_none(), "{file}: a candle with no row");
            assert!(
                traded_rows > 0 && carried_rows > 0,
                "{file}: the capture holds a traded period and an untraded one that carries previous"
            );
        }
    }
}
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions candles
```

Expected: fails to compile, `error[E0432]: unresolved import polyoxide_kalshi::predictions::venue::CandleExt` and `error[E0277]: the trait bound Predictions: Candles is not satisfied`.

- [ ] **Step 3: Implement**

In `polyoxide-kalshi/src/predictions/types/markets.rs`, add the width in seconds directly below the `PeriodInterval` declaration: replace

```rust
        /// One day: wire value `1440`.
        Day1 => "1440",
    }
}
```

with:

```rust
        /// One day: wire value `1440`.
        Day1 => "1440",
    }
}

impl PeriodInterval {
    /// The width in seconds.
    pub(crate) fn seconds(self) -> i64 {
        match self {
            Self::Minute1 => 60,
            Self::Hour1 => 3_600,
            Self::Day1 => 86_400,
        }
    }
}
```

and append to the `mod tests` of that file, after `a_period_interval_is_its_width_in_minutes_and_nothing_else_parses`; replace its last lines

```rust
        for other in ["0", "5", "15", "240", "10080", "hour"] {
            assert!(other.parse::<PeriodInterval>().is_err(), "{other}");
        }
    }
}
```

with:

```rust
        for other in ["0", "5", "15", "240", "10080", "hour"] {
            assert!(other.parse::<PeriodInterval>().is_err(), "{other}");
        }
    }

    #[test]
    fn a_period_interval_is_as_many_seconds_as_its_minutes() {
        for width in PeriodInterval::ALL {
            let minutes: i64 = width.as_str().parse().unwrap();
            assert_eq!(width.seconds(), minutes * 60);
        }
    }
}
```

In `polyoxide-kalshi/src/predictions/venue.rs`, edit the module docs and imports.

Replace the first doc lines (current lines 3-4):

```rust
//! [`Predictions`] implements `MarketData`, `Trades` and `EventGroups` over its
//! own routes, keyed
```

with:

```rust
//! [`Predictions`] implements `MarketData`, `Trades`, `Candles` and `EventGroups` over its
//! own routes, keyed
```

Replace (current lines 22-24):

```rust
//! not parse is a `Decode` error naming the field, never a zero.
//!
//! | Method | Route | Requests |
```

with:

```rust
//! not parse is a `Decode` error naming the field, never a zero.
//!
//! `candles` reads one ticker from the batch candlestick route at the three widths
//! Kalshi serves, one minute, one hour and one day (`intervals()`). Any other width
//! is refused as `InvalidRequest` with nothing sent, since the route itself answers
//! one it does not serve with `200` and no rows. Kalshi selects a candlestick by
//! the time it ends and the trait by the time it opens, so the request's span is
//! the query's moved forward by one width: `start` rounded up to the second, and
//! `end` (or now) rounded down to the second, each plus the width. A candle opens
//! at its end less one width, and one opening outside the query is dropped. Its
//! prices are the trade prices, and a `no` key's are each turned into *1 - price*
//! with its high and low swapped. A period with no trade closes at the last price
//! before it (`previous`) and has no open, high or low, and a period with no
//! `previous` either is left out. Its volume is the contracts traded in the period,
//! in shares. `CandleExt` carries the YES bid and ask distributions and the open
//! interest in YES terms whatever the key's outcome, and the mean trade price,
//! turned over for a `no` key like the other prices. A market that settled before
//! Kalshi's historical cutoff has candlesticks only on its historical route, which
//! this client does not call: the live route's answer for it, an empty list or a
//! refusal, is returned as the venue gave it.
//!
//! | Method | Route | Requests |
```

Replace (current line 30):

```rust
//! | `trades` | `GET /trade-api/v2/markets/trades` for the ticker, 1000 rows | 1 |
```

with:

```rust
//! | `trades` | `GET /trade-api/v2/markets/trades` for the ticker, 1000 rows | 1 |
//! | `candles` | `GET /trade-api/v2/markets/candlesticks` for the ticker | 1 |
```

Replace the two `use` lines (current lines 40 and 44):

```rust
use polyoxide_venue::{Side, Trade, TradeId, Trades};
```

with:

```rust
use polyoxide_venue::{Candle, CandleInterval, CandleQuery, Candles};
use polyoxide_venue::{Side, Trade, TradeId, Trades};
```

and

```rust
use super::types::{self, MarketStatus, OrderbookFp, OutcomeSide, PriceRange};
```

with:

```rust
use super::types::{
    self, BidAskDistribution, MarketCandlestick, MarketStatus, OrderbookFp, OutcomeSide,
    PeriodInterval, PriceRange,
};
```

Add the implementation directly above the `EventGroupExt` struct: replace the line

```rust
/// Kalshi's facts about an event, beside [`EventGroup`]'s fields.
```

with:

```rust
/// The widths Kalshi's candlestick routes serve, shortest first: a minute, an hour and a day.
const SERVED: &[CandleInterval] = &[
    CandleInterval::Minute1,
    CandleInterval::Hour1,
    CandleInterval::Day1,
];

/// The first, highest, lowest and last price of one side's offers over a candle's period, in
/// dollars.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Ohlc {
    /// The price at the start of the period.
    pub open: Decimal,
    /// The highest price in the period.
    pub high: Decimal,
    /// The lowest price in the period.
    pub low: Decimal,
    /// The price at the end of the period.
    pub close: Decimal,
}

/// Kalshi's facts about a candle, beside [`Candle`]'s fields.
///
/// The bid and ask distributions and the open interest are in YES terms as the venue sends them,
/// whatever outcome the key names; the mean is turned over for a `no` key like the candle's
/// prices.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CandleExt {
    /// Offers to buy YES over the period.
    pub yes_bid: Ohlc,
    /// Offers to sell YES over the period.
    pub yes_ask: Ohlc,
    /// Contracts bought, without netting, by the end of the period.
    pub open_interest: Decimal,
    /// The mean trade price of the period; `None` for a period with no trade.
    pub mean: Option<Decimal>,
}

/// The wire width of a served interval.
fn wire_period(interval: CandleInterval) -> Result<PeriodInterval, ClassifiedError> {
    match interval {
        CandleInterval::Minute1 => Ok(PeriodInterval::Minute1),
        CandleInterval::Hour1 => Ok(PeriodInterval::Hour1),
        CandleInterval::Day1 => Ok(PeriodInterval::Day1),
        other => Err(ClassifiedError::new(
            Class::InvalidRequest,
            format!("{other:?} candles are not served"),
        )),
    }
}

/// A YES price as `outcome` sees it: itself for YES, `1 - price` for NO.
fn for_outcome(outcome: Outcome, price: Decimal) -> Decimal {
    match outcome {
        Outcome::Yes => price,
        Outcome::No => Decimal::ONE - price,
    }
}

/// Unix seconds, rounded up.
fn seconds_up(time: UnixMillis) -> i64 {
    i64::try_from(time.0.div_ceil(1000)).unwrap_or(i64::MAX)
}

/// Unix seconds, rounded down.
fn seconds_down(time: UnixMillis) -> i64 {
    i64::try_from(time.0 / 1000).unwrap_or(i64::MAX)
}

fn ohlc(offers: &BidAskDistribution) -> Ohlc {
    Ohlc {
        open: offers.open_dollars,
        high: offers.high_dollars,
        low: offers.low_dollars,
        close: offers.close_dollars,
    }
}

/// The candle a row describes, for `outcome`, or `None` for a period with neither a trade nor a
/// price before it.
fn to_candle(
    key: &MarketKey,
    outcome: Outcome,
    interval: CandleInterval,
    width: i64,
    row: &MarketCandlestick,
) -> Result<Option<Candle>, ClassifiedError> {
    let open_time = row
        .end_period_ts
        .checked_sub(width)
        .and_then(|seconds| u64::try_from(seconds).ok())
        .and_then(|seconds| seconds.checked_mul(1_000))
        .map(UnixMillis)
        .ok_or_else(|| {
            ClassifiedError::new(
                Class::Decode,
                format!(
                    "end_period_ts {} is less than one width after 1970",
                    row.end_period_ts
                ),
            )
        })?;
    let shown = |price: Decimal| for_outcome(outcome, price);
    let price = &row.price;
    let (open, high, low, close) = match (price.close_dollars, price.previous_dollars) {
        (Some(close), _) => {
            // A NO key's high is the YES low turned over, and its low the YES high.
            let (high, low) = match outcome {
                Outcome::Yes => (price.high_dollars, price.low_dollars),
                Outcome::No => (price.low_dollars, price.high_dollars),
            };
            (
                price.open_dollars.map(shown),
                high.map(shown),
                low.map(shown),
                shown(close),
            )
        }
        (None, Some(previous)) => (None, None, None, shown(previous)),
        (None, None) => return Ok(None),
    };
    let mut candle = Candle::new(key.clone(), interval, open_time, close);
    candle.open = open;
    candle.high = high;
    candle.low = low;
    candle.volume = Some(row.volume_fp);
    candle.extensions = Extensions::of(CandleExt {
        yes_bid: ohlc(&row.yes_bid),
        yes_ask: ohlc(&row.yes_ask),
        open_interest: row.open_interest_fp,
        mean: price.mean_dollars.map(shown),
    });
    Ok(Some(candle))
}

impl Candles for Predictions {
    fn intervals(&self) -> &'static [CandleInterval] {
        SERVED
    }

    /// The market's candles of one served width opening in the query's span, oldest first, in
    /// one request: up to 10,000 of them, which a minute query spans in a week.
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let id = key::outcome(key)?;
        query.ensure_served(self.intervals())?;
        let period = wire_period(query.interval)?;
        let width = period.seconds();
        let end = query.end.unwrap_or_else(UnixMillis::now);
        let response = self
            .markets()
            .candlesticks(
                &id.ticker,
                period,
                seconds_up(query.start).saturating_add(width),
                seconds_down(end).saturating_add(width),
            )
            .send()
            .await?;
        let Some(market) = response
            .markets
            .into_iter()
            .find(|market| market.market_ticker == id.ticker.as_str())
        else {
            return Ok(Vec::new());
        };
        let mut candles = Vec::new();
        for row in &market.candlesticks {
            if let Some(candle) = to_candle(key, id.outcome, query.interval, width, row)? {
                if candle.open_time >= query.start && candle.open_time <= end {
                    candles.push(candle);
                }
            }
        }
        candles.sort_by_key(|candle| candle.open_time);
        Ok(candles)
    }
}

/// Kalshi's facts about an event, beside [`EventGroup`]'s fields.
```

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib
nix develop --command cargo clippy -p polyoxide-kalshi --no-default-features --features predictions --all-targets -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --no-default-features --features predictions'
```

Expected: `test result: ok.` for the suites, then clippy and rustdoc print nothing to fix. Each rule of the mapping has a test that fails without it: without the `+ width` the exact query of `the_span_is_the_query_rounded_inward_to_the_second_and_moved_forward_by_one_width` is not served and the mock is not hit; without the high/low swap the NO row of `a_traded_period_maps_its_trade_prices_and_an_untraded_one_closes_at_its_previous` reads `high` 0.55; without the filter `a_candle_opening_outside_the_query_is_dropped_and_the_rest_come_oldest_first` returns four candles.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git commit -m "feat(kalshi): Candles on Predictions, with CandleExt and Ohlc (Epic 9, Story 3)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -- polyoxide-kalshi/src/predictions/types/markets.rs polyoxide-kalshi/src/predictions/venue.rs polyoxide-kalshi/tests/venue_predictions.rs
```

### Task 3.5: The three widths live

**Files:**
- Modify: `polyoxide-kalshi/tests/live_predictions_venue.rs:25-28`
- Modify: `docs/specs/kalshi/OBSERVED.md` (only for a finding of this task, directly above the `<!-- epic 9 findings go above this line -->` anchor)

- [ ] **Step 1: Write the live test**

In `polyoxide-kalshi/tests/live_predictions_venue.rs`, replace the import line

```rust
use polyoxide_venue::{Book, EventGroups, Level, MarketData, MarketKey, Quote, SizeUnit, Trades};
```

with:

```rust
use polyoxide_venue::{
    Book, CandleInterval, CandleQuery, Candles, EventGroups, Level, MarketData, MarketKey, Quote,
    SizeUnit, Trades, UnixMillis,
};
```

and append after the file's last test (`live_the_event_group_holds_both_outcomes_of_the_market`):

```rust
/// How far back each served width is read: enough for several candles of it.
const LOOKBACKS: [(CandleInterval, u64); 3] = [
    (CandleInterval::Minute1, 3 * 60 * 60 * 1000),
    (CandleInterval::Hour1, 3 * 24 * 60 * 60 * 1000),
    (CandleInterval::Day1, 14 * 24 * 60 * 60 * 1000),
];

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_candles_at_each_served_width_are_ordered_aligned_and_mirror_each_other() {
    let client = client();
    assert_eq!(
        client.intervals(),
        [
            CandleInterval::Minute1,
            CandleInterval::Hour1,
            CandleInterval::Day1
        ]
    );
    let ticker = common::busiest_market(&client).await;
    let yes = key::key(&ticker, Outcome::Yes);
    let no = key::key(&ticker, Outcome::No);

    for (width, lookback) in LOOKBACKS {
        let step = width.duration().as_secs() * 1000;
        let now = UnixMillis::now();
        let query = CandleQuery::new(width, UnixMillis(now.0 - lookback)).end(now);
        let candles = client.candles(&yes, &query).await.or_fail("YES candles");
        if candles.is_empty() {
            environmental(&format!("{yes}: no {width:?} candle in the lookback"));
        }
        for candle in &candles {
            assert_eq!(candle.key, yes);
            assert_eq!(candle.interval, width);
            assert!(
                candle.open_time >= query.start && candle.open_time <= now,
                "{yes}: a {width:?} candle opens at {:?}, outside the query",
                candle.open_time
            );
            // Minute and hourly candles open on their own boundary, and the daily candle ends at
            // 04:00 UTC, so it opens on an hour.
            let boundary = if width == CandleInterval::Day1 {
                3_600_000
            } else {
                step
            };
            assert_eq!(
                candle.open_time.0 % boundary,
                0,
                "{yes}: a {width:?} candle opens off its boundary"
            );
            assert!(
                candle.close >= Decimal::ZERO && candle.close <= Decimal::ONE,
                "{yes}: a close of {} is not a price in dollars",
                candle.close
            );
            if let (Some(open), Some(high), Some(low)) = (candle.open, candle.high, candle.low) {
                assert!(
                    low <= open && open <= high && low <= candle.close && candle.close <= high,
                    "{yes}: {width:?} candle at {:?} has open {open}, high {high}, low {low}, close {}",
                    candle.open_time,
                    candle.close
                );
            }
        }
        assert!(
            candles
                .windows(2)
                .all(|pair| pair[0].open_time < pair[1].open_time),
            "{yes}: the {width:?} candles are not strictly oldest first"
        );

        // The two outcomes are one market read from both sides: a completed candle's closes sum
        // to one, and one outcome's high is one less the other's low.
        let others = client.candles(&no, &query).await.or_fail("NO candles");
        let mut compared = 0;
        for own in &candles {
            if own.open_time.0 + 2 * step + 60_000 >= now.0 {
                continue;
            }
            let Some(other) = others.iter().find(|c| c.open_time == own.open_time) else {
                continue;
            };
            assert_eq!(
                own.close + other.close,
                Decimal::ONE,
                "{ticker}: {width:?} closes at {:?}",
                own.open_time
            );
            if let (Some(high), Some(low)) = (own.high, other.low) {
                assert_eq!(
                    high + low,
                    Decimal::ONE,
                    "{ticker}: {width:?} high against the other outcome's low at {:?}",
                    own.open_time
                );
            }
            compared += 1;
        }
        if compared == 0 {
            environmental(&format!(
                "{ticker}: no completed {width:?} candle to compare across outcomes"
            ));
        }
    }
}
```

- [ ] **Step 2: Run it to see it fail without the implementation**

The implementation is already in the tree from Task 3.4, so the test cannot be seen failing against a missing `Candles`. To prove it reads the mapping, run it once with the high and low swap removed from a scratch copy of `to_candle` (`Outcome::No => (price.low_dollars, price.high_dollars)` written as `(price.high_dollars, price.low_dollars)`), without committing the change:

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue live_candles -- --ignored
```

Expected with the swap removed: the test fails on `high against the other outcome's low` for a traded candle (or, when no compared candle traded, passes with an `environmental` skip: pick another market night, since a pass proves nothing). Restore the swap before Step 3.

- [ ] **Step 3: Run it for real**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions -- --ignored live_the_route_table_agrees_with_the_endpoint_costs
```

Expected: `test result: ok.` for both (an `environmental` skip prints its reason and counts as a pass for a quiet host). If the second fails because the host lists either candlestick route at a cost other than the crate's default of 10, stop and report it to the owner: the drift test lives in the route table's contract, and a route costing more needs a per-route default, which is a design decision. If either run shows a behaviour the captures did not (a width answered with rows that end off its boundary, a daily candle on another hour), insert it directly above the anchor, under a `###` heading of Story 3's own, with this edit to `docs/specs/kalshi/OBSERVED.md`, replacing

```
<!-- epic 9 findings go above this line -->
```

with

```
### Candlesticks (Story 3)

- <date>, <host>, <route>, <width>: <what the host did>

<!-- epic 9 findings go above this line -->
```

(the bullet written out with the real date, host, route, width and behaviour, one bullet for each finding; the heading is Story 3's own, never another story's, such as Story 7's `### Rate limits`, so the finding never lands under a heading that is not its own, and the anchor line stays unchanged).

- [ ] **Step 4: Commit**

```bash
nix develop --command cargo fmt --all
git commit -m "test(kalshi): live_predictions_venue reads candles at each served width (Epic 9, Story 3)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -- polyoxide-kalshi/tests/live_predictions_venue.rs docs/specs/kalshi/OBSERVED.md
```

(Leave `docs/specs/kalshi/OBSERVED.md` out of the pathspec when nothing was inserted.)

### Task 3.6: The story's gates

- [ ] **Step 1: Run the gates, in order**

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings
nix develop --command cargo test -p polyoxide-kalshi --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh
```

Expected: every command exits 0 and prints nothing to fix. `live_unwraps.py` holds the live test at zero `.unwrap()`s and bare failure sites (the new live test fails only through `or_fail` and `environmental`). The touch-set gate prints no path, since every file this story changed is in Story 1's allowlist. No metadata changed, so `gen_registry.py --check` has no region to rewrite. No test body that existed before this story changed apart from the four route-count tests of Task 3.1, which the owner allowed.

- [ ] **Step 2: Run the story's own Verify line**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored
```

Expected: `test result: ok.` for both. A story with nothing left to commit here ends after this step.

## Story 4: `Protocol::follow_up` in the socket kit (spec D7)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/supervisor/tests.rs`
- **Interfaces:** exposes "Story 4 → Story 5"; uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-ws --all-features --lib supervisor && cargo test -p polyoxide-ws --all-features && python3 scripts/venue_gate.py && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** deep (a foundation crate's supervision loop)

A protocol sometimes learns from a frame it has just decoded that it must ask the server for something at once, and until now it had no way to send on the connection from `decode`. This story gives `Protocol` a provided `follow_up(&mut self) -> Option<Message>` (default `None`) and makes the supervisor task call it once after each `decode` that leaves the connection up, sending what it returns on the same connection, one frame per call, bounded by `Timing::stale_after`, without awaiting an answer. A send that fails, or outlives `stale_after`, ends the connection as a failed ping does (`End::Failed`, then `Protocol::recover` decides). It is not called after a decode error that reconnects or stops, nor on a connection that has already ended (the server's close frame is the last item and leaves `is_finished()` true), so a frame the protocol still holds stays with it and its `reset` clears it. The change is additive (a provided method). The docs and tests name no venue. The story holds none of the profile's landmine markers.

### Task 4.1: `Protocol::follow_up` and its call after `decode`

**Files:**
- Modify: `polyoxide-ws/src/supervisor/mod.rs:216-219` (after `Protocol::ping`) and `polyoxide-ws/src/supervisor/mod.rs:304-305` (the `Supervisor` rules list)
- Modify: `polyoxide-ws/src/supervisor/task.rs:218-238` (a helper beside `ping`) and `polyoxide-ws/src/supervisor/task.rs:535-540` (the call in `pump`)
- Test: `polyoxide-ws/src/supervisor/tests.rs` (append only)

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-ws/src/supervisor/tests.rs`, after its last test (`the_default_connect_reports_a_handshake_that_cannot_be_built`). Nothing above is edited: `Feed`, `feed` and the existing tests keep their bodies, and a new protocol wraps `Feed` the way the file's `Dialer` does.

```rust
use std::collections::VecDeque;

/// A protocol that sends a frame after the ones it decodes, through
/// `follow_up`: for each data event it decodes it holds `reply(n)`, and it
/// returns the held frames one per call. `reset` forgets what is held. The
/// rest is the file's `Feed`.
struct Followed {
    feed: Feed,
    reply: fn(u64) -> Message,
    held: VecDeque<Message>,
}

fn followed(server: &ScriptedServer, reply: fn(u64) -> Message) -> Followed {
    Followed {
        feed: feed(server),
        reply,
        held: VecDeque::new(),
    }
}

/// `{"ack": n}`. The scripted server's answerer reads it as a request that
/// names no topic and answers `{"id": null, "ok": true}`, which `Feed`
/// decodes to nothing.
fn ack(n: u64) -> Message {
    text(json!({"ack": n}))
}

/// The reply to the first event is a frame far larger than a socket's
/// buffers, so a peer that has stopped reading never takes it; the rest are
/// plain acknowledgements.
fn bulky(n: u64) -> Message {
    if n == 1 {
        Message::Text("x".repeat(64 << 20).into())
    } else {
        ack(n)
    }
}

impl Protocol for Followed {
    type Event = Ev;
    type Error = TestError;
    type Membership = Topics;
    type Change = Change;
    type Ack = usize;

    fn handshake(&mut self) -> Result<Request, TestError> {
        self.feed.handshake()
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
        let decoded = self.feed.decode(frame);
        match &decoded {
            Decoded::Event(Ev::Data(n)) => self.held.push_back((self.reply)(*n)),
            Decoded::Events(events) => {
                for event in events {
                    if let Ev::Data(n) = event {
                        self.held.push_back((self.reply)(*n));
                    }
                }
            }
            _ => {}
        }
        decoded
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

    fn follow_up(&mut self) -> Option<Message> {
        self.held.pop_front()
    }

    fn is_retriable(err: &TestError) -> bool {
        <Feed as Protocol>::is_retriable(err)
    }

    fn recover(&self, err: &TestError) -> Recovery {
        self.feed.recover(err)
    }

    fn disconnected(&mut self, outage: Outage<TestError>) -> Option<Ev> {
        self.feed.disconnected(outage)
    }

    fn reconnected(&mut self) -> Ev {
        self.feed.reconnected()
    }

    fn reset(&mut self) {
        self.held.clear();
    }

    fn queue(&self) -> Queue {
        self.feed.queue()
    }

    fn stalled(silent: Duration) -> TestError {
        <Feed as Protocol>::stalled(silent)
    }

    fn stopped() -> TestError {
        <Feed as Protocol>::stopped()
    }
}

async fn connect_followed(
    protocol: Followed,
    timing: Timing,
    membership: Topics,
) -> Supervised<Followed> {
    Supervisor::new(protocol, timing)
        .connect(membership)
        .await
        .expect("connect")
}

/// The acknowledgements the server has received, in arrival order.
fn acks(server: &ScriptedServer) -> Vec<String> {
    server
        .client_frames()
        .into_iter()
        .filter(|frame| frame.contains(r#""ack""#))
        .collect()
}

#[tokio::test]
async fn follow_up_frame_is_sent_after_the_decode_that_prompted_it() {
    let server = server(vec![after_replay(data([1, 2]))]).await;
    let mut ws = connect_followed(followed(&server, ack), fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    server
        .wait_for("both acknowledgements", |s| acks(s).len() == 2)
        .await;
    // One for each event, in the order the events were decoded, on the
    // connection that carried them and after its replay.
    assert_eq!(acks(&server), [r#"{"ack":1}"#, r#"{"ack":2}"#]);
    let frames = server.client_frames();
    assert!(frames[0].contains(r#""op":"sub""#), "{frames:?}");
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn follow_up_none_sends_nothing() {
    // `Feed` keeps the default `follow_up`. The same events arrive as in the
    // test above, and the client's only frame is its replay.
    let server = server(vec![after_replay(data([1, 2]))]).await;
    let mut ws = connect(feed(&server), fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    quiet(&mut ws, Duration::from_millis(150)).await;
    let frames = server.client_frames();
    assert_eq!(frames.len(), 1, "{frames:?}");
    assert!(frames[0].contains(r#""op":"sub""#), "{frames:?}");
    ws.close().await.expect("close");
}

#[tokio::test]
async fn follow_up_sends_one_frame_per_decode_and_leaves_the_rest_to_reset() {
    // One frame carries three events, so three replies are held and one goes
    // out. The server then closes; the protocol's `reset` clears the other
    // two before the next connection, whose own event gets its own reply.
    let burst = vec![text(json!({"burst": [3, 4, 5]}))];
    let server = server(vec![
        after_replay(burst).drop_after(Duration::from_millis(150)),
        after_replay(data([9])),
    ])
    .await;
    let mut ws = connect_followed(followed(&server, ack), fast(), topics(&["a"])).await;
    for n in [3, 4, 5] {
        assert_eq!(next_event(&mut ws).await, Ev::Data(n));
    }
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("closed"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(9));
    server
        .wait_for("the second connection's acknowledgement", |s| {
            acks(s).len() >= 2
        })
        .await;
    quiet(&mut ws, Duration::from_millis(150)).await;
    assert_eq!(acks(&server), [r#"{"ack":3}"#, r#"{"ack":9}"#]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn follow_up_send_that_outlives_the_staleness_window_replaces_the_connection() {
    // The server reads the replay, sends one event and stops reading, so the
    // 64 MiB reply to that event fills the socket's buffers and its send
    // never completes. The supervisor gives up at `stale_after` and treats it
    // as a failed ping: an outage with an error, not a stale one.
    let server = server(vec![
        after_replay(data([1])).go_silent(),
        after_replay(data([2])),
    ])
    .await;
    let mut ws = connect_followed(followed(&server, bulky), fast(), topics(&["a"])).await;
    assert_eq!(next_event(&mut ws).await, Ev::Data(1));
    assert_eq!(next_event(&mut ws).await, Ev::Disconnected("error"));
    assert_eq!(next_event(&mut ws).await, Ev::Reconnected);
    assert_eq!(next_event(&mut ws).await, Ev::Data(2));
    server
        .wait_for("the second connection's acknowledgement", |s| {
            acks(s) == [r#"{"ack":2}"#]
        })
        .await;
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-ws --all-features --lib supervisor::tests::follow_up`

Expected: the build fails with `error[E0407]: method `follow_up` is not a member of trait `Protocol``, pointing at `Followed`'s `fn follow_up`.

- [ ] **Step 3: Implement**

In `polyoxide-ws/src/supervisor/mod.rs`, add the provided method after `ping`. Replace, quoted exactly,

```rust
    /// The ping to send at a ping tick, or `None` for none (D4). It is sent
    /// without awaiting an answer; the answer is a frame like any other.
    fn ping(&mut self, connection: &mut Connection) -> Option<Message>;
```

with

```rust
    /// The ping to send at a ping tick, or `None` for none (D4). It is sent
    /// without awaiting an answer; the answer is a frame like any other.
    fn ping(&mut self, connection: &mut Connection) -> Option<Message>;

    /// A frame to send right after a frame was decoded, or `None` for none (the
    /// default): how a protocol asks the server for something its last frame
    /// showed it needs, such as the state of a stream it can no longer trust.
    ///
    /// The supervisor calls it once after each [`decode`](Self::decode) that
    /// leaves the connection up: not after a decode error that reconnects or
    /// stops, and not on a connection that has already ended, such as after
    /// the server's close frame. What it returns is sent on the same
    /// connection, one frame for each call, within [`Timing::stale_after`],
    /// and the answer is not awaited: it is a frame like any other. A send
    /// that fails, or outlives that window, ends the connection as a failed
    /// [`ping`](Self::ping) does, and [`recover`](Self::recover) decides what
    /// follows.
    ///
    /// A protocol with several frames to send returns one for each call, so
    /// the rest go out after later frames. What it still holds when the
    /// connection ends stays with it, and is cleared in
    /// [`reset`](Self::reset).
    fn follow_up(&mut self) -> Option<Message> {
        None
    }
```

and add the rule to the `Supervisor` docs. Replace, quoted exactly,

```rust
/// - **Staleness.** Silence for [`Timing::stale_after`], by the protocol's
///   liveness rule, replaces the connection.
```

with

```rust
/// - **Staleness.** Silence for [`Timing::stale_after`], by the protocol's
///   liveness rule, replaces the connection.
/// - **Follow-ups.** After each decoded frame that leaves the connection up,
///   the frame [`Protocol::follow_up`] returns is sent within
///   [`Timing::stale_after`]; a send that fails or outlives it replaces the
///   connection, as a failed ping does.
```

In `polyoxide-ws/src/supervisor/task.rs`, add the helper after `ping`. Replace, quoted exactly,

```rust
    match tokio::time::timeout(timing.stale_after, connection.send(ping)).await {
        Ok(sent) => sent.map_err(P::Error::from),
        Err(_) => Err(P::stalled(timing.stale_after)),
    }
}

/// Send a close frame, but not wait on a peer that has stopped reading.
```

with

```rust
    match tokio::time::timeout(timing.stale_after, connection.send(ping)).await {
        Ok(sent) => sent.map_err(P::Error::from),
        Err(_) => Err(P::stalled(timing.stale_after)),
    }
}

/// Send the protocol's follow-up frame, if it has one, within the staleness
/// window.
///
/// A connection that has already ended is not asked: the frame the protocol
/// would return stays with it, and its `reset` clears it. The server's close
/// frame is the last item the connection yields and leaves it finished, so
/// the decode of a close prompts no send on the dead socket to hide it.
async fn follow_up<P: Protocol>(
    protocol: &mut P,
    timing: &Timing,
    connection: &mut Connection,
) -> Result<(), P::Error> {
    if connection.is_finished() {
        return Ok(());
    }
    let Some(frame) = protocol.follow_up() else {
        return Ok(());
    };
    match tokio::time::timeout(timing.stale_after, connection.send(frame)).await {
        Ok(sent) => sent.map_err(P::Error::from),
        Err(_) => Err(P::stalled(timing.stale_after)),
    }
}

/// Send a close frame, but not wait on a peer that has stopped reading.
```

and call it in `pump`, after the decoded frame has been routed. Replace, quoted exactly,

```rust
                            Decoded::Error(err) => match self.protocol.recover(&err) {
                                Recovery::Skip => self.pending.push_back(Err(err)),
                                Recovery::Reconnect | Recovery::Stop => return End::Failed(err),
                            },
                        }
```

with

```rust
                            Decoded::Error(err) => match self.protocol.recover(&err) {
                                Recovery::Skip => self.pending.push_back(Err(err)),
                                Recovery::Reconnect | Recovery::Stop => return End::Failed(err),
                            },
                        }
                        let sent = follow_up(&mut self.protocol, &self.timing, connection).await;
                        if let Err(err) = sent {
                            return End::Failed(err);
                        }
```

A decode error that reconnects or stops has already returned from the arm, so the call is reached only when the connection stays up. The events the decode produced are already in `pending`, so a failed send ends the connection with them still queued, and they are delivered ahead of the outage marker, as after a failed ping.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-ws --all-features --lib supervisor::tests::follow_up`

Expected: `test result: ok. 4 passed; 0 failed`.

Run: `nix develop --command cargo test -p polyoxide-ws --all-features`

Expected: every suite `ok`; the existing supervisor tests are unchanged, since no existing protocol overrides `follow_up`.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-ws --all-targets --all-features -- -D warnings && bash -c 'RUSTDOCFLAGS="-D warnings" nix develop --command cargo doc --no-deps -p polyoxide-ws --all-features'`

Expected: fmt prints nothing; clippy and doc end `Finished` with no warnings.

Run: `python3 scripts/venue_gate.py && python3 scripts/test_body_diff.py --baseline v0.40.0`

Expected: both pass: the docs and tests say no venue's words, and no existing test item changed (the gate sees only new items in `tests.rs`).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(ws): Protocol::follow_up sends a frame after a decode (Epic 9, Story 4)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_01FijMBSPGBERJ4VNP5AgG6U" -- polyoxide-ws/src/supervisor/mod.rs polyoxide-ws/src/supervisor/task.rs polyoxide-ws/src/supervisor/tests.rs
```

## Story 5: The socket protocol: the `ticker` and `trade` channels, `get_snapshot` and the gap re-snapshot (spec D8, D9, §7)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/event.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `docs/specs/kalshi/OBSERVED.md`
- **Interfaces:** exposes "Story 5 → Stories 6, 8 and 9"; uses Story 1's ws fixtures and anchor and Story 4's `Protocol::follow_up`
- **Verify:** `cargo test -p polyoxide-kalshi --all-features && cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws -- --ignored && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** deep (sequence recovery and the empty-subscription rule)

Gives Kalshi's socket protocol its `ticker` and `trade` channels and the in-place repair of a gap. `Channel` and `PredictionsWsBuilder::channels` choose the channels (the default stays the order book); the replay sends one `subscribe` naming them all, the protocol keeps a `sid` for each from the `subscribed` answers (the correlation closure waits for one answer per channel and records each `sid` even when it reads it during a later request), a change goes out as one `update_subscription` (or `unsubscribe`) per `sid` in channel order, and an empty set subscribes nothing on any channel. `TickerUpdate` and `PublicTrade` are decoded from frames the protocol takes only on a channel that was asked for; a `trade` gap, like any gap off the order book, reconnects. A gap on the order-book `sid` yields `Event::Resyncing { tickers }` ahead of the frame that raised it, queues one `get_snapshot` for the whole membership through Story 4's `follow_up`, drops that `sid`'s deltas per ticker until each snapshot answering it arrives, re-anchors the `sid`'s numbering on each answering snapshot, and reconnects on `SnapshotTimeout`, on an error answering the command, or on a second gap while the first is being repaired. `PredictionsMembership::snapshot` asks for snapshots of subscribed tickers by the same command. The code uses none of the profile's landmine markers. Every edit to `ws/protocol.rs`, `event.rs`, `error.rs` and `builder.rs` in this epic is this story's. Task 5.2 extends the scripted answer first so the protocol can then be driven; the story depends on Story 4 having landed `Protocol::follow_up` in `polyoxide-ws`.

### Task 5.1: The new types: `Channel`, the events, the frames and the two errors

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/ws/event.rs:5-12`, `:24-25`, `:73`, `:151`, `:160-163`, `:363-369`, `:454-459`
- Modify: `polyoxide-kalshi/src/predictions/ws/error.rs:60-73`, `:75-115`, `:324-328`
- Modify: `polyoxide-kalshi/src/predictions/ws/protocol.rs:33-37`, `:1339-1344`
- Modify: `polyoxide-kalshi/src/predictions/ws/mod.rs:23-37`
- Test: the same files' `mod tests`

- [ ] **Step 1: Write the failing tests**

Append to `event.rs`'s `mod tests`, after `an_unknown_side_does_not_decode` and before the module's closing brace:

```rust
    // The mirror's own examples for the two channels (`docs/specs/kalshi/asyncapi.yaml`,
    // version 2.0.0), as the `msg` of a frame.
    const TICKER_MSG: &str = r#"{"market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","market_ticker":"FED-23DEC-T3.00","price_dollars":"0.4800","yes_bid_dollars":"0.4500","yes_ask_dollars":"0.5300","volume_fp":"33896.00","open_interest_fp":"20422.00","dollar_volume":16948,"dollar_open_interest":10211,"yes_bid_size_fp":"300.00","yes_ask_size_fp":"150.00","last_trade_size_fp":"25.00","ts":1669149841,"ts_ms":1669149841000,"time":"2022-11-22T20:44:01Z"}"#;
    const TRADE_MSG: &str = r#"{"trade_id":"d91bc706-ee49-470d-82d8-11418bda6fed","market_ticker":"HIGHNY-22DEC23-B53.5","yes_price_dollars":"0.3600","no_price_dollars":"0.6400","count_fp":"136.00","taker_side":"no","taker_outcome_side":"no","taker_book_side":"ask","is_block_trade":false,"ts":1669149841,"ts_ms":1669149841000}"#;

    fn decimal(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn ticker_update(msg: &str) -> TickerUpdate {
        TickerUpdate::from(serde_json::from_str::<frames::TickerMsg>(msg).unwrap())
    }

    fn public_trade(msg: &str) -> PublicTrade {
        PublicTrade::from(serde_json::from_str::<frames::TradeMsg>(msg).unwrap())
    }

    #[test]
    fn a_ticker_update_carries_the_mirrors_values() {
        let update = ticker_update(TICKER_MSG);
        assert_eq!(update.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(update.market_id, "9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1");
        assert_eq!(update.price, decimal("0.4800"));
        assert_eq!(update.yes_bid, decimal("0.4500"));
        assert_eq!(update.yes_ask, decimal("0.5300"));
        assert_eq!(update.yes_bid_size, decimal("300.00"));
        assert_eq!(update.yes_ask_size, decimal("150.00"));
        assert_eq!(update.volume, decimal("33896.00"));
        assert_eq!(update.open_interest, decimal("20422.00"));
        assert_eq!(update.last_trade_size, decimal("25.00"));
        assert_eq!(update.dollar_volume, 16_948);
        assert_eq!(update.dollar_open_interest, 10_211);
        // `ts_ms`, not the deprecated `ts` in seconds.
        assert_eq!(update.ts, UnixMillis(1_669_149_841_000));
    }

    #[test]
    fn the_ticker_type_writes_back_every_key_the_mirrors_example_carries() {
        // Bug: a key the type drops, which the wire-agreement suite would then call
        // unmodelled only once a capture sends it.
        let wire: serde_json::Value = serde_json::from_str(TICKER_MSG).unwrap();
        let msg = serde_json::from_str::<frames::TickerMsg>(TICKER_MSG).unwrap();
        assert_eq!(serde_json::to_value(msg).unwrap(), wire);
    }

    #[test]
    fn a_ticker_missing_a_key_the_mirror_requires_does_not_decode() {
        let without_time = TICKER_MSG.replace(r#""ts_ms":1669149841000,"#, "");
        assert_ne!(without_time, TICKER_MSG, "the replacement found its key");
        assert!(serde_json::from_str::<frames::TickerMsg>(&without_time).is_err());
    }

    #[test]
    fn a_trade_carries_the_mirrors_values_and_its_direction_in_both_vocabularies() {
        let trade = public_trade(TRADE_MSG);
        assert_eq!(trade.trade_id, "d91bc706-ee49-470d-82d8-11418bda6fed");
        assert_eq!(trade.market_ticker, "HIGHNY-22DEC23-B53.5");
        assert_eq!(trade.yes_price, decimal("0.3600"));
        assert_eq!(trade.no_price, decimal("0.6400"));
        assert_eq!(trade.count, decimal("136.00"));
        // The taker is positioned for NO, which in the book's vocabulary is an ask.
        assert_eq!(trade.taker_outcome_side, OutcomeSide::No);
        assert_eq!(trade.taker_book_side, BookSide::Ask);
        assert!(!trade.is_block_trade);
        assert_eq!(trade.ts, UnixMillis(1_669_149_841_000));
    }

    #[test]
    fn the_trade_type_writes_back_every_key_the_mirrors_example_carries() {
        let wire: serde_json::Value = serde_json::from_str(TRADE_MSG).unwrap();
        let msg = serde_json::from_str::<frames::TradeMsg>(TRADE_MSG).unwrap();
        assert_eq!(serde_json::to_value(msg).unwrap(), wire);
    }

    #[test]
    fn a_trade_side_this_client_does_not_know_keeps_its_spelling() {
        // Bug: refusing a whole trade because Kalshi added a spelling.
        let odd = TRADE_MSG.replace(
            r#""taker_outcome_side":"no""#,
            r#""taker_outcome_side":"maybe""#,
        );
        assert_ne!(odd, TRADE_MSG, "the replacement found its key");
        assert_eq!(
            public_trade(&odd).taker_outcome_side,
            OutcomeSide::Other("maybe".to_owned())
        );
    }
```

Append to `error.rs`'s `mod tests`, after `the_transport_is_the_kits_table` and before the module's closing brace:

```rust
    #[test]
    fn a_missing_snapshot_is_an_unavailable_fault_the_socket_retries() {
        let err = PredictionsWsError::SnapshotTimeout {
            sid: 1,
            waited: Duration::from_secs(30),
        };
        assert_eq!(err.class(), Class::Unavailable { code: None });
        assert!(err.is_retriable() && err.is_fault());
        assert_eq!(err.retry_after(), None);
        assert_eq!(
            err.to_string(),
            "the snapshots of subscription 1 did not arrive within 30s"
        );
    }

    #[test]
    fn a_request_the_socket_refuses_is_an_invalid_request_and_is_not_retried() {
        let err = PredictionsWsError::InvalidRequest {
            reason: "the socket needs at least one channel".to_owned(),
        };
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(!err.is_retriable());
        assert_eq!(err.retry_after(), None);
        assert_eq!(
            err.to_string(),
            "invalid request: the socket needs at least one channel"
        );
    }
```

Append to `protocol.rs`'s `mod tests`, after `a_url_that_is_not_a_request_is_refused_before_signing` and before the module's closing brace:

```rust
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
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib predictions::ws`

Expected: the build fails with `cannot find type `TickerUpdate` in this scope`, `cannot find type `PublicTrade``, `no variant named `SnapshotTimeout` found for enum `PredictionsWsError``, `no variant named `InvalidRequest`` and `cannot find type `Channel` in this scope`.

- [ ] **Step 3: Implement**

`event.rs`. Replace the imports (lines 9-12):

```rust
use crate::predictions::{
    types::{BookSide, PriceLevel},
    ws::PredictionsWsError,
};
```

with:

```rust
use crate::predictions::{
    types::{BookSide, OutcomeSide, PriceLevel},
    ws::PredictionsWsError,
};
```

Replace these two lines of `enum Event` (lines 24-25):

```rust
    /// One change to one price level of a book.
    Delta(OrderbookDelta),
```

with:

```rust
    /// One change to one price level of a book.
    Delta(OrderbookDelta),
    /// A market's price, best bid and ask, volume and open interest, on the `ticker`
    /// channel. Only a socket built with that channel yields it.
    Ticker(TickerUpdate),
    /// One public trade, on the `trade` channel. Only a socket built with that channel
    /// yields it.
    Trade(PublicTrade),
    /// The order book's sequence skipped a number, so the books of these markets cannot be
    /// trusted. The socket asks Kalshi for a fresh snapshot of each on the same connection:
    /// a consumer drops their books, and the snapshots that follow rebuild them. Deltas of a
    /// market are withheld until its snapshot arrives. No `Disconnected` or `Reconnected`
    /// goes with it, and the stream reconnects only if the snapshots do not come.
    Resyncing {
        /// Every market of the membership: all of their books are suspect.
        tickers: Vec<String>,
    },
```

Insert before the line `/// Why a connection went down, in a `Disconnected` event: the kit's `Outage`.` (line 73):

```rust
/// A market's last price, best bid and ask, volume and open interest.
///
/// Prices are in dollars, sizes in contracts.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TickerUpdate {
    /// The market's ticker.
    pub market_ticker: String,
    /// The market's id.
    pub market_id: String,
    /// The last traded price.
    pub price: Decimal,
    /// The best YES bid.
    pub yes_bid: Decimal,
    /// The best YES ask.
    pub yes_ask: Decimal,
    /// Contracts resting at the best YES bid.
    pub yes_bid_size: Decimal,
    /// Contracts resting at the best YES ask.
    pub yes_ask_size: Decimal,
    /// Contracts traded in all.
    pub volume: Decimal,
    /// Contracts held open.
    pub open_interest: Decimal,
    /// The cumulative traded notional, in whole dollars.
    pub dollar_volume: i64,
    /// The open-interest notional, in whole dollars.
    pub dollar_open_interest: i64,
    /// Contracts in the last trade.
    pub last_trade_size: Decimal,
    /// When the update happened.
    pub ts: UnixMillis,
}

/// One public trade.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PublicTrade {
    /// Kalshi's id for the trade.
    pub trade_id: String,
    /// The market's ticker.
    pub market_ticker: String,
    /// The YES side's price.
    pub yes_price: Decimal,
    /// The NO side's price.
    pub no_price: Decimal,
    /// Contracts traded.
    pub count: Decimal,
    /// The outcome the taker is positioned for: buying YES and selling NO are `yes`, buying
    /// NO and selling YES are `no`. It does not change the price.
    pub taker_outcome_side: OutcomeSide,
    /// The same direction in the book's vocabulary: `bid` for `yes`, `ask` for `no`.
    pub taker_book_side: BookSide,
    /// Whether the trade was matched off the book as a block trade.
    pub is_block_trade: bool,
    /// When the trade happened.
    pub ts: UnixMillis,
}

```

Insert before the line `/// The frames Kalshi sends, as the wire writes them, for the protocol's decode and for` (line 151):

```rust
impl From<frames::TickerMsg> for TickerUpdate {
    fn from(msg: frames::TickerMsg) -> Self {
        Self {
            market_ticker: msg.market_ticker,
            market_id: msg.market_id,
            price: msg.price_dollars,
            yes_bid: msg.yes_bid_dollars,
            yes_ask: msg.yes_ask_dollars,
            yes_bid_size: msg.yes_bid_size_fp,
            yes_ask_size: msg.yes_ask_size_fp,
            volume: msg.volume_fp,
            open_interest: msg.open_interest_fp,
            dollar_volume: msg.dollar_volume,
            dollar_open_interest: msg.dollar_open_interest,
            last_trade_size: msg.last_trade_size_fp,
            ts: UnixMillis(msg.ts_ms),
        }
    }
}

impl From<frames::TradeMsg> for PublicTrade {
    fn from(msg: frames::TradeMsg) -> Self {
        Self {
            trade_id: msg.trade_id,
            market_ticker: msg.market_ticker,
            yes_price: msg.yes_price_dollars,
            no_price: msg.no_price_dollars,
            count: msg.count_fp,
            taker_outcome_side: msg.taker_outcome_side,
            taker_book_side: msg.taker_book_side,
            is_block_trade: msg.is_block_trade,
            ts: UnixMillis(msg.ts_ms),
        }
    }
}

```

In `mod frames`, replace the import (line 163):

```rust
    use crate::predictions::types::PriceLevel;
```

with:

```rust
    use crate::predictions::types::{BookSide, OutcomeSide, PriceLevel};
```

and replace the end of `DeltaMsg` and of the module (lines 363-367):

```rust
        /// The subaccount of the account's own order that caused the change.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subaccount: Option<u64>,
    }
}
```

with:

```rust
        /// The subaccount of the account's own order that caused the change.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subaccount: Option<u64>,
    }

    /// `ticker`: a market's price, volume and open interest. The channel carries no `seq`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TickerFrame {
        /// Always `ticker`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The subscription.
        pub sid: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The update.
        pub msg: TickerMsg,
    }

    /// The body of `ticker`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TickerMsg {
        /// The market's id.
        pub market_id: String,
        /// The market's ticker.
        pub market_ticker: String,
        /// The last traded price.
        #[serde(with = "rust_decimal::serde::str")]
        pub price_dollars: Decimal,
        /// The best YES bid.
        #[serde(with = "rust_decimal::serde::str")]
        pub yes_bid_dollars: Decimal,
        /// The best YES ask.
        #[serde(with = "rust_decimal::serde::str")]
        pub yes_ask_dollars: Decimal,
        /// Contracts traded in all.
        #[serde(with = "rust_decimal::serde::str")]
        pub volume_fp: Decimal,
        /// Contracts held open.
        #[serde(with = "rust_decimal::serde::str")]
        pub open_interest_fp: Decimal,
        /// The cumulative traded notional, in whole dollars.
        pub dollar_volume: i64,
        /// The open-interest notional, in whole dollars.
        pub dollar_open_interest: i64,
        /// Contracts resting at the best YES bid.
        #[serde(with = "rust_decimal::serde::str")]
        pub yes_bid_size_fp: Decimal,
        /// Contracts resting at the best YES ask.
        #[serde(with = "rust_decimal::serde::str")]
        pub yes_ask_size_fp: Decimal,
        /// Contracts in the last trade.
        #[serde(with = "rust_decimal::serde::str")]
        pub last_trade_size_fp: Decimal,
        /// When the update happened, in milliseconds.
        pub ts_ms: u64,
        /// Deprecated: the time in seconds. `ts_ms` replaces it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ts: Option<i64>,
        /// Deprecated: the time as RFC 3339. `ts_ms` replaces it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub time: Option<String>,
    }

    /// `trade`: one public trade.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TradeFrame {
        /// Always `trade`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The subscription.
        pub sid: u64,
        /// Its sequence number.
        pub seq: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The trade.
        pub msg: TradeMsg,
    }

    /// The body of `trade`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TradeMsg {
        /// Kalshi's id for the trade.
        pub trade_id: String,
        /// The market's ticker.
        pub market_ticker: String,
        /// The YES side's price.
        #[serde(with = "rust_decimal::serde::str")]
        pub yes_price_dollars: Decimal,
        /// The NO side's price.
        #[serde(with = "rust_decimal::serde::str")]
        pub no_price_dollars: Decimal,
        /// Contracts traded.
        #[serde(with = "rust_decimal::serde::str")]
        pub count_fp: Decimal,
        /// The outcome the taker is positioned for.
        pub taker_outcome_side: OutcomeSide,
        /// The same direction in the book's vocabulary.
        pub taker_book_side: BookSide,
        /// Whether the trade was a block trade.
        pub is_block_trade: bool,
        /// When the trade happened, in milliseconds.
        pub ts_ms: u64,
        /// Deprecated: the taker's side. `taker_outcome_side` replaces it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub taker_side: Option<String>,
        /// Deprecated: the time in seconds. `ts_ms` replaces it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ts: Option<i64>,
    }
}
```

`error.rs`. Insert two variants before `/// The handshake could not be signed.` (line 70), after `InvalidUrl`:

```rust
    /// Kalshi did not send the snapshots a gap in the order book's sequence asked for within
    /// the staleness window. The socket reconnects, and the replay brings fresh snapshots.
    #[error("the snapshots of subscription {sid} did not arrive within {waited:?}")]
    SnapshotTimeout {
        /// The order-book subscription that gapped.
        sid: u64,
        /// How long the socket waited.
        waited: Duration,
    },

    /// The socket refused a request before sending it: an empty set of channels at
    /// `connect`, or a `snapshot` of a market that is not subscribed or on a socket without
    /// the order-book channel.
    #[error("invalid request: {reason}")]
    InvalidRequest {
        /// Why it was refused.
        reason: String,
    },

```

In `impl Classify for PredictionsWsError`, replace the arm (line 100):

```rust
            Self::InvalidUrl { .. } => Class::InvalidRequest,
```

with:

```rust
            Self::InvalidUrl { .. } | Self::InvalidRequest { .. } => Class::InvalidRequest,
            Self::SnapshotTimeout { .. } => Class::Unavailable { code: None },
```

and replace the end of the impl's doc comment (lines 82-85):

```rust
/// replay resubscribes. A gap in the sequence is a lost message, `Network` and a fault; a
/// frame that did not decode is `Decode`; a URL or a key that cannot be used is
/// `InvalidRequest`.
```

with:

```rust
/// replay resubscribes. A gap in the sequence is a lost message, `Network` and a fault; a
/// snapshot that never came is `Unavailable`; a frame that did not decode is `Decode`; a URL
/// or a key that cannot be used, and a request the socket refuses before sending it, are
/// `InvalidRequest`.
```

`protocol.rs`. Insert directly after the two lines (35-36):

```rust
/// The one channel this socket carries.
const CHANNEL: &str = "orderbook_delta";
```

the channel enum:

```rust

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
```

`mod.rs`. Replace the module block (lines 23-37):

```rust
mod builder;
mod error;
mod event;
mod protocol;
#[cfg(test)]
mod scripted;

pub use builder::PredictionsWsBuilder;
pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot};
#[doc(hidden)]
pub use protocol::signed_request;
pub use protocol::{Change, PredictionsMembership, PredictionsProtocol};
```

with:

```rust
mod builder;
mod error;
mod event;
mod protocol;
#[cfg(test)]
mod scripted;

pub use builder::PredictionsWsBuilder;
pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{
    DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot, PublicTrade, TickerUpdate,
};
#[doc(hidden)]
pub use protocol::signed_request;
pub use protocol::{Change, Channel, PredictionsMembership, PredictionsProtocol};
```

- [ ] **Step 4: Run it to see it pass**

Run `nix develop --command cargo fmt --all` first and keep what it does to this task's files (the snippets above are written in rustfmt's form, but it may wrap a long line another way).

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib predictions::ws`

Expected: `test result: ok.` with the new tests among the passes and no failures. Then `nix develop --command cargo fmt --all -- --check` and `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings` print nothing.

- [ ] **Step 5: Commit**

No new files.

```bash
git commit -m "feat(kalshi): the channel enum, the ticker and trade types and the two new socket errors (Epic 9, Story 5)" -- polyoxide-kalshi/src/predictions/ws/event.rs polyoxide-kalshi/src/predictions/ws/error.rs polyoxide-kalshi/src/predictions/ws/protocol.rs polyoxide-kalshi/src/predictions/ws/mod.rs
```

### Task 5.2: The scripted answer learns channels and `get_snapshot`

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/ws/scripted.rs:69-71`, `:257-258`, `:268-453`, `:621`
- Test: `polyoxide-kalshi/src/predictions/ws/scripted.rs`

The answer is Kalshi's as the AsyncAPI describes it: a `subscribe` naming several channels gets one `subscribed` frame per channel (each with the command's `id` and its own `sid`) and then the order-book channel's snapshots; the `ticker` channel's frames carry no `seq`; a `get_snapshot` is answered with `orderbook_snapshot` frames that carry the command's `id` and take the subscription's next numbers. It names nothing of the crate's own, because the integration suite includes it by `#[path]`. Nothing here changes what a default script does, so the existing suites stay green.

- [ ] **Step 1: Write the failing tests**

Append to `scripted.rs`'s `mod tests`, after `a_subscription_error_carries_the_code_it_is_given` and before the module's closing brace:

```rust
    fn subscribe_channels(tickers: &[&str], channels: &[&str]) -> Value {
        json!({"id": 1, "cmd": "subscribe", "params": {
            "channels": channels, "market_tickers": tickers, "use_yes_price": true,
        }})
    }

    #[test]
    fn a_subscribe_on_three_channels_gets_a_subscribed_each_then_the_books_snapshots() {
        let mut state = State::default();
        let frames = ask(
            &mut state,
            &Script::default(),
            subscribe_channels(&["A-1"], &["orderbook_delta", "ticker", "trade"]),
        );
        assert_eq!(frames.len(), 4);
        for (n, channel) in ["orderbook_delta", "ticker", "trade"].iter().enumerate() {
            assert_eq!(frames[n]["type"], "subscribed");
            assert_eq!(frames[n]["id"], 1);
            assert_eq!(frames[n]["msg"]["channel"], *channel);
            assert_eq!(frames[n]["msg"]["sid"], n as u64 + 1);
        }
        // Only the order book has a snapshot, and it counts on the book's subscription.
        assert_eq!(frames[3]["type"], "orderbook_snapshot");
        assert_eq!(
            (frames[3]["sid"].as_u64(), frames[3]["seq"].as_u64()),
            (Some(1), Some(1))
        );
    }

    #[test]
    fn a_subscribe_with_no_channels_is_the_order_book() {
        let mut state = State::default();
        let request = json!({"id": 1, "cmd": "subscribe", "params": {"market_tickers": ["A-1"]}});
        let frames = ask(&mut state, &Script::default(), request);
        assert_eq!(frames[0]["msg"]["channel"], "orderbook_delta");
        assert_eq!(frames[1]["type"], "orderbook_snapshot");
    }

    #[test]
    fn the_ticker_channel_numbers_nothing_and_the_trade_channel_does() {
        let mut state = State::default();
        ask(
            &mut state,
            &Script::default(),
            subscribe_channels(&["A-1"], &["ticker", "trade"]),
        );
        let add = |sid: u64| {
            json!({"id": 2, "cmd": "update_subscription", "params": {
                "sid": sid, "action": "add_markets", "market_tickers": ["B-2"],
            }})
        };
        let ticker = ask(&mut state, &Script::default(), add(1));
        assert_eq!(ticker.len(), 1, "no snapshot on a channel without books");
        assert!(ticker[0].get("seq").is_none(), "{:?}", ticker[0]);
        let trade = ask(&mut state, &Script::default(), add(2));
        assert_eq!(trade[0]["seq"], 1);
        let end = json!({"id": 3, "cmd": "unsubscribe", "params": {"sids": [1]}});
        assert!(ask(&mut state, &Script::default(), end)[0]
            .get("seq")
            .is_none());
    }

    fn get_snapshot_request(id: u64, tickers: &[&str]) -> Value {
        json!({"id": id, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "get_snapshot", "market_tickers": tickers,
        }})
    }

    #[test]
    fn a_get_snapshot_is_answered_with_snapshots_that_carry_its_id_and_the_next_numbers() {
        let mut state = State::default();
        ask(
            &mut state,
            &Script::default(),
            subscribe_request(&["A-1", "B-2"]),
        );
        let frames = ask(
            &mut state,
            &Script::default(),
            get_snapshot_request(77, &["B-2", "A-1"]),
        );
        // No `ok`: the snapshots are the answer, in the order asked.
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0]["type"], "orderbook_snapshot");
        assert_eq!(frames[0]["id"], 77);
        assert_eq!(frames[0]["msg"]["market_ticker"], "B-2");
        assert_eq!(
            (frames[0]["sid"].as_u64(), frames[0]["seq"].as_u64()),
            (Some(1), Some(3))
        );
        assert_eq!(frames[1]["msg"]["market_ticker"], "A-1");
        assert_eq!(frames[1]["seq"], 4);
        // The subscription is unchanged.
        let again = ask(
            &mut state,
            &Script::default(),
            get_snapshot_request(78, &["A-1"]),
        );
        assert_eq!(again[0]["seq"], 5);
    }

    #[test]
    fn the_snapshot_knobs_renumber_limit_refuse_and_follow_the_answer() {
        let mut state = State::default();
        ask(
            &mut state,
            &Script::default(),
            subscribe_request(&["A-1", "B-2"]),
        );
        let numbered = Script {
            snapshot_seq: Some(10),
            snapshot_limit: Some(1),
            after_snapshot: vec![delta(1, 11, "A-1").to_string()],
            ..Script::default()
        };
        let frames = ask(
            &mut state,
            &numbered,
            get_snapshot_request(5, &["A-1", "B-2"]),
        );
        assert_eq!(frames.len(), 2, "one snapshot, then the frame that follows");
        assert_eq!(frames[0]["seq"], 10);
        assert_eq!(frames[1]["type"], "orderbook_delta");
        assert_eq!(frames[1]["seq"], 11);
        let silent = Script {
            snapshot_limit: Some(0),
            ..Script::default()
        };
        assert!(ask(&mut state, &silent, get_snapshot_request(6, &["A-1"])).is_empty());
        let refused = Script {
            snapshot_error: Some((11, "Invalid parameter".to_owned())),
            ..Script::default()
        };
        let frames = ask(&mut state, &refused, get_snapshot_request(7, &["A-1"]));
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["type"], "error");
        assert_eq!(frames[0]["id"], 7);
        assert_eq!(frames[0]["msg"]["code"], 11);
    }

    #[test]
    fn the_ticker_and_trade_frames_are_the_mirrors_examples() {
        let update = ticker_frame(11, "FED-23DEC-T3.00");
        assert_eq!(update["type"], "ticker");
        assert!(update.get("seq").is_none());
        assert_eq!(update["msg"]["price_dollars"], "0.4800");
        let public = trade_frame(11, 2, "HIGHNY-22DEC23-B53.5");
        assert_eq!(public["type"], "trade");
        assert_eq!((public["sid"].as_u64(), public["seq"].as_u64()), (Some(11), Some(2)));
        assert_eq!(public["msg"]["taker_book_side"], "ask");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib predictions::ws::scripted`

Expected: the build fails with `no field `snapshot_seq` on type `Script``, `cannot find function `ticker_frame` in this scope` and the like.

- [ ] **Step 3: Implement**

Add to `Script`, replacing (lines 69-71):

```rust
    /// Send a delta ahead of that `ok`.
    pub delta_before_ok: bool,
}
```

with:

```rust
    /// Send a delta ahead of that `ok`.
    pub delta_before_ok: bool,
    /// The `seq` the first snapshot answering a `get_snapshot` takes, instead of the next
    /// number; the ones after it count on from there.
    pub snapshot_seq: Option<u64>,
    /// Answer a `get_snapshot` with at most this many snapshots, from the first market it
    /// names: `Some(0)` leaves it unanswered.
    pub snapshot_limit: Option<usize>,
    /// Answer every `get_snapshot` with an `error` of this code and message, carrying the
    /// command's `id`.
    pub snapshot_error: Option<(u32, String)>,
    /// Frames sent right after the snapshots that answer a `get_snapshot`. Their sequence
    /// numbers are the test's own.
    pub after_snapshot: Vec<String>,
}
```

Insert the two frame builders before `fn error(id: &Value, code: u32, message: &str) -> Value {` (line 259):

```rust
/// A `ticker` frame with the mirror's example values. The channel carries no `seq`.
pub fn ticker_frame(sid: u64, market: &str) -> Value {
    json!({
        "type": "ticker",
        "sending_ts_ms": SENDING_TS_MS,
        "sid": sid,
        "msg": {
            "market_id": MARKET_ID,
            "market_ticker": market,
            "price_dollars": "0.4800",
            "yes_bid_dollars": "0.4500",
            "yes_ask_dollars": "0.5300",
            "volume_fp": "33896.00",
            "open_interest_fp": "20422.00",
            "dollar_volume": 16_948,
            "dollar_open_interest": 10_211,
            "yes_bid_size_fp": "300.00",
            "yes_ask_size_fp": "150.00",
            "last_trade_size_fp": "25.00",
            "ts": 1_669_149_841_u64,
            "ts_ms": 1_669_149_841_000_u64,
            "time": "2022-11-22T20:44:01Z",
        },
    })
}

/// A `trade` frame with the mirror's example values: a taker positioned for NO.
pub fn trade_frame(sid: u64, seq: u64, market: &str) -> Value {
    json!({
        "type": "trade",
        "sending_ts_ms": SENDING_TS_MS,
        "sid": sid,
        "seq": seq,
        "msg": {
            "trade_id": "d91bc706-ee49-470d-82d8-11418bda6fed",
            "market_ticker": market,
            "yes_price_dollars": "0.3600",
            "no_price_dollars": "0.6400",
            "count_fp": "136.00",
            "taker_side": "no",
            "taker_outcome_side": "no",
            "taker_book_side": "ask",
            "is_block_trade": false,
            "ts": 1_669_149_841_u64,
            "ts_ms": 1_669_149_841_000_u64,
        },
    })
}

```

Replace everything from the line `/// What one connection has subscribed.` (line 268) up to, but not including, `#[cfg(test)]` (line 455) with:

```rust
/// The channel whose subscriptions carry books.
const BOOK: &str = "orderbook_delta";

/// What one connection has subscribed.
#[derive(Default)]
struct State {
    /// The last subscription id issued on this connection.
    issued: u64,
    subscriptions: HashMap<u64, Subscription>,
}

#[derive(Default)]
struct Subscription {
    /// The channel it was subscribed on.
    channel: String,
    /// The last sequence number issued.
    seq: u64,
    tickers: Vec<String>,
}

impl State {
    /// The next sequence number of subscription `sid`.
    fn next_seq(&mut self, sid: u64) -> u64 {
        let subscription = self.subscriptions.entry(sid).or_default();
        subscription.seq += 1;
        subscription.seq
    }

    /// Whether subscription `sid` is on the order-book channel.
    fn is_book(&self, sid: u64) -> bool {
        self.subscriptions
            .get(&sid)
            .is_some_and(|subscription| subscription.channel == BOOK)
    }

    /// Whether the frames of subscription `sid` carry a `seq`: the `ticker` channel's do not.
    fn numbered(&self, sid: u64) -> bool {
        self.subscriptions
            .get(&sid)
            .is_some_and(|subscription| subscription.channel != "ticker")
    }
}

/// One connection's answerer: Kalshi's answer to `subscribe`, `update_subscription` and
/// `unsubscribe`, given the subscriptions the connection carries, with the script's
/// refusals and injected faults.
fn answer(script: Script) -> impl FnMut(&str) -> Vec<Message> + Send {
    let mut state = State::default();
    move |text| {
        let Ok(request) = serde_json::from_str::<Value>(text) else {
            return Vec::new();
        };
        reply(&mut state, &script, &request)
            .into_iter()
            .map(|frame| Message::Text(frame.to_string().into()))
            .collect()
    }
}

/// The frames Kalshi sends for one request, in order.
fn reply(state: &mut State, script: &Script, request: &Value) -> Vec<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = &request["params"];
    let tickers: Vec<String> = params["market_tickers"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if let Some((refused, code)) = &script.refuse_ticker {
        if tickers.contains(refused) {
            return vec![error(&id, *code, "Invalid parameter")];
        }
    }
    match request["cmd"].as_str() {
        Some("subscribe") => subscribe(state, script, &id, params, &tickers),
        Some("update_subscription") if script.ignore_updates => Vec::new(),
        Some("update_subscription") => update(state, script, &id, params, &tickers),
        Some("unsubscribe") => unsubscribe(state, &id, params),
        _ => vec![error(&id, 5, "Unknown command")],
    }
}

/// The channels a `subscribe` names; the order book when it names none.
fn channels_of(params: &Value) -> Vec<String> {
    let named: Vec<String> = params["channels"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if named.is_empty() {
        vec![BOOK.to_owned()]
    } else {
        named
    }
}

/// A `subscribed` for each channel named, each with its own `sid`, then the order-book
/// channel's snapshot of each market, counting from 1.
fn subscribe(
    state: &mut State,
    script: &Script,
    id: &Value,
    params: &Value,
    tickers: &[String],
) -> Vec<Value> {
    let channels = channels_of(params);
    if script.undecodable_subscribed {
        return vec![json!({
            "id": id,
            "type": "subscribed",
            "sending_ts_ms": SENDING_TS_MS,
            "msg": {"channel": channels[0]},
        })];
    }
    let mut frames = Vec::new();
    let mut first = None;
    let mut book = None;
    for channel in &channels {
        state.issued += 1;
        let sid = state.issued;
        state.subscriptions.insert(
            sid,
            Subscription {
                channel: channel.clone(),
                seq: 0,
                tickers: tickers.to_vec(),
            },
        );
        frames.push(json!({
            "id": id,
            "type": "subscribed",
            "sending_ts_ms": SENDING_TS_MS,
            "msg": {"channel": channel, "sid": sid},
        }));
        if first.is_none() {
            first = Some(sid);
        }
        if book.is_none() && channel == BOOK {
            book = Some(sid);
        }
    }
    if let Some(sid) = book {
        for ticker in tickers {
            let seq = state.next_seq(sid);
            frames.push(snapshot(sid, seq, ticker));
        }
    }
    if let (Some((code, message)), Some(sid)) = (&script.error_after_subscribe, book.or(first)) {
        let seq = state.next_seq(sid);
        frames.push(json!({
            "type": "error",
            "sid": sid,
            "seq": seq,
            "sending_ts_ms": SENDING_TS_MS,
            "msg": {"code": code, "msg": message},
        }));
    }
    frames
}

fn update(
    state: &mut State,
    script: &Script,
    id: &Value,
    params: &Value,
    tickers: &[String],
) -> Vec<Value> {
    let Some(sid) = params["sid"]
        .as_u64()
        .filter(|sid| state.subscriptions.contains_key(sid))
    else {
        return vec![error(id, 7, "Unknown subscription ID")];
    };
    if params["action"].as_str() == Some("get_snapshot") {
        return get_snapshot(state, script, id, sid, tickers);
    }
    let adding = params["action"].as_str() == Some("add_markets");
    {
        let subscription = state.subscriptions.entry(sid).or_default();
        if adding {
            subscription.tickers.extend(tickers.iter().cloned());
        } else {
            subscription.tickers.retain(|held| !tickers.contains(held));
        }
    }
    let book = state.is_book(sid);
    let numbered = state.numbered(sid);
    let mut frames = Vec::new();
    if script.delta_before_ok && book {
        let ticker = state.subscriptions[&sid]
            .tickers
            .first()
            .cloned()
            .unwrap_or_default();
        let seq = state.next_seq(sid);
        frames.push(delta(sid, seq, &ticker));
    }
    if script.ok_skips_seq && book {
        // A number the client never sees: the answer that follows has a gap behind it.
        state.next_seq(sid);
    }
    let seq = state.next_seq(sid);
    let held = state.subscriptions[&sid].tickers.clone();
    let mut ok = json!({
        "id": id,
        "sid": sid,
        "type": "ok",
        "sending_ts_ms": SENDING_TS_MS,
        "msg": {"market_tickers": held},
    });
    if numbered {
        ok["seq"] = json!(seq);
    }
    frames.push(ok);
    if adding && book {
        for ticker in tickers {
            let seq = state.next_seq(sid);
            frames.push(snapshot(sid, seq, ticker));
        }
    }
    frames
}

/// The answer to a `get_snapshot`: an `orderbook_snapshot` of each market asked for, in the
/// order asked, carrying the command's `id` and counting on the subscription's numbers,
/// with no `ok`; the script's knobs renumber, limit, refuse and follow it.
fn get_snapshot(
    state: &mut State,
    script: &Script,
    id: &Value,
    sid: u64,
    tickers: &[String],
) -> Vec<Value> {
    if let Some((code, message)) = &script.snapshot_error {
        return vec![error(id, *code, message)];
    }
    let limit = script.snapshot_limit.unwrap_or(usize::MAX);
    if let (Some(from), true) = (script.snapshot_seq, limit > 0) {
        state.subscriptions.entry(sid).or_default().seq = from.saturating_sub(1);
    }
    let mut frames = Vec::new();
    for ticker in tickers.iter().take(limit) {
        let seq = state.next_seq(sid);
        let mut frame = snapshot(sid, seq, ticker);
        frame["id"] = id.clone();
        frames.push(frame);
    }
    for frame in &script.after_snapshot {
        frames.push(parse(frame));
    }
    frames
}

fn unsubscribe(state: &mut State, id: &Value, params: &Value) -> Vec<Value> {
    let sids: Vec<u64> = params["sids"]
        .as_array()
        .map(|sids| sids.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let mut frames = Vec::new();
    for sid in sids {
        if !state.subscriptions.contains_key(&sid) {
            frames.push(error(id, 7, "Unknown subscription ID"));
            continue;
        }
        let numbered = state.numbered(sid);
        let seq = state.next_seq(sid);
        state.subscriptions.remove(&sid);
        let mut frame = json!({
            "id": id,
            "sid": sid,
            "type": "unsubscribed",
            "sending_ts_ms": SENDING_TS_MS,
        });
        if numbered {
            frame["seq"] = json!(seq);
        }
        frames.push(frame);
    }
    frames
}

```

- [ ] **Step 4: Run it to see it pass**

Run `nix develop --command cargo fmt --all` first and keep what it does to this task's files (the snippets above are written in rustfmt's form, but it may wrap a long line another way).

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib predictions::ws::scripted && nix develop --command cargo test -p polyoxide-kalshi --all-features --test supervision_predictions`

Expected: both end `test result: ok.` with no failures: the old and the new scripted tests pass, and the whole integration suite still passes on the unchanged protocol. Then `nix develop --command cargo fmt --all -- --check` and `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings` print nothing.

- [ ] **Step 5: Commit**

No new files.

```bash
git commit -m "test(kalshi): the scripted answer carries several channels and answers get_snapshot (Epic 9, Story 5)" -- polyoxide-kalshi/src/predictions/ws/scripted.rs
```

### Task 5.3: The protocol: channels, a `sid` for each, the empty-set rule and the gap re-snapshot

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/ws/protocol.rs:1-2`, `:5`, `:22`, `:35-36` (as Task 5.1 left them), `:38-668`, `:935-937`, `:1198`, and its `mod tests`
- Modify: `polyoxide-kalshi/src/predictions/ws/builder.rs:8-14`, `:16-21`, `:42-45`, `:77-93`, and its `mod tests`
- Modify: `polyoxide-kalshi/src/predictions/ws/mod.rs:1-21`, `:39-41`
- Modify: `polyoxide-kalshi/tests/supervision_predictions.rs:500-570` (the two gap tests)
- Test: `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`

This is the story's one large edit: the protocol's non-test code is replaced as a whole, from its first doc line to the closing brace of `impl PredictionsMembership for MembershipHandle<PredictionsProtocol>`. Its pieces: a `sid` for each channel, recorded by the correlation closure from every `subscribed` it reads (the closure returns `Push` for all but the last answer of a `subscribe`, so the request waits for one answer per channel); one command per `sid` for a change; the empty-set rule; `Sequences::observe`, which leaves a `sid` unchecked after a gap on the order book until the first snapshot answering the re-snapshot, and re-anchors on each such snapshot; `gapped`, which starts the re-snapshot (or errors for any other gap, or a second one); the follow-up queue; the pending set that drops deltas per ticker; and the timeout on each decoded frame. The follow-up's id comes from the protocol's own counter (`FOLLOW_UP_ID_BASE` up), since `follow_up` has no connection to take one from; a `snapshot()` call takes the connection's own id. Two existing unit tests change by one line each, and the two gap tests of `supervision_predictions.rs` are rewritten to D9, both by the owner's exceptions (spec §13).

- [ ] **Step 1: Write the failing tests**

*Unit tests.* In `protocol.rs`'s `mod tests`, replace the two lines of `commands_are_written_as_the_asyncapi_gives_them` (lines 935-937):

```rust
        let subscribe = Command::Subscribe {
            tickers: vec!["A-1".to_owned(), "B-2".to_owned()],
        };
```

with:

```rust
        let subscribe = Command::Subscribe {
            channels: vec![Channel::OrderbookDelta],
            tickers: vec!["A-1".to_owned(), "B-2".to_owned()],
        };
```

and replace the line in `a_fresh_connection_counts_from_scratch` (line 1198):

```rust
        assert!(protocol.sid.is_none());
```

with:

```rust
        assert!(protocol.sequences.sids.is_empty());
```

Append to `mod tests`, after `channels_are_kalshis_wire_names_and_sort_in_channel_order` (Task 5.1) and before the module's closing brace:

```rust
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
        json!({"id": id, "type": "subscribed", "msg": {"channel": channel, "sid": sid}})
            .to_string()
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
            answer(&mut sequences, 4, &subscribed_frame(4, "orderbook_delta", 1)),
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
        assert_eq!((update.dollar_volume, update.dollar_open_interest), (16_948, 10_211));
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
        let Decoded::Event(Event::Trade(trade)) = decoded(&mut protocol, &fixture("trade"))
        else {
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
        assert_eq!(trade.is_block_trade, msg["is_block_trade"].as_bool().unwrap());
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
        protocol.accepted(&Command::Unsubscribe { sid: 1 });
        assert!(protocol.sequences.sids.is_empty());
        assert!(protocol.sequences.last.is_empty());
        assert!(protocol.sequences.unanchored.is_empty());
        assert!(protocol.resync.is_none());
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
```

The new tests need no import the module lacks: `BTreeMap`, `BTreeSet`, `Message`, `Recovery` and the rest come from `super::*`, and `Decimal`, `UnixMillis`, `BookSide`, `json` and `Value` from its existing `use` lines.

*Builder test.* Append to `builder.rs`'s `mod tests`, after `the_url_rule_is_wss_or_a_loopback_ws` and before the module's closing brace:

```rust
    #[test]
    fn the_default_channel_is_the_order_book_and_channels_are_deduplicated_in_order() {
        assert_eq!(
            PredictionsWsBuilder::new().channels,
            BTreeSet::from([Channel::OrderbookDelta])
        );
        let builder = PredictionsWsBuilder::new().channels([
            Channel::Trade,
            Channel::OrderbookDelta,
            Channel::Trade,
        ]);
        assert_eq!(
            builder.channels.iter().copied().collect::<Vec<_>>(),
            [Channel::OrderbookDelta, Channel::Trade]
        );
        // An empty set is the builder's to hold and `connect`'s to refuse.
        assert!(PredictionsWsBuilder::new()
            .channels(Vec::new())
            .channels
            .is_empty());
    }
```

*The two gap tests.* In `tests/supervision_predictions.rs`, replace the two tests below, each from its `#[tokio::test]` attribute through its closing brace. First:

```rust
#[tokio::test]
async fn a_sequence_gap_reconnects_and_resnapshots() {
    // Bug: carrying on past a lost delta, so a book built on it is silently wrong.
    let server = KalshiServer::start(vec![
        Script {
            // The snapshot is seq 1; a delta at 3 skips 2.
            pushes: vec![delta(1, 3, A).to_string()],
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (1, 2, 3)),
        other => panic!("expected the gap to end the connection, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}
```

with:

```rust
fn label(event: &Event) -> &'static str {
    match event {
        Event::Snapshot(_) => "snapshot",
        Event::Delta(_) => "delta",
        Event::Ticker(_) => "ticker",
        Event::Trade(_) => "trade",
        Event::Resyncing { .. } => "resyncing",
        Event::Disconnected { .. } => "disconnected",
        Event::Reconnected => "reconnected",
        _ => "other",
    }
}

/// The labels of the next `count` events, naming every kind the socket yields.
async fn labels(ws: &mut SupervisedPredictionsWs, count: usize) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for _ in 0..count {
        seen.push(label(&next_event(ws).await));
    }
    seen
}

#[tokio::test]
async fn a_sequence_gap_resnapshots_in_place() {
    // Bug: carrying on past a lost delta, so a book built on it is silently wrong; and
    // reconnecting for it, which loses every other market's book and the connection too.
    let server = KalshiServer::start(vec![Script {
        // The snapshot is seq 1; a delta at 3 skips 2.
        pushes: vec![delta(1, 3, A).to_string()],
        // The snapshot that answers the request is number 10, and the delta after it 11:
        // had the subscription not re-anchored on 10, that delta would be a gap.
        snapshot_seq: Some(10),
        after_snapshot: vec![delta(1, 11, A).to_string()],
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Resyncing { tickers } => assert_eq!(tickers, [A]),
        other => panic!("expected the gap to start a re-snapshot, got {other:?}"),
    }
    match next_event(&mut ws).await {
        Event::Snapshot(book) => assert_eq!(book.market_ticker, A),
        other => panic!("expected the snapshot that answers it, got {other:?}"),
    }
    assert!(matches!(next_event(&mut ws).await, Event::Delta(_)));
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    // One connection throughout, and one request for the snapshot.
    assert_eq!(server.connection_count(), 1);
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "{requests:?}");
    assert_eq!(requests[1]["cmd"], "update_subscription");
    assert_eq!(
        requests[1]["params"],
        json!({"sid": 1, "action": "get_snapshot", "market_tickers": [A]})
    );
    assert_ne!(requests[1]["id"], requests[0]["id"]);
    ws.close().await.expect("close");
}
```

Second:

```rust
#[tokio::test]
async fn a_gap_in_an_answer_reconnects() {
    // Bug: checking only the pushes, so an `ok` that skipped a number goes unnoticed and
    // the delta lost behind it is never found.
    let server = KalshiServer::start(vec![
        Script {
            ok_skips_seq: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // The change itself succeeded; the gap behind it ends the connection.
    membership.subscribe([t(B)]).await.expect("the change");
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (1, 2, 3)),
        other => panic!("expected the gap in the answer, got {other:?}"),
    }
    assert_eq!(
        kinds(&mut ws, 3).await,
        ["reconnected", "snapshot", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A, B]);
    ws.close().await.expect("close");
}
```

with:

```rust
#[tokio::test]
async fn a_gap_in_an_answer_resnapshots_in_place() {
    // Bug: checking only the pushes, so an `ok` that skipped a number goes unnoticed and
    // the delta lost behind it is never found; and, once found, repairing it by a
    // reconnect, where a snapshot of the whole membership on the same connection is enough.
    let server = KalshiServer::start(vec![Script {
        ok_skips_seq: true,
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // The change itself succeeded; the gap behind its `ok` starts the re-snapshot, ahead of
    // the added market's own snapshot, which was the frame that raised it and is not lost.
    membership.subscribe([t(B)]).await.expect("the change");
    match next_event(&mut ws).await {
        Event::Resyncing { tickers } => assert_eq!(tickers, [A, B]),
        other => panic!("expected the gap in the answer to start a re-snapshot, got {other:?}"),
    }
    // B's snapshot from the change, then the two that answer the request.
    assert_eq!(
        labels(&mut ws, 3).await,
        ["snapshot", "snapshot", "snapshot"]
    );
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    let requests = server.requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    assert_eq!(
        requests[2]["params"],
        json!({"sid": 1, "action": "get_snapshot", "market_tickers": [A, B]})
    );
    ws.close().await.expect("close");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib predictions::ws`

Expected: the build fails with `no field `channels` on type `PredictionsWsBuilder``, `no method named `configured` found for struct `PredictionsProtocol``, `no field `sids` on type `Sequences``, `cannot find value `FOLLOW_UP_ID_BASE` in this scope` and `struct `Sequences` has no field named `awaiting``.

- [ ] **Step 3: Implement**

*`protocol.rs`, the header.* Replace the first two lines:

```rust
//! Kalshi's order-book socket protocol, as the kit's `Supervisor` drives it, and the
//! membership calls on its handle.
```

with:

```rust
//! Kalshi's socket protocol, as the kit's `Supervisor` drives it, and the membership calls on
//! its handle.
```

Replace the collections import:

```rust
    collections::{BTreeSet, HashMap},
```

with:

```rust
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
```

Replace:

```rust
use serde_json::json;
```

with:

```rust
use serde_json::json;
use tokio::time::Instant;
```

Replace the constant Task 5.1 left above `Channel`:

```rust
/// The one channel this socket carries.
const CHANNEL: &str = "orderbook_delta";
```

with:

```rust
/// The first id of the commands the protocol numbers itself: the re-snapshot it sends from
/// `follow_up`, which has no connection to take an id from. The kit's own ids count from 1 on
/// each connection, so the two never meet.
const FOLLOW_UP_ID_BASE: u64 = 1_000_000_000;
```

*`protocol.rs`, the body.* Replace everything from the line `/// One change to the subscribed markets, made through [`PredictionsMembership`] or` (the line after `Channel`'s closing brace) through the closing brace of `impl PredictionsMembership for MembershipHandle<PredictionsProtocol>`, the last item above `#[cfg(test)]`, with:

```rust
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
    /// A `snapshot` call's request.
    Requested,
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

/// Kalshi's socket, which [`PredictionsWsBuilder`](crate::predictions::ws::PredictionsWsBuilder)
/// hands to the kit's `Supervisor`.
///
/// Its membership is the set of tickers subscribed, one set for every channel chosen, replayed
/// on every fresh connection as one `subscribe` naming the channels (with `use_yes_price: true`
/// when the order book is one of them). Kalshi answers with a `subscribed` for each channel, and
/// the protocol keeps each channel's `sid`. A change goes out as one `update_subscription` on
/// each live `sid`, in channel order; the first add with no subscription sends a `subscribe`,
/// and emptying the membership sends an `unsubscribe` for each `sid` and keeps the connection.
/// Kalshi reads a `ticker` or `trade` subscribe with no markets as every market, so an empty
/// membership is never subscribed on any channel, on the first connection or after a
/// reconnect. A change is recorded before its frames are sent: one whose connection dies under
/// it is answered `Ok` and replayed, one Kalshi refuses is undone, and one made during an
/// outage is answered at once and replayed. Its divergence rows:
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
///   subscription's frames unchecked until the first snapshot answering it, re-anchors the
///   numbering on each such snapshot, and withholds each market's deltas until its snapshot
///   has arrived. It reconnects when the snapshots are not all in by the staleness window
///   after the command was queued (checked on each decoded frame), when an error frame
///   answers the command, and on a second gap while one is being repaired.
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
                self.resync = None;
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
            let id = resync.id;
            self.sequences.follow_ups.remove(&id);
            self.resync = None;
        }
    }

    /// A snapshot of `market` arrived, answering command `id` when it is the re-snapshot's.
    fn snapshot_arrived(&mut self, id: Option<u64>, market: &str) {
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
                return self.ask_for_snapshots(connection, membership, &tickers).await;
            }
        };
        for command in &commands {
            match send(&mut self.sequences, connection, command).await {
                Ok(_) => self.accepted(command),
                Err(PredictionsWsError::Connection(err)) => {
                    tracing::debug!(%err, "the connection failed under a change; it is recorded for the replay");
                    return Ok(());
                }
                Err(err) => {
                    *membership = before;
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
        let Some(sid) = self
            .sequences
            .sids
            .get(&Channel::OrderbookDelta)
            .copied()
        else {
            return Ok(());
        };
        if wanted.is_empty() {
            return Ok(());
        }
        let id = connection.take_id();
        self.sequences.follow_ups.insert(id, FollowUp::Requested);
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
/// checked until the first snapshot that answers the re-snapshot, and each such snapshot
/// re-anchors the numbering on its own `seq`. The correlation closure of a request observes
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
    /// reads a `ticker` or `trade` subscribe with no markets as every market. A refusal on a
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
    /// undoes the change and is the caller's error; the connection failing under it leaves it
    /// recorded and answers `Ok`, since the next read ends the connection and the replay
    /// applies it.
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

    /// Kalshi's recovery: a frame that does not decode is skipped on a live connection and
    /// stops the stream when it was the answer to the replay's `subscribe`; an error frame's
    /// code decides (9 stops the stream, 10, 17, 18, 25 and 27 reconnect, any other is
    /// skipped on a live connection, stops the stream when it refused the replay's
    /// `subscribe`, and reconnects when it answers the re-snapshot's command); a gap
    /// reconnects, and so does a snapshot that never came; a connection used after it closed
    /// reconnects (D19); and anything else reconnects exactly when its class is retriable, so
    /// a refused handshake's 401 or 403 stops, and a 429 or 5xx retries. No code here matches
    /// over `tungstenite::Error`: the socket table decides the class.
    ///
    /// A skipped error during an attempt reads as a reconnect, the kit's rule for a failed
    /// attempt, which is why a refused replay has its own row: it would otherwise
    /// reconnect for ever, refused each time, and so would an answer that never decodes.
    fn recover(&self, err: &PredictionsWsError) -> Recovery {
        match err {
            PredictionsWsError::Json { .. } if self.replay_refused => Recovery::Stop,
            PredictionsWsError::Json { .. } => Recovery::Skip,
            PredictionsWsError::Server { code, .. } => match code {
                9 => Recovery::Stop,
                10 | 17 | 18 | 25 | 27 => Recovery::Reconnect,
                _ if self.replay_refused => Recovery::Stop,
                _ if self.snapshot_refused => Recovery::Reconnect,
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
    /// stopped.
    fn subscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send;

    /// Unsubscribe from markets, failing as [`subscribe`](Self::subscribe) does.
    /// Unsubscribing every market sends an `unsubscribe` for each subscription and keeps the
    /// connection, which keeps pinging; a reconnect then replays nothing, since Kalshi reads
    /// a `ticker` or `trade` subscription with no markets as every market.
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
```

*`builder.rs`.* Replace the imports (lines 8-14):

```rust
use crate::predictions::{
    key::Ticker,
    ws::{
        protocol::PredictionsProtocol, PredictionsWsError, SupervisedPredictionsWs, PRODUCTION_WS,
    },
    Credentials,
};
```

with:

```rust
use crate::predictions::{
    key::Ticker,
    ws::{
        protocol::PredictionsProtocol, Channel, PredictionsWsError, SupervisedPredictionsWs,
        PRODUCTION_WS,
    },
    Credentials,
};
```

Replace the struct and the start of `new` (lines 16-45):

```rust
/// Builder for a supervised order-book socket.
#[derive(Debug, Clone)]
pub struct PredictionsWsBuilder {
    url: String,
    timing: Timing,
}
```

with:

```rust
/// Builder for a supervised socket of Kalshi's channels.
#[derive(Debug, Clone)]
pub struct PredictionsWsBuilder {
    url: String,
    timing: Timing,
    channels: BTreeSet<Channel>,
}
```

Replace in `new`:

```rust
        Self {
            url: PRODUCTION_WS.to_owned(),
            timing,
        }
    }
```

with:

```rust
        Self {
            url: PRODUCTION_WS.to_owned(),
            timing,
            channels: BTreeSet::from([Channel::OrderbookDelta]),
        }
    }

    /// The channels to carry, replacing the default, the order book alone. Duplicates are
    /// dropped, and the order given does not matter: the channels go out, and a membership
    /// change reaches them, in [`Channel`]'s order. At least one is needed, and an empty set
    /// is refused at [`connect`](Self::connect) as [`PredictionsWsError::InvalidRequest`].
    ///
    /// One set of tickers serves every channel. Kalshi reads a `ticker` or `trade`
    /// subscription with no markets as every market, so a socket subscribes nothing while
    /// its ticker set is empty.
    pub fn channels(mut self, channels: impl IntoIterator<Item = Channel>) -> Self {
        self.channels = channels.into_iter().collect();
        self
    }
```

Replace the end of `connect` (lines 84-93):

```rust
    pub async fn connect(
        self,
        credentials: Credentials,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> Result<SupervisedPredictionsWs, PredictionsWsError> {
        let membership: BTreeSet<Ticker> = tickers.into_iter().collect();
        Supervisor::new(PredictionsProtocol::new(self.url, credentials), self.timing)
            .connect(membership)
            .await
    }
```

with:

```rust
    pub async fn connect(
        self,
        credentials: Credentials,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> Result<SupervisedPredictionsWs, PredictionsWsError> {
        if self.channels.is_empty() {
            return Err(PredictionsWsError::InvalidRequest {
                reason: "the socket needs at least one channel".to_owned(),
            });
        }
        let membership: BTreeSet<Ticker> = tickers.into_iter().collect();
        let protocol = PredictionsProtocol::new(self.url, credentials)
            .configured(self.channels, self.timing.stale_after);
        Supervisor::new(protocol, self.timing)
            .connect(membership)
            .await
    }
```

and replace the connect doc's first lines (77-80):

```rust
    /// Connect, sign the handshake, subscribe to `tickers` and start supervising. With no
    /// tickers the connection opens and subscribes nothing; the first
    /// [`subscribe`](crate::predictions::ws::PredictionsMembership::subscribe) does.
```

with:

```rust
    /// Connect, sign the handshake, subscribe to `tickers` on every channel chosen and start
    /// supervising. With no tickers the connection opens and subscribes nothing, on any
    /// channel; the first
    /// [`subscribe`](crate::predictions::ws::PredictionsMembership::subscribe) does.
```

*`mod.rs`.* Replace the module docs (lines 1-21):

```rust
//! The order-book socket of the predictions exchange.
//!
//! One connection to [`PRODUCTION_WS`] or [`DEMO_WS`] carries the `orderbook_delta`
//! channel: a snapshot of each subscribed market's book, then the changes to it, with
//! every price in YES pricing (the subscription asks for `use_yes_price`, so a NO bid is
//! reported as the YES ask it is). Kalshi refuses a handshake that carries no API-key
//! headers, public data included, so a socket is built from `Credentials`, and every
//! connection attempt signs its own handshake: a signature carries its timestamp, and a
//! reconnect that replayed an old one would be refused.
//!
//! The socket is run by the socket kit's `Supervisor`. It pings every 10 s, counts every
//! frame as liveness, reconnects after a drop, a stall or a gap in a subscription's
//! `seq`, and replays the whole membership as one `subscribe`. Every outage yields one
//! `Event::Disconnected` and one `Event::Reconnected`, and what the socket carried in
//! between is lost, so a consumer rebuilds its books from the fresh snapshots that
//! follow. If Kalshi refuses that `subscribe` with a code that does not reconnect (a
//! market it will not serve, say), or answers it with a frame that does not decode, the
//! stream yields the error and ends, for every market: a change made during an outage was
//! answered `Ok`, so this is where its caller learns of the refusal.
//!
//! Errors are `PredictionsWsError`, classified through the one socket table.
```

with:

```rust
//! The socket of the predictions exchange.
//!
//! One connection to [`PRODUCTION_WS`] or [`DEMO_WS`] carries the channels chosen with
//! [`PredictionsWsBuilder::channels`]: `orderbook_delta`, the default, a snapshot of each
//! subscribed market's book and then the changes to it, with every price in YES pricing (the
//! subscription asks for `use_yes_price`, so a NO bid is reported as the YES ask it is);
//! `ticker`, a market's last price, best bid and ask, volume and open interest; and `trade`,
//! each public trade. One set of tickers serves every channel, and Kalshi gives each channel
//! its own subscription, whose id the protocol keeps. Kalshi refuses a handshake that carries
//! no API-key headers, public data included, so a socket is built from `Credentials`, and every
//! connection attempt signs its own handshake: a signature carries its timestamp, and a
//! reconnect that replayed an old one would be refused.
//!
//! The socket is run by the socket kit's `Supervisor`. It pings every 10 s, counts every
//! frame as liveness, reconnects after a drop or a stall, and replays the whole membership as
//! one `subscribe` naming every channel. Every outage yields one `Event::Disconnected` and
//! one `Event::Reconnected`, and what the socket carried in between is lost, so a consumer
//! rebuilds its books from the fresh snapshots that follow. If Kalshi refuses that
//! `subscribe` with a code that does not reconnect (a market it will not serve, say), or
//! answers it with a frame that does not decode, the stream yields the error and ends, for
//! every market: a change made during an outage was answered `Ok`, so this is where its
//! caller learns of the refusal.
//!
//! Kalshi reads a `ticker` or `trade` subscription with no markets as every market, so a
//! socket subscribes nothing while its ticker set is empty, on any channel: emptying the set
//! unsubscribes every channel, and the first market added afterwards subscribes afresh.
//!
//! A gap in the order book's `seq` is repaired on the same connection. The stream yields
//! `Event::Resyncing` for the whole membership, the socket asks Kalshi for a snapshot of every
//! market with `get_snapshot`, withholds each market's deltas until its snapshot has arrived,
//! and carries on from the snapshots, with no `Disconnected`. It reconnects instead if the
//! snapshots do not come within the staleness window, if Kalshi answers the request with an
//! error, or if a second gap comes while the first is being repaired. A gap on the `trade`
//! subscription, which has no snapshot, reconnects. `PredictionsMembership::snapshot` asks for
//! snapshots of subscribed markets by the same request.
//!
//! Errors are `PredictionsWsError`, classified through the one socket table.
```

and replace the alias's doc (lines 39-52):

```rust
/// A supervised connection: a `Stream` of [`Event`]s that survives drops, stalls and gaps
/// by reconnecting and replaying its subscription, run by the kit's `Supervisor` on
/// [`PredictionsProtocol`]. Build one with [`PredictionsWsBuilder`].
```

with:

```rust
/// A supervised connection: a `Stream` of [`Event`]s that survives drops and stalls by
/// reconnecting and replaying its subscription, and repairs a gap in the order book's
/// sequence in place, run by the kit's `Supervisor` on [`PredictionsProtocol`]. Build one
/// with [`PredictionsWsBuilder`].
```

*`protocol.rs`'s old unit tests that this replacement touches.* The test `an_answer_is_matched_by_its_id`, `a_push_read_while_a_request_waits_is_counted_once` and `a_gap_in_an_answer_is_raised_by_the_next_frame` keep their bodies: their `Sequences` has no `sids`, so a gap there is not the order book's and still comes out as `SequenceGap`.

- [ ] **Step 4: Run it to see it pass**

Run `nix develop --command cargo fmt --all` first and keep what it does to this task's files (the snippets above are written in rustfmt's form, but it may wrap a long line another way).

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib predictions::ws && nix develop --command cargo test -p polyoxide-kalshi --all-features --test supervision_predictions`

Expected: both end `test result: ok.` with no failures: the new unit tests, the two rewritten gap tests and every earlier supervision test pass. Then `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings` and `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'` print no warning.

- [ ] **Step 5: Commit**

No new files.

```bash
git commit -m "feat(kalshi): the socket carries ticker and trade, empties to nothing and re-snapshots a gap in place (Epic 9, Story 5)" -- polyoxide-kalshi/src/predictions/ws/protocol.rs polyoxide-kalshi/src/predictions/ws/builder.rs polyoxide-kalshi/src/predictions/ws/mod.rs polyoxide-kalshi/tests/supervision_predictions.rs
```

### Task 5.4: The supervised socket's suites for channels, the empty set and the re-snapshot

**Files:**
- Modify: `polyoxide-kalshi/tests/supervision_predictions.rs:7-17`, `:28`, and append after its last test
- Test: `polyoxide-kalshi/tests/supervision_predictions.rs`

Appended tests drive the whole path on the scripted server. Each names the bug it exists to catch, and Step 2 proves the two that guard the new recovery rules fail without them.

- [ ] **Step 1: Write the failing tests**

Replace the file's imports (lines 7-17):

```rust
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        DisconnectReason, Event, PredictionsMembership, PredictionsWsBuilder, PredictionsWsError,
        SupervisedPredictionsWs,
    },
    Credentials,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use polyoxide_ws::test_server::{next_event, next_item, WINDOW};
use ring::signature::{UnparsedPublicKey, ED25519};
use serde_json::{json, Value};
```

with:

```rust
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        Channel, DisconnectReason, Event, PredictionsMembership, PredictionsWsBuilder,
        PredictionsWsError, SupervisedPredictionsWs,
    },
    Credentials,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use polyoxide_ws::test_server::{next_event, next_item, WINDOW};
use ring::signature::{UnparsedPublicKey, ED25519};
use rust_decimal::Decimal;
use serde_json::{json, Value};
```

and replace the scripted import (line 28):

```rust
use scripted::{delta, KalshiServer, Script};
```

with:

```rust
use scripted::{delta, ticker_frame, trade_frame, KalshiServer, Script};
```

Append to `tests/supervision_predictions.rs`, after `an_empty_membership_connects_sends_nothing_and_keeps_pinging`:

```rust
/// Asserts a request is a `subscribe` naming exactly `channels`, in that order, and the
/// markets `expected`.
fn assert_subscribe_to(frame: &Value, channels: &[&str], expected: &[&str]) {
    assert_eq!(frame["cmd"], "subscribe");
    assert_eq!(frame["params"]["channels"], json!(channels));
    let mut tickers: Vec<&str> = frame["params"]["market_tickers"]
        .as_array()
        .expect("a ticker list")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    tickers.sort_unstable();
    assert_eq!(tickers, expected);
}

const EVERY_CHANNEL: [&str; 3] = ["orderbook_delta", "ticker", "trade"];

#[tokio::test]
async fn the_replay_names_every_channel_once_in_channel_order() {
    // Bug: a subscribe naming the first channel only, or the channels in the order they
    // were given, which a later update then reads against the wrong subscriptions.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .channels([
            Channel::Trade,
            Channel::OrderbookDelta,
            Channel::Trade,
            Channel::Ticker,
        ])
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    let firsts = server.first_frames();
    assert_subscribe_to(&firsts[0], &EVERY_CHANNEL, &[A]);
    assert_eq!(firsts[0]["params"]["use_yes_price"], true);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_subscribe_without_the_order_book_does_not_ask_for_yes_pricing() {
    // Bug: sending the order book's flag on channels it does not apply to.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let ws = builder_for(&server)
        .channels([Channel::Ticker, Channel::Trade])
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let firsts = server.first_frames();
    assert_subscribe_to(&firsts[0], &["ticker", "trade"], &[A]);
    assert!(firsts[0]["params"].get("use_yes_price").is_none());
    drop(ws);
}

#[tokio::test]
async fn a_change_updates_every_channels_subscription_in_channel_order() {
    // Bug: an update sent to the order book's subscription alone, which leaves the other
    // channels on the old set of markets.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .channels(Channel::ALL.iter().copied())
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    membership.subscribe([t(B)]).await.expect("add");
    match next_event(&mut ws).await {
        Event::Snapshot(book) => assert_eq!(book.market_ticker, B),
        other => panic!("expected the added market's snapshot, got {other:?}"),
    }
    membership.unsubscribe([t(A)]).await.expect("delete");
    let requests = server.requests();
    assert_eq!(requests.len(), 7, "{requests:?}");
    for (n, sid) in [1u64, 2, 3].into_iter().enumerate() {
        assert_eq!(
            requests[1 + n]["params"],
            json!({"sid": sid, "action": "add_markets", "market_tickers": [B]}),
            "{requests:?}"
        );
        assert_eq!(
            requests[4 + n]["params"],
            json!({"sid": sid, "action": "delete_markets", "market_tickers": [A]}),
            "{requests:?}"
        );
    }
    // The kit's ids, one for each command.
    let ids: Vec<u64> = requests.iter().filter_map(|r| r["id"].as_u64()).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn emptying_the_set_unsubscribes_every_subscription_and_the_next_add_subscribes_afresh() {
    // Bug: leaving the ticker and trade subscriptions open on an empty set, which Kalshi
    // reads as every market, or sending the first add to subscriptions that are gone.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .channels(Channel::ALL.iter().copied())
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    membership
        .unsubscribe([t(A)])
        .await
        .expect("empty the membership");
    membership.subscribe([t(C)]).await.expect("add after empty");
    let requests = server.requests();
    assert_eq!(requests.len(), 5, "{requests:?}");
    for (n, sid) in [1u64, 2, 3].into_iter().enumerate() {
        assert_eq!(requests[1 + n]["cmd"], "unsubscribe", "{requests:?}");
        assert_eq!(requests[1 + n]["params"], json!({"sids": [sid]}));
    }
    assert_subscribe_to(&requests[4], &EVERY_CHANNEL, &[C]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_empty_set_is_never_subscribed_on_any_channel_not_even_after_a_reconnect() {
    // Bug: replaying a subscribe with no markets after a reconnect, which on the ticker
    // and trade channels subscribes to every market on Kalshi.
    let server = KalshiServer::start(vec![
        Script {
            cut_after: Some(Duration::from_millis(400)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .channels(Channel::ALL.iter().copied())
        .connect(credentials(), Vec::<Ticker>::new())
        .await
        .expect("connect");
    // Nothing is sent while the set is empty.
    server
        .wait_for("a handshake", |s| s.handshake_headers().len() == 1)
        .await;
    assert!(server.requests().is_empty());
    let membership = ws.membership();
    membership.subscribe([t(A)]).await.expect("add");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    membership
        .unsubscribe([t(A)])
        .await
        .expect("empty the membership");
    assert_eq!(labels(&mut ws, 2).await, ["disconnected", "reconnected"]);
    assert_quiet(&mut ws, Duration::from_millis(200)).await;
    // The first connection's subscribe and three unsubscribes; the second sent nothing.
    assert_eq!(server.requests().len(), 4, "{:?}", server.requests());
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn ticker_frames_carry_no_seq_and_pass() {
    // Bug: checking a channel that has no sequence, which reads the second frame as a gap.
    let server = KalshiServer::start(vec![Script {
        pushes: vec![
            ticker_frame(2, A).to_string(),
            ticker_frame(2, A).to_string(),
            ticker_frame(2, A).to_string(),
        ],
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .channels([Channel::OrderbookDelta, Channel::Ticker])
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Ticker(update) => {
            assert_eq!(update.market_ticker, A);
            assert_eq!(update.yes_bid, "0.4500".parse::<Decimal>().unwrap());
            assert_eq!(update.yes_ask, "0.5300".parse::<Decimal>().unwrap());
        }
        other => panic!("expected a ticker update, got {other:?}"),
    }
    assert_eq!(labels(&mut ws, 2).await, ["ticker", "ticker"]);
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn trades_are_events_on_the_trade_subscription() {
    let server = KalshiServer::start(vec![Script {
        pushes: vec![
            trade_frame(2, 1, A).to_string(),
            trade_frame(2, 2, A).to_string(),
        ],
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .channels([Channel::OrderbookDelta, Channel::Trade])
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Trade(trade) => {
            assert_eq!(trade.market_ticker, A);
            assert_eq!(trade.yes_price, "0.3600".parse::<Decimal>().unwrap());
            assert_eq!(trade.count, "136.00".parse::<Decimal>().unwrap());
        }
        other => panic!("expected a trade, got {other:?}"),
    }
    assert!(matches!(next_event(&mut ws).await, Event::Trade(_)));
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_gap_on_the_trade_subscription_reconnects() {
    // Bug: re-snapshotting the order book for a lost trade, which repairs nothing: the
    // trade channel has no snapshot to ask for.
    let server = KalshiServer::start(vec![
        Script {
            // Numbers 1 then 3 on the trade subscription.
            pushes: vec![
                trade_frame(2, 1, A).to_string(),
                trade_frame(2, 3, A).to_string(),
            ],
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .channels([Channel::OrderbookDelta, Channel::Trade])
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(labels(&mut ws, 2).await, ["snapshot", "trade"]);
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (2, 2, 3)),
        other => panic!("expected the gap to end the connection, got {other:?}"),
    }
    assert_eq!(labels(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    let requests = server.requests();
    assert!(
        requests.iter().all(|request| request["cmd"] == "subscribe"),
        "{requests:?}"
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn deltas_between_a_gap_and_the_snapshot_are_dropped() {
    // Bug: yielding a delta that applies to a book the consumer has dropped, which the
    // snapshot then replaces after a flash of wrong prices.
    let server = KalshiServer::start(vec![Script {
        // The gap, then a delta the stale book must not see.
        pushes: vec![delta(1, 3, A).to_string(), delta(1, 4, A).to_string()],
        snapshot_seq: Some(10),
        after_snapshot: vec![delta(1, 11, A).to_string()],
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        labels(&mut ws, 4).await,
        ["snapshot", "resyncing", "snapshot", "delta"]
    );
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_missing_snapshot_reconnects_after_the_staleness_window() {
    // Bug: waiting for ever for a snapshot Kalshi does not send, with the books dropped.
    let server = KalshiServer::start(vec![
        Script {
            pushes: vec![delta(1, 3, A).to_string()],
            snapshot_limit: Some(0),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(labels(&mut ws, 2).await, ["snapshot", "resyncing"]);
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Error(PredictionsWsError::SnapshotTimeout { sid, waited }),
        } => {
            assert_eq!(sid, 1);
            assert!(waited >= Duration::from_millis(300), "{waited:?}");
        }
        other => panic!("expected the missing snapshot to end the connection, got {other:?}"),
    }
    assert_eq!(labels(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_error_answering_the_resnapshot_reconnects() {
    // Bug: skipping the refusal as an ordinary code-11 frame, which leaves the books
    // dropped and their deltas withheld for ever.
    let server = KalshiServer::start(vec![
        Script {
            pushes: vec![delta(1, 3, A).to_string()],
            snapshot_error: Some((11, "Invalid parameter".to_owned())),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(labels(&mut ws, 2).await, ["snapshot", "resyncing"]);
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Error(PredictionsWsError::Server { code, .. }),
        } => assert_eq!(code, 11),
        other => panic!("expected the refusal to end the connection, got {other:?}"),
    }
    assert_eq!(labels(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_second_gap_during_a_resnapshot_reconnects() {
    // Bug: starting a second re-snapshot over the first, which never finishes under a feed
    // that keeps losing messages.
    let server = KalshiServer::start(vec![
        Script {
            // Snapshots 1 and 2, then a delta at 4: a gap.
            pushes: vec![delta(1, 4, A).to_string()],
            // Only A's snapshot answers, as number 10; B's is still awaited when the
            // delta at 13 skips 11 and 12.
            snapshot_limit: Some(1),
            snapshot_seq: Some(10),
            after_snapshot: vec![delta(1, 13, A).to_string()],
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A), t(B)])
        .await
        .expect("connect");
    assert_eq!(
        labels(&mut ws, 4).await,
        ["snapshot", "snapshot", "resyncing", "snapshot"]
    );
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (1, 11, 13)),
        other => panic!("expected the second gap to end the connection, got {other:?}"),
    }
    assert_eq!(
        labels(&mut ws, 3).await,
        ["reconnected", "snapshot", "snapshot"]
    );
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn snapshot_asks_for_snapshots_without_changing_the_membership() {
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    membership.snapshot([t(A)]).await.expect("the request");
    match next_event(&mut ws).await {
        Event::Snapshot(book) => assert_eq!(book.market_ticker, A),
        other => panic!("expected the snapshot that answers it, got {other:?}"),
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "{requests:?}");
    assert_eq!(
        requests[1]["params"],
        json!({"sid": 1, "action": "get_snapshot", "market_tickers": [A]})
    );
    assert_ne!(requests[1]["id"], requests[0]["id"]);
    // The answer was on the subscription's own numbering: no gap, no reconnect.
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_snapshot_of_a_market_outside_the_membership_is_refused_and_sends_nothing() {
    // Bug: asking Kalshi for a book of a market the socket does not follow, which it
    // answers with an error that reads like the stream's.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    let err = membership
        .snapshot([t(B)])
        .await
        .expect_err("B is not subscribed");
    assert!(
        matches!(err, PredictionsWsError::InvalidRequest { .. }),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest);
    assert_eq!(server.requests().len(), 1, "only the subscribe");
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_snapshot_without_the_order_book_channel_is_refused_and_sends_nothing() {
    let server = KalshiServer::start(vec![Script::default()]).await;
    let ws = builder_for(&server)
        .channels([Channel::Ticker])
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    let err = membership
        .snapshot([t(A)])
        .await
        .expect_err("no order book to ask for");
    assert!(
        matches!(err, PredictionsWsError::InvalidRequest { .. }),
        "{err:?}"
    );
    assert_eq!(server.requests().len(), 1, "only the subscribe");
    drop(ws);
}

#[tokio::test]
async fn a_snapshot_during_an_outage_is_answered_ok_and_sends_nothing() {
    // Bug: holding the request for the next connection, which asks for a snapshot the
    // replay has already brought, or failing it for an outage that is the stream's.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    // A long backoff keeps the outage open while the request is made.
    let mut ws = builder_for(&server)
        .backoff(Duration::from_millis(200), Duration::from_millis(200))
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert_eq!(labels(&mut ws, 2).await, ["snapshot", "disconnected"]);
    membership
        .snapshot([t(A)])
        .await
        .expect("answered at once during the outage");
    assert_eq!(labels(&mut ws, 2).await, ["reconnected", "snapshot"]);
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "{requests:?}");
    assert!(
        requests.iter().all(|request| request["cmd"] == "subscribe"),
        "{requests:?}"
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_connect_with_no_channel_is_refused_before_any_connection() {
    let server = KalshiServer::start(vec![Script::default()]).await;
    let err = builder_for(&server)
        .channels(Vec::new())
        .connect(credentials(), [t(A)])
        .await
        .expect_err("no channel");
    assert!(
        matches!(err, PredictionsWsError::InvalidRequest { .. }),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest);
    assert_eq!(server.connection_count(), 0);
}
```

- [ ] **Step 2: Run it, then prove the two recovery rules are tested**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test supervision_predictions`

Expected: `test result: ok.` with no failure (the implementation is in place, so these pass at once).

Then prove each of two guards fails without its rule, and put each change back:

1. In `protocol.rs`, in `recover`, delete the row `_ if self.snapshot_refused => Recovery::Reconnect,`. Run `nix develop --command cargo test -p polyoxide-kalshi --all-features --test supervision_predictions an_error_answering_the_resnapshot_reconnects`: it fails with `an event, not an error: Server { code: 11, message: "Invalid parameter" }` (the refusal is yielded and skipped, and the books stay dropped). Restore the row.
2. In `protocol.rs`, in `decode_frame`, change `Ok(frame) if self.is_pending(&frame.msg.market_ticker) => Decoded::Nothing,` to `Ok(frame) if false => Decoded::Nothing,`. Run `nix develop --command cargo test -p polyoxide-kalshi --all-features --test supervision_predictions deltas_between_a_gap_and_the_snapshot_are_dropped`: it fails with `left: ["snapshot", "resyncing", "delta", "snapshot"]` against the expected order. Restore the line.

Run the whole suite again and see it pass.

- [ ] **Step 3: Implement**

Nothing to add: Task 5.3 holds the code these tests exercise. If a test here fails on the unmodified protocol, the failure is the finding: fix the protocol in `ws/protocol.rs` (this story's file) and add the assertion that would have caught it to Task 5.3's unit tests.

- [ ] **Step 4: Run it to see it pass**

Run `nix develop --command cargo fmt --all` first and keep what it does to this task's files (the snippets above are written in rustfmt's form, but it may wrap a long line another way).

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features && nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`

Expected: every test passes and neither check prints anything.

- [ ] **Step 5: Commit**

No new files.

```bash
git commit -m "test(kalshi): the supervised socket's channels, empty-set rule and re-snapshot on the scripted server (Epic 9, Story 5)" -- polyoxide-kalshi/tests/supervision_predictions.rs
```

### Task 5.5: Wire agreement, the live suite and the records

**Files:**
- Modify: `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs:9-15`, and append after its last test
- Modify: `polyoxide-kalshi/tests/live_predictions_ws.rs:35-45`, and append after its last test
- Modify: `docs/specs/kalshi/OBSERVED.md` (above the `<!-- epic 9 findings go above this line -->` anchor, only for a finding of this story)
- Test: both test files

The wire suite pins the two new frame types to Story 1's captures and the premises the protocol rests on: the ticker channel carries no `seq`, the trade channel does, a requested snapshot carries the command's `id` and the subscription's own numbering, and the deltas after it continue from it. The mirror's required keys are checked against what the types write for the capture (the spec-agreement coverage of `tickerPayload` and `tradePayload`). The live suite subscribes all three channels, asks for a snapshot, and reads every channel's frames for keys the types do not model. Without `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` the live steps stop and ask the owner.

- [ ] **Step 1: Write the tests**

In `wire_agreement_predictions_ws.rs`, replace the imports (lines 9-15):

```rust
use polyoxide_kalshi::predictions::ws::frames::{
    Delta, Rejected, Snapshot, Subscribed, Unsubscribed, Updated,
};
use polyoxide_test_support::{agreement, fixtures, Fixtures};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};
```

with:

```rust
use std::collections::BTreeSet;

use polyoxide_kalshi::predictions::ws::frames::{
    Delta, Rejected, Snapshot, Subscribed, TickerFrame, TradeFrame, Unsubscribed, Updated,
};
use polyoxide_test_support::{agreement, fixtures, Fixtures};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

const ASYNCAPI: &str = include_str!("../../docs/specs/kalshi/asyncapi.yaml");

/// `(schema, key)`: keys the mirror requires of a payload's `msg` that the capture does not
/// carry, each an `Option` of its type and recorded in `docs/specs/kalshi/OBSERVED.md`. Empty
/// until the capture lacks a key the mirror requires; a developer who meets one adds its row.
const ABSENT_ON_THE_WIRE: &[(&str, &str)] = &[];

fn asyncapi() -> Value {
    let options = serde_saphyr::options! { budget: None };
    serde_saphyr::from_str_with_options(ASYNCAPI, options).expect("the mirror is YAML")
}

/// The keys the type writes in a frame's `msg`.
fn written_msg_keys<T: DeserializeOwned + Serialize>(text: &str) -> BTreeSet<String> {
    let frame: T = serde_json::from_str(text).expect("the fixture decodes");
    let written = serde_json::to_value(frame).expect("the type serialises");
    written["msg"]
        .as_object()
        .map(|msg| msg.keys().cloned().collect())
        .unwrap_or_default()
}
```

Append to `wire_agreement_predictions_ws.rs`, after `the_answers_carry_the_ids_and_sequences_the_protocol_reads`:

```rust
#[test]
fn the_ticker_fixture_agrees() {
    assert_agrees::<TickerFrame>("ticker", &ws().text("ticker"));
}

#[test]
fn the_trade_fixture_agrees() {
    assert_agrees::<TradeFrame>("trade", &ws().text("trade"));
}

#[test]
fn the_ticker_channel_carries_no_seq_and_the_trade_channel_does() {
    // The premise of the protocol's sequence check: a `seq` on a ticker frame would make
    // every one after the first a candidate gap, and none on a trade would leave its
    // subscription unchecked.
    let ticker = ws().json("ticker");
    assert!(ticker["sid"].is_u64(), "{ticker}");
    assert!(ticker.get("seq").is_none(), "{ticker}");
    let trade = ws().json("trade");
    assert!(trade["sid"].is_u64() && trade["seq"].is_u64(), "{trade}");
}

#[test]
fn the_subscribed_answers_of_the_other_channels_agree_and_name_their_channel() {
    // One `subscribe` naming three channels is answered by one `subscribed` for each; the
    // protocol keeps the `sid` of each by the channel the answer names.
    for (name, channel) in [("subscribed_ticker", "ticker"), ("subscribed_trade", "trade")] {
        assert_agrees::<Subscribed>(name, &ws().text(name));
        let answer: Subscribed = serde_json::from_str(&ws().text(name)).unwrap();
        assert_eq!(answer.msg.channel, channel, "{name}");
        assert!(answer.id.is_some(), "{name}: the answer carries the command's id");
    }
}

#[test]
fn the_requested_snapshot_agrees_and_carries_the_commands_id_and_the_subscriptions_numbering() {
    let text = ws().text("snapshot_answer");
    assert_agrees::<Snapshot>("snapshot_answer", &text);
    let answer: Snapshot = serde_json::from_str(&text).unwrap();
    // The re-snapshot recognises its answers by the id, and re-anchors on the `sid` and
    // `seq` they carry.
    assert!(answer.id.is_some(), "{answer:?}");
    // And it is on a subscription's numbering: `deltas_after_a_requested_snapshot_continue_its_numbering`
    // pins that the frames after it continue from this `seq`.
    assert!(answer.seq >= 1, "{answer:?}");
}

#[test]
fn deltas_after_a_requested_snapshot_continue_its_numbering() {
    // The premise of re-anchoring: after the snapshot that answered `get_snapshot`, the
    // subscription's next frame is the answer's `seq` plus one. If the capture shows
    // otherwise, the design (D9) does not hold and the story stops and asks the owner.
    let answer: Snapshot = serde_json::from_str(&ws().text("snapshot_answer")).unwrap();
    let deltas: Vec<Delta> = serde_json::from_str(&ws().text("deltas_after_snapshot")).unwrap();
    assert!(
        !deltas.is_empty(),
        "a capture with no delta after the snapshot proves nothing about the numbering"
    );
    let mut expected = answer.seq;
    for delta in &deltas {
        expected += 1;
        assert_eq!((delta.sid, delta.seq), (answer.sid, expected), "{delta:?}");
    }
    let frames = ws().json("deltas_after_snapshot");
    for (n, frame) in frames.as_array().expect("an array").iter().enumerate() {
        assert_agrees::<Delta>(&format!("delta after the snapshot {n}"), &frame.to_string());
    }
}

#[test]
fn the_error_for_a_snapshot_outside_the_subscription_answers_the_command() {
    // The protocol reads an error that carries a request's id as that request's answer.
    let text = ws().text("error_snapshot_outside");
    assert_agrees::<Rejected>("error_snapshot_outside", &text);
    let rejected: Rejected = serde_json::from_str(&text).unwrap();
    assert!(rejected.id.is_some(), "{rejected:?}");
}

#[test]
fn the_answer_to_an_update_naming_two_subscriptions_agrees_whatever_it_is() {
    // Story 1 records what Kalshi does with several `sids` in one command: one `ok`, one for
    // each, or an error. Whichever it is, the frames decode into the types the protocol
    // reads them as.
    let wire = ws().json("multi_sid_answer");
    let frames = match wire {
        Value::Array(frames) => frames,
        single => vec![single],
    };
    assert!(!frames.is_empty());
    for (n, frame) in frames.iter().enumerate() {
        let text = frame.to_string();
        match frame["type"].as_str() {
            Some("ok") => assert_agrees::<Updated>(&format!("multi_sid_answer {n}"), &text),
            Some("error") => assert_agrees::<Rejected>(&format!("multi_sid_answer {n}"), &text),
            other => panic!("multi_sid_answer {n}: a frame type the protocol does not read: {other:?}"),
        }
    }
}

#[test]
fn the_ticker_and_trade_types_write_every_key_the_asyncapi_requires_of_them() {
    // The spec-agreement coverage of `tickerPayload` and `tradePayload`: every key the mirror
    // requires of a `msg` is one the type writes for the capture, so the mirror and the types
    // cannot part without a test saying so. A key the capture lacks is an `Option` of its type,
    // excused in `ABSENT_ON_THE_WIRE` and recorded in `OBSERVED.md`.
    let spec = asyncapi();
    for (schema, written) in [
        (
            "tickerPayload",
            written_msg_keys::<TickerFrame>(&ws().text("ticker")),
        ),
        (
            "tradePayload",
            written_msg_keys::<TradeFrame>(&ws().text("trade")),
        ),
    ] {
        let required = spec["components"]["schemas"][schema]["properties"]["msg"]["required"]
            .as_array()
            .unwrap_or_else(|| panic!("{schema} lists no required keys in the mirror"));
        for key in required.iter().filter_map(Value::as_str) {
            if ABSENT_ON_THE_WIRE.contains(&(schema, key)) {
                continue;
            }
            assert!(
                written.contains(key),
                "{schema}: the mirror requires {key}, which the type does not write for the capture"
            );
        }
    }
}
```

In `live_predictions_ws.rs`, replace the imports (lines 35-45):

```rust
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        frames::{Delta, Rejected, Snapshot, Subscribed, Unsubscribed, Updated},
        signed_request, Event, PredictionsWsBuilder, PredictionsWsError, DEMO_WS,
    },
    Credentials, Predictions, DEMO,
};
use polyoxide_test_support::{agreement, load_env, transient, ResultExt};
```

with:

```rust
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        frames::{
            Delta, Rejected, Snapshot, Subscribed, TickerFrame, TradeFrame, Unsubscribed, Updated,
        },
        signed_request, Channel, Event, PredictionsMembership, PredictionsWsBuilder,
        PredictionsWsError, DEMO_WS,
    },
    Credentials, Predictions, DEMO,
};
use polyoxide_test_support::{agreement, environmental, load_env, transient, ResultExt};
```

Append to `live_predictions_ws.rs`, after `live_frames_carry_no_unmodelled_keys`:

```rust
/// As `unmodelled_keys`, for every channel the socket carries: the two it does not read are
/// `ticker` and `trade`.
fn unmodelled_keys_on_any_channel(text: &str) -> Option<Vec<String>> {
    if let Some(found) = unmodelled_keys(text) {
        return Some(found);
    }
    let kind = serde_json::from_str::<Value>(text)
        .ok()?
        .get("type")?
        .as_str()?
        .to_owned();
    let found = match kind.as_str() {
        "ticker" => agreement::compare::<TickerFrame>(&kind, text),
        "trade" => agreement::compare::<TradeFrame>(&kind, text),
        _ => return None,
    };
    Some(
        found
            .unmodelled
            .into_iter()
            .map(|path| format!("{kind}: {path}"))
            .collect(),
    )
}

#[tokio::test]
#[ignore]
async fn live_three_channels_deliver_a_ticker_frame() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let mut feed = PredictionsWsBuilder::new()
        .url(DEMO_WS)
        .or_fail("the demo socket URL")
        .channels(Channel::ALL.iter().copied())
        .connect(credentials, [ticker.clone()])
        .await
        .or_fail("connect to the demo socket on every channel");
    let deadline = Instant::now() + LISTEN;
    let mut update = None;
    while update.is_none() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            break;
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the stream before a ticker frame"))
            .or_fail("the supervised demo feed");
        if let Event::Ticker(found) = event {
            update = Some(found);
        }
    }
    let Some(update) = update else {
        environmental("no ticker frame within 30 s: the market was quiet");
    };
    assert_eq!(update.market_ticker, ticker.as_str());
    assert!(
        update.yes_bid <= update.yes_ask,
        "the ticker's bid {} is above its ask {}",
        update.yes_bid,
        update.yes_ask
    );
    feed.close().await.or_fail("close the demo feed");
}

#[tokio::test]
#[ignore]
async fn live_a_requested_snapshot_arrives() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let mut feed = PredictionsWsBuilder::new()
        .url(DEMO_WS)
        .or_fail("the demo socket URL")
        .connect(credentials, [ticker.clone()])
        .await
        .or_fail("connect to the demo socket");
    let membership = feed.membership();
    let deadline = Instant::now() + LISTEN;
    let mut snapshots = 0usize;
    let mut asked = false;
    while snapshots < 2 {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            break;
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the stream before a snapshot"))
            .or_fail("the supervised demo feed");
        if let Event::Snapshot(book) = event {
            assert_eq!(book.market_ticker, ticker.as_str());
            snapshots += 1;
            if !asked {
                // The first snapshot is the subscription's own; ask for a second.
                asked = true;
                membership
                    .snapshot([ticker.clone()])
                    .await
                    .or_fail("ask for a snapshot");
            }
        }
    }
    if snapshots < 2 {
        transient("no snapshot answered the request within 30 s");
    }
    feed.close().await.or_fail("close the demo feed");
}

#[tokio::test]
#[ignore]
async fn live_frames_of_every_channel_carry_no_unmodelled_keys() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let request = signed_request(&credentials, DEMO_WS).or_fail("sign the handshake");
    let socket = polyoxide_ws::connect(request, DEFAULT_CONNECT_TIMEOUT)
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("connect a raw socket through the kit");
    let mut connection = Connection::new(socket, Mode::Data);
    let subscribe = json!({"id": 1, "cmd": "subscribe", "params": {
        "channels": ["orderbook_delta", "ticker", "trade"],
        "market_tickers": [ticker.as_str()],
        "use_yes_price": true,
    }});
    connection
        .send(Message::Text(subscribe.to_string().into()))
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("send the subscribe");
    let deadline = Instant::now() + LISTEN;
    let mut frames = 0usize;
    let mut unmodelled = BTreeSet::new();
    while let Ok(next) = tokio::time::timeout_at(deadline, connection.next()).await {
        let message = next
            .unwrap_or_else(|| transient("the server ended the connection"))
            .map_err(PredictionsWsError::from)
            .or_fail("read a frame");
        let Message::Text(text) = message else {
            continue;
        };
        frames += 1;
        if let Some(found) = unmodelled_keys_on_any_channel(text.as_str()) {
            unmodelled.extend(found);
        }
    }
    assert!(frames > 0, "no frame arrived within 30 s");
    assert!(
        unmodelled.is_empty(),
        "the wire sent keys no frame type models: {unmodelled:?}"
    );
}
```

- [ ] **Step 2: Run them**

Run: `nix develop --command cargo test -p polyoxide-kalshi --features predictions-ws --test wire_agreement_predictions_ws && nix develop --command cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws`

Expected: the wire suite passes against Story 1's captures, and the live suite compiles with every test `ignored` (`0 passed; 0 failed; 5 ignored`: the two it had and the three added here).

A wire test that fails is a finding, as Step 3 says.

- [ ] **Step 3: Implement**

The tests are the deliverable. If a wire test fails, the capture and a type disagree, and the fix is the type, in `ws/event.rs`'s `frames`:

- a key the capture sends that the type lacks (the test names it under `unmodelled`): add it to `TickerMsg`, `TradeMsg` or the frame as an `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, with a doc line saying what it is;
- a key the type requires that the capture lacks (a `decode` error naming it): make the field an `Option` the same way, and where the mirror requires it, add `("tickerPayload", "<key>")` or `("tradePayload", "<key>")` to `ABSENT_ON_THE_WIRE`;
- a `seq` on a ticker frame, a trade frame with none, a requested snapshot with no `id`, or deltas after it that do not continue its numbering: the design (D9, D8) does not hold. Stop and ask the owner; do not change the protocol.

Each such change is a finding. Record it above the anchor in `docs/specs/kalshi/OBSERVED.md`: insert a subsection directly above the line `<!-- epic 9 findings go above this line -->`, headed `### The ticker and trade frames (Epic 9, Story 5)` with a `Captured <date>T<time>Z` line and one row of a table for each finding (the key, what the capture shows, what the type does). Where there is no finding, add nothing.

If `KALSHI_DEMO_API_KEY_ID` or `KALSHI_DEMO_PRIVATE_KEY` is absent from the environment, stop here and ask the owner: no fixture is ever written by hand, and Step 4's live run cannot start.

- [ ] **Step 4: Run it to see it pass, then the live suite and the story's gates**

Run `nix develop --command cargo fmt --all` first and keep what it does to this task's files (the snippets above are written in rustfmt's form, but it may wrap a long line another way).

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`

Expected: every test passes.

Run the live suite against the demo host: `nix develop --command cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws -- --ignored`

Expected: `test result: ok.` for the five tests, or, where the demo market is quiet, the ticker test ends as an `environmental` skip printing `polyoxide-class=environmental`. A failure that prints no tag, or `polyoxide-class=real`, is a finding: record it in `OBSERVED.md` above the anchor and, where it contradicts the design, ask the owner.

Run the story's gates:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'
bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh
```

Expected: each exits 0 and prints nothing but its own summary; the touch-set gate prints no path.

- [ ] **Step 5: Commit**

No new files.

```bash
git commit -m "test(kalshi): wire agreement for ticker, trade and the requested snapshot, and the live suite for all three channels (Epic 9, Story 5)" -- polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs polyoxide-kalshi/tests/live_predictions_ws.rs docs/specs/kalshi/OBSERVED.md
```

## Story 6: The local order book, `ws::book` (spec D10, §8)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-kalshi/src/predictions/ws/book.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`
- **Interfaces:** exposes "Story 6 → Stories 8 and 9"; uses Story 5's `Event`, `OrderbookSnapshot`, `OrderbookDelta` and module block, Epic 8's `ws/snapshot.json` and `ws/deltas.json`, and `crate::predictions::key::Outcome` and `crate::predictions::types::BookSide`
- **Verify:** `cargo test -p polyoxide-kalshi --all-features --lib book && cargo test -p polyoxide-kalshi --all-features && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** light (a pure data structure with literal-value tests)

Adds `Books` and `OrderBook`, a pure value the consumer feeds Story 5's events into: a snapshot replaces a ticker's YES-priced ladder, a delta adds its signed size and removes a level at zero or below, a delta for a ticker with no snapshot is ignored, `Resyncing` drops its tickers' books and `Reconnected` drops all. `levels(outcome)` gives one outcome's view, a NO bid at *q* being a YES ask at 1 − *q*. No I/O and no clock. Unit tests, in the file, replay Epic 8's captured `ws/snapshot.json` and `ws/deltas.json` and assert literal prices and sizes. Replaces `ws/mod.rs`'s module block with Story 6's. The code holds none of the profile's landmine markers. Note on the block: rustfmt orders consecutive `mod` items and consecutive `use` items alphabetically, so `pub mod book;` and `pub use book::{Books, OrderBook};` land first in their groups, not after `mod protocol;` and `pub use builder::PredictionsWsBuilder;`; the block below is the form `cargo fmt` leaves.

### Task 6.1: `OrderBook` and `Books`: snapshots and deltas

**Files:**
- Create: `polyoxide-kalshi/src/predictions/ws/book.rs`
- Modify: `polyoxide-kalshi/src/predictions/ws/mod.rs` (the module block, whole)
- Test: `polyoxide-kalshi/src/predictions/ws/book.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-kalshi/src/predictions/ws/book.rs` holding only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::predictions::ws::OrderbookDelta;

    fn level(price: &str, size: &str) -> Level {
        Level::new(price.parse().unwrap(), size.parse().unwrap())
    }

    fn ladder(pairs: &[(&str, &str)]) -> Vec<Level> {
        pairs.iter().map(|(price, size)| level(price, size)).collect()
    }

    fn snapshot(ticker: &str, bids: &[(&str, &str)], asks: &[(&str, &str)]) -> Event {
        Event::Snapshot(OrderbookSnapshot {
            market_ticker: ticker.to_owned(),
            market_id: "id".to_owned(),
            bids: ladder(bids),
            asks: ladder(asks),
        })
    }

    fn delta(ticker: &str, side: BookSide, price: &str, delta: &str) -> Event {
        Event::Delta(OrderbookDelta {
            market_ticker: ticker.to_owned(),
            side,
            price: price.parse().unwrap(),
            delta: delta.parse().unwrap(),
            ts: None,
            client_order_id: None,
        })
    }

    #[test]
    fn a_snapshot_builds_both_sides_best_first() {
        let mut books = Books::new();
        // Listed worst first on purpose: the book sorts, whatever order it is sent in.
        let snap = snapshot(
            "A-1",
            &[("0.40", "10.00"), ("0.45", "5.00")],
            &[("0.60", "3.00"), ("0.55", "7.00")],
        );
        let book = books.apply(&snap).unwrap();
        assert_eq!(book.market_ticker(), "A-1");
        assert_eq!(book.bids(), ladder(&[("0.45", "5.00"), ("0.40", "10.00")]));
        assert_eq!(book.asks(), ladder(&[("0.55", "7.00"), ("0.60", "3.00")]));
        assert_eq!(book.best_bid(), Some(level("0.45", "5.00")));
        assert_eq!(book.best_ask(), Some(level("0.55", "7.00")));
        assert_eq!(books.get("A-1").unwrap().best_bid(), Some(level("0.45", "5.00")));
    }

    #[test]
    fn a_second_snapshot_replaces_the_book() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[("0.60", "3.00")]));
        let book = books
            .apply(&snapshot("A-1", &[("0.30", "1.00")], &[("0.70", "2.00")]))
            .unwrap();
        // Nothing of the first book survives: not its bid, not its ask.
        assert_eq!(book.bids(), ladder(&[("0.30", "1.00")]));
        assert_eq!(book.asks(), ladder(&[("0.70", "2.00")]));
    }

    #[test]
    fn a_snapshot_keeps_no_empty_level_and_may_hold_no_side() {
        let mut books = Books::new();
        let book = books
            .apply(&snapshot("A-1", &[("0.40", "0.00")], &[]))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
        assert_eq!(book.asks(), Vec::new());
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn a_delta_adds_to_a_level_and_opens_a_new_one() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[("0.60", "3.00")]));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "2.50"))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.40", "12.50")]));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.42", "4.00"))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.42", "4.00"), ("0.40", "12.50")]));
        // An ask is the other side of the ladder: a level of its own, not a bid.
        let book = books
            .apply(&delta("A-1", BookSide::Ask, "0.60", "-1.00"))
            .unwrap();
        assert_eq!(book.asks(), ladder(&[("0.60", "2.00")]));
        assert_eq!(book.bids(), ladder(&[("0.42", "4.00"), ("0.40", "12.50")]));
    }

    #[test]
    fn a_level_reaching_zero_or_below_is_removed() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[]));
        // Down to six, then to exactly zero: the level goes, not a zero-size row.
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "-4.00"))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.40", "6.00")]));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "-6.00"))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
        // A change that takes a level below zero removes it too, and one on a level the
        // book never had opens nothing.
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.30", "-5.00"))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
        books.apply(&delta("A-1", BookSide::Bid, "0.30", "3.00"));
        let book = books
            .apply(&delta("A-1", BookSide::Bid, "0.30", "-10.00"))
            .unwrap();
        assert_eq!(book.bids(), Vec::new());
    }

    #[test]
    fn a_delta_for_a_ticker_with_no_snapshot_is_ignored() {
        let mut books = Books::new();
        assert!(books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "2.50"))
            .is_none());
        assert!(books.get("A-1").is_none());
        // It does not leave a half-built book for a later snapshot to inherit.
        let book = books
            .apply(&snapshot("A-1", &[("0.45", "1.00")], &[]))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.45", "1.00")]));
    }

    #[test]
    fn books_are_kept_per_ticker() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[]));
        books.apply(&snapshot("B-2", &[("0.10", "1.00")], &[]));
        books.apply(&delta("A-1", BookSide::Bid, "0.40", "5.00"));
        assert_eq!(
            books.get("A-1").unwrap().bids(),
            ladder(&[("0.40", "15.00")])
        );
        assert_eq!(books.get("B-2").unwrap().bids(), ladder(&[("0.10", "1.00")]));
    }

    #[test]
    fn a_delta_on_a_side_it_cannot_place_changes_nothing() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[("0.60", "3.00")]));
        let before = books.get("A-1").cloned();
        let sideways = delta("A-1", BookSide::Other("mid".to_owned()), "0.50", "9.00");
        assert!(books.apply(&sideways).is_none());
        assert_eq!(books.get("A-1").cloned(), before);
    }

    #[test]
    fn the_yes_view_is_the_ladder_as_kept() {
        let mut books = Books::new();
        let book = books
            .apply(&snapshot(
                "A-1",
                &[("0.40", "10.00"), ("0.45", "5.00")],
                &[("0.55", "7.00"), ("0.60", "3.00")],
            ))
            .unwrap();
        let (bids, asks) = book.levels(Outcome::Yes);
        assert_eq!(bids, ladder(&[("0.45", "5.00"), ("0.40", "10.00")]));
        assert_eq!(asks, ladder(&[("0.55", "7.00"), ("0.60", "3.00")]));
    }

    #[test]
    fn the_no_view_mirrors_every_price_at_one_minus() {
        let mut books = Books::new();
        let book = books
            .apply(&snapshot(
                "A-1",
                &[("0.40", "10.00"), ("0.45", "5.00")],
                &[("0.55", "7.00"), ("0.60", "3.00")],
            ))
            .unwrap();
        let (bids, asks) = book.levels(Outcome::No);
        // A YES ask at 0.55 is a NO bid at 0.45, and the lowest YES ask is the best NO bid.
        assert_eq!(bids, ladder(&[("0.45", "7.00"), ("0.40", "3.00")]));
        // A YES bid at 0.45 is a NO ask at 0.55, and the highest YES bid is the best NO ask.
        assert_eq!(asks, ladder(&[("0.55", "5.00"), ("0.60", "10.00")]));
    }
}
```

Then replace `ws/mod.rs`'s module block. Quote the block as Story 5 left it (its items after the module docs) and replace it, in `polyoxide-kalshi/src/predictions/ws/mod.rs`:

Current (Story 5's block):

```rust
mod builder;
mod error;
mod event;
mod protocol;
#[cfg(test)]
mod scripted;

pub use builder::PredictionsWsBuilder;
pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{
    DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot, PublicTrade, TickerUpdate,
};
#[doc(hidden)]
pub use protocol::signed_request;
pub use protocol::{Change, Channel, PredictionsMembership, PredictionsProtocol};
```

New:

```rust
pub mod book;
mod builder;
mod error;
mod event;
mod protocol;
#[cfg(test)]
mod scripted;

pub use book::{Books, OrderBook};
pub use builder::PredictionsWsBuilder;
pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{
    DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot, PublicTrade, TickerUpdate,
};
#[doc(hidden)]
pub use protocol::signed_request;
pub use protocol::{Change, Channel, PredictionsMembership, PredictionsProtocol};
```

Leave the module docs above the block, and the `SupervisedPredictionsWs` alias and the two URL consts below it, as they are.

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib book`

Expected: the build fails, `error[E0432]: unresolved imports` for `book::Books` and `book::OrderBook` in `ws/mod.rs`, and in `book.rs` `error[E0433]`/`E0412` (`cannot find type `Level` in this scope`, `cannot find type `Books``, `cannot find type `Event``, `cannot find value `Outcome``). No test runs.

- [ ] **Step 3: Implement**

Insert this directly above the `#[cfg(test)]` line of `polyoxide-kalshi/src/predictions/ws/book.rs`, so the file is the implementation followed by the tests:

```rust
//! A local order book, kept from the socket's events.
//!
//! [`Books`] is a plain value the consumer feeds each [`Event`] the supervised socket
//! yields, through [`Books::apply`]. It does no I/O and has no clock: it keeps one
//! [`OrderBook`] per market ticker, built only from the snapshots the current
//! subscription sent, and changes it by the deltas that follow.
//!
//! A book is kept in YES pricing, as the socket sends it (the subscription asks for
//! `use_yes_price`): the bids are YES offers to buy and the asks are the NO bids, priced
//! as the YES offers to sell they are. [`OrderBook::levels`] turns the ladder over for the
//! NO outcome, where a NO bid at *q* is a YES ask at 1 - *q*, as the REST book is read.
//!
//! The rules are the order-book channel's: a snapshot replaces a market's book, a delta
//! adds its signed size to one level and removes a level whose size reaches zero or
//! below, and a delta for a market with no snapshot is ignored. [`Event::Resyncing`]
//! drops the books of the markets it names, whose deltas stay ignored until each one's
//! fresh snapshot arrives, and [`Event::Reconnected`] drops every book, so no book ever
//! carries a delta onto a snapshot from another connection. Between
//! [`Event::Disconnected`] and [`Event::Reconnected`] the books stay as they were, and
//! are stale.

use std::collections::{BTreeMap, HashMap};

use polyoxide_venue::Level;
use rust_decimal::Decimal;

use crate::predictions::{
    key::Outcome,
    types::BookSide,
    ws::{Event, OrderbookSnapshot},
};

/// One market's book: its YES-priced ladder, price to resting size.
///
/// Built and changed only by [`Books::apply`]; every size in it is above zero.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBook {
    market_ticker: String,
    bids: BTreeMap<Decimal, Decimal>,
    asks: BTreeMap<Decimal, Decimal>,
}

fn level_of((price, size): (&Decimal, &Decimal)) -> Level {
    Level::new(*price, *size)
}

fn ladder_of(levels: &[Level]) -> BTreeMap<Decimal, Decimal> {
    levels
        .iter()
        .filter(|level| level.size > Decimal::ZERO)
        .map(|level| (level.price, level.size))
        .collect()
}

/// Adds `delta` to the size at `price`, and removes the level when the size is not above
/// zero. A level the ladder does not hold starts from zero, so a change that only takes
/// away opens nothing.
fn add(ladder: &mut BTreeMap<Decimal, Decimal>, price: Decimal, delta: Decimal) {
    let size = ladder.get(&price).copied().unwrap_or(Decimal::ZERO) + delta;
    if size > Decimal::ZERO {
        ladder.insert(price, size);
    } else {
        ladder.remove(&price);
    }
}

impl OrderBook {
    fn from_snapshot(snapshot: &OrderbookSnapshot) -> Self {
        Self {
            market_ticker: snapshot.market_ticker.clone(),
            bids: ladder_of(&snapshot.bids),
            asks: ladder_of(&snapshot.asks),
        }
    }

    /// The market's ticker.
    pub fn market_ticker(&self) -> &str {
        &self.market_ticker
    }

    /// The highest YES bid, when the book has one.
    pub fn best_bid(&self) -> Option<Level> {
        self.bids.iter().next_back().map(level_of)
    }

    /// The lowest YES ask, when the book has one.
    pub fn best_ask(&self) -> Option<Level> {
        self.asks.iter().next().map(level_of)
    }

    /// Every YES bid, highest price first.
    pub fn bids(&self) -> Vec<Level> {
        self.bids.iter().rev().map(level_of).collect()
    }

    /// Every YES ask, lowest price first: the NO bids, in YES pricing.
    pub fn asks(&self) -> Vec<Level> {
        self.asks.iter().map(level_of).collect()
    }

    /// One outcome's bids and asks, as `(bids, asks)`, each best first: its bids from the
    /// highest price, and its asks from the lowest.
    ///
    /// For the YES outcome this is the ladder as kept. For NO every price turns over: a
    /// YES ask at *p* is a NO bid at 1 - *p*, and a YES bid at *p* is a NO ask at 1 - *p*.
    pub fn levels(&self, outcome: Outcome) -> (Vec<Level>, Vec<Level>) {
        match outcome {
            Outcome::Yes => (self.bids(), self.asks()),
            Outcome::No => {
                let mirrored = |(price, size): (&Decimal, &Decimal)| {
                    Level::new(Decimal::ONE - *price, *size)
                };
                // The lowest YES ask is the highest NO bid, and the highest YES bid is
                // the lowest NO ask, so the YES ladders are walked in the order that
                // puts the NO levels best first.
                let bids = self.asks.iter().map(mirrored).collect();
                let asks = self.bids.iter().rev().map(mirrored).collect();
                (bids, asks)
            }
        }
    }
}

/// The books of every market a socket carries, kept from its events.
#[derive(Debug, Default, Clone)]
pub struct Books {
    books: HashMap<String, OrderBook>,
}

impl Books {
    /// No books.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event, and returns the book it changed.
    ///
    /// A snapshot replaces its market's book. A delta changes the book of a market that
    /// has a snapshot, and is ignored (`None`) for one that has none, and for a side that
    /// is neither a bid nor an ask. Every other event changes no book and returns `None`.
    pub fn apply(&mut self, event: &Event) -> Option<&OrderBook> {
        match event {
            Event::Snapshot(snapshot) => {
                let entry = self
                    .books
                    .entry(snapshot.market_ticker.clone())
                    .insert_entry(OrderBook::from_snapshot(snapshot));
                Some(&*entry.into_mut())
            }
            Event::Delta(delta) => {
                let book = self.books.get_mut(&delta.market_ticker)?;
                let side = match &delta.side {
                    BookSide::Bid => &mut book.bids,
                    BookSide::Ask => &mut book.asks,
                    _ => return None,
                };
                add(side, delta.price, delta.delta);
                Some(&*book)
            }
            // The events that carry no book: the ticker and trade channels, and the
            // outage marker. The book-dropping events are Task 6.2's.
            _ => None,
        }
    }

    /// The book of `market_ticker`, when it has a snapshot.
    pub fn get(&self, market_ticker: &str) -> Option<&OrderBook> {
        self.books.get(market_ticker)
    }
}

```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib book`

Expected: `test result: ok. 10 passed; 0 failed` (the ten tests of Step 1, all under `predictions::ws::book::tests`).

Then prove two tests catch a break, restoring each edit at once. In `add`, change `size > Decimal::ZERO` to `size >= Decimal::ZERO`: `a_level_reaching_zero_or_below_is_removed` fails (a zero-size level stays). In `OrderBook::levels`, change `Decimal::ONE - *price` to `*price`: `the_no_view_mirrors_every_price_at_one_minus` fails. Run the story's own gates for the package:

`nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`

Expected: no output from fmt, `Finished` from clippy and doc with no warning.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/src/predictions/ws/book.rs
git commit -m "feat(kalshi): a local order book kept from snapshots and deltas (Epic 9, Story 6)" -- polyoxide-kalshi/src/predictions/ws/book.rs polyoxide-kalshi/src/predictions/ws/mod.rs
```

### Task 6.2: `Resyncing` and `Reconnected` drop books

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/ws/book.rs` (the `_ => None` arm of `Books::apply`, and the end of the test module)
- Test: `polyoxide-kalshi/src/predictions/ws/book.rs`

- [ ] **Step 1: Write the failing test**

Append these tests inside `mod tests`, directly after the last test (`the_no_view_mirrors_every_price_at_one_minus`), before the module's closing brace, and add `use std::time::Duration;` and `use crate::predictions::ws::DisconnectReason;` to the module's `use` lines (below `use crate::predictions::ws::OrderbookDelta;`; rustfmt orders them):

```rust
    #[test]
    fn resyncing_drops_the_books_it_names_and_only_those() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[("0.60", "3.00")]));
        books.apply(&snapshot("B-2", &[("0.10", "1.00")], &[]));
        let resync = Event::Resyncing {
            tickers: vec!["A-1".to_owned()],
        };
        assert!(books.apply(&resync).is_none());
        assert!(books.get("A-1").is_none());
        assert_eq!(books.get("B-2").unwrap().bids(), ladder(&[("0.10", "1.00")]));
        // Its deltas are ignored until its fresh snapshot arrives, so none lands on the
        // dropped book's levels...
        assert!(books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "5.00"))
            .is_none());
        // ...and the snapshot builds the book anew, with nothing of the old one.
        let book = books
            .apply(&snapshot("A-1", &[("0.41", "2.00")], &[]))
            .unwrap();
        assert_eq!(book.bids(), ladder(&[("0.41", "2.00")]));
        assert_eq!(book.asks(), Vec::new());
    }

    #[test]
    fn resyncing_names_a_ticker_with_no_book_without_harm() {
        let mut books = Books::new();
        books.apply(&snapshot("B-2", &[("0.10", "1.00")], &[]));
        let resync = Event::Resyncing {
            tickers: vec!["A-1".to_owned()],
        };
        assert!(books.apply(&resync).is_none());
        assert!(books.get("B-2").is_some());
    }

    #[test]
    fn a_reconnect_drops_every_book() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[]));
        books.apply(&snapshot("B-2", &[("0.10", "1.00")], &[]));
        assert!(books.apply(&Event::Reconnected).is_none());
        assert!(books.get("A-1").is_none());
        assert!(books.get("B-2").is_none());
        // A delta from the new connection that precedes its snapshot is ignored.
        assert!(books
            .apply(&delta("A-1", BookSide::Bid, "0.40", "1.00"))
            .is_none());
    }

    #[test]
    fn an_outage_marker_alone_keeps_the_books() {
        let mut books = Books::new();
        books.apply(&snapshot("A-1", &[("0.40", "10.00")], &[]));
        let outage = Event::Disconnected {
            reason: DisconnectReason::Stale {
                silent: Duration::from_secs(30),
            },
        };
        assert!(books.apply(&outage).is_none());
        assert_eq!(books.get("A-1").unwrap().bids(), ladder(&[("0.40", "10.00")]));
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib book`

Expected: the build passes and two tests fail: `resyncing_drops_the_books_it_names_and_only_those` at `assert!(books.get("A-1").is_none())` and `a_reconnect_drops_every_book` at `assert!(books.get("A-1").is_none())`, since `apply` returns `None` for both events and drops nothing. `resyncing_names_a_ticker_with_no_book_without_harm` and `an_outage_marker_alone_keeps_the_books` pass already, and stay as guards on the arms below. `test result: FAILED. 12 passed; 2 failed`.

- [ ] **Step 3: Implement**

In `Books::apply`, replace the catch-all arm and its comment

```rust
            // The events that carry no book: the ticker and trade channels, and the
            // outage marker. The book-dropping events are Task 6.2's.
            _ => None,
```

with

```rust
            Event::Resyncing { tickers } => {
                for ticker in tickers {
                    self.books.remove(ticker);
                }
                None
            }
            Event::Reconnected => {
                self.books.clear();
                None
            }
            // The events that carry no book: the ticker and trade channels, and the
            // outage marker.
            _ => None,
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib book`

Expected: `test result: ok. 14 passed; 0 failed`.

Then the gates: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`. Expected: no output from fmt, `Finished` from clippy with no warning.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(kalshi): Resyncing drops its tickers' books and Reconnected drops all (Epic 9, Story 6)" -- polyoxide-kalshi/src/predictions/ws/book.rs
```

### Task 6.3: Replay the captured snapshot and deltas

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/ws/book.rs` (the end of the test module)
- Test: `polyoxide-kalshi/src/predictions/ws/book.rs`

The fixtures are Epic 8's capture (`tests/fixtures/predictions/ws/snapshot.json`, one frame, and `deltas.json`, 44 frames in wire order, both for `KXBTCD-26OCT2317-T81999.99` on the demo host, see that directory's `PROVENANCE.md`). The expected ladders below are the arithmetic of those frames, summed per level, not read back from the code under test.

- [ ] **Step 1: Write the failing test**

Add `use polyoxide_test_support::fixtures;`, `use serde_json::Value;` and `frames` to the `crate::predictions::ws` import (`use crate::predictions::ws::{frames, DisconnectReason, OrderbookDelta};`) in `mod tests`, then append these directly after the last test of Task 6.2:

```rust
    const CAPTURED: &str = "KXBTCD-26OCT2317-T81999.99";

    fn captured_snapshot() -> Event {
        let frame = fixtures!("predictions/ws").json("snapshot");
        let msg: frames::SnapshotMsg = serde_json::from_value(frame["msg"].clone()).unwrap();
        Event::Snapshot(OrderbookSnapshot::from(msg))
    }

    fn captured_deltas() -> Vec<Event> {
        let frames_on_wire: Value = fixtures!("predictions/ws").json("deltas");
        frames_on_wire
            .as_array()
            .unwrap()
            .iter()
            .map(|frame| {
                let msg: frames::DeltaMsg = serde_json::from_value(frame["msg"].clone()).unwrap();
                Event::Delta(OrderbookDelta::from(msg))
            })
            .collect()
    }

    fn assert_ladders(book: &OrderBook, bids: &[(&str, &str)], asks: &[(&str, &str)]) {
        assert_eq!(book.bids(), ladder(bids));
        assert_eq!(book.asks(), ladder(asks));
    }

    #[test]
    fn the_captured_snapshot_is_the_ladder_its_frame_lists() {
        let mut books = Books::new();
        let book = books.apply(&captured_snapshot()).unwrap();
        assert_eq!(book.market_ticker(), CAPTURED);
        assert_ladders(
            book,
            &[
                ("0.4500", "19999.98"),
                ("0.4400", "145830.30"),
                ("0.4300", "149222.15"),
                ("0.4200", "96030.44"),
                ("0.0100", "24580.00"),
            ],
            &[
                ("0.4600", "86177.72"),
                ("0.4700", "136521.00"),
                ("0.4800", "76041.18"),
                ("0.4900", "25849.88"),
                ("0.5000", "25333.08"),
            ],
        );
        assert_eq!(book.best_bid(), Some(level("0.45", "19999.98")));
        assert_eq!(book.best_ask(), Some(level("0.46", "86177.72")));
    }

    #[test]
    fn replaying_the_captured_deltas_gives_the_ladder_they_sum_to() {
        let deltas = captured_deltas();
        assert_eq!(deltas.len(), 44);
        // Every captured delta is for the snapshot's market, so none of them is ignored.
        assert!(deltas.iter().all(|event| matches!(
            event,
            Event::Delta(delta) if delta.market_ticker == CAPTURED
        )));

        let mut books = Books::new();
        books.apply(&captured_snapshot());
        for (index, event) in deltas.iter().enumerate() {
            let book = books.apply(event).unwrap();
            // The book never crosses: the best bid stays under the best ask.
            assert!(
                book.best_bid().unwrap().price < book.best_ask().unwrap().price,
                "crossed after delta {index}"
            );
            match index + 1 {
                // The first four deltas (seq 2 to 5) take 716.91 off the 0.46 ask.
                4 => assert_ladders(
                    book,
                    &[
                        ("0.4500", "19999.98"),
                        ("0.4400", "145830.30"),
                        ("0.4300", "149222.15"),
                        ("0.4200", "96030.44"),
                        ("0.0100", "24580.00"),
                    ],
                    &[
                        ("0.4600", "85460.81"),
                        ("0.4700", "136521.00"),
                        ("0.4800", "76041.18"),
                        ("0.4900", "25849.88"),
                        ("0.5000", "25333.08"),
                    ],
                ),
                // Four more (seq 6 to 9) add to the 0.45 bid and the 0.48 to 0.50 asks.
                8 => assert_ladders(
                    book,
                    &[
                        ("0.4500", "20370.35"),
                        ("0.4400", "145830.30"),
                        ("0.4300", "149222.15"),
                        ("0.4200", "96030.44"),
                        ("0.0100", "24580.00"),
                    ],
                    &[
                        ("0.4600", "85460.81"),
                        ("0.4700", "136521.00"),
                        ("0.4800", "76388.40"),
                        ("0.4900", "26190.01"),
                        ("0.5000", "25666.41"),
                    ],
                ),
                _ => {}
            }
        }

        let book = books.get(CAPTURED).unwrap();
        assert_ladders(
            book,
            &[
                ("0.4500", "21111.09"),
                ("0.4400", "145830.30"),
                ("0.4300", "149222.15"),
                ("0.4200", "96030.44"),
                ("0.0100", "24560.00"),
            ],
            &[
                ("0.4600", "84026.99"),
                ("0.4700", "136521.00"),
                ("0.4800", "77082.84"),
                ("0.4900", "26870.27"),
                ("0.5000", "26333.07"),
            ],
        );
        // The NO outcome's view of the same book: every price at 1 minus, best first.
        let (no_bids, no_asks) = book.levels(Outcome::No);
        assert_eq!(
            no_bids,
            ladder(&[
                ("0.54", "84026.99"),
                ("0.53", "136521.00"),
                ("0.52", "77082.84"),
                ("0.51", "26870.27"),
                ("0.50", "26333.07"),
            ])
        );
        assert_eq!(
            no_asks,
            ladder(&[
                ("0.55", "21111.09"),
                ("0.56", "145830.30"),
                ("0.57", "149222.15"),
                ("0.58", "96030.44"),
                ("0.99", "24560.00"),
            ])
        );
    }

    #[test]
    fn a_reconnect_then_the_same_snapshot_rebuilds_the_captured_book() {
        let mut books = Books::new();
        books.apply(&captured_snapshot());
        for event in captured_deltas() {
            books.apply(&event);
        }
        books.apply(&Event::Reconnected);
        assert!(books.get(CAPTURED).is_none());
        // Deltas that arrive before the new snapshot are dropped, and the snapshot alone
        // gives back the book it lists: none of the 44 deltas is carried over.
        for event in captured_deltas() {
            assert!(books.apply(&event).is_none());
        }
        let book = books.apply(&captured_snapshot()).unwrap();
        assert_eq!(book.best_ask(), Some(level("0.46", "86177.72")));
        assert_eq!(book.best_bid(), Some(level("0.45", "19999.98")));
    }
```

- [ ] **Step 2: Run it to see it pass at once**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib book`

Expected: `test result: ok. 17 passed; 0 failed`. Tasks 6.1 and 6.2 already built the behaviour, so these pass at once; they pin it to the capture. Prove they bite: in `add`, change `ladder.insert(price, size)` to `ladder.insert(price, delta)` and the unit tests and the replay test fail (a level then holds the delta, not the sum); in `Books::apply`'s `Event::Reconnected` arm, comment out `self.books.clear();` and `a_reconnect_then_the_same_snapshot_rebuilds_the_captured_book` fails at `assert!(books.get(CAPTURED).is_none())`. Restore both edits.

- [ ] **Step 3: Implement**

Nothing to implement: the behaviour is Tasks 6.1 and 6.2's. This task adds only the replay tests of Step 1.

- [ ] **Step 4: Run the story's gates**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib book && nix develop --command cargo test -p polyoxide-kalshi --all-features && nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features' && nix develop --command python3 scripts/gen_registry.py --check && nix develop --command python3 scripts/venue_gate.py && nix develop --command python3 scripts/live_unwraps.py && nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0 && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`

Expected: every command exits 0; the touch-set gate prints nothing (it admits `polyoxide-kalshi/`).

- [ ] **Step 5: Commit**

```bash
git commit -m "test(kalshi): the local book replays the captured snapshot and deltas (Epic 9, Story 6)" -- polyoxide-kalshi/src/predictions/ws/book.rs
```

## Story 7: Measured read limits: the unsigned soak and the signed-bucket probe (spec D11, §9)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/tests/mock_predictions_signed.rs`, `docs/specs/kalshi/OBSERVED.md`, `Cargo.lock`
- **Interfaces:** exposes "Story 7 → Story 9"; uses Story 1's anchor and the `[package.metadata.polyoxide]` anchor
- **Verify:** `cargo build -p polyoxide-kalshi --features predictions --example read_soak && cargo test -p polyoxide-kalshi --no-default-features --features predictions && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** deep (RESERVED_FRACTION: a rate-limit sizing measured against the live host)

Writes `polyoxide-kalshi/examples/read_soak.rs` on test-support's soak helpers (a ramp of the `market` and `event` routes over distinct URLs with each path probed on the host first, a validation that paces the shipped client by its own throttle, and a signed probe on the demo host with a private-route control), runs it on production and on demo, records every run in a `### Rate limits` subsection of `OBSERVED.md`'s Epic 9 section (the Epic 8 `## Rate limits` section above it stays as the record of what Kalshi publishes), and sizes `throttle.rs` from what it found. The stages are requests a second: Basic's Read bucket refills 200 tokens a second, which is 20 reads at the default cost of 10, so the default ramp runs from half of that to twelve times it, past the spec's 200. **The code holds the landmine marker `RESERVED_FRACTION`** (the unsigned client's refill is the measured clean rate less a tenth, a private copy of the constant core and Binance carry, which must stay a margin below what the host served and never become the measured rate itself), so the supervisor should dispatch its developer with `effort: xhigh`. Two live results decide code, so the code that depends on a result is written for both outcomes and the developer applies the one the record shows: Task 7.5 sizes the unsigned client (branch A when the host refused at or below Basic's 20 reads a second, which replaces Basic's sizing with the measured clean rate less a tenth; branch B when Basic's rate was served clean, which confirms Basic's sizing, whether the host refused above it or never refused; the sizing never rises above Basic's) and Task 7.6 either leaves a keyed client's public reads on the key's bucket (the signed probe followed it) or moves them to a public bucket sized as an unsigned client's (it did not). The move edits two existing tests, `a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse` and `the_bucket_takes_the_capacity_the_account_reports`, which spec §13 forbids changing except by the owner's third exception (2026-10-21), which pre-grants exactly this: if the probe moves public reads, that branch rewrites both to charge a signed account route (`GET /account/limits`), so they still pin the key's bucket, and appends the move's own tests. `request.rs` and `mod.rs` are not in this story's files, so the split lives in `throttle.rs` alone: the throttle picks a bucket from the request's path. The story needs network access to `external-api.kalshi.com` and, for Task 7.4, `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`; it stops and asks at a step that lacks either. Story 9 closes the two `deferred-work.md` entries this story settles.

### Task 7.1: The harness's pure core, and its manifest wiring

**Files:**
- Modify: `polyoxide-kalshi/Cargo.toml:58-61`
- Modify: `polyoxide-kalshi/Cargo.toml:146`
- Create: `polyoxide-kalshi/examples/read_soak.rs`
- Test: `polyoxide-kalshi/examples/read_soak.rs`

The harness's arguments, URL supply, stage arithmetic and the signed probe's verdict are pure, so they are written and pinned first; the drivers that send requests are Task 7.2. The example is a target with `test = true`, as polymarket's `info_soak` is, because `cargo test` builds examples but does not run their unit tests unless asked.

- [ ] **Step 1: Write the failing tests**

First the manifest. The dev-dependency gains test-support's `soak` feature, which holds the pacer, the throttle observer and the verdict rules.

Current (lines 58-61):

```toml
# The tests' query and header helpers, the failure tags and the credential
# loaders. Path-only: it is never published, and cargo strips a path-only
# dev-dependency on publish.
polyoxide-test-support = { path = "../polyoxide-test-support", features = ["query"] }
```

Replace with:

```toml
# The tests' query and header helpers, the failure tags and the credential
# loaders, and `examples/read_soak.rs`'s pacer, throttle observer and verdicts (`soak`).
# Path-only: it is never published, and cargo strips a path-only dev-dependency on publish.
polyoxide-test-support = { path = "../polyoxide-test-support", features = ["query", "soak"] }
```

Then the example's stanza, directly before the registration comment (the `[package.metadata.polyoxide]` anchor). Current (line 146):

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
# The unsigned soak and the signed-bucket probe that sized `src/predictions/throttle.rs` (Epic 9,
# Story 7). `cargo test` builds examples but does not run their unit tests unless asked, so the
# harness's own checks run only because `test = true` is set here.
[[example]]
name = "read_soak"
path = "examples/read_soak.rs"
test = true
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Then create `polyoxide-kalshi/examples/read_soak.rs` holding only the tests; Step 3 writes the harness above them.

```rust
//! The harness's tests come first; the next step writes the harness above them.

fn main() {}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use polyoxide_test_support::fixtures;
    use ring::signature::{UnparsedPublicKey, ED25519};

    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn config(list: &[&str]) -> Config {
        Config::from_args(args(list))
            .expect("the arguments parse")
            .expect("not a request for help")
    }

    fn refused(list: &[&str]) -> String {
        Config::from_args(args(list)).expect_err("the arguments were refused")
    }

    fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(ticker, event)| ((*ticker).to_owned(), (*event).to_owned()))
            .collect()
    }

    fn ids(count: usize) -> Vec<String> {
        (1..=count).map(|n| format!("T{n}")).collect()
    }

    // ── arguments ───────────────────────────────────────────────

    #[test]
    fn a_ramp_takes_exactly_one_route_and_a_validation_takes_a_list() {
        assert!(refused(&["--routes", "market,event"]).contains("one route at a time"));
        let ramp = config(&["--routes", "event"]);
        assert_eq!((ramp.mode, ramp.routes), (Mode::Ramp, vec![Route::Event]));

        let validation = config(&["--routes", "all", "--pace", "client"]);
        assert_eq!(validation.mode, Mode::Validate);
        assert_eq!(validation.routes, vec![Route::Market, Route::Event]);
        assert_eq!(validation.stage_secs, 120);
        assert_eq!(validation.concurrency, 4);
        assert_eq!(ramp.stage_secs, 60);
        assert_eq!(ramp.concurrency, 48);
    }

    #[test]
    fn a_run_names_its_routes_unless_it_is_the_signed_probe() {
        assert!(refused(&[]).contains("--routes is required"));
        assert!(refused(&["--routes", "orderbook"]).contains("unknown route"));
        let signed = config(&["--signed"]);
        assert_eq!(signed.mode, Mode::Signed);
        assert!(signed.routes.is_empty());
        assert!(refused(&["--signed", "--routes", "market"]).contains("--signed takes no"));
        assert!(refused(&["--signed", "--stages", "10"]).contains("--signed takes no"));
        assert!(refused(&["--signed", "--pace", "client"]).contains("--signed takes no"));
    }

    #[test]
    fn stages_belong_to_a_ramp_and_must_ascend_under_the_ceiling() {
        assert!(
            refused(&["--routes", "market", "--pace", "client", "--stages", "10"])
                .contains("--stages is a ramp's")
        );
        assert!(parse_stages("10,5").is_err());
        assert!(parse_stages("10,301").is_err());
        assert_eq!(parse_stages("5,10").unwrap(), vec![5.0, 10.0]);
        assert_eq!(config(&["--routes", "market"]).stages, DEFAULT_STAGES);
    }

    #[test]
    fn the_default_ramp_starts_below_basics_twenty_reads_and_ends_past_two_hundred() {
        // Basic's 200 tokens a second are 20 reads at 10 tokens, and the spec's 200 is the
        // figure the ramp must reach.
        let stages = config(&["--routes", "market"]).stages;
        assert!(stages.first().is_some_and(|first| *first < 20.0));
        assert!(stages
            .last()
            .is_some_and(|last| *last > 200.0 && *last <= CEILING_RPS));
        assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_zero_count_is_refused_so_a_run_cannot_pass_with_no_workers() {
        for flag in ["--concurrency", "--stage-secs", "--pages"] {
            let message = refused(&["--routes", "market", flag, "0"]);
            assert!(message.contains("at least 1"), "{flag}: {message}");
        }
        let message = refused(&[
            "--routes",
            "all",
            "--pace",
            "client",
            "--max-concurrent",
            "0",
        ]);
        assert!(message.contains("at least 1"), "{message}");
    }

    #[test]
    fn the_client_cap_is_a_validations_and_a_help_request_runs_nothing() {
        assert!(refused(&["--routes", "market", "--max-concurrent", "8"])
            .contains("--max-concurrent is for --pace client"));
        let validation = config(&[
            "--routes",
            "market",
            "--pace",
            "client",
            "--max-concurrent",
            "8",
        ]);
        assert_eq!(validation.max_concurrent, Some(8));
        assert!(Config::from_args(args(&["--help"])).unwrap().is_none());
        assert!(refused(&["--nope"]).contains("unknown argument"));
        assert!(refused(&["--routes"]).contains("requires a value"));
        assert!(refused(&["--pace", "server"]).contains("takes only `client`"));
    }

    #[test]
    fn a_base_url_loses_its_trailing_slash() {
        let config = config(&["--routes", "market", "--base-url", "http://127.0.0.1:9/"]);
        assert_eq!(config.base_url.as_deref(), Some("http://127.0.0.1:9"));
    }

    // ── URLs ────────────────────────────────────────────────────

    #[test]
    fn a_route_target_is_the_hosts_url_and_the_path_a_signature_covers() {
        let market = Route::Market.target("https://host.test", "KXA-1");
        assert_eq!(market.url, "https://host.test/trade-api/v2/markets/KXA-1");
        assert_eq!(market.path, "/trade-api/v2/markets/KXA-1");
        let event = Route::Event.target("https://host.test", "KXA");
        assert_eq!(event.url, "https://host.test/trade-api/v2/events/KXA");
        assert_eq!(event.path, "/trade-api/v2/events/KXA");
    }

    #[test]
    fn a_listing_keeps_each_ticker_and_event_once_in_order_and_drops_what_cannot_be_sent() {
        let listing = Listing::from_rows(rows(&[
            ("KXA-1", "KXA"),
            ("KXA-2", "KXA"),
            ("KXA-1", "KXA"),
            // A lowercase ticker is a 404 on the host, which is case-sensitive.
            ("kxb-1", "KXB"),
            // A space cannot sit in a URL path.
            ("KXC-1", "KX C"),
            ("KXD.1", "KXD"),
            // Dots alone are dot segments.
            ("KXE-1", ".."),
        ]));
        assert_eq!(
            listing.tickers,
            ["KXA-1", "KXA-2", "KXC-1", "KXD.1", "KXE-1"]
        );
        assert_eq!(listing.events, ["KXA", "KXB", "KXD"]);
        assert_eq!(listing.ids(Route::Market), listing.tickers.as_slice());
        assert_eq!(listing.ids(Route::Event), listing.events.as_slice());
    }

    #[test]
    fn a_pool_hands_out_each_id_once_and_refuses_a_stage_it_cannot_fill() {
        let mut pool = Pool::new(&ids(5));
        assert_eq!(pool.draw().as_deref(), Some("T1"));
        assert_eq!(pool.take(2), Ok(vec!["T2".to_owned(), "T3".to_owned()]));

        let short = pool.take(3).unwrap_err();
        assert!(
            short.contains("3 more distinct URLs") && short.contains("left 2"),
            "{short}"
        );
        // The refusal handed out nothing.
        assert_eq!(pool.take(2), Ok(vec!["T4".to_owned(), "T5".to_owned()]));
        assert_eq!(pool.draw(), None);
        assert_eq!(pool.take(0), Ok(vec![]));
    }

    #[test]
    fn a_stage_needs_a_url_for_every_slot_and_one_more() {
        assert_eq!(stage_urls(40.0, 60), 2401);
        assert_eq!(stage_urls(0.5, 1), 2);
        assert_eq!(stage_urls(10.0, 1), 11);
    }

    #[test]
    fn the_highest_clean_stage_stops_at_the_first_that_was_not() {
        let throttled = Verdict::Throttled {
            after: Duration::from_secs(3),
            code: "too many requests".into(),
        };
        let stages = vec![
            (10.0, Verdict::Clean),
            (20.0, Verdict::Clean),
            (40.0, throttled),
            (80.0, Verdict::Clean),
        ];
        assert_eq!(highest_clean(&stages), Some(20.0));
        assert_eq!(
            highest_clean(&[(10.0, Verdict::Invalid { errors: 1 })]),
            None
        );
        assert_eq!(
            highest_clean(&[(10.0, Verdict::UnderDriven { achieved: 4.0 })]),
            None
        );
        assert_eq!(highest_clean(&[]), None);
    }

    #[test]
    fn the_first_throttle_is_when_the_first_429_arrived() {
        let throttled = |at| {
            (
                Duration::from_secs(at),
                Reply::Throttled {
                    code: "x".into(),
                    retry_after: None,
                },
            )
        };
        let replies = vec![
            (Duration::from_secs(1), Reply::Ok),
            throttled(4),
            throttled(5),
        ];
        assert_eq!(first_throttle(&replies), Some(Duration::from_secs(4)));
        assert_eq!(first_throttle(&replies[..1]), None);
        assert_eq!(first_throttle(&[]), None);
    }

    // ── the signed probe ────────────────────────────────────────

    #[test]
    fn a_drain_runs_at_twice_the_refills_reads_and_empties_the_bucket_in_capacity_over_refill() {
        // Basic: 600 tokens refilling 200 a second, 20 reads a second at 10 tokens. Twice that
        // is 40 reads a second, 400 tokens, and 200 of them a second come out of the bucket.
        assert_eq!(
            Drain::new(600, 200, 10),
            Ok(Drain {
                rate: 40.0,
                model: Duration::from_secs(3),
                window: MIN_WINDOW,
            })
        );
        // A bucket that empties in a second still waits the minimum window.
        assert_eq!(
            Drain::new(1200, 1200, 10),
            Ok(Drain {
                rate: 240.0,
                model: Duration::from_secs(1),
                window: MIN_WINDOW,
            })
        );
        // The ceiling caps the rate, and the bucket then empties more slowly.
        assert_eq!(
            Drain::new(6000, 2000, 10),
            Ok(Drain {
                rate: 300.0,
                model: Duration::from_secs(6),
                window: Duration::from_secs(18),
            })
        );
    }

    #[test]
    fn a_drain_waits_at_most_the_maximum_window_and_refuses_a_bucket_it_cannot_empty() {
        let slow = Drain::new(60_000, 1000, 10).unwrap();
        assert_eq!(slow.window, MAX_WINDOW);
        for (capacity, refill, cost) in [(0, 200, 10), (600, 0, 10), (600, 200, 0)] {
            assert!(
                Drain::new(capacity, refill, cost)
                    .unwrap_err()
                    .contains("cannot be probed"),
                "{capacity} {refill} {cost}"
            );
        }
        // At 300 reads of 10 tokens a second the harness spends 3000 tokens a second, which does
        // not outrun a refill of 3000.
        let message = Drain::new(600, 3000, 10).unwrap_err();
        assert!(message.contains("cannot be drained"), "{message}");
        assert!(Drain::new(600, 4000, 10).is_err());
    }

    #[test]
    fn a_throttle_is_on_the_model_between_half_its_time_and_twice_it_with_a_second() {
        let model = Duration::from_secs(3);
        let at = |millis| timing(model, Duration::from_millis(millis));
        assert_eq!(at(0), Timing::Early);
        assert_eq!(at(1499), Timing::Early);
        assert_eq!(at(1500), Timing::OnModel);
        assert_eq!(at(3000), Timing::OnModel);
        assert_eq!(at(7000), Timing::OnModel);
        assert_eq!(at(7001), Timing::Late);
    }

    #[test]
    fn the_verdict_needs_a_control_that_throttled_where_the_keys_bucket_says() {
        let model = Duration::from_secs(3);
        let on = Some(Duration::from_secs(3));
        let early = Some(Duration::from_secs(1));
        let late = Some(Duration::from_secs(20));
        let inconclusive =
            |verdict: SignedVerdict| matches!(verdict, SignedVerdict::Inconclusive(_));

        // A quiet control proves the host did not enforce the bucket at this rate, and a control
        // anywhere but the model proves the model wrong. Neither lets a public read speak.
        assert!(inconclusive(signed_verdict(model, None, model, None)));
        assert!(inconclusive(signed_verdict(model, None, model, on)));
        assert!(inconclusive(signed_verdict(model, early, model, None)));
        assert!(inconclusive(signed_verdict(model, late, model, None)));

        // With a good control, the public read decides.
        assert_eq!(signed_verdict(model, on, model, on), SignedVerdict::Follows);
        assert_eq!(
            signed_verdict(model, on, model, None),
            SignedVerdict::DoesNotFollow
        );
        assert_eq!(
            signed_verdict(model, on, model, early),
            SignedVerdict::DoesNotFollow
        );
        assert!(inconclusive(signed_verdict(model, on, model, late)));
    }

    #[test]
    fn the_verdict_judges_each_phase_against_its_own_model() {
        // The public route costs less, so its bucket empties later: a throttle at 8 s is on its
        // model of 6 s, and would be late judged against the control's 3 s.
        let control = Duration::from_secs(3);
        let public = Duration::from_secs(6);
        let eight = Some(Duration::from_secs(8));
        assert_eq!(
            signed_verdict(control, Some(control), public, eight),
            SignedVerdict::Follows
        );
        assert!(matches!(
            signed_verdict(control, Some(control), control, eight),
            SignedVerdict::Inconclusive(_)
        ));
    }

    /// The signing vectors' Ed25519 key, a throwaway one, as the PEM Kalshi issues.
    pub(super) fn throwaway_key() -> Credentials {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let body = vectors["ed25519"]["pkcs8_body"]
            .as_str()
            .expect("the vectors hold an Ed25519 key body");
        let label = "PRIVATE KEY";
        let pem = format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n");
        Credentials::from_pem("test-key", &pem).expect("the throwaway key parses")
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("the vectors hold hex"))
            .collect()
    }

    #[test]
    fn a_signed_request_carries_the_key_the_time_and_a_signature_over_time_get_path() {
        let path = "/trade-api/v2/markets/KXA-1";
        let headers = signed_headers(&throwaway_key(), path, 1_700_000_000_000).unwrap();

        let names: Vec<&str> = headers.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "KALSHI-ACCESS-KEY",
                "KALSHI-ACCESS-TIMESTAMP",
                "KALSHI-ACCESS-SIGNATURE"
            ]
        );
        assert_eq!(headers[0].1, "test-key");
        assert_eq!(headers[1].1, "1700000000000");
        // The signature verifies under the vectors' public key over the message Kalshi checks.
        let vectors = fixtures!("predictions/signing").json("vectors");
        let public_key = hex(vectors["ed25519"]["public_key_hex"]
            .as_str()
            .expect("the vectors hold the public key"));
        let signature = STANDARD
            .decode(&headers[2].1)
            .expect("the signature is base64");
        let verified = UnparsedPublicKey::new(&ED25519, &public_key)
            .verify(b"1700000000000GET/trade-api/v2/markets/KXA-1", &signature);
        assert!(verified.is_ok(), "the signature does not verify");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --example read_soak`

Expected: the example fails to compile and no test runs. The errors are `error[E0433]: failed to resolve: use of undeclared type` for `Config`, `Route`, `Mode`, `Listing`, `Pool`, `Drain`, `Timing`, `SignedVerdict`, `Verdict` and `Reply`, `error[E0425]: cannot find function` for `parse_stages`, `stage_urls`, `highest_clean`, `first_throttle`, `timing`, `signed_verdict` and `signed_headers`, and `error[E0412]: cannot find type` for `Credentials`.

- [ ] **Step 3: Implement**

Replace the file's first three lines with the harness's pure core. The new text ends with a stub `main`, which Task 7.2 replaces; `#![allow(dead_code)]` is a temporary stub for the same reason, since nothing calls the items yet, and Task 7.2 drops it.

Current (lines 1-3):

```rust
//! The harness's tests come first; the next step writes the harness above them.

fn main() {}
```

Replace with:

```rust
//! Measures what Kalshi's production host serves an unsigned client, validates the shipped client's
//! sizing against it, and probes which Read bucket a signed public read draws on.
//!
//! Kalshi publishes no limit for unauthenticated requests, so the Predictions client sizes its
//! unsigned Read bucket from this harness's measurement. A ramp steps one route up through fixed
//! rates, each for `--stage-secs`, until the host refuses one; the highest clean rate is what
//! `src/predictions/throttle.rs` sizes from, less a tenth. The stages are requests a second:
//! Basic's Read bucket refills 200 tokens a second, which is 20 reads at the default cost of 10.
//!
//! ```sh
//! # Ramp one route on production, unsigned.
//! cargo run --release -p polyoxide-kalshi --features predictions --example read_soak -- \
//!     --routes market --stages 10,20,40,80,160,240
//!
//! # Validate the shipped client: pace both routes by its own throttle and require zero 429s.
//! cargo run --release -p polyoxide-kalshi --features predictions --example read_soak -- \
//!     --routes all --pace client --concurrency 32 --max-concurrent 32
//!
//! # Probe the demo host with the demo key: does a signed public read draw on the key's bucket?
//! cargo run --release -p polyoxide-kalshi --features predictions --example read_soak -- --signed
//! ```
//!
//! Every URL of a run is distinct, because a repeated URL can be answered from a cache without
//! reaching the origin, and a route is probed once before any stage so a run never measures a path
//! the host does not serve. The ramp's rules are `polyoxide_test_support::soak::verdict::strict`,
//! unit-tested there.
#![allow(dead_code)] // Task 7.2 calls everything below from `main` and drops this line.

use std::{collections::HashSet, time::Duration};

use polyoxide_kalshi::predictions::{key::Ticker, Credentials};
use polyoxide_test_support::soak::{
    self,
    verdict::strict::{Reply, Verdict},
};

const DEFAULT_STAGES: [f64; 6] = [10.0, 20.0, 40.0, 80.0, 160.0, 240.0];
/// The most requests a second a stage or a probe may ask for.
const CEILING_RPS: f64 = 300.0;
/// Pages of the open listing a walk reads, of `LISTING_LIMIT` markets each.
const DEFAULT_PAGES: usize = 40;
const LISTING_LIMIT: u32 = 1000;

/// The shortest and longest a phase of the signed probe waits for a `429`.
const MIN_WINDOW: Duration = Duration::from_secs(10);
const MAX_WINDOW: Duration = Duration::from_secs(60);
/// A throttle earlier than this share of the time the key's bucket takes to empty is not the
/// key's bucket.
const EARLIEST_SHARE: f64 = 0.5;
/// A throttle later than this multiple of that time, and the slack, is not the key's bucket
/// either.
const LATEST_FACTOR: f64 = 2.0;
const SLACK: Duration = Duration::from_secs(1);
/// What the bucket is given to refill between the signed probe's phases, beyond a full refill.
const REFILL_MARGIN: Duration = Duration::from_secs(5);

/// The templates the signed probe looks up in `GET /account/endpoint_costs`.
const PUBLIC_TEMPLATE: &str = "/trade-api/v2/markets/:ticker";
const PRIVATE_TEMPLATE: &str = "/trade-api/v2/account/limits";

// ── Routes and URLs ─────────────────────────────────────────────

/// A public route the harness reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Market,
    Event,
}

/// One request: where it goes, and the path a signature covers.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    url: String,
    path: String,
}

impl Route {
    const ALL: [Route; 2] = [Route::Market, Route::Event];

    fn name(self) -> &'static str {
        match self {
            Route::Market => "market",
            Route::Event => "event",
        }
    }

    /// The request for one ticker, or one event ticker, on `base`.
    fn target(self, base: &str, id: &str) -> Target {
        let path = match self {
            Route::Market => format!("/trade-api/v2/markets/{id}"),
            Route::Event => format!("/trade-api/v2/events/{id}"),
        };
        Target {
            url: format!("{base}{path}"),
            path,
        }
    }
}

fn parse_routes(raw: &str) -> Result<Vec<Route>, String> {
    soak::parse_routes(raw, &Route::ALL, Route::name)
}

fn parse_stages(raw: &str) -> Result<Vec<f64>, String> {
    soak::parse_stages(raw, CEILING_RPS)
}

/// Whether `text` can sit in a URL path as it is, and is not made of dots alone.
fn path_safe(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
        && text.bytes().any(|byte| byte.is_ascii_alphanumeric())
}

/// The tickers and event tickers a listing walk found, each once, in listing order.
#[derive(Debug, Default, PartialEq, Eq)]
struct Listing {
    tickers: Vec<String>,
    events: Vec<String>,
}

impl Listing {
    /// Keeps the first sight of each `(ticker, event ticker)` row's ticker and event ticker, and
    /// drops a ticker the client would not send as it is listed, and an event ticker that would
    /// not sit in a URL path.
    fn from_rows(rows: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut listing = Self::default();
        let mut tickers = HashSet::new();
        let mut events = HashSet::new();
        for (ticker, event) in rows {
            let canonical = Ticker::new(&ticker).is_ok_and(|parsed| parsed.as_str() == ticker);
            if canonical && tickers.insert(ticker.clone()) {
                listing.tickers.push(ticker);
            }
            if path_safe(&event) && events.insert(event.clone()) {
                listing.events.push(event);
            }
        }
        listing
    }

    /// The ids `route` is read for.
    fn ids(&self, route: Route) -> &[String] {
        match route {
            Route::Market => &self.tickers,
            Route::Event => &self.events,
        }
    }
}

/// Ids handed out once each, so no URL of a run repeats.
#[derive(Debug)]
struct Pool {
    ids: Vec<String>,
    next: usize,
}

impl Pool {
    fn new(ids: &[String]) -> Self {
        Self {
            ids: ids.to_vec(),
            next: 0,
        }
    }

    /// The next id, or `None` once every id has gone out.
    fn draw(&mut self) -> Option<String> {
        let id = self.ids.get(self.next)?.clone();
        self.next += 1;
        Some(id)
    }

    /// The next `count` ids, or an error that hands out none when fewer are left.
    fn take(&mut self, count: usize) -> Result<Vec<String>, String> {
        let left = self.ids.len() - self.next;
        if left < count {
            return Err(format!(
                "{count} more distinct URLs are needed and the listing walk left {left}; lower \
                 --stage-secs or the stage rate, or raise --pages"
            ));
        }
        let taken = self.ids[self.next..self.next + count].to_vec();
        self.next += count;
        Ok(taken)
    }
}

/// The distinct URLs a stage of `secs` seconds at `rate` a second can send: one for each slot,
/// and one more for a slot that lands on the deadline.
fn stage_urls(rate: f64, secs: u64) -> usize {
    (rate * secs as f64).ceil() as usize + 1
}

/// The rate of the last stage before the first one that was not clean, and `None` when the first
/// was not.
fn highest_clean(stages: &[(f64, Verdict)]) -> Option<f64> {
    stages
        .iter()
        .take_while(|(_, verdict)| *verdict == Verdict::Clean)
        .last()
        .map(|(rate, _)| *rate)
}

/// When the first `429` arrived, from the start of the phase.
fn first_throttle(replies: &[(Duration, Reply)]) -> Option<Duration> {
    replies
        .iter()
        .find(|(_, reply)| matches!(reply, Reply::Throttled { .. }))
        .map(|(at, _)| *at)
}

// ── The signed probe ────────────────────────────────────────────

/// One phase of the signed probe: reads sent faster than a bucket refills.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drain {
    /// Requests a second the phase sends.
    rate: f64,
    /// When a bucket that starts full empties at that rate, from the first request.
    model: Duration,
    /// How long the phase sends before it stops waiting for a `429`.
    window: Duration,
}

impl Drain {
    /// A phase against a bucket of `capacity` tokens refilling `refill` a second, for reads of
    /// `cost` tokens: twice the rate the refill sustains, so the bucket empties after
    /// `capacity / refill` seconds, unless the harness's ceiling is lower.
    fn new(capacity: u32, refill: u32, cost: u32) -> Result<Self, String> {
        if capacity == 0 || refill == 0 || cost == 0 {
            return Err(format!(
                "a bucket of {capacity} tokens refilling {refill} a second, read at {cost} tokens, \
                 cannot be probed"
            ));
        }
        let (capacity, refill, cost) = (f64::from(capacity), f64::from(refill), f64::from(cost));
        let rate = (2.0 * refill / cost).min(CEILING_RPS);
        let net = rate * cost - refill;
        if net <= 0.0 {
            return Err(format!(
                "{CEILING_RPS} requests a second at {cost} tokens a read do not outrun a refill of \
                 {refill} tokens a second, so the bucket cannot be drained"
            ));
        }
        let model = Duration::from_secs_f64(capacity / net);
        Ok(Self {
            rate,
            model,
            window: (model * 3).clamp(MIN_WINDOW, MAX_WINDOW),
        })
    }
}

/// Where a first `429` falls against the time the key's bucket takes to empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Timing {
    Early,
    OnModel,
    Late,
}

fn timing(model: Duration, observed: Duration) -> Timing {
    if observed < model.mul_f64(EARLIEST_SHARE) {
        Timing::Early
    } else if observed > model.mul_f64(LATEST_FACTOR) + SLACK {
        Timing::Late
    } else {
        Timing::OnModel
    }
}

/// What the signed probe found.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SignedVerdict {
    /// The public read throttled where the key's bucket empties: it draws on the key's bucket.
    Follows,
    /// The public read did not throttle there, or throttled well before: it does not.
    DoesNotFollow,
    /// The control did not behave as the key's bucket predicts, or the public read throttled well
    /// after it: nothing is concluded.
    Inconclusive(String),
}

/// Judges the two phases. The control is a route that needs the key and so certainly draws on its
/// bucket; a public read is judged only when the control throttled where the model says.
fn signed_verdict(
    control_model: Duration,
    control: Option<Duration>,
    public_model: Duration,
    public: Option<Duration>,
) -> SignedVerdict {
    let Some(control) = control else {
        return SignedVerdict::Inconclusive(
            "the control never throttled, so the host did not enforce the key's bucket at this \
             rate and a quiet public read proves nothing"
                .into(),
        );
    };
    if timing(control_model, control) != Timing::OnModel {
        return SignedVerdict::Inconclusive(format!(
            "the control throttled at {control:?}, not where a bucket that empties after \
             {control_model:?} puts it"
        ));
    }
    match public.map(|at| (at, timing(public_model, at))) {
        None | Some((_, Timing::Early)) => SignedVerdict::DoesNotFollow,
        Some((_, Timing::OnModel)) => SignedVerdict::Follows,
        Some((at, Timing::Late)) => SignedVerdict::Inconclusive(format!(
            "the public read throttled at {at:?}, long after the {public_model:?} the key's bucket \
             takes to empty"
        )),
    }
}

/// The three headers Kalshi signs a request with: the key id, `now_ms`, and the signature of
/// `now_ms + "GET" + path`, with `path` as sent and no query.
fn signed_headers(
    credentials: &Credentials,
    path: &str,
    now_ms: u128,
) -> Result<[(&'static str, String); 3], String> {
    let timestamp = now_ms.to_string();
    let signature = credentials
        .sign_message(format!("{timestamp}GET{path}").as_bytes())
        .map_err(|error| error.to_string())?;
    Ok([
        ("KALSHI-ACCESS-KEY", credentials.key_id().to_owned()),
        ("KALSHI-ACCESS-TIMESTAMP", timestamp),
        ("KALSHI-ACCESS-SIGNATURE", signature),
    ])
}

// ── Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ramp,
    Validate,
    Signed,
}

#[derive(Debug, Clone)]
struct Config {
    mode: Mode,
    routes: Vec<Route>,
    stages: Vec<f64>,
    stage_secs: u64,
    cooldown_secs: u64,
    concurrency: usize,
    max_concurrent: Option<usize>,
    pages: usize,
    base_url: Option<String>,
}

const USAGE: &str = "\
Measure what Kalshi's production host serves an unsigned client, validate the shipped client
against it, or probe which Read bucket a signed public read draws on.

Usage: read_soak --routes <routes> [options]
       read_soak --routes <routes> --pace client [options]
       read_soak --signed [options]

  --routes <routes>       market | event. A ramp takes one; --pace client takes a comma list
                          or `all`
  --stages <r1,r2,...>    Ramp rates in requests a second, ascending, at most 300
                          (default: 10,20,40,80,160,240)
  --pace client           Validate instead: pace by the shipped client's own throttle and
                          require zero 429s
  --signed                Probe the demo host with KALSHI_DEMO_API_KEY_ID and
                          KALSHI_DEMO_PRIVATE_KEY: does a signed public read draw on the key's
                          Read bucket?
  --stage-secs <n>        Seconds per stage (default: 60 ramp, 120 validation)
  --cooldown-secs <n>     Idle seconds between ramp stages (default: 30)
  --concurrency <n>       Workers (default: 48 ramp and signed, 4 per route validation)
  --max-concurrent <n>    Validation: the client's in-flight cap (default: the client's own)
  --pages <n>             Pages of 1000 markets the listing walk reads (default: 40)
  --base-url <url>        Override the host (default: production, the demo host with --signed)
  -h, --help              Show this message";

impl Config {
    fn from_args(mut args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut routes = None;
        let mut stages = None;
        let mut client_paced = false;
        let mut signed = false;
        let mut stage_secs = None;
        let mut cooldown_secs = 30;
        let mut concurrency = None;
        let mut max_concurrent = None;
        let mut pages = DEFAULT_PAGES;
        let mut base_url = None;
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{flag} requires a value"))
            };
            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--routes" => routes = Some(parse_routes(&value()?)?),
                "--stages" => stages = Some(parse_stages(&value()?)?),
                "--pace" => match value()?.as_str() {
                    "client" => client_paced = true,
                    other => return Err(format!("--pace takes only `client`, got {other:?}")),
                },
                "--signed" => signed = true,
                "--stage-secs" => {
                    stage_secs = Some(value()?.parse().map_err(|_| "bad --stage-secs")?)
                }
                "--cooldown-secs" => {
                    cooldown_secs = value()?.parse().map_err(|_| "bad --cooldown-secs")?
                }
                "--concurrency" => {
                    concurrency = Some(value()?.parse().map_err(|_| "bad --concurrency")?)
                }
                "--max-concurrent" => {
                    max_concurrent = Some(value()?.parse().map_err(|_| "bad --max-concurrent")?)
                }
                "--pages" => pages = value()?.parse().map_err(|_| "bad --pages")?,
                "--base-url" => base_url = Some(value()?.trim_end_matches('/').to_owned()),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        let mode = if signed {
            Mode::Signed
        } else if client_paced {
            Mode::Validate
        } else {
            Mode::Ramp
        };
        match mode {
            Mode::Signed => {
                if routes.is_some() || stages.is_some() || client_paced {
                    return Err("--signed takes no --routes, --stages or --pace".into());
                }
            }
            Mode::Validate | Mode::Ramp => {
                let listed = routes.as_ref().ok_or("--routes is required")?;
                if mode == Mode::Ramp && listed.len() != 1 {
                    return Err(
                        "a ramp measures one route at a time; list several only with --pace client"
                            .into(),
                    );
                }
                if mode == Mode::Validate && stages.is_some() {
                    return Err("--stages is a ramp's; --pace client takes none".into());
                }
            }
        }
        if max_concurrent.is_some() && mode != Mode::Validate {
            return Err("--max-concurrent is for --pace client".into());
        }
        if concurrency == Some(0)
            || stage_secs == Some(0)
            || max_concurrent == Some(0)
            || pages == 0
        {
            return Err(
                "--concurrency, --stage-secs, --max-concurrent and --pages must be at least 1"
                    .into(),
            );
        }
        Ok(Some(Self {
            mode,
            routes: routes.unwrap_or_default(),
            stages: stages.unwrap_or_else(|| DEFAULT_STAGES.to_vec()),
            stage_secs: stage_secs.unwrap_or(if mode == Mode::Validate { 120 } else { 60 }),
            cooldown_secs,
            concurrency: concurrency.unwrap_or(if mode == Mode::Validate { 4 } else { 48 }),
            max_concurrent,
            pages,
            base_url,
        }))
    }
}

fn main() {} // Task 7.2 replaces this with the harness's drivers and `main`.
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --example read_soak`

Expected: `running 19 tests`, each `ok`, and `test result: ok. 19 passed; 0 failed`.

Then the gates this task can fail:

- `nix develop --command cargo fmt --all -- --check` prints nothing.
- `nix develop --command cargo clippy -p polyoxide-kalshi --no-default-features --features predictions --all-targets -- -D warnings` prints no warning.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/examples/read_soak.rs
git commit -m "feat(kalshi): read_soak's arguments, URL supply and the signed probe's verdict (Epic 9, Story 7)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/examples/read_soak.rs
```

### Task 7.2: The drivers and `main`

**Files:**
- Modify: `polyoxide-kalshi/examples/read_soak.rs:27-35`
- Modify: `polyoxide-kalshi/examples/read_soak.rs:476`
- Test: `polyoxide-kalshi/examples/read_soak.rs`

The drivers send the requests: a paced multi-worker sender that stops at the first `429`, the listing walk, the one-request probe of each route, the ramp, the validation through the shipped client, and the signed probe. Its tests run them against a mock host, so the pacing, the stop at a `429`, the signature headers, the walk's query and its paging are pinned without the live host.

- [ ] **Step 1: Write the failing tests**

Append this module at the end of `polyoxide-kalshi/examples/read_soak.rs`, after the `tests` module.

```rust
#[cfg(test)]
mod drive_tests {
    use std::sync::atomic::AtomicUsize;

    use mockito::{Matcher, Mock, Server, ServerGuard};
    use polyoxide_test_support::fixtures;

    use super::tests::throwaway_key;
    use super::*;

    /// What the mock host saw of one request.
    #[derive(Debug, Clone)]
    struct Seen {
        path_and_query: String,
        key: Option<String>,
        timestamp: Option<String>,
        signature: Option<String>,
    }

    #[derive(Clone, Default)]
    struct Log(Arc<Mutex<Vec<Seen>>>);

    impl Log {
        fn all(&self) -> Vec<Seen> {
            lock(&self.0).clone()
        }
    }

    fn ids(count: usize) -> Vec<String> {
        (1..=count).map(|n| format!("T{n}")).collect()
    }

    /// A host that records every GET, answers with `status_of` the request's number counted from
    /// zero, and sends the body `body_of` makes of the request.
    async fn host(
        log: &Log,
        status_of: impl Fn(usize) -> usize + Send + Sync + 'static,
        body_of: impl Fn(&mockito::Request) -> String + Send + Sync + 'static,
    ) -> (ServerGuard, Mock) {
        let mut server = Server::new_async().await;
        let served = AtomicUsize::new(0);
        let recorder = log.clone();
        let mock = server
            .mock("GET", Matcher::Any)
            .match_query(Matcher::Any)
            .with_status_code_from_request(move |request| {
                let header = |name: &str| {
                    request
                        .header(name)
                        .first()
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned)
                };
                lock(&recorder.0).push(Seen {
                    path_and_query: request.path_and_query().to_owned(),
                    key: header("kalshi-access-key"),
                    timestamp: header("kalshi-access-timestamp"),
                    signature: header("kalshi-access-signature"),
                });
                status_of(served.fetch_add(1, Ordering::SeqCst))
            })
            .with_header("content-type", "application/json")
            .with_body_from_request(move |request| body_of(request).into_bytes())
            .create_async()
            .await;
        (server, mock)
    }

    #[tokio::test]
    async fn a_stage_over_distinct_urls_gets_only_origin_replies_and_sends_no_key() {
        let log = Log::default();
        let (server, _mock) = host(&log, |_| 200, |_| "{}".to_owned()).await;
        let base = server.url();
        let targets: Vec<Target> = ids(12)
            .iter()
            .map(|id| Route::Market.target(&base, id))
            .collect();
        let http = http_client().unwrap();

        let replies = drive(&http, targets, None, 20.0, 5, 2).await;

        assert_eq!(replies.len(), 12);
        assert!(replies.iter().all(|(_, reply)| *reply == Reply::Ok));
        let seen = log.all();
        let mut paths: Vec<&str> = seen
            .iter()
            .map(|request| request.path_and_query.as_str())
            .collect();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), 12, "a URL went out twice: {paths:?}");
        assert!(paths
            .iter()
            .all(|path| path.starts_with("/trade-api/v2/markets/T")));
        // An unsigned drive carries none of Kalshi's headers.
        assert!(seen
            .iter()
            .all(|request| request.key.is_none() && request.signature.is_none()));
    }

    #[tokio::test]
    async fn the_first_429_ends_the_stage_and_is_its_verdict() {
        let log = Log::default();
        // Kalshi's documented 429 body.
        let (server, _mock) = host(
            &log,
            |n| if n < 3 { 200 } else { 429 },
            |_| r#"{"error":"too many requests"}"#.to_owned(),
        )
        .await;
        let base = server.url();
        let targets: Vec<Target> = ids(20)
            .iter()
            .map(|id| Route::Market.target(&base, id))
            .collect();
        let http = http_client().unwrap();

        // One worker, so nothing is in flight behind the 429 when the stage ends.
        let replies = drive(&http, targets, None, 50.0, 5, 1).await;

        assert_eq!(replies.len(), 4);
        match judge(&replies, 50.0, 5) {
            Verdict::Throttled { code, .. } => assert_eq!(code, "too many requests"),
            other => panic!("expected a throttled stage, got {other:?}"),
        }
        assert!(first_throttle(&replies).is_some());
    }

    #[tokio::test]
    async fn a_signed_drive_sends_the_three_headers_for_the_path_it_was_given() {
        let log = Log::default();
        let (server, _mock) = host(&log, |_| 200, |_| "{}".to_owned()).await;
        let target = Route::Market.target(&server.url(), "KXA-1");
        let http = http_client().unwrap();

        let replies = drive(
            &http,
            vec![target],
            Some(Arc::new(throwaway_key())),
            10.0,
            5,
            1,
        )
        .await;

        assert_eq!(replies.len(), 1);
        let seen = log.all();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path_and_query, "/trade-api/v2/markets/KXA-1");
        assert_eq!(seen[0].key.as_deref(), Some("test-key"));
        let timestamp = seen[0].timestamp.as_deref().expect("a timestamp was sent");
        assert!(
            timestamp.len() == 13 && timestamp.bytes().all(|byte| byte.is_ascii_digit()),
            "{timestamp}"
        );
        assert!(seen[0]
            .signature
            .as_deref()
            .is_some_and(|signature| !signature.is_empty()));
    }

    #[tokio::test]
    async fn a_probe_names_the_route_that_does_not_answer_2xx() {
        let log = Log::default();
        let (missing, _missing_mock) = host(
            &log,
            |_| 404,
            |_| r#"{"error":{"code":"not_found","message":"not found"}}"#.to_owned(),
        )
        .await;
        let (found, _found_mock) = host(&log, |_| 200, |_| "{}".to_owned()).await;
        let http = http_client().unwrap();

        let refusal = probe(
            &http,
            "event",
            &Route::Event.target(&missing.url(), "KXA"),
            None,
        )
        .await
        .unwrap_err();
        assert!(
            refusal.contains("probe event")
                && refusal.contains("/trade-api/v2/events/KXA")
                && refusal.contains("Error(404)"),
            "{refusal}"
        );
        probe(
            &http,
            "event",
            &Route::Event.target(&found.url(), "KXA"),
            None,
        )
        .await
        .unwrap();
    }

    #[test]
    fn a_signed_phase_with_a_reply_that_is_neither_2xx_nor_429_is_not_a_finding() {
        let ok = (Duration::from_secs(1), Reply::Ok);
        let throttled = (
            Duration::from_secs(2),
            Reply::Throttled {
                code: "x".into(),
                retry_after: None,
            },
        );
        assert_eq!(check_accepted("control", &[ok.clone(), throttled]), Ok(()));

        let rejected = (Duration::from_secs(3), Reply::Error(401));
        let message = check_accepted("control", &[ok, rejected]).unwrap_err();
        assert!(
            message
                .contains("control: 1 replies were neither 2xx nor 429, the first with status 401"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn a_listing_walk_pages_until_the_cursor_ends_and_leaves_the_combinations_out() {
        let log = Log::default();
        let first = fixtures!("predictions").text("markets_page");
        let mut last: serde_json::Value =
            serde_json::from_str(&first).expect("the fixture is JSON");
        let cursor = last["cursor"]
            .as_str()
            .expect("the fixture's page has a cursor")
            .to_owned();
        last["cursor"] = serde_json::json!("");
        let last = last.to_string();
        let (server, _mock) = host(
            &log,
            |_| 200,
            move |request| {
                let target = request.path_and_query();
                if target.contains("endpoint_costs") {
                    r#"{"default_cost":10,"endpoint_costs":[]}"#.to_owned()
                } else if target.contains("cursor=") {
                    last.clone()
                } else {
                    first.clone()
                }
            },
        )
        .await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .build()
            .unwrap();

        let listing = walk(&kalshi, 5).await.unwrap();

        let walks: Vec<String> = log
            .all()
            .into_iter()
            .map(|request| request.path_and_query)
            .filter(|target| target.starts_with("/trade-api/v2/markets?"))
            .collect();
        assert_eq!(walks.len(), 2, "{walks:?}");
        for pair in ["status=open", "mve_filter=exclude", "limit=1000"] {
            assert!(walks[0].contains(pair), "{pair}: {walks:?}");
        }
        assert!(!walks[0].contains("cursor="), "{walks:?}");
        assert!(walks[1].contains(&format!("cursor={cursor}")), "{walks:?}");
        // The fixture's five markets, each once though both pages hold them.
        assert_eq!(listing.tickers.len(), 5);
        assert_eq!(listing.tickers[0], "KXUAEPLSPREAD-26OCT18KHFALW-ALW4");
        assert_eq!(listing.events.len(), 5);
        assert_eq!(listing.events[0], "KXUAEPLSPREAD-26OCT18KHFALW");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --example read_soak`

Expected: the example fails to compile and no test runs. The errors are `error[E0425]: cannot find function` for `drive`, `http_client`, `probe`, `check_accepted`, `walk` and `lock`, `error[E0433]: failed to resolve: use of undeclared type` for `Arc`, `Mutex`, `Ordering` and `Predictions`, and `error[E0412]: cannot find type` for `Mutex`.

- [ ] **Step 3: Implement**

Two edits to `polyoxide-kalshi/examples/read_soak.rs`. The first replaces the temporary `dead_code` stub and the imports; the second replaces the stub `main` with the drivers. The line numbers are those of the file before the first edit, which adds 16 lines above the second; find each block by its text.

Current (`polyoxide-kalshi/examples/read_soak.rs`, lines 27-35):

```rust
#![allow(dead_code)] // Task 7.2 calls everything below from `main` and drops this line.

use std::{collections::HashSet, time::Duration};

use polyoxide_kalshi::predictions::{key::Ticker, Credentials};
use polyoxide_test_support::soak::{
    self,
    verdict::strict::{Reply, Verdict},
};
```

Replace with:

```rust
use std::{
    collections::HashSet,
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use polyoxide_core::reqwest::{Client, RequestBuilder};
use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{MarketStatusFilter, MveFilter},
    Credentials, Predictions, DEMO, PRODUCTION,
};
use polyoxide_test_support::{
    load_env,
    soak::{
        self,
        observe::install_observer,
        verdict::strict::{classify, judge, summarize, Reply, Verdict},
        Pacer,
    },
};
```

Current (`polyoxide-kalshi/examples/read_soak.rs`, line 476):

```rust
fn main() {} // Task 7.2 replaces this with the harness's drivers and `main`.
```

Replace with:

```rust
// ── Drivers ─────────────────────────────────────────────────────

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
}

/// The whole seconds that cover `duration`.
fn whole_secs(duration: Duration) -> u64 {
    duration.as_secs_f64().ceil() as u64
}

/// A bare client for the raw requests. It sets no `Accept-Encoding` of its own, so it sends what
/// core's reqwest sends every polyoxide client.
fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())
}

/// Sends `request` and reads the reply as the ramp's rules do: its status, its cache header, its
/// `Retry-After` and its body.
async fn fetch(request: RequestBuilder) -> Reply {
    let Ok(response) = request.send().await else {
        return Reply::Error(0);
    };
    let status = response.status().as_u16();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let cache = header("x-cache").or_else(|| header("cf-cache-status"));
    let retry_after = header("retry-after");
    let body = response.text().await.unwrap_or_default();
    classify(status, cache.as_deref(), retry_after.as_deref(), &body)
}

/// One request of the harness, signed when `signer` is given.
async fn send(client: &Client, target: &Target, signer: Option<&Credentials>) -> Reply {
    let mut request = client.get(&target.url);
    if let Some(credentials) = signer {
        match signed_headers(credentials, &target.path, now_millis()) {
            Ok(headers) => {
                for (name, value) in headers {
                    request = request.header(name, value);
                }
            }
            Err(_) => return Reply::Error(0),
        }
    }
    fetch(request).await
}

/// Sends `targets` at `rate` a second from `concurrency` workers until `secs` seconds pass, every
/// target has gone out, or the first `429` lands. Returns the replies in arrival order.
async fn drive(
    client: &Client,
    targets: Vec<Target>,
    signer: Option<Arc<Credentials>>,
    rate: f64,
    secs: u64,
    concurrency: usize,
) -> Vec<(Duration, Reply)> {
    let pacer = Arc::new(Pacer::new(Duration::from_secs_f64(1.0 / rate)));
    let replies = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    let mut shares: Vec<Vec<Target>> = (0..concurrency).map(|_| Vec::new()).collect();
    for (n, target) in targets.into_iter().enumerate() {
        shares[n % concurrency].push(target);
    }
    let mut workers = Vec::new();
    for share in shares {
        let client = client.clone();
        let pacer = Arc::clone(&pacer);
        let replies = Arc::clone(&replies);
        let stop = Arc::clone(&stop);
        let signer = signer.clone();
        workers.push(tokio::spawn(async move {
            for target in share {
                if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break;
                }
                pacer.wait().await;
                let reply = send(&client, &target, signer.as_deref()).await;
                let throttled = matches!(reply, Reply::Throttled { .. });
                lock(&replies).push((start.elapsed(), reply));
                if throttled {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }));
    }
    for worker in workers {
        let _ = worker.await;
    }
    let mut out = lock(&replies).clone();
    out.sort_by_key(|(at, _)| *at);
    out
}

/// Walks the open listing, without the multivariate combinations it serves first, for up to
/// `pages` pages.
async fn walk(client: &Predictions, pages: usize) -> Result<Listing, String> {
    let mut rows = Vec::new();
    let mut cursor = String::new();
    for _ in 0..pages {
        let mut request = client
            .markets()
            .list()
            .status(MarketStatusFilter::Open)
            .mve_filter(MveFilter::Exclude)
            .limit(LISTING_LIMIT);
        if !cursor.is_empty() {
            request = request.cursor(cursor.clone());
        }
        let page = request
            .send()
            .await
            .map_err(|error| format!("the listing walk failed: {error}"))?;
        rows.extend(
            page.markets
                .into_iter()
                .map(|market| (market.ticker, market.event_ticker)),
        );
        if page.cursor.is_empty() {
            break;
        }
        cursor = page.cursor;
    }
    Ok(Listing::from_rows(rows))
}

/// One request to a route before any stage uses it, so a run never measures a path the host does
/// not serve: it must answer 2xx.
async fn probe(
    client: &Client,
    label: &str,
    target: &Target,
    signer: Option<&Credentials>,
) -> Result<(), String> {
    match send(client, target, signer).await {
        Reply::Ok | Reply::CacheHit => {
            eprintln!("probe {label}: {} answers 2xx", target.path);
            Ok(())
        }
        other => Err(format!(
            "probe {label}: {} did not answer 2xx: {other:?}",
            target.path
        )),
    }
}

/// A signed phase is a finding only when every reply was a 2xx or a `429`: any other reply means
/// the host did not accept the signed requests, and the phase says nothing about a bucket.
fn check_accepted(label: &str, replies: &[(Duration, Reply)]) -> Result<(), String> {
    let statuses: Vec<u16> = replies
        .iter()
        .filter_map(|(_, reply)| match reply {
            Reply::Error(status) => Some(*status),
            _ => None,
        })
        .collect();
    if let Some(first) = statuses.first() {
        return Err(format!(
            "{label}: {} replies were neither 2xx nor 429, the first with status {first} (0 is no \
             response): {}",
            statuses.len(),
            summarize(replies)
        ));
    }
    Ok(())
}

/// One read through the shipped client.
async fn read(kalshi: &Predictions, route: Route, id: &str) -> Result<(), String> {
    match route {
        Route::Market => {
            let ticker = Ticker::new(id).map_err(|error| error.to_string())?;
            kalshi
                .markets()
                .get(&ticker)
                .send()
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Route::Event => kalshi
            .events()
            .get(id)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
    }
}

/// Steps one route up through `--stages` until the host refuses a stage.
async fn run_ramp(cfg: &Config) -> Result<ExitCode, String> {
    let base = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| PRODUCTION.to_owned());
    let route = cfg.routes[0];
    let kalshi = Predictions::builder()
        .base_url(base.as_str())
        .build()
        .map_err(|error| error.to_string())?;
    let listing = walk(&kalshi, cfg.pages).await?;
    eprintln!(
        "the listing walk found {} tickers and {} events",
        listing.tickers.len(),
        listing.events.len()
    );
    let mut pool = Pool::new(listing.ids(route));
    let http = http_client()?;
    let probed = pool
        .draw()
        .ok_or_else(|| format!("the listing walk found no {} ids", route.name()))?;
    probe(&http, route.name(), &route.target(&base, &probed), None).await?;
    let mut stages = Vec::new();
    for &rate in &cfg.stages {
        let ids = match pool.take(stage_urls(rate, cfg.stage_secs)) {
            Ok(ids) => ids,
            Err(reason) => {
                eprintln!("== stopping before {rate:.1} req/s: {reason}");
                break;
            }
        };
        let targets = ids.iter().map(|id| route.target(&base, id)).collect();
        eprintln!(
            "== {} at {rate:.1} req/s for {}s",
            route.name(),
            cfg.stage_secs
        );
        let replies = drive(&http, targets, None, rate, cfg.stage_secs, cfg.concurrency).await;
        let verdict = judge(&replies, rate, cfg.stage_secs);
        eprintln!("   {} replies, verdict {verdict:?}", replies.len());
        eprintln!("   {}", summarize(&replies));
        let unclean = verdict != Verdict::Clean;
        stages.push((rate, verdict));
        if unclean {
            break;
        }
        eprintln!("   cooling down {}s", cfg.cooldown_secs);
        tokio::time::sleep(Duration::from_secs(cfg.cooldown_secs)).await;
    }
    let refused = stages
        .last()
        .is_some_and(|(_, verdict)| matches!(verdict, Verdict::Throttled { .. }));
    let clean = highest_clean(&stages);
    println!(
        "RESULT ramp route={} highest_clean={clean:?} refused={refused} stages={stages:?}",
        route.name()
    );
    Ok(if clean.is_some() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Paces reads by the shipped client's own throttle and requires zero `429`s.
async fn run_validation(cfg: &Config) -> Result<ExitCode, String> {
    let observer = install_observer(Instant::now());
    let base = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| PRODUCTION.to_owned());
    // The shipped client, untouched but for the in-flight cap a caller asks to lift: its own
    // throttle paces every request.
    let mut builder = Predictions::builder().base_url(base.as_str());
    if let Some(cap) = cfg.max_concurrent {
        builder = builder.max_concurrent(cap);
    }
    let kalshi = builder.build().map_err(|error| error.to_string())?;
    let listing = walk(&kalshi, cfg.pages).await?;
    eprintln!(
        "the listing walk found {} tickers and {} events",
        listing.tickers.len(),
        listing.events.len()
    );
    let http = http_client()?;
    let mut pools = Vec::new();
    for &route in &cfg.routes {
        let mut pool = Pool::new(listing.ids(route));
        let probed = pool
            .draw()
            .ok_or_else(|| format!("the listing walk found no {} ids", route.name()))?;
        probe(&http, route.name(), &route.target(&base, &probed), None).await?;
        pools.push((route, Arc::new(Mutex::new(pool))));
    }
    let deadline = Instant::now() + Duration::from_secs(cfg.stage_secs);
    let sent = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let exhausted = Arc::new(AtomicBool::new(false));
    let mut workers = Vec::new();
    for (route, pool) in &pools {
        for _ in 0..cfg.concurrency {
            let route = *route;
            let pool = Arc::clone(pool);
            let kalshi = kalshi.clone();
            let sent = Arc::clone(&sent);
            let failed = Arc::clone(&failed);
            let exhausted = Arc::clone(&exhausted);
            workers.push(tokio::spawn(async move {
                while Instant::now() < deadline {
                    let drawn = lock(&pool).draw();
                    let Some(id) = drawn else {
                        exhausted.store(true, Ordering::Relaxed);
                        break;
                    };
                    let result = read(&kalshi, route, &id).await;
                    sent.fetch_add(1, Ordering::Relaxed);
                    if let Err(error) = result {
                        failed.fetch_add(1, Ordering::Relaxed);
                        eprintln!("{}: {error}", route.name());
                    }
                }
            }));
        }
    }
    for worker in workers {
        let _ = worker.await;
    }
    let sent = sent.load(Ordering::Relaxed);
    let failed = failed.load(Ordering::Relaxed);
    let exhausted = exhausted.load(Ordering::Relaxed);
    let throttles = observer.throttles_by_path();
    let throttled: u64 = throttles.values().sum();
    // `sent` counts calls: a call the retry loop re-sends is several requests on the wire but one
    // here; the throttle counts are per request.
    println!(
        "validation: {sent} calls over {}s, {throttled} throttled, {failed} failed{}",
        cfg.stage_secs,
        if exhausted {
            ", and the distinct URLs ran out first"
        } else {
            ""
        }
    );
    for (path, count) in &throttles {
        println!("  {path}: {count} throttled");
    }
    let clean = throttled == 0 && failed == 0 && !exhausted;
    println!("RESULT validation clean={clean} calls={sent} throttled={throttled} failed={failed}");
    Ok(if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Drains the demo key's Read bucket on a route that needs the key and then on a public route,
/// and says whether the public read's `429` follows the key's bucket.
async fn run_signed(cfg: &Config) -> Result<ExitCode, String> {
    let creds = load_env(&["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"])
        .map_err(|missing| missing.to_string())?;
    let credentials = Credentials::from_pem(
        creds.get("KALSHI_DEMO_API_KEY_ID"),
        creds.get("KALSHI_DEMO_PRIVATE_KEY"),
    )
    .map_err(|error| error.to_string())?;
    let base = cfg.base_url.clone().unwrap_or_else(|| DEMO.to_owned());
    let kalshi = Predictions::builder()
        .base_url(base.as_str())
        .credentials(credentials.clone())
        .build()
        .map_err(|error| error.to_string())?;
    let limits = kalshi
        .account()
        .limits()
        .send()
        .await
        .map_err(|error| format!("account().limits() failed: {error}"))?;
    let costs = kalshi
        .account()
        .endpoint_costs()
        .send()
        .await
        .map_err(|error| format!("account().endpoint_costs() failed: {error}"))?;
    let cost_of = |template: &str| {
        costs
            .endpoint_costs
            .iter()
            .find(|entry| entry.method == "GET" && entry.path == template)
            .map_or(costs.default_cost, |entry| entry.cost)
    };
    let (capacity, refill) = (limits.read.bucket_capacity, limits.read.refill_rate);
    let (public_cost, private_cost) = (cost_of(PUBLIC_TEMPLATE), cost_of(PRIVATE_TEMPLATE));
    let public = Drain::new(capacity, refill, public_cost)?;
    let private = Drain::new(capacity, refill, private_cost)?;
    eprintln!(
        "the key's Read bucket: {capacity} tokens refilling {refill} a second (tier {}); a public \
         read costs {public_cost} tokens and the limits read {private_cost}",
        limits.usage_tier
    );

    let listing = walk(&kalshi, cfg.pages).await?;
    let mut pool = Pool::new(listing.ids(Route::Market));
    let http = http_client()?;
    let signer = Arc::new(credentials);
    let control_target = Target {
        url: format!("{base}{PRIVATE_TEMPLATE}"),
        path: PRIVATE_TEMPLATE.to_owned(),
    };
    probe(
        &http,
        "private control",
        &control_target,
        Some(signer.as_ref()),
    )
    .await?;
    let probed = pool.draw().ok_or("the listing walk found no tickers")?;
    probe(
        &http,
        "public",
        &Route::Market.target(&base, &probed),
        Some(signer.as_ref()),
    )
    .await?;

    let window = whole_secs(private.window);
    eprintln!(
        "== control: {PRIVATE_TEMPLATE}, signed, {:.1} req/s for {window}s; a bucket that starts \
         full empties after {:?}",
        private.rate, private.model
    );
    let targets = vec![control_target; stage_urls(private.rate, window)];
    let control = drive(
        &http,
        targets,
        Some(Arc::clone(&signer)),
        private.rate,
        window,
        cfg.concurrency,
    )
    .await;
    check_accepted("control", &control)?;
    eprintln!("   {}", summarize(&control));

    let refill_wait =
        Duration::from_secs_f64(f64::from(capacity) / f64::from(refill)) + REFILL_MARGIN;
    eprintln!("   waiting {refill_wait:?} for the bucket to refill");
    tokio::time::sleep(refill_wait).await;

    let window = whole_secs(public.window);
    let ids = pool.take(stage_urls(public.rate, window))?;
    let targets: Vec<Target> = ids
        .iter()
        .map(|id| Route::Market.target(&base, id))
        .collect();
    eprintln!(
        "== public: {PUBLIC_TEMPLATE}, signed, {:.1} req/s for {window}s over {} distinct URLs; \
         a bucket that starts full empties after {:?}",
        public.rate,
        targets.len(),
        public.model
    );
    let public_replies = drive(
        &http,
        targets,
        Some(Arc::clone(&signer)),
        public.rate,
        window,
        cfg.concurrency,
    )
    .await;
    check_accepted("public", &public_replies)?;
    eprintln!("   {}", summarize(&public_replies));

    let control_first = first_throttle(&control);
    let public_first = first_throttle(&public_replies);
    let verdict = signed_verdict(private.model, control_first, public.model, public_first);
    println!(
        "RESULT signed verdict={verdict:?} control_first_429={control_first:?} \
         control_model={:?} public_first_429={public_first:?} public_model={:?}",
        private.model, public.model
    );
    Ok(match verdict {
        SignedVerdict::Inconclusive(_) => ExitCode::from(1),
        SignedVerdict::Follows | SignedVerdict::DoesNotFollow => ExitCode::SUCCESS,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let cfg = match Config::from_args(std::env::args().skip(1)) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("error: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let outcome = match cfg.mode {
        Mode::Ramp => run_ramp(&cfg).await,
        Mode::Validate => run_validation(&cfg).await,
        Mode::Signed => run_signed(&cfg).await,
    };
    outcome.unwrap_or_else(|error| {
        eprintln!("error: {error}");
        ExitCode::from(2)
    })
}
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --example read_soak`

Expected: `running 25 tests`, each `ok`, and `test result: ok. 25 passed; 0 failed`.

Then the offline smoke runs and the gates this task can fail:

- `nix develop --command cargo run -p polyoxide-kalshi --no-default-features --features predictions --example read_soak -- --help` prints the usage, whose first line is `Measure what Kalshi's production host serves an unsigned client, validate the shipped client`, and exits 0.
- `nix develop --command cargo run -p polyoxide-kalshi --no-default-features --features predictions --example read_soak -- --routes market,event` prints `error: a ramp measures one route at a time; list several only with --pace client` above the usage and exits 2.
- `nix develop --command cargo fmt --all -- --check` prints nothing.
- `nix develop --command cargo clippy -p polyoxide-kalshi --no-default-features --features predictions --all-targets -- -D warnings` prints no warning, and with the `dead_code` stub gone it proves every item of Task 7.1 is called.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(kalshi): read_soak's drivers, the ramp, the shipped-client validation and the signed probe (Epic 9, Story 7)" -- polyoxide-kalshi/examples/read_soak.rs
```

### Task 7.3: Probe the paths and ramp the unsigned limit on production, and record it

**Files:**
- Modify: `docs/specs/kalshi/OBSERVED.md`

A live run against `https://external-api.kalshi.com`, unsigned and with no secrets. It needs the network, and its result is the host's for the machine's IP, so run it from an ordinary network and say where in the record. Write run logs to `/tmp`, never into the repository: the touch-set gate refuses an untracked file. If the build is killed for memory, set `CARGO_INCREMENTAL=0`.

- [ ] **Step 1: Ramp the `market` route**

Run:

```bash
nix develop --command cargo run --release -p polyoxide-kalshi --no-default-features --features predictions --example read_soak -- --routes market --stages 10,20,40,80,160,240 --stage-secs 60 --cooldown-secs 30 2>&1 | tee /tmp/read_soak_market.log
```

Expected, on stderr: the listing walk's count of tickers and events, `probe market: /trade-api/v2/markets/<ticker> answers 2xx`, then for each stage `== market at 10.0 req/s for 60s`, `   <n> replies, verdict Clean` and a line such as `   <n> origin, 0 cached, 0 throttled, 0 errors; last reply at 60.0s; first Retry-After None`. The ramp stops after the first stage that is not `Clean`. The last stdout line is `RESULT ramp route=market highest_clean=Some(<rate>) refused=<true|false> stages=[…]`. Exit 0 when some stage was clean, 1 when none was, and 2 on a setup error (the probe did not answer 2xx, or the walk failed).

If the harness stops with `== stopping before 160.0 req/s: 9601 more distinct URLs are needed and the listing walk left 4300; …`, the open listing ran out of distinct tickers. Run the stages from there up again with a shorter window, `--stages 160,240 --stage-secs 30`, and put that window in the record's table beside those rows.

- [ ] **Step 2: Ramp the `event` route**

Run:

```bash
nix develop --command cargo run --release -p polyoxide-kalshi --no-default-features --features predictions --example read_soak -- --routes event --stages 10,20,40 --stage-secs 60 --cooldown-secs 30 2>&1 | tee /tmp/read_soak_event.log
```

Expected: the same shape of output for `event`. The walk finds far fewer distinct event tickers than tickers, so the harness may stop early with the message of Step 1, and the record says where it stopped and why.

- [ ] **Step 3: Read the verdicts**

A stage's verdict is `strict`'s. Only a `Clean` or a `Throttled` stage is a finding; any other ends the ramp without one:

- `Throttled { after, code }`: the host refused. Kalshi documents a `429` body of `{"error":"too many requests"}`, so `code` reads `too many requests`; record what the run printed.
- `UnderDriven { achieved }`: the harness did not reach its own rate, so a clean result would be about a lower one. Rerun that stage and the ones above it with a larger `--concurrency` (96), and do not record the short run as clean.
- `Saturated { origin_share }`: most replies were served from a cache, so the URLs were not distinct enough to reach the origin. Stop and ask: the host's caching needs a different probe.
- `Invalid { errors }`: some replies were neither 2xx nor 429, for example a `403` or a `5xx` from a front door. Record the statuses, rerun the stage once, and stop and ask if it repeats.

If the `event` route was refused at a rate below the `market` route's highest clean stage, the limit is per route and one number cannot size the client. Stop and ask the owner; Task 7.5 sizes from the `market` route only when the `event` route did not refuse earlier.

- [ ] **Step 4: Record the paths and the ramp**

Insert the subsection below directly above the Epic 9 section's last line, with the run's numbers in place of the angle-bracketed fields. The closing `<!-- rate limits end -->` is this subsection's own anchor: Tasks 7.4 to 7.6 add their paragraphs directly above it, so the subsection stays in one piece whatever the other stories insert above the Epic 9 anchor.

Current (the Epic 9 section's last line, written by Story 1):

```markdown
<!-- epic 9 findings go above this line -->
```

Replace with:

````markdown
### Rate limits

Measured with `polyoxide-kalshi/examples/read_soak.rs` (Epic 9, Story 7). Every URL of a run was distinct, because a repeated URL can be answered from a cache without reaching the origin, and each route was probed once on the host before any stage. A stage is requests a second: Basic's Read bucket refills 200 tokens a second, which is 20 reads at the default cost of 10.

**The paths, probed first** (<UTC date and time>, from <where the run was made>):

| Route | Path | Answered |
|---|---|---|
| market | `GET /trade-api/v2/markets/{ticker}` | <status> |
| event | `GET /trade-api/v2/events/{event_ticker}` | <status> |

**The unsigned ramp** on production, `--stage-secs 60 --cooldown-secs 30` unless a row says otherwise:

| Route | Stage (requests a second) | Replies | Verdict | First 429, seconds into the stage, and its body |
|---|---|---|---|---|
| market | 10 | <replies> | Clean | |

<One row for every stage either route printed: the stage's rate, the replies it counted, the verdict word and, for a Throttled stage, when its first 429 arrived and the body's `error` text.>

<For the market route, one of these two sentences.>
Highest clean stage: <N> requests a second (market). The host refused <rate> requests a second after <seconds> s with `<code>`.
No stage was refused: every stage of the market route through <top> requests a second was clean.

<For the event route, the same sentence, and where the walk's distinct event tickers ended its ramp when they did.>

<!-- rate limits end -->

<!-- epic 9 findings go above this line -->
````

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(kalshi): the unsigned read soak on production (Epic 9, Story 7)" -- docs/specs/kalshi/OBSERVED.md
```

### Task 7.4: The signed-bucket probe on the demo host, and record it

**Files:**
- Modify: `docs/specs/kalshi/OBSERVED.md`

D11's second measurement: whether a signed public read draws on the key's Read bucket. The probe drains the bucket twice, once on `GET /account/limits`, which needs the key and so certainly draws on it (the control), and once on a public route over distinct URLs. A public read counts only against a control that throttled where the bucket's size says it should; without that control, a quiet public route could mean only that the demo host does not enforce the bucket.

- [ ] **Step 1: Run the probe**

It reads `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` through test-support's loaders, from the environment and then a `.env` file. If the run ends `error: credentials not configured: …`, stop and ask the owner for the two variables (spec §15); do not skip the task and do not write a fixture.

```bash
nix develop --command cargo run --release -p polyoxide-kalshi --no-default-features --features predictions --example read_soak -- --signed 2>&1 | tee /tmp/read_soak_signed.log
```

Expected, on stderr: `the key's Read bucket: <capacity> tokens refilling <refill> a second (tier <tier>); a public read costs <n> tokens and the limits read <n>`, two `probe … answers 2xx` lines, `== control: /trade-api/v2/account/limits, signed, <rate> req/s for <window>s; a bucket that starts full empties after <model>`, the control's counts, `waiting <duration> for the bucket to refill`, then the same for `== public: /trade-api/v2/markets/:ticker …`. The last stdout line is `RESULT signed verdict=<Follows|DoesNotFollow|Inconclusive(…)> control_first_429=… control_model=… public_first_429=… public_model=…`. Exit 0 for `Follows` and `DoesNotFollow`, 1 for `Inconclusive`, and 2 on a setup error (the probe of a path did not answer 2xx, a phase met a reply that was neither 2xx nor 429, or the demo listing had too few tickers for the public phase).

- [ ] **Step 2: Read the verdict**

- `Follows`: the control throttled where the key's bucket empties and so did the public read. A signed public read draws on the key's bucket. Task 7.6 takes its stay branch.
- `DoesNotFollow`: the control throttled where the bucket empties and the public read did not throttle in its window, or throttled well before. A signed public read does not draw on the key's bucket. Task 7.6 takes its move branch.
- `Inconclusive(reason)`: nothing is concluded, most often because the control never throttled, so the demo host did not enforce the bucket at this rate. Stop and ask the owner; the keyed client stays as it is until they answer, and the record says so.

A `check_accepted` failure (`control: 3 replies were neither 2xx nor 429, the first with status 401 (0 is no response): …`) means the host did not accept the signed requests; it is a setup problem, not a result. Status 0 is a request that got no response.

- [ ] **Step 3: Record the probe**

Insert the paragraph directly above the subsection's own anchor, with the run's numbers in place of the angle-bracketed fields.

Current:

```markdown
<!-- rate limits end -->
```

Replace with:

````markdown
**The signed-bucket probe**, demo host (`https://external-api.demo.kalshi.co`), <UTC date and time>. The demo key's account is tier <usage_tier> with a Read bucket of <capacity> tokens refilling <refill> a second; a public read costs <n> tokens and the limits read <n>. Each phase sends signed reads faster than the bucket refills, so a bucket that starts full empties after the time in the table, and stops at its first 429. The control repeats the limits route, which is signed and so uncached; the public phase reads distinct tickers.

| Phase | Route | Requests a second | The key's bucket empties after | First 429 |
|---|---|---|---|---|
| control (needs the key) | `GET /account/limits` | <rate> | <model> | <time, or none in the <window> s window> |
| public | `GET /markets/{ticker}` | <rate> | <model> | <time, or none in the <window> s window> |

Verdict: <follows | does not follow | inconclusive>. <One sentence: the control throttled at <time>, where a bucket that empties after <model> puts it, so the host enforces the key's bucket at this rate, and the public read then <throttled at <time>, so it draws on the key's bucket | did not throttle in <window> s, or throttled at <time>, well before the key's bucket empties, so it does not>.>

<!-- rate limits end -->
````

- [ ] **Step 4: Commit**

```bash
git commit -m "docs(kalshi): the signed-bucket probe on the demo host (Epic 9, Story 7)" -- docs/specs/kalshi/OBSERVED.md
```

### Task 7.5: Size the unsigned client from the soak

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:1-2`
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:26-27`
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:204-215`
- Test: `polyoxide-kalshi/src/predictions/throttle.rs`
- Modify: `docs/specs/kalshi/OBSERVED.md:47`

An unsigned client's Read bucket is sized from the soak only when the host refused at or below Basic's own rate, 20 reads a second (spec D11: "If no rate below Basic's 200/s refuses, the sizing stays Basic's and is marked confirmed"). Then its refill is the highest clean stage less a tenth, at the default cost of a read, which is always below Basic's 200 tokens a second; the measurement never raises the sizing above Basic's. When Basic's rate was served clean, the sizing stays Basic's and is marked confirmed. Either way its depth stays Basic's. The depth stays because the ramp measured a rate (its `Pacer` banks no credit) and not a depth, and the validation run below holds Basic's depth against the host. **This is the story's landmine code.** Core and Binance each carry a private `RESERVED_FRACTION` of 10 beside their published figures, measured to be a margin the host's count never agrees with exactly (core's `quota` table: 95% of a published rate was refused, 90% ran clean); this task's copy does the same to a measured clean rate, and it is a margin below the measurement, never the measurement. No bucket is given a burst setting, and no existing test changes: the sizing lands in the one place an unsigned client's bucket is confirmed, so the tests that pin Basic's 600 tokens (`a_new_client_has_the_provisional_basic_read_bucket_and_no_key`, `an_anonymous_client_confirms_at_basic_so_a_cost_above_it_is_refused_not_waited_on` and the `PredictionsThrottle::new()` tests) hold, because only the refill changes and `new()` is untouched.

Choose the branch from the market route's sentence in `OBSERVED.md`'s `### Rate limits`:

- **Branch A, the host refused at or below Basic's rate.** The record says `Highest clean stage: N requests a second (market)` with N below 20, Basic's 20 reads a second (200 tokens at 10 a read): the stage after N, at most Basic's own rate, was refused, and a client at Basic's sizing would send into that refusal. Round N down to a whole number. The refill the measurement gives, `(N − ⌈N/10⌉) × 10`, is then below Basic's 200 tokens a second (N of 19 comes to 170), so it replaces Basic's sizing.
- **Branch B, Basic's rate was served clean.** The stage of 20 requests a second, or a higher one, was clean, so no rate at or below Basic's refused. The record says either `No stage was refused: every stage of the market route through <top> requests a second was clean` (240 with the default stages), or `Highest clean stage: N requests a second (market)` with N of 20 or more and a refusal above it; `<top>` is that highest clean stage. Basic's sizing is confirmed and stays: a measurement above Basic's never raises it.
- Anything else (a ramp that ended on `UnderDriven`, `Saturated` or `Invalid`, a first stage that was not clean, or a clean stage below 20 followed by a refused one above 20, since then a rate at Basic's was never tried) is not a result: finish Task 7.3 first. If even the first stage was refused, rerun the ramp with `--stages 1,2,5` so some stage is clean. A ramp whose clean stage is below 20 and whose refusal is above it is rerun with `--stages 20,40`, and those rows join the record's table.

Both branches define `UNSIGNED_READ_REFILL`, so Task 7.6 reads the same either way.

- [ ] **Step 1: Write the failing tests**

Append one of these modules at the end of `polyoxide-kalshi/src/predictions/throttle.rs`, after the `tests` module. `10` and `90` in Branch A's module stand for the record: write N, rounded down, as `RECORDED_CLEAN_RATE`, and `(N − ⌈N/10⌉) × 10` worked by hand as `RECORDED_REFILL_TOKENS` (for N of 10, 10 − 1 = 9 and 90; for 15, 15 − 2 = 13 and 130; N is always below 20 in this branch). The module's table pins `after_reserve` against hand-worked pairs, so a wrong constant and a wrong formula cannot hide each other.

Branch A:

```rust
#[cfg(test)]
mod unsigned_sizing_tests {
    use mockito::{Mock, Server, ServerGuard};
    use polyoxide_test_support::fixtures;

    use super::*;
    use crate::predictions::{Credentials, Predictions};

    const COSTS: &str = "/trade-api/v2/account/endpoint_costs";
    const LIMITS: &str = "/trade-api/v2/account/limits";
    const STATUS: &str = "/trade-api/v2/exchange/status";
    const NO_COSTS: &str = r#"{"default_cost":10,"endpoint_costs":[]}"#;
    const STATUS_BODY: &str = r#"{"exchange_active":true,"trading_active":true}"#;

    /// The highest clean stage of `docs/specs/kalshi/OBSERVED.md`'s "Rate limits" record, in
    /// requests a second, and what the arithmetic makes of it: less a tenth, at 10 tokens a read.
    const RECORDED_CLEAN_RATE: u32 = 10;
    const RECORDED_REFILL_TOKENS: u32 = 90;

    /// The Ed25519 key of the signing vectors, which are throwaway ones.
    fn keyed() -> Credentials {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let body = vectors["ed25519"]["pkcs8_body"]
            .as_str()
            .expect("the vectors hold an Ed25519 key body");
        let label = "PRIVATE KEY";
        let pem = format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n");
        Credentials::from_pem("test-key", &pem).unwrap()
    }

    /// A mock answering `path` with `status` and `body`, expected `hits` times.
    async fn answer(
        server: &mut ServerGuard,
        path: &str,
        status: usize,
        body: &str,
        hits: usize,
    ) -> Mock {
        server
            .mock("GET", path)
            .with_status(status)
            .with_header("content-type", "application/json")
            .with_body(body)
            .expect(hits)
            .create_async()
            .await
    }

    #[test]
    fn a_tenth_of_a_measured_rate_is_left_unused() {
        for (measured, kept) in [
            (1, 1),
            (2, 1),
            (9, 8),
            (10, 9),
            (11, 9),
            (20, 18),
            (37, 33),
            (40, 36),
            (200, 180),
        ] {
            assert_eq!(after_reserve(measured), kept, "{measured}");
        }
    }

    #[test]
    fn the_unsigned_refill_is_the_recorded_clean_rate_less_a_tenth_at_ten_tokens_a_read() {
        assert_eq!(UNSIGNED_CLEAN_RATE, RECORDED_CLEAN_RATE);
        assert_eq!(UNSIGNED_READ_REFILL, RECORDED_REFILL_TOKENS);
        assert_eq!(
            UNSIGNED_READ_REFILL,
            after_reserve(RECORDED_CLEAN_RATE) * 10
        );
    }

    #[test]
    fn the_measured_sizing_is_below_basics_and_never_raises_it() {
        use std::cmp::Ordering;

        // Basic's 200 tokens a second are 20 reads at the default cost. The soak replaces Basic's
        // sizing only when the host refused at or below that rate, so the clean rate it measured
        // is under it and the sizing it gives is under Basic's 200 tokens, never over.
        assert_eq!(
            UNSIGNED_CLEAN_RATE.cmp(&(BASIC_READ_REFILL / DEFAULT_COST)),
            Ordering::Less
        );
        assert_eq!(
            UNSIGNED_READ_REFILL.cmp(&BASIC_READ_REFILL),
            Ordering::Less,
            "{UNSIGNED_READ_REFILL} tokens a second"
        );
    }

    #[tokio::test]
    async fn an_unsigned_client_is_resized_to_the_measured_refill_and_keeps_basics_depth() {
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, 200, NO_COSTS, 1).await;
        let limits = answer(&mut server, LIMITS, 200, "{}", 0).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .build()
            .unwrap();

        kalshi.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        let bucket = format!("{:?}", kalshi.inner.throttle.bucket());
        assert!(
            bucket.contains(&format!("refill_per_sec: {RECORDED_REFILL_TOKENS}")),
            "{bucket}"
        );
        assert!(
            bucket.contains(&format!("capacity: {BASIC_READ_CAPACITY}")),
            "{bucket}"
        );
        assert!(bucket.contains("provisional: false"), "{bucket}");
    }

    #[tokio::test]
    async fn a_keyed_clients_unreadable_limits_leave_basics_refill_not_the_unsigned_one() {
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, 200, NO_COSTS, 1).await;
        let limits = answer(&mut server, LIMITS, 500, "{}", 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .credentials(keyed())
            .build()
            .unwrap();

        kalshi.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        let bucket = format!("{:?}", kalshi.inner.throttle.bucket());
        assert!(
            bucket.contains(&format!("refill_per_sec: {BASIC_READ_REFILL}")),
            "{bucket}"
        );
    }
}
```

Branch B:

```rust
#[cfg(test)]
mod unsigned_sizing_tests {
    use mockito::{Mock, Server, ServerGuard};

    use super::*;
    use crate::predictions::Predictions;

    const COSTS: &str = "/trade-api/v2/account/endpoint_costs";
    const STATUS: &str = "/trade-api/v2/exchange/status";
    const NO_COSTS: &str = r#"{"default_cost":10,"endpoint_costs":[]}"#;
    const STATUS_BODY: &str = r#"{"exchange_active":true,"trading_active":true}"#;

    /// A mock answering `path` with 200 and `body`, expected once.
    async fn answer(server: &mut ServerGuard, path: &str, body: &str) -> Mock {
        server
            .mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .expect(1)
            .create_async()
            .await
    }

    #[test]
    fn the_unsigned_refill_is_basics_because_the_soak_confirmed_it() {
        assert_eq!(UNSIGNED_READ_REFILL, 200);
        assert_eq!(UNSIGNED_READ_REFILL, BASIC_READ_REFILL);
    }

    #[tokio::test]
    async fn an_unsigned_client_ends_confirmed_at_basics_capacity_and_refill() {
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, NO_COSTS).await;
        let status = answer(&mut server, STATUS, STATUS_BODY).await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .build()
            .unwrap();

        kalshi.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        status.assert_async().await;
        let bucket = format!("{:?}", kalshi.inner.throttle.bucket());
        assert!(bucket.contains("capacity: 600"), "{bucket}");
        assert!(bucket.contains("refill_per_sec: 200"), "{bucket}");
        assert!(bucket.contains("provisional: false"), "{bucket}");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib unsigned_sizing_tests`

Expected, Branch A: the crate's unit tests fail to compile with `error[E0425]: cannot find value` for `UNSIGNED_CLEAN_RATE` and `UNSIGNED_READ_REFILL` and `error[E0425]: cannot find function` for `after_reserve`. Branch B: `error[E0425]: cannot find value` for `UNSIGNED_READ_REFILL`.

- [ ] **Step 3: Implement**

Three edits to `polyoxide-kalshi/src/predictions/throttle.rs`.

The module doc names the unsigned size. Current (lines 1-2):

```rust
//! The Read throttle: one token bucket, sized at Kalshi's Basic tier until the client has read the
//! account's own size, and the per-route costs it charges.
```

Replace with:

```rust
//! The Read throttle: one token bucket, sized at Kalshi's Basic tier until the client has read the
//! account's own size or, holding no key, taken the size the unsigned soak measured, and the
//! per-route costs it charges.
```

The unsigned sizing sits beside Basic's. Current (lines 26-27):

```rust
/// Tokens a second the Basic tier's Read bucket refills. No tier's is lower.
pub(crate) const BASIC_READ_REFILL: u32 = 200;
```

Replace with, Branch A (write N, rounded down, where `10` stands):

```rust
/// Tokens a second the Basic tier's Read bucket refills. No tier's is lower.
pub(crate) const BASIC_READ_REFILL: u32 = 200;

/// Reciprocal of the share of a measured clean rate the unsigned client leaves unused, as in core's
/// `RESERVED_FRACTION` and Binance's: the highest rate a host served clean is not a rate to aim at,
/// because the server's count and the client's never agree exactly. `10` reserves a tenth, so the
/// client targets 90%.
const RESERVED_FRACTION: u32 = 10;

/// The highest rate, in requests a second, at which Kalshi's production host served an unsigned
/// client over distinct URLs with no `429` before it refused one (`docs/specs/kalshi/OBSERVED.md`,
/// "Rate limits": "Highest clean stage"). It is below Basic's 20 reads a second: the soak replaces
/// Basic's sizing only when the host refused at or below that rate.
pub(crate) const UNSIGNED_CLEAN_RATE: u32 = 10;

/// `measured` less its reserve, and never below one: a bucket that never refills is a panic.
const fn after_reserve(measured: u32) -> u32 {
    let kept = measured - measured.div_ceil(RESERVED_FRACTION);
    if kept == 0 {
        1
    } else {
        kept
    }
}

/// Tokens a second the unsigned client's Read bucket refills: its measured clean rate less a tenth,
/// at the default cost of a read. Its depth stays Basic's: the soak measured a rate and not a
/// depth, and the validation run held Basic's depth clean on the host.
pub(crate) const UNSIGNED_READ_REFILL: u32 = after_reserve(UNSIGNED_CLEAN_RATE) * DEFAULT_COST;
```

or, Branch B (write the record's highest clean stage in place of `240` if it differs):

```rust
/// Tokens a second the Basic tier's Read bucket refills. No tier's is lower.
pub(crate) const BASIC_READ_REFILL: u32 = 200;

/// Tokens a second the unsigned client's Read bucket refills: Basic's. The soak served an unsigned
/// client over distinct URLs clean through every stage up to 240 requests a second, at least the
/// 20 reads a second Basic's 200 tokens come to, so no rate at or below Basic's refused and
/// Basic's sizing is confirmed, never raised or replaced (`docs/specs/kalshi/OBSERVED.md`,
/// "Rate limits").
pub(crate) const UNSIGNED_READ_REFILL: u32 = BASIC_READ_REFILL;
```

The unsigned arm of the confirmation takes the size instead of only confirming Basic's. Current (lines 204-215):

```rust
    /// Sizes the Read bucket from `/account/limits` when the client holds a key, and confirms it
    /// in every case.
    async fn read_limits(&self) {
        use crate::predictions::request::routes;
        use crate::predictions::types::AccountLimits;

        let bucket = self.throttle.bucket();
        if self.auth.is_none() {
            // No limit for unauthenticated requests is published: Basic's sizing stands.
            bucket.confirm();
            return;
        }
```

Replace with:

```rust
    /// Sizes the Read bucket from `/account/limits` when the client holds a key, and from the
    /// unsigned soak's measurement when it holds none. Either way it ends confirmed.
    async fn read_limits(&self) {
        use crate::predictions::request::routes;
        use crate::predictions::types::AccountLimits;

        let bucket = self.throttle.bucket();
        if self.auth.is_none() {
            // No limit for unauthenticated requests is published: the unsigned soak sized this
            // bucket (`UNSIGNED_READ_REFILL`, recorded in `docs/specs/kalshi/OBSERVED.md`), and
            // `resize` confirms it.
            bucket.resize(BASIC_READ_CAPACITY, UNSIGNED_READ_REFILL);
            return;
        }
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib unsigned_sizing_tests`

Expected: Branch A `running 5 tests`, Branch B `running 2 tests`, all `ok`.

Then the whole crate, where every existing test, the ones that pin Basic's 600 tokens included, still passes: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions`.

Show that the new tests catch a changed margin, then restore it. Branch A: set `RESERVED_FRACTION` to `20` and rerun the filter above. `a_tenth_of_a_measured_rate_is_left_unused` fails at the `(11, 9)` pair with `left: 10` and `right: 9`. Restore it, then set `UNSIGNED_CLEAN_RATE` to `24`, a rate above Basic's: `the_measured_sizing_is_below_basics_and_never_raises_it` fails with `left: Greater` and `right: Less`, and `the_unsigned_refill_is_the_recorded_clean_rate_less_a_tenth_at_ten_tokens_a_read` fails too. Branch B: set `BASIC_READ_REFILL` to `199`; `the_unsigned_refill_is_basics_because_the_soak_confirmed_it` fails with `left: 199` and `right: 200`. Restore the line afterwards, and check with `git diff polyoxide-kalshi/src/predictions/throttle.rs` that only this task's edits remain.

Then `nix develop --command cargo fmt --all -- --check` prints nothing and `nix develop --command cargo clippy -p polyoxide-kalshi --no-default-features --features predictions --all-targets -- -D warnings` prints no warning.

- [ ] **Step 5: Validate the shipped client on production, and record the sizing**

The validation paces both routes through the shipped client, built as `Predictions::builder().build()`, and requires zero `429`s over distinct URLs. The cap of 32 in flight lifts the client's own default of 4 so the bucket, not the concurrency, binds.

```bash
nix develop --command cargo run --release -p polyoxide-kalshi --no-default-features --features predictions --example read_soak -- --routes all --pace client --concurrency 32 --max-concurrent 32 --stage-secs 120 2>&1 | tee /tmp/read_soak_validation.log
```

Expected: two `probe … answers 2xx` lines, `validation: <n> calls over 120s, 0 throttled, 0 failed`, and `RESULT validation clean=true calls=<n> throttled=0 failed=0`; exit 0.

- A throttle (`<path>: <n> throttled`) means Basic's depth is not safe on the host. Stop and ask the owner. Do not change `BASIC_READ_CAPACITY` and do not edit the existing tests that pin it: the depth is the owner's decision.
- `, and the distinct URLs ran out first` means the listing walk ended before the window did, so the run is shorter than asked and is not clean. Rerun with `--stage-secs 60`, and put that window in the record.

Then record the sizing. First the Sizing and Validation paragraphs, directly above the subsection's own anchor.

Current:

```markdown
<!-- rate limits end -->
```

Replace with, Branch A:

````markdown
**Sizing.** The host refused <rate> requests a second, at or below Basic's 20 reads a second, so a client at Basic's sizing would have sent into the refusal, and the measurement replaces Basic's sizing. The unsigned client's Read bucket refills `(N − ⌈N/10⌉) × 10` tokens a second for N = <N>, the highest clean stage: <refill> tokens a second, below Basic's 200, a tenth of the measured rate left unused (`RESERVED_FRACTION`, as core's and Binance's) at the default cost of 10 tokens a read. Its depth stays Basic's 600 tokens, since the ramp measured a rate and not a depth; the validation below ran that depth against the host.

**Validation**, the shipped client on production (`--routes all --pace client --concurrency 32 --max-concurrent 32 --stage-secs 120`): <calls> calls over 120 s, <n> throttled, <n> failed.

<!-- rate limits end -->
````

or, Branch B:

````markdown
**Sizing.** Basic's sizing stands for the unsigned client: 600 tokens refilling 200 a second, which is 20 reads a second at 10 tokens. The host served an unsigned client clean at every stage through <top> requests a second, <top ÷ 20> times that rate, <and refused <rate> requests a second above it | and refused no stage>, so no rate at or below Basic's was refused, and the provisional sizing is confirmed and not replaced. A measurement above Basic's never raises it.

**Validation**, the shipped client on production (`--routes all --pace client --concurrency 32 --max-concurrent 32 --stage-secs 120`): <calls> calls over 120 s, <n> throttled, <n> failed.

<!-- rate limits end -->
````

Then the Epic 8 section says what is no longer deferred. Current (line 47):

```markdown
- A client with no credentials keeps that sizing. No anonymous limit is published, so for it the sizing is a guess. A soak that measures the anonymous limit is deferred.
```

Replace with, Branch A:

```markdown
- A client with no credentials starts at that sizing and, when its first send confirms, takes the refill the Epic 9 soak measured (`### Rate limits` in the Epic 9 section below), keeping Basic's depth.
```

or, Branch B:

```markdown
- A client with no credentials keeps that sizing, which the Epic 9 soak confirmed (`### Rate limits` in the Epic 9 section below).
```

- [ ] **Step 6: Commit**

Branch A:

```bash
git commit -m "feat(kalshi): size the unsigned Read bucket from the soak, a tenth below the clean rate (Epic 9, Story 7)" -- polyoxide-kalshi/src/predictions/throttle.rs docs/specs/kalshi/OBSERVED.md
```

Branch B:

```bash
git commit -m "test(kalshi): the unsigned soak confirmed Basic's sizing, pinned (Epic 9, Story 7)" -- polyoxide-kalshi/src/predictions/throttle.rs docs/specs/kalshi/OBSERVED.md
```

### Task 7.6: A keyed client's public reads

**Files:**
- Modify: `docs/specs/kalshi/OBSERVED.md`
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:11-13` (the move branch only)
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:29-63` (the move branch only)
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:66-76` (the move branch only)
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:147-150` (the move branch only)
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:161-163` (the move branch only)
- Modify: `polyoxide-kalshi/tests/mock_predictions_signed.rs:608-643` (the move branch only, under spec §13's third exception)
- Test: `polyoxide-kalshi/tests/mock_predictions_signed.rs` (the move branch only)

Line numbers of `throttle.rs` are those of the file before Task 7.5, which added lines above them; find each block by its text.

Choose the branch from the verdict in `OBSERVED.md`'s signed-bucket paragraph (Task 7.4). `follows` takes the stay branch, `does not follow` the move branch, and `inconclusive` was settled with the owner in Task 7.4: if the owner's answer was to leave the keyed client as it is, take the stay branch and say in the record that the probe was inconclusive.

#### The stay branch

A signed public read draws on the key's bucket, which is what `Predictions` does today, so no code changes. The existing `a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse` already pins that a keyed client's public read is charged to the account's resized bucket, so no test is added.

- [ ] **Step 1: Record the decision**

Insert the paragraph directly above the subsection's own anchor.

Current:

```markdown
<!-- rate limits end -->
```

Replace with:

````markdown
**Decision, a keyed client's public reads.** They stay charged to the key's bucket, as `Predictions` charges them today: <the public read throttled where the key's bucket empties (the probe above) | the probe was inconclusive and the owner kept the keyed client as it is, <date>>. `a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse` in `tests/mock_predictions_signed.rs` pins that a keyed client's public read is admitted by the account's resized bucket.

<!-- rate limits end -->
````

- [ ] **Step 2: Commit**

```bash
git commit -m "docs(kalshi): a keyed client's public reads stay on the key's bucket (Epic 9, Story 7)" -- docs/specs/kalshi/OBSERVED.md
```

Then go on to Task 7.7; the move branch is not taken.

#### The move branch

A signed public read does not draw on the key's bucket, so a keyed client must not spend the key's tokens on them. The throttle gets a second Read bucket, sized as an unsigned client's (`BASIC_READ_CAPACITY` tokens refilling `UNSIGNED_READ_REFILL` a second), and a keyed client's confirmation turns a split on: a request for any path but the routes that need the key (`GET /account/limits`) is charged to the second bucket, and the key's bucket serves the routes that need the key. The two buckets share the one hold, so a `429` still holds every request. An anonymous client never splits.

- [ ] **Step 1: Confirm the owner's exception covers the change**

The owner pre-granted this branch's one edit to existing tests in spec §13, as its third exception (2026-10-21), so there is nothing to ask. Run:

```bash
grep -n -A5 'A third (2026-10-21, drafting)' docs/superpowers/specs/2026-10-19-epic-9-kalshi-published-design.md
```

Expected: the exception's text, naming `a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse` and `the_bucket_takes_the_capacity_the_account_reports`, which Story 7 rewrites to charge a signed account route so they still pin the key's bucket, with the move's own tests appended. Those two tests are the only existing test bodies this story changes, and Step 5 changes them to `GET /account/limits`, the route that does draw on the key's bucket. The grant is the spec's, so this branch does not stop for an answer.

- [ ] **Step 2: Write the failing tests**

Append to the end of `polyoxide-kalshi/tests/mock_predictions_signed.rs`, after `the_captured_limits_fit_the_type_in_both_directions`. The first test fails on today's code because the 700-token read is admitted by the key's 1200 tokens; the second because the key's bucket, holding one read, would make the second status read wait ten seconds.

```rust
/// `/account/endpoint_costs` listing the limits route at 700 tokens, more than Basic's Read bucket
/// holds.
const COSTLY_LIMITS: &str = r#"{"default_cost":10,"endpoint_costs":[{"method":"GET","path":"/trade-api/v2/account/limits","cost":700}]}"#;

#[tokio::test]
async fn a_keyed_clients_public_read_is_charged_to_the_unsigned_bucket_not_the_keys() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(1200, 1200);
    let bodies = [
        (COSTS, COSTLY_STATUS),
        (LIMITS, limits.as_str()),
        (STATUS, STATUS_BODY),
    ];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    // The key's bucket holds 1200 tokens, so a 700-token read would fit it. A public read is
    // charged to the unsigned bucket, which holds Basic's 600.
    let refused = refusal(within(client.exchange().status().send()).await);

    assert_eq!((refused.units, refused.capacity), (700, 600));
    assert!(seen.to(STATUS).is_empty(), "a refused cost was sent");
}

#[tokio::test]
async fn a_keyed_clients_public_reads_do_not_spend_the_keys_tokens() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    // A key whose bucket holds one read and refills a token a second: a second read charged to it
    // would wait ten seconds.
    let limits = limits_body(10, 1);
    let bodies = [
        (COSTS, NO_COSTS),
        (LIMITS, limits.as_str()),
        (STATUS, STATUS_BODY),
    ];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    for read in 1..=3 {
        tokio::time::timeout(Duration::from_secs(3), client.exchange().status().send())
            .await
            .unwrap_or_else(|_| panic!("read {read} waited on the key's bucket"))
            .unwrap();
    }

    assert_eq!(seen.to(STATUS).len(), 3);
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_signed -- a_keyed_clients_public`

Expected: both fail. `a_keyed_clients_public_read_is_charged_to_the_unsigned_bucket_not_the_keys` panics with `expected a refusal, got a success`, and `a_keyed_clients_public_reads_do_not_spend_the_keys_tokens` panics with `read 2 waited on the key's bucket`.

- [ ] **Step 4: Implement**

Five edits to `polyoxide-kalshi/src/predictions/throttle.rs`. Neither `mod.rs` nor `request.rs` changes: the throttle reads the request's path (`RequestMeta::path`, without its query) and decides.

The split needs an atomic flag. Current (lines 11-13):

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
```

Replace with:

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
```

The throttle holds the second bucket and the flag. Current (lines 29-63):

```rust
/// Charges each request its cost on the Read layer, and holds every request when the retry policy
/// says to.
///
/// The bucket is [provisional](CapacityBucket::provisional): a cost above its capacity waits for
/// the sizing to be confirmed or replaced instead of being refused. The hold is unbounded, since
/// every hold this module's policy sets is the loop's own `retry_delay(0)`, which
/// `RetryConfig::max_backoff_ms` bounds.
///
/// A clone shares the bucket and the hold.
#[derive(Debug, Clone)]
pub(crate) struct PredictionsThrottle {
    read: CapacityBucket,
    hold: Hold,
}

impl PredictionsThrottle {
    /// A throttle with a full Read bucket at Basic's sizing.
    pub(crate) fn new() -> Self {
        let hold = Hold::unbounded();
        Self {
            read: CapacityBucket::provisional(
                READ,
                BASIC_READ_CAPACITY,
                BASIC_READ_REFILL,
                hold.clone(),
            ),
            hold,
        }
    }

    /// The Read bucket, for resizing or confirming it in place.
    pub(crate) fn bucket(&self) -> &CapacityBucket {
        &self.read
    }
}
```

Replace with:

```rust
/// Charges each request its cost on the Read layer, and holds every request when the retry policy
/// says to.
///
/// The bucket is [provisional](CapacityBucket::provisional): a cost above its capacity waits for
/// the sizing to be confirmed or replaced instead of being refused. The hold is unbounded, since
/// every hold this module's policy sets is the loop's own `retry_delay(0)`, which
/// `RetryConfig::max_backoff_ms` bounds.
///
/// A keyed client's public reads draw on a second bucket, sized as an unsigned client's, once its
/// confirmation turns the split on: a signed public read does not draw on the key's bucket (the
/// signed probe of `docs/specs/kalshi/OBSERVED.md`), so it must not spend the key's tokens. Both
/// buckets share the one hold.
///
/// A clone shares the buckets, the split and the hold.
#[derive(Debug, Clone)]
pub(crate) struct PredictionsThrottle {
    read: CapacityBucket,
    public: CapacityBucket,
    split: Arc<AtomicBool>,
    hold: Hold,
}

/// The paths that need the key. Every other path is a public read.
const PRIVATE_PATHS: [&str; 1] = [crate::predictions::request::routes::ACCOUNT_LIMITS.template];

impl PredictionsThrottle {
    /// A throttle with a full Read bucket at Basic's sizing and a public bucket at the unsigned
    /// client's.
    pub(crate) fn new() -> Self {
        let hold = Hold::unbounded();
        Self {
            read: CapacityBucket::provisional(
                READ,
                BASIC_READ_CAPACITY,
                BASIC_READ_REFILL,
                hold.clone(),
            ),
            public: CapacityBucket::provisional(
                READ,
                BASIC_READ_CAPACITY,
                UNSIGNED_READ_REFILL,
                hold.clone(),
            ),
            split: Arc::new(AtomicBool::new(false)),
            hold,
        }
    }

    /// The Read bucket, for resizing or confirming it in place.
    pub(crate) fn bucket(&self) -> &CapacityBucket {
        &self.read
    }

    /// The bucket a request for `path` is charged to: the public one once the split is on and
    /// `path` is not a route that needs the key, else the Read bucket.
    fn bucket_for(&self, path: &str) -> &CapacityBucket {
        if self.split.load(Ordering::Acquire) && !PRIVATE_PATHS.contains(&path) {
            &self.public
        } else {
            &self.read
        }
    }

    /// The capacity of the bucket a request for `path` is charged to.
    pub(crate) fn capacity_for(&self, path: &str) -> u32 {
        self.bucket_for(path).capacity()
    }

    /// From now on a public read is charged to the public bucket, confirmed at its sizing, and
    /// not to the Read bucket.
    pub(crate) fn split_public_reads(&self) {
        self.public.confirm();
        self.split.store(true, Ordering::Release);
    }
}
```

`acquire` charges the bucket the request's path belongs to. Current (lines 66-76):

```rust
    /// Waits out the hold, charges each cost on the Read layer (a cost on any other layer is not
    /// this throttle's), and waits out the hold again, since one set while the bucket made this
    /// request wait for tokens still applies (AD-23).
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.hold.wait().await;
        let mut charge = Charge::none();
        for cost in meta.costs {
            if cost.layer != READ {
                continue;
            }
            self.read.acquire(cost.units, cost.exact).await?;
```

Replace with:

```rust
    /// Waits out the hold, charges each cost on the Read layer (a cost on any other layer is not
    /// this throttle's) to the bucket the request's path belongs to, and waits out the hold
    /// again, since one set while the bucket made this request wait for tokens still applies
    /// (AD-23).
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.hold.wait().await;
        let mut charge = Charge::none();
        let bucket = self.bucket_for(meta.path);
        for cost in meta.costs {
            if cost.layer != READ {
                continue;
            }
            bucket.acquire(cost.units, cost.exact).await?;
```

A keyed client's confirmation turns the split on before it reads anything. Current (lines 147-150):

```rust
    async fn confirm(&self) {
        self.read_costs().await;
        self.read_limits().await;
    }
```

Replace with:

```rust
    async fn confirm(&self) {
        if self.auth.is_some() {
            // A signed public read does not draw on the key's bucket (the signed probe in
            // `docs/specs/kalshi/OBSERVED.md`), so a keyed client charges its public reads to
            // the public bucket from here on, this confirmation's costs read included.
            self.throttle.split_public_reads();
        }
        self.read_costs().await;
        self.read_limits().await;
    }
```

The confirmation's own reads are capped at the capacity of the bucket they are charged to, which is the public bucket's for the costs read. Current (lines 161-163):

```rust
        let mut cost = self.read_cost(route);
        cost.units = cost.units.min(self.throttle.bucket().capacity());
        let request = Request::new(self.http.clone(), route.path(&[])).with_cost(cost);
```

Replace with:

```rust
        let path = route.path(&[]);
        let mut cost = self.read_cost(route);
        cost.units = cost.units.min(self.throttle.capacity_for(&path));
        let request = Request::new(self.http.clone(), path).with_cost(cost);
```

- [ ] **Step 5: Re-point the two tests the split makes wrong, under spec §13's third exception**

First see them fail. Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_signed`

Expected: the two new tests pass, and exactly two old ones fail. `a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse` panics on its `unwrap` with an `Api(Refused(…))` error that names units 700 and capacity 600, and `the_bucket_takes_the_capacity_the_account_reports` fails its `assert_eq!((refused.units, refused.capacity), (700, 650))` with `left: (700, 600)`. Each used the public status route as the vehicle for the key's bucket, and the vehicle is now charged to the public bucket.

Re-point both at `GET /account/limits`, which draws on the key's bucket: the confirmation reads the limits at a capped cost and resizes the bucket, and the route's own send is then charged at the listed 700. The premise of each test is unchanged. Current (lines 608-643):

```rust
#[tokio::test]
async fn a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(1200, 1200);
    let bodies = [
        (COSTS, COSTLY_STATUS),
        (LIMITS, limits.as_str()),
        (STATUS, STATUS_BODY),
    ];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    within(client.exchange().status().send()).await.unwrap();

    assert_eq!(seen.to(STATUS).len(), 1, "the 700-token send went out");
}

#[tokio::test]
async fn the_bucket_takes_the_capacity_the_account_reports() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(650, 100);
    let bodies = [
        (COSTS, COSTLY_STATUS),
        (LIMITS, limits.as_str()),
        (STATUS, STATUS_BODY),
    ];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    let refused = refusal(within(client.exchange().status().send()).await);

    assert_eq!((refused.units, refused.capacity), (700, 650));
    assert!(seen.to(STATUS).is_empty(), "a refused cost was sent");
}
```

Replace with:

```rust
#[tokio::test]
async fn a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(1200, 1200);
    let bodies = [(COSTS, COSTLY_LIMITS), (LIMITS, limits.as_str())];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    within(client.account().limits().send()).await.unwrap();

    assert_eq!(
        seen.to(LIMITS).len(),
        2,
        "the confirmation's read and the 700-token send both went out"
    );
}

#[tokio::test]
async fn the_bucket_takes_the_capacity_the_account_reports() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(650, 100);
    let bodies = [(COSTS, COSTLY_LIMITS), (LIMITS, limits.as_str())];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    let refused = refusal(within(client.account().limits().send()).await);

    assert_eq!((refused.units, refused.capacity), (700, 650));
    assert_eq!(
        seen.to(LIMITS).len(),
        1,
        "only the confirmation's read went out, not the refused cost"
    );
}
```

- [ ] **Step 6: Run everything**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions`

Expected: every suite passes, the crate's unit tests (the throttle's, which pin Basic's 600 and an anonymous client's refusal at it, included) and `mock_predictions_signed`'s, with the two new tests and the two re-pointed ones.

Then `nix develop --command cargo fmt --all -- --check` prints nothing and `nix develop --command cargo clippy -p polyoxide-kalshi --no-default-features --features predictions --all-targets -- -D warnings` prints no warning.

- [ ] **Step 7: Record the decision, and commit**

Insert the paragraph directly above the subsection's own anchor.

Current:

```markdown
<!-- rate limits end -->
```

Replace with:

````markdown
**Decision, a keyed client's public reads.** A signed public read does not draw on the key's bucket (the probe above), so a keyed client charges every route but `GET /account/limits` to a second Read bucket, sized as an unsigned client's (Basic's 600 tokens refilling `UNSIGNED_READ_REFILL` a second), and keeps the key's bucket for the routes that need the key. The split starts when the client's first send confirms, and the confirmation's own costs read is its first public read. An anonymous client never splits. The owner's third exception in the spec's §13 (2026-10-21) allowed the two existing tests that used the public status route to show the key's bucket (`a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse` and `the_bucket_takes_the_capacity_the_account_reports`) to be re-pointed at `GET /account/limits`.

<!-- rate limits end -->
````

```bash
git commit -m "feat(kalshi): a keyed client's public reads draw on the unsigned bucket, not the key's (Epic 9, Story 7)" -- polyoxide-kalshi/src/predictions/throttle.rs polyoxide-kalshi/tests/mock_predictions_signed.rs docs/specs/kalshi/OBSERVED.md
```

### Task 7.7: The story's gates and the touch set

**Files:** none are edited here; a gate that fails is fixed in the file it names, in a new commit.

- [ ] **Step 1: Run the profile's story gates, in order**

The packages are `polyoxide-kalshi` and what depends on it, which is nothing yet: Story 8 gives the umbrella its dependency.

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings
nix develop --command cargo test -p polyoxide-kalshi --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
```

Expected: each exits 0. The `cargo test` run includes the example's 25 unit tests (its stanza sets `test = true`) and `mock_predictions_signed`'s. `live_unwraps.py` and `test_body_diff.py` have nothing to say about this story: it edits no live suite, and `polyoxide-kalshi` has no test at `v0.40.0` to change.

- [ ] **Step 2: Run the story's Verify line and the touch-set gate**

```bash
nix develop --command cargo build -p polyoxide-kalshi --features predictions --example read_soak
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions
bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh
git status --short
```

Expected: the build and the tests pass, the touch-set gate prints nothing and exits 0, and `git status --short` shows nothing: no log, no scratch file, and `Cargo.lock` unchanged (the `soak` feature adds an edge to a package the lock already holds). If cargo did change `Cargo.lock`, commit it with `git commit -m "chore(kalshi): Cargo.lock for the soak feature (Epic 9, Story 7)" -- Cargo.lock`.

- [ ] **Step 3: Confirm no rule in CLAUDE.md states the sizing**

Run: `grep -n -i "unsigned\|anonymous\|Read bucket" CLAUDE.md`

Expected: no line states Kalshi's Read sizing as a rule, so AD-21 asks for no CLAUDE.md edit in this story's commits.

- [ ] **Step 4: Commit any formatting the gates changed**

If `cargo fmt --all` rewrote a file of this story, commit it by pathspec, naming the story; otherwise there is nothing to commit.

```bash
git commit -m "style(kalshi): rustfmt for Story 7 (Epic 9, Story 7)" -- polyoxide-kalshi/examples/read_soak.rs polyoxide-kalshi/src/predictions/throttle.rs polyoxide-kalshi/tests/mock_predictions_signed.rs
```

## Story 8: Publish `polyoxide-kalshi` and re-export it from the umbrella

- **Developer:** intermediate-dev
- **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/README.md`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/README.md`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`, `polyoxide/examples/venue_neutral.rs`, `polyoxide/tests/venue_neutral.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`, `ci/dep-allowlists/polyoxide@kalshi-predictions.txt`, `ci/dep-allowlists/polyoxide@kalshi-predictions-ws.txt`, `.github/scripts/tests/test_dep_allowlists.py`, `.github/scripts/tests/test_ci_workflow.py`, `.github/workflows/ci.yml`, `README.md`, `CLAUDE.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml`
- **Interfaces:** exposes "Story 8 → Story 9"; uses Story 3's `Candles` impl, Story 5's and Story 6's public names for the prelude, and the Cargo.toml anchors
- **Verify:** `python3 scripts/publish_order.py check-manifests && cargo publish --workspace --dry-run --no-verify --locked && cargo test -p polyoxide --features full && cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions && cargo hack check -p polyoxide -p polyoxide-kalshi --each-feature --no-dev-deps && cargo test -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_ci_workflow.py tests/test_gen_registry.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** deep (publishes a crate and changes CI gates)

The crate stops being a walking skeleton and becomes a member of the release: its manifest drops `publish = false` and its workspace pin gains a version, the umbrella gains the optional dependency and the features `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws` (all three in `full`), `polyoxide::kalshi`, `parse_key`'s Kalshi arm, the prelude block with spec D13's aliases and the `const _` pins. The venue-neutral example gains a Kalshi arm in `main` with no `cfg`, so Kalshi's own `#[path]` include of it keeps compiling, and Kalshi's `polyoxide` dev-dependency gains `kalshi-predictions` (a path-only dev-dependency cycle that cargo allows and publishing strips). The umbrella's two Kalshi allowlists, their rules and `MUST_BUILD` rows land, the real-tree check of `full` loses any room for an unpublished Kalshi, the registry is regenerated, the comment above `ci.yml`'s umbrella header pins stops calling Kalshi unpublished (a comment only, no CI step changes), and the package, cargo-hack and header-pin steps are run. Hand-written passages of CLAUDE.md that still call the crate a walking skeleton are Story 9's: this story touches CLAUDE.md, README.md, `docs/specs/INDEX.md`, `SELF-HEALING.md` and the nightly workflow only through `gen_registry.py --write`. No code here holds one of the profile's landmine markers.

Every task commits the files `python3 scripts/gen_registry.py --write` rewrote, and nothing else by hand. Commands that read Rust formatting are exact: when `cargo fmt --all -- --check` reports a diff in a file the task wrote, run `nix develop --command cargo fmt --all` and keep the result.

### Task 8.1: Publish the crate and add the umbrella's features

The manifests change together, because a published venue crate the umbrella does not depend on fails the existing header-pin check under `full` (the umbrella forgot it). The real-tree test that Step 1 appends passes `umbrella_missing` an empty `unpublished` set, so no exemption for Kalshi remains: the existing tests derive their `unpublished` set from cargo metadata and lapse by themselves, and this one fails if `publish = false` ever returns or a Kalshi feature drops out of `full`.

**Files:**
- Modify: `Cargo.toml:27`, `Cargo.toml:277`, `Cargo.toml:290`
- Modify: `polyoxide-kalshi/Cargo.toml` (the `publish = false` and `description` lines, and the `readme` line under `[package.metadata.polyoxide]`, each by its quoted text)
- Modify: `polyoxide/Cargo.toml:9`, `polyoxide/Cargo.toml:33-34`, `polyoxide/Cargo.toml:48-49`, `polyoxide/Cargo.toml:57-58`, `polyoxide/Cargo.toml:100`
- Test: `.github/scripts/tests/test_ci_workflow.py`
- Generated: `Cargo.lock`, and the files `gen_registry.py --write` rewrites

- [ ] **Step 1: Write the failing test**

Append to `.github/scripts/tests/test_ci_workflow.py`, after its last test. The anchor is that test's last line:

```python
    assert sorted(_caches("removals")) == ["target/semver-checks/git-*"]
```

Replace it with itself and the new test:

```python
    assert sorted(_caches("removals")) == ["target/semver-checks/git-*"]


def test_kalshi_is_published_and_the_umbrellas_full_turns_on_its_modules() -> None:
    """Kalshi is a published venue crate the umbrella builds, so nothing exempts it from `full`.
    `unpublished` is left empty: a `publish = false` that came back, a Kalshi feature `full`
    forgot, or an umbrella that stopped depending on the crate fails here."""
    metadata = _load_publish_order().cargo_metadata(REPO / "Cargo.toml")
    packages = {p["name"]: p for p in metadata["packages"]}
    kalshi, umbrella = packages["polyoxide-kalshi"], packages[UMBRELLA]
    assert kalshi.get("publish") != [], "polyoxide-kalshi is still `publish = false`"
    built = {dep["name"] for dep in umbrella["dependencies"] if dep["kind"] is None}
    assert "polyoxide-kalshi" in built, f"the umbrella does not depend on Kalshi: {sorted(built)}"
    modules = http_modules(metadata)
    assert ("polyoxide-kalshi", "predictions") in modules, modules
    enabled = umbrella_enables(umbrella["features"], "full")
    assert {"polyoxide-kalshi/predictions", "polyoxide-kalshi/predictions-ws"} <= enabled, enabled
    assert umbrella_missing(umbrella, modules, enabled) == []
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py -q -k kalshi_is_published'`
Expected: `1 failed`, with `AssertionError: polyoxide-kalshi is still `publish = false``.

- [ ] **Step 3: Implement**

Edit `Cargo.toml` (the workspace root). The pin gains a version and `default-features = false`, as `polyoxide-binance`'s has (the release commit raises every pin to `0.42.0`, spec §12). Replace:

```toml
polyoxide-kalshi = { path = "polyoxide-kalshi" }
```

with:

```toml
polyoxide-kalshi = { path = "polyoxide-kalshi", version = "0.41.1", default-features = false }
```

In the same file, the generated registry reads the Kalshi mirror's two sentences. Replace:

```toml
description = "Event-contract market data and its order-book stream. A walking skeleton, in a crate that is not published"
```

with:

```toml
description = "Event-contract market data and its order-book, ticker and trade streams"
```

and replace:

```toml
covers = "Trade API WebSocket (15 channels); `orderbook_delta` is implemented"
```

with:

```toml
covers = "Trade API WebSocket (15 channels); `orderbook_delta`, `ticker` and `trade` are implemented"
```

Edit `polyoxide-kalshi/Cargo.toml`. Replace:

```toml
description = "Rust client library for Kalshi's event-contract market data and its order-book stream (a walking skeleton, not published)"
publish = false
```

with:

```toml
description = "Rust client library for Kalshi's event-contract market data and its order-book, ticker and trade streams"
```

and replace the registration's `readme` line:

```toml
readme = "Client library for Kalshi's APIs, one feature per module: `predictions` (event-contract market data, with its order-book stream under `predictions-ws`); a walking skeleton, not published"
```

with:

```toml
readme = "Client library for Kalshi's APIs, one feature per module: `predictions` (event-contract market data, with its streams under `predictions-ws`); re-exported by the umbrella as `polyoxide::kalshi`"
```

Edit `polyoxide/Cargo.toml`. The description:

```toml
description = "Multi-venue Rust toolkit: Polymarket (CLOB, Gamma, Data, Relay, Perps, RTDS, Sports) and Binance USDⓈ-M futures, each venue behind features named for it"
```

becomes:

```toml
description = "Multi-venue Rust toolkit: Polymarket (CLOB, Gamma, Data, Relay, Perps, RTDS, Sports), Binance USDⓈ-M futures and Kalshi's event-contract market data, each venue behind features named for it"
```

The features. Replace:

```toml
binance-usdm = ["dep:polyoxide-binance", "polyoxide-binance/usdm", "polyoxide-venue/market"]
binance-usdm-ws = ["binance-usdm", "polyoxide-binance/usdm-ws"]
```

with:

```toml
binance-usdm = ["dep:polyoxide-binance", "polyoxide-binance/usdm", "polyoxide-venue/market"]
binance-usdm-ws = ["binance-usdm", "polyoxide-binance/usdm-ws"]
kalshi = ["kalshi-predictions"]
kalshi-predictions = ["dep:polyoxide-kalshi", "polyoxide-kalshi/predictions", "polyoxide-venue/market"]
kalshi-predictions-ws = ["kalshi-predictions", "polyoxide-kalshi/predictions-ws"]
```

`full`. Replace:

```toml
    "binance-usdm-ws",
    "keychain",
]
```

with:

```toml
    "binance-usdm-ws",
    "kalshi",
    "kalshi-predictions",
    "kalshi-predictions-ws",
    "keychain",
]
```

The optional dependency. Replace:

```toml
# Binance's modules, under the `binance-<module>` features.
polyoxide-binance = { workspace = true, optional = true }
```

with:

```toml
# Binance's modules, under the `binance-<module>` features.
polyoxide-binance = { workspace = true, optional = true }
# Kalshi's modules, under the `kalshi-<module>` features.
polyoxide-kalshi = { workspace = true, optional = true }
```

The registration line. Replace:

```toml
readme = "The multi-venue umbrella: `polyoxide::polymarket` and `polyoxide::binance` behind features named for each venue and module, `polyoxide::venue` with the market-data traits, `polyoxide::parse_key` and a `prelude`"
```

with:

```toml
readme = "The multi-venue umbrella: `polyoxide::polymarket`, `polyoxide::binance` and `polyoxide::kalshi` behind features named for each venue and module, `polyoxide::venue` with the market-data traits, `polyoxide::parse_key` and a `prelude`"
```

Refresh the lock and prove the features compile on their own. The umbrella's entry in `Cargo.lock` gains `polyoxide-kalshi`, and nothing else moves:

Run: `nix develop --command cargo check -p polyoxide --no-default-features --features kalshi-predictions-ws`
Expected: `Finished`. Then `git diff --stat Cargo.lock` shows one hunk, the `polyoxide` package gaining `"polyoxide-kalshi"` in its `dependencies`.

The umbrella's new optional dependency closes a cycle with Kalshi's existing path-only dev-dependency on the umbrella. If cargo refuses the cycle, stop and ask: spec D13 expects cargo to allow it.

Regenerate the registry:

Run: `nix develop --command python3 scripts/gen_registry.py --write`
Expected: it names the regions it rewrote. `README.md`'s crate table loses "a walking skeleton, not published" and the umbrella's row names Kalshi, `docs/specs/INDEX.md`'s Kalshi rows lose the same words, and `CLAUDE.md`'s crate list and "Today's order" place `polyoxide-kalshi` between `polyoxide-ws` and the unpublished crates' tail by publish order, with the umbrella's feature list gaining the three features.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py tests/test_gen_registry.py tests/test_dep_allowlists.py -q'`
Expected: all pass, `test_kalshi_is_published_and_the_umbrellas_full_turns_on_its_modules` among them, and no allowlist differs (the umbrella's existing files and Kalshi's two are unchanged, since the new dependency is optional).

Run: `nix develop --command python3 scripts/publish_order.py check-manifests && nix develop --command python3 scripts/gen_registry.py --check`
Expected: both exit 0.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide -p polyoxide-kalshi --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(kalshi): publish polyoxide-kalshi and add the umbrella's kalshi features (Epic 9, Story 8)" -- Cargo.toml Cargo.lock polyoxide-kalshi/Cargo.toml polyoxide/Cargo.toml .github/scripts/tests/test_ci_workflow.py README.md CLAUDE.md SELF-HEALING.md docs/specs/INDEX.md docs/ARCHITECTURE.md
```

If `git status --short` then lists another file `--write` rewrote, add it to the pathspec and amend nothing: commit it with the same command before moving on.

### Task 8.2: `polyoxide::kalshi`, `parse_key`'s Kalshi arm, the prelude and the pins

The umbrella's crate root holds `venue`, `polymarket`, `binance`, `parse_key` and the prelude, and gains `kalshi`. Each passthrough and each prelude block gets a `const _` that names one type through the path it opens, outside `cfg(test)`, so `cargo hack --each-feature` fails a re-export whose `cfg` leaves a feature out. `parse_key` reads `kalshi.predictions:<TICKER>:<yes|no>` through the crate's own `predictions::key::parse`. The prelude names are spec D13's: `Credentials` is aliased as `KalshiCredentials` because a bare `Credentials` already names the CLOB's, and the socket's `Event` and `Books` are aliased by their venue (`PredictionsEvent`, `PredictionsBooks`).

**Files:**
- Modify: `polyoxide/src/lib.rs:6-12`, `:20-21`, `:125`, `:127-130`, `:144-146`, `:153-160`, `:170-174`, `:218-222`, `:247-248`, `:270-271`, `:275-298`
- Modify: `polyoxide/Cargo.toml` (the `parse_key` test stanza)
- Test: `polyoxide/tests/parse_key.rs`

- [ ] **Step 1: Write the failing test**

In `polyoxide/tests/parse_key.rs`, add Kalshi's key module to the imports. Replace:

```rust
use polyoxide::binance::usdm::{self, types::Symbol};
```

with:

```rust
use polyoxide::binance::usdm::{self, types::Symbol};
use polyoxide::kalshi::predictions::key::{self as kalshi_key, Outcome, Ticker};
```

Append after the file's last test (`a_product_refuses_an_id_it_cannot_read`, which ends with `Err(KeyError::InvalidId { .. })` and `));` and `}`). The anchor is its last four lines:

```rust
    assert!(matches!(
        parse("polymarket.clob:0x1234"),
        Err(KeyError::InvalidId { .. })
    ));
}
```

Replace them with themselves and:

```rust
    assert!(matches!(
        parse("polymarket.clob:0x1234"),
        Err(KeyError::InvalidId { .. })
    ));
}

#[test]
fn kalshis_keys_come_from_its_own_constructors() {
    let ticker = Ticker::new("KXTEST-26OCT18-T50.5").unwrap();
    // The product canonicalizes: the ticker is uppercased and the outcome lowercased.
    let raw_yes = "kalshi.predictions:kxtest-26oct18-t50.5:YES";
    let yes = parse(raw_yes).unwrap();
    assert_eq!(yes, kalshi_key::key(&ticker, Outcome::Yes));
    let text = yes.to_string();
    assert_eq!(text, "kalshi.predictions:KXTEST-26OCT18-T50.5:yes");
    let raw_no = "kalshi.predictions:KXTEST-26OCT18-T50.5:no";
    let no = parse(raw_no).unwrap();
    assert_eq!(no, kalshi_key::key(&ticker, Outcome::No));
    let text = no.to_string();
    assert_eq!(text, "kalshi.predictions:KXTEST-26OCT18-T50.5:no");
}

#[test]
fn kalshis_other_products_and_unreadable_ids_are_refused() {
    // Kalshi's other exchange is not covered, and its name is left free.
    let parsed = parse("kalshi.margin:KXTEST-26OCT18-T50.5:yes");
    assert!(
        matches!(parsed, Err(KeyError::UnknownProduct { .. })),
        "{parsed:?}"
    );
    for text in [
        "kalshi.predictions:KXTEST-26OCT18-T50.5",
        "kalshi.predictions:KXTEST-26OCT18-T50.5:maybe",
        "kalshi.predictions:BAD/TICKER:yes",
    ] {
        let parsed = parse(text);
        assert!(
            matches!(parsed, Err(KeyError::InvalidId { .. })),
            "{text}: {parsed:?}"
        );
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions --test parse_key`
Expected: a compile error, `error[E0433]: failed to resolve: could not find `kalshi` in `polyoxide``.

- [ ] **Step 3: Implement**

Edit `polyoxide/src/lib.rs`, in this order.

The module docs. Replace:

```rust
//! - `polyoxide::venue` is `polyoxide-venue`, the vocabulary every venue
//!   shares, such as the error classes and the `Classify` trait. It is always
//!   on. With any module that implements them (`polymarket-clob`,
//!   `polymarket-perps`, `binance-usdm`) it also holds the market-data traits,
//!   `MarketData` and its capabilities, which read every venue the same way:
//!   `examples/venue_neutral.rs` reads the best bid and ask of the CLOB, perps
//!   and USDⓈ-M with one function.
```

with:

```rust
//! - `polyoxide::venue` is `polyoxide-venue`, the vocabulary every venue
//!   shares, such as the error classes and the `Classify` trait. It is always
//!   on. With any module that implements them (`polymarket-clob`,
//!   `polymarket-perps`, `binance-usdm`, `kalshi-predictions`) it also holds
//!   the market-data traits, `MarketData` and its capabilities, which read
//!   every venue the same way: `examples/venue_neutral.rs` reads the best bid
//!   and ask of the CLOB, perps, USDⓈ-M and Kalshi's Predictions with one
//!   function.
```

and replace:

```rust
//! - `polyoxide::binance` is `polyoxide-binance`, under `binance` (its
//!   defaults: `usdm`), `binance-usdm` or `binance-usdm-ws`.
```

with:

```rust
//! - `polyoxide::binance` is `polyoxide-binance`, under `binance` (its
//!   defaults: `usdm`), `binance-usdm` or `binance-usdm-ws`.
//! - `polyoxide::kalshi` is `polyoxide-kalshi`, under `kalshi` (its
//!   defaults: `predictions`), `kalshi-predictions` or `kalshi-predictions-ws`.
```

The re-export. Replace:

```rust
pub use polyoxide_binance as binance;
```

with:

```rust
pub use polyoxide_binance as binance;

/// Kalshi's venue crate, `polyoxide-kalshi`: one module per
/// `kalshi-<module>` feature.
#[cfg(any(
    feature = "kalshi",
    feature = "kalshi-predictions",
    feature = "kalshi-predictions-ws"
))]
pub use polyoxide_kalshi as kalshi;
```

`parse_key`'s docs. Replace:

```rust
/// The key `raw` names, made by the constructors of its product, among the
/// venues and modules this build enables: Polymarket's `clob` (with
/// `polymarket-clob`, `-gamma` or `-data`) and `perps` (`polymarket-perps`),
/// and Binance's `usdm` (`binance-usdm`).
```

with:

```rust
/// The key `raw` names, made by the constructors of its product, among the
/// venues and modules this build enables: Polymarket's `clob` (with
/// `polymarket-clob`, `-gamma` or `-data`) and `perps` (`polymarket-perps`),
/// Binance's `usdm` (`binance-usdm`), and Kalshi's `predictions`
/// (`kalshi-predictions`).
```

and, below the Binance doctest, add Kalshi's. Replace:

```rust
/// # #[cfg(not(feature = "binance-usdm"))]
/// # fn main() {}
/// ```
```

with:

```rust
/// # #[cfg(not(feature = "binance-usdm"))]
/// # fn main() {}
/// ```
///
/// ```
/// # #[cfg(feature = "kalshi-predictions")]
/// # fn main() -> Result<(), polyoxide::venue::KeyError> {
/// use polyoxide::venue::RawKey;
///
/// let raw = RawKey::parse("kalshi.predictions:kxtest-26oct18-t50.5:yes")?;
/// assert_eq!(
///     polyoxide::parse_key(&raw)?.to_string(),
///     "kalshi.predictions:KXTEST-26OCT18-T50.5:yes"
/// );
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "kalshi-predictions"))]
/// # fn main() {}
/// ```
```

`parse_key`'s `cfg` and body. Replace:

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-perps",
    feature = "binance-usdm"
))]
pub fn parse_key(raw: &venue::RawKey) -> Result<venue::MarketKey, venue::KeyError> {
```

with:

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "kalshi-predictions"
))]
pub fn parse_key(raw: &venue::RawKey) -> Result<venue::MarketKey, venue::KeyError> {
```

and replace:

```rust
    #[cfg(feature = "binance-usdm")]
    if raw.venue() == &binance::usdm::key::VENUE && raw.product() == &binance::usdm::key::PRODUCT {
        return binance::usdm::key::parse(raw);
    }
    Err(venue::KeyError::unknown(raw))
```

with:

```rust
    #[cfg(feature = "binance-usdm")]
    if raw.venue() == &binance::usdm::key::VENUE && raw.product() == &binance::usdm::key::PRODUCT {
        return binance::usdm::key::parse(raw);
    }
    #[cfg(feature = "kalshi-predictions")]
    if raw.venue() == &kalshi::predictions::key::VENUE
        && raw.product() == &kalshi::predictions::key::PRODUCT
    {
        return kalshi::predictions::key::parse(raw);
    }
    Err(venue::KeyError::unknown(raw))
```

The prelude. Replace:

```rust
    #[cfg(feature = "binance-usdm-ws")]
    pub use polyoxide_binance::usdm::ws::{
        Event as UsdmEvent, StreamName, UsdmWs, UsdmWsBuilder, UsdmWsError,
    };
}
```

with:

```rust
    #[cfg(feature = "binance-usdm-ws")]
    pub use polyoxide_binance::usdm::ws::{
        Event as UsdmEvent, StreamName, UsdmWs, UsdmWsBuilder, UsdmWsError,
    };

    // Aliased: a bare `Credentials` is the CLOB's.
    #[cfg(feature = "kalshi-predictions")]
    pub use polyoxide_kalshi::predictions::{
        Credentials as KalshiCredentials, Predictions, PredictionsBuilder, PredictionsError,
    };

    // Aliased as the other venues' streams are: a bare `Event` or `Books` names no venue.
    #[cfg(feature = "kalshi-predictions-ws")]
    pub use polyoxide_kalshi::predictions::ws::{
        Books as PredictionsBooks, Event as PredictionsEvent, PredictionsWsBuilder,
        PredictionsWsError, SupervisedPredictionsWs,
    };
}
```

The passthrough pins. Replace:

```rust
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::binance::usdm::ws::UsdmWs) = drop;
```

with:

```rust
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::binance::usdm::ws::UsdmWs) = drop;
#[cfg(feature = "kalshi-predictions")]
const _: fn(crate::kalshi::predictions::Predictions) = drop;
#[cfg(feature = "kalshi-predictions-ws")]
const _: fn(crate::kalshi::predictions::ws::PredictionsWsBuilder) = drop;
```

The prelude pins. Replace:

```rust
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::prelude::UsdmWs) = drop;
```

with:

```rust
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::prelude::UsdmWs) = drop;
#[cfg(feature = "kalshi-predictions")]
const _: fn(crate::prelude::Predictions) = drop;
#[cfg(feature = "kalshi-predictions-ws")]
const _: fn(crate::prelude::PredictionsWsBuilder) = drop;
```

The market-data pin. Replace:

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-perps",
    feature = "binance-usdm"
))]
const _: fn(&crate::venue::DynMarketData<'static>) = |_| {};
```

with:

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "kalshi-predictions"
))]
const _: fn(&crate::venue::DynMarketData<'static>) = |_| {};
```

and the `parse_key` pin. Replace:

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-perps",
    feature = "binance-usdm"
))]
const _: fn(&crate::venue::RawKey) -> Result<crate::venue::MarketKey, crate::venue::KeyError> =
    crate::parse_key;
```

with:

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "kalshi-predictions"
))]
const _: fn(&crate::venue::RawKey) -> Result<crate::venue::MarketKey, crate::venue::KeyError> =
    crate::parse_key;
```

Edit `polyoxide/Cargo.toml`: the `parse_key` stanza gains the feature its tests now need. Replace:

```toml
# `parse_key` across both venues' products.
[[test]]
name = "parse_key"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
```

with:

```toml
# `parse_key` across the three venues' products.
[[test]]
name = "parse_key"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions --test parse_key`
Expected: 5 passed (the three existing tests and the two new ones).

Run: `nix develop --command cargo test -p polyoxide --features kalshi-predictions --doc`
Expected: passes, `parse_key`'s two doctests among them (the default `polymarket` features are on, which the README doctests need).

Each Kalshi feature builds alone, so a `cfg` that leaves one out fails here and not in CI:

Run: `nix develop --command bash -c 'for f in kalshi kalshi-predictions kalshi-predictions-ws; do cargo check -p polyoxide --no-default-features --features $f || exit 1; done'`
Expected: three `Finished`.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide --all-features'`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(umbrella): polyoxide::kalshi, parse_key's Kalshi arm and the prelude's Kalshi names (Epic 9, Story 8)" -- polyoxide/src/lib.rs polyoxide/Cargo.toml polyoxide/tests/parse_key.rs
```

### Task 8.3: The venue-neutral example reads Kalshi

The example's `main` gains a Kalshi arm with **no `cfg`**, named by fully qualified `polyoxide::kalshi::predictions::…` paths inside `main`. `polyoxide-kalshi/tests/venue_predictions.rs` includes the file by `#[path]`, and a `cfg(feature = "kalshi-…")` in it would be an `unexpected_cfgs` error under `-D warnings` there, where Kalshi declares no such feature. So Kalshi's `polyoxide` dev-dependency gains `kalshi-predictions`, which is why Step 2 fails in that suite before Step 3 and passes after it. The umbrella's mock suite gains Kalshi cases, appended: the same `best_bid_ask` and the same `classify` run unchanged over a Kalshi mock server, with literal prices from a book whose YES and NO sides are known.

**Files:**
- Modify: `polyoxide/examples/venue_neutral.rs:1-12`, `:92-105`
- Modify: `polyoxide/Cargo.toml` (the `venue_neutral` example and test stanzas)
- Modify: `polyoxide-kalshi/Cargo.toml` (the `polyoxide` dev-dependency)
- Test: `polyoxide/tests/venue_neutral.rs`

- [ ] **Step 1: Write the failing tests and the example's Kalshi arm**

In `polyoxide/tests/venue_neutral.rs`, add Kalshi's imports. Replace:

```rust
use polyoxide::polymarket::clob::{Clob, ClobBuilder};
```

with:

```rust
use polyoxide::kalshi::predictions::{
    key::{self as kalshi_key, Outcome, Ticker},
    Predictions,
};
use polyoxide::polymarket::clob::{Clob, ClobBuilder};
```

Append after the file's last test. The anchor is its last four lines:

```rust
    trades::<Usdm>();
    candles::<Clob>();
    funding::<Perps>();
    instruments::<Usdm>();
}
```

Replace them with themselves and:

```rust
    trades::<Usdm>();
    candles::<Clob>();
    funding::<Perps>();
    instruments::<Usdm>();
}

/// A ticker of this suite's own, for Kalshi's inline bodies.
const KALSHI_TICKER: &str = "KXTEST-26OCT18-T50.5";

/// A book as Kalshi serves it: each side's bids in ascending price, worst first. Two YES bids,
/// 0.40 and 0.45, and two NO bids, 0.50 and 0.52.
const KALSHI_BOOK: &str = r#"{"orderbook_fp":{"yes_dollars":[["0.4000","10.00"],["0.4500","5.00"]],"no_dollars":[["0.5000","7.00"],["0.5200","3.00"]]}}"#;

fn kalshi_book_path() -> String {
    format!("/trade-api/v2/markets/{KALSHI_TICKER}/orderbook")
}

fn kalshi_client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

fn kalshi_key_of(outcome: Outcome) -> MarketKey {
    kalshi_key::key(&Ticker::new(KALSHI_TICKER).unwrap(), outcome)
}

#[tokio::test]
async fn best_bid_ask_reads_kalshi_predictions_unchanged() {
    let mut server = Server::new_async().await;
    // A client's first send also reads `/account/endpoint_costs`, which this server leaves
    // unanswered: the confirmation never fails a send, and keeps the compiled costs.
    let mock = server
        .mock("GET", kalshi_book_path().as_str())
        .match_query(Matcher::Exact("depth=1".to_owned()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(KALSHI_BOOK)
        .expect(2)
        .create_async()
        .await;
    let kalshi_venue = DynMarketData::new_arc(kalshi_client(&server));

    // YES: its best bid, and the best NO bid priced as a YES ask (1 - 0.52).
    let yes = best_bid_ask(&kalshi_venue, &kalshi_key_of(Outcome::Yes))
        .await
        .unwrap();
    assert_eq!(yes, (Some(level("0.45", "5")), Some(level("0.48", "3"))));
    // NO is the same book read from the other side: its best bid is the NO bid, and its ask
    // is the best YES bid priced as a NO ask (1 - 0.45).
    let no = best_bid_ask(&kalshi_venue, &kalshi_key_of(Outcome::No))
        .await
        .unwrap();
    assert_eq!(no, (Some(level("0.52", "3")), Some(level("0.55", "5"))));
    mock.assert_async().await;
}

#[tokio::test]
async fn one_classifier_reads_kalshis_errors_natively_and_through_the_traits() {
    let ticker = Ticker::new(KALSHI_TICKER).unwrap();
    let cases = [
        (
            429,
            Some("1"),
            Handling::Backoff(Some(Duration::from_secs(1))),
        ),
        (503, None, Handling::Retry),
        (401, None, Handling::Reauthenticate),
        (451, None, Handling::Restricted { alert: false }),
        (400, None, Handling::Rejected { alert: true }),
    ];
    for (status, retry_after, expected) in cases {
        let mut server = Server::new_async().await;
        let mut mock = server
            .mock("GET", kalshi_book_path().as_str())
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(r#"{"error":{"code":"refused","message":"refused"}}"#);
        if let Some(wait) = retry_after {
            mock = mock.with_header("retry-after", wait);
        }
        let _mock = mock.create_async().await;

        let native = kalshi_client(&server)
            .markets()
            .orderbook(&ticker)
            .depth(1)
            .send()
            .await
            .unwrap_err();
        assert_eq!(classify(&native), expected, "{status}, native");
        let kalshi_venue = DynMarketData::new_arc(kalshi_client(&server));
        let err = best_bid_ask(&kalshi_venue, &kalshi_key_of(Outcome::Yes))
            .await
            .unwrap_err();
        assert_eq!(classify_classified(&err), expected, "{status}, through the traits");
    }
}

#[test]
fn the_traits_are_on_polyoxide_venue_for_kalshi() {
    fn market_data<T: polyoxide::venue::MarketData>() {}
    fn trades<T: polyoxide::venue::Trades>() {}
    fn candles<T: polyoxide::venue::Candles>() {}
    fn event_groups<T: polyoxide::venue::EventGroups>() {}
    market_data::<Predictions>();
    trades::<Predictions>();
    candles::<Predictions>();
    event_groups::<Predictions>();
}

#[test]
fn kalshi_serves_three_candle_widths_through_the_umbrella() {
    use polyoxide::venue::{CandleInterval, Candles};

    let kalshi = Predictions::new().unwrap();
    assert_eq!(
        kalshi.intervals(),
        &[
            CandleInterval::Minute1,
            CandleInterval::Hour1,
            CandleInterval::Day1,
        ]
    );
}
```

Edit `polyoxide/examples/venue_neutral.rs`. The header. Replace:

```rust
//! One function reads any venue's best bid and ask, and one classifier reads
//! any venue's error. Written once, against `polyoxide::venue`'s
//! `MarketData` and `Classify`, they run unchanged on the Polymarket CLOB,
//! Polymarket perps and Binance USDⓈ-M; adding a venue adds no code here.
//!
//! ```text
//! cargo run -p polyoxide --example venue_neutral \
//!     --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm
//! ```
```

with:

```rust
//! One function reads any venue's best bid and ask, and one classifier reads
//! any venue's error. Written once, against `polyoxide::venue`'s
//! `MarketData` and `Classify`, they run unchanged on the Polymarket CLOB,
//! Polymarket perps, Binance USDⓈ-M and Kalshi's Predictions; adding a venue
//! adds only its client and key to `main`'s list.
//!
//! ```text
//! cargo run -p polyoxide --example venue_neutral \
//!     --no-default-features \
//!     --features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions
//! ```
```

`main`'s venue list. Replace:

```rust
    let venues: [(Arc<DynMarketData<'static>>, MarketKey); 3] = [
        (
            DynMarketData::new_arc(Usdm::new()?),
            usdm::key(&Symbol::new("BTCUSDT")?),
        ),
        (
            DynMarketData::new_arc(Perps::new()?),
            keys::perps(InstrumentId(1)),
        ),
        (
            DynMarketData::new_arc(clob),
            keys::clob(&TokenId::new(token)?),
        ),
    ];
```

with:

```rust
    // Kalshi's first open market of one page that leaves out the multivariate combinations the
    // unfiltered listing serves first, keyed `yes`. It is named by path and has no `cfg`:
    // `polyoxide-kalshi`'s `venue_predictions` suite includes this file by `#[path]`, where a
    // feature that crate does not declare would fail `unexpected_cfgs`.
    let kalshi = polyoxide::kalshi::predictions::Predictions::new()?;
    let page = kalshi
        .markets()
        .list()
        .status(polyoxide::kalshi::predictions::types::MarketStatusFilter::Open)
        .mve_filter(polyoxide::kalshi::predictions::types::MveFilter::Exclude)
        .send()
        .await?;
    let market = page
        .markets
        .first()
        .ok_or("no open market on Kalshi's first page")?;
    let kalshi_key = polyoxide::kalshi::predictions::key::key(
        &polyoxide::kalshi::predictions::key::Ticker::new(&market.ticker)?,
        polyoxide::kalshi::predictions::key::Outcome::Yes,
    );

    let venues: [(Arc<DynMarketData<'static>>, MarketKey); 4] = [
        (
            DynMarketData::new_arc(Usdm::new()?),
            usdm::key(&Symbol::new("BTCUSDT")?),
        ),
        (
            DynMarketData::new_arc(Perps::new()?),
            keys::perps(InstrumentId(1)),
        ),
        (
            DynMarketData::new_arc(clob),
            keys::clob(&TokenId::new(token)?),
        ),
        (DynMarketData::new_arc(kalshi), kalshi_key),
    ];
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions --no-run`
Expected: a compile error in the included example, `error[E0433]: failed to resolve: could not find `kalshi` in `polyoxide``, because Kalshi's dev-dependency on the umbrella does not turn `kalshi-predictions` on yet.

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm --test venue_neutral --no-run`
Expected: the same `E0433` in `tests/venue_neutral.rs`, since the stanza's `required-features` lacks `kalshi-predictions` and the file now names the Kalshi module.

- [ ] **Step 3: Implement**

Edit `polyoxide-kalshi/Cargo.toml`. Replace:

```toml
# `tests/venue_predictions.rs` includes the umbrella's `examples/venue_neutral.rs`
# by `#[path]`. The three features are that example's `required-features`; they
# build the CLOB, perps and USDⓈ-M stacks into Kalshi's test builds only.
# Path-only, so cargo strips it on publish.
polyoxide = { path = "../polyoxide", default-features = false, features = ["polymarket-clob", "polymarket-perps", "binance-usdm"] }
```

with:

```toml
# `tests/venue_predictions.rs` includes the umbrella's `examples/venue_neutral.rs`
# by `#[path]`. The four features are that example's `required-features`; they
# build the CLOB, perps and USDⓈ-M stacks into Kalshi's test builds only, and
# `kalshi-predictions` is the example's Kalshi arm, which names the umbrella's
# `polyoxide::kalshi`. That makes a dev-dependency cycle with the umbrella's own
# dependency on this crate, which cargo allows. Path-only, so cargo strips it on
# publish.
polyoxide = { path = "../polyoxide", default-features = false, features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"] }
```

Edit `polyoxide/Cargo.toml`. Replace:

```toml
# The example's generic code and its two suites use one module of each kind:
# the CLOB, perps and USDⓈ-M.
[[example]]
name = "venue_neutral"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
```

with:

```toml
# The example's generic code and its two suites use one module of each kind:
# the CLOB, perps, USDⓈ-M and Kalshi's Predictions.
[[example]]
name = "venue_neutral"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]
```

and replace:

```toml
[[test]]
name = "venue_neutral"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
```

with:

```toml
[[test]]
name = "venue_neutral"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]
```

(`live_venue`'s stanza is Task 8.4's.)

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions`
Expected: passes; the included example compiles with its Kalshi arm. If cargo refuses the dev-dependency cycle, stop and ask: spec D13 requires it.

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions --test venue_neutral`
Expected: 7 passed (the three existing tests and the four new ones).

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm --test venue_neutral`
Expected: cargo refuses before compiling, `error: target `venue_neutral` in package `polyoxide` requires the features: `kalshi-predictions``, so a build without the Kalshi module never sees the file.

Run: `nix develop --command cargo build -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions --example venue_neutral`
Expected: `Finished`.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: clean, and Kalshi's suites pass with the cycle in place.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(umbrella): the venue-neutral example and its mock suite read Kalshi (Epic 9, Story 8)" -- polyoxide/examples/venue_neutral.rs polyoxide/tests/venue_neutral.rs polyoxide/Cargo.toml polyoxide-kalshi/Cargo.toml Cargo.lock
```

### Task 8.4: The live case, and the nightly job that builds it

`tests/live_venue.rs` gains a Kalshi case: it reads one real market's best bid and ask through the same `read` helper as the other three. The market is picked on the route the trait's quote reads (the order book at `depth=1`), among quoted open markets of the first filtered page, so a quiet night is an `environmental` skip and an empty book is not a failure. The test reads production unsigned, so it needs no secret. Its stanza and the registration's `features` list gain `kalshi-predictions`, which the registry turns into the nightly job's flags.

**Files:**
- Modify: `polyoxide/tests/live_venue.rs:1-16`
- Modify: `polyoxide/Cargo.toml` (the `live_venue` stanza and its registration)
- Test: `polyoxide/tests/live_venue.rs`
- Generated: `.github/workflows/nightly-behavioral.yml`, `CLAUDE.md`, `SELF-HEALING.md`

- [ ] **Step 1: Write the failing test**

In `polyoxide/tests/live_venue.rs`, replace the file's opening docs:

```rust
//! The example's `best_bid_ask` against the live hosts: the same function
//! reads Binance USDⓈ-M, Polymarket perps and the Polymarket CLOB.
```

with:

```rust
//! The example's `best_bid_ask` against the live hosts: the same function
//! reads Binance USDⓈ-M, Polymarket perps, the Polymarket CLOB and Kalshi's
//! Predictions.
```

Add the imports. Replace:

```rust
use polyoxide::binance::usdm::{self, types::Symbol, Usdm};
use polyoxide::polymarket::clob::{Clob, ClobError};
```

with:

```rust
use polyoxide::binance::usdm::{self, types::Symbol, Usdm};
use polyoxide::kalshi::predictions::{
    key::{self as kalshi_key, Outcome, Ticker},
    types::{MarketStatus, MarketStatusFilter, MveFilter},
    Predictions,
};
use polyoxide::polymarket::clob::{Clob, ClobError};
```

and replace:

```rust
use polyoxide_test_support::{environmental, fail, ClassifiedResultExt, ResultExt};
```

with:

```rust
use polyoxide_test_support::{environmental, fail, ClassifiedResultExt, ResultExt};
use rust_decimal::Decimal;
```

Append after the file's last test. The anchor is its last four lines:

```rust
    assert!(
        bid.is_some() && ask.is_some(),
        "the order book had both sides"
    );
}
```

Replace them with themselves and:

```rust
    assert!(
        bid.is_some() && ask.is_some(),
        "the order book had both sides"
    );
}

/// Quoted Kalshi markets to confirm on the order book route the trait's quote reads.
const KALSHI_CANDIDATES: usize = 20;

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_kalshi_predictions() {
    let kalshi = Predictions::new().or_fail("build the predictions client");
    // The unfiltered open listing serves multivariate combinations first, none of them quoted,
    // so the walk asks to leave them out.
    let page = kalshi
        .markets()
        .list()
        .status(MarketStatusFilter::Open)
        .mve_filter(MveFilter::Exclude)
        .limit(1000)
        .send()
        .await
        .or_fail("list the open markets");
    let quoted = page
        .markets
        .iter()
        .filter(|market| {
            market.status == MarketStatus::Active
                && market.yes_bid_dollars > Decimal::ZERO
                && market.yes_ask_dollars < Decimal::ONE
        })
        .take(KALSHI_CANDIDATES);
    // The market is picked on the route the trait's quote reads (depth 1), so it has both
    // sides there.
    let mut found = None;
    for market in quoted {
        let Ok(ticker) = Ticker::new(&market.ticker) else {
            continue;
        };
        let book = kalshi
            .markets()
            .orderbook(&ticker)
            .depth(1)
            .send()
            .await
            .or_fail("read a quoted market's order book")
            .orderbook_fp;
        if !book.yes_dollars.is_empty() && !book.no_dollars.is_empty() {
            found = Some(ticker);
            break;
        }
    }
    let Some(ticker) = found else {
        environmental(&format!(
            "none of the first {KALSHI_CANDIDATES} quoted open markets has an order book with both sides"
        ));
    };
    let key = kalshi_key::key(&ticker, Outcome::Yes);
    let (bid, ask) = read(DynMarketData::new_arc(kalshi), &key).await;
    assert!(
        bid.is_some() && ask.is_some(),
        "the order book had both sides"
    );
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm --test live_venue --no-run`
Expected: a compile error, `error[E0433]: failed to resolve: could not find `kalshi` in `polyoxide``, since `live_venue`'s stanza and the feature set above leave the Kalshi module out.

- [ ] **Step 3: Implement**

Edit `polyoxide/Cargo.toml`. Replace:

```toml
[[test]]
name = "live_venue"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
```

with:

```toml
[[test]]
name = "live_venue"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]
```

and, in the registration, replace:

```toml
features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
secrets = []
```

with:

```toml
features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]
secrets = []
```

Regenerate the registry. The nightly job `live-polyoxide-venue` builds with the target's `features`, so its flags, `CLAUDE.md`'s job list and `SELF-HEALING.md`'s table change with it. If the harness refuses the first write to `.github/workflows/nightly-behavioral.yml`, retry the same command unchanged.

Run: `nix develop --command python3 scripts/gen_registry.py --write`
Expected: `.github/workflows/nightly-behavioral.yml`'s `live-polyoxide-venue` flags become `--no-default-features --features binance-usdm,kalshi-predictions,polymarket-clob,polymarket-perps --test live_venue`, and `CLAUDE.md` and `SELF-HEALING.md` list `--features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions`.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions --test live_venue --no-run`
Expected: it builds.

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions --test live_venue -- --ignored best_bid_ask_reads_kalshi_predictions`
Expected: passes against production, unsigned, or ends as an `environmental` skip with a `polyoxide-class=environmental` line when no quoted open market has a two-sided book. Any other failure prints `polyoxide-class=real` or `transient` and is reported to the owner.

Run: `nix develop --command python3 scripts/gen_registry.py --check && nix develop --command python3 scripts/live_unwraps.py`
Expected: both exit 0, with no `.unwrap()` or `.expect(` in the new test and no assertion message that interpolates an error or a status.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(umbrella): live_venue reads Kalshi's Predictions, and its nightly job builds it (Epic 9, Story 8)" -- polyoxide/tests/live_venue.rs polyoxide/Cargo.toml .github/workflows/nightly-behavioral.yml CLAUDE.md SELF-HEALING.md docs/ARCHITECTURE.md
```

### Task 8.5: The umbrella's Kalshi allowlists and their rules

`ci/dep-allowlists/polyoxide@kalshi-predictions.txt` and `@kalshi-predictions-ws.txt` pin what the two umbrella features build. The umbrella's HTTP-stack rule admits `kalshi-predictions` (and so `kalshi` and `kalshi-predictions-ws`, which turn it on), and `alloy*`, `hmac` and `keyring` stay refused for all three. `MUST_BUILD` gains `polyoxide@kalshi-predictions.txt: reqwest`, so the rule cannot pass because nothing it admits is built, and `polyoxide@kalshi-predictions-ws.txt: polyoxide-ws`, for the stream's file.

The umbrella's rule table has no socket rule: a rule that admits `polyoxide-ws` only with `kalshi-predictions-ws` would refuse it under the umbrella's `polymarket-rtds`, `polymarket-sports` and `binance-usdm-ws` too, and an existing test (`test_the_umbrellas_socket_passthroughs_may_not_build_the_http_stack`) runs the table over an empty feature set with `polyoxide-ws` among its names. The socket half of spec D14 is therefore three added tests that read the two files: the REST file holds no socket crate, the stream's file holds all three, and only `kalshi-predictions-ws` turns the stream on.

The two files are new, and the `--write` mode of `test_dep_allowlists.py` rewrites only files that exist, so each is created empty first. The commit says why the files changed, as the repo requires: they are the trees the new umbrella features build, and no existing file moves.

**Files:**
- Modify: `.github/scripts/tests/test_dep_allowlists.py:35-37`, `:50-51`, `:130-132`, `:151`, and its end after `:546`
- Create: `ci/dep-allowlists/polyoxide@kalshi-predictions.txt`, `ci/dep-allowlists/polyoxide@kalshi-predictions-ws.txt`
- Test: `.github/scripts/tests/test_dep_allowlists.py`

- [ ] **Step 1: Write the failing tests**

Append to `.github/scripts/tests/test_dep_allowlists.py`, before the `if __name__ == "__main__":` block. The anchor is the last test's final line:

```python
    assert enabled((), TABLE) == set()
```

Replace it with itself and:

```python
    assert enabled((), TABLE) == set()


UMBRELLA_KALSHI_FILES = (
    "polyoxide@kalshi-predictions.txt",
    "polyoxide@kalshi-predictions-ws.txt",
)


def test_the_umbrellas_kalshi_files_exist() -> None:
    # A glob that matched neither would leave the two Kalshi features unpinned.
    names = {p.name for p in allowlists()}
    assert set(UMBRELLA_KALSHI_FILES) <= names, names


def test_the_umbrellas_kalshi_features_admit_the_http_stack_and_never_the_signing_stack() -> None:
    table = feature_table(UMBRELLA)
    for features in (("kalshi",), ("kalshi-predictions",), ("kalshi-predictions-ws",)):
        assert denied(UMBRELLA, HTTP_STACK, features, table) == [], features
        assert denied(UMBRELLA, ["alloy", "hmac", "keyring"], features, table) == [
            f"alloy ({UMBRELLA_SIGNING})", f"hmac ({UMBRELLA_SIGNING})",
            f"keyring ({KEYCHAIN})"], features


def test_only_the_umbrellas_kalshi_stream_feature_builds_the_socket_kit() -> None:
    # The real table: a `kalshi` or `kalshi-predictions` that came to imply the stream would
    # put the kit in the REST file, and the file would pass listing it.
    table = feature_table(UMBRELLA)
    assert "kalshi-predictions-ws" not in enabled(("kalshi",), table)
    assert "kalshi-predictions-ws" not in enabled(("kalshi-predictions",), table)
    assert "kalshi-predictions" in enabled(("kalshi-predictions-ws",), table)
    rest = read_allowlist(ALLOWLISTS / "polyoxide@kalshi-predictions.txt")
    assert [n for n in SOCKET_STACK if n in rest] == [], rest
    stream = read_allowlist(ALLOWLISTS / "polyoxide@kalshi-predictions-ws.txt")
    assert [n for n in SOCKET_STACK if n not in stream] == [], stream
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q -k "umbrellas_kalshi or only_the_umbrellas"'`
Expected: `3 failed`: the file-existence test (`assert {...} <= set(names)`), the admission test (`denied(...)` returns `governor (reqwest, polyoxide-core, governor and sha2 only with an HTTP module)` and the rest of the HTTP stack, for each of the three feature sets), and the stream test (`FileNotFoundError` on `polyoxide@kalshi-predictions.txt`).

- [ ] **Step 3: Implement**

Edit `.github/scripts/tests/test_dep_allowlists.py`.

The umbrella's HTTP rule admits Kalshi's module. Replace:

```python
             frozenset({"polymarket-clob", "polymarket-gamma", "polymarket-data",
                        "polymarket-relay", "polymarket-perps", "binance-usdm"}),
```

with:

```python
             frozenset({"polymarket-clob", "polymarket-gamma", "polymarket-data",
                        "polymarket-relay", "polymarket-perps", "binance-usdm",
                        "kalshi-predictions"}),
```

The files that must build what a rule admits. Replace:

```python
    "polyoxide@polymarket-clob,keychain.txt": "keyring",
    "polyoxide-kalshi@predictions.txt": "reqwest",
}
```

with:

```python
    "polyoxide@polymarket-clob,keychain.txt": "keyring",
    "polyoxide-kalshi@predictions.txt": "reqwest",
    "polyoxide@kalshi-predictions.txt": "reqwest",
    "polyoxide@kalshi-predictions-ws.txt": "polyoxide-ws",
}
```

The module docstring says what the umbrella's rule covers. Replace:

```
names: the HTTP stack only with an HTTP module of either venue
(`polymarket-clob`, `-gamma`, `-data`, `-relay` or `-perps`, or
`binance-usdm`), so its `@polymarket-rtds` and `@polymarket-sports` files are
```

with:

```
names: the HTTP stack only with an HTTP module of any venue
(`polymarket-clob`, `-gamma`, `-data`, `-relay` or `-perps`, `binance-usdm`
or `kalshi-predictions`), so its `@polymarket-rtds` and `@polymarket-sports` files are
```

and replace:

```
`@predictions` file must exist, and its `@predictions-ws` file, once written, must
hold `polyoxide-ws`.
```

with:

```
`@predictions` file must exist, and its `@predictions-ws` file, once written, must
hold `polyoxide-ws`. The umbrella's `@kalshi-predictions` file must hold `reqwest` and
no socket crate, and its `@kalshi-predictions-ws` file `polyoxide-ws`.
```

Create the two files, empty, and let `--write` fill them from the trees cargo reports:

Run: `touch 'ci/dep-allowlists/polyoxide@kalshi-predictions.txt' 'ci/dep-allowlists/polyoxide@kalshi-predictions-ws.txt'`

Run: `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'`
Expected: no output. `git status --short ci/dep-allowlists` lists exactly the two new files and no other: the existing allowlists are the trees they were. If another file changed, stop and ask, since the tree of an existing feature moved.

Run: `git add -N 'ci/dep-allowlists/polyoxide@kalshi-predictions.txt' 'ci/dep-allowlists/polyoxide@kalshi-predictions-ws.txt'`

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q'`
Expected: all pass: the three new tests, the parametrized `test_each_allowlist_is_the_tree_cargo_builds` and `test_no_allowlist_admits_a_denied_crate` for the two new files, and `test_each_per_feature_rule_admits_what_it_guards` for their two `MUST_BUILD` rows.

- [ ] **Step 5: Commit**

```bash
git commit -m "ci(umbrella): allowlists for kalshi-predictions and kalshi-predictions-ws; the trees the two new umbrella features build, no existing file moves (Epic 9, Story 8)" -- .github/scripts/tests/test_dep_allowlists.py 'ci/dep-allowlists/polyoxide@kalshi-predictions.txt' 'ci/dep-allowlists/polyoxide@kalshi-predictions-ws.txt'
```

### Task 8.6: The crate docs drop "walking skeleton"

The two READMEs and the crate docs say what a published Kalshi crate offers, with no mention of a recipe or of not being published. The READMEs are doctests (`include_str!` under `cfg(doctest)`), so they stay prose with no code fence. The umbrella's README gains the Kalshi row, install example and feature rows. The check is a grep, since no suite reads these words.

**Files:**
- Modify: `polyoxide-kalshi/README.md:5`, `:7`, `:9`
- Modify: `polyoxide-kalshi/src/predictions/README.md:3`, `:9`
- Modify: `polyoxide-kalshi/src/lib.rs:6-8`, `:10-18`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs:3-5`
- Modify: `.github/workflows/ci.yml` (the comment above the `Header pins, umbrella full build` step)
- Modify: `polyoxide/README.md:12`, `:14-17`, `:42-43`, `:67`

- [ ] **Step 1: Write the failing check**

The check is that no crate doc or README of the Kalshi crate says it is a walking skeleton or unpublished, and that the umbrella's README names Kalshi:

```bash
! grep -n -i 'walking skeleton\|not published' polyoxide-kalshi/README.md polyoxide-kalshi/src/lib.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/README.md && grep -c 'kalshi' polyoxide/README.md
```

- [ ] **Step 2: Run it to see it fail**

Run: the command above.
Expected: it prints four matches (`polyoxide-kalshi/README.md:5`, `polyoxide-kalshi/src/lib.rs:6`, `polyoxide-kalshi/src/predictions/mod.rs:4` and `polyoxide-kalshi/src/predictions/README.md:3`) and exits 1.

- [ ] **Step 3: Implement**

Edit `polyoxide-kalshi/README.md`. Replace the second paragraph:

```markdown
This crate is a walking skeleton and it is not published. It exists to show that a venue lands by the architecture guide's recipe: a crate of its own, registered in its manifest, touching the workspace only in the few places the recipe names.
```

with:

```markdown
The umbrella crate `polyoxide` re-exports it as `polyoxide::kalshi`, under its `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws` features.
```

and replace the paragraph that begins "It has one module":

```markdown
It has one module, `predictions`, behind the `predictions` feature, which is on by default. The module covers Kalshi's Predictions exchange on `external-api.kalshi.com`, and on the demo host `external-api.demo.kalshi.co`. Its documentation says what it offers.
```

with:

```markdown
It has one module, `predictions`, behind the `predictions` feature, which is on by default. The module covers Kalshi's Predictions exchange on `external-api.kalshi.com`, and on the demo host `external-api.demo.kalshi.co`. It reads market data through `polyoxide-venue`'s traits, `MarketData`, `Trades`, `Candles` and `EventGroups`; its candles come at three widths, one minute, one hour and one day, which `Candles::intervals()` lists, and a request for any other width is refused before anything is sent. Its documentation says what else it offers.
```

and replace the last paragraph:

```markdown
The feature `predictions-ws`, which is off by default and turns on `predictions`, adds the module's order-book socket, `predictions::ws`, on the polyoxide socket kit. It streams a snapshot of each subscribed market's book and then its changes, in YES pricing, and reconnects after a drop, a stall or a gap. Kalshi refuses a socket handshake that carries no API-key headers, public data included, so the socket needs `Credentials`, where the REST reads of public data need none.
```

with:

```markdown
The feature `predictions-ws`, which is off by default and turns on `predictions`, adds the module's sockets, `predictions::ws`, on the polyoxide socket kit. It streams the order-book channel, a snapshot of each subscribed market's book and then its changes in YES pricing, and the `ticker` and `trade` channels, chosen with `PredictionsWsBuilder::channels`. A local book, `Books`, folds the order-book events into each market's ladders. The stream reconnects after a drop or a stall, and a gap in the order-book subscription's sequence is repaired in place with a fresh snapshot, falling back to a reconnect. Kalshi refuses a socket handshake that carries no API-key headers, public data included, so the socket needs `Credentials`, where the REST reads of public data need none.
```

Edit `polyoxide-kalshi/src/predictions/README.md`. In the first paragraph, replace:

```markdown
The crate is a walking skeleton and it is not published. Kalshi's other exchange, Margin, is not covered, and the name `margin` is left free for it.
```

with:

```markdown
Kalshi's other exchange, Margin, is not covered, and the name `margin` is left free for it.
```

and replace the last paragraph:

```markdown
With the `predictions-ws` feature, which is off by default, the module also streams the order book of the markets it is given, as `predictions::ws`, on the socket kit and not on the send loop. Every price is in YES pricing, so a NO bid is reported as the YES ask it is. Kalshi refuses a socket handshake that carries no API-key headers, public data included, so the socket is built from `Credentials` and signs each connection attempt afresh, where the REST reads of public data need none. The demo host's socket takes the demo key, and keys do not cross environments.
```

with:

```markdown
With the `predictions-ws` feature, which is off by default, the module also streams the order book of the markets it is given, as `predictions::ws`, on the socket kit and not on the send loop, and with it the `ticker` and `trade` channels, chosen with `PredictionsWsBuilder::channels`. Every price is in YES pricing, so a NO bid is reported as the YES ask it is. A local book, `Books`, folds the order-book events into each market's ladders, and a gap in the order-book subscription's sequence is repaired in place with a fresh snapshot, falling back to a reconnect. Kalshi refuses a socket handshake that carries no API-key headers, public data included, so the socket is built from `Credentials` and signs each connection attempt afresh, where the REST reads of public data need none. The demo host's socket takes the demo key, and keys do not cross environments.
```

Edit `polyoxide-kalshi/src/predictions/mod.rs`. Replace:

```rust
//! feature, which is on by default. The module is a walking skeleton: it grows a part at a time,
//! and what it offers today is in its namespaces.
```

with:

```rust
//! feature, which is on by default. What the module offers is in its namespaces.
```

Edit `polyoxide-kalshi/src/lib.rs`. Replace:

```rust
//! The crate is a walking skeleton and it is not published. It is the workspace's third venue,
//! built by the architecture guide's recipe to show that a venue lands by editing its own files
//! and the few places the recipe names.
//!
//! `predictions` (feature `predictions`, on by default) reads Kalshi's Predictions exchange
//! (`external-api.kalshi.com`, and the demo host `external-api.demo.kalshi.co`). Public REST reads
//! need no credentials. The module's own docs carry its example.
//!
//! `predictions-ws` (feature `predictions-ws`, off by default, and it turns on `predictions`) adds
//! the module's order-book socket, `predictions::ws`, on the socket kit: a snapshot of each
//! subscribed market's book, then its changes, in YES pricing, reconnecting after a drop, a
//! stall or a gap. Kalshi refuses a socket handshake that carries no API-key headers, public data
//! included, so the socket needs `Credentials` where the REST reads of public data need none.
```

with:

```rust
//! It is the workspace's third venue, built by the architecture guide's recipe, and the umbrella
//! crate `polyoxide` re-exports it as `polyoxide::kalshi`, under its `kalshi`,
//! `kalshi-predictions` and `kalshi-predictions-ws` features.
//!
//! `predictions` (feature `predictions`, on by default) reads Kalshi's Predictions exchange
//! (`external-api.kalshi.com`, and the demo host `external-api.demo.kalshi.co`). Public REST reads
//! need no credentials. It implements `polyoxide-venue`'s `MarketData`, `Trades`, `Candles` and
//! `EventGroups`, its candles at one minute, one hour and one day. The module's own docs carry its
//! example.
//!
//! `predictions-ws` (feature `predictions-ws`, off by default, and it turns on `predictions`) adds
//! the module's sockets, `predictions::ws`, on the socket kit: the order-book channel, a snapshot
//! of each subscribed market's book and then its changes in YES pricing, and the `ticker` and
//! `trade` channels, chosen with `PredictionsWsBuilder::channels`. A local book, `Books`, folds the
//! order-book events into each market's ladders. The stream reconnects after a drop or a stall,
//! and repairs a gap in the order-book subscription's sequence in place with a fresh snapshot,
//! falling back to a reconnect. Kalshi refuses a socket handshake that carries no API-key
//! headers, public data included, so the socket needs `Credentials` where the REST reads of
//! public data need none.
```

Edit `polyoxide/README.md`. The path table. Replace:

```markdown
| `polyoxide::prelude` | A selection of the enabled modules' clients, errors and stream types | |
```

with:

```markdown
| `polyoxide::kalshi` | `polyoxide-kalshi`: Kalshi's Predictions market data, candles and streams | `kalshi` and `kalshi-<module>` |
| `polyoxide::prelude` | A selection of the enabled modules' clients, errors and stream types | |
```

The example paragraph. Replace:

```markdown
Polymarket CLOB, Polymarket perps and Binance USDⓈ-M with one function, and their errors with one
classifier.
```

with:

```markdown
Polymarket CLOB, Polymarket perps, Binance USDⓈ-M and Kalshi's Predictions with one function, and
their errors with one classifier.
```

The install examples. Replace:

```
# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws
```

with:

```
# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws

# Kalshi event-contract market data and its streams
cargo add polyoxide --no-default-features --features kalshi-predictions-ws
```

The feature table. Replace:

```markdown
| `binance-usdm-ws` | no | Binance USDⓈ-M market streams via `polyoxide-binance`'s `usdm-ws` feature |
```

with:

```markdown
| `binance-usdm-ws` | no | Binance USDⓈ-M market streams via `polyoxide-binance`'s `usdm-ws` feature |
| `kalshi` | no | `kalshi-predictions`, `polyoxide-kalshi`'s defaults |
| `kalshi-predictions` | no | Kalshi event-contract market data and candles via `polyoxide-kalshi`'s `predictions` module |
| `kalshi-predictions-ws` | no | Kalshi order-book, ticker and trade streams via `polyoxide-kalshi`'s `predictions-ws` feature |
```

Edit `.github/workflows/ci.yml`. The comment above the `Header pins, umbrella full build` step says Kalshi is not in the umbrella. Replace:

```yaml
      # literal (AD-18's third build). `polyoxide-kalshi` is `publish = false`, so
      # the umbrella does not include it: its pin runs beside `polyoxide/full` all
      # the same, with no umbrella feature of its own to turn on.
```

with:

```yaml
      # literal (AD-18's third build). `polyoxide-kalshi` is published and the
      # umbrella includes it, so its pin runs beside `polyoxide/full` like the
      # others, and `full` turns on `kalshi`, `kalshi-predictions` and
      # `kalshi-predictions-ws` in the pinned build.
```

No step of the workflow changes, only this comment.

- [ ] **Step 4: Run it to see it pass**

Run: `! grep -n -i 'walking skeleton\|not published' polyoxide-kalshi/README.md polyoxide-kalshi/src/lib.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/README.md && grep -c 'kalshi' polyoxide/README.md`
Expected: the grep finds nothing, and the count is at least 6.

Run: `nix develop --command cargo test --doc -p polyoxide-kalshi -p polyoxide --all-features && nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_module_readmes.py -q'`
Expected: passes. The READMEs compile as doctests and hold no code fence, and each module README is still included exactly once.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide -p polyoxide-kalshi --all-features' && nix develop --command cargo fmt --all -- --check`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(kalshi): the crate docs and READMEs describe a published crate, and the umbrella's README names Kalshi (Epic 9, Story 8)" -- polyoxide-kalshi/README.md polyoxide-kalshi/src/lib.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/README.md polyoxide/README.md .github/workflows/ci.yml
```

### Task 8.7: The package, feature and header-pin gates

Spec §10's run, once, on the finished story: the manifest check, the publish order, the publish dry run, cargo-hack's each-feature step over the umbrella and Kalshi, the header pin under the umbrella's `full`, the umbrella's tests with `full`, and the story's gates. Nothing here edits a file unless a run changes `Cargo.lock`. Nothing publishes: the only publish command is the dry run, with `--dry-run`.

**Files:**
- Modify: `Cargo.lock` (only if a command below changes it)

- [ ] **Step 1: The package job's steps**

Run: `nix develop --command python3 scripts/publish_order.py check-manifests`
Expected: exit 0.

Run: `nix develop --command python3 scripts/publish_order.py list`
Expected: it asks crates.io, so it needs the network, and prints the `(crate, version)` pairs crates.io lacks. `polyoxide-kalshi 0.41.1` is among them, and before `polyoxide` when both are listed, since Kalshi is ordered by its dependencies after `polyoxide-ws`. If it exits 3 (more than five new names) or 4 (a cycle, or a dependency on an unpublishable member), stop and ask.

Run: `nix develop --command cargo publish --workspace --dry-run --no-verify --locked`
Expected: packaging succeeds for every publishable member, `polyoxide-kalshi` included, and nothing is uploaded. If cargo reports a dependency cycle between `polyoxide` and `polyoxide-kalshi`, stop and ask: the umbrella's dependency on Kalshi and Kalshi's path-only dev-dependency on the umbrella are meant to order nothing, since publishing strips the second.

Run: `nix develop --command bash -c 'for m in tombstones/*/Cargo.toml; do [ -e "$m" ] || continue; cargo package --no-verify --manifest-path "$m" || exit 1; done'`
Expected: exit 0 with no output (there are no tombstones in S3).

- [ ] **Step 2: The features job's steps**

`--no-dev-deps` rewrites every manifest while it runs and restores them after, so nothing else may edit a manifest meanwhile.

Run: `nix develop --command cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`
Expected: it lists `polyoxide-kalshi` among the packages it checks, which `--ignore-private` skipped while the crate was `publish = false` (this closes Epic 8's A8-10), and exits 0.

Run: `nix develop --command cargo hack check -p polyoxide -p polyoxide-kalshi --each-feature --no-dev-deps`
Expected: exit 0, with `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws` each built on their own.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`
Expected: passes (the minimal-build pin, unchanged).

Run: `nix develop --command cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions`
Expected: passes. The header set the Kalshi client sends is still the literal `accept: */*`, `accept-encoding: gzip` and `host`, with the umbrella's `full` unifying every module of every venue into the build; the umbrella feature this step needs now exists, so Kalshi's pin is no longer an exception, as Task 8.6's rewritten `ci.yml` comment above the step says.

- [ ] **Step 3: The story's tests**

Run: `nix develop --command cargo test -p polyoxide --features full`
Expected: passes: the umbrella's unit tests, `parse_key`, `venue_neutral` (7), the doctests, and every other target with its `required-features` met. `live_venue` is built, and its tests are ignored.

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: passes.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_ci_workflow.py tests/test_gen_registry.py -q'`
Expected: all pass.

- [ ] **Step 4: The profile's gates**

Run, in order: `nix develop --command cargo fmt --all -- --check`, then `nix develop --command cargo clippy -p polyoxide -p polyoxide-kalshi --all-targets --all-features -- -D warnings`, then `nix develop --command cargo test -p polyoxide -p polyoxide-kalshi --all-features`, then `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide -p polyoxide-kalshi --all-features'`, then `nix develop --command python3 scripts/gen_registry.py --check`, then `nix develop --command python3 scripts/venue_gate.py`, then `nix develop --command python3 scripts/live_unwraps.py`, then `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`.
Expected: each exits 0. `test_body_diff.py` compares nothing new: the umbrella's tests and examples and the whole Kalshi crate came after `v0.40.0`.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
Expected: it prints nothing and exits 0: every path this story changed is admitted, `docs/ARCHITECTURE.md` among them if `gen_registry.py --write` rewrote it.

- [ ] **Step 5: Commit**

```bash
git status --short
```

Expected: empty. If `Cargo.lock` shows as modified, commit it and say why; otherwise there is nothing to commit and the story is done:

```bash
git commit -m "chore(kalshi): Cargo.lock after the package and feature runs (Epic 9, Story 8)" -- Cargo.lock
```

## Story 9: Docs, records and amendments (spec D15, §11)

- **Developer:** intermediate-dev
- **Files:** `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `.env.example`, `docs/specs/INDEX.md`, `docs/s3-migration.md`, `.github/scripts/tests/test_classify_coverage.py`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md`, `_bmad-output/implementation-artifacts/deferred-work.md`
- **Interfaces:** uses every public name above for the docs; exposes nothing
- **Verify:** `python3 scripts/gen_registry.py --check && cargo test --doc -p polyoxide-kalshi -p polyoxide --all-features && bash -c 'cd .github/scripts && uv run pytest tests/ -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh`
- **Review:** light (docs and records)

Brings the hand-written docs up to a published Kalshi, and writes the records of the epic. Task 9.1 edits CLAUDE.md (the venue lists, the Key Patterns paragraphs D15 names, `Candles::intervals()` and `UnservedInterval`, the socket's channels, re-snapshot and book, the kit's `follow_up`, the CI paragraph's Kalshi pins, and the Environment Variables); Task 9.2 the lead-ins of `README.md`, `docs/specs/INDEX.md` and `SELF-HEALING.md`, and `.env.example`; Task 9.3 `docs/s3-migration.md`'s notes for the release; Task 9.4 a Kalshi vacuity guard in `test_classify_coverage.py`; Task 9.5 `spine-amendments/epic-9.md` (A9-1 to A9-4); Task 9.6 closes the `deferred-work.md` entries the epic settled, each with its commit. Generated regions change only through `python3 scripts/gen_registry.py --write`, and `docs/ARCHITECTURE.md` is not edited. The story writes no Rust and holds none of the profile's landmine markers.

Story 8 also lists `CLAUDE.md`, `README.md`, `SELF-HEALING.md` and `docs/specs/INDEX.md` among its files, but it touches them only through `gen_registry.py --write`, inside generated regions, and it owns the root `Cargo.toml`'s `mirrors.kalshi` strings that feed the generated INDEX rows. Every hand-written line below is this story's, so each edit targets today's hand-written text unconditionally.

### Task 9.1: CLAUDE.md names Kalshi as a published venue

CLAUDE.md's crate list, its publish-order line and its umbrella feature list are generated regions that Story 8's `gen_registry.py --write` rewrites; this task edits the hand-written text around them (D15's list), in a commit of its own, so the diff reads as the rules changing (AD-21).

**Files:**
- Modify: `CLAUDE.md` (the twenty-six anchors below)
- Test: a throwaway check script outside the tree

- [ ] **Step 1: Write the failing check**

```bash
cat > "${TMPDIR:-/tmp}/epic9_claude_check.py" <<'PY'
import sys
from pathlib import Path

text = Path("CLAUDE.md").read_text(encoding="utf-8")
problems = []
for phrase in [
    'walking skeleton',
    'the umbrella does not include it',
    'is exempt, as `polyoxide-kalshi` is',
    'each width every candle venue serves',
]:
    if phrase in text:
        problems.append(f"still says: {phrase}")
for phrase in [
    "`polyoxide-kalshi` for Kalshi's event-contract market data and its order-book, ticker and trade streams",
    '`kalshi` (`polyoxide-kalshi`, under any `kalshi*` feature)',
    '`kalshi` turns on `kalshi-predictions`',
    '`binance-usdm`, `kalshi-predictions`) also turns on `polyoxide-venue/market`',
    'CLOB, perps, USDⓈ-M and Kalshi',
    '`PredictionsEvent`',
    '`Credentials` as `KalshiCredentials`',
    '`Books` as `PredictionsBooks`',
    '`Predictions::builder()`',
    'perps, Binance and Kalshi 4',
    "Kalshi's `credentials`",
    'Kalshi: `kalshi.health()`',
    "Kalshi's `PredictionsError`",
    "Kalshi's turns an unsuccessful response",
    "Kalshi's `InvalidTicker` and `CredentialsError`",
    '`UnknownVariant` and `UnservedInterval`',
    '`PredictionsWsError::Connection`',
    "Kalshi's `predictions::key` (`PredictionsKeys`",
    '`Candles::intervals()`',
    '`CandleQuery::ensure_served`',
    "Kalshi's `predictions::venue` implements `MarketData`, `Trades`, `EventGroups` and `Candles`",
    '`follow_up`, one frame the protocol sends',
    '`PredictionsWsBuilder::channels`',
    'one `update_subscription` per `sid`',
    'an empty set never subscribes',
    're-snapshots in place',
    '`PredictionsWsError::SnapshotTimeout`',
    '`PredictionsMembership::snapshot(tickers)`',
    '`Books::apply(&event)`',
    '`KALSHI_DEMO_API_KEY_ID`',
    'would be exempt, and none is left',
    '`@kalshi-predictions` must hold `reqwest`',
]:
    if phrase not in text:
        problems.append(f"lacks: {phrase}")
print("\n".join(problems) or "CLAUDE.md names Kalshi as a published venue")
sys.exit(1 if problems else 0)
PY
```

- [ ] **Step 2: Run it to see it fail**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_claude_check.py"`
Expected: a `lacks:` line for each of the 32 new phrases and a `still says:` line for the stale ones that the hand-written text holds (`walking skeleton`, `the umbrella does not include it`, `is exempt, as `polyoxide-kalshi` is` and `each width every candle venue serves`), then exit code 1.

- [ ] **Step 3: Implement**

Apply the twenty-six edits below to `CLAUDE.md`, each with the editor's exact-string replace. Every old text is hand-written, unique in the file today and copied from the Read tool; a text that spans a line break in the file (E19) keeps the break. Apply each edit unconditionally. An edit whose old text matches nothing, or more than once, stops the task: report it.

**E1** (Project Overview, the one paragraph). Replace

```text
streams) and `polyoxide-binance` for Binance's USDⓈ-M futures market data and streams, under a `polyoxide` umbrella with a module per venue (a third crate, `polyoxide-kalshi`, is a walking skeleton for Kalshi's event-contract market data: it is not published, and the umbrella does not include it).
```

with

```text
streams), `polyoxide-binance` for Binance's USDⓈ-M futures market data and streams and `polyoxide-kalshi` for Kalshi's event-contract market data and its order-book, ticker and trade streams, under a `polyoxide` umbrella with a module per venue.
```

**E2** (Build & Development Commands, the CI paragraph (one long line starting `CI runs nine jobs`), the header-pin sentence). Replace

```text
only a `publish = false` crate the umbrella does not depend on is exempt, as `polyoxide-kalshi` is, and its pin runs under `full` with no umbrella feature of its own to turn on)
```

with

```text
only a `publish = false` crate the umbrella does not depend on would be exempt, and none is left, since `polyoxide-kalshi` is published and the umbrella's `kalshi-predictions` turns its module on under `full`)
```

**E3** (the same CI line, the dependency-allowlist sentence about the umbrella). Replace

```text
and the umbrella `polyoxide`'s per feature on its own names, so the HTTP stack only with an HTTP module of either venue (`polymarket-clob`, `-gamma`, `-data`, `-relay`, `-perps` or `binance-usdm`), `alloy*` and `hmac` only with `polymarket-clob` or `polymarket-relay`, and `keyring` only with `keychain` (its `@polymarket-rtds` and `@polymarket-sports` files are the socket-only tree, and `@polymarket-perps` must hold `reqwest`)
```

with

```text
and the umbrella `polyoxide`'s per feature on its own names, so the HTTP stack only with an HTTP module of any venue (`polymarket-clob`, `-gamma`, `-data`, `-relay`, `-perps`, `binance-usdm` or `kalshi-predictions`), `alloy*` and `hmac` only with `polymarket-clob` or `polymarket-relay`, and `keyring` only with `keychain` (its `@polymarket-rtds` and `@polymarket-sports` files are the socket-only tree, `@polymarket-perps` and `@kalshi-predictions` must hold `reqwest`, and `@kalshi-predictions-ws` holds the socket kit)
```

**E4** (the umbrella paragraph (`The umbrella's crate root holds`), its third line). Replace

```text
any `binance*` feature), `parse_key` (under
```

with

```text
any `binance*` feature), `kalshi` (`polyoxide-kalshi`, under any `kalshi*` feature), `parse_key` (under
```

**E5** (the same paragraph, the line with `a `-ws` one turning its module's on`). Replace

```text
relay's included;
```

with

```text
relay's included; `kalshi` turns on `kalshi-predictions`;
```

**E6** (the same paragraph, the sentence on `polyoxide-venue/market`). Replace

```text
(`polymarket-clob`, `polymarket-perps`, `binance-usdm`) also turns on `polyoxide-venue/market`, and
```

with

```text
(`polymarket-clob`, `polymarket-perps`, `binance-usdm`, `kalshi-predictions`) also turns on `polyoxide-venue/market`, and
```

**E7** (the same paragraph, the sentence on `examples/venue_neutral.rs`). Replace

```text
reads the best bid and ask of the CLOB, perps and USDⓈ-M with one
```

with

```text
reads the best bid and ask of the CLOB, perps, USDⓈ-M and Kalshi with one
```

**E8** (the same paragraph, the prelude sentence). Replace

```text
`Event` by its venue (`PerpsEvent`, `SportsEvent`, `UsdmEvent`). One `const _` per prelude block
```

with

```text
`Event` by its venue (`PerpsEvent`, `SportsEvent`, `UsdmEvent`, `PredictionsEvent`). One `const _` per prelude block
```

**E9** (the same paragraph, its last line). Replace

```text
pins it to its feature, and one more pins the unified client to `polymarket`.
```

with

```text
pins it to its feature, and one more pins the unified client to `polymarket`. Kalshi's prelude names are `Predictions`, `PredictionsBuilder` and `PredictionsError`, `Credentials` as `KalshiCredentials` (a bare `Credentials` names the CLOB's), and, with `kalshi-predictions-ws`, `PredictionsWsBuilder`, `SupervisedPredictionsWs`, `PredictionsWsError` and `Books` as `PredictionsBooks`; `parse_key` also names `kalshi.predictions` under `kalshi-predictions`.
```

**E10** (Key Patterns, `**Builder pattern**`, the builder list). Replace

```text
`Perps::builder()`, `Usdm::builder()`, `RelayClient::default_builder()`
```

with

```text
`Perps::builder()`, `Usdm::builder()`, `Predictions::builder()`, `RelayClient::default_builder()`
```

**E11** (the same paragraph, the default-concurrency list). Replace

```text
relay 2; gamma, data, perps and Binance 4; clob 8)
```

with

```text
relay 2; gamma, data, perps, Binance and Kalshi 4; clob 8)
```

**E12** (the same paragraph, the venue-knobs sentence). Replace

```text
and Binance's `weight_budget`. Relay keeps
```

with

```text
Binance's `weight_budget`, and Kalshi's `credentials` (the key id and private key that sign a keyed client's requests; a client built without them sends unsigned ones). Relay keeps
```

**E13** (`**API namespaces**`, the Perps and Binance bullet). Replace

```text
Binance: `usdm.health()`, `usdm.exchange()`, `usdm.market()`.
```

with

```text
Binance: `usdm.health()`, `usdm.exchange()`, `usdm.market()`; Kalshi: `kalshi.health()`, `kalshi.exchange()`, `kalshi.markets()`, `kalshi.events()`, `kalshi.account()`.
```

**E14** (`**Error hierarchy**`, the first sentence). Replace

```text
`RelayError`, Binance's `UsdmError`), each module's
```

with

```text
`RelayError`, Binance's `UsdmError`, Kalshi's `PredictionsError`), each module's
```

**E15** (the same paragraph, the sentence on each module's `From<ApiError>`). Replace

```text
and then its `{code, msg}` body. `RequestError`
```

with

```text
and then its `{code, msg}` body, and Kalshi's turns an unsuccessful response whose body has one of its three shapes (`{"error": {"code", "message", "details"}}`, `{"error": "<text>"}` or `{"msg": "<text>"}`) into `Venue` and keeps any other as core's `Api`. `RequestError`
```

**E16** (`**Every error implements `polyoxide_venue::Classify`**`, the list of inner and auxiliary types). Replace

```text
`ParseTickSizeError`, `polyoxide-venue`'s `UnknownVariant`, `InvalidSymbol`, `InvalidStreamName`)
```

with

```text
`ParseTickSizeError`, Kalshi's `InvalidTicker` and `CredentialsError`, `polyoxide-venue`'s `UnknownVariant` and `UnservedInterval`, `InvalidSymbol`, `InvalidStreamName`)
```

**E17** (the same paragraph, the list of errors wrapping `polyoxide_ws::WsError`). Replace

```text
`UsdmWsError::Connect`, `RtdsError::Connection`
```

with

```text
`UsdmWsError::Connect`, `PredictionsWsError::Connection`, `RtdsError::Connection`
```

**E18** (`**Market keys**`, the sentence ending the list of `KeySpace` impls). Replace

```text
A clob key names one outcome token, never a market.
```

with

```text
A clob key names one outcome token, never a market. Kalshi's `predictions::key` (`PredictionsKeys`, over `OutcomeId`: a `Ticker` of 1 to 200 characters of `A-Z`, `0-9`, `.` and `-` whose ASCII letters are uppercased, and an `Outcome`, `yes` or `no`, lowercased; `predictions::key(&ticker, outcome)`, `key::parse` and `key::outcome`) likewise names one outcome of one market, never the market.
```

**E19** (`**Market-data traits**`, the sentence on `CandleQuery` (two lines in the file; keep the line break)). Replace

```text
`CandleInterval` (one
minute to one week, each width every candle venue serves) and a span.
```

with

```text
`CandleInterval` (one
minute to one week) and a span. `Candles::intervals()` is a provided method returning the widths the venue serves, every one (`CandleInterval::ALL`) by default, which the CLOB, perps and USDⓈ-M keep and Kalshi overrides; a query whose width is outside it is refused as `InvalidRequest`, sent nowhere, through `CandleQuery::ensure_served`, whose `UnservedInterval` (`Classify`: `InvalidRequest`, a fault, no wait) an implementation converts with `?` as it does a `KeyError` (amendment A9-2).
```

**E20** (the same paragraph, the line on Polymarket's `perps::venue` (its last sentence)). Replace

```text
Polymarket's `perps::venue` implements the same five traits on `Perps` (see Perps public routes).
```

with

```text
Polymarket's `perps::venue` implements the same five traits on `Perps` (see Perps public routes).
Kalshi's `predictions::venue` implements `MarketData`, `Trades`, `EventGroups` and `Candles` on `Predictions`, with `InstrumentExt`, `TradeExt`, `EventGroupExt` and `CandleExt`. Its `Candles::intervals()` is one minute, one hour and one day (`period_interval` 1, 60 and 1440), and any other width is the `UnservedInterval` refusal before a request, since the batch candlestick route answers an unserved width with a `200` and no rows. Its `candles` read `GET /markets/candlesticks` for the one ticker, asking for the query's span moved on by one width because Kalshi selects a candle by its end and the trait by its open, and put each candle's `open_time` one width before its end; a `no` key mirrors every price (*1 - p*, with high and low swapped), a period with no trade has only a `close`, the venue's previous price, and is skipped without one, and `CandleExt` holds the YES bid and ask as `Ohlc`, `open_interest` and `mean`. A market settled before Kalshi's historical cutoff is served only by a `/historical` route this client does not implement, so the live route's answer for it passes through as the venue gave it.
```

**E21** (`## Environment Variables`, after the Relay paragraph). Replace

```text
Relay also reads `RELAYER_URL` and `CHAIN_ID` optionally.
```

with

```text
Relay also reads `RELAYER_URL` and `CHAIN_ID` optionally.

Kalshi's signed routes and its socket take `polyoxide_kalshi::predictions::Credentials`, an API key id and an Ed25519 or RSA private key read once from PEM, through `PredictionsBuilder::credentials`; no client reads them from the environment. The two live suites that need them, `live_predictions_account` and `live_predictions_ws`, run against the demo host and read `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` (the PEM text) through test-support's loaders, and are auth-gated without them; keys do not cross environments, and production credentials never reach CI.
```

**E22** (the kit paragraph (`**The socket kit is `polyoxide-ws`; …**`), the list of what a `Protocol` describes). Replace

```text
(by default `None`), `record` for a change made during an outage
```

with

```text
(by default `None`), `follow_up`, one frame the protocol sends after a `decode` that leaves the connection up, bounded by `stale_after` and not awaited (by default none; a failed or timed-out send ends the connection as a failed ping does, and the protocol clears the frames it still holds in `reset`), `record` for a change made during an outage
```

**E23** (`**WebSocket**` (Module Organization), Kalshi's socket: the channels). Replace

```text
subscribes `orderbook_delta` with `use_yes_price: true`, so a snapshot's asks are the NO bids in YES pricing and one ladder serves both outcomes.
```

with

```text
subscribes `orderbook_delta` with `use_yes_price: true`, so a snapshot's asks are the NO bids in YES pricing and one ladder serves both outcomes, and, when `PredictionsWsBuilder::channels` asks for them, `ticker` and `trade` as well (a `Channel`, deduplicated, `OrderbookDelta` by default; an empty set is `PredictionsWsError::InvalidRequest` at `connect`): one `subscribe` names every channel and the tickers, each channel's `subscribed` answer gives it its own `sid`, and the stream yields `Event::Ticker` and `Event::Trade` beside the snapshots and deltas.
```

**E24** (the same paragraph, the membership). Replace

```text
The membership is a set of tickers, replayed as one `subscribe` on every connection and changed with `update_subscription` on the live `sid` (the first add sends a `subscribe`, emptying it an `unsubscribe`);
```

with

```text
The membership is a set of tickers, replayed as one `subscribe` on every connection and changed with one `update_subscription` per `sid`, in channel order (the first add sends a `subscribe`, emptying it an `unsubscribe` for every `sid`, and an empty set never subscribes, since Kalshi reads a `ticker` or `trade` subscribe with no markets as every market, A9-4);
```

**E25** (the same paragraph, the sequence check). Replace

```text
Every frame that carries `sid` and `seq`, the answers included, is checked per `sid`, and a gap is `PredictionsWsError::SequenceGap` and reconnects.
```

with

```text
Every frame that carries `sid` and `seq`, the answers included, is checked per `sid`. A gap on the `trade` `sid` is `PredictionsWsError::SequenceGap` and reconnects, since that channel has no snapshot (the `ticker` channel carries no `seq`). A gap on the order-book `sid` re-snapshots in place: the stream yields `Event::Resyncing { tickers }` (the whole membership), drops the frame and every `orderbook_delta` on that `sid` until each ticker's snapshot is in, and queues through the kit's `Protocol::follow_up` (A9-3) one `update_subscription` with `action: "get_snapshot"` for every ticker; each snapshot that answers re-anchors the `sid`'s sequence, and a frame that carries the follow-up's id is a push, never the answer to a membership command. The stream reconnects instead (`PredictionsWsError::SnapshotTimeout`, `Unavailable`) when the snapshots are not all in by `stale_after`, on an error frame answering the follow-up, and on a second gap during the resync. `PredictionsMembership::snapshot(tickers)` asks for the same snapshots on demand (`Change::Snapshot`: answered `Ok` once accepted, at once and unrecorded during an outage, since the replay brings a fresh snapshot of every ticker, and `PredictionsWsError::InvalidRequest` for a ticker outside the membership or without the order-book channel).
```

**E26** (the same paragraph, its last sentence). Replace

```text
so a signed handshake never travels unencrypted to a remote host.
```

with

```text
so a signed handshake never travels unencrypted to a remote host. `ws::book::Books` is a value the consumer feeds events into: `Books::apply(&event)` returns the `OrderBook` the event changed. It keeps each ticker's ladder in YES pricing from `Snapshot` and `Delta` (a delta adds its signed size to its level and removes the level at zero or below, and a delta for a ticker with no snapshot is ignored), drops the books `Resyncing` names and every book on `Reconnected`, and gives `best_bid`, `best_ask`, `bids` (highest first), `asks` (lowest first) and `levels(outcome)`, where a NO bid at *q* is a YES ask at *1 - q*; it does no I/O and has no clock.
```

Then regenerate and confirm no generated region moved: run `nix develop --command python3 scripts/gen_registry.py --write`, then `git status --short`, which must list `CLAUDE.md` alone. A second file changed means a generated region was stale before this task; stop and report it.

- [ ] **Step 4: Run it to see it pass**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_claude_check.py"`
Expected: `CLAUDE.md names Kalshi as a published venue`, exit code 0.

Run: `nix develop --command python3 scripts/gen_registry.py --check`
Expected: exit code 0.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(claude): Kalshi is a published venue, with its intervals, channels, re-snapshot and book (Epic 9, Story 9)" -- CLAUDE.md
```

### Task 9.2: The README, the spec index, SELF-HEALING and `.env.example`

The hand-written lead-ins of three documents say "Polymarket and Binance" or "both venues", and `.env.example` documents no Kalshi key. The generated rows of `README.md` and `docs/specs/INDEX.md` that name Kalshi (including those fed by the root manifest's `mirrors.kalshi` strings) are Story 8's.

**Files:**
- Modify: `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.env.example`
- Test: a throwaway check script outside the tree

- [ ] **Step 1: Write the failing check**

```bash
cat > "${TMPDIR:-/tmp}/epic9_docs_check.py" <<'PY'
import sys
from pathlib import Path

CHECKS = {
    "README.md": (
        ["Kalshi's event-contract market data", "`kalshi-<module>`", "`polyoxide::kalshi`",
         "--features kalshi-predictions-ws"],
        [],
    ),
    "docs/specs/INDEX.md": (
        ["https://docs.kalshi.com (Kalshi's)", "trade across venues"],
        [],
    ),
    "SELF-HEALING.md": (
        ["Polymarket, Binance and Kalshi ship schema", "and so does Kalshi's",
         "Kalshi's demo-host tests", "`KALSHI_DEMO_*`"],
        [],
    ),
    ".env.example": (["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"], []),
}
problems = []
for name, (present, absent) in CHECKS.items():
    text = Path(name).read_text(encoding="utf-8")
    problems += [f"{name} still says: {p}" for p in absent if p in text]
    problems += [f"{name} lacks: {p}" for p in present if p not in text]
print("\n".join(problems) or "the docs name Kalshi as a published venue")
sys.exit(1 if problems else 0)
PY
```

- [ ] **Step 2: Run it to see it fail**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_docs_check.py"`
Expected: a `lacks:` line for each phrase above, then exit code 1.

- [ ] **Step 3: Implement**

Apply the edits below with the editor's exact-string replace, each old text hand-written and unique in its file today. Apply each unconditionally; an edit whose old text matches nothing, or more than once, stops the task: report it.

In `README.md`:

**R1** (the opening paragraph). Replace

```text
Polymarket's APIs and Binance's USDⓈ-M futures market data, one
```

with

```text
Polymarket's APIs, Binance's USDⓈ-M futures market data and Kalshi's event-contract market data, one
```

**R2** (`### Libraries`, the sentence on feature names). Replace

```text
`polymarket-<module>` and `binance-<module>`,
```

with

```text
`polymarket-<module>`, `binance-<module>` and `kalshi-<module>`,
```

**R3** (the same sentence, its second line). Replace

```text
`polyoxide::polymarket` and `polyoxide::binance`. Or install
```

with

```text
`polyoxide::polymarket`, `polyoxide::binance` and `polyoxide::kalshi`. Or install
```

**R4** (the install examples, after Binance's). Replace

```text
cargo add polyoxide --no-default-features --features binance-usdm-ws
```

with

```text
cargo add polyoxide --no-default-features --features binance-usdm-ws

# Kalshi event-contract market data and its streams
cargo add polyoxide --no-default-features --features kalshi-predictions-ws
```

In `docs/specs/INDEX.md`:

**I1** (the opening paragraph, its last line). Replace

```text
for USDⓈ-M, as its hosts were observed to behave ([Other venues](#other-venues)).
```

with

```text
for USDⓈ-M, as its hosts were observed to behave, and Kalshi's, which publishes an OpenAPI and an AsyncAPI, as it publishes them, with what its hosts were observed to do beside them ([Other venues](#other-venues)).
```

**I2** (the line above the `index-upstream` region). Replace

```text
Polymarket's specs are sourced from https://docs.polymarket.com and the OpenAPI specs at:
```

with

```text
The specs are sourced from https://docs.polymarket.com (Polymarket's) and https://docs.kalshi.com (Kalshi's), and the OpenAPI specs at:
```

**I3** (`## Other venues`, its lead-in). Replace

```text
Not Polymarket hosts. Read by a polyoxide crate for consumers that trade both venues:
```

with

```text
Not Polymarket hosts. Read by a polyoxide crate for consumers that trade across venues:
```

In `SELF-HEALING.md`:

**S1** (the opening paragraph, its first line). Replace

```text
Polyoxide tracks venues it does not control. Polymarket and Binance ship schema
```

with

```text
Polyoxide tracks venues it does not control. Polymarket, Binance and Kalshi ship schema
```

**S2** (the same paragraph, the sentence on published specs). Replace

```text
ways, in the live tests and against the vendored mirrors; Binance publishes none
```

with

```text
ways, in the live tests and against the vendored mirrors, and so does Kalshi's; Binance publishes none
```

**S3** (`Enabling authenticated coverage`, its first line). Replace

```text
session-key round trip and relay's credentialed tests): set the
```

with

```text
session-key round trip, relay's credentialed tests and Kalshi's demo-host tests): set the
```

**S4** (the same bullet, the secrets sentence). Replace

```text
`POLYMARKET_*`, `BUILDER_*` and `RELAYER_*` repo secrets each target declares.
```

with

```text
`POLYMARKET_*`, `BUILDER_*`, `RELAYER_*` and `KALSHI_DEMO_*` repo secrets each target declares.
```

In `.env.example`:

**V1** (the file's last line). Replace

```text
# CHAIN_ID=137
```

with

```text
# CHAIN_ID=137

# ── Kalshi: demo-host live tests (optional) ──────────────────────────────────
# `live_predictions_account` and `live_predictions_ws` in `polyoxide-kalshi` run against Kalshi's
# demo host with a demo API key; unset, they fail as auth-gated and are skipped. Keys do not cross
# environments, so a production key does not work. The private key is the PEM text, which spans
# several lines, so pass it from the shell instead of this file:
#     KALSHI_DEMO_API_KEY_ID=... KALSHI_DEMO_PRIVATE_KEY="$(cat demo.pem)" \
#         cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws -- --ignored
# KALSHI_DEMO_API_KEY_ID=your-demo-api-key-id
# KALSHI_DEMO_PRIVATE_KEY=the-pem-text-of-your-demo-key
```

Then confirm no generated region moved: run `nix develop --command python3 scripts/gen_registry.py --write`, then `git status --short`, which must list `.env.example`, `README.md`, `SELF-HEALING.md` and `docs/specs/INDEX.md`, and no other path. Another path means a generated region was stale before this task: stop and report it.

- [ ] **Step 4: Run it to see it pass**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_docs_check.py"`
Expected: `the docs name Kalshi as a published venue`, exit code 0.

Run: `nix develop --command python3 scripts/gen_registry.py --check`
Expected: exit code 0.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py -q'`
Expected: every test passes.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs: the README, spec index, SELF-HEALING and .env.example name a published Kalshi (Epic 9, Story 9)" -- README.md docs/specs/INDEX.md SELF-HEALING.md .env.example
```

### Task 9.3: The release's consumer-impact notes

`docs/s3-migration.md` holds each S3 piece of work's consumer-impact note for the CHANGELOG section of the release that ships it, and the `0.42.0` commit quotes this one (spec D16). The notes say what is new and that none of it breaks.

**Files:**
- Modify: `docs/s3-migration.md` (the opening paragraph, and a section appended at the end)
- Test: a throwaway check script outside the tree

- [ ] **Step 1: Write the failing check**

```bash
cat > "${TMPDIR:-/tmp}/epic9_migration_check.py" <<'PY'
import sys
from pathlib import Path

text = Path("docs/s3-migration.md").read_text(encoding="utf-8")
problems = []
for phrase in [
    "## Epic 9: Kalshi published",
    "**Consumer impact: additive.**",
    "`Candles::intervals()`",
    "`CandleQuery::ensure_served`",
    "`UnservedInterval`",
    "`Protocol::follow_up`",
    "`kalshi-predictions-ws`",
    "`PredictionsBooks`",
    "and a third venue, Kalshi.",
]:
    if phrase not in text:
        problems.append(f"lacks: {phrase}")
if "and the Kalshi skeleton" in text:
    problems.append("still says: and the Kalshi skeleton")
print("\n".join(problems) or "the migration notes cover Epic 9")
sys.exit(1 if problems else 0)
PY
```

- [ ] **Step 2: Run it to see it fail**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_migration_check.py"`
Expected: a `lacks:` line for each of the nine phrases and `still says: and the Kalshi skeleton`, then exit code 1.

- [ ] **Step 3: Implement**

**G1** (the opening paragraph). Replace

```text
supervised clob socket, and the Kalshi skeleton. Its removals, which are few, are listed in
```

with

```text
supervised clob socket, and a third venue, Kalshi. Its removals, which are few, are listed in
```

Then append this section after the file's last line (the bullet ending `host takes 100.`), keeping one blank line before its heading:

```markdown
## Epic 9: Kalshi published

**Consumer impact: additive.** Nothing is removed or renamed, and no signature changes; code
that builds on 0.41 builds unchanged. `polyoxide-kalshi` is new on crates.io, and the umbrella's
default build, `polymarket`, gains no crate.

- `polyoxide-kalshi`, first published in 0.42.0 (it was `publish = false`). The module
  `predictions`, feature `predictions` (on by default): `Predictions` and `PredictionsBuilder`,
  for Kalshi's production and demo hosts, with optional `Credentials` that sign a keyed client's
  requests; the namespaces `health`, `exchange`, `markets` (`list`, `get`, `orderbook`, `trades`,
  `candlesticks` and `series_candlesticks`), `events` and `account`; `PredictionsError`;
  `predictions::key`, whose keys read `kalshi.predictions:<TICKER>:yes|no`; and
  `predictions::venue`, whose `Predictions` implements `MarketData`, `Trades`, `EventGroups`
  and `Candles` with `InstrumentExt`, `TradeExt`, `EventGroupExt` and `CandleExt`. Its candles
  come at one minute, one hour and one day.
- `polyoxide-kalshi`, with `predictions-ws`: `PredictionsWsBuilder` connects a
  `SupervisedPredictionsWs` on the three channels `orderbook_delta`, `ticker` and `trade`
  (`Channel`, chosen with `channels`, the order book by default). It yields `Event::Snapshot`,
  `Delta`, `Ticker`, `Trade`, `Resyncing`, `Disconnected` and `Reconnected`. A gap in the
  order-book sequence asks Kalshi for fresh snapshots on the same connection instead of
  reconnecting (`PredictionsMembership::snapshot` asks on demand), and `ws::book::Books` keeps
  each ticker's ladder from the events. The handshake needs `Credentials`, public data
  included.
- The umbrella: the features `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws`, all in
  `full` and none in the default; `polyoxide::kalshi`; `polyoxide::parse_key` names
  `kalshi.predictions` under `kalshi-predictions`; and the prelude names `Predictions`,
  `PredictionsBuilder`, `PredictionsError`, `KalshiCredentials`, `PredictionsEvent`,
  `PredictionsWsBuilder`, `SupervisedPredictionsWs`, `PredictionsWsError` and `PredictionsBooks`.
- `polyoxide-venue`, feature `market`: `Candles::intervals()`, a provided method that returns
  the widths a venue serves, `CandleInterval::ALL` by default, so every existing implementation
  keeps its behaviour and `DynCandles` forwards it. `CandleQuery::ensure_served` and the new
  `UnservedInterval` (`InvalidRequest`, a fault) let a venue that serves fewer widths refuse
  another before it sends anything.
- `polyoxide-ws`: `Protocol::follow_up`, a provided method that returns no frame by default. The
  `Supervisor` calls it once after each `decode` that leaves the connection up and sends the
  frame it returns, bounded by `stale_after`. A `Protocol` written before it is unchanged.

No dependency tree of an existing build changes. `kalshi-predictions` builds core's HTTP stack,
as the other venues' HTTP modules do, and `kalshi-predictions-ws` adds the socket kit.
```

- [ ] **Step 4: Run it to see it pass**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_migration_check.py"`
Expected: `the migration notes cover Epic 9`, exit code 0.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(migration): Epic 9's consumer-impact notes (Epic 9, Story 9)" -- docs/s3-migration.md
```

### Task 9.4: A Kalshi vacuity guard in the classify sweep

`test_classify_coverage.py` sweeps every `polyoxide*` crate for public error types, and Kalshi's already pass it, but its vacuity guards (`test_the_sweep_finds_the_known_types` and `test_the_ws_sweep_finds_every_socket_error`) list no Kalshi type, so a sweep that stopped reading Kalshi's crate would pass unseen. This task appends the guards in tests of their own, leaving every existing test and its parameter list as it is, and adds `UnservedInterval`, Story 2's refusal, to the sweep's auxiliary names, since it neither ends in `Error` nor is found by the sweep's other rules if its `Error` impl is spelled another way.

**Files:**
- Modify: `.github/scripts/tests/test_classify_coverage.py` (the `AUXILIARY` set near the top of the file, and four tests appended after the last one)
- Test: `.github/scripts/tests/test_classify_coverage.py`

- [ ] **Step 1: Write the failing tests**

Append after the file's last test, `test_sources_are_read_as_utf8`, keeping two blank lines before the first:

```python
@pytest.mark.parametrize(("crate", "names"), [
    ("polyoxide-kalshi", {"PredictionsError", "VenueError", "InvalidTicker", "CredentialsError",
                          "PredictionsWsError"}),
    (VOCABULARY, {"UnservedInterval"}),
])
def test_the_sweep_finds_kalshis_types_and_the_candle_refusal(crate: str, names: set[str]) -> None:
    # A sweep that read nothing of Kalshi's crate, or missed a refusal that is not named
    # `...Error`, would pass the sweep above vacuously.
    assert names <= set(error_types(REPO / crate / "src"))


def test_kalshis_assertion_names_every_error_type_the_crate_has() -> None:
    listed = asserted(REPO / "polyoxide-kalshi" / "src" / "lib.rs")
    assert listed is not None, "polyoxide-kalshi/src/lib.rs has no Classify assertion"
    assert {"PredictionsError", "VenueError", "InvalidTicker", "CredentialsError",
            "PredictionsWsError"} <= listed


def test_the_candle_refusal_is_in_the_vocabulary_crates_assertion() -> None:
    listed = asserted(REPO / VOCABULARY / "src" / "lib.rs")
    assert listed is not None, "polyoxide-venue/src/lib.rs has no Classify assertion"
    assert "UnservedInterval" in listed


def test_the_ws_sweep_finds_kalshis_socket_error() -> None:
    # The fences above would pass vacuously if the sweep never found this wrapper.
    [path] = [path for found, path in ws_wrappers(REPO / "polyoxide-kalshi" / "src")
              if found == "PredictionsWsError"]
    assert expands_the_table(path)
    assert uncalled(path) == []
```

- [ ] **Step 2: Prove the guards can fail**

Remove one of Kalshi's assertion lines on a backed-up copy, run the file, and restore the copy.

Run:
```bash
cp polyoxide-kalshi/src/lib.rs "${TMPDIR:-/tmp}/kalshi-lib.rs.bak" && sed -i '/is::<predictions::key::InvalidTicker>/d' polyoxide-kalshi/src/lib.rs && nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'; cp "${TMPDIR:-/tmp}/kalshi-lib.rs.bak" polyoxide-kalshi/src/lib.rs && git diff --stat -- polyoxide-kalshi/src/lib.rs
```
Expected: pytest reports two failures, `test_every_public_error_type_is_listed_in_its_crates_assertion` (its message ends with `polyoxide-kalshi: InvalidTicker (polyoxide-kalshi/src/predictions/key.rs)`) and `test_kalshis_assertion_names_every_error_type_the_crate_has`; the final `git diff --stat` prints nothing, which shows the file is restored.

- [ ] **Step 3: Implement**

In `.github/scripts/tests/test_classify_coverage.py`, replace

```python
AUXILIARY = frozenset({"UnknownVariant", "InvalidSymbol", "InvalidStreamName", "BurstCapacityExceeded"})
```

with

```python
AUXILIARY = frozenset({"UnknownVariant", "InvalidSymbol", "InvalidStreamName", "BurstCapacityExceeded",
                       "UnservedInterval"})
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'`
Expected: every test passes, the four new ones included.

Now prove the `UnservedInterval` guard on the venue crate the same way. Run `grep -n 'UnservedInterval' polyoxide-venue/src/lib.rs` to see its assertion line, then:
```bash
cp polyoxide-venue/src/lib.rs "${TMPDIR:-/tmp}/venue-lib.rs.bak" && sed -i '/is::<[A-Za-z_:]*UnservedInterval>/d' polyoxide-venue/src/lib.rs && nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'; cp "${TMPDIR:-/tmp}/venue-lib.rs.bak" polyoxide-venue/src/lib.rs && git diff --stat -- polyoxide-venue/src/lib.rs
```
Expected: two failures, `test_every_public_error_type_is_listed_in_its_crates_assertion` (naming `polyoxide-venue: UnservedInterval`) and `test_the_candle_refusal_is_in_the_vocabulary_crates_assertion`; the final `git diff --stat` prints nothing. If the `sed` matched no line (the assertion is spelled another way), nothing fails: edit the pattern to the line `grep` showed, and rerun.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
Expected: the whole suite passes.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(ci): the classify sweep knows Kalshi's error types and the candle refusal (Epic 9, Story 9)" -- .github/scripts/tests/test_classify_coverage.py
```

### Task 9.5: The epic's spine amendments

`epic-9.md` records A9-1 to A9-4 in the format of `epic-8.md` (title line, a sentence on AD-21, then one section per amendment with its proposer, current rule, proposed rule, why and follow-up in the code). Merging them into the spine and regenerating `docs/ARCHITECTURE.md` is an owner session, not this one.

**Files:**
- Create: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md`
- Test: a throwaway check script outside the tree

- [ ] **Step 1: Write the failing check**

```bash
cat > "${TMPDIR:-/tmp}/epic9_amendments_check.py" <<'PY'
import re
import sys
from pathlib import Path

path = Path("_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md")
if not path.exists():
    sys.exit(f"missing: {path}")
text = path.read_text(encoding="utf-8")
headings = re.findall(r"^## (A9-\d+): ", text, re.M)
problems = []
if headings[:4] != ["A9-1", "A9-2", "A9-3", "A9-4"]:
    problems.append(f"headings are {headings}")
for field in ("Proposed by", "Current rule", "Proposed rule", "Why", "Follow-up in the code"):
    count = text.count(f"- **{field}")
    if count < len(headings):
        problems.append(f"{field}: {count} for {len(headings)} amendments")
for phrase in ("`Candles::intervals()`", "`Protocol::follow_up`", "A8-3", "empty subscription"):
    if phrase not in text:
        problems.append(f"lacks: {phrase}")
print("\n".join(problems) or f"{len(headings)} amendments, each complete")
sys.exit(1 if problems else 0)
PY
```

- [ ] **Step 2: Run it to see it fail**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_amendments_check.py"`
Expected: `missing: _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md`, exit code 1.

- [ ] **Step 3: Implement**

Create `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md` with this text:


```markdown
# Proposed spine amendments: Epic 9

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A9-1: the Kalshi crate is published and in the umbrella (amends AD-16, and the bindings of AD-7 and AD-11)

- **Proposed by:** the owner, 2026-10-19, during Epic 9's design (spec §1.3, D12 and D13, in `docs/superpowers/specs/2026-10-19-epic-9-kalshi-published-design.md`).
- **Current rule:**
  - AD-16 lists, among S3's additions, "the Kalshi skeleton, `publish = false` until Kalshi integration".
  - AD-7 and AD-11 bind "the Kalshi skeleton".
- **Proposed rule:**
  - S3's additions list the Kalshi venue crate, published and re-exported by the umbrella as `polyoxide::kalshi`, and AD-7 and AD-11 bind the Kalshi crate.
  - The crate drops `publish = false`. The umbrella gains `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws`, all in `full` and none in its default.
- **Why:**
  - **The skeleton did its job.** Epic 8 built it to show that a venue lands by the guide's recipe. Epic 9 fills the market-data gaps it deferred, so the first version on crates.io is the complete one.
  - **"Integration" is the umbrella.** A venue that the umbrella re-exports is how a consumer of both venues reaches it, and the guide's recipe already ends there.
- **Follow-up in the code:** Story 8 drops `publish = false`, gives the workspace pin a version, adds the umbrella's features, passthrough, `parse_key` arm and prelude names, and the dependency-allowlist rules. The release commit, `0.42.0`, is an owner session after the run (spec §12 and D16). The merge session edits AD-16, AD-7 and AD-11, and the guide's text.

## A9-2: `Candles::intervals()` and the refusal of an unserved width (resolves A8-3, amends AD-6)

- **Proposed by:** the owner, 2026-10-19, who chose the form (spec §1.3, D2 and D3).
- **Current rule (AD-6):** optional abilities are separate traits, implemented only where offered, and no method returns `Unsupported`. `CandleInterval` names one minute to one week, "each width every candle venue serves". A8-3 left the choice between a venue declaring its widths and a venue refusing the others.
- **Proposed rule:**
  - Both, in the form that adds no method answering "unsupported". `Candles` has a provided method, `intervals(&self) -> &'static [CandleInterval]`, whose default is `CandleInterval::ALL`, so a caller can ask before it calls and a venue that serves fewer widths overrides it.
  - A query whose width is outside `intervals()` is refused as `InvalidRequest` with nothing sent, as a key of another product is. `CandleQuery::ensure_served` makes the refusal, `UnservedInterval`, which implements `Classify` as `InvalidRequest` and a fault, and an implementation converts it to `ClassifiedError` with `?`.
  - The CLOB, perps and USDⓈ-M keep the default. Kalshi serves one minute, one hour and one day.
  - `CandleInterval`'s "each width every candle venue serves" no longer holds, and the guide and the traits' docs stop saying it.
- **Why:**
  - **Kalshi serves three widths.** Its batch candlestick route answers another `period_interval` with a `200` and no rows (`docs/specs/kalshi/OBSERVED.md`), so the venue itself does not guard the width and the refusal is the only guard.
  - **A provided method is additive.** Every implementation written before it keeps its behaviour, and the change is semver-minor.
  - **A refused request is not an unsupported operation.** The venue offers `Candles`; the caller asked for a width it does not serve.
- **Follow-up in the code:** Story 2 adds the method, `ensure_served` and `UnservedInterval` to `polyoxide-venue`, and Story 3 implements `Candles` for Kalshi. The merge session edits AD-6 and the guide.

## A9-3: `Protocol::follow_up` lets a protocol send a frame from `decode` (amends AD-11 and the guide's step 4)

- **Proposed by:** Epic 9's design, 2026-10-19 (spec D7 and D9).
- **Current rule:** AD-11 says a venue implements only a `Protocol`, and lists its parts. Every frame a protocol sends is the handshake, a ping, or a membership frame from `replay` or `apply`. `decode` returns events and sends nothing.
- **Proposed rule:**
  - The list gains an optional part, `follow_up`: one frame the protocol sends after a `decode` that leaves the connection up. By default there is none.
  - The `Supervisor` calls it once after each such `decode` and sends the frame it returns, bounded by `stale_after` and without awaiting an answer, which comes back through `decode`. A send that fails or times out ends the connection as a failed ping does. A frame the protocol still holds when the connection ends stays with the protocol, which clears it in `reset`.
- **Why:**
  - **Kalshi's recovery from a gap needs it.** A gap in the order-book sequence is seen in `decode`, and the cheaper recovery is a `get_snapshot` on the same connection. `replay` and `apply` run only for a membership change, and a ping only on its schedule, so reconnecting was the only recovery the kit allowed.
  - **It is additive and venue-neutral.** The default changes no existing protocol, and the kit's docs name no venue.
- **Follow-up in the code:** Story 4 adds the method and its call, with three tests in `polyoxide-ws/src/supervisor/tests.rs`, and Story 5 uses it for the re-snapshot. The merge session edits AD-11 and the guide.

## A9-4: a venue that reads an empty subscription as "everything" must never send one (amends AD-11)

- **Proposed by:** Epic 9's design, 2026-10-19 (spec D8).
- **Current rule:** AD-11's membership rules say that Binance answers false on empty, that perps and the clob market channel answer true, and that the clob user channel's membership is an `Option` so that an empty `Some` never collapses into `None`, which means every market. They do not say what a venue sends for an empty membership.
- **Proposed rule:**
  - Where a venue reads a subscription with no markets as every market, its protocol sends no `subscribe` while the membership is empty, sends an `unsubscribe` for every subscription when the membership empties, and sends a fresh `subscribe` at the first add.
  - The kit is unchanged: the rule is the protocol's `replay` and `apply`.
- **Why:**
  - **Kalshi widens an empty subscribe on two channels.** It reads a `ticker` or `trade` subscribe with no markets as every market (`docs/specs/kalshi/OBSERVED.md`, Epic 9), so replaying an empty membership would subscribe the consumer to the whole exchange. AD-11's "Prevents" list already names widened subscriptions.
  - **Epic 8 had the rule for one channel.** Its protocol sent no `subscribe` for an empty order-book membership; Epic 9 makes that the rule for all three channels.
- **Follow-up in the code:** Story 5 applies the rule to all three channels, and its supervision tests pin the empty replay and the first add. The merge session adds the rule to AD-11.
```

If the dispatch of this story names further gaps the run met (a spine rule, a recipe step or a contract that a story had to work around), append one section per gap after A9-4, numbered A9-5 onward, in the same five fields. If it names none, the file ends after A9-4 and nothing is added.

- [ ] **Step 4: Run it to see it pass**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_amendments_check.py"`
Expected: `4 amendments, each complete` (more if gaps were appended), exit code 0.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`.

- [ ] **Step 5: Commit**

```bash
git add -N _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md
git commit -m "docs(spine): Epic 9's amendments A9-1 to A9-4 (Epic 9, Story 9)" -- _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md
```

### Task 9.6: Close what the epic settled in `deferred-work.md`

Six entries that Epic 8 filed are settled by this epic: the anonymous soak and the signed-bucket probe (Story 7, which does not touch this file, so this task closes both with Story 7's commit, spec D15), the local book with the channels and `get_snapshot` (Stories 5 and 6), A8-10's cargo-hack gap (Story 8), A8-3's `Candles` decision (Stories 2 and 3), and the doc debt (this story). Each gets a `status:` line in the form the file already uses (`status: Done in …`) naming the commits that settled it. The schema corrections of 2026-10-22, the Kalshi trading probes, the trading mapping's move and the maker-frame capture stay open.

**Files:**
- Modify: `_bmad-output/implementation-artifacts/deferred-work.md`
- Test: a throwaway check script outside the tree

- [ ] **Step 1: Write the failing check**

```bash
cat > "${TMPDIR:-/tmp}/epic9_deferred_check.py" <<'PY'
import sys
from pathlib import Path

CLOSED = [
    "Soak Kalshi's unauthenticated read limit over distinct URLs",
    "Probe which Read bucket a signed public read draws from",
    "Give `predictions::ws` a local order book",
    "Make CI check `polyoxide-kalshi`'s features one at a time",
    "Decide how `Candles` serves a venue that offers fixed widths",
    "Mention Kalshi in the hand-written docs that Epic 8 left alone",
]
OPEN = [
    "Adopt Kalshi's schema corrections dated 2026-10-22",
    "Run the eight Kalshi demo-host probes",
    "Move the Kalshi trading mapping from",
    "Capture a user-channel `trade` frame in which the account is only a maker",
]
lines = Path("_bmad-output/implementation-artifacts/deferred-work.md").read_text(encoding="utf-8").split("\n")
starts = [i for i, line in enumerate(lines) if line.startswith("- source_spec:")]
blocks = [lines[a:b] for a, b in zip(starts, starts[1:] + [len(lines)])]


def status(prefix: str) -> str | None:
    for block in blocks:
        if any(line.startswith("  summary: " + prefix) for line in block):
            return next((line for line in block if line.startswith("  status:")), None)
    raise SystemExit(f"no entry starts with: {prefix}")


problems = []
for prefix in CLOSED:
    line = status(prefix)
    if line is None or not line.startswith("  status: Done in Epic 9"):
        problems.append(f"not closed: {prefix}")
for prefix in OPEN:
    if status(prefix) is not None:
        problems.append(f"closed by mistake: {prefix}")
print("\n".join(problems) or "six entries closed, four left open")
sys.exit(1 if problems else 0)
PY
```

- [ ] **Step 2: Run it to see it fail**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_deferred_check.py"`
Expected: six `not closed:` lines, one for each entry above, then exit code 1. If it ends with `no entry starts with: …` instead, the entry's wording changed since this plan: read the file's last forty lines and use the entry's own opening words in the check and in Step 3.

- [ ] **Step 3: Implement**

Find the commit that settled each piece. Run:

```bash
for n in 2 3 5 6 7 8; do echo "Story $n: $(git log --format=%h -1 --grep="Epic 9, Story $n")"; done
git log --format='%h %s' -1 --grep='Kalshi is a published venue'
git log --format='%h %s' -1 --grep='name a published Kalshi'
git log --format='%h %s' -1 --grep='classify sweep knows Kalshi'
```

Expected: one short SHA after each `Story n:` (the story's latest commit, which is the one that settled it; if a story's commits do not carry `Epic 9, Story n`, find it with `git log --oneline`), and the three Task 9.1, 9.2 and 9.4 commits of this story. Then run the script below, which writes the six `status:` lines, each after its entry's last line, and fails without writing when a SHA is missing, an entry is not found, or one already has a status:

```bash
S2=$(git log --format=%h -1 --grep='Epic 9, Story 2') \
S3=$(git log --format=%h -1 --grep='Epic 9, Story 3') \
S5=$(git log --format=%h -1 --grep='Epic 9, Story 5') \
S6=$(git log --format=%h -1 --grep='Epic 9, Story 6') \
S7=$(git log --format=%h -1 --grep='Epic 9, Story 7') \
S8=$(git log --format=%h -1 --grep='Epic 9, Story 8') \
DOCS1=$(git log --format=%h -1 --grep='Kalshi is a published venue') \
DOCS2=$(git log --format=%h -1 --grep='name a published Kalshi') \
GUARD=$(git log --format=%h -1 --grep='classify sweep knows Kalshi') \
python3 -I - <<'PY'
import os
from pathlib import Path

path = Path("_bmad-output/implementation-artifacts/deferred-work.md")
names = ("S2", "S3", "S5", "S6", "S7", "S8", "DOCS1", "DOCS2", "GUARD")
sha = {name: os.environ.get(name, "") for name in names}
missing = [name for name, value in sha.items() if not value]
if missing:
    raise SystemExit(f"no commit found for {missing}: find each with git log --oneline and export it")

closings = [
    ("Soak Kalshi's unauthenticated read limit over distinct URLs",
     f"Done in Epic 9, Story 7 ({sha['S7']}): `examples/read_soak.rs` soaked the unauthenticated routes over distinct URLs, and the `### Rate limits` subsection of `docs/specs/kalshi/OBSERVED.md`'s Epic 9 section records the runs and the sizing they gave `throttle.rs`."),
    ("Probe which Read bucket a signed public read draws from",
     f"Done in Epic 9, Story 7 ({sha['S7']}): the probe ran on the demo host, and its answer and the sizing of a keyed client's public reads are in the same `### Rate limits` subsection."),
    ("Give `predictions::ws` a local order book",
     f"Done in Epic 9, Stories 5 and 6 ({sha['S5']}, {sha['S6']}): the `ticker` and `trade` channels, `get_snapshot` and the in-place re-snapshot on a gap are Story 5's, and `ws::book`'s `Books` is Story 6's."),
    ("Make CI check `polyoxide-kalshi`'s features one at a time",
     f"Done in Epic 9, Story 8 ({sha['S8']}): the crate is published, so `--ignore-private` no longer skips it, and the step needed no `ci.yml` edit (A8-10, spec D14)."),
    ("Decide how `Candles` serves a venue that offers fixed widths",
     f"Done in Epic 9, Stories 2 and 3 ({sha['S2']}, {sha['S3']}): `Candles::intervals()` and the `UnservedInterval` refusal (A9-2), and Kalshi's `Candles` at one minute, one hour and one day."),
    ("Mention Kalshi in the hand-written docs that Epic 8 left alone",
     f"Done in Epic 9, Story 9 ({sha['DOCS1']}, {sha['DOCS2']}, {sha['GUARD']}): CLAUDE.md, README.md, `docs/specs/INDEX.md`, SELF-HEALING.md and `.env.example` name Kalshi as a published venue, and `test_classify_coverage.py` holds its error types."),
]

lines = path.read_text(encoding="utf-8").split("\n")
starts = [i for i, line in enumerate(lines) if line.startswith("- source_spec:")]
ends = starts[1:] + [len(lines)]
inserts = {}
for start, end in zip(starts, ends):
    block = lines[start:end]
    summary = next((line for line in block if line.startswith("  summary: ")), "")
    for prefix, status in closings:
        if summary.startswith("  summary: " + prefix):
            if any(line.startswith("  status:") for line in block):
                raise SystemExit(f"already has a status: {prefix}")
            last = max(i for i in range(start, end) if lines[i].strip())
            inserts[last] = f"  status: {status}"
if len(inserts) != len(closings):
    raise SystemExit(f"matched {len(inserts)} of {len(closings)} entries")
for index in sorted(inserts, reverse=True):
    lines.insert(index + 1, inserts[index])
path.write_text("\n".join(lines), encoding="utf-8")
PY
```

- [ ] **Step 4: Run it to see it pass**

Run: `python3 -I "${TMPDIR:-/tmp}/epic9_deferred_check.py"`
Expected: `six entries closed, four left open`, exit code 0.

Run: `git diff --stat -- _bmad-output/implementation-artifacts/deferred-work.md`
Expected: one file, six insertions, no deletions.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(deferred-work): close what Epic 9 settled (Epic 9, Story 9)" -- _bmad-output/implementation-artifacts/deferred-work.md
```

**Final check of the story.**

Run the entry's **Verify** line with the dev shell's prefix, then the profile's gates a docs story can move:

```bash
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command cargo test --doc -p polyoxide-kalshi -p polyoxide --all-features
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/ -q'
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
bash _bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.sh
```

Each exits 0, and the touch-set gate prints no path. The story changes no Rust, so fmt, clippy, the Rust unit tests, rustdoc and `test_body_diff.py` have nothing new to run on.
