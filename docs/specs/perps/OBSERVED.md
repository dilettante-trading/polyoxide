# Perps API — observed behaviour

Where the live host disagrees with, or goes beyond, [openapi.json](openapi.json).
The mirror stays byte-faithful so the nightly drift check can compare it with
upstream; what the server actually does is recorded here instead.

Each entry names its evidence. Re-check before relying on an old one.

## Fields on the wire that the schema omits

Captured 2026-09-30 (`polyoxide-perps/tests/fixtures/`, see `PROVENANCE.md`
there). Each is modelled as an `Option` and allowed through `OBSERVED_EXTRA`
in `polyoxide-perps/tests/spec_agreement.rs`.

| Schema | Field | Seen as | Note |
|--------|-------|---------|------|
| `Instrument` | `display_symbol` | `"USA500-USD"` | on 2 of 88 rows |
| `Instrument` | `close_only` | `false` | on every row |
| `Instrument` | `logo` | `{"light": URL, "dark": URL}` | on 84 of 88 rows; an object, one SVG per colour scheme, never a bare URL |
| `TradeData` | `settlement` | `false` | on every row |
| `AccountTradeData` | `builder_fee` | `"0"` | on every row |
| `AccountTradeData` | `total_fee` | decimal string | on every row; equal to `fee` when `builder_fee` is `0` |
| `AccountTradeData` | `settlement` | `false` | on every row |
| `LimitTier` | `connects_per_minute_limit` | `4294967295` on every tier | WebSocket budget name |
| `LimitTier` | `max_connections` | `4294967295` on every tier | WebSocket budget name |
| `LimitTier` | `ws_messages_burst_limit` | `4294967295` on every tier | WebSocket budget name |
| `LimitTier` | `ws_messages_per_minute_limit` | `4294967295` on every tier | WebSocket budget name |

The four `LimitTier` extras are `u32::MAX` on all four tiers in
`limit_tiers.json`: a sentinel (unset or unlimited), not a budget. They must
not be used to size a client bucket; the WebSocket budget remains unpublished.

## `tickers` and `statistics` ignore `instrument_id`

The schema documents an `instrument_id` query parameter on `GET /v1/info/tickers`
and `GET /v1/info/statistics`. On 2026-09-30 both routes answered every request
with all 88 instruments: `?instrument_id=1`, `?instrument_id=2`, no parameter,
`?instrument_id=1&instrument_id=2` and even `?instrument_id=abc` all returned
200 with 88 rows and no validation error. The fixtures `tickers.json` and
`statistics.json`, captured with `?instrument_id=1`, hold 88 rows each.
`/v1/info/bbo` and `/v1/info/instruments` honour the same parameter (one row
for `?instrument_id=2`). The builders still send it, since it is documented;
callers must filter the response themselves until the host does.

## Every index has empty constituents

On 2026-09-30 `GET /v1/info/index?asset=…` answered 200 with
`"constituents": []` for all 88 listed base assets (the capture script probes
each in turn, preferring one with constituents, and fell through to `SP500`).
`IndexConstituent` is therefore held to the schema by `spec_agreement.rs` but
has never been seen on the wire; the first capture that carries one should be
checked by hand before `wire_agreement.rs` is trusted on it.

## Error bodies carry more than `{status, error}`

A 400 (`GET /v1/info/book` with no `instrument_id`, 2026-09-30):

```json
{"status":"err","error":"invalid query parameters: missing field `instrument_id`","arts":1790762375318,"ts":1790762375318,"ref":"g-5928ce9c74b68"}
```

`ref` is a gateway trace id; `VenueError::reference` carries it. A 404
(`GET /v1/info/nope`) is the bare `{"status":"err","error":"not_found"}`. On a
400 `error` is a human-readable message, as the spec says, not an identifier.
Both error responses carry `x-cache: Error from cloudfront` and no
`cache-control`.

## An unknown instrument is an empty book, not a 404

`GET /v1/info/book?instrument_id=999999` answers 200 with
`{"instrument_id":999999,"bids":[],"asks":[],"timestamp":…,"sequence":…}`
(2026-09-30). Callers cannot tell "no liquidity" from "no such instrument" on
this route; use `/v1/info/instruments` for existence.

## Responses are served through CloudFront

Every response carries `x-cache: Hit from cloudfront` or `Miss from cloudfront`
and a `cache-control` of `public, max-age=0` (`instruments`) or
`public, max-age=1, must-revalidate` (`book`), observed 2026-09-30. A repeated
URL can be answered without reaching the origin, so a rate-limit soak must vary
the URL (`examples/info_soak.rs`, added by the rate-limit task), and
`/v1/info/instruments` cannot be soaked
at all: it has only 88 × 5 distinct parameterisations.

