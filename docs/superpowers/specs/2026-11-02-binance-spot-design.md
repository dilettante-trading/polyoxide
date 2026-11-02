# Binance spot: public market data over HTTP and WebSocket

**Date:** 2026-11-02
**Branch:** `aidanb/binance-spot`
**Slice:** 2 of `polyoxide-binance`: Binance spot's public market data and market streams, the
counterpart of slice 1's `usdm` (`docs/superpowers/specs/2026-10-07-polyoxide-binance-design.md`,
whose decisions stand unless this spec says otherwise). Signed routes, the user-data stream, the
WebSocket API, a maintained local book from diff depth, and Python bindings are later specs.
**Status:** approved by the owner in brainstorming, 2026-11-02. Read it with
`.claude/rules/binance.md` and CLAUDE.md. Where this spec and an earlier record disagree, this
spec binds.

## Goal

Give `polyoxide-binance` a `spot` module that reads Binance spot public market data on
`api.binance.com` and streams its market streams on `stream.binance.com`, with no credentials,
`Decimal` money, typed errors, a measured weight budget and a supervised socket that enforces
spot's connection rules, so a consumer treats spot the way it treats `usdm`. Spot is the first
product of a new class, so `polyoxide-venue` gains `ProductClass::Spot` and the `SpotMarket`
bundle.

## Scope

In:

- A crate-private `shared` module holding the Binance-wide plumbing `usdm` has today, with
  `usdm`'s public API unchanged.
- `ProductClass::Spot` and `SpotMarket` in `polyoxide-venue`.
- Fourteen REST methods over eleven routes (table below), a spot `WeightBudget` and `SpotError`.
- `spot::venue`: `MarketData`, `Instruments`, `Trades`, `Candles`.
- Nine market streams (table below), a bare socket and a supervised one, `SpotWsError`.
- The umbrella's `binance-spot` and `binance-spot-ws` features, `polyoxide binance spot stream`,
  the docs, probes, fixtures and the nightly jobs.

Out, each a later spec: `historicalTrades` (it needs an API-key header), `uiKlines`, the
rolling-window `ticker`, `ticker/tradingDay` and the `symbols=[…]` list forms; the diff-depth
stream and a maintained local book; the rolling-window `ticker_<w>` streams, `!ticker@arr` and
kline timezone offsets; `data-api.binance.vision` as a preset; a watcher of
`binance-spot-api-docs`'s CHANGELOG; signed routes, the user-data stream, the WebSocket API and
SBE; Python bindings; REST data commands in the CLI.

## Decisions taken during brainstorming

| Question | Decision | Why |
|---|---|---|
| Slice | Public market data, REST and streams, matching `usdm` | The precedent's shape, no credentials |
| Default host | `https://api.binance.com`, settable on the builder | The host Binance's pages and SDKs name, as `usdm` uses `fapi`; a 451 on a US runner is already environmental |
| Shared plumbing | Extract a crate-private `shared` first, then build spot on it | The known differences are parameters (limits, layers, paths, pacing), not mechanisms; one copy of every Binance-wide fix |
| Spec docs | As `usdm`: `docs/specs/binance/` is not a mirror; the live suites are the drift detectors | `binance-api-swagger/spot_api.yaml` was last released 2024-10-08 (v1.22.0), lacks fields the wire sends today (`amendAllowed`, `opoAllowed`, `pegInstructionsAllowed`), and is 262 `/sapi` paths to 35 `/api/v3`; a nightly diff of it would never fire |
| Product class | `ProductClass::Spot` with a `SpotMarket: MarketData` bundle | Every class has one bound; a bundle names only what defines its class (ADR 0001), and nothing beyond `MarketData` defines spot |
| `book` depth | The default `depth`, 100 levels, weight 5 | 1000 levels costs 50; the native client offers up to 5000 |
| Diff depth | Out | Keeping a local book needs a REST snapshot sync, its own slice |

## Venue contract

Measured against `api.binance.com` on 2026-11-02 unless marked documented.

