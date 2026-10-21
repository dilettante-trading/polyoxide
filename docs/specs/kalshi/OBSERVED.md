# Kalshi: what the hosts do

Measured against `external-api.kalshi.com` (production) and `external-api.demo.kalshi.co` (demo) on 2026-10-18, with unsigned `GET`s, unless an entry says otherwise. `openapi.yaml` and `asyncapi.yaml` beside this file are Kalshi's served OpenAPI (`info.version` 3.34.0) and AsyncAPI (`info.version` 2.0.0), byte for byte. Kalshi publishes neither as JSON: `openapi.json` and `asyncapi.json` answer 404. Kalshi announced schema corrections dated 2026-10-22, so a drift issue will follow, and adopting it is that issue's `curl`.

This file records what the specs leave out or get wrong. A later story's findings go in a section of their own, directly above the last line.

## Probes of the production host

Read-only requests at about 09:10 UTC. Rows 2 and 3 and the error bodies below were repeated at 10:30 UTC.

1. `GET /trade-api/v2/account/endpoint_costs` answers with no key. `default_cost` is 10, and the listed paths are full templates (`/trade-api/v2/portfolio/orders/:order_id`, `/trade-api/v2/cfbenchmarks/*endpoint`). None of the Predictions client's routes is listed, so each costs 10.
2. `GET /trade-api/v2/exchange/status` answers with no key, with `exchange_index_statuses` for indexes 0 to 3 (Default, Combos, Crypto & Commodities, and Tennis, Baseball, Basketball). The response is cacheable for a second (`cache-control: public, max-age=1`). The demo host answers it with 200 too.
3. For an unknown ticker, `/markets/{t}` and `/events/{e}` answer `404 {"error":{"code":"not_found","message":"not found"}}`, but `/markets/{t}/orderbook` answers `200 {"orderbook_fp":{"no_dollars":[],"yes_dollars":[]}}`. The orderbook route cannot tell an unlisted ticker from an empty book, so a listing is confirmed on `/markets/{t}`.
4. Tickers are case-sensitive: a lowercase ticker is a 404 on `/markets/{t}` and an empty book on `/orderbook`. Tickers contain `-` and `.` (`KXBTCD-26OCT1806-T82099.99`). The longest of 1,000 open tickers was 51 characters.
5. The REST orderbook lists each side's levels in ascending price, so worst first, where the OpenAPI's description says best to worst. `depth=n` keeps the n best levels, and a `depth` of 0 or less keeps every level.
6. Error bodies: see below.
7. Of 1,000 open markets, every one has `notional_value_dollars` of `1.0000` and three `price_ranges` (steps `0.0001`, `0.0010` and `0.0001`, with `price_level_structure` `center_deci_edge_centi_cent`). Trades come newest first, and `count_fp` can be fractional (`46.91`).
8. `GET /trade-api/v2/events/{e}?with_nested_markets=true` returned all 188 markets of `KXBTCD-26OCT1806`, as many as `/markets?event_ticker=` paged. The OpenAPI says markets settled before the historical cutoff are left out.

## Error bodies

Kalshi answers with three shapes, which the client's one decode reads (`PredictionsError`'s `From<ApiError>`).

- `{"error":{"code":"…","message":"…","details":"…"}}`, with `details` optional. A 404 is `{"error":{"code":"not_found","message":"not found"}}`, and an unsigned request to a route that needs a key, `GET /account/limits`, is a 401 with `{"error":{"code":"token_authentication_failure","message":"token authentication failure"}}`.
- `{"error":"<text>"}`. The documentation gives a 429 the body `{"error":"too many requests"}`. A 429 was not provoked.
- `{"msg":"<text>"}`: a parameter-validation 400. The wording varies. The first probe recorded `Parameter validation failed for …`, and `GET /markets?limit=abc` at 10:30 UTC answered `{"msg":"Invalid format for parameter limit: error binding string parameter: strconv.ParseInt: parsing \"abc\": invalid syntax"}`. The client reads `msg` whole.

The documentation says a 429 carries no `Retry-After` and no `X-RateLimit-*` header.

