# Epic 9: Kalshi published — design

Status: approved by the owner, 2026-10-19. This spec is the design record for the plan
`docs/plans/2026-10-19-epic-9-kalshi-published.md`. It builds on Epic 8's spec
(`docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`), whose decisions
stand unless this spec replaces one. Read it with the architecture spine
(`_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-SPINE.md`),
its amendments through `spine-amendments/epic-8.md`, the guide (`docs/ARCHITECTURE.md`) and
CLAUDE.md. Where this spec and an earlier record disagree, this spec binds.

## 1. Scope, stage and sources

**1.1 Goal.** Ship `polyoxide-kalshi` on crates.io as a complete market-data venue: fill the
market-data gaps Epic 8 deferred, then publish it, re-exported by the umbrella. The first version on
crates.io is the complete one. The crate has no semver baseline until it is published, so its API
may be reshaped freely before the release commit.

**1.2 Stage.** The workspace stays in S3. The epic adds API and removes none from any published
crate, so `docs/s3-removals.md` gains nothing. `polyoxide-kalshi` is absent from the removals
baseline (`v0.40.0`), which `api_removals.py` skips with a warning. The release is `0.42.0`,
one new crate name on crates.io, made after the run as its own commit on `main` (D16, §12).

**1.3 The owner's choices (2026-10-19).**

- Scope: market data plus the gap fill. `Trading` is not in this epic.
- Facades: the umbrella only. The CLI and the Python bindings do not change.
- `Candles` for a venue with fixed widths: a provided `Candles::intervals()` and a refusal of a
  width outside it (D2).
- A sequence gap on the order-book subscription re-snapshots in place with `get_snapshot`, falling
  back to a reconnect (D9).
- Order: fill the gaps first, publish last (approach A).

**1.4 Sources.** The vendored mirrors `docs/specs/kalshi/openapi.yaml` (3.34.0) and
`asyncapi.yaml` (2.0.0), `docs/specs/kalshi/OBSERVED.md`, and the probes of Story 1. Where a probe
and a mirror disagree, the probe wins and `OBSERVED.md` records it; the mirrors stay byte for byte.

**1.5 The plan's stories.**

| Story | What | Needs |
|---|---|---|
| 1 | Probes and captures: candlesticks, `ticker` and `trade` frames, `get_snapshot`, multi-`sid` updates | — |
| 2 | `polyoxide-venue`: `Candles::intervals()` and `UnservedInterval` | — |
| 3 | Kalshi's `Candles` | 1, 2 |
| 4 | `polyoxide-ws`: `Protocol::follow_up` | — |
| 5 | The socket protocol: the `ticker` and `trade` channels, `get_snapshot`, the gap re-snapshot | 1, 4 |
| 6 | The local book (`ws/book.rs`, pure) | 5 |
| 7 | Limits: the unsigned soak and the signed-bucket probe | 1 |
| 8 | Publish wiring: manifest, umbrella, CI | 3, 5, 6, 7 |
| 9 | Docs, records and amendments | 8 |

Stories 2 and 4 may run at any time and in parallel with Story 1. Story 7 may run in parallel with
Stories 2 to 6. No story quotes code another story writes: every edit to `ws/protocol.rs`,
`ws/event.rs`, `ws/error.rs` and `ws/builder.rs` is Story 5's, and Story 6 adds `ws/book.rs` and
uses Story 5's public names only (the plan's names contract).

**1.6 The design session's probe** (production, unsigned, 2026-10-19). `GET /markets/candlesticks`
for `KXMLBGAME-26OCT192008CLETB-TB` at 1, 60 and 1440 answered `200` with
`{markets: [{market_ticker, candlesticks}]}`. A traded period's `price` held `open`, `high`, `low`,
`close`, `mean` and `previous` (`_dollars`), the first traded period no `previous`; an untraded
period after a trade held only `previous_dollars`; a period before any trade held `price: {}`.
`min_dollars` and `max_dollars` never appeared. `period_interval=5` answered `200` with no rows on
the batch route and `400` (a validation `msg`) on the series route. Hourly candles end on the hour;
the daily candle ended at 04:00 UTC. Story 1 writes these to `OBSERVED.md` with its captures.

## 2. Decisions (binding)

