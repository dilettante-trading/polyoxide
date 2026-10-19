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
- `src/predictions/venue.rs` (modify): `impl Candles`, `CandleExt` and `Ohlc`, and the module docs.
- `tests/mock_predictions_routes.rs`, `tests/query_setters_predictions.rs`, `tests/spec_agreement_predictions.rs`, `tests/wire_agreement_predictions.rs`, `tests/venue_predictions.rs`, `tests/live_predictions_venue.rs` (modify, append only).
- `docs/specs/kalshi/OBSERVED.md` (modify, above the anchor, only for a finding of its own).

**Story 4: `Protocol::follow_up`.**

- `polyoxide-ws/src/supervisor/mod.rs` (modify): the provided method.
- `polyoxide-ws/src/supervisor/task.rs` (modify): the call after `decode` and the bounded send.
- `polyoxide-ws/src/supervisor/tests.rs` (modify, append only).

**Story 5: the socket protocol.**

- `src/predictions/ws/builder.rs`, `ws/protocol.rs`, `ws/event.rs`, `ws/error.rs` (modify).
- `src/predictions/ws/scripted.rs` (modify): the answerer learns several channels and `get_snapshot`.
- `src/predictions/ws/mod.rs` (modify): the module block the names contract gives for Story 5.
- `tests/supervision_predictions.rs`, `tests/wire_agreement_predictions_ws.rs`, `tests/live_predictions_ws.rs` (modify, append only).
- `docs/specs/kalshi/OBSERVED.md` (modify, above the anchor, only for a finding of its own).

**Story 6: the local book.**

- `src/predictions/ws/book.rs` (create): `Books`, `OrderBook` and their unit tests.
- `src/predictions/ws/mod.rs` (modify): replaces Story 5's block whole with Story 6's.

**Story 7: limits.**

- `polyoxide-kalshi/Cargo.toml` (modify, by its anchors): the `read_soak` example and test-support's `soak` feature.
- `examples/read_soak.rs` (create).
- `src/predictions/throttle.rs` (modify): the unsigned sizing, and a keyed client's public-read bucket if the probe says so.
- `tests/mock_predictions_signed.rs` (modify, append only).
- `docs/specs/kalshi/OBSERVED.md` (modify, above the anchor): `## Rate limits` runs.

**Story 8: publish wiring.**

- `Cargo.toml` (modify): the `polyoxide-kalshi` workspace pin.
- `polyoxide-kalshi/Cargo.toml` (modify, by its anchors): `publish`, `description`, the `polyoxide` dev-dependency's features, the metadata `readme`.
- `polyoxide-kalshi/README.md`, `src/predictions/README.md`, `src/lib.rs` (modify): the crate docs drop "walking skeleton".
- `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`, `polyoxide/examples/venue_neutral.rs` (modify).
- `polyoxide/tests/venue_neutral.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs` (modify, append only).
- `ci/dep-allowlists/polyoxide@kalshi-predictions.txt`, `polyoxide@kalshi-predictions-ws.txt` (create, by `--write`).
- `.github/scripts/tests/test_dep_allowlists.py`, `.github/scripts/tests/test_ci_workflow.py` (modify).
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
- **The `OBSERVED.md` anchor.** Story 1 appends a section headed `## Epic 9` whose last line is, verbatim, `<!-- epic 9 findings go above this line -->`. Stories 3, 5 and 7 insert their findings directly above that line.
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
- `#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive] pub struct UnservedInterval { pub interval: CandleInterval, pub served: &'static [CandleInterval] }`, with `Display`, `std::error::Error` and `Classify` (`InvalidRequest`, a fault, no `retry_after`); `ClassifiedError: From<UnservedInterval>` through the existing blanket impl.

**Story 3 → Stories 8 and 9.**

