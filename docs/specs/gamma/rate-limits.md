# Gamma Rate Limits

Base URL: `https://gamma-api.polymarket.com`

Enforcement: sliding time window. Upstream describes these as IP-based Cloudflare
throttling — over-limit requests are delayed/queued rather than rejected outright.

Source: <https://docs.polymarket.com/api-reference/rate-limits>, fetched 2026-08-05.

## General

| Limit | Window |
|-------|--------|
| 4,000 requests | 10 seconds |

## Endpoint-Specific

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/events` | 500 | 10s |
| `/public-search` | 350 | 10s |
| `/markets` | 300 | 10s |
| `/comments` | 200 | 10s |
| `/tags` | 200 | 10s |
| `/status` (health) | 100 | 10s |

### The health row is published as `/ok`, but that route does not exist here

As with the Data API, upstream spells the health row `Health check (/ok)` and
that path returns **404** on this host:

```
GET https://gamma-api.polymarket.com/status  -> 200  OK
GET https://gamma-api.polymarket.com/ok      -> 404  404 page not found
```

`/status` is the route this host answers on and the one `Gamma::health().ping()`
requests.

### The `/markets` + `/events` group cap is not modelled

Upstream also publishes a 900/10s cap shared across `/markets` + `/events`
listing. polyoxide deliberately does not model it: the per-endpoint caps sum to
300 + 500 = 800, so the group cap can never bind. The
`the_markets_plus_events_group_cap_can_never_bind` test in
`polyoxide-core/src/rate_limit.rs` watches that arithmetic — if either
per-endpoint cap is raised upstream, the omission stops being safe and the shared
bucket has to be added.
