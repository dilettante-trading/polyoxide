---
paths:
  - "polyoxide-binance/**"
  - "polyoxide-cli/src/commands/binance/**"
  - "polyoxide-cli/tests/ws_binance.rs"
  - "scripts/capture_binance_usdm.py"
  - "docs/specs/binance/**"
---

# Binance USDⓈ-M

`polyoxide-binance`'s `usdm` module (feature `usdm`, on by default) reads public USDⓈ-M futures
market data on `fapi.binance.com` (`usdm::Usdm::new()`; namespaces `usdm.health()`, `exchange()`,
`market()`) for consumers that trade both venues. Binance is not a Polymarket host. The umbrella
re-exports the crate as `polyoxide::binance` under its `binance`, `binance-usdm`,
`binance-usdm-ws` and `full` features, none of which its default, `polymarket`, turns on.

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

## Streams (`usdm-ws`)

Eight USDⓈ-M market streams on `fstream.binance.com`: `UsdmWs` (one connection on one path, on the
kit's `Connection`) and `UsdmWsBuilder`/`SupervisedUsdmWs`.

- One kit `Supervisor` per routed path, `/market` or `/public` (a stream on the wrong path may
  deliver nothing), behind a router that sends each change to its path, opens a path for its first
  stream, and forwards both paths into one queue of 1024.
- The private `UsdmProtocol` (`src/usdm/ws/protocol.rs`) keeps Binance's rows: pings on the wall
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
  `#[path]`). The `test-server` feature, with `usdm-ws`, exposes the captured frames as
  `usdm::ws::fixtures`, which the CLI's tests import, and nothing else; it turns no module on, so a
  build that wants the frames names both features.
