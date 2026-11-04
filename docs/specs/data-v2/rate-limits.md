# Data API Rate Limits

Base URL: `https://data-api.polymarket.com` (every route under `/v2`; Polymarket retired
Data API v1 on 2026-11-02).

Enforcement: sliding time window. Upstream describes these as IP-based Cloudflare
throttling — over-limit requests are delayed/queued rather than rejected outright.

Source: <https://docs.polymarket.com/api-reference/rate-limits>, its Data API v2 section,
fetched 2026-11-04. What `shared::data_limits` pins, and the measurements behind each row
it does not take from this table, are in [OBSERVED.md](OBSERVED.md#measured-rate-limits).

## General

| Limit | Window |
|-------|--------|
| 800 requests (all `/v2` endpoints) | 10 seconds |

## Endpoint-Specific

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/v2/trades` | 300 | 10s |
| `/v2/positions`, `/v2/positions/combos` | 200 | 10s |
| `/v2/activity`, `/v2/activity/combos` | 200 | 10s |
| `/v2/prices-history` | 200 | 10s |
| `/v2/status` | 100 | 10s |

Each two-path row is read as one shared allowance, as the same page's combined CLOB ledger
row is. `/v2/trades` drew 429s at the pace its 300 allows, so the client keeps the measured
200 (OBSERVED.md). v1's table, which the sections below were measured against, is in git
history.

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

Two separate defects sat here, and only the first is arithmetic.

**The client spent the published quota twice.** `quota()` gave each bucket a
depth of `count` *and* a refill of `count/period`, and a bucket that starts full
admits its depth plus everything the refill adds — so the first window let
through `count + count`. Fixed by dropping the burst allowance entirely:
capacity is one token, and requests are paced uniformly.

**The published count is not reachable as a sustained rate.** Fixing the
arithmetic alone still failed. `quota()` therefore also reserves a tenth of
every published count, pacing this route at 13.4 req/s.

Measured against the live host with
`closed_positions_soak.rs` (removed with v1, in git history), whose `--rate` flag drove a
chosen rate:

| Shape | Rate | Share of published | Result |
|---|---|---|---|
| one-shot burst of 140 | — | — | clean (0.46s) |
| one-shot burst of 150 | — | 100% | **clean** (0.70s) |
| sustained, pre-fix | ~150 in second 0, then 15/s | 200% first window | 1015 at 0.48s and 0.73s |
| sustained | 14.9/s | 100% | 1015 at 15.70s |
| sustained | 14.25/s | 95% | 1015 at 17.28s |
| sustained | 13.5/s | 90% | **clean over 180s, 2,430 requests** |

Read together these say something the table alone cannot. The two pre-fix soaks
tripped at the same *cumulative* count at different rates — a window cap, not a
rate cap — and a one-shot 150 inside 0.70s is clean, which rules out the
tempting explanation that Cloudflare's window is shorter than the published 10s.
Yet a *sustained* 142.5 per 10s is refused. So the count is reachable as a burst
and not as a rate: Cloudflare's sliding-window estimator does not count the way
a naive interval count does, and nothing outside the server can observe the
difference. Aiming at a published quota is a bug even when the arithmetic is
right.

Note that a 429 the client retries away is **invisible to the caller**: the
retry loops in `polyoxide-core` log a `WARN` and return `Ok`. A harness that
counts `Ok` against `Err` reports a clean run straight through sustained
throttling, so the examples detect throttling with a `tracing` subscriber
instead.

**What still falls to the caller.** The limiter is per-process, and 1015 is
scoped to the IP. Two polyoxide processes behind one address each believe they
hold the whole 135/10s allowance, and neither can see the other — so a pair of
them runs at 180% of a rate that is only clean at 90%. Anything running a fleet
has to divide the allowance across it; the client cannot do that for you. Within
a single process, no self-pacing is needed for this cap.
