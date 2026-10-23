---
paths:
  - "polyoxide-polymarket/src/perps/**"
  - "polyoxide-polymarket/tests/*perps*.rs"
  - "polyoxide-polymarket/tests/fixtures/perps/**"
  - "polyoxide-polymarket/examples/info_soak.rs"
  - "scripts/capture_polymarket_perps*.py"
  - "docs/specs/perps/**"
---

# Polymarket perps

The `perps` module implements the 29 public `/v1/info/*` routes of `api.perpetuals.polymarket.com`
(`perps::Perps::new()`; namespaces `health()`, `exchange()`, `market()`, `public()`,
`competitions()`; its own builder knob is `with_rate_limiter`). Pending: credentials (perps has
its own `POLYMARKET-PROXY`/`POLYMARKET-SECRET` header auth, not the L1/L2 scheme), the
`/v1/account/*` reads, the signed `/v1/trade/*` routes, funds and BLP.
`docs/specs/perps/OBSERVED.md` records where the host and the schema part ways.

## The wire

- Klines, mark points and book levels are positional arrays, decoded through tuples of exact arity
  over `polyoxide_venue::positional::DecimalStr`.
- The host ignores `instrument_id` on `/v1/info/tickers` and `/v1/info/statistics` and returns
  every instrument, so a caller filters client-side.
- The four WebSocket fields on `LimitTier` are a `u32::MAX` sentinel, not a budget, and must not
  size a bucket.
- Every index has empty constituents today.
- The host is fronted by CloudFront; its measured rate rows are in `rate-limits.md`.
- Three test files hold it in place, on the pattern of Data v2: `tests/spec_agreement_perps.rs`
  (types, enums and query keys against `docs/specs/perps/openapi.json`, restricted to schemas
  reachable from `/v1/info/*`), `tests/wire_agreement_perps.rs` (against `tests/fixtures/perps/`,
  refreshed by `scripts/capture_polymarket_perps.py`) and `tests/live_perps.rs`. Wire-only fields
  are allowed through `OBSERVED_EXTRA` and recorded in `OBSERVED.md`.

## `perps::venue`

`MarketData`, `Instruments`, `Trades`, `Candles` and `Funding` on `Perps`, keyed by
`shared::keys::perps`.

- An instrument is its `/v1/info/instruments` row plus its type and category's `/v1/info/fees`
  entry (two requests), with sizes in `SizeUnit::Base` (one contract is one unit of the base asset,
  which `OBSERVED.md` shows from the fixtures) and no `tick_size`, since a non-integer price keeps
  five significant figures; its fee tiers, risk tiers, price bounds and decimals go in
  `InstrumentExt`.
- A quote is the `bbo` row: a side of zero quantity is none, and an instrument with no row an
  empty quote.
- The funding rate is the instrument's row of `tickers`, which the host sends for every
  instrument. Its `interval` is the instrument's `funding_interval` (`1h`, read as a whole number
  of minutes, hours or days; an unreadable one is a `Decode`), one more `/v1/info/instruments`
  request, which a `funding_history` with no rows skips.
- `trades` and `funding_history` reverse the host's newest-first pages. `funding_history` asks for
  the hourly settlements from `start`, at most 100 (`end_timestamp` is `start` + 100 h less a
  minute), since the host answers a start alone with the newest 100 rows since it.
- `tests/venue_perps.rs` holds each method to the native client's values on the captured
  fixtures.

## Sockets (`perps-ws`)

The six public WebSocket channels, `perps::ws`: `PerpsWs` (bare) and `PerpsWsBuilder`, whose
`connect` returns `SupervisedPerpsWs`, an alias for the kit's
`polyoxide_ws::Supervised<PerpsProtocol>`.

- The kit's `Supervisor` keeps the JSON `post/ping` on the wall clock regardless of traffic, and
  also while the 1024-event queue is full, without awaiting the pong. Staleness counts any text
  frame, a pong included, and a pong that is not `ok` reconnects. It reconnects with resubscribe,
  with a 10 s connect timeout (DRIFT R2).
- The host's own keep-alive is a protocol Ping every 10 s: its pings and the client's automatic
  pongs count as traffic, so a client that reads was never closed in 180 s, and only with the
  pongs withheld did it close, with `1011 pong_timeout` after about 70 s (probe of 2026-10-10; the
  documented 60 s idle close was not seen). Whether it also counts `post/ping` was not measured.
  The pongs go out only while the connection is read, so a supervised stream whose queue stays
  full that long may be closed with a 1011 and reconnect.
- Membership changes go through `membership()`, a `MembershipHandle<PerpsProtocol>`, whose
  `PerpsMembership` trait gives `subscribe` and `unsubscribe` (or `apply` with a
  `MembershipChange`). `PerpsProtocol` keeps perps' rows: no `Disconnected` (D8); a
  `message_rate_limited` subscribe retried within the change, and `SequenceRegressed` per
  connection (D10); `AlreadyClosed` stops the stream (D19); an empty membership stays wanted, so a
  reconnect replays nothing. Supervision logs under `polyoxide_ws`.
- `Channel` is the only way to name a subscription; payload structs carry the REST field names
  over the socket's terse keys.
- Four wire facts the AsyncAPI mirror gets wrong (`OBSERVED.md`): `tickers`/`statistics` data are
  objects and `::all` fans out per instrument; `ets` is on every frame; `sq` is a server-wide stamp,
  so only regressions are detectable; an unknown instrument subscribes without error.
- Offline tests drive the kit's scripted server with perps' answer through the `cfg(test)`
  adapter `src/perps/ws/scripted.rs`; perps exposes nothing under `test-server`.
  `tests/live_perps_ws.rs` is the live suite, and `tests/wire_agreement_perps_ws.rs` compares the
  payload types value-for-value against `tests/fixtures/perps/ws/` (captured by
  `scripts/capture_polymarket_perps_ws.py`, instrument 6 because `trades::1` was quiet).
