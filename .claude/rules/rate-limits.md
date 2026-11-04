---
paths:
  - "polyoxide-core/src/{rate_limit,capacity,hold,hooks}.rs"
  - "polyoxide-polymarket/src/shared/{limits,signer_limit,throttle}.rs"
  - "polyoxide-binance/src/usdm/{weight,policy}.rs"
  - "polyoxide-binance/src/shared/weight.rs"
  - "polyoxide-binance/src/shared/policy.rs"
  - "polyoxide-binance/src/spot/{weight,policy}.rs"
  - "polyoxide-kalshi/src/predictions/throttle.rs"
  - "**/examples/*{soak,probe,ceiling}*"
  - "polyoxide-polymarket/examples/v2_soak/**"
  - "**/tests/{polymarket_limits,polymarket_throttle,token_cost_throttle,observe}.rs"
  - "polyoxide-test-support/src/soak/**"
---

# Rate limits

Two layers count different things, and a request must satisfy both:

| Layer | Module | Keyed on | Counts | Applies to |
|---|---|---|---|---|
| Cloudflare IP throttling | `polyoxide-core/src/rate_limit.rs` (`WindowQuotaTable`); tables in `polyoxide-polymarket/src/shared/limits.rs` | client IP | requests | every host |
| Per-signer token buckets | `polyoxide-polymarket/src/shared/signer_limit.rs` (feature `clob`) | signer address | orders | CLOB order/cancel only |

## Window quotas

- **A bucket's depth and its refill rate are two spends of one budget.** `quota()` deliberately
  does not call `allow_burst`, leaving governor's default capacity of one token, and
  `WindowQuotaTable` offers no burst setting, so no table can ask for one. Capacity `count`
  refilling at `count/period` reads like a faithful "150 per 10 seconds" and is wrong: a bucket
  that starts full admits its depth plus everything the refill adds, so its first window lets
  through `count + count`. Every table over-permitted by exactly 2x until this was measured.
- Depth is borrowed against the rate: `burst + rate × period ≤ count`, so a burst of `B` costs
  `B` requests of sustained allowance for good. With `rate = count/period` the bound forces
  `B ≤ 0`, which is inherent to token buckets against a sliding-window server, not an artifact of
  this implementation. Minimum depth is therefore also maximum throughput, and the safest shape.
- **Aim under the published count.** `quota()` reserves a tenth (`RESERVED_FRACTION`): the count
  is reachable as a burst, not as a rate. Measured on `/closed-positions` (150/10 s) against the
  live host:

  | Sustained rate | Share of published | Result |
  |---|---|---|
  | 14.9/s | 100% | refused after 15.7 s |
  | 14.25/s | 95% | refused after 17.3 s |
  | 13.5/s | 90% | clean over 180 s, 2,430 requests |

  A one-shot 150 in 0.70 s is accepted, so the table does not overstate the cap: Cloudflare's
  sliding-window estimator does not count the way an interval count does, and nothing outside
  the server can observe the difference. The soak that measured it, `closed_positions_soak.rs`,
  went with Data API v1 (retired upstream on 2026-11-02); it is in git history.
