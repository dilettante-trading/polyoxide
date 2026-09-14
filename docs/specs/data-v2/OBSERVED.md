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

## `0x0000…0001` is a known wallet

`/v2/user-stats?user=0x0000000000000000000000000000000000000001` returns a row
of zeros, not `data: null`. A freshly random address returns `data: null` on
both `/v2/user-stats` and `/v2/leaderboard?user=` (2026-09-14).

**Consequence:** tests that need an unknown wallet generate a random address.
