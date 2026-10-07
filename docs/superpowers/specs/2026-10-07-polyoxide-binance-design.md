# Binance USDⓈ-M: public market data over HTTP and WebSocket

**Date:** 2026-10-07
**Branch:** `aidanb/polyoxide-binance`
**Slice:** 1 of `polyoxide-binance`: the public market data of Binance USDⓈ-M futures that
prader-rs's perps page reads. Spot, COIN-M, options, signed routes, user-data streams and
Python bindings are later specs.
**Consumer:** prader-rs's Binance perps venue
(`prader-rs/docs/superpowers/specs/2026-10-07-binance-perps-venue-design.md`, being amended
to read Binance through this crate instead of `binance-sdk`).
**Plans:** `docs/superpowers/plans/2026-10-07-polyoxide-binance-http.md` (the crate, the
weight limiter and the REST routes) and `docs/superpowers/plans/2026-10-07-polyoxide-binance-ws.md`
(the sockets and `polyoxide ws binance`), as perps was split.

## Goal

Give polyoxide a `polyoxide-binance` crate that reads Binance USDⓈ-M public market data on
`fapi.binance.com` and streams its public market streams on `fstream.binance.com`, with no
credentials, `Decimal` money, typed errors, a request-weight limiter, and a supervised
socket that enforces Binance's connection rules, so a consumer can treat Binance the way it
treats `polyoxide-perps`.

## Scope

In:

- Eleven REST routes: `ping`, `time`, `exchangeInfo`, `fundingInfo`, `ticker/24hr` (one
  symbol and all), `premiumIndex` (one and all), `klines`, `fundingRate`, `openInterest`,
  `aggTrades`, `depth`.
- Eight market streams on the two routed socket paths: `!ticker@arr`, `!markPrice@arr@1s`,
  `<s>@aggTrade`, `<s>@kline_<i>`, `<s>@markPrice@1s`, `<s>@ticker` on `/market`;
  `<s>@depth<5|10|20>@<100ms|250ms|500ms>`, `<s>@bookTicker` on `/public`.
- A bare socket and a supervised one (keep-alive, staleness, reconnect with resubscribe,
  outage markers, rotation before the 24 h cutoff, a membership handle).
- A crate-local request-weight limiter whose weights are measured and pinned.
- `polyoxide ws binance` in the CLI.
- `docs/specs/binance/` with `INDEX.md` and `OBSERVED.md`, fixtures and a capture script.

Out, each a later spec: spot, COIN-M and options; API keys, signed REST, the user-data
stream and the WebSocket API; a maintained local order book from diff-depth streams;
continuous, index and mark-price klines; liquidation, composite-index and contract-info
streams; Python bindings; a `binance` feature on the `polyoxide` umbrella crate.

## Decisions taken during brainstorming