- **Agreement tests assert the effective quota a request resolves to**
  (`RateLimiter::effective_quota`: the general bucket first, then the matched row's buckets), not
  merely that an entry exists. Both tables are pinned this way; the IP tables'
  `documented_*_limits` live in `polyoxide-polymarket/tests/polymarket_limits.rs` and use public
  API only. Tests that only checked presence and ordering are how `/balance-allowance` went
  missing and `/closed-positions` sat at 66x its cap, both undetected.
- **Published tables name routes that 404.** Upstream lists `Health check (/ok)` under every
  surface, but only `clob.polymarket.com` serves it (data's health route is `/v2/status`, gamma's
  `/status`). Probe the path on its host before pinning a row.
- **Where the rows come from.** A surface Polymarket publishes figures for has its rows transcribed
  from https://docs.polymarket.com/api-reference/rate-limits and pinned row for row by the
  `documented_*_limits` tests; `quota()` aims a tenth under each. Where a host publishes no figures
  (two Data v2 routes) or sits behind CloudFront (perps), the rows were measured against the live host over
  distinct URLs and recorded in its `OBSERVED.md` (below): several v2 routes are CDN-cached, and a
  repeated URL is answered by CloudFront without reaching the origin, so a soak that repeats URLs
  reports a clean run at any rate.
- **A bucket checks and sleeps on the runtime's clock.** governor's `until_ready` sleeps on its own
  timer and reads its own clock, so `Bucket::until_ready` (`polyoxide-core/src/rate_limit.rs`)
  loops on `check`, sleeps the time `NotUntil::wait_time_from` reports, and checks again, on the
  runtime of the table's hold (`WindowQuotaTable::with_runtime`). The bucket's governor limiter
  runs on a `Clock` over that runtime's `now()` (`RuntimeClock`), made when the bucket is first
  awaited: on a paused tokio clock a quanta-clocked check would barely move while the sleep
  advances virtual time, and the loop would spin through minutes of it. The bucket's depth, its
  refill and the reserve are unchanged.

## 429s and holds

The buckets model the published quota; a 429 is what the server actually said. They disagree:
Cloudflare's `error code: 1015` is an IP-scoped block with its own window, and arrives as a 429
however many tokens the buckets still hold.

- A 429 holds the whole client. The policy's `Decision::hold` goes to `Throttle::hold`, which
  extends the throttle's `Hold` (`polyoxide-core/src/hold.rs`; for a `RateLimiter`,
  `begin_cooldown` does the same), and every later `acquire` waits it out whatever the path,
  before charging and again after any bucket wait of its own (AD-23), so a hold set meanwhile is
  not sent into. Every layer of one throttle shares one `Hold`; `Hold::with_ceiling` clamps it.
  The hold also carries the throttle's `Runtime` (`Hold::unbounded_on`, `Hold::with_ceiling_on`,
  `Hold::runtime`): it reads its deadline off that clock, and its waits and those of every bucket
  sharing it sleep on it, so a client built on an injected runtime never waits on another.
- The hold is `RetryConfig::retry_delay(0)`, whatever attempt saw the 429; the attempt's own wait
  is `retry_delay(attempt)`. A `425` retries without holding anyone else.
- The send loop applies a policy's hold **before** it decides whether to retry, whatever it
  decides: a request that is out of attempts still publishes what it learned.
- A request that arrives during a hold waits before any bucket and is paced when it ends. One
  already waiting on a bucket when a hold begins takes its token, waits out the hold (AD-23) and is
  released with the others, so that burst is at most the requests in flight (on the send loop,
  the concurrency permits).

Two rules make this work, both mutation-tested (`docs/MUTANTS.md`):

- **`Retry-After` may only extend the wait, never shorten it.** Cloudflare sends one that floors
  to zero; obeying it verbatim once made the retry delay `Duration::from_millis(0)`, so three
  retries landed inside 65 ms and extended the very ban they were waiting on. The floor is the
  client's own exponential backoff, and `polyoxide_venue::retry_delay`, which
  `RetryConfig::retry_delay` calls, is the one place the rule lives.
- **Cooldowns extend, never truncate.** Concurrent requests see the same 429 milliseconds apart;
  taking the newest value would let the smallest delay release everyone early. `Hold::wait`
  re-checks after waking, so a hold extended mid-wait is honoured in full.

A 429 the client retries away is invisible to the caller (the loop logs a WARN and returns `Ok`),
so a harness that counts `Ok` against `Err` reports a clean run straight through sustained
throttling. Detect it through a `tracing` subscriber: `polyoxide_test_support::soak::observe`.

## Per-signer buckets (CLOB orders)

- Each is a `CapacityBucket` (`polyoxide-core/src/capacity.rs`) whose capacity is the tier's
  published burst. Polymarket publishes rate **and** burst for this layer and says burst is the
  bucket's capacity, so copying both is a faithful model of a bucket the server also implements.
  Cloudflare publishes a window quota with no capacity term. Same two numbers, opposite meanings:
  making the two modules "consistent" would reintroduce the bug.