- `impl polyoxide_venue::Candles for Predictions`, whose `intervals()` returns `&[CandleInterval::Minute1, CandleInterval::Hour1, CandleInterval::Day1]`.
- `predictions::venue::CandleExt { pub yes_bid: Ohlc, pub yes_ask: Ohlc, pub open_interest: Decimal, pub mean: Option<Decimal> }` and `predictions::venue::Ohlc { pub open: Decimal, pub high: Decimal, pub low: Decimal, pub close: Decimal }`, each `#[derive(Debug, Clone, PartialEq)] #[non_exhaustive]`.
- `MarketsApi::candlesticks(&self, ticker: &Ticker, period: PeriodInterval, start_ts: i64, end_ts: i64) -> GetCandlesticks` and `MarketsApi::series_candlesticks(&self, series_ticker: &str, ticker: &Ticker, period: PeriodInterval, start_ts: i64, end_ts: i64) -> GetSeriesCandlesticks`; `predictions::types::PeriodInterval { Minute1, Hour1, Day1 }`.

**Story 4 → Story 5.** In `polyoxide_ws::Protocol`: `fn follow_up(&mut self) -> Option<Message> { None }`, `Message` being the type `Protocol::ping` returns. The task calls it after every `decode` and sends what it returns, bounded by `stale_after`, a failed or timed-out send ending the connection as a failed ping does.

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

  Story 6 replaces it whole with the same block plus `pub mod book;` directly after `mod protocol;` and `pub use book::{Books, OrderBook};` directly after `pub use builder::PredictionsWsBuilder;`. The `SupervisedPredictionsWs` alias and the two URL consts below the block stay as they are.
- `pub enum Channel { OrderbookDelta, Ticker, Trade }` from `polyoxide_venue::wire_enum!` (`"orderbook_delta"`, `"ticker"`, `"trade"`), also `Ord`, in that order.
- `PredictionsWsBuilder::channels(self, channels: impl IntoIterator<Item = Channel>) -> Self`.
- `Event` gains `Ticker(TickerUpdate)`, `Trade(PublicTrade)` and `Resyncing { tickers: Vec<String> }`; `Snapshot(OrderbookSnapshot)`, `Delta(OrderbookDelta)`, `Disconnected { reason }` and `Reconnected` stay.
- `OrderbookSnapshot { pub market_ticker: String, pub market_id: String, pub bids: Vec<Level>, pub asks: Vec<Level> }` and `OrderbookDelta { pub market_ticker: String, pub side: BookSide, pub price: Decimal, pub delta: Decimal, pub ts: Option<UnixMillis>, pub client_order_id: Option<String> }` stay as Epic 8 left them, in YES pricing (`BookSide::Bid` a YES bid, `BookSide::Ask` a NO bid priced as a YES ask).
- `TickerUpdate` and `PublicTrade`, `#[derive(Debug, Clone, PartialEq)] #[non_exhaustive]`, with spec D8's fields, all `pub`; prices and sizes `Decimal`, `ts: UnixMillis`, `taker_outcome_side: OutcomeSide`, `taker_book_side: BookSide` (both from `crate::predictions::types`).
- `Change` gains `Snapshot(Vec<Ticker>)`; `PredictionsMembership` gains `fn snapshot(&self, tickers: impl IntoIterator<Item = Ticker>) -> impl Future<Output = Result<(), PredictionsWsError>> + Send`.
- `PredictionsWsError` gains `SnapshotTimeout { pub sid: u64, pub waited: Duration }`, classed `Unavailable`, with recovery `Reconnect`.

**Story 6 → Stories 8 and 9.** In `predictions::ws::book`, re-exported at `predictions::ws`:

