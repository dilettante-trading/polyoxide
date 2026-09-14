# Data API v2 — observed behaviour

Where the live host disagrees with, or goes beyond, [openapi.json](openapi.json).
The mirror stays byte-faithful so the nightly drift check can compare it with
upstream; what the server actually does is recorded here instead.

Each entry names its evidence. Re-check before relying on an old one.

## `/v2/user-pnl` is not the `user-pnl-api` series

The spec describes `trade_pnl` as "the compatibility chart series (`p` on the
bare user-pnl route)", which suggests `/v2/user-pnl` can replace
`data.pnl()`. It cannot. Same wallets, `interval=1w`, `fidelity=1d`, joined on
timestamp, 2026-09-14 (`scripts/compare_v2_overlaps.py`, plus one wallet checked by
hand during design):

| wallet | points joined | `trade_pnl` vs `p` |
|--------|---------------|--------------------|
| `0x3048d65321be3497164cdfc2996f94f98a2e7537` | 7 | −0.55% to −0.42% |
| `0x51698a47f840a242abc2ca0351371c7ffac41842` | 6 | +0.002% to +0.010% |
| `0xf0318c32136c2db7fec88b84869aee6a1106c80c` | 7 | +1.1% to +5.2% |
| `0x168a51f1cac0ad3b166797069382495a8aa10d24` | 7 | +0.003% to +62.7% |

The gap has no fixed size or sign. `user-pnl-api` also appends one off-grid point
for the current moment on every wallet; `/v2/user-pnl` does not.

**Consequence:** `PnlApi` documents `/v2/user-pnl` as a different series, not a
replacement.

## `/v2/leaderboard` volume is shares; `lb-api` volume is USDC

The spec documents v2 `volume` as "both-sides traded volume in shares; never USD".
`lb-api`'s `amount` is USDC. For the top three of the weekly v2 volume board on
2026-09-14, against `lb-api` `/volume?window=7d`:

| wallet | v2 `volume` | lb-api `amount` | ratio |
|--------|-------------|-----------------|-------|
| `0x2005d16a84ceefa912d4e380cd32e7ff827875ea` | 47,835,766 | 9,227,949 | 5.18 |
| `0xfe787d2da716d60e8acff57fb87eb13cd4d10319` | 46,890,990 | 8,159,351 | 5.75 |
| `0x821dab0565ebf5b327f51db06223fdcfe01acf16` | 42,731,398 | 9,573,469 | 4.46 |

The windows also differ: v2 takes `day`/`week`/`month`/`all`, `lb-api` takes
trailing `1d`/`7d`/`30d`/`all`.

**Consequence:** `RankingsApi` documents `/v2/leaderboard` as a different
ranking, not a replacement.

## Required parameters the schema marks optional

Every query parameter in the schema is optional and nullable. The server
enforces these anyway (2026-09-14):

| Request | Response |
|---------|----------|
| `GET /v2/user-pnl` | `400` `required query param 'user' not provided`, `parameter: "user"` |
| `GET /v2/positions` | `400` `required query param 'user' or 'condition' not provided` |
| `GET /v2/resolutions?event_id=1&condition=0x1` | `400` `provide exactly one of 'question_id', 'condition', or 'event_id'` |

**Consequence:** these are method arguments in `polyoxide-data` (`user_pnl(user)`,
`positions(PositionAnchor)`, `resolutions(ResolutionKey)`), not setters.

## Responses are cached at the CDN, per route

Responses pass through CloudFront. Some routes set `cache-control: public`, and a
repeated identical URL is then answered from the cache with the **same
`x-trace-id`** without reaching the origin (2026-09-14, three `GET /v2/oi` one
second apart: `x-cache: Hit from cloudfront`, `age` 8, 10, 11, one trace id):

| Route | `cache-control` |
|-------|-----------------|
| `/v2/trades` | `public, max-age=300` |
| `/v2/oi` | `public, max-age=300` |
| `/v2/activity` | `public, max-age=15` |
| `/v2/positions` | `public, max-age=5` |
| `/v2/user-pnl`, `/v2/leaderboard`, `/v2/status` | none |