- **D1. Approach A.** Gap fill first, publish last, one release.
- **D2. `Candles::intervals()`.** `polyoxide-venue`'s `Candles` gains a provided method
  `fn intervals(&self) -> &'static [CandleInterval] { CandleInterval::ALL }`. The trait docs say:
  a query whose `interval` is not in `intervals()` is refused as `InvalidRequest` with nothing
  sent. The CLOB, perps and USDⓈ-M keep the default, since each serves every width. Adding a
  provided method is additive (semver-minor).
- **D3. `UnservedInterval`.** `polyoxide-venue` (feature `market`) adds
  `#[non_exhaustive] pub struct UnservedInterval { pub interval: CandleInterval, pub served: &'static [CandleInterval] }`,
  `Display` "`<interval>` candles are not served; this venue serves <list>", `Error`, and
  `Classify` as `InvalidRequest`, `is_fault` true, no `retry_after`. It is in the crate's
  `Classify` assertion. `CandleQuery` gains
  `pub fn ensure_served(&self, served: &'static [CandleInterval]) -> Result<(), UnservedInterval>`,
  and an implementation converts the error to `ClassifiedError` with `?`, as it does `KeyError`.
- **D4. Kalshi's widths.** `Predictions::intervals()` returns `[Minute1, Hour1, Day1]`, sent as
  `period_interval` 1, 60 and 1440.
- **D5. Kalshi's candle route.** `candles` uses `GET /trade-api/v2/markets/candlesticks` with one
  ticker in `market_tickers`, which needs no series lookup and returns up to 10,000 candlesticks.
  The design session probed it on production, unsigned, on 2026-10-19 (§1.6): it answers `200`.
  `GET /series/{series_ticker}/markets/{ticker}/candlesticks` answers the same rows under
  `{candlesticks, ticker}`. Both routes are added to the route table with their costs, and the native
  client exposes both (`markets().candlesticks(..)` for the batch route,
  `markets().series_candlesticks(..)` for the series route), with query setters through
  `query_setters!`.
- **D6. Kalshi's candle mapping.** A field the venue has no value for is omitted from the JSON,
  never `null` (§1.6), so every `price` field and `previous_dollars` deserialize as `Option`. For key `kalshi.predictions:<T>:<outcome>` and width *w*:
  - The request sends `start_ts` = `query.start` rounded up to the second plus *w*, and `end_ts` =
    (`query.end`, or now) rounded down to the second plus *w*, since Kalshi selects candlesticks by
    their end and the trait by their open. `include_latest_before_start` is not sent.
  - `open_time` = `end_period_ts` − *w*. Rows whose `open_time` falls outside the query are
    dropped. The list is oldest first.
  - Prices are the trade price, `price.{open,high,low,close}_dollars`. For a `no` key each price
    *p* becomes 1 − *p*, and high and low swap.
  - A period with no trade (`price.close_dollars` null) has `close` = `price.previous_dollars`
    (mirrored for `no`) and no `open`, `high` or `low`. A period whose `previous_dollars` is also
    null is skipped.
  - `volume` = `volume_fp`, in `SizeUnit::Shares`. `notional` and `trades` are `None`.
  - `CandleExt { yes_bid: Ohlc, yes_ask: Ohlc, open_interest: Decimal, mean: Option<Decimal> }`,
    in YES terms as the venue sends them whatever the key's outcome, where
    `Ohlc { open, high, low, close: Decimal }` (`mean` mirrored for a `no` key like the prices).
  - A market settled before Kalshi's historical cutoff is served only by
    `/historical/markets/{ticker}/candlesticks`, which this epic does not implement: its answer from
    the live route passes through as the venue gave it (an empty list or a `VenueRefusal`), and the
    module docs say so.
  - A width outside D4 is refused by `ensure_served` before any request. The batch route itself
    does not check `period_interval` (it answers `5` with `200` and no rows, §1.6), so the refusal
    is the only guard; the native `markets().candlesticks(..)` takes a `PeriodInterval`
    (`wire_enum!`-style closed set `Minute1` → `1`, `Hour1` → `60`, `Day1` → `1440`, sent as its
    number), so it cannot send another width either.