- `#[derive(Debug, Default, Clone)] pub struct Books`, with `pub fn new() -> Self`, `pub fn apply(&mut self, event: &Event) -> Option<&OrderBook>`, `pub fn get(&self, market_ticker: &str) -> Option<&OrderBook>`.
- `#[derive(Debug, Clone, PartialEq)] pub struct OrderBook`, with `pub fn market_ticker(&self) -> &str`, `pub fn best_bid(&self) -> Option<Level>`, `pub fn best_ask(&self) -> Option<Level>`, `pub fn bids(&self) -> Vec<Level>` (highest first), `pub fn asks(&self) -> Vec<Level>` (lowest first), `pub fn levels(&self, outcome: Outcome) -> (Vec<Level>, Vec<Level>)` (that outcome's bids highest first and asks lowest first; `Outcome` from `crate::predictions::key`).

**Story 7 → Story 9.** `polyoxide-kalshi/examples/read_soak.rs`, run as `cargo run -p polyoxide-kalshi --example read_soak --features predictions -- --stages <stages> --routes <routes>`; the `## Rate limits` subsection of `OBSERVED.md`'s Epic 9 section.

**Story 8 → Story 9.** The umbrella's features `kalshi`, `kalshi-predictions`, `kalshi-predictions-ws`; `polyoxide::kalshi`; the prelude names of spec D13 (`Predictions`, `PredictionsBuilder`, `PredictionsError`, `KalshiCredentials`, `PredictionsEvent`, `PredictionsWsBuilder`, `SupervisedPredictionsWs`, `PredictionsWsError`, `PredictionsBooks`).

**Anchors in `polyoxide-kalshi/Cargo.toml`** (existing lines, quoted verbatim; a story inserts directly before or after one and never quotes another story's text):

- `publish = false` (Story 8 deletes it);
- the `description = "…"` line (Story 8 replaces it);
- `[package.metadata.polyoxide]` (Story 7 inserts its `[[example]]` stanza directly before the comment line above it, `# Registration, read by scripts/gen_registry.py; run it with --write after an edit.`);
- the `polyoxide = { path = "../polyoxide", … }` dev-dependency line (Story 8 replaces it);
- the `readme = "…"` line under `[package.metadata.polyoxide]` (Story 8 replaces it).

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
  - **Files:** `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs`, `polyoxide-kalshi/src/predictions/types/markets.rs`, `polyoxide-kalshi/src/predictions/venue.rs`, `polyoxide-kalshi/tests/mock_predictions_routes.rs`, `polyoxide-kalshi/tests/query_setters_predictions.rs`, `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions.rs`, `polyoxide-kalshi/tests/venue_predictions.rs`, `polyoxide-kalshi/tests/live_predictions_venue.rs`, `docs/specs/kalshi/OBSERVED.md`
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

  Adds `Books` and `OrderBook`, a pure value the consumer feeds Story 5's events into: a snapshot replaces a ticker's YES-priced ladder, a delta adds its signed size and removes a level at zero or below, a delta for a ticker with no snapshot is ignored, `Resyncing` drops its tickers' books and `Reconnected` drops all. `levels(outcome)` gives one outcome's view, a NO bid at *q* being a YES ask at 1 − *q*. No I/O and no clock. Unit tests, in the file, replay Story 1's captured snapshot and deltas and assert literal prices and sizes. Replaces `ws/mod.rs`'s module block with Story 6's.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/src/predictions/ws/book.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`
  - **Interfaces:** exposes "Story 6 → Stories 8 and 9"; uses Story 5's `Event`, `OrderbookSnapshot`, `OrderbookDelta` and module block, Story 1's ws fixtures, and `crate::predictions::key::Outcome` and `crate::predictions::types::BookSide`
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
  - **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/README.md`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/README.md`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`, `polyoxide/examples/venue_neutral.rs`, `polyoxide/tests/venue_neutral.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`, `ci/dep-allowlists/polyoxide@kalshi-predictions.txt`, `ci/dep-allowlists/polyoxide@kalshi-predictions-ws.txt`, `.github/scripts/tests/test_dep_allowlists.py`, `.github/scripts/tests/test_ci_workflow.py`, `README.md`, `CLAUDE.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml`
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