## The AsyncAPI's ticker pattern

`asyncapi.yaml` gives `marketTicker` the pattern `^[A-Z0-9-]+$`, and its own examples carry dots (`FED-23DEC-T3.00`), as production's tickers do. The client accepts `[A-Z0-9.-]`, 1 to 200 characters (the OpenAPI's `maxLength` for the tickers of `GET /markets/orderbooks`), at least one of them a letter or digit, so `.` and `..`, which a URL resolves as dot segments, are refused, and uppercases ASCII letters.

## Rate limits

Published (`getting_started/rate_limits`, read 2026-10-18):

- every authenticated request costs tokens, 10 by default, and `GET /account/endpoint_costs` lists the exceptions;
- Read and Write are separate token buckets. Basic's Read bucket refills 200 tokens a second and holds 3 seconds of it, 600. Its Write bucket refills 100 a second and holds 1 second. No tier's Read capacity or refill is below Basic's;
- `GET /portfolio/balance` has a bucket of its own (since 2026-10-15), and Write buckets are per shard;
- a 429 carries no penalty or cooldown, and the client should back off;
- no limit for unauthenticated requests is published.

What the client builds, and what is provisional:

- One Read layer, `predictions.read`, a token bucket sized at Basic's published capacity (600) and refill (200 a second). Kalshi publishes a bucket's capacity and the server is a bucket, so the client's bucket holds the published burst. It is provisional until the account's own size is read from `GET /account/limits` (Story 4 of the epic). Basic is the floor of every tier, so a client that cannot read its account's size under-sends and never over-sends. Corrected by Epic 9, Story 7 (2026-10-21): this is a keyed client's bucket for the routes that need the key, and a client without a key sizes its bucket from the unsigned soak. A keyed client has a second Read bucket for its public reads, sized as an unsigned client's and sharing the one hold; see the `### Rate limits` subsection of `## Epic 9`.
- A client with no credentials starts at that sizing and, when its first send confirms, takes the refill the Epic 9 soak measured (`### Rate limits` in the Epic 9 section below), keeping Basic's depth.
- Every route costs the default, 10. The first-use confirmation (Story 4) replaces a cost with what `GET /account/endpoint_costs` lists.
- An instrument's `min_size` and `size_increment` will be 0.01, the granularity the OpenAPI gives `FixedPointCount` for orders. They are provisional until Kalshi's `Trading` probes them.

Recorded, not built: the Write bucket, the balance bucket and the per-shard Write buckets come with Kalshi's `Trading`.

## Public REST (Story 2, captured 2026-10-18)

Read from `https://external-api.kalshi.com/trade-api/v2` with no credentials by
`scripts/capture_kalshi_predictions.py`; the fixtures are in
`polyoxide-kalshi/tests/fixtures/predictions/` and their `PROVENANCE.md` says how each was trimmed.

### The open listing serves multivariate combinations first

`GET /markets?status=open&limit=1000`, paged by cursor, returned 9,996 multivariate combinations
(`mve_collection_ticker` set) in its first ten pages of 10,000 rows, and no single market with a
quote on both sides. With `mve_filter=exclude` the first page held 579 rows with a YES bid above 0
and a YES ask below 1. So a walk for a quoted single market filters: the capture script and the live
suites' `two_sided_market` page `markets?status=open&mve_filter=exclude&limit=1000` (at most 10 pages
for the capture, 5 for the live helper) and confirm both sides on that market's orderbook route, since
a CDN caches per URL. `mve_filter` takes `only` and `exclude`, and the client's `MveFilter` carries
both.

### The `status` filter takes five values

The OpenAPI's parameter enum for `GET /markets`'s `status` lists `unopened`, `open`, `paused`,
`closed` and `settled`; its prose leaves out `paused`. `MarketStatusFilter` carries all five, and
`spec_agreement_predictions` checks them against the mirror's parameter enum.

### Keys the wire carries and the schema lacks

Each is modelled as an optional field of its type and excused in `OBSERVED_EXTRA` in
`spec_agreement_predictions.rs`, as perps' are. The capture found no key the schema lacks.

## Account limits and signing (Epic 8, Story 4)