### HTTP

- `exchangeInfo`'s `rateLimits`: `REQUEST_WEIGHT` 6000 per minute, `RAW_REQUESTS` 300,000 per
  5 minutes, and two `ORDERS` limits that public routes never touch.
- Every response carries `x-mbx-used-weight` and `x-mbx-used-weight-1m`.
- `api.binance.com` and `data-api.binance.vision` appear to share one counter per IP: one call
  read 20 on the first and the next 40 on the second. One observation; the planning probe
  confirms or refutes it, and `OBSERVED.md` records the result.

| Route | Method | Weight (measured) |
|---|---|---|
| `/api/v3/ping` | `spot.health().ping()` | 1 |
| `/api/v3/time` | `spot.health().time()` | 1 |
| `/api/v3/exchangeInfo` | `spot.exchange().exchange_info()`, with `.symbol(&s)` or without | 20 either way |
| `/api/v3/depth` | `spot.market().depth(&s)` | 5 without `limit` and at 5 to 100; 25 at 101 to 500; 50 at 501 to 1000; 250 at 1001 to 5000 (documented) |
| `/api/v3/trades` | `trades(&s)` | 25, with or without `limit` |
| `/api/v3/aggTrades` | `agg_trades(&s)` | 4, with or without `limit` |
| `/api/v3/klines` | `klines(&s, interval)` | 2, with or without `limit` |
| `/api/v3/avgPrice` | `avg_price(&s)` | 2 |
| `/api/v3/ticker/24hr` | `ticker_24h(&s)` / `tickers_24h()` | 2 / 80 |
| `/api/v3/ticker/price` | `price(&s)` / `prices()` | 2 / 4 |
| `/api/v3/ticker/bookTicker` | `book_ticker(&s)` / `book_tickers()` | 2 / 4 |

Neither `depth` nor `ticker/bookTicker` returns a time.

### WebSocket

Documented, and confirmed by a `probe_ws.py` spot run into `OBSERVED.md` before the protocol
story is planned:

- Combined streams on `wss://stream.binance.com:9443/stream`, one path; frames are
  `{"stream": …, "data": …}`.
- At most 5 incoming messages per second per connection, pings, pongs and JSON requests
  counted; more disconnects, and repeated excess bans the IP.
- At most 1024 streams per connection; at most 300 connections per 5 minutes per IP.
- The server pings every 20 s and disconnects a connection that has not ponged within a
  minute; a connection lives at most 24 hours.
- A partial-depth frame (`lastUpdateId`, `bids`, `asks`) and a `bookTicker` frame carry no
  event type, and partial depth carries no symbol: both take their stream from the combined
  envelope.

## Design

### Crate layout

```
polyoxide-binance/src/
  shared/            pub(crate); #[cfg(any(feature = "usdm", feature = "spot"))]
    mod.rs
    weight.rs        the minute engine
    policy.rs        BinanceRetryPolicy
    error.rs         the venue body, Retry-After, clipping, the status → Class map
    request.rs       Routed<T>, route_builder!
    positional.rs    the kline and level visitors
    symbol.rs        Symbol, InvalidSymbol
    ws/              #[cfg(any(feature = "usdm-ws", feature = "spot-ws"))]
  usdm/              as today; public paths unchanged
  spot/              mod.rs (Spot, SpotBuilder), api/, error.rs, key.rs, types.rs, venue.rs,
                     weight.rs, README.md, ws/
```

`cargo hack --each-feature` builds `spot` without `usdm` and `usdm` without `spot`, so every
`shared` item compiles under either, and `shared::ws` under either socket feature.

### `shared`: what moves and what stays

Moves, parameterised where the products differ:

- **The minute engine** from `usdm/weight.rs`: the UTC clock minute at the published limit less
  the reserved tenth, raised by `X-MBX-USED-WEIGHT-1M` only for the minute its request was
  charged in, and the client-wide `Hold` for a `429` or `418`, only ever extended, with the
  3-day ceiling. A product supplies its published limit and its `LayerId`.