**Consequences:** the first page of a walk can lag the feed by up to the route's
`max-age` (later pages have unique cursor URLs). A rate-limit soak that repeats a
URL measures the cache, not the origin's allowance, so it must never repeat one
within the window.

## Validation messages enumerate valid values

Most enum-like parameters are typed as a bare `string`. The server's `400`
message lists the accepted values, which is where `polyoxide-data`'s enums come
from (2026-09-14):

| Parameter | Message |
|-----------|---------|
| `positions` `sort_by` | `invalid position sortBy: NOPE` (values from the spec prose) |
| `positions/combos` `sort_by` | `sortBy must be one of FIRST_ENTRY, ENTRY_COST, CURRENT_VALUE, UPDATED` |
| `leaderboard` `time_period` | `time_period must be one of day, week, month, all` |
| `leaderboard` `sort_by` | `sort_by must be PNL or VOLUME` |
| `user-pnl` `interval` | `interval must be one of max, all, 1m, 1w, 1d, 12h, 6h` |
| `user-pnl` `fidelity` | `fidelity must be one of 1d, 18h, 12h, 3h, 1h` |
| `prices-history` `interval` | `interval must be one of max, all, 1m, 1w, 1d, 6h, 1h` |
| `builders/volume` `interval` | `time_period must be one of day, week, month, all` |
| `trades` `side` | `side must be BUY or SELL` |
| `trades`/`positions` `filter_type` | `filterType must be CASH or TOKENS` |

`leaderboard` `category` accepts any string without error. All thirteen activity
types (`TRADE` … `TAKER_REBATE`, `TIP`) are accepted by `/v2/activity?type=`.

## `/v2/activity`'s sort error message is stale

`sort_by=NOPE` answers `only sortBy=TIMESTAMP with sortDirection=DESC is
supported`, but `sort_direction=ASC` is accepted and returns ascending rows
(2026-09-14). The spec's "`ASC` or `DESC`" is right; the message is not.

## `/v2/resolutions` condition rows can omit documented fields

`GET /v2/resolutions?condition=0x789f0872f66cfffd21a33020e5c90e11f95f947e03be77ac2df7e86b0cb71527`
(a market from the weekly winners board, resolved 2026-09-09; captured
2026-09-14 as `polyoxide-data/tests/fixtures/v2/resolutions.json`) returned a
row with `resolution_source: "reported"` but:

- no `reporter`, which the spec documents without any condition;
- no `market_type`, which the spec documents as present on condition-keyed rows;
- `transaction_hash` and `log_index` as empty strings rather than absent.

Both missing fields are `Option` in `polyoxide-data`, so nothing fails to
decode; `tests/v2_wire_agreement.rs` excuses them for this fixture. An earlier
capture of a different market omitted `was_arbitrated` too, so which fields a
row carries varies by market.

## Combo trades carry 62-digit condition ids

`/v2/trades` includes combo trades. Their `condition_id` is a combo condition id of
62 hex digits, ending in zero padding, rather than a `0x`-plus-64-hex market
condition id, and they have empty `slug` and `outcome` and a title of legs joined by
`AND`. In one page of 1,000 trades on 2026-09-14, 8 rows were combo trades.

`/v2/holders` answers a combo condition id with `400 invalid condition id`
(`parameter: "condition"`), so anything that feeds trade rows into market routes
must filter them. The schema describes `condition_id` as a market condition id and
does not mention the combo form.

## `0x0000…0001` is a known wallet

`/v2/user-stats?user=0x0000000000000000000000000000000000000001` returns a row
of zeros, not `data: null`. A freshly random address returns `data: null` on
both `/v2/user-stats` and `/v2/leaderboard?user=` (2026-09-14).

**Consequence:** tests that need an unknown wallet generate a random address.

## Measured rate limits

Upstream publishes no v2 figures. Each route below was ramped with
`polyoxide-data/examples/v2_soak` (raw requests, every URL distinct so the CDN
cannot answer, abort on the first 429) and then validated at the shipped
limiter's pace. `RateLimiter::data_default` pins the counts in the table at the
end of this section.