Captured from the demo host (`https://external-api.demo.kalshi.co`) with the owner's demo key. The answer of `GET /trade-api/v2/account/limits` is `polyoxide-kalshi/tests/fixtures/predictions/account/limits.json`, and `polyoxide-kalshi/tests/fixtures/predictions/account/PROVENANCE.md` says when. The demo account's tier and sizing are in that file.

- **The signing scheme is accepted as the docs give it.** `GET /trade-api/v2/account/limits`, signed over `timestamp_ms + METHOD + path` (the path written from the API root and without its query) and sent with `KALSHI-ACCESS-KEY`, `KALSHI-ACCESS-TIMESTAMP` and `KALSHI-ACCESS-SIGNATURE`, answered `200` with the registered key. That route is the proof that the signature verifies.
- **The demo host did not check the key or the signature on `GET /trade-api/v2/markets` (2026-10-18).** During the capture, that route signed with a key id the demo host did not know answered `200`, while `GET /trade-api/v2/account/limits` with the same key answered `401` (`authentication_error`, details `NOT_FOUND`). Only that one public route was tried, so whether other public routes check the key is not known. A `200` on a signed read of `/markets` shows only that the headers are accepted there, not that the signature verified.
- **`/account/limits` is modelled whole.** `AccountLimits` holds every key of the captured answer, and `the_captured_limits_fit_the_type_in_both_directions` fails on one it lacks. `read` and `write` are each a `refill_rate` and a `bucket_capacity`, in whole tokens, and the live suite holds the demo account's Read bucket at or above Basic's published 600 tokens refilling 200 a second.
- **What the client does with it.** The first send of a client reads `/account/endpoint_costs` and, with a key, `/account/limits`, and resizes the Read bucket in place to the answer's `read`. It keeps Basic's sizing, with one WARN, when a read fails or the answer reports a zero capacity or refill. A client without a key keeps Basic's sizing, since no anonymous limit is published. Corrected by Epic 9, Story 7 (2026-10-21): a client without a key takes the refill the unsigned soak measured (180 tokens a second, Basic's depth), and a keyed client's public reads draw on a second bucket of that size, so the key's bucket serves `GET /account/limits` alone; see the `### Rate limits` subsection of `## Epic 9`.
- **Recorded, not built.** The Write bucket, the per-shard Write buckets and the separate bucket of `GET /portfolio/balance` (2026-10-15) are published and not modelled by the throttle. They come with Kalshi's `Trading`.

<!-- kalshi-ws-capture:begin -->
## The order-book socket, as the demo host answers it

Captured 2026-10-18T15:15:57Z by `scripts/capture_kalshi_predictions_ws.py` from `wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2` on `KXBTCD-26OCT2317-T81999.99`, with `KXTRUMPAPPROVE-26OCT19-E39.1` added to the subscription. Fixtures: `polyoxide-kalshi/tests/fixtures/predictions/ws/`.

| Question | Observed |
|----------|----------|
| First `seq` of a subscription | 1, on its `orderbook_snapshot` frame |
| `ok` to `update_subscription` advances `seq` | yes: `seq` 45 before it, 46 on it |
| `unsubscribed` advances `seq` | yes: `seq` 55 before it, 56 on it |
| `add_markets` is followed by a snapshot of the added market | yes |
| Snapshot book not crossed | yes: best bid 0.4500, best ask 0.4600 |
| Order of each side's levels on the wire | YES side ascending (0.0100 to 0.4500); NO side descending (0.5000 to 0.4600) |
| Bound on markets per subscription | none met: the capture subscribed one market, then added a second |

**An uncrossed book proves little for this market.** Read as raw NO prices it is uncrossed too (best bid 0.4500, best ask 0.5000), so the crossing cannot tell the two pricings apart here. The order of the levels points to YES pricing. The mirror's own example (`docs/specs/kalshi/asyncapi.yaml`) lists both sides ascending, each ending at its best level in its own pricing. Here the YES side ascends to its best bid and the NO side descends to its lowest price, the best ask in YES pricing, so both sides run outside in to the touch; in raw NO pricing the best NO bid is the highest price and the side would ascend. That is an inference from the listing order, not a measurement, and a market priced well away from 0.5 would settle it by arithmetic. The client does not read the order: `event.rs` sorts both sides.
<!-- kalshi-ws-capture:end -->