- **The retry policy**, `UsdmRetryPolicy` renamed `BinanceRetryPolicy`, unchanged.
- **The error decode**: the `{code, msg}` body, `retry_after_secs`, `clip`, and the status
  rules (`418`, `451` and the firewall `403` are `Restricted`; a `429` is `RateLimited`). Each
  product keeps its own public `<Module>Error` and its `From<ApiError>`, which call these.
- **`Routed<T>` and `route_builder!`.**
- **The positional visitors** for klines and book levels.
- **`Symbol` and `InvalidSymbol`.** Spot accepts the `_` that only `usdm` quarterlies use; the
  venue answers an unknown symbol, as it does any other.
- **`shared::ws`**: the `Protocol` (wall-clock pings while the queue is full, staleness counting
  every frame, reconnect with a paced replay bounded by `stale_after`, rotation at 23 h 50 min,
  a change recorded before it is sent), the router and its forwarding queue of 1024, the
  `StopCause` machinery and the client-side limits. It is generic over a product's stream set,
  which supplies: the stream name type and its wire spelling, each name's path, the paths, the
  frame decode, the pacing interval and the cap on names per request. The plan fixes the
  trait's signatures.

Stays in each product: route tables and `Route::cost`, response types and `Filter` enums,
`Interval`, `StreamName`, `Event` and `Payload`, `<Module>Error`, `<Module>WsError` and
`InvalidStreamName`, the key space, the venue impls, `DEFAULT_BASE_URL`.

**`usdm`'s public API does not change.** `usdm::types::Symbol` and `InvalidSymbol` are `pub use`
of the shared items; `WeightBudget`, `Cost`, `Route`, `WEIGHT_LAYER`, `FUNDING_LAYER`,
`USED_WEIGHT_HEADER`, `Interval`, `StreamName`, `StreamPath` and the error enums keep their paths,
names and variants (prader-rs pins `Interval`, `StreamPath` and `Cost` exhaustive).
`usdm::weight::WeightBudget` wraps the shared engine and keeps its funding layer.
`python3 scripts/api_removals.py check --baseline v0.40.0` must report nothing.

**Moved tests.** The unit tests inside the moved code move with it and keep their bodies
(AD-12); a test whose subject is renamed (`UsdmRetryPolicy` to `BinanceRetryPolicy`) takes a
`[[drift]]` entry. Their `docs/MUTANTS.md` rows move to the new lines and are re-proved. The
integration suites (`mock_usdm`, `supervision_usdm*`, `wire_agreement_usdm*`, `venue_usdm` and
the rest) do not move and are the refactor's regression guard.

### `polyoxide-venue`

- `ProductClass::Spot`, `as_str` `"spot"`: buying and selling the asset itself; a price is
  unbounded, a position is a balance of the base asset and so never negative, and there is no
  funding. The enum is already `#[non_exhaustive]`.
- `pub trait SpotMarket: MarketData {}` with `impl<T: MarketData> SpotMarket for T {}`, under
  feature `market`, documented as the other two bundles are.
- `gen_registry.py` accepts the class `"spot"`.

### Client

