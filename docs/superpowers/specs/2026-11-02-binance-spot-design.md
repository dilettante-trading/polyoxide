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
**Plans:** two, run with the supervised option (§6). Plan 1,
`docs/plans/2026-11-02-binance-spot-rest.md`: the shared REST plumbing, `ProductClass::Spot`,
spot REST, `spot::venue` and the umbrella's `binance-spot`. Plan 2,
`docs/plans/2026-11-02-binance-spot-ws.md`: the shared socket plumbing, `spot-ws`, the CLI command
and `binance-spot-ws`, written after the WebSocket probe (§10) settled spot's socket limits.

## 1. Goal and scope

**1.1 Goal.** Give `polyoxide-binance` a `spot` module that reads Binance spot public market data
on `api.binance.com` and streams its market streams on `stream.binance.com`, with no
credentials, `Decimal` money, typed errors, a measured weight budget and a supervised socket that
enforces spot's connection rules, so a consumer treats spot the way it treats `usdm`. Spot is the
first product of a new class, so `polyoxide-venue` gains `ProductClass::Spot` and the
`SpotMarket` bundle.

**1.2 In.**

- A crate-private `shared` module holding the Binance-wide plumbing `usdm` has today, with
  `usdm`'s public API unchanged (§5.2): REST in plan 1, sockets in plan 2.
- `ProductClass::Spot` and `SpotMarket` in `polyoxide-venue` (§5.3).
- Fourteen REST methods over eleven routes (§3.1), a spot `WeightBudget` and `SpotError`.
- `spot::venue`: `MarketData`, `Instruments`, `Trades`, `Candles` (§5.8).
- Nine market streams (§5.9), a bare socket and a supervised one, `SpotWsError`.
- The umbrella's `binance-spot` and `binance-spot-ws` features, `polyoxide binance spot stream`,
  the docs, probes, fixtures and the nightly jobs.

**1.3 Out**, each a later spec: `historicalTrades` (it needs an API-key header), `uiKlines`, the
rolling-window `ticker`, `ticker/tradingDay` and the `symbols=[…]` list forms; the diff-depth
stream and a maintained local book; the rolling-window `ticker_<w>` streams, `!ticker@arr` and
kline timezone offsets; `data-api.binance.vision` as a preset; a watcher of
`binance-spot-api-docs`'s CHANGELOG; signed routes, the user-data stream, the WebSocket API and
SBE; Python bindings; REST data commands in the CLI.

## 2. Decisions

| Question | Decision | Why |
|---|---|---|
| Slice | Public market data, REST and streams, matching `usdm` | The precedent's shape, no credentials |
| Default host | `https://api.binance.com`, settable on the builder | The host Binance's pages and SDKs name, as `usdm` uses `fapi`; a 451 on a US runner is already environmental |
| Shared plumbing | Extract a crate-private `shared` first, then build spot on it | The known differences are parameters (limits, layers, paths, pacing), not mechanisms; one copy of every Binance-wide fix |
| Moved tests | None: `usdm`'s unit tests stay in `usdm`'s files and reach the shared code through `usdm`'s wrappers, whose test hooks keep their bodies | The test-bodies gate keys an item by file and module path, so a moved test is a lost one; leaving them in place needs no `[[drift]]` or `[[retired]]` entry |
| Shared types | `Symbol` and `InvalidSymbol` only; every response type, `Interval` and `Filter` stays per product | A shared response type would couple two products' wire shapes; `polyoxide_venue::positional` already holds the shared decode |
| Spec docs | As `usdm`: `docs/specs/binance/` is not a mirror; the live suites are the drift detectors | `binance-api-swagger/spot_api.yaml` was last released 2024-10-08 (v1.22.0), lacks fields the wire sends today (`amendAllowed`, `opoAllowed`, `pegInstructionsAllowed`), and is 262 `/sapi` paths to 35 `/api/v3`; a nightly diff of it would never fire |
| Product class | `ProductClass::Spot` with a `SpotMarket: MarketData` bundle | Every class has one bound; a bundle names only what defines its class (ADR 0001), and nothing beyond `MarketData` defines spot |
| `book` depth | The default `depth`, 100 levels, weight 5 | 1000 levels costs 50; the native client offers up to 5000 |
| Diff depth | Out | Keeping a local book needs a REST snapshot sync, its own slice |
| Plans | Two: REST first, sockets after the WebSocket probe | The socket kit's generic shape depends on facts only the probe gives |
| Default features | `["usdm", "spot"]` | Each venue's HTTP modules are on by default; a default build (prader-rs's included) compiles spot too |
| `spot::Interval` | Exhaustive, with `1s` through `1M` | As `usdm`'s, which the owner pinned exhaustive on 2026-10-07 |
| `Symbol` on spot | `usdm`'s, accepting `_` | Spot lists no `_` symbol; the venue answers an unknown symbol `400` `-1121`, as for any other |

