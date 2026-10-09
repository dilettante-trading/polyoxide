---
title: 'Stories 3.1, 3.2 and 3.3: One send loop, the public window-quota table, and capacity buckets with Polymarket''s composed throttle'
type: 'refactor'
created: '2026-10-09'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '9cca9971a370a66e25827c6632ceb543e0c7a055'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-3-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:**
- Core's send-and-retry loop exists twice: `Request::send_raw` and `get_bytes`.
- Its retry set (429 and 425) is fixed in `HttpClient::should_retry`, and its 429 feedback is a hand-placed `note_rate_limited` call.
- `HttpClient` names `RateLimiter`.
- The five Polymarket limit tables can be built only inside core, from private helpers, and their `documented_*_limits` tests read private fields.
- The cooldown lives inside `RateLimiter`, so no other layer can share it.
- The per-signer layer is a governor bucket that can neither be resized nor share a hold. Nothing composes it with the IP layer.
- A venue with token-cost buckets (CAP-2) would have to edit core.
- The decode-and-log block (H3) is copied four times.

**Approach:** Core's primitives land in one bundle. Clob's request path moves onto them in Story 3.4.
- **Story 3.1:**
  - Three hook traits (`Throttle`, `RetryPolicy`, `Authenticator`).
  - One loop, `HttpClient::send`, which runs AD-8's order.
  - `HttpClient` holds one `Arc<DynThrottle<'static>>` and one `Arc<DynRetryPolicy<'static>>`.
  - `RateLimiter` implements `Throttle`.
  - Polymarket's policy (429 and 425) lives in a new `polyoxide_core::polymarket` module. Gamma, data and perps install it.
  - `Request::send_raw` and `get_bytes` run on the loop.
  - One `decode_json` replaces the four H3 copies.
  - DRIFT R10 lands as its own commit.
- **Story 3.2:**
  - A public `WindowQuotaTable` builder builds `RateLimiter`, with `effective_quota` and `rows()` for inspection.
  - The five tables are rebuilt as `polymarket::{clob,gamma,data,relay,perps}_limits()`.
  - In their own commit, every test that names a table asserts only through public API.
  - A location-only commit moves those tests to `polyoxide-core/tests/polymarket_limits.rs`.
- **Story 3.3:**
  - A public `Hold`: the cooldown, extracted verbatim, with a ceiling parameter.
  - AD-23's re-check after the throttle's own waits.
  - A public `CapacityBucket`, offering refusal, `resize` and provisional sizing.
  - `SignerLimiter` is rebuilt on `CapacityBucket`, keeping its API.
  - `polymarket::ClobThrottle` composes the IP table and the signer layer over one `Hold`. It is built and proven in core; clob does not use it until Story 3.4.
  - The CAP-2 token-cost test lives outside core.
- **First, before any line moves:** record and prove mutants for `RESERVED_FRACTION` and the signer layer's `allow_burst`.