<!-- capture findings go above this line -->

## Epic 9

Findings of Epic 9's Story 1 and of the stories after it. Each is dated and names its host. A block between `kalshi-…-capture:begin` and `…:end` markers is written by the capture script that names it, and the script's next run replaces it. A later story inserts its findings directly above the last line of this section.

### The design session's probe of the candlestick routes (production, unsigned, 2026-10-19)

`GET /trade-api/v2/markets/candlesticks` for `KXMLBGAME-26OCT192008CLETB-TB` at `period_interval` 1, 60 and 1440 answered `200` with `{markets: [{market_ticker, candlesticks}]}`. In a traded period `price` held `open_dollars`, `high_dollars`, `low_dollars`, `close_dollars`, `mean_dollars` and `previous_dollars`; the first traded period had no `previous_dollars`. An untraded period after a trade held only `previous_dollars`, and a period before any trade held `price: {}`. `min_dollars` and `max_dollars` never appeared. `period_interval=5` answered `200` with no rows on the batch route and `400` (a validation `msg`) on `GET /series/{series_ticker}/markets/{ticker}/candlesticks`. Corrected by Story 1's capture (2026-10-21): the batch route answers `5` with `200` and the daily candles. Hourly candles end on the hour; the daily candle ended at 04:00 UTC. The captures below re-measure each of these.

<!-- kalshi-candlesticks-capture:begin -->
### Candlesticks, production, unsigned

Captured 2026-10-21T09:32:12Z by `scripts/capture_kalshi_predictions.py --candlesticks` from `https://external-api.kalshi.com/trade-api/v2` on `KXUCLGAME-26OCT21RMARBL-RMA` (series `KXUCLGAME`, opened 2026-10-07 00:35Z). Fixtures: `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/`.

| Width (min) | Span (UTC) | Rows | Traded / untraded / empty | Wire order by `end_period_ts` | `end_period_ts` mod the width (s) | `price` keys seen |
|-------------|------------|------|---------------------------|-------------------------------|-----------------------------------|-------------------|
| 1 | 2026-10-21 07:31Z to 2026-10-21 09:31Z | 89 | 76 / 13 / 0 | ascending | 0 | `close_dollars`, `high_dollars`, `low_dollars`, `mean_dollars`, `open_dollars`, `previous_dollars` |
| 60 | 2026-10-14 09:31Z to 2026-10-21 09:31Z | 167 | 152 / 15 / 0 | ascending | 0 | `close_dollars`, `high_dollars`, `low_dollars`, `mean_dollars`, `open_dollars`, `previous_dollars` |
| 1440 | 2026-10-07 00:35Z to 2026-10-21 09:31Z | 15 | 13 / 1 / 1 | ascending | 14400 (04:00 UTC) | `close_dollars`, `high_dollars`, `low_dollars`, `mean_dollars`, `open_dollars`, `previous_dollars` |

