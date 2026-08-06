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

## What the published table cannot tell you

The caps above are enforced by Cloudflare, and Cloudflare has a second gear the
table does not mention. When its rule trips, it answers:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 0

error code: 1015
```

Two properties matter for any client:

1. **It is an IP-scoped block, not a per-route counter.** Once tripped, every
   path on the host returns 1015 — `/trades` is refused because of what
   `/closed-positions` did. A limiter that reasons purely per-path will happily
   keep issuing requests from buckets that are still full.
2. **It is timed, and traffic during the block prolongs it.** Retrying into a
   1015 does not merely fail; it feeds the rule that is blocking you.

The `Retry-After: 0` in that response is the trap. Taken literally it turns a
retry loop into an immediate resend — the failure that prompted this section
burned three attempts in 65ms:

```
WARN Retriable status 429 Too Many Requests on /closed-positions, retry 1 after 0ms
WARN Retriable status 429 Too Many Requests on /closed-positions, retry 2 after 0ms
WARN Retriable status 429 Too Many Requests on /closed-positions, retry 3 after 0ms
ERROR Request failed: Api(RateLimit("error code: 1015\n"))
```

polyoxide therefore treats `Retry-After` as a **lower bound the server may
raise, never lower**, and converts any observed 429 into a cooldown shared by
every request on the limiter. See the "two rate limit layers" section of
`CLAUDE.md`.

### Staying under the rule in the first place

The client's buckets start full (`allow_burst`), so a fresh process may issue
150 `/closed-positions` requests as fast as its concurrency limit allows before
any throttling engages. That is within "150 per 10 seconds" read as an average,
but it is a burst — and Cloudflare's own window is not published. Callers doing
bulk hydration should pace themselves rather than rely on the client's buckets
alone.