### Ramps

<!-- For each route: the date, the exact command, then the harness's table and
its "Pin for" line pasted verbatim. Re-runs (lower stages, more concurrency)
get their own entry below the first, with the reason. -->

#### `/v2/positions` (2026-09-14T13:05Z)

`target/release/examples/v2_soak --route positions` (500 wallets, 76,500 distinct probe URLs)

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 294 | 943 | 0 | 0 | 1 | clean |
| 15 req/s | 799 | 13.32 | 418 | 6184 | 0 | 0 | 20 | **invalid**: 20 of 799 requests failed (status 503) |

The ramp is invalid, so nothing may be pinned: stage at 15 req/s: 20 of 799 requests failed (status 503)

No 429 at all: at 15 req/s the origin answered `503` and p99 rose 6.5x over the
10 req/s stage. Re-run once after 10 minutes, per the plan's rule for 5xx errors.

#### `/v2/positions`, re-run (2026-09-14T13:19Z)

`target/release/examples/v2_soak --route positions`

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 245 | 4924 | 0 | 0 | 5 | clean |
| 15 req/s | 900 | 15.00 | 312 | 2087 | 0 | 0 | 1 | clean |
| 20 req/s | 1200 | 20.00 | 254 | 3483 | 0 | 0 | 3 | clean |
| 30 req/s | 1719 | 28.65 | 287 | 2576 | 6 | 0 | 5 | **throttled** on /v2/positions (unrecognised 429, retry-after none, at 57.7s) |

Pin for /v2/positions: 200 per 10s

The 503s did not recur, so this ramp stands. The 429s at 30 req/s were neither the
v2 JSON body (`code: rate_limited`) nor Cloudflare's `error code: 1015` page, and
carried no `Retry-After`; `v2_soak` does not keep bodies, so the shape is recorded
only as what it was not. The first stage's p99 was noisy (4.9 s against 2–3.5 s
later), which loosens the saturation check for this ramp; no later stage came near
3x it.

#### `/v2/positions/combos` (2026-09-14T13:34Z)

`target/release/examples/v2_soak --route combo-positions` (500 wallets, 51,000 distinct probe URLs)

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 195 | 492 | 0 | 0 | 0 | clean |
| 15 req/s | 900 | 15.00 | 193 | 560 | 0 | 0 | 0 | clean |
| 20 req/s | 1200 | 20.00 | 192 | 460 | 0 | 0 | 0 | clean |
| 30 req/s | 1800 | 30.00 | 194 | 473 | 0 | 0 | 0 | clean |
| 40 req/s | 2400 | 40.00 | 191 | 458 | 0 | 0 | 0 | clean |

Pin for /v2/positions/combos: 400 per 10s

Clean to the harness ceiling with flat latency. Most sampled wallets hold no combo
positions, so these are light queries; the ceiling, not the server, ended the ramp.

#### `/v2/trades` (2026-09-14T14:00Z)

`target/release/examples/v2_soak --route trades` (500 wallets, 51,000 distinct probe URLs)

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 220 | 449 | 0 | 0 | 0 | clean |
| 15 req/s | 900 | 15.00 | 212 | 465 | 0 | 0 | 0 | clean |
| 20 req/s | 1200 | 20.00 | 213 | 479 | 0 | 0 | 0 | clean |
| 30 req/s | 1779 | 29.65 | 210 | 431 | 6 | 0 | 0 | **throttled** on /v2/trades (unrecognised 429, retry-after none, at 59.3s) |

Pin for /v2/trades: 200 per 10s

Same shape as `/v2/positions`: clean at 20 req/s, then an unrecognised 429 with no
`Retry-After` in the last seconds of the 30 req/s stage (57.7 s there, 59.3 s here),
with flat latency. A cap reached late in a stage at a fixed rate looks like a window
count rather than server strain. The same count on two routes could have meant one
rule for the whole client, but `/v2/activity` below ran clean at 30 and 40 req/s, so
the cap is not client-wide at that rate; the mixed validation still tests the
aggregate.

#### `/v2/activity` (2026-09-14T14:15Z)