- `min_dollars` or `max_dollars` in any `price`: never. A `null` in any `price`: never, the keys are omitted.
- An untraded period's `price` holds a key besides `previous_dollars`: no.
- A period before the first trade held `price` = `{}`, `yes_bid` = `{"close_dollars": "0.0600", "high_dollars": "0.0600", "low_dollars": "0.0100", "open_dollars": "0.0100"}`, `yes_ask` = `{"close_dollars": "0.8100", "high_dollars": "1.0000", "low_dollars": "0.8100", "open_dollars": "1.0000"}`, `volume_fp` = `0.00`.
- First traded period: 1: the earliest traded period has `previous_dollars` (the span starts after the market opened); 60: the earliest traded period has `previous_dollars` (the span starts after the market opened); 1440: the earliest traded period lacks `previous_dollars` (the span starts at the market's open).
- The series route at 60 answered `['candlesticks', 'ticker']`, identical to the batch route's rows.
- Width 5 on the batch route: `200`, 7 rows.
- Width 5 on the series route: `400`, body `{"msg": "Parameter validation failed for GetMarketCandlesticks: Key: 'GetMarketCandlesticksParams.PeriodInterval' Error:Field validation for 'PeriodInterval' failed on the 'oneof' tag"}`.
- One batch for 100 open markets at width 1 over 3 hours: `400` with no market list: {'error': {'code': 'bad_request', 'message': 'bad request', 'details': 'requested candlesticks across all markets: 18000, max candlesticks: 10000'}}
- Spec D6's reading of the wire (every `*_dollars` and `*_fp` a decimal string, `yes_bid` and `yes_ask` carrying exactly the four distribution keys, `end_period_ts` an integer): holds in every captured row.
<!-- kalshi-candlesticks-capture:end -->

<!-- kalshi-ws-epic9:begin -->
### The multi-channel socket, as the demo host answers it

Captured 2026-10-21T11:08:11Z by `scripts/capture_kalshi_predictions_ws.py --epic9` from `wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2`. `first` `KXRAINSHARD2-26OCT22-NYC` and `second` `KXQUICKSETTLE-21OCT26H0710-3` were subscribed, `third` `KXQUICKSETTLE-21OCT26H0710-2` added by the one-`sid` `add_markets` (the host refused the two-`sid` update), and `outside` `KXCS2GAME-26OCT210415EVOEXMETA-EVOE` never subscribed. Fixtures: `polyoxide-kalshi/tests/fixtures/predictions/ws/`.

| Question | Observed |
|----------|----------|
| One `subscribe` for the three channels | answered by 3 `subscribed` frames, one for each channel, each with its own `sid`: `orderbook_delta` 1, `ticker` 2, `trade` 3 |
| A `ticker` frame | on `sid` 2 (the answered 2); carries `seq`: no; required keys absent: none; keys the mirror lacks: none |
| A `trade` frame | on `sid` 3 (the answered 3); carries `seq`: yes; required keys absent: none; keys the mirror lacks: none |
| `get_snapshot` for `first` on the order-book `sid`: the frames carrying its id | `orderbook_snapshot` |
| The snapshot answering it | carries the command's `id`: yes; `sid` 1 (the order-book `sid` is 1); `seq` 35; the last `seq` on that `sid` before it was 34; 5 deltas on the `sid` came between the command and the snapshot |
| The snapshot's `seq` is the next number of the `sid`'s numbering | yes |
| The first delta after the snapshot continues from its `seq` | yes: `seq` 36 after 35; 100 deltas captured, 92 of them for a market other than `first` |
| Spec D9's premise (the answer is on the `sid`'s numbering and the next delta follows it) | holds |
| One `update_subscription` naming two `sids` (`add_markets` for `third`) | `error` code 12: Exactly one subscription ID is required; a snapshot of `third` followed on the order-book `sid`: no |
| One-`sid` `update_subscription` `add_markets` for `third` on the `orderbook_delta` `sid` (1) | `ok`, `id` 4, `sid` 1, `seq` 347; a snapshot of `third` followed on the order-book `sid`: yes |
| One-`sid` `update_subscription` `add_markets` for `third` on the `ticker` `sid` (2) | `ok`, `id` 5, `sid` 2, `seq` absent; a snapshot of `third` followed on the order-book `sid`: no |
| One-`sid` `update_subscription` `add_markets` for `third` on the `trade` `sid` (3) | `ok`, `id` 6, `sid` 3, `seq` 4; a snapshot of `third` followed on the order-book `sid`: no |
| One-`sid` `update_subscription` `delete_markets` for `third` on the `ticker` `sid` (2) | `ok`, `id` 7, `sid` 2, `seq` absent; a snapshot of `third` followed on the order-book `sid`: no |
| One-`sid` `update_subscription` `delete_markets` for `third` on the `trade` `sid` (3) | `ok`, `id` 8, `sid` 3, `seq` 5; a snapshot of `third` followed on the order-book `sid`: no |
| `get_snapshot` for a ticker outside the subscription (the owner took the host's answer in place of an error fixture, spec 3) | `orderbook_snapshot`; the frame carrying the command's `id`: `orderbook_snapshot` for `KXCS2GAME-26OCT210415EVOEXMETA-EVOE`, `sid` 1, `seq` 909 (the last before it 908) |
| `orderbook_delta` frames for `outside` after its snapshot, to the end of the session (does the snapshot subscribe it?) | 0 of the 450 `orderbook_delta` frames after it, in a window this capture did not time: consistent with the snapshot not subscribing it (a quiet market shows the same) |
| `get_snapshot` on the order-book `sid` for a ticker that does not exist (`KXNOSUCHMARKET-99XXX-T1`) | `orderbook_snapshot`, `id` 10, `sid` 1, `seq` 962; `market_id` ''; no levels |
| `get_snapshot` on the `ticker` channel's `sid` | `error` code 13 ('Unsupported action'), `id` 11, `sid` 2, `seq` absent |
| `get_snapshot` on the `trade` channel's `sid` | `error` code 13 ('Unsupported action'), `id` absent, `sid` 3, `seq` 6 |
| A `subscribe` for `ticker` and `trade` naming no market | refused (code 14, 'Market Ticker required'); no `sid` |
| The final `unsubscribe` of every `sid` | `unsubscribed`, `unsubscribed`, `unsubscribed` |
| Code 27 (too many requests) met | 0 |
<!-- kalshi-ws-epic9:end -->

### The batch candlestick route leaves out whole periods (production, unsigned, 2026-10-21)

Found in review of Story 1, from the committed fixtures `batch_1.json`, `batch_60.json` and `batch_1440.json` in `polyoxide-kalshi/tests/fixtures/predictions/candlesticks/` (market `KXUCLGAME-26OCT21RMARBL-RMA`). Some periods have no row: untraded rows do exist, but for some periods the answer holds no row at all, and the next row's `end_period_ts` jumps by more than the width. Counting the periods between the first and last row of each file:

| Width (min) | Rows | Periods from the first to the last row's `end_period_ts` | Periods with no row | Gaps |
|-------------|------|-----------------------------------------------------------|---------------------|------|
| 1 | 89 | 119 | 30 | 23 |
| 60 | 167 | 168 | 1 | 1 |
| 1440 | 15 | 15 | 0 | 0 |

At width 1 the gaps are one to three periods long: the first is `end_period_ts` 1792568160 (07:36Z) to 1792568340 (07:39Z), two periods (07:37Z and 07:38Z) with no row. At width 60 the one gap is the hour ending 2026-10-15 09:00Z (`end_period_ts` 1792054800), between rows ending 1792051200 and 1792058400. So the rows' `end_period_ts` do not run at every multiple of the width, which `describe()`'s offset column (the remainder of `end_period_ts` by the width, 0 at widths 1 and 60, and 14400 at width 1440, whose periods end at 04:00Z) does not show. This does not contradict D6: a period absent from the answer yields no candle (D6's mapping, as Story 3 implements it), so `Candles::candles` may skip periods, and a caller that needs a regular grid fills the gaps itself.

### Rate limits

Measured with `polyoxide-kalshi/examples/read_soak.rs` (Epic 9, Story 7). Every URL of a run was distinct, because a repeated URL can be answered from a cache without reaching the origin, and each route was probed once on the host before any stage. A stage is requests a second: Basic's Read bucket refills 200 tokens a second, which is 20 reads at the default cost of 10.

**The paths, probed first** (2026-10-21 12:22Z and 12:26Z, from the development workstation `fresco` over its ordinary network connection, unsigned):

| Route | Path | Answered |
|---|---|---|
| market | `GET /trade-api/v2/markets/{ticker}` | 2xx |
| event | `GET /trade-api/v2/events/{event_ticker}` | 2xx |

**The unsigned ramp** on production, `--stage-secs 60 --cooldown-secs 30` unless a row says otherwise. The market ramp started at 12:22:18Z, the event ramp at 12:26:18Z and the event rerun at 12:30:16Z. The `code` column is what the harness printed: it reads a flat `{"error":"<text>"}`, and the body the host sent is nested (below), so it prints `unknown`.

| Route | Stage (requests a second) | Replies | Verdict | First 429, seconds into the stage, and its body |
|---|---|---|---|---|
| market | 10 | 601 | Clean | |
| market | 20 | 1201 | Clean | |
| market | 40 | 169 (151 from the origin, 18 throttled) | Throttled | 3.26 s, code `unknown` |
| event | 10 | 601 | Clean | |
| event | 20 | 1201 | Clean | |
| event | 40, `--stage-secs 30` (a rerun, see below) | 179 (162 from the origin, 17 throttled) | Throttled | 3.51 s, code `unknown` |

The market ramp stopped at its first refused stage, 40, and ran no higher. The event ramp stopped before its stage of 40 at 60 s, with `2401 more distinct URLs are needed and the listing walk left 2083`: the walk found 3886 event tickers and the first two stages and the probe spent 1803 of them. It was rerun from 40 with a 30 s window, which needs 1201.

Highest clean stage: 20 requests a second (market). The host refused 40 requests a second after 3.26 s with `{"error":{"code":"too_many_requests","message":"too many requests"}}`.

The event route was clean at 20 requests a second and refused 40 after 3.51 s, so it is refused no earlier than the market route and the two read as one limit.

**Where the refusal fell.** A bucket of 600 tokens refilling 200 a second, read at 10 tokens, takes 600 / (40 × 10 − 200) = 3.0 s to empty at 40 reads a second. The host's first 429 came at 3.26 s on the market route and 3.51 s on the event route, and a 24-worker burst of about 34 reads a second against distinct tickers was served 156 reads before its first 429 at 4.97 s (a bucket of that size empties after 600 / (34 × 10 − 200) = 4.3 s). The unsigned host behaves as Basic's Read bucket.

**The 429 as the host sent it.** A 429 was provoked on purpose once more, with a 24-worker burst of at most 250 reads that stopped issuing reads at its first refusal (13 refusals in all, the reads already in flight included): `{"error":{"code":"too_many_requests","message":"too many requests"}}`, `Content-Type: application/json; charset=utf-8`, `cache-control: no-store`, with no `Retry-After` and no `X-RateLimit-*` header. That is the nested shape of Kalshi's other errors and not the flat `{"error":"too many requests"}` the rate-limit page documents, and `tests/fixtures/predictions/error_rate_limited.json` is the flat one, written from the page and not captured.

**The signed-bucket probe**, demo host (`https://external-api.demo.kalshi.co`), 2026-10-21 12:31Z. The demo key's account is tier `basic` with a Read bucket of 600 tokens refilling 200 a second; a public read costs 10 tokens and the limits read 10. Each phase sends signed reads faster than the bucket refills, so a bucket that starts full empties after the time in the table, and stops at its first 429. The control repeats the limits route, which is signed and so uncached; the public phase reads distinct tickers.

| Phase | Route | Requests a second | The key's bucket empties after | First 429 |
|---|---|---|---|---|
| control (needs the key) | `GET /account/limits` | 40 | 3 s | 2.97 s (155 replies: 131 served, 24 throttled) |
| public | `GET /markets/{ticker}` | 40 | 3 s | none in the 10 s window (401 replies over 401 distinct tickers, all served) |

Verdict: does not follow. The control throttled at 2.97 s, where a bucket that empties after 3 s puts it, so the host enforces the key's bucket at this rate, and the public read then did not throttle in 10 s, three times the time the key's bucket takes to empty, so a signed public read does not draw on the key's bucket. The probe was made on the demo host; the production host was measured unsigned only (above), where a public read at the same 40 requests a second was refused after 3.26 s. The verdict is the demo host's alone: that host did not check the key or the signature on `GET /markets` when it was tried before (`## Account limits and signing (Epic 8, Story 4)`, 2026-10-18), and no signed public read was measured on production.

**Sizing.** The ramp was clean through 20 requests a second, Basic's own rate (20 reads at 10 tokens is Basic's 200 tokens a second), and refused 40. The shipped client at exactly Basic's sizing was refused all the same. A ramp's pacer spends exactly the refill, which leaves the host's bucket full; the client spends its 60-read depth first and then sits at the refill with demand to spare, so the host's bucket hovers near empty and any jitter in arrival times is a refusal. At Basic's sizing (600 tokens refilling 200 a second) it drew retried 429s on production, and at 180 none (the validation below). So the unsigned client's Read bucket refills `(N − ⌈N/10⌉) × 10` tokens a second for N = 20, the highest clean stage: 180 tokens a second, below Basic's 200, a tenth of the measured rate left unused (`RESERVED_FRACTION`, as core's and Binance's) at the default cost of 10 tokens a read. Its depth stays Basic's 600 tokens, since the ramp measured a rate and not a depth; the validation below ran that depth against the host. The owner set this sizing on 2026-10-21 (spec D11).

**Validation**, the shipped client on production (`--routes all --pace client --concurrency 32 --max-concurrent 32 --stage-secs 120`), four runs of 120 s on 2026-10-21 from the same workstation. Every 429 was retried by the send loop, so no call failed.

| Time (UTC) | Unsigned refill (depth 600) | Calls | Throttled | Failed |
|---|---|---|---|---|
| 12:35 | 200, Basic's | 2509 | 10 | 0 |
| 12:38 | 180 (a binary patched to resize the bucket to it; the sizing was not yet committed) | 2279 | 0 | 0 |
| 12:41 | 200, Basic's | 2513 | 6 | 0 |
| 12:48 | 180, as committed | 2279 | 0 | 0 |

At Basic's sizing the host refused about 0.3% of calls in both runs, spread over the market and event routes. At 180 it refused none in either run. The 12:48 run is the committed client.

**Decision, a keyed client's public reads.** A signed public read does not draw on the key's bucket (the probe above, which ran on the demo host only: production was measured unsigned), so a keyed client charges every route but `GET /account/limits` to a second Read bucket, sized as an unsigned client's (Basic's 600 tokens refilling `UNSIGNED_READ_REFILL`, 180, a second), and keeps the key's bucket for the routes that need the key. The two buckets share one hold, so a 429 still holds every request. The split starts when the client's first send confirms, and the confirmation's own costs read is its first public read. An anonymous client never splits. The owner's third exception in the spec's section 13 (2026-10-21) allowed the two existing tests that used the public status route to show the key's bucket (`a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse` and `the_bucket_takes_the_capacity_the_account_reports`) to be re-pointed at `GET /account/limits`.

