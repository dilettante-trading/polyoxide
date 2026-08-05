# Data API Rate Limits

Base URL: `https://data-api.polymarket.com`

Enforcement: sliding time window. Upstream describes these as IP-based Cloudflare
throttling — over-limit requests are delayed/queued rather than rejected outright.

Source: <https://docs.polymarket.com/api-reference/rate-limits>, fetched 2026-08-05.

## General

| Limit | Window |
|-------|--------|
| 1,000 requests | 10 seconds |

## Endpoint-Specific

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/trades` | 200 | 10s |
| `/positions` | 150 | 10s |
| `/closed-positions` | 150 | 10s |
| `/` (health) | 100 | 10s |

### The health row is published as `/ok`, but that route does not exist here

Upstream's table spells the health row `Health check (/ok)`. That path returns
**404** on this host:

```
GET https://data-api.polymarket.com/      -> 200  {"data":"OK"}
GET https://data-api.polymarket.com/ok    -> 404  404 page not found
```

The `/ok` spelling is boilerplate repeated into every surface's table on that
page. Only `clob.polymarket.com` actually serves `/ok`. The cap belongs to `/`,
which is the route this host answers on and the one `polyoxide-data` requests.

Attaching the cap to `/ok` instead is not a harmless spelling difference: it
leaves the real health route governed only by the 1,000/10s general bucket, a
10x over-permit, while the `/ok` entry caps a route nobody can call.

## Sibling hosts

Two namespaces target sibling hosts that share one `RateLimiter` with the main
Data API client (see `HttpClient::with_base_url`).

| Host | Namespace | Limit | Window |
|------|-----------|-------|--------|
| `user-pnl-api.polymarket.com` | `data.pnl()` | 200 | 10s |
| `lb-api.polymarket.com` | `data.rankings()` | *(none published)* | — |

Upstream publishes the User PNL figure as a host-wide allowance ("User PNL API |
200 req / 10s"), not a per-path one. polyoxide models it as a path rule on
`/user-pnl`, which is exact today because that is the only route it calls on
that host — but note the limiter matches on **path alone and has no host
dimension**, so a future sibling route colliding with a main-host path (`/trades`,
`/positions`) would draw from the wrong bucket.

`lb-api` has no published limit, so `/volume` and `/profit` fall to the shared
general bucket.
