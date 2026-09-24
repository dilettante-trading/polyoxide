# Data API v2

Base URL: `https://data-api.polymarket.com` (routes under `/v2`)

The second contract for the Data API: the same host and data as
[../data/INDEX.md](../data/INDEX.md), behind one shared set of conventions
instead of per-route ones.

> **Implemented by `polyoxide-data`** as `data.v2()`, alongside the v1 routes,
> which upstream says keep working. Where the live host disagrees with or goes
> beyond this spec, see [OBSERVED.md](OBSERVED.md).

Machine-readable schema: [openapi.json](openapi.json) (OpenAPI 3.1, mirror of
`https://data-api.polymarket.com/v2/openapi.json`).

**This spec is served by the API host, not the docs site.** Every other mirror
comes from `docs.polymarket.com/api-spec/`; nothing there covers `/v2`. Upstream's
`llms.txt` links this URL under "OpenAPI Specs". The file is minified onto a single
line and is vendored byte-for-byte like every other mirror, so read it through
`jq . openapi.json`. The drift check compares canonicalized forms, so the
formatting has no effect on it. Fetched five times on 2026-09-14 it was
byte-identical each time, contains no timestamps, and is served uncached
(`cf-cache-status: DYNAMIC`).

Migration guide: <https://docs.polymarket.com/migrate/data-api-v1-to-v2>.

## Conventions

All from the spec's `info.description`, which is the only place they are stated
once for every route:

- **Envelope.** Every response is `{"data": ...}`; paged routes add `pagination`.
  A documented miss is `data: null` or an empty list, never an error.
- **Cursor-only pagination.** Follow `pagination.next_cursor` until `null`.
  Sending `offset` is a `400`. Cursors are signed, typed per endpoint, and bind
  the filters they were minted with. Contradicting those filters on a boards,
  positions or combo-positions cursor is a `400`. On `trades`/`activity`, changing
  a filter mid-walk re-anchors the walk **without an error**. Following a
  cursor still needs the anchor: `/v2/positions` answers a bare `?cursor=` with a
  `400` naming the missing `user`/`condition`, so resend the first page's anchor.
- **snake_case fields.** Query parameters accept either spelling.
- **`condition`** (aliases `condition_id`, `conditionId`) replaces v1's `market`.
- **Units.** Bare `volume`/`size` are shares; `_usdc` fields are USD.
- **Sentinels.** `outcome_index: 999` means the outcome could not be labeled. A
  missing or `null` number means unavailable, never zero.
- **Errors.** JSON with `error`, a stable `code`, a `retryable` flag and a
  `trace_id`; every response echoes the id in `x-trace-id`. `429` and `503` carry
  `Retry-After` when worth retrying.
- **Rate limiting.** A heavy query may be queued for a capacity slot before a
  `429`, and a per-client allowance also answers bursts with `429` +
  `Retry-After`. No figures are published, and `data/rate-limits.md` covers
  only v1 routes. Measured figures: [OBSERVED.md](OBSERVED.md#measured-rate-limits).
- **Auth.** None.

## Endpoints

20 endpoints across five tags.

| Tag | Endpoint | Summary |
|-----|----------|---------|
| wallet | `GET /v2/approvals` | Get wallet approvals |
| wallet | `GET /v2/positions` | List positions for a user or market |
| wallet | `GET /v2/positions/combos` | List combo positions |
| wallet | `GET /v2/user-pnl` | Get a user's PnL series |
| wallet | `GET /v2/user-stats` | Get a user's profile stats |
| wallet | `GET /v2/user-volume` | Get a user's trading volume |
| wallet | `GET /v2/value` | Get portfolio value |
| feeds | `GET /v2/activity` | List account activity |
| feeds | `GET /v2/activity/combos` | List combo activity |
| feeds | `GET /v2/trades` | List trades |
| markets | `GET /v2/holders` | List a market's top holders |
| markets | `GET /v2/live-volume` | Get live volume for an event |
| markets | `GET /v2/oi` | Get open interest |
| markets | `GET /v2/prices-history` | Get a token's price history |
| markets | `GET /v2/resolutions` | Get resolution state |
| boards | `GET /v2/biggest-winners` | List the biggest wins |
| boards | `GET /v2/builders/leaderboard` | Get the builders leaderboard |
| boards | `GET /v2/builders/volume` | Get builder volume over time |
| boards | `GET /v2/leaderboard` | Get the trader leaderboard |
| service | `GET /v2/status` | Get data freshness |

The migration guide maps 16 v1 routes onto v2. `/positions`, `/closed-positions`
and `/v1/market-positions` fold into `/v2/positions?status=`, and `/traded` into
`/v2/user-stats`. It names `/v1/accounting/snapshot` as staying on v1. It says
nothing about `/other`, `/revisions` or the `/` health route, which are in the v1
spec and have no `/v2` path; `/v2/status` reports data freshness, not liveness. The new routes are `user-pnl`, `user-stats`,
`user-volume`, `biggest-winners`, `prices-history`, `resolutions` and `status`.

## Overlap with what polyoxide already calls

The first two are compared in [OBSERVED.md](OBSERVED.md): neither v2 route is a
drop-in replacement for the undocumented host.

| v2 route | polyoxide today |
|----------|-----------------|
| `/v2/user-pnl` | `data.pnl()` on the undocumented `user-pnl-api` host ([../undocumented/INDEX.md](../undocumented/INDEX.md)) |
| `/v2/leaderboard` | `data.leaderboard()` (v1), and `data.rankings()` on the undocumented `lb-api` host |
| `/v2/prices-history` | CLOB `GET /prices-history`, used by `polyoxide clob prices download` |
| `/v2/holders` | `data.holders()`, whose v1 `null` miss the SDK reads as `[]` ([../data/holders.md](../data/holders.md)) |
