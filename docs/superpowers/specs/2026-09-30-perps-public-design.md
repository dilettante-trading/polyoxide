# Perps: public market data over HTTP and WebSocket

**Date:** 2026-09-30
**Branch:** `aidanb/perps-impl`
**Slice:** 1 of the perps rollout (public reads and public streaming). Credentials, signed
trading, private channels, funds, Python and CLI are later specs.
**Plans:** `docs/superpowers/plans/2026-09-30-perps-http.md` (HTTP, this slice's first
half); the WebSocket plan follows once the crate exists.

## Goal

Give polyoxide a `polyoxide-perps` crate that reads every public route on
`api.perpetuals.polymarket.com` and streams every public channel on
`ws.perpetuals.polymarket.com/v1/ws`, with no credentials, so every test in the crate can
run against the live host today.

## Scope

In:

- All 21 `GET /v1/info/*` routes.
- All six public WebSocket channels: `bbo`, `book`, `trades`, `klines`, `tickers`,
  `statistics`.
- A bare WebSocket stream and a supervised one (keep-alive, staleness watchdog,
  reconnect-with-resubscribe).
- A measured HTTP rate-limit table, soaked before it is written.
- `docs/specs/perps/OBSERVED.md` for every place the wire and the mirror disagree.

Out, each a later spec: proxy credentials (`POST /v1/account/proxy`), the
header-authenticated `/v1/account/*` reads, every signed `/v1/trade/*` route, withdraw and
transfers, BLP, private WebSocket channels and WebSocket trading, the WebSocket budget
soak, Python bindings, CLI.

## Decisions taken during brainstorming

| Question | Decision | Why |
|---|---|---|
| First outcome | Streaming market data; orders later | The consumer is a feed before it is a trader |
| Account reads with caller-supplied credentials | No, public only | Credentials need the EOA-signed `CreateProxy` flow, which is slice 2 |
| WebSocket tiers | Bare and supervised, as in `polyoxide-rtds` | The 60 s idle close means a bare stream dies on a quiet instrument |
| Channel coverage | All six public channels | Plumbing is shared; the marginal cost is fixture capture |
| HTTP coverage | All 21 `/v1/info/*` routes | Lets the parity audit mark the group done |
| Rate-limit table | Soak first, then write the table | Nothing is published for public routes; a guess is a guess |
| Crate shape | One crate, WebSocket behind `ws`, signing later behind `trading` | REST already costs `core`; a feature that is off costs nothing |

Rejected: a separate `polyoxide-perps-ws` crate (REST and WS share vocabulary, so a split
needs a third types crate), and hosting the channels in `polyoxide-rtds` (different host,
envelope and keep-alive contract).

## Venue contract

Sources: `docs/specs/perps/openapi.json` (mirror of
`docs.polymarket.com/api-spec/perps-openapi.json`), `docs/specs/perps/asyncapi.json`
(mirror of `docs.polymarket.com/asyncapi-perps.json`), and the prose pages
`docs.polymarket.com/perps/{market-data,realtime-updates,errors}.md`. Where the wire
disagrees with all of them, the wire wins and `OBSERVED.md` records it.

### HTTP

Base URL `https://api.perpetuals.polymarket.com`. All 21 routes are unauthenticated `GET`.
Timestamps are Unix milliseconds. Prices and quantities are decimal strings. Instrument ids
are integers.

| Route | Required | Optional | Response |
|---|---|---|---|
| `/v1/info/ping` | | | object |
| `/v1/info/time` | | | `Time` |
| `/v1/info/exchange` | | | `Exchange` |
| `/v1/info/assets` | | | `[Asset]` |
| `/v1/info/instruments` | | `instrument_id`, `instrument_type`, `category` | `[Instrument]` |
| `/v1/info/tickers` | | `instrument_id` | `[Ticker]` |
| `/v1/info/statistics` | | `instrument_id` | `[Statistic]` |
| `/v1/info/exchange-stats` | `start_timestamp`, `end_timestamp` | | `ExchangeStatistics` |
| `/v1/info/klines` | `instrument_id`, `interval`, `start_timestamp` | `end_timestamp` | `KlinesResponse` |
| `/v1/info/mark-history` | `instrument_id`, `interval`, `start_timestamp` | `end_timestamp` | `MarkHistoryResponse` |
| `/v1/info/bbo` | | `instrument_id` | `[BBO]` |
| `/v1/info/book` | `instrument_id` | `depth` | `Book` |
| `/v1/info/index` | `asset` | | `Index` |
| `/v1/info/trades` | `instrument_id` | `start_timestamp`, `end_timestamp` | `Trades` |
| `/v1/info/portfolio` | `address` | | `PublicPortfolio` |
| `/v1/info/position-fills` | `address`, `instrument_id` | `cursor`, `sort` | `AccountTrades` |
| `/v1/info/funding` | `instrument_id` | `start_timestamp`, `end_timestamp` | `FundingHistory` |
| `/v1/info/fees` | | | `FeesInfo` |
| `/v1/info/limit-tiers` | | | `[LimitTier]` |
| `/v1/info/invite` | `code` | `address` | `InviteCheckResponse` |
| `/v1/info/leaderboard` | | `window`, `sort_by`, `limit`, `offset`, `address` | `Leaderboard` |