## Long decimals and `Decimal` precision

The longest values in the 2026-09-30 capture have 28 fractional places:
`"previous_entry_price":"4.9027684994968432815256990529"` on
`/v1/info/position-fills` (29 significant digits) and
`"return_on_equity":"-0.3510891034606133314543425132"` on `/v1/info/portfolio`
(28). `rust_decimal::Decimal` holds up to 28 fractional places on a 96-bit
mantissa, so each fits because its mantissa is below 2^96, and both round-trip
byte for byte. `/v1/info/portfolio` also sends `unrealized_pnl` with 25
fractional places and `/v1/info/exchange-stats` sends `open_interest` with 18.
A longer fraction would be rounded on decode, since the crate's string serde
uses `Decimal::from_str`, not `from_str_exact`. `wire_agreement.rs` compares
values as well as key paths, so a rounded or overflowed value fails it.

## Leaderboard `account` is request-dependent

`account` appears only when the request names an `address`; otherwise the key
is absent rather than `null`. Modelled as `Option<LeaderboardAccount>`. The
captured account was ranked, so `rank` was present; the schema says it is
absent for an unranked account.

## Position-fills `cursor` is omitted on the last page

`GET /v1/info/position-fills` with 39 fills and `"more": false` carried no
`cursor` key at all (2026-09-30). Modelled as `Option<String>`.

## Rate limits

Nothing numeric is published for the public routes. Measured with
`polyoxide-perps/examples/info_soak.rs` on 2026-09-30, one route per run, 60 s
stages, 120 s cooldowns, 8 in-flight, distinct URLs throughout. The default
ramp (5, 10, 15, 20, 30 req/s) was throttled at its first stage on three
routes, so those were re-run at 1, 2, 3, 4 req/s as the harness prescribes;
both runs are listed.

| Route | Stages (req/s → verdict) | Pinned (per 10 s) |
|-------|--------------------------|-------------------|
| `/v1/info/klines` | 5.0 → Throttled after 8.2 s; re-run: 1.0 → Clean, 2.0 → Clean, 3.0 → Clean, 4.0 → Throttled after 16.6 s | 30 |
| `/v1/info/trades` | 5.0 → Throttled after 2.4 s; re-run: 1.0 → Clean, 2.0 → Throttled after 16.9 s | 10 |
| `/v1/info/portfolio` | 5.0 → Throttled after 9.6 s; re-run: 1.0 → Clean, 2.0 → Clean, 3.0 → Clean, 4.0 → Throttled after 26.4 s | 30 |
| `/v1/info/bbo` | 5.0 → Clean, 10.0 → Throttled after 20.8 s | 50 |

Verbatim `pin` lines from the runs:

```
klines: pin 30 per 10s ([(1.0, Clean), (2.0, Clean), (3.0, Clean), (4.0, Throttled { after: 16.622050001s, code: "ip_rate_limited" })])
trades: pin 10 per 10s ([(1.0, Clean), (2.0, Throttled { after: 16.870869722s, code: "ip_rate_limited" })])
portfolio: pin 30 per 10s ([(1.0, Clean), (2.0, Clean), (3.0, Clean), (4.0, Throttled { after: 26.412322897s, code: "ip_rate_limited" })])
bbo: pin 50 per 10s ([(5.0, Clean), (10.0, Throttled { after: 20.772812787s, code: "ip_rate_limited" })])
```

After the first 429 the remaining workers went on receiving origin replies
at a declining rate (klines at 4 req/s: 164 origin, 6 throttled, last reply
at 42.8 s of 60; trades at 2 req/s: 99 origin, 3 throttled; bbo at 10 req/s:
369 origin, 5 throttled). The harness stops each worker at its first 429, so
a stage can record at most 8 throttles and the counts say nothing about what
share of requests the host would refuse under sustained overload. The
budgets differ per route, so each soaked route has its own row; the mixed
validation runs below show the budget is also partly shared, which is what
the general bucket models. Every route answered from the origin on distinct URLs
(`x-cache: Miss from cloudfront`); bbo, whose URL space is one per
instrument, saw 4 and 3 cache hits per 300-plus replies.