- **D7. `Protocol::follow_up`.** `polyoxide-ws`'s `Protocol` gains a provided method
  `fn follow_up(&mut self) -> Option<Message> { None }`. The supervisor task calls it after each
  `decode` and sends what it returns on the same connection, bounded by `stale_after` as a ping is,
  without awaiting an answer; a send that fails or times out ends the connection as a failed ping
  does. Its docs name no venue. `src/supervisor/tests.rs` pins: a frame returned is sent after the
  decode that prompted it, `None` sends nothing, and a send that outlives `stale_after` reconnects.
- **D8. Channels.** `predictions::ws` gains `Channel` (`wire_enum!`: `OrderbookDelta` →
  `"orderbook_delta"`, `Ticker` → `"ticker"`, `Trade` → `"trade"`) and
  `PredictionsWsBuilder::channels(impl IntoIterator<Item = Channel>)`, deduplicated, at least one
  (an empty set is refused at `connect` as `InvalidRequest`). The default is `[OrderbookDelta]`,
  today's behaviour.
  - One ticker set serves every channel. The replay sends one `subscribe` naming every channel and
    the tickers; each channel's `subscribed` answer gives it its own `sid`, which the protocol keeps
    per channel.
  - A membership change sends one `update_subscription` per `sid`, in channel order
    (`OrderbookDelta`, `Ticker`, `Trade`), whatever Kalshi does with several `sids` in one command;
    the change is answered once every command is, and the first refusal fails the change and
    undoes it, as Epic 8's does. Story 1 records what a several-`sids` command does, for a later
    epic.
  - **An empty ticker set never subscribes.** Kalshi reads a `ticker` or `trade` subscribe with no
    markets as "every market", so the protocol sends no `subscribe` while the set is empty, emptying
    the set sends `unsubscribe` for every `sid`, and the first ticker added afterwards sends a fresh
    `subscribe`. Epic 8's rule for the order-book channel becomes the rule for all three.
  - New events: `Event::Ticker(TickerUpdate)` and `Event::Trade(PublicTrade)`.
    `TickerUpdate { market_ticker, market_id, price, yes_bid, yes_ask, yes_bid_size, yes_ask_size, volume, open_interest, dollar_volume, dollar_open_interest, last_trade_size, ts: UnixMillis }`
    from `tickerPayload` (`_dollars` fields as `Decimal`, `_fp` fields as `Decimal`, `ts` from
    `ts_ms`; the deprecated `ts` and `time` are not modelled).
    `PublicTrade { trade_id, market_ticker, yes_price, no_price, count, taker_outcome_side: OutcomeSide, taker_book_side: BookSide, is_block_trade, ts: UnixMillis }`
    from `tradePayload` (the deprecated `taker_side` is not modelled). Both structs are
    `#[non_exhaustive]`. Any field the capture shows absent becomes an `Option` and `OBSERVED.md`
    records it.
  - The `ticker` channel carries no `seq`. The `trade` channel does, and its gap reconnects
    (`SequenceGap`), since it has no snapshot.
- **D9. The gap re-snapshot.** A `seq` gap on the order-book `sid`:
  - yields `Event::Resyncing { tickers: Vec<String> }`, the whole membership, ahead of anything
    else from that frame, and drops the frame;
  - queues, through `follow_up`, one `update_subscription` with `action: "get_snapshot"` on that
    `sid` for every ticker in the membership, with a command id the protocol records as a
    follow-up;
  - re-anchors the `sid`'s sequence on each snapshot that answers the follow-up: its `seq` becomes
    the last seen, so the next frame on the `sid` must be `seq + 1`. Frames on the `sid` before
    the first such snapshot are not checked. If Story 1's capture shows the answers are not on the
    `sid`'s numbering, Story 6 stops and asks;
  - drops `orderbook_delta` frames on that `sid` until each requested ticker's snapshot arrives,
    then resumes them per ticker;
  - reconnects (`PredictionsWsError::SnapshotTimeout`, class `Unavailable`) when the snapshots are
    not all in by `stale_after` after the command was queued, checked on each decoded frame (the
    supervisor's own staleness covers silence), and reconnects on an error frame that answers the
    follow-up's id (the error, decoded as Epic 8's error frames are, with recovery `Reconnect`);
  - a second gap while one is being resynced reconnects.
  A frame that carries a follow-up's id is decoded as a push, never as an answer to a membership
  command. `get_snapshot` is also public: `PredictionsMembership::snapshot(tickers)` asks for
  snapshots of tickers already in the membership (a ticker outside it is refused as
  `InvalidRequest`, nothing sent), answered `Ok` once the command is accepted; the snapshots arrive
  on the stream. It needs the `OrderbookDelta` channel and is refused as `InvalidRequest` without
  it. It goes through the membership handle as `Change::Snapshot(Vec<Ticker>)`; one made during an
  outage is answered `Ok` at once and not recorded, since the replay brings a fresh snapshot of
  every ticker. It never changes the membership.