`target/release/examples/v2_soak --route activity` (500 wallets, 51,000 distinct probe URLs)

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 313 | 581 | 0 | 0 | 0 | clean |
| 15 req/s | 900 | 15.00 | 314 | 595 | 0 | 0 | 0 | clean |
| 20 req/s | 1200 | 20.00 | 314 | 575 | 0 | 0 | 0 | clean |
| 30 req/s | 1800 | 30.00 | 339 | 586 | 0 | 0 | 0 | clean |
| 40 req/s | 2388 | 39.80 | 310 | 588 | 0 | 0 | 0 | clean |

Pin for /v2/activity: 400 per 10s

Clean to the harness ceiling with flat latency, despite being a heavier query than
combo positions (p50 about 310 ms).

#### `/v2/user-pnl` (2026-09-14T14:33Z)

`target/release/examples/v2_soak --route user-pnl` (500 wallets, 17,500 distinct probe URLs)

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 211 | 542 | 0 | 0 | 0 | clean |
| 15 req/s | 900 | 15.00 | 205 | 460 | 0 | 0 | 0 | clean |
| 20 req/s | 1200 | 20.00 | 192 | 314 | 0 | 0 | 0 | clean |
| 30 req/s | 1738 | 28.97 | 200 | 1431 | 0 | 0 | 0 | clean |
| 40 req/s | 2289 | 38.15 | 182 | 1309 | 0 | 0 | 0 | clean |

Pin for /v2/user-pnl: 400 per 10s

Clean by the rules, but the nearest to them of any route: p99 rose from 314 ms at
20 req/s to 1.4 s at 30 (the saturation threshold was 1.6 s), and achieved rate fell
to 96–97% of target. Validation at the pinned pace is the check on this one.

#### `/v2/holders?include_pnl=true` (2026-09-14T14:52Z)

`target/release/examples/v2_soak --route holders-pnl` (300 markets, 30,000 distinct probe URLs)

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 208 | 444 | 0 | 0 | 26 | **invalid**: 26 of 600 requests failed (status 400) |

The ramp is invalid, so nothing may be pinned: stage at 10 req/s: 26 of 600 requests failed (status 400)

A harness defect, not a limit: the market pool included combo condition ids from
combo trades (see "Combo trades carry 62-digit condition ids" above), which
`/v2/holders` rejects. Fixed in `v2_soak` (`is_market_condition_id`) and re-run.

#### `/v2/holders?include_pnl=true`, re-run (2026-09-14T14:55Z)

`target/release/examples/v2_soak --route holders-pnl` (300 market condition ids, 30,000 distinct probe URLs)

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 212 | 447 | 0 | 0 | 0 | clean |
| 15 req/s | 900 | 15.00 | 209 | 440 | 0 | 0 | 0 | clean |
| 20 req/s | 1200 | 20.00 | 213 | 511 | 0 | 0 | 0 | clean |
| 30 req/s | 1800 | 30.00 | 203 | 463 | 0 | 0 | 0 | clean |
| 40 req/s | 2394 | 39.90 | 201 | 484 | 0 | 0 | 0 | clean |

Pin for /v2/holders: 400 per 10s

Clean to the harness ceiling with flat latency, although `include_pnl=true` is the
heaviest holders shape and `/v2/holders` is cached for 120 s (no cache hits).

### Validation

<!-- One row per `--pace client` run: command, result table row, PASS/FAIL,
and what was changed before the next run if it failed. -->

### Pinned

| Route | Per 10s | Ramp stopped by |
|-------|---------|-----------------|
| `/v2/positions` | 200 | 429 (unrecognised body) at 30 req/s |
| `/v2/positions/combos` | 400 | clean to 40 req/s (harness ceiling) |
| `/v2/trades` | 200 | 429 (unrecognised body) at 30 req/s |
| `/v2/activity` | 400 | clean to 40 req/s (harness ceiling) |
| `/v2/user-pnl` | 400 | clean to 40 req/s (harness ceiling); p99 near the saturation threshold at 30 |
| `/v2/holders` | 400 | clean to 40 req/s (harness ceiling), measured with `include_pnl=true` |