<!-- rate limits end -->

### A daily candle may not span 24 hours across a clock change (open question, 2026-10-21)

One capture's daily candles ended at 04:00 UTC (the 1440 row of the candlestick capture above: `end_period_ts` mod the width was 14,400 s). That is midnight Eastern time while daylight saving is in force. If Kalshi's day follows Eastern midnight, the day of 2026-11-01, when the clocks go back, runs from 04:00 UTC to 05:00 UTC on 2026-11-02 and is 25 hours long, and the client's `open_time` (`end_period_ts` less the width, 86,400 seconds) is an hour late for that candle (and an hour early for the 23-hour day when the clocks go forward). This was not observed: no capture crosses a clock change, and the reading that the day follows Eastern time rests on one end time. A recapture of a daily candle across 2026-11-01 settles it (`uv run scripts/capture_kalshi_predictions.py --candlesticks <dir> --ticker T`, with a `--ticker` that was open before the change, since the script chooses no aged market by default).

### The client's re-snapshot command ids are accepted (demo, 2026-10-21)

polyoxide-kalshi numbers a gap's `get_snapshot` from `FOLLOW_UP_ID_BASE`, 1,000,000,000, so it never meets a membership command's id. On the demo host, with `orderbook_delta` subscribed on `KXITFMATCH-26OCT22BANSAH-SAH` (sid 1), `get_snapshot` sent with ids 1,000,000,000, 1,000,000,001 and 2,147,483,653 (past 2^31) was each answered with an `orderbook_snapshot` echoing that id, on sid 1's next `seq` (3, 4, 5). Kalshi raised no error for any of them. The probe was a one-off script, not committed; it settles the final review's M7.

<!-- epic 9 findings go above this line -->