- **D10. The local book.** `predictions::ws::book::Books` is a value the consumer feeds events
  into: `Books::apply(&mut self, event: &Event) -> Option<&OrderBook>` returns the book an event
  changed.
  - `OrderBook` keeps the ticker's ladder in YES pricing, as the socket sends it
    (`use_yes_price: true`): `bids` and `asks` as price → size maps, with `best_bid()`,
    `best_ask()`, `bids()` highest first and `asks()` lowest first, each a `Level`, and
    `levels(outcome: Outcome)` giving one outcome's view (a NO bid at *q* is a YES ask at 1 − *q*,
    as Epic 8's `venue.rs` reads it).
  - A snapshot replaces the ticker's book. A delta adds its signed size to the level, and a level
    whose size reaches zero or below is removed. A delta for a ticker with no snapshot is ignored.
  - `Resyncing { tickers }` drops those tickers' books, and `Reconnected` drops every book, so a
    book is only ever built from a snapshot the current subscription sent.
  - It does no I/O and has no clock; its unit tests replay fixture frames.
- **D11. Limits.** Story 7 runs two measurements and sizes from them:
  - **The unsigned soak**, on production, with a new `polyoxide-kalshi/examples/read_soak.rs` on
    test-support's `soak` helpers (`Pacer`, `parse_stages`, `observe`, `verdict::strict`), over
    distinct URLs (`GET /markets/{ticker}` across distinct open tickers from one listing walk, and
    `GET /events/{event_ticker}`), each path probed once on the host first. The measured clean rate,
    with a tenth reserved (`RESERVED_FRACTION`), replaces the unsigned client's provisional Basic
    sizing in `throttle.rs`, and `OBSERVED.md`'s `## Rate limits` records the runs. If no rate below
    Basic's 200/s refuses, the sizing stays Basic's and is marked confirmed.
  - **The signed-bucket probe**, on demo, with `KALSHI_DEMO_API_KEY_ID` and
    `KALSHI_DEMO_PRIVATE_KEY`: drain the key's Read bucket past its capacity on a public route over
    distinct URLs and see whether the 429 follows the key's bucket. If it does, a keyed client's
    public reads stay on the key's bucket (today's code); if not, they move to the unsigned bucket,
    and the change is tested on the mock suites. Without the two variables the story stops and asks
    the owner.
  - Both close their `deferred-work.md` entries.
- **D12. Publishing the crate.** `polyoxide-kalshi`'s manifest drops `publish = false`, its
  description and READMEs drop "walking skeleton" and "not published", and its
  `[workspace.dependencies]` entry gains `version = "0.41.1", default-features = false`, as
  `polyoxide-binance`'s has (the release commit raises every pin to `0.42.0`, §12). Its `polyoxide-test-support` dev-dependency stays path-only.
  `[package.metadata.polyoxide] readme` loses "a walking skeleton, not published". It must pass
  `publish_order.py check-manifests` and the publish dry run.