## 3. Venue contract

Measured against `api.binance.com` on 2026-11-02 unless marked documented.

### 3.1 HTTP

- `exchangeInfo`'s `rateLimits`: `REQUEST_WEIGHT` 6000 per minute, `RAW_REQUESTS` 300,000 per
  5 minutes, and two `ORDERS` limits that public routes never touch.
- Every response carries `x-mbx-used-weight` and `x-mbx-used-weight-1m`.
- `api.binance.com` and `data-api.binance.vision` appear to share one counter per IP: one call
  read 20 on the first and the next 40 on the second. One observation; plan 1's probe confirms or
  refutes it, and `OBSERVED.md` records the result.
- **A `limit` past a route's maximum is truncated, not refused**: `klines`, `trades` and
  `aggTrades` at `limit=1001` return 1000 rows, and `depth` at 5001 returns its maximum.
  `depth` takes any `limit` (7 returns 7 levels). An `aggTrades` window of two hours is accepted
  (500 rows). An unknown symbol is `400` with `{"code": -1121, "msg": "Invalid symbol."}`.

| Route | Method | Weight (measured) |
|---|---|---|
| `/api/v3/ping` | `spot.health().ping()` | 1 |
| `/api/v3/time` | `spot.health().time()` | 1 |
| `/api/v3/exchangeInfo` | `spot.exchange().exchange_info()`, with `.symbol(&s)` or without | 20 either way |
| `/api/v3/depth` | `spot.market().depth(&s)` | 5 without `limit` and at 1 to 100; 25 at 101 to 500; 50 at 501 to 1000; 250 above 1000 (documented) |
| `/api/v3/trades` | `trades(&s)` | 25, with or without `limit` |
| `/api/v3/aggTrades` | `agg_trades(&s)` | 4, with or without `limit` |
| `/api/v3/klines` | `klines(&s, interval)` | 2, with or without `limit` |
| `/api/v3/avgPrice` | `avg_price(&s)` | 2 |
| `/api/v3/ticker/24hr` | `ticker_24h(&s)` / `tickers_24h()` | 2 / 80 |
| `/api/v3/ticker/price` | `price(&s)` / `prices()` | 2 / 4 |
| `/api/v3/ticker/bookTicker` | `book_ticker(&s)` / `book_tickers()` | 2 / 4 |

Neither `depth` nor `ticker/bookTicker` returns a time.

### 3.2 WebSocket

Documented, and confirmed by the WebSocket probe into `OBSERVED.md` before plan 2 is written:

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

## 4. Limits

- **Rate budget.** REST: 5400 weight per UTC minute (the published 6000 less a tenth), per
  `WeightBudget`, shared by every `Spot` built with it. Sockets: §5.9's client-side limits.
- **Timeout.** Core's `ClientConfig` default, settable through `client_config_setters!`, as on
  `usdm`. The wait for the budget or a hold has no deadline of its own (a ban lasts up to 3 days),
  as `usdm`'s route builders document; a caller wraps a call in `tokio::time::timeout`.
- **Failure.** As `usdm` (`.claude/rules/binance.md`, "The retry policy"): a `429` with a retry
  left is retried and holds the client; one with none left holds to its `Retry-After` or the next
  UTC minute and fails; a `418` holds for its `Retry-After` or 2 minutes and fails; a `425`, 5xx
  and every other status fail with no retry; `418`, `451` and the firewall `403` are
  `Restricted`.