| Question | Decision | Why |
|---|---|---|
| Where the client lives | A new crate in polyoxide | It reuses core's `HttpClient`, retry schedule and the supervised-socket pattern; it adds no dependency the consumer does not already build; the owner already maintains polyoxide |
| `binance-sdk` (Binance's official crate) | Rejected | Its socket handler builds a runtime and calls `expect` per message, panics on unsubscribing an unknown stream, and never resubscribes after a reconnect; its REST client defaults to a 1 s timeout, asks for `br` but decodes only gzip, reports HTTP 451 only as a message string, models one-or-many responses as untagged enums, and pulls in `native-tls` |
| Reviving the `dilettante-trading/binance-rs` fork | Rejected | An untouched fork of wisespace-io's 0.21.2: blocking `reqwest` 0.11 and `tungstenite` 0.21, prices parsed as `f64`, the legacy socket paths, a closed filter enum, `error-chain` |
| `polyoxide` umbrella crate | No `binance` feature, not in `full` | The umbrella describes itself as a Polymarket client |
| Weight limiter | Crate-local; promote to core when a second weight-based host appears | Core's limiter models Cloudflare window quotas per path; Binance charges a per-IP request weight that varies with parameters |
| CLI and Python | `polyoxide ws binance` now; Python later | A consumer and a probe tool for this slice; Python has no socket bindings for any feed yet |
| Module shape | Everything under `usdm`, the crate root re-exporting | Spot or COIN-M can be added later without renaming anything |
| Coverage | The eleven routes and eight streams the consumer reads | Binance has about twenty stream kinds; each costs a payload type and fixtures |
| Compression | Core's builder gains a gzip switch, off by default; this crate turns it on | `exchangeInfo` is 1.15 MB raw and 51 KB gzipped; no other crate's wire changes |

## Venue contract

Binance publishes no OpenAPI or AsyncAPI document for USDⓈ-M futures:
`github.com/binance/binance-api-swagger` holds `spot_api.yaml` only, and `binance-sdk` is
generated from inputs that are not public. The sources are the prose pages and the wire,
captured and probed on 2026-10-07. The REST pages are under
`developers.binance.com/docs/derivatives/usds-margined-futures/`. The stream pages have
moved to `developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/`,
one page per path (`market`, `public`), and their old URLs land on a generic page. Where
the pages and the wire disagree, the wire wins and `OBSERVED.md` records it. The
host is excluded from `nightly-schema.yml`; the live suites are its drift detector, as for
rtds and sports.

### HTTP

Base URL `https://fapi.binance.com`, fronted by CloudFront (`X-Cache: Miss from
cloudfront`, `cache-control: no-cache`). Every route here is an unauthenticated `GET`.
Prices and quantities are decimal strings, timestamps Unix milliseconds, symbols uppercase
on REST (`BTCUSDT`, and Chinese-character symbols such as `币安人生USDT`).

| Route | Parameters | Response | Weight |
|---|---|---|---|
| `/fapi/v1/ping` | | `{}` | 1 |
| `/fapi/v1/time` | | `{ serverTime }` | 1 |
| `/fapi/v1/exchangeInfo` | | `ExchangeInfo` | 1 |
| `/fapi/v1/fundingInfo` | | `[FundingInfo]` | none; shares 500 per 5 min per IP with `fundingRate` |
| `/fapi/v1/ticker/24hr` | `symbol` optional | one `Ticker24h`, or all without `symbol` | 1, or 40 for all |
| `/fapi/v1/premiumIndex` | `symbol` optional | one `PremiumIndex`, or all | 1, or 10 for all |
| `/fapi/v1/klines` | `symbol`, `interval`; `startTime`, `endTime`, `limit` (≤ 1500) | `[Kline]`, positional arrays | by `limit`: up to 100 → 1, up to 500 → 2, up to 1000 → 5, over 1000 → 10; no `limit` → 5 |
| `/fapi/v1/fundingRate` | `symbol`, `startTime`, `endTime`, `limit` (≤ 1000), all optional | `[FundingRate]` | none; shares 500 per 5 min |
| `/fapi/v1/openInterest` | `symbol` | `OpenInterest` | 1 |
| `/fapi/v1/aggTrades` | `symbol`; `fromId`, `startTime`, `endTime`, `limit` (≤ 1000); last 48 h only | `[AggTrade]` | 20 |
| `/fapi/v1/depth` | `symbol`; `limit` ∈ 5, 10, 20, 50, 100, 500, 1000 | `Depth` | 2 up to 50, 5 at 100, 10 at 500, 20 at 1000; no `limit` → 1 |

Measured on 2026-10-07 from `X-MBX-USED-WEIGHT-1M` deltas, every row except `ping` and
`time`: `exchangeInfo` 1, `ticker/24hr` 40 and 1, `premiumIndex` 10 and 1, `openInterest`
1, `aggTrades` 20 at limits 1, 100 and 1000. `klines` was measured at every band edge,
twice: 99 and 100 cost 1, 101 to 500 cost 2, 501 to 1000 cost 5, 1001 and 1500 cost 10.
The page's bands (under 100, 100 to 499, 500 to 1000) are off by one at each edge, and a
request without `limit` costs 5 although it returns 500 rows. `depth` costs 2 at 20 and 50,
5 at 100, 10 at 500 and 20 at 1000, and 1 without `limit`, which also returns 500 levels.
`fundingInfo` and `fundingRate` answer with no weight header. A request the venue refuses
still costs weight, an unknown symbol its route's: 1 on `premiumIndex`, 20 on `aggTrades`.
A `klines` limit of 1501 costs 10.

`aggTrades` serves only the last 48 hours, as its page documents. An older window is
refused with `400 {"code":-4166,"msg":"Search window is restricted to recent 2 days
only."}` (2026-10-07; the page does not give the code), so a backfill cannot reach further
back. The page also says a window with both `startTime` and `endTime` must span less than
an hour, but a two-hour window was answered `200` on 2026-10-07, so the client does not
enforce the rule.

The weight budget is `REQUEST_WEIGHT` 2400 per minute per IP, from `exchangeInfo`'s
`rateLimits`, reported on every weighted response in `X-MBX-USED-WEIGHT-1M`.

Errors are `{"code": <negative int>, "msg": <string>}` on a 4xx: an unknown symbol is
`400 {"code":-1121,"msg":"Invalid symbol."}` and costs its route's weight. A 429 carries
`Retry-After` in seconds; continuing after one earns a 418, an IP ban the docs say lasts
from 2 minutes to 3 days. A 451 means the caller's location is restricted, a 403 is the
web application firewall. Classification is by status; the bodies of 418, 451 and 403
were not observed from this IP.

`ticker/24hr` and `premiumIndex` answer an object with `symbol` and an array without it.
The client gives each shape its own method, never an untagged enum.

### WebSocket

Base `wss://fstream.binance.com` with routed paths: `/public` for high-frequency data
(partial depth, book ticker), `/market` for the rest, and `/private` for user data (out of
scope). The combined-stream form `<path>/stream` takes requests
`{"method": "SUBSCRIBE" | "UNSUBSCRIBE" | "LIST_SUBSCRIPTIONS", "params": [...], "id": n}`,
answers `{"result": null, "id": n}` or `{"error": {"code", "msg"}, "id": n}`, and pushes
`{"stream": <name>, "data": <payload>}`. A connection without a routed path receives
public-path data only: the legacy `/stream` serves depth but sent nothing for
`!markPrice@arr@1s` on 2026-10-07.

| Rule | Documented | Measured 2026-10-07 on `/market` |
|---|---|---|
| Streams per connection | 1024 | 1024 accepted. The 1025th is answered `{"error":{"code":4,"msg":"Too many subscriptions"}}` and the server then closes with 1008 "Invalid request", losing all 1024 |
| Incoming messages | 10 per second | 15 back-to-back requests all answered; 40 back to back: 15 answered, then close 1008 "Too many requests" |
| Acknowledgement | | Every `SUBSCRIBE` is acknowledged: an unknown symbol, a stream type that does not exist, an uppercase symbol. `LIST_SUBSCRIPTIONS` echoes them back. An `UNSUBSCRIBE` of a name never subscribed is acknowledged |
| Case | Symbols in stream names are lowercase | `BTCUSDT@aggTrade` is acknowledged and delivers nothing; `btcusdt@aggTrade` delivers |
| Server ping | A ping frame every 3 minutes; no pong for 10 minutes disconnects | Pings at 140 s and 320 s on a quiet connection, payload a millisecond timestamp |
| Client ping | | Answered in about 0.3 s |
| Connection lifetime | 24 hours | Not measured |
| Request size | | 200 names in one `SUBSCRIBE` acknowledged |
| Non-ASCII symbols | | Raw UTF-8 in `SUBSCRIBE` accepted; the stream name is echoed exactly (lowercase ASCII, `@1s` kept) |
| `!markPrice@arr@1s` | Every 1 s | Two frames per second (745 and 217 rows) |

So an acknowledgement proves nothing about a stream's validity, and a name that will never
deliver is indistinguishable from a quiet one. The crate prevents bad names by
construction instead.

## Design

### Crate layout

```
polyoxide-binance/
  Cargo.toml        depends on polyoxide-core; features: ws, test-server
  README.md         included as crate docs, so its examples are doctests
  src/
    lib.rs          re-exports usdm
    error.rs        BinanceError
    weight.rs       WeightBudget, the funding bucket, the weight table
    usdm/
      mod.rs        Usdm, UsdmBuilder, DEFAULT_BASE_URL
      request.rs    WeightedRequest, the send loop
      types.rs      Symbol, Interval, ContractType, SymbolStatus, UnderlyingType,
                    Filter, REST rows
      api/
        health.rs   ping, time
        exchange.rs exchange_info, funding_info
        market.rs   ticker_24h, tickers_24h, premium_index, premium_indices, klines,
                    funding_rate, open_interest, agg_trades, depth
      ws/           feature = "ws"
        mod.rs      ensure_crypto_provider (fifth copy), USDM_WS_BASE, StreamPath
        stream.rs   StreamName, DepthLevels, DepthSpeed: path(), Display, FromStr
        event.rs    Update, Payload and the six payload types, SymbolType
        error.rs    UsdmWsError, Recovery
        client.rs   UsdmWs, the bare tier (one connection, one path)
        supervised.rs  UsdmWsBuilder, SupervisedUsdmWs, MembershipHandle, Backoff
        test_server.rs scripted server; cfg(test) or feature test-server
        fixtures.rs    the captured frames as constants; cfg(test) or test-server
  examples/
    weight_probe.rs measures weights live; its unit tests pin the table (test = true)
  tests/
    mock_api.rs
    wire_agreement.rs
    ws_wire_agreement.rs  feature = "ws"
    supervision.rs        feature = "test-server"
    live_api.rs           #[ignore]
    live_ws.rs            #[ignore], feature = "ws"
    fixtures/             rest/, ws/, PROVENANCE.md
scripts/capture_binance_fixtures.py
```

Dependencies: `polyoxide-core`, `reqwest` (workspace, plus `gzip`), `rust_decimal`,
`serde`, `serde_json`, `thiserror`, `tracing`, `url`, `tokio` (`time`, `sync`); under `ws`
also `tokio-tungstenite`, `futures-util` and `rustls` with `ring` and `std` declared
explicitly, for the reason in CLAUDE.md's TLS note.

### Client

`Usdm::new()` and `Usdm::builder()` with `base_url`, `timeout_ms`, `pool_size`,
`with_retry_config`, `max_concurrent` (default 4, as perps) and
`weight_budget(WeightBudget)`, mirroring `PerpsBuilder`. The client is built with no core
`RateLimiter`, because core's path quotas do not model weights, and with `gzip(true)`. The
budget is per process and shared by cloning, because Binance's limit is per IP: two
`Usdm` clients in one process should be given one budget.

Three namespaces, each holding a cloned `HttpClient` and budget:

- `usdm.health()`: `ping()`, `time()`.
- `usdm.exchange()`: `exchange_info()`, `funding_info()`.
- `usdm.market()`: `ticker_24h(symbol)`, `tickers_24h()`, `premium_index(symbol)`,
  `premium_indices()`, `klines(symbol, interval)`, `funding_rate()`,
  `open_interest(symbol)`, `agg_trades(symbol)`, `depth(symbol)`.

Every route is a request builder ending in `.send().await?`. Required parameters are
constructor arguments, optional ones chained methods. Each builder knows its route's
weight from its parameters, so the limiter charges the right amount before the request
leaves.

### Types

- `Symbol`: 1 to 32 characters, each a Unicode letter, a digit or `_`, because Binance
  lists Chinese-character symbols (`币安人生USDT`) and quarterly contracts carry their
  delivery date after an underscore (`BTCUSDT_261225`). On 2026-10-07 the 924 listed
  symbols used no other character and the longest was 17. `Symbol::new` refuses anything
  else and uppercases ASCII letters: no listed symbol has a lowercase one, the REST host
  accepts `btcusdt` and answers `BTCUSDT`, and stream names spell symbols in lowercase, so
  uppercasing is what lets an echoed stream name parse back to the symbol that built it.
  `Symbol` is for what a caller sends; response rows carry symbols as `String`, taken as
  sent, so a symbol `Symbol::new` would refuse (`fundingInfo` also lists COIN-M symbols
  such as `BTCUSD_PERP`) never fails a whole response.
- `Interval`: Binance's fifteen (`1m 3m 5m 15m 30m 1h 2h 4h 6h 8h 12h 1d 3d 1w 1M`), with
  the exact wire spelling, shared by REST `klines` and the kline stream. The docs' list
  also has `1s`, which this host refuses (`-1120 "Invalid interval."`).
- `ContractType` (`Perpetual`, `TradifiPerpetual`, `CurrentQuarter`, `NextQuarter`),
  `SymbolStatus` (`Trading`, `Settling`, `PendingTrading`, and the documented others) and
  `UnderlyingType` (`Coin`, `Index`, `Premarket`, `Commodity`, `Equity`, `CnEquity`,
  `HkEquity`, `KrEquity`, `Fx`, seen live on 2026-10-07): each with an `Other(String)`
  variant, because Binance adds values (`TRADIFI_PERPETUAL` and three regional equity
  types are recent).
- `Filter`: the seven types seen live as struct variants named after the wire
  (`PriceFilter { tick_size, .. }`, `LotSize`, `MarketLotSize`, `MaxNumOrders`,
  `MinNotional`, `PercentPrice`, `PositionRiskControl`) and `Other { filter_type, raw }`,
  with hand-written serde, so an unseen filter type never fails a whole `exchangeInfo`
  (the closed enum in `binance-rs` does exactly that).
- `DepthLimit` (`Five`, `Ten`, `Twenty`, `Fifty`, `Hundred`, `FiveHundred`, `Thousand`),
  so a `depth` limit Binance refuses (`-4021`) cannot be built. Other limits are `u32`.
- REST rows named after their response (`ExchangeInfo`, `SymbolInfo`, `RateLimit`,
  `FundingInfo`, `Ticker24h`, `PremiumIndex`, `Kline`, `FundingRate`, `OpenInterest`,
  `AggTrade`, `Depth`), `Decimal` with `rust_decimal::serde::str` for prices, quantities and
  rates, `u64` for timestamps and ids, `Option<u64>` where the wire sends `null`
  (`fundingInfo`'s `updateTime` is `null` for `BTCUSDT`), long field names over the
  wire's terse keys where the wire is terse (`aggTrades`: `is_buyer_maker` for `m`).
  `#[non_exhaustive]`.
  `underlyingSubType` stays `Vec<String>` (24 free-text tags on 2026-10-07). `Kline` and
  depth levels are positional arrays on the wire and get hand-written serde.
- Stream payloads are separate structs (`TickerEvent`, `MarkPriceEvent`, `AggTradeEvent`,
  `KlineEvent`, `PartialDepthEvent`, `BookTickerEvent`), because the socket uses one-letter
  keys and different fields than REST. They share the vocabulary types above, not rows.
- Two fields the docs list and every capture carries. `st`, "(After CM migration) Symbol
  type: 1 = UM, 2 = CM", is on every stream payload except `KlineEvent`, and becomes
  `symbol_type: SymbolType` (`Um`, `Cm`, `Other(u8)`); every captured frame has `1`. `nq`,
  "Normal quantity without the trades involving RPI orders", is on `AggTrade` and
  `AggTradeEvent`, and becomes `normal_quantity: Decimal` beside `quantity`. It is
  required: `aggTrades` refuses a window older than two days (`-4166` "Search window is
  restricted to recent 2 days only.", 2026-10-07), so no trade from before the field
  existed can be fetched.

### Errors

`BinanceError` wraps `ApiError` through `impl_api_error_conversions!` (transport, timeout,
serialisation, URL) and adds:

| Variant | From | Retriable |
|---|---|---|
| `Venue { status, code, msg }` | any non-success whose body is `{code, msg}` | 408, 425 and 5xx, as core's `ApiError::is_retriable` for the same statuses |
| `RateLimited { retry_after }` | a 429 still refused after the retry schedule | yes |
| `IpBanned { retry_after }` | 418 | no; the budget holds every request until it lifts |
| `RegionBlocked { msg }` | 451 | no |
| `Forbidden { msg }` | 403 | no |

`is_retriable()` follows the last column. Bodies are clipped before they are kept, as
`polyoxide-core` clips them for logs.

`UsdmWsError`: `Connect`, `ConnectTimeout`, `Closed { code, reason }`, `Refused { code,
msg }` (an error answer to a request), `NoAnswer { id, timeout }`, `Response { id, raw }`
(a malformed answer), `TooManyStreams { path, limit }` (refused client-side, never sent),
`WrongPath { stream, path }` (bare tier only), `Frame { stream, raw, reason }`, `Stopped`,
each with a `Recovery` as in perps. Every variant can be built outside the crate; only the
enum is `#[non_exhaustive]`. Staleness is not an error: only the supervised tier detects
it, and reports it as a `DisconnectReason`.

### Weight limiter

`WeightBudget` (crate-local, `Clone`, shared through an `Arc`):

- **Budget.** 2400 per minute less a tenth, 2160, following core's `RESERVED_FRACTION`
  lesson that aiming at a published quota is a bug.
- **Charging.** `acquire(weight)` waits while the window's count plus `weight` would pass
  the budget, then adds `weight`. The window is the UTC clock minute: on 2026-10-07 the
  header fell to 1 within 3 s after 08:35:00 and again after 08:36:00, where a sliding
  window would have kept counting. A response's header is applied only to the window its
  request was charged in, so a request in flight across a boundary cannot carry the old
  minute's count into the new one.
- **Server count.** Every response's `X-MBX-USED-WEIGHT-1M` raises the window's count to
  at least that value, because other processes on the same IP spend the same budget. The
  count never moves down within a window.
- **Funding bucket.** `fundingRate` and `fundingInfo` draw on their own 500 per 5 minutes,
  450 after the reserve, as a token bucket of depth 1 (core's rule: depth is borrowed
  against the rate).
- **429.** A client-wide cooldown, where `Retry-After` may only extend the wait, as in
  core; the request is retried on core's schedule (`HttpClient::should_retry`).
- **418.** A client-wide cooldown for `Retry-After`, or 2 minutes when absent, the
  documented minimum ban. The request is not retried and returns `IpBanned`.
- **Table.** `weight(route, params)` encodes the HTTP table. The `documented_weights`
  test pins the effective weight of each row, and `examples/weight_probe.rs` re-measures
  it live from header deltas. That costs a few dozen weight, so unlike perps' soak it
  never approaches the limit.

`WeightedRequest` (in `usdm/request.rs`) is the send loop: take a concurrency permit, take
the weight, send through the shared `reqwest` client, record the header, apply the 429 and
418 rules, map a failure to `BinanceError`, decode. It follows `polyoxide_core::Request`
and uses only `HttpClient`'s public API.

### WebSocket: streams and paths

`StreamName` is the only way to name a stream. It is an enum, as prader-rs's contract
matches on its variants:

| Variant | Wire name | Path |
|---|---|---|
| `AllTickers` | `!ticker@arr` | market |
| `AllMarkPrices` | `!markPrice@arr@1s` | market |
| `AggTrade(Symbol)` | `<s>@aggTrade` | market |
| `Kline(Symbol, Interval)` | `<s>@kline_<i>` | market |
| `MarkPrice(Symbol)` | `<s>@markPrice@1s` | market |
| `Ticker(Symbol)` | `<s>@ticker` | market |
| `PartialDepth(Symbol, DepthLevels, DepthSpeed)` | `<s>@depth<5\|10\|20>@<100ms\|250ms\|500ms>` | public |
| `BookTicker(Symbol)` | `<s>@bookTicker` | public |

`<s>` is the symbol with its ASCII letters lowercased, so an uppercase name cannot be
built. `Display` renders the wire name and `FromStr` parses an echoed one, refusing an
uppercase symbol; since `Symbol::new` uppercases ASCII, the two round-trip for every
variant. `path()` says which connection carries the stream. A payload's symbol is a
`String` from its `s`, in the listing's case.

`Update::from_json(&str)` decodes one combined-stream envelope, failing with
`UsdmWsError::Frame`, and an `Update` serialises back to its envelope, so the CLI's JSON
output is the frame as sent, less the kline's `B`, which Binance documents as "Ignore".
`Payload` has one variant per stream kind plus `Unknown { event_type, raw }`, for an
object whose event type is not its stream's: kept whole rather than failing the frame,
and the live suite fails on one. Each payload carries `symbol_type: SymbolType` from `st`
(`Um`, `Cm`, `Other(u64)`) except the kline event, which has none; COIN-M rows do arrive
on this host (30 of 745 in one `!markPrice@arr@1s` frame, and `AAVEUSD_PERP` in
`!ticker@arr`). A mark-price row with no funding scheduled sends `T: 0` and
`r: "0.00000000"` (51 of 745 rows), so `next_funding_time: u64` reads 0 as none.

### WebSocket, bare tier

`UsdmWs::connect(path, streams)` connects to one path with a 10 s timeout and sends one
`SUBSCRIBE`. It implements `Stream<Item = Result<Update, UsdmWsError>>`, where
`Update { stream: StreamName, payload: Payload }` and `Payload` has one variant per stream
kind plus `Unknown { event_type, raw }`. Both are `#[non_exhaustive]`.

Methods on `&mut self`: `subscribe(&[StreamName])`, `unsubscribe(&[StreamName])`,
`list_subscriptions()`, `ping()`, `close()`. A stream for the other path is refused
(`WrongPath`), and so is a subscribe that would pass 1024 streams (`TooManyStreams`). In
both cases nothing is sent. Requests are batched at most 200 names each, sent no faster
than 5 per second (half the documented 10; of 40 sent back to back, 15 were answered
before the server closed the connection), and each waits for the answer carrying its `id`. Protocol pings are answered
while the caller polls, as in sports.

### WebSocket, supervised tier

```rust
let feed = UsdmWsBuilder::new()      // base url, ping_interval, stale_after, backoff,
    .streams([...])                  // max_connection_age, connect_timeout
    .connect()                       // eager; Err if a first connect fails
    .await?;                         // SupervisedUsdmWs: Stream<Item = Result<Event, UsdmWsError>>
let membership = feed.membership();  // taken before the stream is driven, as in perps
```

It owns one connection per path, each on its own task with the perps shape (a command
queue, because requests are sent). A path's connection opens when its first stream is
wanted and closes when its last leaves. `connect()` opens the paths its initial streams
need and fails if any of those first connects fails; with no initial streams it opens
nothing until the first `subscribe`. Each task:

- pings every `ping_interval` (default 20 s) on the wall clock, whatever the traffic,
  because a connection on quiet streams can go three minutes between server pings;
- treats `stale_after` (default 30 s) without any inbound frame, pongs included, as dead;
- answers server pings;
- on close, error or staleness yields `Disconnected { path, reason }` once, reconnects with
  backoff (500 ms doubling to 60 s, reset only after a connection that delivered a frame),
  replays that path's membership in batched, paced requests, then yields
  `Reconnected { path }`, once per outage however many attempts it takes;
- at `max_connection_age` (default 23 h 50 min, under the documented 24 h) yields
  `Disconnected { path, reason: Rotation }`, reconnects without backoff, and yields
  `Reconnected { path }`;
- sends every request through the path's one queue under the 5-per-second pacer;
- reports an error answer to the membership call that sent it. Code 4 ("Too many
  subscriptions") cannot happen, because the cap is enforced before sending.

`Event` is `Update(Box<Update>)` (boxed, as sports boxes its update, because the other
variants are small), `Disconnected { path, reason }` or `Reconnected { path }`, where
`DisconnectReason` is `Closed { code, reason }`, `Error(UsdmWsError)`, `Stale` or
`Rotation`. A consumer drops book state for that path on `Reconnected` and can show
staleness from `Disconnected`, which `polyoxide-perps` does not offer today. Its consumers
infer outages from silence instead.

Every `Disconnected { path }` is followed by `Reconnected { path }` while the client
runs. prader-rs folds per-path outages into one state on that invariant, so it holds even
when the path's last stream leaves during the outage: the task stops reconnecting and
yields `Reconnected`, since nothing on the path can then be stale. A path that closes
for want of streams while up yields neither. A fatal error, such as the server refusing
the replay after a reconnect, is yielded as `Err` and ends the stream.

`MembershipHandle::subscribe` and `unsubscribe` take `StreamName`s and route each to its
path. The membership is a set: subscribing a name twice holds it once, and reference
counting is the caller's. During a path's outage a change is recorded and answered `Ok`
at once, and the replay applies it. A change that opens a path waits for that path's first
connect: a refusal fails the call, and a transport failure is answered `Ok` and reported
as an outage like any other. A call fails only with `Refused`, `TooManyStreams` or
`Stopped`.

### CLI

```
polyoxide ws binance [--symbol BTCUSDT,币安人生USDT]
                     [--kind agg-trade,book-ticker,depth5,depth10,depth20,kline-1m,mark-price,ticker]
                     [--all-tickers] [--all-mark-prices]
                     [--format pretty|json] [-n COUNT] [-t DURATION]
```

- Reads `SupervisedUsdmWs`. Depth kinds stream at 100 ms; `kline-<interval>` takes any
  `Interval` spelling. `--symbol` and `--kind` are `Vec<String>` with
  `value_delimiter = ','`, per CLAUDE.md's clap note. Each symbol goes through
  `Symbol::new`, each kind through a parser naming the valid kinds.
- Updates go to stdout: in pretty mode one line each, and one per row of an array stream;
  in json mode the frame's envelope as compact JSON, one per line.
  `Disconnected` and `Reconnected` go to stderr. `-n` counts printed updates, `-t` exits 0
  when it elapses.
- Filtering and output live in `run_with(events, stdout, stderr)`, generic over any
  `Stream<Item = Result<Event, UsdmWsError>>`, as `ws sports` does.

### Spec docs

- `docs/specs/binance/INDEX.md`: hosts, the routes and streams covered, the crate, and
  that the directory is not a mirror (nothing is published to mirror).
- `docs/specs/binance/OBSERVED.md`: every measured fact in the venue contract above with
  its date, and the method (header deltas; probe scripts).
- `docs/specs/INDEX.md` gains the row; CLAUDE.md gains a Binance section (the socket rules
  and why the crate enforces them), the dependency graph and the publishing order;
  `SELF-HEALING.md` lists the host where it lists drift detectors;
  `.github/workflows/nightly-schema.yml`'s exclusion comment names Binance.

### Workspace wiring

| Place | Change |
|---|---|
| `Cargo.toml` | Member; `polyoxide-binance` in `[workspace.dependencies]` at the shared version; the workspace `reqwest` gains `gzip` |
| `polyoxide-core` | `HttpClientBuilder::gzip(bool)`, default `false`, so enabling the `reqwest` feature changes no other crate's requests |
| `polyoxide/Cargo.toml` | No change (decision above) |
| `polyoxide-cli` | Depends on `polyoxide-binance` with `ws` |
| `release.yml` and `finish_release.sh` | `CRATES` order core, rtds, sports, perps, binance, relay, gamma, data, clob, polyoxide |
| `nightly-behavioral.yml` | Rows for `live_api` and `live_ws --features ws` |
| CLAUDE.md, root `README.md` | Graph, publishing order, crate table row, the Binance and CLI paragraphs |

The release is 0.37.0. Nothing in it breaks an existing API.

## Testing

Every test names the bug it exists to catch. The plan breaks the code on purpose to show
each supervision and limiter test going red.

### Fixtures

`scripts/capture_binance_fixtures.py OUT_DIR` records one response per route (with
`exchangeInfo` trimmed to a crypto perpetual, a Chinese-character perpetual, a TradFi
perpetual, a `SETTLING` perpetual and a quarterly) and one combined-stream envelope per
stream kind (an array keeps a USDⓈ-M row with a scheduled funding time and, when the
frame has one, a COIN-M row; depth sides keep three levels), and writes `PROVENANCE.md`. It keeps every top-level key: the 2026-10-07 captures handed over with
this spec built `exchangeInfo` from chosen keys and dropped `futuresType`
(`"U_MARGINED"`), so the HTTP plan re-captures them.

### Types and limiter (unit)

- `Symbol`: Chinese-character symbols accepted and rendered unchanged in lowercase stream
  names; empty, space, punctuation and 33-character names refused.
- `StreamName`: `Display`/`FromStr` round trip, and `path()` for every constructor.
- `Filter`: an unseen filter type parses as `Other` and its `exchangeInfo` still parses.
- `WeightBudget`: charges block past the budget and release at the window boundary; a
  header above the local count raises it and one below does not lower it; a 429 cooldown
  only extends; a 418 holds every request; the funding bucket admits 450 per 5 minutes.
- `documented_weights`: the effective weight of every table row.

### Wire agreement (`tests/wire_agreement.rs`, `tests/ws_wire_agreement.rs`)

Every fixture deserialises, every declared field appears in at least one fixture, and
values survive a round trip where the wire form is preserved. A price with more digits
than an `f64` keeps survives exactly. `funding_info.json`'s `null` `updateTime` decodes
as `None`.

### Mock HTTP (`tests/mock_api.rs`, mockito)

Query encoding for every route; `{code, msg}` to `Venue`; 451 to `RegionBlocked`; 418 to
`IpBanned` and the next request waiting; a 429 with `Retry-After` to a cooldown then
success; a high `X-MBX-USED-WEIGHT-1M` delaying the next request; a gzip-encoded body
decoding.

### Supervision (`tests/supervision.rs`, scripted server, limits in hundreds of ms)

| Test | Bug it catches |
|---|---|
| A depth stream reaches `/public` and an aggTrade stream `/market` | One connection for both paths, which drops market data silently |
| The server sees at most the pacer's rate of requests during a 1000-stream replay | Bursting a resubscribe past Binance's limit, which closes the connection |
| No request carries more than 200 names | An oversized request |
| The 1025th stream is refused and nothing reaches the server | Sending it, which costs all 1024 |
| The server stops answering pings; `Disconnected { Stale }` then `Reconnected` | Staleness counting only data, or not enforced |
| The server sends only pings for three stale periods while pongs flow; no `Disconnected` | Dropping a healthy quiet connection |
| The server closes; updates, `Disconnected`, `Reconnected`, then resubscribed updates | Missing resubscribe after a reconnect |
| N refused connects then success: one `Disconnected`, one `Reconnected` | A marker per attempt |
| A short `max_connection_age` rotates with no backoff and both markers | Running into the 24 h cutoff unannounced |
| An error answer fails only the membership call that sent it | One bad request tearing down the path |
| A bad frame yields `Err(Frame)` and the next update arrives | One frame ending the stream |
| The last stream on a path leaves and that path's socket closes | A leaked connection |
| Dropping the stream closes every socket | A leaked task |
| A first connect refused makes `connect()` return `Err` | Silent retry when the setup is wrong |

### CLI

Parse tests for `--symbol` with a Chinese-character symbol and `--kind` lists; `run_with`
over a `futures::stream::iter` of events (filters, `-n`, markers on stderr only, json
lines parse); live `ws binance --all-mark-prices -n 1 --format json`.

### Live (`#[ignore]`, nightly)

- REST: every route, for `BTCUSDT`, a Chinese-character symbol and a TradFi symbol; an
  unknown symbol answers `Venue { code: -1121 }`.
- Socket: a frame of every stream kind on both paths within a bounded wait, phrased so a
  timeout "can legitimately time out" and classifies as environmental; a supervised
  connection held past `stale_after` with no `Disconnected`; a client ping answered.
- Drift: every stream frame and REST response in a window decodes with no unknown
  top-level keys; a new key fails the test and names it. This is the host's only drift
  detector.

### Gate

`cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
`cargo nextest run`, doctests, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
--all-features --workspace`, since a red doc build withholds the release tag.

## Open questions settled by evidence, not by this spec

- **The weight window** is the UTC clock minute, measured across two boundaries while
  planning.
- **The funding bucket** (500 per 5 minutes) is documented and unmeasured: those routes
  send no header.
- **The `/public` path's limits** are assumed equal to `/market`'s; only `/market` was
  probed.
- **The 418, 451 and 403 bodies** were not seen from this IP; classification uses the
  status alone.
- **Make-before-break rotation**, opening the new connection before closing the old,
  would remove the brief outage at rotation. It is deferred until a consumer needs it.
- **Klines at limits 500 to 1000, and depth at 500 and 1000,** were measured while
  planning; the table above has the results, which differ from the page at every klines
  band edge and for a request without `limit`.