`spot::Spot::new()` and `Spot::builder()` on a `ClientConfig`, with `client_config_setters!`,
`SpotBuilder::weight_budget` (one budget shared by every `Spot` built with it, as `usdm`'s is),
gzip on, `DEFAULT_BASE_URL = "https://api.binance.com"` and `DEFAULT_MAX_CONCURRENT = 4`.
Namespaces `health()`, `exchange()`, `market()` from `namespaces!`; query setters from
`query_setters!`, except `GetDepth::limit`, hand-written because it re-prices the route
(`clients.md`).

### Weight budget

`spot::weight::WeightBudget` on the shared engine at 5400 of the published 6000 per minute,
`WEIGHT_LAYER = LayerId("binance-spot-weight")`, no funding layer. `spot::weight::Cost` and
`Route` are spot's own. `RAW_REQUESTS` is not modelled: every request weighs at least 1, so the
weight budget caps a client at 27,000 requests per 5 minutes, under the 300,000 limit; the
module docs say so. `Route::cost` holds the measured table above, pinned by a
`documented_weights` test, and `examples/weight_probe.rs` gains the spot routes.

### Types

Spot's own: `ServerTime`, `ExchangeInfo`, `RateLimit`, `SymbolInfo` (with `permissionSets`, the
`*Allowed` flags and the precisions), an `open_enum!` `Filter` (`PRICE_FILTER`,
`PERCENT_PRICE_BY_SIDE`, `LOT_SIZE`, `MARKET_LOT_SIZE`, `NOTIONAL`, `MIN_NOTIONAL`,
`ICEBERG_PARTS`, `TRAILING_DELTA`, `MAX_NUM_ORDERS`, `MAX_NUM_ALGO_ORDERS`,
`MAX_NUM_ICEBERG_ORDERS`, `MAX_POSITION` and whatever else the captured `exchangeInfo` carries),
`Ticker24h`, `PriceTicker`, `BookTicker`, `AvgPrice`, `Trade`, `AggTrade`, `Kline`, `Depth`
(`last_update_id`, `bids`, `asks`) and `Level`. Money is `Decimal`; response rows carry symbols
as `String`. `spot::Interval` is exhaustive, as `usdm`'s is, with `1s` through `1M`. A decimal
the wire sends as `""` is an `Option`, as `FundingRate::mark_price` is; the captures say which.

### Errors

`SpotError`, `#[non_exhaustive]`, with `usdm`'s variants and decode through `shared::error`, one
`From<ApiError>`, and `Classify`. A `418` logs its body at WARN under
`polyoxide_binance::spot::error`. Listed in `lib.rs`'s `const _` assertion.

### `spot::venue` and keys

`SpotKeys` (`spot/key.rs`) makes `binance.spot:<SYMBOL>` keys with `CLASS = ProductClass::Spot`,
canonicalising through `Symbol`. `Spot` implements:

- `MarketData`: `instrument` from `exchangeInfo?symbol=` (20); `quote` from `ticker/bookTicker`
  (2), `time` `None`; `book` from the default `depth`, 100 levels (5).
- `Instruments`: `exchangeInfo` (20); a row that does not convert is skipped with a WARN naming
  the symbol and the error, and the live suite fails when one is skipped.
- `Trades`: `aggTrades` with `limit=1000` (4).
- `Candles`: `klines` with `limit=1000` (2).

With `InstrumentExt`, `QuoteExt`, `BookExt` (`last_update_id`), `TradeExt` and `CandleExt`.
`lib.rs` pins `spot::Spot` to `SpotMarket`.

### Streams (`spot-ws`)

`SpotWs` (one connection) and `SpotWsBuilder`/`SupervisedSpotWs`, on `shared::ws` with one path.
`DEFAULT_WS_URL = "wss://stream.binance.com:9443/stream"`, settable.

| `StreamName` | Wire name | `Payload` |
|---|---|---|
| `AggTrade(s)` | `<s>@aggTrade` | `AggTrade(AggTradeEvent)` |
| `Trade(s)` | `<s>@trade` | `Trade(TradeEvent)` |
| `Kline(s, interval)` | `<s>@kline_<i>` | `Kline(KlineEvent)` |
| `MiniTicker(s)` | `<s>@miniTicker` | `MiniTicker(MiniTickerEvent)` |
| `AllMiniTickers` | `!miniTicker@arr` | `MiniTickers(Vec<MiniTickerEvent>)` |
| `Ticker(s)` | `<s>@ticker` | `Ticker(TickerEvent)` |
| `BookTicker(s)` | `<s>@bookTicker` | `BookTicker(BookTickerEvent)` |
| `PartialDepth(s, levels, speed)` | `<s>@depth<5\|10\|20>`, bare (1000 ms) or `@100ms` | `PartialDepth(PartialDepthEvent)` |
| `AvgPrice(s)` | `<s>@avgPrice` | `AvgPrice(AvgPriceEvent)` |