- **Caller-supplied numbers.** `limit` on `klines`, `trades` and `aggTrades` is a `u32` sent as
  given: the venue truncates past 1000 (§3.1), and the cost does not change. `GetDepth::limit` is
  a `u32` sent as given and priced by its band (§3.1); past 5000 the venue truncates and the cost
  is 250. `start_time`, `end_time` and `from_id` are sent as given, and the venue's refusal is a
  `SpotError::Venue`. The client validates only `Symbol` (1 to 32 letters, digits or `_`).
- **In flight.** One `Spot` at the default 4 concurrent requests has at most 320 weight in flight
  on every route at its default (4 × `tickers_24h`'s 80), under the reserve of 600; four `depth`
  calls above 1000 levels (1000) pass it. The builder's docs say so, as `UsdmBuilder`'s do.

## 5. Design

### 5.1 Crate layout

```
polyoxide-binance/src/
  shared/            pub(crate)
    mod.rs
    weight.rs        the minute engine, its clock, the header and the ban constants
    policy.rs        the retry decision
    error.rs         the decode by status and the class rules
    request.rs       Routed<T, R, E>, the Priced trait, route_builder!
    symbol.rs        Symbol, InvalidSymbol
    ws/              plan 2
  usdm/              as today; public paths unchanged
  spot/              mod.rs (Spot, SpotBuilder), api/, error.rs, key.rs, policy.rs, request.rs,
                     types.rs, venue.rs, weight.rs, README.md; ws/ in plan 2
```

`shared` is `#[cfg(any(feature = "usdm", feature = "spot"))]` once `spot` exists, and
`#[cfg(feature = "usdm")]` before that; `cargo hack --each-feature` builds `spot` without `usdm`
and `usdm` without `spot`, so every `shared` item compiles under either.

### 5.2 `shared`: what moves and what stays

**Moves**, parameterised where the products differ:

- **The minute engine** from `usdm/weight.rs`: the UTC clock minute at the published limit less
  the reserved tenth, raised by `X-MBX-USED-WEIGHT-1M` only for the minute its request was
  charged in; the client-wide `Hold` for a `429` or `418`, only ever extended, with the 3-day
  ceiling; an optional paced layer (`usdm`'s funding routes); and the clock (the system clock, and
  the test clocks). `USED_WEIGHT_HEADER`, `DEFAULT_BAN` and `MAX_COOLDOWN` move with it.
- **The retry decision** from `usdm/policy.rs`, as a function of the response, the attempt, the
  schedule and the time to the next minute.
- **The error decode** from `usdm/error.rs`: the status rules (`418`, `429`, `451`, `403` by
  status alone; the `{code, msg}` body; any other body kept as core's `ApiError` clipped, with its
  `Retry-After` clamped), `retry_after_secs`, `clip`, and the class of a `{code, msg}` error.
- **`Routed<T>` and `route_builder!`** from `usdm/request.rs`, generic over the product's route
  and error types.
- **`Symbol` and `InvalidSymbol`** from `usdm/types.rs`, their docs no longer naming one host.

**Stays in each product:** its `WeightBudget` (a handle to the shared engine, with its own
layers and its `Throttle` impl), its `<Module>RetryPolicy`, its `<Module>Error` with its variants,
`From<ApiError>`, the 418's WARN in its own module and its `Classify`, route tables and
`Route::cost`, response types and `Filter` enums, `Interval`, the key space, the venue impls,
`DEFAULT_BASE_URL`, and in plan 2 its `StreamName`, `Event`, `Payload`, `<Module>WsError` and
`InvalidStreamName`.

**`usdm`'s public API does not change.** `usdm::types::Symbol` and `InvalidSymbol`,
`usdm::weight::DEFAULT_BAN` and `MAX_COOLDOWN`, and `usdm::request::USED_WEIGHT_HEADER` are
`pub use` of the shared items; `WeightBudget`, `Cost`, `Route`, `WEIGHT_LAYER`, `FUNDING_LAYER`,
`PUBLISHED_WEIGHT_PER_MINUTE`, `PUBLISHED_FUNDING_PER_FIVE_MINUTES`, `Interval`, `UsdmError`
and the rest keep their paths, names, variants and docs' meaning (prader-rs pins `Interval`,
`StreamPath` and `Cost` exhaustive). `python3 scripts/api_removals.py check --baseline v0.40.0`
reports nothing new.