- A `CapacityBucket` refuses an exact cost above its capacity at once (`Refused`), charges an
  inexact one at capacity, and can be resized in place keeping its tokens and its `Hold`; a new
  tier still replaces both signer buckets at full capacity.
- Batch endpoints cost their full size (`POST /orders` N, `DELETE /orders` N, `cancel-all` 1+N),
  so a batch can cost more than the bucket can ever hold: permanently rejected, not throttled.
  `ClobThrottle` refuses it client-side before anything is sent, and clob's `From<ApiError>` turns
  the refusal into `ClobError::BurstCapacityExceeded` (not retriable), rather than letting the
  loop burn attempts on a 429 it would misread as transient. `cancel-all`/`cancel-market-orders`
  costs are not knowable client-side; `TradingRequest::cost_is_exact` flags that.
- The tier starts at `Standard` (tightest) and is adopted from every response's
  `Poly-RateLimit-Tier` header, a 429's included, since it derives from 30-day volume the client
  cannot compute. `account_api().rate_limit_tier()` (`GET /v1/user/rate-limit-tier`) reads it from
  the venue but does not set the throttle's.
- `shared::ClobThrottle` (`shared::clob_throttle()`, `polyoxide-polymarket/src/shared/throttle.rs`)
  composes the two layers over one `Hold`: each attempt is charged one request against
  `clob_limits()` by method and path, then the signer bucket its `shared::signer_cost` names.
  Every trading route names its bucket with `shared::signer_cost`.
- Each table and the throttle has an `_on` twin that keeps time on a given runtime (`clob_limits_on`,
  `gamma_limits_on`, `data_limits_on`, `relay_limits_on`, `perps_limits_on`, `clob_throttle_on`,
  `SignerLimiter::new_on`). Every builder resolves its runtime once with
  `ClientConfig::runtime_or_default` and builds its table or throttle with the twin, so a client
  and its throttle share one runtime; the plain constructors take the target's default.

## Other measured throttles

- **Perps** (`shared::perps_limits`, host behind CloudFront), measured with
  `polyoxide-polymarket/examples/info_soak.rs` over distinct URLs: klines 30, trades 10,
  portfolio 30 and bbo 50 per 10 s, a `/v1/info` catch-all at 10 for the routes not soaked, and a
  client-wide general bucket of 30 set by two mixed validation runs (`docs/specs/perps/OBSERVED.md`,
  `## Rate limits`).
- **Data v2** (`shared::data_limits`): the published v2 table (fetched 2026-11-04): 800/10 s
  general; `/v2/positions` with `/v2/positions/combos`, and `/v2/activity` with
  `/v2/activity/combos`, 200 each pair, one shared bucket per pair, as the same page's combined CLOB
  ledger row is; `/v2/prices-history` 200; `/v2/status` 100. `/v2/trades` keeps the measured 200,
  since the published 300 drew 429s at its pace (2026-11-04); `/v2/user-pnl` and `/v2/holders`,
  which it does not list, keep 400 each. All measured with `polyoxide-polymarket/examples/v2_soak`
  over distinct URLs (`docs/specs/data-v2/OBSERVED.md`), pinned by `documented_data_limits`.
- **Binance**: `usdm::WeightBudget` and `spot::WeightBudget` (`binance.md`), each of whose `new_on`
  keeps its hold and its minute waits (and `usdm`'s funding pace) on a given runtime.
- **Kalshi**: the unsigned client's Read bucket is depth 600 refilling 180 tokens a second (20
  reads a second measured clean, less a tenth). A keyed client's public reads draw on a second
  bucket sized as the unsigned client's, so only `GET /account/limits` draws on the key's bucket,
  which holds its published capacity like the signer buckets (the signed probe ran on the demo
  host; production signed public reads are unmeasured). Both buckets share one `Hold` on the
  client's runtime (`PredictionsThrottle::new_on`).