- **D13. The umbrella.**
  - Features: `kalshi = ["kalshi-predictions"]`;
    `kalshi-predictions = ["dep:polyoxide-kalshi", "polyoxide-kalshi/predictions", "polyoxide-venue/market"]`;
    `kalshi-predictions-ws = ["kalshi-predictions", "polyoxide-kalshi/predictions-ws"]`. `full`
    gains all three. The default stays `polymarket`. `polyoxide-kalshi` is an optional dependency
    without default features.
  - `pub use polyoxide_kalshi as kalshi;` under any `kalshi*` feature.
  - `parse_key` handles `kalshi.predictions` under `kalshi-predictions`, through the crate's
    `predictions::key` parser, and its docs list it.
  - The prelude: under `kalshi-predictions`, `Predictions`, `PredictionsBuilder`,
    `PredictionsError` and `Credentials as KalshiCredentials` (a bare `Credentials` already names
    the CLOB's); under `kalshi-predictions-ws`, `Event as PredictionsEvent`, `PredictionsWsBuilder`,
    `SupervisedPredictionsWs`, `PredictionsWsError` and `Books as PredictionsBooks`. Exact names
    follow the crate's public items; a name the crate lacks is a question for the owner.
  - One `const _` per passthrough and per prelude block, as the others have, and the
    `DynMarketData` and `parse_key` pins gain `kalshi-predictions`.
  - `examples/venue_neutral.rs` gains a Kalshi arm in `main`, with no `cfg`: the first open market
    of one `status=open&mve_filter=exclude` page, keyed `yes`, named by fully qualified
    `polyoxide::kalshi::predictions::…` paths inside `main`. No `cfg(feature = "kalshi-…")` goes in
    the example, because `polyoxide-kalshi/tests/venue_predictions.rs` includes the file by
    `#[path]`, where a feature `polyoxide-kalshi` does not declare fails `unexpected_cfgs` under
    `-D warnings`. So `polyoxide-kalshi`'s `polyoxide` dev-dependency gains `kalshi-predictions`,
    a path-only dev-dependency cycle cargo allows and publishing strips; if cargo refuses it, the
    story stops and asks. The example's,
    `venue_neutral`'s, `live_venue`'s and `parse_key`'s `required-features` gain
    `kalshi-predictions`, and `tests/venue_neutral.rs` gains a Kalshi mock case, `tests/parse_key.rs`
    a Kalshi case, `tests/live_venue.rs` a Kalshi case. Kalshi's own `tests/venue_predictions.rs`
    keeps including the example by `#[path]`; the generic half moving to its own file (A8-8) is not
    in this epic.
  - The crate docs, `polyoxide/README.md` and the docs.rs feature list name Kalshi.
- **D14. CI.**
  - `ci/dep-allowlists/polyoxide@kalshi-predictions.txt` and `@kalshi-predictions-ws.txt`, written
    with `test_dep_allowlists.py --write`. The umbrella's rule in `PER_FEATURE` adds
    `kalshi-predictions` to the HTTP modules and adds the socket rule for `kalshi-predictions-ws`;
    `alloy*`, `hmac` and `keyring` stay refused for both. `MUST_BUILD` gains
    `polyoxide@kalshi-predictions.txt: reqwest`.
  - The full-pin rule needs no edit: it already requires `full` to turn on every module of every
    published venue crate, so the umbrella's features satisfy it. Its test's `unpublished` set loses
    Kalshi where a test names the real workspace, and its synthetic cases stay.
  - cargo-hack's `--ignore-private` stops skipping Kalshi once it is published, which closes A8-10
    with no `ci.yml` edit; Story 8 confirms `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`
    lists `polyoxide-kalshi`.
  - The `features` job's `--features polyoxide/full` pin for Kalshi now turns on an umbrella feature;
    the derived step needs no edit, and Story 8 runs it.
  - `scripts/publish_order.py` places Kalshi after `polyoxide-ws` and before the umbrella, by its
    dependencies; nothing is written by hand. CLAUDE.md's "Today's order" line is updated.
- **D15. Docs and records** (Story 9).
  - CLAUDE.md: the hand-written passages the Epic 8 doc-debt entry lists (Builder pattern and its
    default-concurrency list, API namespaces, Error hierarchy, Classify's socket-error list, Market
    keys, Market-data traits, Environment Variables), the umbrella paragraph and its feature list,
    "Today's order", the CI paragraph's description of Kalshi's pins as outside the umbrella, and
    every "walking skeleton, not published" phrase. `Candles::intervals()` and `UnservedInterval`
    join "Market-data traits". The socket paragraph gains the channels, the book and the gap
    re-snapshot.
  - `README.md`'s and `docs/specs/INDEX.md`'s hand-written lead-ins, `SELF-HEALING.md`'s intro and
    secrets sentence, and `.env.example` (`KALSHI_DEMO_API_KEY_ID`, `KALSHI_DEMO_PRIVATE_KEY`).
  - `.github/scripts/tests/test_classify_coverage.py`'s vacuity guards gain a Kalshi type.
  - `docs/s3-migration.md` gains the release's consumer-impact notes: the new crate and umbrella
    features, `Candles::intervals()`, `UnservedInterval`, and `Protocol::follow_up`, none breaking.
  - `docs/specs/kalshi/OBSERVED.md` holds every probe, capture and soak finding.
  - `spine-amendments/epic-9.md`, in `epic-8.md`'s format: A9-1, the spine's S3 row says "the
    Kalshi skeleton (`publish = false`)" and the crate is now published and in the umbrella; A9-2,
    `Candles::intervals()` and the refusal resolve A8-3; A9-3, `Protocol::follow_up` lets a protocol
    send from `decode`; A9-4, an empty subscription means "every market" on some channels, so a
    venue's empty membership must not subscribe; plus any gap the run meets. Merging them into the
    spine and regenerating `docs/ARCHITECTURE.md` is an owner session.
  - `deferred-work.md`: close the entries this epic settles (the anonymous soak, the signed-bucket
    probe, the local book and channels, A8-10, A8-3, and the doc debt), each with the commit that
    settled it. The 2026-10-22 schema corrections, the trading probes and the mapping's move stay.
- **D16. The release** is not a plan story: a version bump is its own commit on `main` (the
  profile's landmine rule), made after the branch lands. One commit, `chore(release): v0.42.0`:
  the workspace version, the inter-crate pins, `Cargo.lock`, and CHANGELOG.md's `0.42.0` section
  quoting `docs/s3-migration.md`'s notes for this release (§12).

## 3. Story 1: probes and captures

No Rust. Each finding goes in `OBSERVED.md` under a `## Epic 9` section, with the date and host.

- **Candlesticks**, production, unsigned: `GET /trade-api/v2/markets/candlesticks` for one open
  ticker at each of 1, 60 and 1440 (status, body shape against `BatchGetMarketCandlesticksResponse`,
  which fields are null in a period with no trade, whether `price.previous_dollars` is set there),
  and the same for `/series/{s}/markets/{t}/candlesticks`. Then a width outside the enum (5) on
  each, and a span over 10,000 rows on the batch route (truncated, refused, or `adjusted_end_ts`).
  §1.6 settled the route and the field shapes; the captures make the fixtures.
  Fixtures under `polyoxide-kalshi/tests/fixtures/predictions/` with `PROVENANCE.md` rows, written
  by `scripts/capture_kalshi_predictions.py`.
- **Socket**, demo, signed (`KALSHI_DEMO_*`), with `scripts/capture_kalshi_predictions_ws.py`:
  - one `subscribe` with `channels: ["orderbook_delta","ticker","trade"]` on two active tickers:
    capture each `subscribed`, `ticker` and `trade` frame;
  - an `update_subscription` naming two `sids` at once (accepted, refused, or applied to one);
  - `get_snapshot` on the order-book `sid`: the answer's `id`, `sid` and `seq`, and whether later
    deltas continue from it;
  - `get_snapshot` naming a ticker outside the subscription, and one on the `ticker` channel's
    `sid`: the error codes;
  - a `ticker`/`trade` subscribe with no `market_tickers`, to confirm it means every market (stop
    after a few frames).
  Fixtures under `tests/fixtures/predictions/ws/`.
- Without `KALSHI_DEMO_*` in the environment the socket half stops and asks the owner.
- Commit the scripts, fixtures and `OBSERVED.md`. The story records findings and changes no
  decision. A finding that contradicts D6 or D9 (a field shape, the snapshot answer's `seq`) goes
  to the owner before Story 3 or 6 runs.

## 4. Story 2: `Candles::intervals()` (D2, D3)

- `polyoxide-venue/src/market.rs`: the provided method and the trait docs' refusal sentence.
- `records.rs` (or the file `CandleQuery` lives in): `CandleQuery::ensure_served`.
- `UnservedInterval` beside the other `market` errors, in the `Classify` assertion.
- `DynCandles` must forward `intervals()`; a test calls it through `Arc<DynCandles<'static>>`. If
  dynosaur 0.3.1 cannot forward a provided sync method, the story stops and asks.
- Tests (`polyoxide-venue/tests/`): the default is `ALL`; an override is seen through the dyn
  wrapper; `ensure_served` refuses an unserved width and accepts a served one; the error's class,
  fault and message; and `ws_classification`-style coverage is unaffected.
- No other venue changes.

## 5. Story 3: Kalshi's `Candles` (D4 to D6)

- Routes, types, costs and query setters for both candle routes (D5); the types follow the mirror's
  schemas and the Story 1 captures, with `spec_agreement_predictions.rs` and
  `wire_agreement_predictions.rs` cases for each, and `query_setters_predictions.rs` entries.
- `mock_predictions_routes.rs` cases for both routes.
- `venue.rs`: `impl Candles for Predictions` and `CandleExt`/`Ohlc`, with the module docs naming the
  widths, the span rule and the historical cutoff.
- `tests/venue_predictions.rs`: each mapping rule of D6 on the captured fixtures (a traded period, an
  untraded period carried by `previous`, an untraded period with no `previous` skipped, a `no` key
  mirrored with high and low swapped, open-time filtering, an unserved width refused with nothing
  sent: a mock with `expect(0)` and `match_query(Any)`).
- `tests/live_predictions_venue.rs`: candles at each served width for one open market, through the
  trait, with `ClassifiedResultExt::or_fail`.
- `gen_registry.py --write` if metadata changes.

## 6. Story 4: `Protocol::follow_up` (D7)

- `polyoxide-ws/src/supervisor/mod.rs`: the provided method and its docs; `task.rs`: the call after
  `decode` and the bounded send.
- `src/supervisor/tests.rs`: D7's three pins, appended. No venue changes, and the venue gate passes
  (the docs use no venue's words).

## 7. Story 5: the socket protocol (D8, D9)

- `ws/builder.rs`: `channels(..)`; `ws/protocol.rs`: per-channel `sid`s, the replay, the
  membership commands, the empty-set rule for every channel, the gap path through `follow_up`, the
  follow-up id table, re-anchoring, delta suppression per ticker, `SnapshotTimeout`,
  `Change::Snapshot` and `PredictionsMembership::snapshot`; `ws/event.rs`: the two events, their
  payloads and `Event::Resyncing`; `ws/error.rs`: `SnapshotTimeout` (the socket table is
  untouched); `ws/mod.rs` re-exports.
- Offline tests on the scripted server (`supervision_predictions.rs`, appended): the replay names
  every chosen channel; each `subscribed` maps to its channel; a change updates every `sid` with one
  command per `sid`; emptying the set unsubscribes every `sid` and sends no
  `subscribe` on reconnect; the first add after empty subscribes afresh; a `trade` gap reconnects;
  ticker frames carry no `seq` and pass.
- `wire_agreement_predictions_ws.rs`: the two payloads against Story 1's frames;
  `spec_agreement` coverage of `tickerPayload` and `tradePayload`.
- Offline tests on the scripted server, for the gap: a gap yields `Resyncing`, sends one `get_snapshot` naming
  the membership, and the stream continues from the snapshots with no `Disconnected`; deltas between
  the gap and a ticker's snapshot are dropped; a missing snapshot reconnects after `stale_after`; an
  error frame answering the follow-up reconnects; a second gap during a resync reconnects;
  `snapshot()` outside the membership and without the order-book channel is refused with nothing
  sent; a `snapshot()` during an outage is answered `Ok` and sends nothing.
- `tests/live_predictions_ws.rs`: a demo case that subscribes all three channels and waits for a
  ticker frame (`environmental` if the market is quiet), and one where `snapshot()` on a subscribed
  ticker yields a snapshot.

## 8. Story 6: the local book (D10)

- `ws/book.rs` with `Books` and `OrderBook`, and `ws/mod.rs`'s module block and re-exports (the
  block the names contract gives, replaced whole), with unit tests: a snapshot builds both sides; a
  delta adds and a zero removes; a delta before a snapshot is ignored; `Resyncing` and
  `Reconnected` drop books; `levels(No)` mirrors; replaying the captured snapshot and deltas gives
  the ladder the frames imply, asserted by literal prices and sizes.

## 9. Story 7: limits (D11)

- `polyoxide-kalshi/examples/read_soak.rs`, its stages and routes on the command line as
  `closed_positions_soak` and `info_soak` take them; `[dev-dependencies]` gains test-support's
  `soak` feature if not already on.
- Run it on production (stages up to and past Basic's 200/s) and record each run in `OBSERVED.md`.
- Run the signed probe on demo and record it.
- Change `throttle.rs`'s unsigned sizing (or confirm it) and, if D11's second result says so, a
  keyed client's public-read bucket, with the tests that pin each, appended.

## 10. Story 8: publish wiring (D12 to D14)

- The manifest and workspace pin; the umbrella's features, re-export, `parse_key`, prelude, pins,
  example and tests; the allowlists and `test_dep_allowlists.py`; `test_ci_workflow.py`'s real-tree
  exemption; `gen_registry.py --write`.
- Run: `publish_order.py check-manifests`, `publish_order.py list` (Kalshi in order),
  `cargo publish --workspace --dry-run --no-verify --locked`, the features job's cargo-hack line and
  each header pin under `polyoxide/full`, and the umbrella's tests with `--features full`.

## 11. Story 9: docs and records (D15)

As D15 lists. Generated regions only by `gen_registry.py --write`. `docs/ARCHITECTURE.md` is not
edited.

## 12. After the run: the release (D16)

An owner session, after the run, not a story.

- Before picking the version, check origin's `main` and tags (`git fetch`, `git ls-remote --tags`):
  if `v0.42.0` exists or `main` has moved past `0.41.1`, stop and ask.
- The branch lands on `main` as a main-first merge, so the release commit's first parent carries
  the old version (`publish_order.py decide` compares with the first parent).
- The owner pushes, and confirms the crates.io token may publish a new crate name.

## 13. Gates every story keeps

- The flow profile's story gates under `nix develop --command`: fmt, clippy `-D warnings`, tests,
  rustdoc `-D warnings`, `gen_registry.py --check`, `venue_gate.py`, `live_unwraps.py`,
  `test_body_diff.py --baseline v0.40.0`.
- The touch-set gate, as Epic 8's, against
  `_bmad-output/specs/spec-venue-extensibility/epic-9-touch-set.txt`, which Story 1 writes, matching:
  the root `Cargo.toml` and `Cargo.lock`; `polyoxide-kalshi/`; `polyoxide-venue/src/`,
  `polyoxide-venue/tests/`; `polyoxide-ws/src/supervisor/`; `polyoxide/`; `docs/specs/kalshi/`;
  `scripts/capture_kalshi_*.py`; `ci/dep-allowlists/polyoxide-kalshi*.txt` and
  `ci/dep-allowlists/polyoxide@kalshi*.txt`; `.github/scripts/tests/test_dep_allowlists.py`,
  `test_ci_workflow.py` and `test_classify_coverage.py`; the files `gen_registry.py` writes;
  `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `.env.example`, `docs/specs/INDEX.md`,
  `docs/s3-migration.md`;
  this spec, the plan, `_bmad-output/specs/`, `spine-amendments/epic-9.md` and
  `_bmad-output/implementation-artifacts/deferred-work.md`.
- The final gates add `api_removals.py check --baseline v0.40.0`, the package job's three steps,
  and the features job's steps.
- The foundation crates name no venue (D2, D3, D7 use no venue's words).
- Every new error type is `#[non_exhaustive]` where it is an enum, implements `Classify`, and is in
  its crate's assertion.
- Add tests; never change an existing Rust test's body (AD-12). A test that must change goes back to
  the owner.
- Mirrors stay byte for byte; live tests follow test-support's loaders and tags, with no
  `.unwrap()`; no `connect_async` outside the kit, no `.gzip(`, no `allow_burst(`.
- Commits by pathspec, conventional, naming the story, never `--no-verify`.

## 14. Not in this epic

- Kalshi's `Trading`, its probes, and the trading mapping's move.
- The CLI, the Python bindings, and keychain storage of Kalshi credentials.
- The `/historical` candle route, `Instruments`, the user channels.
- The 2026-10-22 schema corrections (not yet published on 2026-10-19).
- Moving the venue-neutral example's generic half to its own file (A8-8).
- Merging the A8 and A9 amendments into the spine and regenerating `docs/ARCHITECTURE.md`.

## 15. Owner actions

- Put `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` in the environment of the sessions
  that run Stories 1, 5, 6 and 7. Rotate the two demo keys exposed in Epic 8's transcripts first.
- Land the branch on `main` (main-first merge), make the release commit (§12), confirm the
  crates.io token can publish a new crate name, and push `main`.