**No test moves.** `usdm`'s unit tests stay where they are, unchanged, and are the refactor's
regression guard with every `usdm` integration suite. The `#[cfg(test)]` hooks they call
(`WeightBudget::with_clock`, `at_ms`, `is_shared_with`, `hold_until_next_minute`,
`UsdmRetryPolicy { budget }`, the `Clock` variants) keep their names, signatures and bodies, so
`python3 scripts/test_body_diff.py --baseline v0.40.0` stays clean with no new entry. The
`docs/MUTANTS.md` rows whose lines move into `shared` move with them and are re-proved. `shared`
gets its own tests only for what `usdm`'s tests do not reach.

### 5.3 `polyoxide-venue`

- `ProductClass::Spot`, `as_str` `"spot"`: buying and selling the asset itself; a price is
  unbounded, a position is a balance of the base asset and so never negative, and there is no
  funding. The enum is already `#[non_exhaustive]`.
- `pub trait SpotMarket: MarketData {}` with `impl<T: MarketData> SpotMarket for T {}`, under
  feature `market`, re-exported at the crate root and documented as the other two bundles are.
- The docs and tests name no venue (`scripts/venue_gate.py`).

### 5.4 Client

`spot::Spot::new()` and `Spot::builder()` on a `ClientConfig`, with `client_config_setters!`,
`SpotBuilder::weight_budget` (one budget shared by every `Spot` built with it),
`Spot::weight_budget()`, gzip as `usdm` has it, `DEFAULT_BASE_URL = "https://api.binance.com"`
and `DEFAULT_MAX_CONCURRENT = 4`. Namespaces `health()`, `exchange()`, `market()` from
`namespaces!`; query setters from `query_setters!`, except `GetDepth::limit`, hand-written
because it re-prices the route (`clients.md` gains the row).

### 5.5 Weight budget

`spot::weight::WeightBudget` on the shared engine at 5400 of `PUBLISHED_WEIGHT_PER_MINUTE =
6000`, `WEIGHT_LAYER = LayerId("binance-spot-weight")`, no paced layer. `spot::weight::Cost` and
`Route` are spot's own. `RAW_REQUESTS` is not modelled: every request weighs at least 1, so the
budget caps a client at 27,000 requests per 5 minutes, under the 300,000 limit; the module docs
say so. `Route::cost` holds §3.1's table, pinned by a `documented_weights` test, and a new
example, `examples/weight_probe_spot.rs` (its own `[[example]]` with `test = true` and
`required-features = ["spot"]`, and its own unit tests), probes the spot routes.
`examples/weight_probe.rs` stays byte for byte: it is `test = true`, so the test-bodies gate
counts every item in it, and a `--product` argument would change them (owner, 2026-11-02).

### 5.6 Types

Spot's own, in `spot::types`: `ServerTime`, `ExchangeInfo`, `RateLimit`, `SymbolInfo` (with
`permissionSets`, the `*Allowed` flags and the precisions), an `open_enum!` `Filter` with the
filter types the captured `exchangeInfo` carries, `Ticker24h`, `PriceTicker`, `BookTicker`,
`AvgPrice`, `Trade`, `AggTrade`, `Kline`, `Depth` (`last_update_id`, `bids`, `asks`) and
`Level`, `Kline` and `Level` decoding positionally through `polyoxide_venue::positional` in
visitors that tolerate extra elements. Money is `Decimal`; response rows carry symbols as
`String`. `spot::Interval` is exhaustive, `1s` through `1M`. A decimal the wire sends as `""` is
an `Option`; the captures say which. `Symbol` and `InvalidSymbol` are `pub use` of the shared
items.

### 5.7 Errors

`SpotError`, `#[non_exhaustive]`, with `UsdmError`'s variants and meaning, decoded in its one
`From<ApiError>` through `shared::error`, and `Classify` as `UsdmError`'s. A `418` logs its body
at WARN under `polyoxide_binance::spot::error`. Listed in `lib.rs`'s `const _` assertion.

