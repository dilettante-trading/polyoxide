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

- One Read layer, `predictions.read`, a token bucket sized at Basic's published capacity (600) and refill (200 a second). Kalshi publishes a bucket's capacity and the server is a bucket, so the client's bucket holds the published burst. It is provisional until the account's own size is read from `GET /account/limits` (Story 4 of the epic). Basic is the floor of every tier, so a client that cannot read its account's size under-sends and never over-sends.
- A client with no credentials keeps that sizing. No anonymous limit is published, so for it the sizing is a guess. A soak that measures the anonymous limit is deferred.
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
- **The demo host does not check the key or the signature on a public route (2026-10-18).** During the capture, `GET /trade-api/v2/markets` signed with a key id the demo host did not know answered `200`, while `GET /trade-api/v2/account/limits` with the same key answered `401` (`authentication_error`, details `NOT_FOUND`). So a `200` on a signed public read shows only that the headers are accepted on that route, not that the signature verified.
- **`/account/limits` is modelled whole.** `AccountLimits` holds every key of the captured answer, and `the_captured_limits_fit_the_type_in_both_directions` fails on one it lacks. `read` and `write` are each a `refill_rate` and a `bucket_capacity`, in whole tokens, and the live suite holds the demo account's Read bucket at or above Basic's published 600 tokens refilling 200 a second.
- **What the client does with it.** The first send of a client reads `/account/endpoint_costs` and, with a key, `/account/limits`, and resizes the Read bucket in place to the answer's `read`. It keeps Basic's sizing, with one WARN, when a read fails or the answer reports a zero capacity or refill. A client without a key keeps Basic's sizing, since no anonymous limit is published.
- **Recorded, not built.** The Write bucket, the per-shard Write buckets and the separate bucket of `GET /portfolio/balance` (2026-10-15) are published and not modelled by the throttle. They come with Kalshi's `Trading`.

<!-- capture findings go above this line -->
