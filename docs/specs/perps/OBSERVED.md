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

Nothing numeric is published for the public routes. See the soak runs recorded
below by Task 12 of `docs/superpowers/plans/2026-09-30-perps-http.md`.
