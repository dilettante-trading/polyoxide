# CLOB Rate Limits

Base URL: `https://clob.polymarket.com`

Enforcement: sliding time window. Requests are throttled (delayed/queued) rather than immediately rejected when limits are exceeded.

**This page covers only the Cloudflare IP-based layer.** Order and cancellation
requests are *additionally* evaluated against per-signer token buckets that count
orders rather than requests — see
[trading-rate-limits.md](trading-rate-limits.md). A request must satisfy both.

Source: <https://docs.polymarket.com/api-reference/rate-limits>, re-fetched 2026-07-25.
An earlier revision of this page carried stale trading numbers (`POST /order` at
3,500/10s rather than 5,000, `DELETE /order` with no sustained window at all).
`RateLimiter::clob_default()` in `polyoxide-core` is pinned against the table
below by the `documented_limits` tests.

## General

| Limit | Window |
|-------|--------|
| 9,000 requests | 10 seconds |
| `GET /ok` (health) | 100 / 10s |

## Account

| Endpoint | Limit | Window |
|----------|-------|--------|
| Get balance allowance | 200 | 10s |
| Update balance allowance | 50 | 10s |

Upstream names these "GET balance allowance" and "UPDATE balance allowance"
without giving paths. The SDK reaches them as `GET /balance-allowance` and
`GET /balance-allowance/update` — the update route is a distinct *path*, not a
distinct method, so the limiter keys on the path. Note `/balance-allowance/update`
also matches the `/balance-allowance` prefix at a segment boundary, so the
tighter rule has to be ordered first.

## Market Data

| Endpoint | Limit | Window |
|----------|-------|--------|
| `GET /book` | 1,500 | 10s |
| `POST /books` | 500 | 10s |
| `GET /price` | 1,500 | 10s |
| `POST /prices` | 500 | 10s |
| `GET /midpoint` | 1,500 | 10s |
| `POST /midpoints` | 500 | 10s |
| `GET /prices-history` | 1,000 | 10s |
| `GET /tick-size` | 200 | 10s |

The batch forms are 3x tighter than their singular siblings and are separate
rules: a prefix match on `/book` must not capture `/books`.

## Trading (Dual Limits)

| Endpoint | Burst | Burst Window | Sustained | Sustained Window |
|----------|-------|--------------|-----------|------------------|
| `POST /order` | 5,000 | 10s | 120,000 | 10 min |
| `DELETE /order` | 5,000 | 10s | 120,000 | 10 min |
| `POST /orders` | 2,000 | 10s | 21,000 | 10 min |
| `DELETE /orders` | 2,000 | 10s | 15,000 | 10 min |
| `DELETE /cancel-all` | 250 | 10s | 6,000 | 10 min |
| `DELETE /cancel-market-orders` | 1,500 | 10s | 21,000 | 10 min |

## Ledger

The first row is a cap **shared across the listed endpoints as a group** — one
allowance consumed jointly, not 900 each. Individual endpoints may also carry a
tighter per-endpoint cap (`/notifications` does), in which case both apply.

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/trades`, `/orders`, `/notifications`, `/order` (shared group cap) | 900 | 10s |
| `/data/orders` | 500 | 10s |
| `/data/trades` | 500 | 10s |
| `/notifications` (per-endpoint cap, on top of the group) | 125 | 10s |

**Method scoping is an inference, not published.** The group names `/order` and
`/orders`, which the trading table above allows 5,000 and 2,000 per 10s. Both
tables can only hold at once if the group cap governs the ledger *reads*; a
900/10s cap across all methods would make the published trading burst
unreachable. The SDK therefore scopes the group to `GET`. If upstream ever
clarifies otherwise, `clob_default()` is where to change it.

## Auth

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/auth/*` | 100 | 10s |

## Other hosts

For reference — these are enforced by the sibling APIs, not the CLOB host.

| Host / Endpoint | Limit | Window |
|-----------------|-------|--------|
| Gamma general | 4,000 | 10s |
| Gamma `/events` | 500 | 10s |
| Gamma `/markets` | 300 | 10s |
| Gamma `/markets` + `/events` listing (shared) | 900 | 10s |
| Gamma `/public-search` | 350 | 10s |
| Gamma `/comments`, `/tags` | 200 | 10s |
| Data API general | 1,000 | 10s |
| Data API `/trades` | 200 | 10s |
| Data API `/positions`, `/closed-positions` | 150 | 10s |
| User PNL API | 200 | 10s |
| Bridge API general | 50 | 10s |
| Relayer `/submit` | 25 | 1 min |

The Gamma `/markets` + `/events` shared cap of 900/10s is not modelled in
`gamma_default()` because it can never bind: the per-endpoint caps sum to 800.