Errors are `{ "status": "err", "error": "<snake_case identifier>" }`. The identifier is a
stable part of the contract. A 429 carries `Retry-After` in whole seconds and one of
`ip_rate_limited`, `action_rate_limited`, `open_orders_limit`; only the first can occur on
public routes. No numeric quota is published for public routes.

### WebSocket

One multiplexed connection to `wss://ws.perpetuals.polymarket.com/v1/ws`. Requests are
`{ "id": <int>, "req": "sub" | "unsub" | "post", ... }`; `sub` and `unsub` carry
`"chs": [<channel name>]`, `post` carries `"op": { "type": "ping" }`. Responses echo `id`
and carry `data`: for `sub`/`unsub` an array with one `{status: "ok"}` or
`{status: "err", error}` per requested channel, for `ping` an object with `status`, `ts`,
`sq`. The server closes a connection after 60 seconds without an inbound message.
Inbound requests consume a weighted budget whose weights and window are not published.

Channel names, from the AsyncAPI patterns:

| Channel | Name |
|---|---|
| Best bid and offer | `bbo::{iid}` |
| Book, top 20 or 50 per side | `book::{iid}` or `book::{iid}::20` or `book::{iid}::50` |
| Trades | `trades::{iid}` |
| Klines | `klines::{iid}::{1s|1m|5m|15m|30m|1h|4h|6h|12h|1d|1w}` |
| Tickers | `tickers::{iid}` or `tickers::all` |
| 24 h statistics | `statistics::{iid}` |

Push frames are `{ "ch", "ts", "sq", "data" }`; `book` adds `ets`, where `0` means no
event horizon is attested. Frame payloads use terse keys (`iid`, `bp`, `bq`, `ap`, `aq`,
`b`, `a`, `tid`, `p`, `qty`), unlike the REST responses, which use long snake_case. The
`trades` payload is declared `object` but described as an array; the fixtures settle it.

## Design

### Crate layout

```
polyoxide-perps/
  Cargo.toml         features: ws (tokio-tungstenite, futures-util, rustls/ring)
  README.md          a doctest, like the other crate READMEs
  src/
    lib.rs
    client.rs        Perps, PerpsBuilder
    error.rs         PerpsError, PerpsWsError (ws)
    types.rs         InstrumentId, Interval, BookDepth, Side, enums, REST rows
    api/
      health.rs      ping, time
      exchange.rs    exchange, assets, instruments, fees, limit_tiers
      market.rs      tickers, statistics, exchange_stats, klines, mark_history,
                     bbo, book, index, trades, funding
      public.rs      portfolio, position_fills, leaderboard, invite
    ws/              feature = "ws"
      mod.rs
      channel.rs     Channel: Display + FromStr
      event.rs       Event and the six payload types
      client.rs      PerpsWs (bare tier)
      supervised.rs  PerpsWsBuilder, SupervisedPerpsWs, membership handle
      test_server.rs local tokio-tungstenite server for tests
  examples/
    info_soak.rs
  tests/
    spec_agreement.rs
    wire_agreement.rs
    mock_api.rs
    live_api.rs
    live_ws.rs       feature = "ws"
    fixtures/
```

### Client

`Perps::builder().build()?` with `base_url`, `timeout_ms`, `pool_size`, `with_rate_limiter`,
`with_retry_config`, `with_max_concurrent`, mirroring `DataApiBuilder`. No credentials.
Four namespaces, each a struct holding a cloned `HttpClient`:

- `perps.health()`: `ping()`, `time()`.
- `perps.exchange()`: `exchange()`, `assets()`, `instruments()`, `fees()`, `limit_tiers()`.
- `perps.market()`: `tickers()`, `statistics()`, `exchange_stats(start, end)`,
  `klines(iid, interval, start)`, `mark_history(iid, interval, start)`, `bbo()`,
  `book(iid)`, `index(asset)`, `trades(iid)`, `funding(iid)`.
- `perps.public()`: `portfolio(address)`, `position_fills(address, iid)`, `leaderboard()`,
  `invite(code)`.

Every route is a request builder ending in `.send().await?`. Required query parameters are
constructor arguments; optional ones are chained methods. Rate-limit paths are keyed by
route so `RateLimiter::perps_default()` can match them.

### Types

- `InstrumentId(u64)` newtype. Used by REST params, `Channel` and, later, signed ops.
- `Interval`: the 11 kline intervals, serialising to the exact wire spelling, used by both
  the REST `interval` parameter and the `klines` channel suffix.
- `BookDepth { Ten, Hundred, FiveHundred, Thousand }` for REST `depth` (the schema's enum is
  10/100/500/1000, default 100). The socket accepts 20 or 50 and gets its own enum in the
  WebSocket slice; the two sets do not overlap, so they are not one type.
- `Side { Long, Short }`, `InstrumentType`, `LeaderboardWindow`, `LeaderboardSort`, and any
  other closed set the schema enumerates. Each is checked against the schema enum.
- REST rows are structs named after their schema, `Decimal` with string serde for prices
  and quantities, `u64` for timestamps. `#[non_exhaustive]` where the schema is open.
- WS payloads are separate structs with `#[serde(rename = "...")]` from the terse keys to
  full names. REST and WS share the vocabulary types above, not the row structs.

### WebSocket, bare tier

`PerpsWs::connect(channels: &[Channel])` installs the rustls provider (own copy of
`ensure_crypto_provider`, not shared with rtds or clob), opens the socket, sends one `sub`
frame, awaits the response with the matching `id`, and returns `Err(PerpsWsError::Subscribe)`
carrying every `err` entry with its identifier if any channel was refused. It implements
`Stream<Item = Result<Event, PerpsWsError>>`.

Methods on `&mut self`: `subscribe_more(&[Channel])`, `unsubscribe(&[Channel])`, `ping()`,
`close()`, `subscriptions() -> &[Channel]`. Each control frame gets a fresh `id` from a
counter and its response is awaited before the method returns.

`Event { channel: Channel, ts: u64, sq: u64, payload: Payload }` with
`Payload::{Bbo, Book, Trades, Kline, Ticker, Statistics, Unknown { raw }}`.
`Book` carries `ets: Option<u64>`, `None` when the wire says `0`. A frame whose `ch` parses
but whose payload does not deserialise is `Err(PerpsWsError::Frame { channel, raw })`; a
frame whose `ch` is not a known channel is `Payload::Unknown` and the stream continues.
`Event` and `Payload` are `#[non_exhaustive]`.

`sq` is exposed and not enforced in this tier.

### WebSocket, supervised tier

`PerpsWsBuilder::new().channels(...).ping_interval(20 s).stale_after(d).backoff(...)`
builds `SupervisedPerpsWs`, which owns the subscription set and runs the socket on a task.
The task:

- sends a `ping` op every `ping_interval`, default 20 seconds against the 60-second close;
- treats `stale_after` without any inbound frame as a dead connection;
- on close, error or staleness, reconnects with exponential backoff and replays the full
  subscription set on the new socket;
- serialises all control frames (sub, unsub, ping) through one queue;
- retries a `sub` refused with a rate-limit identifier with backoff instead of failing;
- tracks the last `sq` per channel and emits `Event::Gap { channel, expected, got }` when
  a frame arrives out of order.

It exposes the same `Stream` plus `Event::Reconnected` after each successful reconnect, so
a book consumer discards state there. A `MembershipHandle`, taken before the stream is
driven, adds and removes channels while running, following clob's handle.

### Rate limiting

`RateLimiter::perps_default()` in `polyoxide-core/src/rate_limit.rs`, one row per route
group, with values measured by `examples/info_soak.rs` and reserved by a tenth through the
existing `quota()` helper. The soak sends raw requests at a caller-chosen rate over distinct
URLs (the host is fronted by CloudFront, `x-cache: Hit from cloudfront`, so a repeated URL
never reaches the origin) and, in validation mode, detects throttling through a `tracing`
subscriber watching for the retry loop's `WARN`, not through `Ok` against `Err`. Routes
soaked: `klines`, `trades`, `portfolio`, `bbo`. `instruments` cannot be soaked: it has
88 × 5 distinct parameterisations and the CDN answers the rest. The runs and the resulting table go in
`OBSERVED.md`. The `documented_perps_limits` agreement test asserts the effective quota
each route resolves to, not merely that a row exists.