Validation (`--route all --pace client`, 120 s, 4 tasks per route on one
client, so at most 4 requests in flight overall, the client's default) was
run twice on 2026-09-30, and the two runs are what set the general bucket:

| General bucket | Result |
|----------------|--------|
| 50 per 10 s (the most permissive route's row) | 487 requests, **11 throttled** |
| 30 per 10 s | 327 requests, 0 throttled |

So the per-IP budget is partly shared across routes: each route sustains its
own row alone, but a mix capped at bbo's rate is refused. 30 was the first
cap tried below 50 and was clean; 40 was not run, so 30 is a clean point, not
the highest clean cap. A 429 body seen
during the ramps was `{"status":"err","error":"ip_rate_limited"}` with
`Retry-After: 1`.

**How the table is modelled** (`polymarket::perps_limits`): one row per
soaked route at its pinned count, a `/v1/info` catch-all at the lowest
pinned count (10 per 10 s) for the 17 routes that were not soaked, and a
general bucket of 30 per 10 s that caps the whole client, set by the
validation runs above. `WindowQuotaTable` reserves a tenth of every row.

The WebSocket budget was not soaked and is unpublished; the four WebSocket
fields on `LimitTier` are a sentinel (see the wire-only fields above) and
must not be used to size a bucket.

## WebSocket: where the AsyncAPI and the wire part ways

Captured 2026-09-30 (`polyoxide-perps/tests/fixtures/ws/`, instrument 6, and a
20 s run of 18,326 frames on instrument 1 during design). Every entry is what
`polyoxide-perps`'s `ws` module implements. The fixtures are instrument 6
rather than 1 because `trades::1` stayed quiet for two 20 s windows;
instrument 6 had the largest 24-hour `volume` on `GET /v1/info/statistics`
that day and pushed a trade within 4 s.

- **`data` types.** `trades` and `klines` carry an array; `tickers` and
  `statistics` carry one object per frame. The mirror types `trades` as an
  object and gives array examples for `tickers`/`statistics`.
- **`::all` fans out.** Subscribing `tickers::all` or `statistics::all`
  delivers one frame per instrument labelled `tickers::N`/`statistics::N`
  (174 distinct labels across the two kinds in the 20 s fixture window on
  2026-09-30); no frame ever carries an `::all` label. `Channel::covers`
  relates a subscription to the labels it produces.
- **`ets` is on every push frame**, not only `book`. `0` means unattested and
  is read as `None`.
- **`sq` is a server-wide stamp**, identical across every frame of one
  server batch and non-decreasing per channel. In the fixtures `book::6`,
  `book::6::50` and `tickers::6` share `sq` `59134237518` and `ts`
  `1790781005102`; the design run saw the same on instrument 1
  (`59023555830`). It is not contiguous per channel, so gaps cannot be
  counted; the supervised tier reports only a regression
  (`Event::SequenceRegressed`).
- **Subscribe responses** list one `{status}` per requested channel in
  request order. An unknown instrument (`bbo::999999`) is `ok`; only a
  malformed name (`nonsense::1`, `book::1::30`) is `err` with
  `invalid channel`. The mirror's response example is an empty array.
- **`book::1` and `book::1::20` are one channel**; frames are labelled
  `book::1`. `book::1::50` is a separate channel with its own frames.
- **Ping** is `{"req":"post","op":{"type":"ping"}}` and the reply is
  `{"id","ts","data":{"status":"ok","ts","sq"}}`. Without any inbound message
  the server closes the socket after 60 s, as documented.
- **Rates observed:** bbo, book and tickers about 10 frames/s per channel;
  statistics and klines one frame per 4 s; a `klines` frame may carry an
  empty `data` array while a candle is open (the first two captures on
  instrument 1 saw only empty ones, so the capture script keeps the first
  non-empty `klines` frame when one arrives).
- **`trades` entries carry `settlement`**, as the REST route does.
- **Unsubscribe takes effect at the acknowledgement.** In
  `tests/live_ws.rs` on 2026-09-30, `tickers::all` was unsubscribed while
  nothing consumed the stream; 1,055 fan-out frames were then yielded, every
  one of them received before the acknowledgement, the last of them 1.4 ms
  after it, and no ticker arrived in the following 5 s. A test that counts
  frames after a fixed sleep cannot distinguish frames received before the
  acknowledgement from a server that ignored the request; the live test
  drains the stream while the change is pending and checks when the last
  ticker arrived instead. Draining, the second run yielded 126 tickers
  before the acknowledgement and 994 after it (the pushes the bare tier
  parks while awaiting the reply), the last 1.3 ms after it, and none in
  the following 5 s.
- **Budget.** Each inbound request costs from a weighted per-IP budget the
  AsyncAPI does not quantify; a breach answers `message_rate_limited` and
  leaves the socket open. Not soaked. `LimitTier`'s WebSocket fields are a
  sentinel (see above). The supervised tier retries a rate-limited subscribe
  three times with backoff.
