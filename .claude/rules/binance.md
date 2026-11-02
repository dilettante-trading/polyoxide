---
paths:
  - "polyoxide-binance/**"
  - "polyoxide-cli/src/commands/binance/**"
  - "polyoxide-cli/tests/ws_binance.rs"
  - "scripts/capture_binance_usdm.py"
  - "scripts/capture_binance_spot.py"
  - "docs/specs/binance/**"
---

# Binance: USDⓈ-M and spot

`polyoxide-binance`'s `usdm` module (feature `usdm`, on by default) reads public USDⓈ-M futures
market data on `fapi.binance.com` (`usdm::Usdm::new()`; namespaces `usdm.health()`, `exchange()`,
`market()`) for consumers that trade both venues. Binance is not a Polymarket host. The umbrella
re-exports the crate as `polyoxide::binance` under its `binance`, `binance-usdm`, `binance-usdm-ws`,
`binance-spot`, `binance-spot-ws` and `full` features, none of which its default, `polymarket`,
turns on. Its `spot` module (feature `spot`, on by default) reads public spot market data on
`api.binance.com`; the `shared` and `spot` sections below hold the rules both modules share and
spot's own.

- **No spec to mirror.** Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M, so
  `docs/specs/binance/` is not a mirror: `OBSERVED.md` records what the host does,
  `nightly-schema.yml` has nothing to diff, and
  `tests/live_usdm.rs::live_responses_carry_no_unmodelled_keys` (REST) and
  `tests/live_usdm_ws.rs::live_frames_carry_no_unmodelled_keys` (streams) are the drift detectors.
- `Symbol` is for what a caller sends: it uppercases ASCII and accepts `_` for quarterlies
  (`BTCUSDT_261225`). Response rows carry symbols as `String`.
- `FundingRate::mark_price` is an `Option`, because funding events through at least 2022-01-01
  send `""`.
- Klines and book levels decode with `polyoxide_venue::positional`'s `element` and `drain`, in
  visitors that tolerate extra elements. Its `Interval` is separate from perps' (D16).