### 5.8 `spot::venue` and keys

`spot::key::SpotKeys` makes `binance.spot:<SYMBOL>` keys with `CLASS = ProductClass::Spot`,
canonicalising through `Symbol`, and `spot::key::key` is its constructor, as `usdm::key::key`.
`Spot` implements:

- `MarketData`: `instrument` from `exchangeInfo?symbol=` (20); `quote` from `ticker/bookTicker`
  (2), `time` `None`; `book` from the default `depth`, 100 levels (5).
- `Instruments`: `exchangeInfo` (20); a row that does not convert is skipped with a WARN naming
  the symbol and the error, and the live suite fails when one is skipped.
- `Trades`: `aggTrades` with `limit=1000` (4).
- `Candles`: `klines` with `limit=1000` (2).

With `InstrumentExt`, `BookExt` (`last_update_id`), `TradeExt` and `CandleExt`. A quote has no
extension: `ticker/bookTicker` carries nothing beyond the record's fields.
`lib.rs` pins `spot::Spot` to `SpotMarket`.

### 5.9 Streams (`spot-ws`, plan 2)

`SpotWs` (one connection) and `SpotWsBuilder`/`SupervisedSpotWs`, on `shared::ws` with one path,
`/stream`. `SPOT_WS_BASE = "wss://stream.binance.com:9443"` is the host, as `USDM_WS_BASE` is, and
`base_url` sets another; the client appends `/stream`, so a test server's base works as it does
for `usdm`. `SpotWs::connect(streams)` and `connect_to(base_url, streams)` take no path.

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
one variant, `Stream` (`as_str` `"stream"`), and `Event::Disconnected` and `Reconnected` keep
their `{ path }` field: the shared code and a consumer folding outages treat both products alike.
`StreamName` is the only way to name a stream. A partial-depth frame carries no symbol, so
`PartialDepthEvent::symbol` is parsed from the envelope's `stream` and not serialised back; a
`bookTicker` frame carries its symbol as `s` and no event type (`OBSERVED.md`, `### Frames`). The
keys Binance documents as "Ignore", `M` on trades and aggregate trades and `B` on the kline, are
not modelled, as `usdm`'s kline `B` is not. The payload types' fields are plan 2's names contract.

The client enforces spot's rules first: at most 1024 streams per connection, requests paced at
one per 500 ms (2 a second). The WebSocket probe's request every 250 ms was closed after 16 s
with 1008 "Too many requests", and one every 500 ms held for 45 s with pings and pongs
(`OBSERVED.md`, `## Spot`); at 500 ms a second holds at most three
requests when one slips, with the automatic pong and the wall-clock ping five in all, and since
one request can carry all 1024 names a replay costs little at that pace. A request carries at
most 1024 names, the connection's cap (`usdm`'s is 200): the probe's one `SUBSCRIBE` of 1024
names, 23,598 bytes, was acknowledged and held. `spot::ws` exposes them as
`MAX_STREAMS_PER_CONNECTION = 1024`, `MAX_NAMES_PER_REQUEST = 1024` and
`MIN_REQUEST_INTERVAL = 500 ms`. The builder's defaults are `usdm`'s: a ping every 20 s (the
server's own cadence, measured), stale after 30 s, backoff from 500 ms to 60 s, a 10 s connect
timeout and rotation at 23 h 50 min. Every `Event::Disconnected { path }` is followed by
`Event::Reconnected { path }` while the client runs, from the shared code. A path that stops on
an error ends the stream, and `close()` and later membership calls answer
`SpotWsError::Stopped { cause }`, as `usdm`'s do. `SpotWsError` has `UsdmWsError`'s variants, docs
and `Classify` rows but `WrongPath`, which one path cannot produce, and `spot::ws::StopCause` is
its own, with `usdm`'s fields and `of(&SpotWsError)`.

`shared::ws` holds what `usdm::ws` has that is not USDⓈ-M's: the `Protocol` (wall-clock pings
while the queue is full, staleness counting every frame, reconnect with a paced replay bounded by
`stale_after`, rotation at 23 h 50 min, a change recorded before it is sent), the router and its
forwarding queue of 1024, the `StopCause` machinery and the client-side limits, generic over a
product's stream set (its stream name type and wire spelling, each name's path, the paths, the
frame decode, the pacing interval and the cap on names per request). Its shape:

- a crate-private `StreamSet` trait that each product implements on a marker type: its stream
  name, path, update, error and event types; its paths, URL, decode, pacing interval and names
  per request; and how its events carry an update, an outage and a return;
- a `SocketError` trait its error implements: constructors for the errors the shared code makes
  (a refusal, no answer, a malformed answer, too many streams, a stop with its cause) and a
  `Shape` view of the variant, which the recovery and the router match on instead of the
  product's enum;
- generic internals: the request correlation, the bare connection, `BinanceProtocol<S>` (the
  kit `Protocol`), the router, the membership commands, the forwarding and the stop.

Each product's public types (`UsdmWs`, `UsdmWsBuilder`, `SupervisedUsdmWs`, `MembershipHandle`,
`Event`, `DisconnectReason`, its error and `StopCause`) stay its own structs and enums, wrapping
or feeding the generic code, so `usdm`'s public paths, names, signatures and docs' meaning do not
change. The private items `usdm`'s unit tests use keep their names and signatures:
`UsdmProtocol` and `Change` become aliases of the generic types (under `#[cfg(test)]`, since
only the tests name them), and `UsdmWs::open`, `UsdmWs::close_frame` and the builder's
`base_url` and `timing` fields stay. `Answer`, which no unit test names, moves to
`shared::ws::request` (plan time, 2026-11-02). One hook cannot keep its body:
`UsdmWs::close_frame`'s v0.40.0 body reads `self.connection`, which now lives inside
`shared::ws::bare::Bare`, so that body moves unchanged to `Bare::close_frame` and the hook
delegates to it; it takes a `[[drift]]` entry in `ci/test-body-diff.toml` (the new body's sha256)
with a DRIFT row in `_bmad-output/specs/spec-venue-extensibility/divergences.md` saying so, the
one exception to §5.2's "no new entry" (owner, 2026-11-02). A crate-private type in `shared::ws`
that names `polyoxide_ws::WsError` without being a classified socket error (the `Shape` view's
`Connect(&WsError)`, a test-only error) is named under its own spelling and takes an explicit,
commented exemption in `.github/scripts/tests/test_classify_coverage.py`, never an import alias
that hides it from the fence (owner, 2026-11-02).
`supervision_usdm*` and `wire_agreement_usdm_ws` are its regression guard, unchanged, with
`usdm`'s unit tests.

The offline tests drive the kit's scripted server with Binance's answer. `src/usdm/ws/scripted.rs`
names nothing of either product, so it serves spot too: `spot::ws`'s unit tests and the spot
suites include it with `#[path]`. Moving it would lose a test-context file and copying it would
keep two. `test-server` with `spot-ws` exposes `spot::ws::fixtures`.

### 5.10 Features and registration

```toml
default = ["usdm", "spot"]
spot = [ …the same optional dependencies and venue features as usdm… ]
spot-ws = ["spot", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]  # plan 2
```

- `[package.metadata.polyoxide]`: `products = ["usdm", "spot"]`,
  `classes = { usdm = "perpetual-future", spot = "spot" }`, the `readme` line naming `spot`, and
  `identifiers` gaining `api/v3` (and `stream.binance.com` in plan 2); neither appears in the
  foundation crates today.
- Live entries `live_spot` (`spot`, plan 1) and `live_spot_ws` (`spot-ws`, plan 2), suite
  `spot`, 15 minutes, no secrets: a new nightly job `live-polyoxide-binance-spot`.
- The crate's description and keywords name spot; docs.rs builds `spot` (and `spot-ws`).
- `ci/dep-allowlists/` regenerated for the new feature sets; `test_dep_allowlists.py` learns them.

### 5.11 Umbrella

`binance-spot = ["dep:polyoxide-binance", "polyoxide-binance/spot", "polyoxide-venue/market"]`
and `binance = ["binance-usdm", "binance-spot"]` (plan 1); `binance-spot-ws = ["binance-spot",
"polyoxide-binance/spot-ws"]` and `full` gaining it (plan 2). `parse_key` knows `binance.spot`;
`live_venue`'s features gain `binance-spot`. The umbrella's tests are appended, never edited
(AD-12): a spot key's class and `parse_key` on `binance.spot` are new tests in
`polyoxide/tests/parse_key.rs`. `SpotMarket` is at `polyoxide_venue`'s root beside the other
bundles, so at `polyoxide::venue`; the prelude gains `Spot`, `SpotBuilder`, `SpotError` and
`WeightBudget as SpotWeightBudget` under `binance-spot` (usdm's `WeightBudget` keeps the bare
name), and the passthrough check gains `const _: fn(crate::binance::spot::Spot) = drop;`. Under
`binance-spot-ws` (plan 2) the prelude gains `Event as SpotEvent`, `StreamName as SpotStreamName`,
`SpotWs`, `SpotWsBuilder` and `SpotWsError` (usdm's `StreamName` keeps the bare name), and the
passthrough checks gain `SpotWs` at `crate::binance::spot::ws` and at the prelude.

### 5.12 CLI (plan 2)

`polyoxide binance spot stream`, the counterpart of `binance usdm stream`: the supervised tier,
a `commands::stream::Feed`, the same flags, and `--format json` printing each frame's envelope.
Its kinds are `agg-trade`, `trade`, `kline-<interval>`, `mini-ticker`, `ticker`, `book-ticker`,
`avg-price`, and `depth5`, `depth10` and `depth20`, each every 100 ms; `--all-mini-tickers`
streams `!miniTicker@arr`, in the place of `usdm`'s `--all-tickers` and `--all-mark-prices`.
`pretty` prints a line per update and per array row, as `usdm`'s does.
`polyoxide-cli` builds `polyoxide-binance` with `usdm-ws` and `spot-ws`. `tests/ws_binance.rs`
gains spot cases on `spot::ws::fixtures`.

### 5.13 Docs

- `docs/specs/binance/INDEX.md`: a spot section with the routes table (plan 1) and the streams
  table (plan 2).
- `docs/specs/binance/OBSERVED.md`: a `## Spot` section with the measured weights, the
  truncation rules, the shared-counter result (plan 1), the stream limits, acknowledgements and
  frame shapes (plan 2).
- `docs/specs/binance/probes/`: spot modes for `probe_rest.py` (plan 1) and `probe_ws.py`
  (plan 2); a capture script, `scripts/capture_binance_spot.py`, with fixtures under
  `polyoxide-binance/tests/fixtures/spot/` and a `PROVENANCE.md`.
- The `nightly-schema` exclusion for `docs/specs/binance/` says spot's swagger is stale since
  2024-10-08 and lacks live fields; `gen_registry.py --write` regenerates every region.
- `.claude/rules/binance.md`: `shared` and `spot` sections, and its `paths:` gain the spot
  capture script (and the CLI's spot files in plan 2); `clients.md` gains `GetDepth::limit`'s
  row; `umbrella.md` names the new features.
- `spot/README.md`, compiled as a doctest under `spot`; the crate README names both modules.
- A spine amendment, `spine-amendments/binance-spot.md`, records `ProductClass::Spot` and
  `SpotMarket`.

## 6. Plans and stories

Each story's intermediate commits build, `cargo hack` per feature included.

**Plan 1, REST.**

1. **REST probe and captures.** Spot modes for `probe_rest.py`, `scripts/capture_binance_spot.py`
   and the REST fixtures; `OBSERVED.md`'s `## Spot` section with §3.1 and the shared-counter
   and minute-window results. No Rust.
2. **Shared REST plumbing.** `shared::{weight, policy, error, request, symbol}`; `usdm` on top,
   public API and tests unchanged.
3. **`ProductClass::Spot` and `SpotMarket`.**
4. **The spot client**: the `spot` feature, the client and builder, the weight budget and its
   full route table, the retry policy, `SpotError`, the health and exchange routes and their
   types, and their suites.
5. **Spot market routes**: depth, trades, aggregate trades, klines, average price and the three
   tickers, their types and setters, the wire-agreement suite and the `weight_probe` rows.
6. **`spot::venue`**, keys, registration, `live_spot`, the umbrella's `binance-spot`,
   `parse_key` and `live_venue`.
7. **Docs**: `INDEX.md`, the rules files, the READMEs, the nightly-schema exclusion, the spine
   amendment.

**Plan 2, sockets** (written after the WebSocket probe): the probe and the frame captures;
`shared::ws` under `usdm::ws`; `spot-ws`; the CLI command and `binance-spot-ws`; the docs.

## 7. Testing

| Story | Tests |
|---|---|
| 1.2 | Every `usdm` suite and unit test unchanged; `api_removals.py` and `test_body_diff.py` clean; `MUTANTS.md` rows re-proved at their new lines |
| 1.3 | `ProductClass::Spot.as_str()`; `SpotMarket` holds a type with `MarketData` |
| 1.4 | `mock_spot` for the health and exchange routes and a `429` and `418` holding the budget, `headers_spot`, `error_classes_spot`, `failure_tags_spot`, `ban_log_spot`, `documented_weights` for spot's whole table |
| 1.5 | `mock_spot` for each market route's path, query and `Cost`; `wire_agreement_spot` (each fixture decodes with no unmodelled key); `query_setters_spot` |
| 1.6 | `venue_spot`: each method agrees with the native client on the fixtures, and a row that does not convert is skipped with its WARN; the registration test holds `SpotKeys::CLASS`; new `parse_key.rs` tests for `binance.spot:btcusdt` and its class |
| 2 | `wire_agreement_spot_ws`; `supervision_spot` on the scripted server: one path, the pace, the 1024 cap, the names-per-request cap, `Disconnected` then `Reconnected`, rotation, `Stopped { cause }`; `tests/ws_binance.rs` spot cases |
| Live | `live_spot` with `live_responses_carry_no_unmodelled_keys`; `live_spot_ws` with `live_frames_carry_no_unmodelled_keys`; both `#[ignore]`, failing only through test-support's tags |

`docs/MUTANTS.md` gains rows for spot's depth re-pricing (plan 1) and the socket pace (plan 2),
each with the mutation and the tests that fail under it.

## 8. Gate

The nine CI jobs, run in `nix develop` for each story they touch: format; clippy, nextest,
doctests and the doc build; package; msrv; features (`cargo hack --each-feature`); removals; test
bodies; the CI scripts (dependency allowlists, `Classify` coverage, module READMEs, the venue
gate, generated regions, the rules files). Python is untouched.

## 9. Downstream

No name prader-rs uses changes. When spot ships, the prader session is told: spot klines are what
Polymarket's crypto up-or-down markets resolve on.

## 10. Open questions settled by evidence, not by this spec

- **The shared counter** between `api.binance.com` and `data-api.binance.vision`: one
  observation; plan 1's probe settles it.
- **The weight window** is assumed to be the UTC clock minute, as on `fapi`; plan 1's probe
  measures it across a boundary.
- **`depth` above 1000** (250) is documented, not measured.
- **The WebSocket limits**: whether spot caps names per request, how it answers an unknown or
  uppercase name and an over-limit request, whether `!ticker@arr` still delivers, and the ping
  interval that keeps a connection under 5 messages per second. The WebSocket probe settles each
  before plan 2 is written. It measures the over-limit answer with one deliberate burst on one
  connection and stops at the first disconnect, never repeating it: repeated excess bans the IP,
  which the live suites and other sessions on this host share (owner, 2026-11-02). Settled
  2026-11-02 (`OBSERVED.md`, `### Stream limits`): no cap on names per request below 1024; an
  unknown or uppercase name is acknowledged and delivers nothing; the over-limit answer is a close
  with 1008 "Too many requests", seen at a 250 ms pace, so the burst was not run; `!ticker@arr`
  delivers nothing; the server pings every 20 s, and a 500 ms pace with a ping every 20 s held.
- **The 418, 451 and 403 bodies** on spot are not seen from this IP; classification uses the
  status alone, as on `usdm`.