An unrecognised frame is `Payload::Unknown`. Spot has one path, so `spot::ws::StreamPath` has
one variant, and `Event::Disconnected` and `Reconnected` keep their `{ path }` field: the shared
code and a consumer folding outages treat both products alike. `StreamName`
is the only way to name a stream; `PartialDepthEvent` and `BookTickerEvent` carry the symbol
parsed from the envelope's `stream`.

The client enforces spot's rules first: at most 1024 streams per connection, requests paced at
one per 250 ms (4 a second), leaving room under the 5-a-second limit for the automatic pong; the
client's wall-clock ping counts against the same limit, so its interval, and the pace if pongs
and pings leave too little room, are set from the probe. The cap on names per request is set from the probe (`usdm`'s is 200). Every
`Event::Disconnected { path }` is followed by `Event::Reconnected { path }` while the client
runs, from the shared code. A path that stops on an error ends the stream, and `close()` and
later membership calls answer `SpotWsError::Stopped { cause }`, as `usdm`'s do.

The offline tests drive the kit's scripted server with spot's answer
(`spot/ws/scripted.rs`, included by the integration suites with `#[path]`). `test-server` with
`spot-ws` exposes `spot::ws::fixtures`.

### Features and registration

```toml
default = ["usdm", "spot"]
spot = [ …the same optional deps as usdm… ]
spot-ws = ["spot", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]
```

- `[package.metadata.polyoxide]`: `products = ["usdm", "spot"]`,
  `classes = { usdm = "perpetual-future", spot = "spot" }`, the `readme` line naming `spot`,
  and spot's identifiers for the venue gate (`/api/v3` and a stream-host term), each checked
  absent from the foundation crates first.
- Live entries `live_spot` (`spot`) and `live_spot_ws` (`spot-ws`), suite `spot`, 15 minutes,
  no secrets: a new nightly job `live-polyoxide-binance-spot`.
- The crate's description and keywords name spot; docs.rs builds `spot` and `spot-ws`.
- `ci/dep-allowlists/` regenerated for the new feature sets.

### Umbrella

`binance-spot = ["dep:polyoxide-binance", "polyoxide-binance/spot", "polyoxide-venue/market"]`,
`binance-spot-ws = ["binance-spot", "polyoxide-binance/spot-ws"]`,
`binance = ["binance-usdm", "binance-spot"]`, and `full` gains `binance-spot-ws`. `parse_key`
knows `binance.spot`; the prelude exports `SpotMarket` beside the other bundles; the umbrella's
class test covers spot under `full`. `live_venue`'s features gain `binance-spot`.

### CLI

`polyoxide binance spot stream`, the counterpart of `binance usdm stream`: the supervised tier,
a `commands::stream::Feed`, the same flags, and `--format json` printing each frame's envelope.
`polyoxide-cli` builds `polyoxide-binance` with `usdm-ws` and `spot-ws`. `tests/ws_binance.rs`
gains spot cases on `spot::ws::fixtures`.

### Docs

- `docs/specs/binance/INDEX.md`: a spot section with the routes and streams tables.
- `docs/specs/binance/OBSERVED.md`: a spot section with the measured weights, the shared-counter
  result, the stream limits and acknowledgements, and the frame shapes.
- `docs/specs/binance/probes/`: spot modes for `probe_rest.py` and `probe_ws.py`; the capture
  script gains spot (a `--product` flag or `scripts/capture_binance_spot.py`), with fixtures
  under `tests/fixtures/spot/` and a `PROVENANCE.md`.
- The `nightly-schema` exclusion for `docs/specs/binance/` says spot's swagger is stale since
  2024-10-08 and lacks live fields; `gen_registry.py --write` regenerates every region.
- `.claude/rules/binance.md`: `shared` and `spot` sections, and its `paths:` gain the spot
  capture script and CLI files; `cli.md` and `umbrella.md` name the new command and features.
- `spot/README.md`, compiled as a doctest under `spot`; the crate README names both modules.
- A spine amendment in `spine-amendments/` records `ProductClass::Spot` and `SpotMarket`.

## Stories, in order

1. **Shared REST plumbing.** `shared::{weight, policy, error, request, positional, symbol}`;
   `usdm` on top, public API unchanged. Every `usdm` suite, the removals and the test-bodies jobs
   are green.
2. **Shared socket plumbing.** `shared::ws`, generic over a stream set; `usdm::ws` on top.
   `supervision_usdm*` and `wire_agreement_usdm_ws` are green unchanged.
3. **`ProductClass::Spot` and `SpotMarket`**, with the registry's class.
4. **Spot REST**: probes and captures first, then the client, types, weight budget, errors and
   their suites.
5. **`spot::venue`** and keys, `venue_spot`, `live_spot`, `live_venue`.
6. **`spot-ws`**: the WS probe first, then the streams, supervision suites and `live_spot_ws`.
7. **Umbrella, CLI and docs.**

Each story's intermediate commits build (`cargo hack` per feature included).

## Testing

| Story | Tests |
|---|---|
| 1, 2 | The `usdm` suites unchanged; moved unit tests with bodies kept; `MUTANTS.md` rows re-proved; `api_removals.py` and `test_body_diff.py` clean |
| 3 | `ProductClass::Spot.as_str()`; the registration test holds `SpotKeys::CLASS` to `classes`; the umbrella's class test |
| 4 | `mock_spot` (each route's path, query and `Cost`), `wire_agreement_spot` (each fixture decodes with no unmodelled key), `query_setters_spot`, `headers_spot`, `error_classes_spot`, `failure_tags_spot`, `ban_log_spot`, `documented_weights` spot rows |
| 5 | `venue_spot`: each method agrees with the native client on the fixtures, and a row that does not convert is skipped with its WARN |
| 6 | `wire_agreement_spot_ws`; `supervision_spot` on the scripted server: one path, the 250 ms pace, the 1024 cap, the names-per-request cap, `Disconnected` then `Reconnected`, rotation, `Stopped { cause }` |
| 7 | `tests/ws_binance.rs` spot cases; `parse_key` on `binance.spot:btcusdt` |
| Live | `live_spot` with `live_responses_carry_no_unmodelled_keys`; `live_spot_ws` with `live_frames_carry_no_unmodelled_keys`; both `#[ignore]`, failing only through test-support's tags |

`docs/MUTANTS.md` gains rows for spot's depth re-pricing and the 250 ms pace, each with the
mutation and the tests that fail under it.

### Gate

The nine CI jobs, run in `nix develop` for each story they touch: format; clippy, nextest,
doctests and the doc build; package; msrv; features (`cargo hack --each-feature`); removals; test
bodies; the CI scripts (dependency allowlists, `Classify` coverage, module READMEs, the venue
gate, generated regions, the rules files). Python is untouched.

## Downstream

No name prader-rs uses changes. When spot ships, the prader session is told: spot klines are what
Polymarket's crypto up-or-down markets resolve on.

## Open questions settled by evidence, not by this spec

- **The shared counter** between `api.binance.com` and `data-api.binance.vision`: one
  observation; the REST probe settles it.
- **The weight window** is assumed to be the UTC clock minute, as on `fapi`; the REST probe
  measures it across a boundary.
- **`depth` at 1001 to 5000** (250) is documented, not measured.
- **The WebSocket limits**: whether spot caps names per request, how it answers an unknown or
  uppercase name and an over-limit request, whether `!ticker@arr` still delivers, and the ping
  interval that keeps a connection under 5 messages per second. The WS probe settles each
  before story 6 is planned.
- **The 418, 451 and 403 bodies** on spot are not seen from this IP; classification uses the
  status alone, as on `usdm`.