- A 451 (Binance refusing a US runner's location) is `Restricted` and not a fault, so the nightly
  files it as environmental.

## `shared`

`src/shared/` is crate-private and built under `any(feature = "usdm", feature = "spot")`. It holds
what the two products do the same way, so one fix serves both, and `cargo hack --each-feature`
builds each product without the other, so every item in it compiles under either.

- `shared::weight`: the minute `Engine` (the UTC clock minute at the published count less a tenth,
  raised by `X-MBX-USED-WEIGHT-1M` only for the minute its request was charged in; the client-wide
  `Hold` for a `429` or `418`, only ever extended, with the 3-day ceiling; an optional paced layer,
  which only `usdm`'s funding routes use), its `Clock` and the `USED_WEIGHT_HEADER`, `DEFAULT_BAN`
  and `MAX_COOLDOWN` constants. A product's `WeightBudget` is a handle to one `Engine` with its own
  layers and its own `Throttle` impl.
- `shared::policy::decide`: the retry decision as a function of the response, the attempt, the
  schedule and the time to the next minute. "The retry policy" below is its rules.
- `shared::error`: `decode` by status first into `Decoded`, `retry_after_secs`, `clip` and
  `venue_class`. A product's one `From<ApiError>` maps `Decoded` one to one onto its error enum and
  logs the `418` body at WARN itself, so the WARN's target is `polyoxide_binance::<module>::error`.
- `shared::request`: `Routed<T, R, E>`, the `Priced` trait a route table implements, and the
  `route_builder!` macro that writes every route builder with no setters of its own.
- `shared::ws`, built under `any(feature = "usdm-ws", feature = "spot-ws")` and so compiling under
  either alone: what both products' sockets do the same way, generic over a product's `StreamSet`
  (its stream name, path, update, error and event types, its paths and URL, its frame decode, its
  pacing interval and its cap on names per request) and its error's `SocketError` (constructors for
  the errors the shared code makes, and a `Shape` view the recovery and the router match on): the
  request correlation (`request`), the bare connection (`Bare`), the kit `Protocol`
  (`BinanceProtocol<S>`), and the router with its forwarding queue of 1024 and its stop
  (`supervised`). Each product's public socket types stay its own structs and enums over it.
- `shared::symbol`: `Symbol` and `InvalidSymbol`. Nothing else is shared: response types,
  `Interval` and `Filter` stay per product, so one product's wire change never reaches the other.
- **A product keeps** its `WeightBudget`, its `<Module>RetryPolicy`, its `<Module>Error`, its
  route table and `Route::cost`, its types, its key space, its venue impls and its
  `DEFAULT_BASE_URL`.
- **`usdm`'s public paths do not change.** `usdm::types::Symbol`, `usdm::weight::DEFAULT_BAN` and
  the rest are `pub use` of the shared items. `usdm`'s unit tests stay in `usdm`'s files, where
  they reach the shared code through `usdm`'s wrappers, because the test-bodies gate keys a test by
  its file and module path: a moved test is a lost one.
- `docs/MUTANTS.md`'s rows (a) and (k) name `src/shared/policy.rs`.

## The weight budget

Binance limits each IP by request *weight*, which depends on the route and its parameters, so the
crate has its own `usdm::WeightBudget` (`src/usdm/weight.rs`) instead of core's `RateLimiter`:

- the UTC clock minute at 2160 of the published 2400 (the tenth core's `RESERVED_FRACTION` also
  reserves), raised by every response's `X-MBX-USED-WEIGHT-1M`, but only for the minute its request
  was charged in;
- a separate bucket for the weightless funding routes (450 per 5 minutes, depth one, paced by
  core's `WindowQuotaTable::paced_interval` on the budget's own clock);
- a `429` or `418` held as a client-wide cooldown that is only ever extended (core's `Hold`, with a
  3-day ceiling).

The budget is the client's `Throttle` on core's send loop: each route builder (`GetKlines`,
`GetTime` and the rest, over a private `Routed<T>`) hands core's `Request` its weight as a `Cost`
on `usdm::weight::WEIGHT_LAYER` or `FUNDING_LAYER`, and every `Usdm` built with one budget shares
its hold. Pass one with `UsdmBuilder::weight_budget`.

The weight table, `Route::cost`, is measured, not copied: Binance's page is one off at the 100 and
500 `klines` edges, and does not say that omitting `limit` costs 5 on `klines` and 1 on `depth`.
`cargo run -p polyoxide-binance --example weight_probe` re-measures it. `GetKlines::limit` and
`GetDepth::limit` are hand-written setters because they also re-price the route.

## The retry policy

The private `UsdmRetryPolicy` (`src/usdm/policy.rs`):

- retries a `429` with a retry left, holding the longer of its backoff and its `Retry-After`;
- holds every request to the next minute on a `429` with no retry left and no `Retry-After`, since
  sending into a spent minute is how a `429` becomes a `418` ban;
- fails a `418` and holds its `Retry-After`, or two minutes;
- never retries a `425`.

A `418` logs its body at WARN under `polyoxide_binance::usdm::error`, and the send loop warns of
every hold under `polyoxide_core`. Its `418`, `451` and firewall `403` are `Restricted` (D14), not
retriable.

## `usdm::venue`

`MarketData`, `Instruments`, `Trades`, `Candles` and `Funding` on `Usdm`, with `InstrumentExt`,
`QuoteExt`, `BookExt`, `TradeExt`, `CandleExt` and `FundingExt`.

- Its quote is a 5-level `depth` (weight 2) and its book an unlimited one (500 levels, weight 1);
  its `trades` ask for 1000 rows (weight 20), its `candles` for 500 (weight 2, where no limit
  costs 5) and its `funding_history` for 1000.
- Its `instruments` skips an `exchangeInfo` row that does not convert (a WARN naming the symbol
  and the error), and the live suite fails when it skips one.
- Its funding methods send `fundingInfo` as well (one extra request on the funding layer,
  `fundingRate`'s bucket, not a weight) for the `interval`: `fundingIntervalHours`, or 8 h for a
  symbol it does not list.
- `tests/venue_usdm.rs` holds each method to the native client's values on the captured fixtures.

## `spot`

`polyoxide-binance`'s `spot` module (feature `spot`, on by default) reads public spot market data
on `api.binance.com` (`spot::Spot::new()`; namespaces `spot.health()`, `exchange()`, `market()`),
fourteen methods over eleven routes, with no credentials. It is the first product of
`ProductClass::Spot`.

- **No spec to diff.** Binance's `spot_api.yaml` was last released on 2024-10-08, lacks fields the
  wire sends today (`amendAllowed`, `opoAllowed`, `pegInstructionsAllowed`) and is mostly `/sapi`
  paths, so `nightly-schema.yml` does not watch it.
  `tests/live_spot.rs::live_responses_carry_no_unmodelled_keys` is the drift detector, and a
  measured fact goes in `docs/specs/binance/OBSERVED.md`'s `## Spot` section.
- **The weight budget.** `spot::WeightBudget` is the shared `Engine` at 5400 of
  `PUBLISHED_WEIGHT_PER_MINUTE = 6000`, on `WEIGHT_LAYER`, `binance-spot-weight`, with no paced
  layer. The `RAW_REQUESTS` limit (300,000 per 5 minutes) is not modelled: every request weighs at
  least 1, so the budget caps a client at 27,000 requests per 5 minutes, and the module docs say
  so. Pass a budget with `SpotBuilder::weight_budget`; every `Spot` built with one shares its hold.
- **The weight table**, `spot::weight::Route::cost`, is measured over distinct URLs, not copied,
  and pinned by its `documented_weights` test: `ping` and `time` 1, `exchangeInfo` 20, `depth` 5
  without `limit` and for 1 to 100, 25 for 101 to 500, 50 for 501 to 1000 and 250 above (each band
  measured, `OBSERVED.md`'s `## Spot`), `trades` 25, `aggTrades` 4, `klines`, `avgPrice` and the
  one-symbol tickers 2, `ticker/24hr` without a symbol 80, `ticker/price` and `ticker/bookTicker`
  without one 4. `weight_probe_spot` re-measures it
  (`cargo run -p polyoxide-binance --example weight_probe_spot`, no `--product` argument;
  `weight_probe` is unchanged and measures USDⓈ-M). `GetDepth::limit` is a hand-written setter
  because it also re-prices the route (`clients.md`); `docs/MUTANTS.md`'s row (l) holds the
  re-pricing.
- **A `limit` past a route's maximum is truncated by the venue, not refused**, and is sent as the
  caller gave it; the cost does not change but for `depth`'s bands. Only `Symbol` is validated by
  the client.
- **In flight.** One `Spot` at the default 4 concurrent requests has at most 320 weight in flight
  on a route at its default (4 × `tickers_24h`'s 80), under the reserve of 600; four `depth`
  calls above 1000 levels (1000) pass it. The builder's docs say so.
- **The retry policy** is `UsdmRetryPolicy`'s, above: the private `SpotRetryPolicy`
  (`src/spot/policy.rs`) decides through `shared::policy::decide`, and `SpotError` has
  `UsdmError`'s variants, `code()` and `Classify`. A `418` logs its body at WARN under
  `polyoxide_binance::spot::error`.
- `Interval` is exhaustive (`1s` through `1M`) and separate from `usdm`'s. `Symbol` is `usdm`'s
  (`shared`), so it accepts `_` though spot lists no such symbol; the venue answers an unknown
  symbol `400` with `-1121`. `Filter` keeps an unknown `filterType` whole in `Filter::Other`. Klines
  and book levels decode through `polyoxide_venue::positional` in visitors that tolerate extra
  elements. Response rows carry
  symbols as `String`.
- **`spot::venue`:** `MarketData`, `Instruments`, `Trades` and `Candles` on `Spot`, with
  `InstrumentExt`, `BookExt`, `TradeExt` and `CandleExt`; a quote has no extension. Its `quote` is
  `ticker/bookTicker` (weight 2, `time` `None`), its `book` the default `depth` (100 levels,
  weight 5), its `instrument` and `instruments` `exchangeInfo` (weight 20), its `trades` `aggTrades`
  with `limit=1000` (weight 4) and its `candles` `klines` with `limit=1000` (weight 2). Its
  `instruments` skips a row that does not convert (a WARN naming the symbol and the error), and
  the live suite fails when it skips one. `lib.rs` pins `Spot` to `SpotMarket`. `spot::key`
  makes `binance.spot:<SYMBOL>` keys with `CLASS = ProductClass::Spot`.
- A 451, a `418` and the firewall `403` are `Restricted`, classified by status as on `usdm`.
- `tests/venue_spot.rs` holds each method to the native client's values on the captured fixtures;
  the fixtures are captured, never written by hand, by `scripts/capture_binance_spot.py rest`
  (the stream frames by its `ws`, described in the Streams (`spot-ws`) section below).

## Streams (`usdm-ws`)

Eight USDⓈ-M market streams on `fstream.binance.com`: `UsdmWs` (one connection on one path, on the
kit's `Connection`) and `UsdmWsBuilder`/`SupervisedUsdmWs`.

- One kit `Supervisor` per routed path, `/market` or `/public` (a stream on the wrong path may
  deliver nothing), behind a router that sends each change to its path, opens a path for its first
  stream, and forwards both paths into one queue of 1024. The router, the request correlation and
  the bare connection are `shared::ws`'s (`src/shared/ws/`), generic over `StreamSet`, and
  `usdm::ws`'s public types wrap them.
- The crate-private `BinanceProtocol<S>` (`src/shared/ws/protocol.rs`, generic over the product's
  `StreamSet`; `UsdmProtocol` is its alias on `UsdmStreams`) keeps Binance's rows: pings on the wall
  clock, also while the queue is full, so a slow consumer is never stale; staleness counting every
  frame, pongs and the server's pings included; reconnect with a paced replay, bounded as a whole
  by `stale_after`; rotation at 23 h 50 min, under Binance's 24-hour cutoff; a change recorded
  before it is sent, so one whose connection dies under it, or that the server leaves unanswered
  for `stale_after`, is answered `Ok` and replayed.
- Binance acknowledges every `SUBSCRIBE`, unknown and uppercase names and an explicit `@250ms`
  depth included (a 250 ms partial depth is the bare `<s>@depth<N>`), and enforces its rules by
  closing the connection (on the 1025th stream, or after about 15 requests in a burst). So the
  client enforces them first: `StreamName` is the only way to name a stream, and a connection
  carries at most 1024 streams, 200 names per request, one request per 200 ms.
- A path that stops on an error ends the whole stream after yielding it. Once the router has that
  stop, `close()` and every later membership call answer `UsdmWsError::Stopped { cause }`, the
  cause that error's `StopCause` (its class, fault, wait and text), so a stream stopped by a 451
  refusal is `Restricted` and not a fault. A change in flight when the path stops, or a `close()`
  made right after the stopping error, can still get `Stopped { cause: None }` or `Ok`.
- Every `Event::Disconnected { path }` is followed by `Event::Reconnected { path }` while the
  client runs, even when the path's last stream leaves mid-outage. prader-rs folds outages on that
  invariant; `tests/supervision_usdm.rs`, `tests/supervision_usdm_edges.rs` and
  `tests/supervision_usdm_kit.rs` pin it.
- COIN-M rows (`st: 2`) arrive on this host and count contracts, not USDT, so they do not sum with
  USDⓈ-M rows.
- The offline tests drive the kit's scripted server with Binance's answer
  (`src/usdm/ws/scripted.rs`, built for the unit tests and included by the integration suites with
  `#[path]`; `shared::ws`'s unit tests include it with `#[path]` too, under
  `#![cfg_attr(test, allow(clippy::duplicate_mod))]` at module level, since an allow on the
  `mod` item changes scripted.rs's test-body gate, and a further unit-test include needs the
  same). The `test-server` feature, with `usdm-ws`, exposes the captured frames as
  `usdm::ws::fixtures`, which the CLI's tests import, and nothing else; it turns no module on, so a
  build that wants the frames names both features.

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
  refused with nothing sent), and `docs/MUTANTS.md`'s row (m), the socket pace, holds the interval.
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