The perps 429 flows through the existing cooldown: `note_rate_limited` before
`should_retry`, `Retry-After` may only extend the wait.

The WebSocket budget is not soaked in this slice; the supervised tier's single control
queue and sub-retry are the only mitigation, and `OBSERVED.md` records the gap.

### Errors

`PerpsError` wraps `ApiError` through `impl_api_error_conversions!`, so `is_retriable()`
and the retry loop apply. `PerpsError::Venue { status, code: String, retry_after: Option<u64> }`
is recognised by body shape (`status == "err"` with an `error` string), on the pattern of
`DataApiError::V2`. `code()` returns the identifier. The identifier stays a string because
the catalogue on the errors page is long and grows.

`PerpsWsError`: `Connect`, `Subscribe { refused: Vec<(Channel, String)> }`,
`Frame { channel, raw }`, `Stale`, `ReconnectExhausted`, `Closed`.

### Testing

1. `tests/spec_agreement.rs`: every REST row's field names and optionality against
   `openapi.json`; every builder's query keys against the route's parameters; every enum's
   variants against the schema enum; every WS payload against `asyncapi.json`; every
   `Channel` rendering against the AsyncAPI name patterns. `Channel` round-trips through
   `Display` and `FromStr` under a property test.
2. `tests/wire_agreement.rs` against `tests/fixtures/`, refreshed by
   `scripts/capture_perps_fixtures.py` (one response per route, a bounded number of frames
   per channel). Both directions: every fixture deserialises, and every declared field
   appears in at least one fixture. Disagreements with the schema go in `OBSERVED.md` and
   the type follows the wire.
3. `tests/mock_api.rs` with `mockito`: query encoding, `Venue` mapping, 429 cooldown. A
   local `tokio-tungstenite` server (copied from `polyoxide-rtds/src/test_server.rs`) for
   the subscribe handshake, `err` entries, ping cadence, gap detection and
   reconnect-with-resubscribe.
4. `tests/live_api.rs`, `tests/live_ws.rs`, `#[ignore]`d, no credentials. The WS test
   selects an instrument that is currently trading before asserting on frames and uses the
   `legitimately time out` phrasing so a quiet market classifies as environmental.

### Workspace wiring

- `Cargo.toml`: member and `[workspace.dependencies]` entry at the shared version.
- `polyoxide/Cargo.toml`: `perps = ["dep:polyoxide-perps"]`,
  `perps-ws = ["perps", "polyoxide-perps/ws"]`; both in `full`, neither in default.
- `release.yml`: `polyoxide-perps` in `CRATES` after `polyoxide-core`, before `polyoxide`.
  Publishing order in CLAUDE.md and the workflow comment updated.
- `nightly-behavioral.yml`: rows for `live_api` and `live_ws --features ws`.
- Docs: `docs/specs/perps/INDEX.md` drops the "not implemented" banner and links
  `OBSERVED.md`; `docs/specs/INDEX.md` and CLAUDE.md (workspace diagram, the not-yet-
  implemented paragraph, WebSocket notes, publishing order) updated.

## Wire findings from the 2026-09-30 probes

Recorded in `docs/specs/perps/OBSERVED.md` by the plan. `Instrument` carries
`display_symbol`, `close_only` and `logo`; `TradeData` carries `settlement`; `LimitTier`
carries four WebSocket-budget fields; none is in the schema, and each is modelled as an
`Option` and allowed through an `OBSERVED_EXTRA` list in the spec-agreement test. A 400
body carries `arts`, `ts` and `ref` (a gateway trace id, exposed as `VenueError::reference`).
An unknown instrument on `/v1/info/book` is a 200 with empty sides. `open_interest` on
`exchange-stats` was captured with 18 fractional places, within `Decimal`'s 28.

## Open questions settled by fixtures, not by this spec

- Whether `trades.data` is one object or an array.
- Whether `tickers::all` frames carry one instrument per frame or a batch.
- What the `ping` op response looks like on the wire against the AsyncAPI `Base Response`.
- The numeric rate-limit rows.