**Decisions (Claude, as the user's delegate, 2026-10-09; one-line reasons):**

*Story 3.1*
- **Hook signatures.** They are in Design Notes, and the rest of Epic 3 builds on them.
  - **[RISK]** `RetryPolicy::decide` takes a third argument, the loop's `&RetryConfig`, so a policy can compute core's 429 hold, `retry_delay(0)`, which is not the attempt's floor. The spine fixes only the return type. Record this as amendment A3-1.
  - **[RISK]** `Authenticator::sign` returns `Result<(), ApiError>`. Story 3.4 may need a richer error for clob's L1 signing. Record this as amendment A3-2.
- **The retry rule moves verbatim** into `RetryConfig::retry_delay(attempt, retry_after)`, a new `pub` method. The loop's floor, `should_retry`, `note_rate_limited` and both policies call it. Story 3.10 (R4) replaces the parser.
- **Polymarket's policy, for 429:**
  - it holds `retry_delay(0)`, whatever retries are left;
  - its outcome is `Retry(Duration::ZERO)`, so the loop's floor is the whole wait, exactly today's `retry_delay(attempt)`.
- **Polymarket's policy, for 425 and the rest:**
  - a 425 retries with no hold;
  - a 2xx is `Done`;
  - every other status is `Fail`.
- **Core's `DefaultRetryPolicy`** has the 429 arm only (AD-17).
- **The loop never retries past `max_retries`,** whatever a policy returns.
- **[RISK] `HttpClientBuilder`'s default policy no longer retries 425** for a client built directly on it. AD-17 mandates this, and no DRIFT row covers it. Every Polymarket client installs the Polymarket policy, so no shipped client changes. Record the change for 4.11's release notes.
- **[RISK] `Fail` returns the final response.**
  - `HttpClient::send` returns `Ok(Response)` for both `Done` and `Fail`. Callers decode a non-2xx response as today.
  - Story 3.11 moves `Fail` to an `ApiError` that carries the status, headers, body and `Retry-After`.
  - Only `ApiError::Refused(Refused)` is added now, classed `InvalidRequest`, because the loop must return something for `Err(Refused)`. Variants may change in S1 (AD-16).
- **In 3.1, `RateLimiter::acquire` stays as it is:** cooldown, then the general bucket, then the row's buckets. AD-23's re-check arrives in 3.3, with `Hold`.
- **The loop passes the real method.** It sends `Some(&Method::GET)` where core's loop sent `None`. Every row that gamma, data, perps and `get_bytes` reach is method-agnostic, so the same row matches.
- **The transitional `acquire_rate_limit`** sends `None` as `GET`. Its only `None` callers are the gamma and data pings (method-agnostic rows) and relay's GET loops (an empty table).
- **Four transitional methods are kept, unchanged in signature and behaviour:** `acquire_rate_limit`, `acquire_concurrency`, `should_retry` (still 429 and 425) and `note_rate_limited`.
  - They are reimplemented over the throttle.
  - Clob (3.4), relay (3.5), Binance (3.6) and the R8 sends (3.5) still use them.
  - **Owner:** bundle G (clob, relay, Binance and the R8 sends) removes each of them when it moves the method's last caller, and lists it in `docs/s1-removals.md` with its replacement, `HttpClient::send`.
  - F only rewrites their docs to say so.
  - `retry_after_header`, `RequestError`, `Request` and `impl_api_error_conversions!` also stay (3.10 and 3.11).
- **`with_rate_limiter(RateLimiter)` stays on `HttpClientBuilder` and `PerpsBuilder`,** and forwards to the new `with_throttle`. AC 3.1 binds the `HttpClient` struct, not the builder.
- **H3 lands in 3.1 for all four copies, because the AC says so.**
  - The single `decode_json(path, text)` logs one ERROR line, `Failed to decode {path}: {err}: {truncated body}`, under `polyoxide_core`.
  - Clob, relay and Binance call it now. Their loops wait for 3.4–3.6.
  - Their decode failures now log under `polyoxide_core`, and core's and clob's two lines become one. Record this for 4.11.
- **The hold warning, in R10's commit.** A hold that is not a retry warns once: `Status <code> on <path>, no retry left: every request held <ms>ms`. AD-8 asks for this. The line does not contain `Retriable status`, so the soak verdicts do not change.

*Story 3.2*
- **[RISK] `RateLimiter` keeps its name in S1.**
  - `WindowQuotaTable` is the public builder that builds it.
  - This keeps the protected `test_rate_limiter_debug_format` assertion, and the public `acquire` and `begin_cooldown`.
  - The rename, if wanted, goes into S2's manifest.
  - 3.2 removes only the five `RateLimiter::*_default` constructors.
- **Table API.**
  - The table's own `acquire(path, Option<&Method>)` keeps `None`.
  - `effective_quota(&Method, path)` returns the general bucket first, then the matched row's buckets in await order.
  - `rows()` returns every row in match order, with its pattern, method, `Matching` (`Prefix` or `Exact`) and buckets.
  - `EffectiveQuota { bucket: BucketId, count, period }` offers `interval()` and `admitted_in_one_window()`, both read from the real governor quota.
  - `WindowQuotaTable::paced_interval(count, period)` is the one place the pacing formula lives.
  - Every bucket is depth 1 with a tenth reserved. There is no burst setting, so D1 cannot regress through the builder.
- **[RISK] Scope of the public-API rewrite.** AC 3.2 names the 21 `documented_*` tests. AD-12 also covers every other test that names a Polymarket table, so the rewrite takes 11 more `tests` tests and `quota_arithmetic::every_configured_bucket_satisfies_the_quota_it_publishes`. The 26 tests already on public API change only their constructors. Golden vectors, names and assertions stay. The general bucket is sliced off before comparing. 48 tests move.

*Story 3.3*
- **`Hold` is today's cooldown, extracted verbatim** into `polyoxide-core/src/hold.rs`: extend-only, re-checked after waking, with a poison-tolerant lock. Rows (c) move with it.
  - `Hold` is a cheap `Clone`. `Hold::unbounded()` keeps today's behaviour and is the default.
  - `Hold::with_ceiling(d)` clamps every `extend` (AD-23). Binance passes 3 days in 3.6.
  - Adding to the deadline saturates rather than panics.
  - `RateLimiter` and the window-table builder take a `Hold`. `begin_cooldown` and `Throttle::hold` both call `extend`.
- **AD-23's re-check lands in 3.3.** Every core throttle waits out the hold before charging, and waits again if the hold moved during its own bucket waits: `RateLimiter`, `CapacityBucket` and `ClobThrottle`. Today a request that waited on a bucket could go out into a hold set meanwhile. Name the change in the commit message.
- **[RISK] `CapacityBucket` is hand-written, not built on governor.** governor can neither resize a bucket in place nor report its tokens, and AC 3.3's `resize` keeps the tokens.
  - Behaviour: it starts full; it holds at most `capacity`; it refills `refill` tokens a second; `acquire(units, exact)` waits for `units` tokens and then takes them.
  - An exact cost above `capacity` returns `Refused` at once.
  - `resize(capacity, refill)` keeps the tokens, clamped, keeps the hold, and confirms the sizing.
  - D1 holds through its meaning: the capacity is the published burst, and the moved signer tests pin it.
- **Costs that are not exact.** An inexact cost (`cancel-all`) above capacity is charged at capacity and never refused. Today those cost 1, so nothing changes.
- **Provisional sizing.** `CapacityBucket::provisional(..)` makes an above-capacity cost wait until `confirm()` or `resize()`, then re-evaluates it. It never returns `Refused` while provisional.
- **The signer layer is confirmed, not provisional.** The protected `an_over_capacity_batch_is_rejected_immediately_not_queued` and clob's `:3495` require an immediate refusal.
- **[RISK] A new tier replaces both signer buckets at full capacity, as today, and keeps the shared `Hold`.**
  - Adopting it with `resize` would keep the tokens instead. That is more conservative, but it is a behaviour change: clob's tier-up test at :3581 would wait about 0.5 s, and no DRIFT row covers it.
  - `resize` is still exercised by the `CapacityBucket` tests and by CAP-2.
- **`SignerLimiter` keeps its path and public API** (`new`, `tier`, `last_status`, `observe`, `acquire` returning `BurstCapacityExceeded`). It gains `with_hold(Hold)`. Clob calls it until 3.4. 3.3 removes nothing.
- **`polymarket::ClobThrottle::new(table: RateLimiter, signer: SignerLimiter)`** runs: wait the hold, charge the IP table 1 (request-counting, by method and path), charge the signer bucket named by a `costs` entry, then re-check the hold.
  - The refusal comes after the IP charge, as in clob today.
  - `observe` reads `RateLimitStatus` on every status, so a tier on a 429 is adopted.
  - `hold` extends the one shared `Hold`.
  - `polymarket::clob_throttle()` builds the pair over one `Hold`.
  - `polymarket::{CLOUDFLARE, SIGNER_ORDER, SIGNER_CANCEL}: LayerId` and `polymarket::signer_cost(TradingRequest) -> Cost` are ready for 3.4.
- **[RISK] The CAP-2 test lives in `polyoxide-test-support/tests/token_cost_throttle.rs` and never writes "Kalshi".**
  - It is outside core and uses only public API.
  - S2's CAP-7 gate greps test-support for venue ids, and Kalshi registers in S3, so the word would trip the gate.
  - The test's doc names the shape instead: a per-account venue with separate read and write buckets and integer costs.

*The two new mutant rows*
- **`RESERVED_FRACTION` needs a pinning test first.**
  - Every existing test derives its ceiling from the constant (`rate_limit.rs:273`) or from `sustained_slots` (`:808`), so a value mutant survives them all.
  - A new golden test, `a_published_150_per_10s_admits_135_per_window` in `quota_arithmetic`, pins the measured 90%.
- **The signer layer's `allow_burst` is already pinned.** Dropping it fails `a_batch_within_capacity_is_admitted` (:470), `adopting_a_higher_tier_admits_a_batch_that_was_impossible` (:479), `the_order_and_cancel_buckets_are_independent` (:495) and `batch_cost_is_charged_in_full_not_as_one_request` (:517). This is predicted by reading, and is to be proved.

## Boundaries & Constraints

**Always:**
- **Behaviour.** Gamma, data and perps behave as today:
  - the same rows match, and the same quotas pace;
  - 429 and 425 are retried, and no other status is;
  - every 429 holds `retry_delay(0)`, the last attempt included;
  - a 425 holds nothing;
  - `Retry-After` only lengthens a wait;
  - the permit is taken before `acquire` and released before sleeping;
  - the retry WARN keeps its text and its `polyoxide_core` target;
  - error decoding is unchanged.

  Clob, relay and Binance behave as today. Their suites (clob `mock_api` :3495, :3533, :3552 and :3581 included) pass with names and assertions unchanged.
- **NFR7.** Moved tests keep their names and assertions.
  - Report per-suite counts before and after every commit that moves or rewrites tests, from `cargo test -p <crate> --all-features <target> -- --list`.
  - Re-prove every `docs/MUTANTS.md` row whose line or test moves.
  - Update `.github/scripts/tests/test_mutants_ledger.py` in the same commit.
- **Commit boundaries.** Each is its own commit:
  0. the two new mutant rows, before anything moves;
  1. the 3.1 loop and hooks;
  2. H3;
  3. DRIFT R10, named in the message;
  4. the 3.2 builder and tables;
  5. the 3.2 public-API rewrite;
  6. the 3.2 location-only move;
  7. 3.3's `Hold` and re-check;
  8. 3.3's `CapacityBucket` and signer rebuild;
  9. 3.3's `ClobThrottle`;
  10. 3.3's CAP-2 test.
- **Removals.** Each removed public item goes in `docs/s1-removals.md`, with its story and the replacement.
- **Disk.** Build with `CARGO_INCREMENTAL=0` and `-j 4`. Run `scripts/api_removals.py` at most once, after commit 4 or at the end, then delete `target/semver-checks`.
- **dynosaur.** dynosaur 0.3.1 (MSRV 1.84, in the local registry; it carries supertraits through to the erased trait) is added to `[workspace.dependencies]` and core. Use the exact form `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`: `Send + Sync`, never `'static`, no associated consts.
- **Rustdoc.** `pub` docs never link `pub(crate)` items.
- **Commits.** Commit at each boundary above, on this branch only: never push, tag or switch branches. End every message with the two trailer lines `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01D1HjfRG5rXDy97K86gGckK`. Each commit must build and pass the suites it touches.
- **After bundle E (b515775).** The Code Map was taken before E landed. E rewrote `polyoxide-test-support` and data's and perps' soak examples (data's `examples/common/mod.rs` is gone, and the observer is now `polyoxide_test_support::soak::observe`), so locate each site by name.

**Never:**
- Move clob's request path onto the loop or `ClobThrottle`, or change relay's or Binance's loops beyond the H3 call, or gamma's `post_json` and the gamma, data and clob pings (3.4–3.6, R8).
- Add `allow_burst` to a window quota, drop it from the signer layer, or change `RESERVED_FRACTION`, the tier table, or any table's rows, counts, methods or order.
- Retry 5xx or 408, or narrow Polymarket's policy.
- Unify `RetryConfig::backoff` with the socket `Backoff` (D13).
- Give a venue crate a direct governor dependency.
- Remove `retry_after_header`, `RequestError`, `impl_api_error_conversions!` or any `SignerLimiter`, `Tier` or `TradingRequest` item, or add a shim or re-export for a removed item.
- Hand-edit `docs/ARCHITECTURE.md` or a generated region.
- Start before bundle E (2.8–2.10) has merged. E is editing `polyoxide-test-support/` and the data and perps soak examples. Its `test-support/tests/observe.rs` drives core into a 429 and must stay green.

## I/O & Edge-Case Matrix

The rows assume Polymarket's policy and `max_retries = 3`. "Retry left" means `retries_left > 0`.

| Scenario | Input | Hold | Outcome |
|---|---|---|---|
| Success | 2xx | none | `Done`. Return the response |
| Throttled, retry left | 429 | `retry_delay(0)` | WARN `Retriable status 429 Too Many Requests on <path>, retry <n> after <ms>ms`. Release the permit, sleep `retry_delay(n)`, retry |
| Throttled, no retry left | 429 | `retry_delay(0)` | WARN `… no retry left …`. Return the response, which the caller decodes as `RateLimit` |
| Engine restart | 425 | none | Retry on the floor. Siblings are not held |
| Server or timeout | 5xx or 408 | none | `Fail`. Return the response, with no retry |
| Transport error | no response | none | `observe` and `decide` skipped, no retry. `ApiError::Network`, classed `Network` |
| `Retry-After: 0`, or `2` | 429 | `retry_delay(0)` | The sleep is `max(backoff(n), Retry-After)` |
| A `with_base_url` sibling 429s | 429 on a sibling host | the shared throttle holds | The next request on the main host waits |
| Default policy | 425 | none | `Fail`, with no retry |
| Signer batch within capacity | an exact `costs` entry, N ≤ capacity | none | IP layer charged 1, signer charged N. Waits for N tokens |
| Exact batch above capacity | N > capacity, confirmed | none | `Refused { layer, units, capacity }` before sending. `ApiError::Refused`, never retried |
| Inexact cost above capacity | `exact: false` | none | Charged at capacity, never refused |
| Provisional, above capacity | N > provisional capacity | none | Waits until `confirm` or `resize`, then is charged or refused |
| Hold on `ClobThrottle` | `hold(d)` | `d` | Both layers wait `d`. A hold extended mid-wait is honoured |
| Tier header, on any status | `Poly-RateLimit-Tier: gold` | unchanged | Both buckets replaced at full Gold capacity. The hold is kept |
| `resize(c, r)` | a live bucket | unchanged | Tokens clamped to `c`, the hold kept, sizing confirmed |
| Ceiling | `extend(10 days)` with a 3-day ceiling | 3 days | Held 3 days |

</frozen-after-approval>

## Code Map

- **`polyoxide-core/src/client.rs`**
  - Fields and methods:
    - `HttpClient` :32-41, with `rate_limiter` :38 and `#[derive(Debug, Clone)]`, so a manual `Debug` is needed;
    - `with_base_url` :70-75;
    - `acquire_rate_limit` :78-82;
    - `acquire_concurrency` :89-97;
    - `should_retry` :126-137;
    - private `retry_delay` :145-155 (rule (b) :149, :154);
    - `note_rate_limited` :171-178;
    - `get_bytes` :194-238, a loop copy (`note_rate_limited` :215, WARN :219).
  - The builder: `HttpClientBuilder` :257-353, with `with_rate_limiter` :298.
  - Tests: 29 in all, the rule (b) tests :509 and :532 among them.
- **`polyoxide-core/src/request.rs`**
  - `RequestError` :57-60.
  - `Request::send` :105-120, with H3 at :108-119.
  - `send_raw`'s loop :135-181 (`note_rate_limited` :155, `should_retry` :157, WARN :160).
  - 14 tests.
- **`polyoxide-core/src/rate_limit.rs`**
  - Internals:
    - `MatchMode` :17 (`Exact` is dead code);
    - `Bucket` :41-56;
    - `matches` :72-91;
    - `RateLimiter` :98-122, holding its cooldown slot :121;
    - `quota()` :165-167;
    - `sustained_slots` :177-180, with the reserve at :178;
    - `RESERVED_FRACTION` :186;
    - the private builders :309-347;
    - `begin_cooldown` :360-366 (rule (c) :363), `await_cooldown` :381-396, `acquire` :403-412;
    - `resolve_specs` :419-427.
  - Tables:
    - clob :448-532 (27 rows; ledger :454);
    - gamma :553-570;
    - data :599-626;
    - relay :631-639;
    - perps :666-691.
  - `RetryConfig` :694-730.
  - **63 tests:**
    - `quota_arithmetic` 3 (:244, :261, :284; the ceiling derived from the constant at :273);
    - `agreement` :732-827 (`sustained_slots` at :808);
    - `documented_data_limits` 6, `_gamma_` 4, `_perps_` 4, `documented_limits` 7;
    - `tests` 33, of which 20 name a table and 11 read internals;
    - `cooldown_tests` 6 (:1907, :1925).
- **`polyoxide-core/src/signer_limit.rs`**
  - The module doc :1-13 links `RateLimiter`.
  - Types:
    - `Tier` :29-108, the published table at :78-89;
    - `TradingRequest` :111-176 (`cost` :157, `cost_is_exact` :170);
    - `RateLimitStatus` :187-221;
    - `BurstCapacityExceeded` :229-251.
  - The governor buckets:
    - governor `DirectLimiter` :253;
    - `Buckets::for_tier` :265-281, with `.allow_burst(..)` at **:272**.
  - `SignerLimiter` :296-404:
    - `observe` :349-365 replaces the buckets at full capacity at :362;
    - `acquire` :373-402 maps governor's `InsufficientCapacity` to `BurstCapacityExceeded`.
  - **22 tests:** `limiter_tests` 8 (:420-533), `status_tests` 4, `tests` 10.
- **`polyoxide-core/src/lib.rs`** — re-exports :66-76; the `Classify` assertion :84-90, which `Refused` joins. Core's lib has 154 tests with all features, and `tests/mock_request.rs` has 10.
- **`polyoxide-core/src/error.rs`** — `ApiError` :5-38. `every_variant_classifies` gains a `Refused` row.
- **Callers:**
  - gamma: `client.rs:153`. Left alone: `api/health.rs:27-47`, its test `:73` (`gamma_default`, which 3.2 switches) and `api/markets.rs:132-164`.
  - data: `client.rs:283`, siblings :292-293; `api/accounting.rs:23`. Left alone: `api/health.rs:37-55`.
  - perps: `client.rs:81`, :106, :129.
  - clob: `client.rs:1001`, `request.rs:217` (signer `acquire`) and `:268` (signer `observe`). Clob builds `SignerLimiter::new()` at `client.rs:1034`.
  - relay: `client.rs:2032`.
- **H3 copies:**
  - core `request.rs:108-119`;
  - clob `request.rs:178-191`, above its cited :262, which shifts;
  - Binance `usdm/request.rs:71-79`, above its cited :136-149, which shift;
  - relay `client.rs:1866-1876`, below its cited lines.
- **Protected mock tests:**
  - data `mock_api.rs`: helpers :1483-1506, :1509, :1532;
  - perps `mock_api.rs` :505 and :544;
  - core `mock_request.rs` `retry_releases_permit_during_backoff`;
  - clob `mock_api.rs` :3495, :3533, :3552, :3581.
- **Ledger:**
  - `docs/MUTANTS.md`, rows (a)–(e) and "not yet covered";
  - `.github/scripts/tests/test_mutants_ledger.py`, `SNIPPETS` :22-65;
  - `docs/s1-removals.md` reads "None yet";
  - `deferred-work.md` (from spec 1-6/1-8) asks for these two rows and Binance's 418 row.
- **Docs:**
  - CLAUDE.md :13, :178, :186, :195, :200, :264, :352;
  - `polyoxide-core/README.md`, a doctest;
  - `docs/specs/clob/rate-limits.md:15`, `clob/trading-rate-limits.md`, `data/rate-limits.md:48`, `gamma/rate-limits.md:46`, `data-v2/OBSERVED.md:206` and `perps/OBSERVED.md:164`.
- **Bundle E's area:**
  - `polyoxide-data/examples/v2_soak/main.rs:56,299,368,601` uses `RateLimiter::data_default()` and `acquire`;
  - the soak observer's comment at data `examples/common/mod.rs:167`;
  - `polyoxide-test-support` (its features `query` and `soak`, and `tests/observe.rs`).

## Tasks & Acceptance

**Execution:**
- [x] **Commit 0: the two new mutant rows.**
  - Add the golden test `a_published_150_per_10s_admits_135_per_window` to `rate_limit.rs::quota_arithmetic`. It asserts `admitted_in_one_window(&quota(150, 10 s), 10 s) == 135`.
  - Record row (j), the reserved tenth. Its mutants are:
    - `RESERVED_FRACTION = 20`, which fails the new test;
    - `let target = count;` at :178, which fails the new test and :261.
  - Record row (i), the signer layer's `allow_burst`. Its mutant is "delete `.allow_burst(..)` at `signer_limit.rs:272`", which fails the four `limiter_tests` in the Decisions.
  - Prove both rows, add their snippets, and mark those two items of the 1-6/1-8 deferred entry done.
- [x] **Commit 1, Story 3.1: the loop and its hooks.**
  - `Cargo.toml`: add `dynosaur = "0.3.1"`. Core takes it, plus `tracing-subscriber` as a dev-dependency.
  - New `src/hooks.rs` holds the Design Notes items, `NoThrottle` and `DefaultRetryPolicy`.
  - `ApiError::Refused` (`is_retriable` false, `InvalidRequest`), with its `every_variant_classifies` row. `Refused` derives `Error`, implements `Classify` and is listed in `lib.rs`.
  - `RetryConfig::retry_delay`.
  - `HttpClient` holds `throttle` and `policy`, and gets a manual `Debug`.
  - The builder gains `with_throttle` and `with_retry_policy`. `with_rate_limiter` forwards.
  - New `src/send.rs` holds `HttpClient::send`, the loop. `Request::send_raw` and `get_bytes` call it.
  - The four transitional methods are reimplemented, with docs naming who removes them.
  - `impl Throttle for RateLimiter`.
  - New `src/polymarket/` holds `PolymarketRetryPolicy`. The gamma, data and perps builders install it.
- [x] **Commit 1: new tests** in `polyoxide-core/tests/send_loop.rs`:
  - `each_attempt_runs_acquire_sign_send_observe_decide_hold_in_order`;
  - `acquire_waits_for_the_permit`;
  - `sign_runs_on_every_attempt_with_its_number`;
  - `observe_sees_the_last_attempt`;
  - `a_zero_wait_still_sleeps_the_floor`;
  - `a_429_with_no_retry_left_still_holds`;
  - `the_429_hold_is_retry_delay_zero_not_the_attempts_wait`;
  - `a_425_retries_without_holding`;
  - `the_default_policy_does_not_retry_425`;
  - `a_5xx_and_a_408_are_not_retried`;
  - `a_transport_error_is_not_retried_and_skips_observe_and_decide`;
  - `a_refused_charge_sends_nothing`;
  - `get_bytes_retries_a_429_and_holds`.

  Data `mock_api.rs` gains `a_429_on_the_pnl_host_holds_the_data_host`.
- [x] **Commit 1: `MUTANTS.md` and the ledger.**
  - (a) moves to the loop's `hold` call, with the mutant "call `hold` only on `Retry`".
  - New "(a), the policy": the 429 arm's hold, with the mutant "`hold` only when `retries_left > 0`". Both are pinned by data :1532 and `a_429_with_no_retry_left_still_holds`.
  - (b) moves to `RetryConfig::retry_delay`.
  - New (f): `observe` runs on every response, with the mutant "`observe` only when `retries_left > 0`".
  - New (g): the floor, with the mutant "sleep `wait`", pinned by `a_zero_wait_still_sleeps_the_floor`, data :1509 and perps :544.
  - Drop `get_bytes` from "not yet covered".
- [x] **Commit 1: CLAUDE.md and the spine.**
  - Rewrite CLAUDE.md :13, :178, :195 and :200 to describe the loop and the policy. Clob's and relay's four hand-written loops keep `note_rate_limited` before `should_retry` until 3.4 and 3.5. Add a short "One send loop" paragraph.
  - New `…/spine-amendments/epic-3.md` holds A3-1 and A3-2.
- [x] **Commit 2, H3.**
  - Add `pub fn decode_json<T: DeserializeOwned>(path: &str, text: &str) -> Result<T, serde_json::Error>` to `send.rs`.
  - Core's `Request::send`, clob's `Request::send`, Binance's `WeightedRequest::send` and relay's `post_json` call it, each keeping its own error mapping.
  - Re-cite clob :262 and Binance :136/:149.
- [x] **Commit 3, DRIFT R10.**
  - The loop warns once on a hold that is not a retry.
  - New tests: `a_retry_warns_once_under_polyoxide_core` (exact text and target prefix, for 429 and 425) and `a_hold_with_no_retry_left_warns`.
- [x] **Commit 4, Story 3.2: the builder and tables.**
  - The `WindowQuotaTable` builder:
    - general bucket;
    - shared `BucketId`s;
    - prefix and exact rows, with `Option<Method>`;
    - first match wins;
    - `build() -> RateLimiter`;
    - `paced_interval`.
  - `RateLimiter` gets `effective_quota` and `rows()`, and the new public types `EffectiveQuota`, `QuotaRow` and `Matching`.
  - New builder tests in core.
  - `polymarket/limits.rs` holds the five tables, row for row. The callers switch.
  - Remove the five `RateLimiter::*_default` methods and list them. The predicted keys are `polyoxide-core inherent_method_missing: RateLimiter::{clob,gamma,data,relay,perps}_default (src/rate_limit.rs)`; confirm them in the gate's output.
  - Update CLAUDE.md :186, :264 and :352, the README, the spec docs and v2_soak.
  - Re-cite rows (c), (d) and (j).
- [x] **Commit 5, the public-API rewrite.**
  - `agreement` uses `effective_quota` and `paced_interval`.
  - The 21 `documented_*` tests, the 11 internal-reading `tests` tests and `every_configured_bucket_…` use `rows()`, `effective_quota`, `BucketId` (where `Arc::ptr_eq` was used) and `admitted_in_one_window()`.
  - Report the counts, and re-prove rows (c) and (d).
- [x] **Commit 6, the location-only move.** The 48 tests and `agreement` move to `polyoxide-core/tests/polymarket_limits.rs`, keeping their module names. Core's lib goes from 154 to 106, plus the new tests. Re-cite rows (c) and (d), and fix the path in the gamma `rate-limits.md`.
- [x] **Commit 7, Story 3.3: the hold.**
  - New `src/hold.rs` holds `Hold`, with `unbounded`, `with_ceiling`, `extend` and `wait`, moved from `RateLimiter`.
  - `RateLimiter` holds a `Hold`. The builder accepts one.
  - `acquire` re-checks the hold after its bucket waits.
  - New unit tests:
    - `a_hold_set_during_a_bucket_wait_is_honoured`;
    - `the_ceiling_clamps_a_longer_hold`;
    - `a_hold_never_shortens`, on `Hold` directly;
    - `a_huge_delay_saturates_rather_than_panics`.
  - Re-cite rows (c) at `hold.rs`; their tests stay in `polymarket_limits.rs`.
- [x] **Commit 8: the capacity bucket and the signer layer.**
  - New `src/capacity.rs` holds `CapacityBucket` (`new(layer, capacity, refill_per_sec, hold)`, `provisional(..)`, `acquire(units, exact)`, `resize`, `confirm`, `hold()`), with unit tests:
    - starts full;
    - never holds more than capacity;
    - paces the refill;
    - refuses an exact cost above capacity at once;
    - charges an inexact one at capacity;
    - waits while provisional, then proceeds or refuses after `confirm` or `resize`;
    - `resize` keeps the tokens (clamped) and the hold;
    - concurrent acquirers never over-admit.
  - `SignerLimiter` is rebuilt on two `CapacityBucket`s, keeps its public API, and gains `with_hold`. A tier change replaces the buckets at full capacity with the same `Hold`.
  - Re-prove row (i) against the bucket's capacity argument (mutant: capacity `1`). Its four tests are unchanged.
  - The signer suite (22) and clob's suites keep their counts.
  - Update the module doc, which links `RateLimiter`.
- [x] **Commit 9: `polymarket::ClobThrottle`.** It comes with `clob_throttle()`, the three `LayerId`s and `signer_cost`. New `polyoxide-core/tests/polymarket_throttle.rs`, through `HttpClient::send` and mockito:
  - `charges_the_ip_layer_one_and_the_signer_layer_n`;
  - `an_exact_batch_above_capacity_is_refused_without_sending`, with `expect(0)` and `match_query(Any)`, asserted;
  - `a_hold_on_the_throttle_stops_both_layers`;
  - `the_hold_survives_a_tier_change`;
  - `a_tier_on_a_429_is_adopted`;
  - `a_429_holds_both_layers`.
- [x] **Commit 10: the CAP-2 test.** `polyoxide-test-support/tests/token_cost_throttle.rs` implements `Throttle` for separate read and write `CapacityBucket`s over one `Hold`, with integer `Cost`s. It asserts:
  - the two layers are independent;
  - a 10-unit write drains 10;
  - an above-capacity cost is refused once confirmed, and waits while provisional;
  - `resize` keeps the tokens and the hold;
  - a hold stops both layers.

  It needs no edit to core.
- [x] **Deferred work.** In `deferred-work.md`, record for 4.11's notes:
  - `ApiError::Refused` is added;
  - the decode-error log targets moved;
  - the new hold WARN;
  - the default policy no longer retries 425;
  - AD-23's re-check is a tightening.

  Also record that bundle G removes the four transitional `HttpClient` methods and lists them, and that the Binance 418 row is still missing (3.6).

**Acceptance Criteria:**
- Given `HttpClient`, when it is built, then it holds one `Arc<DynThrottle<'static>>` and names no concrete limiter. Its `with_base_url` siblings share the throttle and its hold.
- Given gamma, data, perps, clob, relay and Binance, when their existing suites run, then each passes with names, assertions and counts unchanged, and each I/O row above has a test.
- Given each `docs/MUTANTS.md` row, rows (i) and (j) included, when its mutant is applied, then its tests fail. Rows (i) and (j) are proved at commit 0 and again after they move. `test_mutants_ledger.py` passes.
- Given the removal gate, when it runs against `v0.38.1`, then it reports only the five listed removals.
- Given `polyoxide-test-support`, when the CAP-2 test runs, then it passes, and `git diff` on `polyoxide-core/` in commit 10 is empty.

## Design Notes

These signatures are load-bearing for Stories 3.4–3.6 and for Kalshi:

```rust
#[dynosaur::dynosaur(pub DynThrottle = dyn(box) Throttle)]
pub trait Throttle: Send + Sync {
    fn acquire(&self, meta: &RequestMeta<'_>) -> impl Future<Output = Result<Charge, Refused>> + Send;
    fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, attempt: &AttemptInfo);
    fn hold(&self, delay: Duration);
}
// RetryPolicy::decide(&self, &ResponseMeta, &AttemptInfo, schedule: &RetryConfig) -> Decision
// Authenticator::sign(&self, &mut RequestParts, attempt: u32) -> impl Future<Output = Result<(), ApiError>> + Send
// HttpClient::send(&self, RequestParts, costs: &[Cost], Option<&DynAuthenticator<'_>>) -> Result<Response, ApiError>
// CapacityBucket::acquire(&self, units: u32, exact: bool) -> impl Future<Output = Result<(), Refused>> + Send
```

The types the hooks take:
- `RequestMeta { method: &Method, path, query: &[(String, String)], costs: &[Cost] }`.
- `ResponseMeta { status, headers }`, with a `retry_after()` helper.
- `AttemptInfo { attempt, retries_left }`.
- `RequestParts { method, path, query, headers, body: Option<String> }`, cloned for each attempt before `sign`.
- `Decision { outcome: Done | Retry(Duration) | Fail, hold: Option<Duration> }`.
- `Cost { layer: LayerId(&'static str), units: u32, exact: bool }`.
- `Charge`, which is opaque: `none()`, `with(LayerCharge { layer, units, window })` and `layers()`.
- `Refused { layer, units, capacity }`.

Core does not re-export the `reqwest` types in these signatures. Story 3.13 decides that.

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo test -p polyoxide-core --all-features -j 4`, and the same for `-p polyoxide-{gamma,data,perps,clob,relay,binance,test-support}` -- expected: green.
- `cargo test -p <crate> --all-features <target> -- --list | grep -c ': test$'`, for each target touched -- expected:
  - before: core lib 154, `mock_request` 10, data `mock_api` 37, signer 22 (inside lib), clob `mock_api` unchanged;
  - after commit 3: lib 155 (commit 0's golden test), `send_loop` 15, data `mock_api` 38;
  - after commit 6: lib 107, plus the builder tests, and `polymarket_limits` 48;
  - after commit 10: plus the hold and capacity unit tests, `polymarket_throttle` 6, and test-support's `token_cost_throttle`.
- `cargo clippy --workspace --all-targets --all-features -j 4 -- -D warnings`, then `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean.
- `cargo hack check -p polyoxide-core --each-feature --no-dev-deps`, and `cargo +1.91 check -p polyoxide-core` if that toolchain is installed -- expected: clean.
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: green (`test_mutants_ledger`, `test_classify_coverage`).
- Each `MUTANTS.md` row, run with its mutant and then without -- expected: fail, then pass. Rows (i) and (j) are proved at commit 0 and again at commits 4 and 8.
- `python3 scripts/api_removals.py check --baseline v0.38.1`, once -- expected: only the five listed keys.

## Implementation Notes

Commits 0–2 (`e05410c`, `0461462`, `50eca25`) were made by the first implementer; 3–10 are
`d425a23`, `13c00ed`, `ed0741d`, `14c23f1`, `dc1127a`, `34d60d3`, `2042d2d` and the CAP-2 commit.

- **`WindowQuotaTable`'s shape.** `new(count, period)` makes the general bucket;
  `bucket(count, period) -> BucketId`, `row(Matching, pattern, Option<Method>, &[BucketId])`,
  `prefix(pattern, method, count, period)` (a row with a bucket of its own), `with_hold(Hold)`,
  `build()` and `paced_interval`. The builder methods take `&mut self`. A `BucketId` carries
  the id of the table that made it, and `row` panics on a foreign one. Patterns are
  `&'static str`. `EffectiveQuota::admitted_in_one_window` returns `u128`, as the arithmetic
  tests' helper did.
- **The public-API rewrite.** `agreement::resolve_specs` became an extension trait over
  `effective_quota` that slices off the general bucket and compares `(count, period)`, so the
  test bodies that call it did not change.
- **Row (c), a hold extended mid-wait.** AD-23's re-check in `RateLimiter::acquire` also waits
  out an extended hold, so the cooldown test passes under that row's mutant. The row is now held
  by `Hold`'s own `a_hold_extended_mid_wait_is_honoured_in_full`, and the re-check has a row of
  its own, held by `a_hold_set_during_a_bucket_wait_is_honoured`.
- **`CapacityBucket`.** `refill_per_sec` is a `u32`, and zero panics. `acquire` is an
  `async fn` (clippy's `manual_async_fn`) with a compile-time `Send` assertion. A provisional
  bucket makes an above-capacity cost wait whether or not it is exact.
- **`ClobThrottle::new(table, signer)`** puts the signer layer on the table's hold with
  `SignerLimiter::with_hold`, which builds a new limiter; `signer()` and `table()` lend the
  layers. `SignerLimiter` also gained `hold()`.
- **The removal gate** ran once, after commit 4, with the local Rust 1.95.0 rather than CI's
  pinned 1.99.0: the five listed removals, plus `enum_variant_added` and
  `auto_trait_impl_removed` (recorded in `deferred-work.md`). `target/semver-checks` was
  deleted after.

## Spec Change Log

## Review Triage Log

All three layers ran: blind (17 findings), edge-case (16) and verification-gap (2 gaps, 1 other finding). Of the 36, 3 are medium, 30 low and 3 false; none is maybe-false. No finding needs the spec changed, so there is no loopback.
Three patches correct CLAUDE.md sentences that this bundle wrote or made false. As in spec 1-6/1-8, they are patched rather than deferred, since AD-21 has each commit edit the rule it changes.
After the patches, gamma's, data's and perps' `mock_api` each have one more test, and core's lib has one more capacity test.

**Patched:**
- **Medium:** `WindowQuotaTable`'s doc, `quota()`'s doc and CLAUDE.md:206 said nothing parked behind a hold resumes as a spike (blind). But AD-23's second wait holds back every request that took its tokens while the hold was in force, then releases them together. The behaviour is the spec's, and it beats baseline, which sent those requests into the hold. All three docs now describe it, bounded by the requests that were waiting on a bucket when the hold began.
- **Medium:** gamma, data and perps retried a 425 only because each builder installs `PolymarketRetryPolicy`, and no test went through the builders (gap). Each crate's `mock_api.rs` now serves 425, then 200, through its public builder.
- **Low:** CLAUDE.md:178 said every decode failure logs through `decode_json` (blind). Gamma's `post_json`, relay's nine `resp.json` reads and clob's `api/account.rs:102` do not, and the spec keeps them out of F. The sentence now names the four H3 call sites.
- **Low:** core's README was behind the new modules, and so was CLAUDE.md:176 (blind). Patched:
  - the README module table lacked `hooks`, `send`, `hold` and `capacity`;
  - its `error` row lacked `Refused`;
  - its `polymarket` row lacked `ClobThrottle`, `clob_throttle()` and `signer_cost`;
  - CLAUDE.md:176's `Classify` list lacked `Refused`.
- **Low:** `CapacityBucket::acquire` waited out an active hold before refusing an exact cost above a confirmed capacity (edge). The spec and the docs say it refuses at once. The refusal is now checked before the first hold wait, with a test. `ClobThrottle` still refuses after its hold and its IP charge, in the order the spec gives.
- **Low:** MUTANTS row (c), the re-check, cited `rate_limit.rs:553` (blind). Its snippet also stands at :543, so the ledger could not see the first wait shift onto :553. The row now cites :551-553, starting at the unique `// AD-23:` line, and is re-proved.
- **Low:** `WindowQuotaTable::new`, `bucket` and `prefix` panic on a zero or too-short period, and had no `# Panics` section (blind, edge). Each has one now.

**Deferred** (deferred-work.md):
- **The hold WARN reads `no retry left` for any hold that is not retried** (blind, edge, gap). So a custom policy's `Fail` with a hold misreads, and so would Binance's 418 in Story 3.6. G8 rewords it, as G's draft plans. The other half of the blind finding is false: baseline logged nothing for a 429 on the last attempt, so `observe.rs` counts the same as before.
- **`note_rate_limited` lost its only covering test when `send_raw` stopped calling it** (gap, medium). Clob's loop and relay's three loops still call it. Bundle G removes it with those four callers (G11), and G's clob and relay 429 rows cover the hold through the loop. S1 ships as one release (AD-16), so the gap does not reach a release.
- **`send` returns `Ok(response)` for `Fail`, against AD-8** (blind). Only this spec's [RISK] note says Story 3.11 changes that. Recorded for 3.11, which the frozen Decision names.
- **No `send_loop` test sends a body or fails `sign`** (blind). Bundle G's clob and relay moves are the first callers of both, and G's I/O matrix tests both. The rest of that finding is false:
  - the server reads `x-attempt` in `sign_runs_on_every_attempt_with_its_number`;
  - `polymarket_throttle.rs` sends POSTs;
  - deleting `drop(permit)` would hold the permit through the 1 s sleep that `mock_request.rs`'s `retry_releases_permit_during_backoff` checks.

**Rejected:**
- **A cost naming a layer the throttle lacks is dropped silently** (blind, edge). Low. `signer_cost` yields only the throttle's own layers, nothing passes costs before bundle G, and the fix is a guard.
- **`acquire_rate_limit` discards `Refused`** (blind, edge). Low:
  - `RateLimiter` and `NoThrottle` never refuse a request with no costs;
  - the spec keeps the transitional signatures unchanged;
  - G11 removes the method.
- **`CapacityBucket` is not fair between small and large costs** (blind, edge). Low. Baseline's governor `until_n_ready` races the same way, and the fix is a waiter queue.
- **A provisional bucket can wait forever, inexact costs included** (blind, edge). Low, and specified:
  - the spec's I/O row has an above-capacity cost wait for `confirm` or `resize`;
  - Implementation Notes record the choice for inexact costs;
  - no shipped caller makes a provisional bucket.
- **A composed throttle charges the IP layer before the signer layer refuses** (blind, edge). Low. The spec puts the refusal after the IP charge, as clob's loop does today, and it costs one IP token per refused batch.
- **Clob callers stop matching `BurstCapacityExceeded`** (blind). False today: no clob path returns `ApiError::Refused`, and G4 plans `burst_from_refused` with its test.
- **The new public types are exhaustive, so each change is another breaking release** (blind). False. S1 ships as one release, or as few as practical, after the error reshape (AD-16), and 3.11 owns `ApiError`'s shape.
- **`signer_limit` and `polymarket` import each other** (blind). False. Modules within one crate may import each other, and `SignerLimiter` is Polymarket code that moves with the module in S2.
- **`WindowQuotaTable`'s other edges** (blind), not patched:
  - the order of `effective_quota(&Method, path)` against `acquire(path, Option<&Method>)` is false as a defect, since both signatures are the spec's Decisions;
  - a missing `Debug` and a row with no buckets are low misuse, and their fixes add surface or a guard.
- **The sweep skips the general bucket and perps** (blind). Low, and inherited from baseline. Every bucket is built by the one `quota()`, so row (d)'s mutant already fails on the row buckets, and NFR7 keeps the moved test's assertions.
- **A tier adopted mid-wait leaves the waiter on the old bucket** (edge). Low, and pre-existing: baseline cloned the governor `Arc` the same way, and the tier follows 30-day volume, so it seldom changes.
- **A capacity of zero, a ceiling of zero, a drained limiter handed to `ClobThrottle::new`** (edge). Low misuse:
  - the only callers pass a tier burst with `.max(1)`, a 3-day ceiling (Story 3.6) and `SignerLimiter::new()`;
  - each fix is a guard.
- **`LayerCharge` overstates an inexact signer charge** (edge). Low. `ClobThrottle::observe` ignores the charge, so nothing reads it.
- **Three edges of a failed attempt** (edge). Low:
  - a refusal or a sign failure on a retry hides the earlier 429;
  - a charge is spent when `sign` or the transport fails;
  - a path that `sign` rewrites is ignored.

  Nothing refuses or signs before bundle G. Baseline also charged before a transport error, and `sign`'s contract is to add headers or query parameters.
