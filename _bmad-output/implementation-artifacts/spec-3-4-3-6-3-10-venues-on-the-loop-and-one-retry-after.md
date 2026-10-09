---
title: 'Stories 3.4, 3.5, 3.6 and 3.10: clob, relay and Binance on the one send loop, one Retry-After parser and one retriable-status rule'
type: 'refactor'
created: '2026-10-09'
status: 'in-progress'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'ecad4c76254c771ca8b12b0a632a2ed0a9e9ca2c'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-3-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:**
- After bundle F (Stories 3.1–3.3), core owns one send loop, but three venues still run their own:
  - clob's `Request::send_raw`, with L1/L2 signing and the per-signer limiter wired in by hand;
  - relay's three loops (`get_with_retry`, `get_with_retry_authed`, `post_json`);
  - Binance's `WeightedRequest::send_text`, with the weight minute, the 418 ban and the next-minute hold.
- Four sends skip every loop (DRIFT R8): clob's ping is ungated, gamma's `post_json` takes the limiter before the permit and never retries, and the gamma and data pings feed no 429 back.
- Relay's errors do not wrap `ApiError` (DRIFT R7): every HTTP failure is an opaque `Api(String)` classed `VenueRefusal` whatever its status, `RateLimit` and `Core` are never built on purpose, and no test covers a relay 429.
- Four `Retry-After` parsers disagree (DRIFT R4), and core, perps and Binance each copy the 408/425/429/5xx rule (H12).
- Clob's and relay's credentials are held in plain types.
- Four transitional `HttpClient` methods survive only for these loops.

**Approach:** Each venue plugs its policy into F's hooks, and the loops, the parsers and the rule copies go.
- **Story 3.4:**
  - core's `Request` becomes the one request builder: it gains a method, a JSON body, an authenticator and costs;
  - clob's L1, L1-signed and L2 headers become `Authenticator`s;
  - clob's client holds F's `polymarket::ClobThrottle` and `PolymarketRetryPolicy`;
  - clob's namespaces return `polyoxide_core::Request<T, ClobError>`, and clob's `request` module is removed;
  - the credentials are held in `Secret`.
- **Story 3.5:**
  - relay sends through `HttpClient::send` with a relay `Authenticator`;
  - `RelayError::Api` wraps `ApiError`, and `RateLimit` and `Core` are dropped (R7);
  - the R8 sends go through the loop;
  - the credentials are held in `Secret`.
- **Story 3.6:**
  - `WeightBudget` implements `Throttle` over core's `Hold` with a 3-day ceiling;
  - a Binance `RetryPolicy` holds the AD-9 Binance rows;
  - `WeightedRequest` is removed, replaced by per-route builders over core's `Request`;
  - a CI test fences governor out of every venue crate.
- **Story 3.10:**
  - every `Retry-After` read goes through `polyoxide_venue::parse_retry_after` with the caller's clamp (R4);
  - the three rule copies call `polyoxide-venue`'s.
- **Core additions:** what G needs beyond F's surface lands in one core commit (G3), with F's spec left as written.
- **Last:** the transitional methods' test callers are rewritten through public API, then `should_retry`, `note_rate_limited` and `acquire_rate_limit` are removed and listed.
- **First, before any line moves:** record and prove the ledger row for Binance's 418 hold.

**Decisions (Claude, as the user's delegate, 2026-10-09; one-line reasons).** Items marked **[ACCEPTED]** were risks that the lead, as the user's delegate, accepted on 2026-10-09. Each one that a consumer can see goes into deferred-work for 4.11's release notes.

*Contract with F.* F's spec stays as written; F is being implemented now. G builds on F's Design Notes: the hook traits, `HttpClient::send`, `PolymarketRetryPolicy`, `WindowQuotaTable`, `Hold`, `CapacityBucket`, `ClobThrottle`, `signer_cost` and the `LayerId`s. Where G needs more, G's core commit (G3) adds it:
- **[ACCEPTED] A3-2, resolved: `ApiError::Sign(Box<dyn Error + Send + Sync>)`.**
  - `Authenticator::sign` keeps F's `Result<(), ApiError>`.
  - A venue's authenticator boxes its own error into `Sign`, and the venue's `From<ApiError>` downcasts it back.
  - So clob's `ClobError::Alloy` from an L1 signer still reaches the caller as `Alloy`.
  - `Sign` is classed `InvalidRequest` and never retried.
  - Record the resolution in `spine-amendments/epic-3.md` under A3-2 (F creates the file with A3-1 and A3-2).
- **[ACCEPTED] A3-3: `RequestParts` gains `timeout: Option<Duration>`.** Relay's session-signer posts wait up to 300 s per attempt (`SESSION_SIGNER_REQUEST_TIMEOUT`), and AD-8's field list has no room for that. Record it in `spine-amendments/epic-3.md` as A3-3.
- **`Hold::is_held()`.** Binance's funding slot re-queues when a hold began during its slot wait, and F's `Hold` offers only `extend` and `wait`.
- **[ACCEPTED] `PolymarketRetryPolicy` returns `Fail`, keeping its hold, when `retries_left == 0`.** This is behaviour-neutral, since F's loop stops there either way. It makes exhaustion a property of `decide`, which the nine rewritten exhaustion tests assert through (G10).
- **[ACCEPTED] `RetryConfig::attempt_info(attempt) -> AttemptInfo`.** The loop builds its `AttemptInfo` with it, so a test that varies `max_retries` exercises the loop's own arithmetic.
- **[ACCEPTED] The burst refusal maps clob-side.** A `pub(crate)` clob function, `burst_from_refused(&Refused) -> Option<BurstCapacityExceeded>`, lives in clob's `error.rs` (G4):
  - the bucket comes from the layer (`polymarket::SIGNER_ORDER` or `SIGNER_CANCEL`);
  - the tier is the one whose published `Tier::burst(bucket)` equals `capacity`. Each bucket's bursts are distinct across the eight tiers, so the lookup is exact.
  - `ClobError::from(ApiError::Refused(..))` uses it, so the burst-capacity tests keep their variant and their `cost` and `capacity`. Core is not edited.
  - The alternative needs `Refused` to carry a tier, which F fixes as `{ layer, units, capacity }`.
- **[ACCEPTED] The R10 hold warning is reworded in G8.**
  - F words the WARN for a hold with no retry as `… no retry left: …`. That misdescribes Binance's 418, a `Fail` with a hold.
  - When Binance moves onto the loop, the line becomes `Status <code> on <path>, not retried: every request held <ms>ms`, which fits both cases.
  - F's test of that line (`a_hold_with_no_retry_left_warns`, F's commit 3) asserts the new text; G8's commit message names it.
  - The `Retriable status <code> on <path>, retry <n> after <ms>ms` line is unchanged, since the soak verdicts read it.

*Story 3.4*
- **Core's `Request<T, E>` is the one request builder.** It gains `method(Method)`, `body(&B) -> Result<Self, E>`, `authenticator(Arc<DynAuthenticator<'static>>)` and `with_cost(Cost)`.
  - The epic lists clob's `request` module among the consolidated removals.
  - The architecture guide puts a route's costs in its request builder.
- **`Request::body` serializes through `serde_json::Value`, as clob's builder did,** so every clob body, and the L2 signature over it, stays byte-identical. Serializing the struct directly would not: without `preserve_order`, which the workspace does not enable, a `Value` sorts its keys.
- **Clob's errors map in `From<ApiError> for ClobError`, written by hand** in place of `#[from]`:
  - `Refused` becomes `BurstCapacityExceeded`, through `burst_from_refused`;
  - `Sign` is downcast back to the `ClobError` it carries;
  - anything else becomes `Api(e)`.

  `classify_order_kill` and `from_response` are unchanged.
- **The authenticators:**
  - **L2** holds `Secret<Credentials>`, the address and the HMAC signer. It signs `method + path + body` with a fresh timestamp each attempt and sends `POLY_ADDRESS` lowercase, as today.
  - **L1** holds the wallet, the nonce and the chain id. It signs the EIP-712 `ClobAuth` with a fresh timestamp each attempt, inside `sign`.
  - **L1-signed** resends its four fixed headers each attempt, with the signature redacted from `Debug`.
- **Credentials:** `Account` holds `Secret<Credentials>`. `Credentials` keeps its public `String` fields and serde, and `Account::credentials()` still returns `&Credentials`.
  - [ACCEPTED] `Secret` goes on the holders, not on the fields. Reading AC 3.4 as "the fields are `Secret`" would change public API, so that is left to S2's credential work (`StoredCredential`).
- **`Clob` keeps a `SignerLimiter` handle, shared with the throttle,** for `tier()` and `rate_limit_status()`.
  - The clob builder installs `ClobThrottle` over F's clob table and that limiter, with one `Hold`, plus `PolymarketRetryPolicy`.
  - [AFTER-F] Use `polymarket::clob_throttle()` if it hands back the limiter; otherwise build the pair as F's `clob_throttle()` does.
- **Trading requests** carry `with_cost(polymarket::signer_cost(req))` in place of `.trading(limiter, req)`. The namespaces drop their `SignerLimiter` and credential clones and hold the account's `Arc`'d L2 authenticator.

*Story 3.5 and the R8 sends*
- **Relay calls `HttpClient::send` directly through one private helper.** Relay's methods are async fns, not builders.
  - The helper turns a non-2xx response into `RelayError::Api(ApiError::from_status_and_body(status, body))`.
  - It keeps today's ERROR line for a failed POST.
  - GET bodies still decode with `resp.json()`, and the POST body with F's `decode_json`.
- **Relay's paths:**
  - relay still joins relative paths onto its base URL, so a path-prefixed base keeps working (`RelayClientBuilder::url` supports one);
  - it hands the loop the joined URL's path and query pairs;
  - the GET authenticator signs the logical path (`/transactions`), and the POST authenticator signs `url.path()`, both as today;
  - the POST authenticator applies the caller's extra headers after its auth headers, then `Content-Type`, as today.
- **Relay's auth checks move ahead of the send.** `auth()` and the allow-lists run before `send`, not inside the loop, so a refused call spends no permit and no token.
- **[ACCEPTED] Relay's local refusals.** About 55 `RelayError::Api(String)` sites that are not HTTP failures (validation, keychain, chain config, header generation, gas estimation) become `RelayError::Api(ApiError::Validation(msg))`, through a `pub(crate) fn validation`.
  - This keeps their class (`VenueRefusal`) and message. The `Display` prefix changes from `Relayer API error:` to `Validation error:`.
  - It mirrors `ClobError::validation`.
  - Story 3.11 moves local validation to `InvalidRequest` for both.
- **`RelayError` keeps `Reqwest`, `UrlParse`, `SerdeJson`, `Signer` and `MissingSigner`.**
  - `Api` becomes `#[error(transparent)] Api(#[from] ApiError)`.
  - `Core` and `RateLimit` go.
  - The enum is not made `#[non_exhaustive]` (3.11).
- **Relay's builder credentials:** `RelayClient`, `RelayClientBuilder` and `BuilderAccount` hold `Option<Secret<AuthConfig>>`. `BuilderConfig`'s public fields and `auth_config()` keep their shape; this is the same reading as clob's.
- **Gas estimation stays on alloy, outside the loop** (AD-8's named exception), with a comment at `estimate_call_gas` saying so.
- **The R8 pings call `HttpClient::send`, not core's `Request`,** since a ping discards its body.
  - [ACCEPTED] Each ping times its last attempt with a private timing `Authenticator`, whose `sign` notes the instant before the send. So the reported latency still excludes permit, throttle and backoff waits, as gamma's and data's do today.
  - The alternative is to time the whole `send`, as Binance's ping does.
  - Story 3.7's `health(path)` replaces all three and settles the semantics.
- **Gamma's `post_json` calls `HttpClient::send` with the body from `serde_json::to_string`,** the bytes reqwest's `.json()` sent, and with `Content-Type: application/json`. Core's `Request::body` would sort its keys and add ERROR log lines.

*Story 3.6*
- **`WeightBudget` implements `Throttle`.**
  - `acquire` reads the request's `costs` (`weight::{WEIGHT_LAYER, FUNDING_LAYER}`) and returns a `Charge` whose weight `LayerCharge` carries the UTC minute as its `window`.
  - `observe` applies `X-MBX-USED-WEIGHT-1M` to that minute, on every status.
  - `hold` extends the budget's `Hold`.
- **The budget's inherent `acquire(Cost)`, `record_used`, `begin_cooldown` and `hold_until_next_minute` stay.** The 15 protected `weight.rs` tests call them by name, and inherent methods win over the trait's.
  - Its private `Charge` is renamed `MinuteCharge` to avoid core's name; it stays `Copy`, as `a_header_raises_the_count_and_never_lowers_it` needs.
- **The cooldown is core's `Hold::with_ceiling(MAX_COOLDOWN)`.** Every `Usdm` built with one `WeightBudget` shares it.
- **[ACCEPTED] Deviation from AC 3.6: the funding bucket keeps its paced slot.** The AC says "the funding bucket uses core's `WindowQuotaTable`".
  - The slot takes its interval from `WindowQuotaTable::paced_interval(500, 5 min)`, which equals today's `300 s / 449`, and its hold from core's `Hold`.
  - It does not run on a `WindowQuotaTable`-built `RateLimiter`. That is governor's, on `QuantaClock` with `futures_timer` waits, a real clock that tokio's paused clock does not drive.
  - Six protected `weight.rs` tests run on the paused clock, five minutes of it in `the_funding_bucket_admits_450_per_five_minutes`. On governor's real clock they hang.
  - Deferred: make `WindowQuotaTable`'s clock injectable, then move the funding bucket onto it, when Binance's tables move into its venue crate in S2.
- **`UsdmRetryPolicy { budget }` holds AD-9's Binance rows:**
  - a 429 with a retry left is `Retry(ZERO)`, with hold `max(schedule.retry_delay(attempt, ra), ra_3d)`;
  - a 429 with none left is `Fail`, with hold `ra_3d`, or the time to the next UTC minute on the budget's clock;
  - a 418 is `Fail`, with hold `ra_3d`, or `DEFAULT_BAN`;
  - a 2xx is `Done`, and everything else is `Fail` with no hold, a 425 included.
- **[ACCEPTED] The request's own 429 wait** becomes the longer of the loop's floor and the hold, which is two jitter draws of the same backoff where today there is one. It stays within the same 0.75–1.25 band, and no test can observe it.
- **`WeightedRequest` goes, replaced by per-route builders over core's `Request`.** The eight methods that returned it directly return new builders, each with `cost()` and `send()` like the existing `GetKlines`:
  - `GetTime`, `GetExchangeInfo`, `GetFundingInfo`;
  - `GetTicker24h`, `GetTickers24h`, `GetPremiumIndex`, `GetPremiumIndices`, `GetOpenInterest`.

  A private `Routed<T>` in `usdm/request.rs` keeps the replace-on-repeat query (`klines_send_every_parameter_once` sends `limit` once). The module keeps `USED_WEIGHT_HEADER`.
  - The rejected alternative returns core's `Request` with a `cost()` extension trait. That adds a `use` line to the protected `mock_api.rs` and an import for every consumer.
- **`BinanceError` implements `RequestError`.** A 418 logs its body (`-1003` names the ban's end) at WARN under `polyoxide_binance`. The hold itself is warned by the loop under `polyoxide_core`.
- **The governor fence is a pytest:** `.github/scripts/tests/test_dependency_fences.py` refuses a direct `governor` dependency in any member that declares `[package.metadata.polyoxide] venue`.
  - Core and the CLI facade (which paces `clob prices download` with governor) are not venue crates.
  - Story 3.13 extends the same file to reqwest.

*Story 3.10*
- **One parser, with each caller's clamp:**
  - core's loop and `RetryConfig::retry_delay` use `max_backoff_ms`, as today;
  - Binance uses `MAX_COOLDOWN`, its `retry_after_secs` becoming a one-line wrapper so its test stays;
  - perps' `VenueError` and data's `V2Error` use `Duration::MAX`, since their values are surfaced, never slept on, and were unclamped.
- **R4's disagreements and their new answers** are each pinned by a test (I/O Matrix):
  - the zero rows of perps' and data's `every_variant_classifies` change their inherent wait from `Some(0)` to `None`;
  - one new test per changed crate pins the rest.
- **[ACCEPTED] H12: the inherent `is_retriable` methods stay (3.11 and 3.12 remove them), and their status arms call `class_for_status(s).is_some_and(|c| c.is_retriable())`.**
  - Three rows of core's `every_variant_classifies` change their inherent answer: `Api{408}` and `Api{429}` become true, and `Api{600}` becomes false.
  - This is AC 3.10's "tests pin the cases where they used to disagree, each with its new answer". G2's commit message names each of the three rows.
  - `from_status_and_body` never builds either of the first two, and no server sends a status of 600 or more, so no reachable answer changes.
  - perps' and Binance's tables pin no such row.
- **`retry_after_header` stays.** Its remaining callers are perps' and data's error decoders, which 3.11 reshapes.

*The transitional methods*
- **`should_retry`, `note_rate_limited` and `acquire_rate_limit` are removed,** after their 17 test callers move to public API in a commit of their own (AD-12):
  - 13 in `client.rs` and 2 in `rate_limit.rs` call `should_retry`;
  - 2 builder tests call `acquire_rate_limit`.
- **The rewrite's mapping:**
  - the six delay tests call `RetryConfig::retry_delay`;
  - the nine status and exhaustion tests call `PolymarketRetryPolicy::decide` with `RetryConfig::attempt_info(n)`;
  - the two builder tests time a `send` to a mock server up to its `sign`.
  - The rewrite relies on G3's policy `Fail` at no retries left and on `RetryConfig::attempt_info`.
- **[ACCEPTED] Deviation from F's note: `acquire_concurrency` stays public.** F's note says G removes all four transitional methods.
  - Eleven protected tests use it to observe the permit: core `mock_request` `retry_releases_permit_during_backoff`, five in core's `client.rs`, the default-concurrency tests of gamma, data, clob and relay, and gamma's `ping_waits_on_the_shared_request_gate`.
  - Holding a permit bypasses no throttle.
  - Deferred: remove it once those tests can observe the permit another way.

## Boundaries & Constraints

**Always:**
- **G starts only after F's 11 commits are reviewed and committed.** Bundle E must have merged too.
- **Re-read F's landed code first.** Re-check every [AFTER-F] reference, and every F name this spec uses, before relying on it.
- **Commit at each boundary (G0–G11), on this branch only.**
  - Never push, tag or switch branches.
  - End every commit message with these two trailer lines:
    - `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
    - `Claude-Session: https://claude.ai/code/session_01VPtJy6PPKDmq7U2qoyhEb2`
- **Behaviour is preserved except R4, R7 and R8, and the edge answers named above.**
  - The retry set stays 429 and 425 for clob and relay.
  - Clob's and relay's 429 holds stay `retry_delay(0)`, the last attempt included.
  - The FAK and FOK kills, the burst refusal, tier adoption and the L1/L2 header sets are unchanged.
  - Binance's weight table, its minute, the reserve of 240, the funding pace and every hold are unchanged.
  - The `Retriable status <code> on <path>, retry <n> after <ms>ms` WARN keeps its text and its `polyoxide_core` target. Only the hold-without-retry line is reworded, in G8.
- **Each DRIFT row (R4, R7, R8) is its own commit, naming its row.**
- **NFR7.**
  - Moved and rewritten tests keep their names and asserted values.
  - Report per-suite counts before and after every commit that moves, rewrites, adds or drops tests, from `cargo test -p <crate> --all-features <target> -- --list | grep -c ': test$'`.
  - Append new mock tests at the end of their files, so no cited line moves.
- **The mutant ledger.**
  - Re-prove every `docs/MUTANTS.md` row whose line or test moves.
  - Update `.github/scripts/tests/test_mutants_ledger.py` in the same commit.
- **Removals.**
  - Every removed public item goes in `docs/s1-removals.md`, with its story and replacement, keyed exactly as the gate prints it.
  - Every changed one, and every consumer-visible [ACCEPTED] item, goes in `deferred-work.md` for 4.11's notes and prader.
- **Standing rules.** CLAUDE.md's rules are rewritten in the commit that supersedes them (AD-21).
- **Rustdoc.** `pub` docs never link a `pub(crate)` item or a removed one.
- **Disk.** Build with `CARGO_INCREMENTAL=0` and `-j 4`. Run `scripts/api_removals.py` once, at the end, then delete `target/semver-checks`.
- **mockito.** An `expect(0)` on a route with a query needs `match_query(Matcher::Any)`, and every mock with an `expect` is asserted.

**Never:**
- Retry 5xx or 408, narrow Polymarket's policy, or retry a 425 on Binance.
- Change a limit table's rows, the tier table, `RESERVED_FRACTION`, the signer layer's `allow_burst`, or Binance's `Route::cost`.
- Move Binance's weight minute into core (D3), or unify `RetryConfig::backoff` with the socket `Backoff` (D13).
- Give a venue crate a direct governor dependency, or add a shim or re-export for a removed item.
- Change `classify_order_kill`, or route relay's alloy gas estimation through the loop.
- Reshape error enums beyond R7:
  - no `#[non_exhaustive]` added;
  - no inherent `is_retriable` or `retry_after` removed;
  - no enum renamed (3.11, 3.12, S2).
- Build 3.7's `health(path)` or builder macro, or 3.13's reqwest fence.
- Hand-edit `docs/ARCHITECTURE.md` or a generated region.

## I/O & Edge-Case Matrix

"Retry left" means `retries_left > 0`. `ra` is the parsed `Retry-After`, and `ra_3d` is the same clamped to 3 days.

| Scenario | Input | Hold | Outcome |
|---|---|---|---|
| R4: zero | `Retry-After: 0` (also `-0` on data v2) | — | No wait anywhere. The `retry_after` that perps and data v2 surface goes from `Some(0s)` to `None`, and Python's from `0.0` to `None` |
| R4: padded | `" 2 "` on core's floor | — | 2 s is honoured (it was ignored) |
| R4: fractional | `1.5` on perps | — | 1.5 s (was `None`) |
| R4: huge | `1e300` on data v2; 20 digits on perps | — | `Duration::MAX`. Data v2 used to panic in `from_secs_f64`, and perps returned `None` |
| R4: clamp | `604800` | — | Core's floor 10 s, Binance 3 days, perps and data 604 800 s (all unchanged) |
| R4: junk | HTTP-date, `abc`, `NaN`, `inf`, `-1`, empty | — | No wait (unchanged) |
| H12 | `ApiError::Api{408}`, `{429}`; `{600}` | — | Inherent `is_retriable` true, true; false |
| Clob 429, retry left | an order POST | `retry_delay(0)` on the IP and signer layers | Retried on the floor, re-signed with a fresh `POLY_TIMESTAMP`; WARN under `polyoxide_core` |
| Clob L1 | `create_api_key` gets a 429 | `retry_delay(0)` | Retried with a fresh L1 signature. L1-signed resends its fixed headers |
| Clob batch above burst | cancel 2,000 at Standard | none | `BurstCapacityExceeded { cost: 2000, capacity: 120, tier: Standard, bucket: Cancel }`; nothing sent |
| Clob L1 signer fails | an alloy error in `sign` | none | `ClobError::Alloy`; nothing sent |
| Clob tier | `Poly-RateLimit-Tier: gold` on any status of a trading request | unchanged | Both signer buckets at Gold |
| Clob FAK kill | 400 with the FAK prose | none | `FakUnmatched`, not retried |
| R8 ping or `post_json` | 429 with a retry left | `retry_delay(0)` | Retried (pings and `post_json` used to fail at once) |
| R8 ping | 429 with none left | `retry_delay(0)` | Error; the next request on the client waits |
| R8 clob ping | the permit is held | — | The ping waits for it |
| Relay, any route | 429 with a retry left | `retry_delay(0)` | Retried, then `Ok` |
| Relay | 429 with none left | `retry_delay(0)` | `RelayError::Api(ApiError::RateLimit)`, classed `RateLimited`; the next request waits |
| Relay | 503; 425 | none | `Api(Api{503})` `Unavailable`, not retried; a 425 is retried |
| Relay local refusal | a blank idempotency key, no auth | none | `Api(Validation(msg))`; nothing sent, no token spent |
| Relay session signer | a slow venue | — | That attempt times out at 300 s |
| Binance 2xx or 4xx | `X-MBX-USED-WEIGHT-1M: 2160` | — | Recorded for the minute its request was charged in |
| Binance 429, retry left | `ra` 1 s, backoff ≤ 100 ms | `max(retry_delay(n), ra_3d)` = 1 s | Retried after ~1 s |
| Binance 429, none left | `ra` 1 s; no `ra` | 1 s; to the next UTC minute | `RateLimited { retry_after }` |
| Binance 418 | `ra` 1 s; no `ra`; 10 days | 1 s; 2 min; 3 days | `IpBanned`, not retried |
| Binance 425, 5xx | — | none | Error, not retried |
| Binance shared budget | a 418 on one `Usdm` | `ra` | Every `Usdm` on that budget waits |
| Binance funding | 10 requests parked by a 60 s hold | 60 s | The first is released at 60 s, the rest ≥ 668 ms apart |
| Polymarket exhaustion | 429 with `retries_left == 0` | `retry_delay(0)` | `decide` returns `Fail`. The loop returns the response, as before |
| Hold, no retry | a Binance 418, or any 429 with no retry left | as above | One WARN under `polyoxide_core`: `Status <code> on <path>, not retried: every request held <ms>ms` |

</frozen-after-approval>

## Code Map

Lines are today's. **[AFTER-F]** marks a file F's commits edit; re-cite those after F lands. F's draft spec is the contract for its surface.

- **Core**
  - `polyoxide-core/src/client.rs` [AFTER-F]:
    - `retry_after_header` :13-21 (stays);
    - `acquire_rate_limit` :78-82, `acquire_concurrency` :89-97, `should_retry` :126-137, `retry_delay` :145-155 (R4 site: no `trim`, ms truncation, `max_backoff_ms` clamp), `note_rate_limited` :171-178.
    - Tests (29):
      - should_retry :376-580 (13; rows (b) cite :509 and :532);
      - builder wiring :586-608 (2, `acquire_rate_limit`);
      - concurrency :613-693 (5, `acquire_concurrency`).
  - `polyoxide-core/src/request.rs` [AFTER-F]: `Request<T, E>` :63-68 (GET-only, `pub(crate)` fields), hand-written `Clone` :86-95, `RequestError` :57-60. 14 tests.
  - `polyoxide-core/src/error.rs` [AFTER-F, `Refused` added]:
    - `is_retriable` :94-104, with the H12 copy at :98;
    - `Classify` from :171;
    - `every_variant_classifies` :404-511, with rows `api(408)` :422, `api(429)` :426, `api(600)` :435 and the comment :505.
  - `polyoxide-core/src/rate_limit.rs` [AFTER-F]: should_retry tests :1797 and :1826; `RetryConfig` :694-730 (F adds `retry_delay`).
  - `polyoxide-core/src/signer_limit.rs` [AFTER-F]: the tier table :78-89, the public `Tier::burst` and `BurstCapacityExceeded` :234-245. Clob's `burst_from_refused` reads them; core is not edited here.
  - `polyoxide-core/tests/mock_request.rs`: 10 tests; :271 `retry_releases_permit_during_backoff` uses `acquire_concurrency`.
- **Venue:** `polyoxide-venue/src/retry_after.rs` — `parse_retry_after` :27-36 (`secs <= 0.0` :29, zero filter :35) and `retry_delay` :44-46. `status.rs` `class_for_status` :23. 5 tests.
- **Clob**
  - `polyoxide-clob/src/request.rs` (removed whole) [AFTER-F, H3 at :178-191]:
    - `AuthMode` :16-36 and its `Debug` :41-74;
    - `Request<T>` :77-92, `trading` :96-99, `body` :164-167 (via `Value`), `send` :178-192;
    - `send_raw` :195-296: IP `acquire_rate_limit` :210, signer `acquire` :216-218, `note_rate_limited` :262, `observe` :267-269, `should_retry` :271;
    - `l1_headers` :301-323, `add_auth_headers` :326-378;
    - 2 tests, :393 and :457.
  - `polyoxide-clob/src/lib.rs` :140 `pub mod request;`.
  - `polyoxide-clob/src/client.rs`:
    - `signer_limiter` :59;
    - the namespace constructors :98-233, which clone credentials;
    - `*_with_signature` :284-336 (L1-signed);
    - `post_orders`, `post_order` :695-776 (trading);
    - the builder `build` :997-1040, with `with_rate_limiter` :1001 [AFTER-F] and `SignerLimiter::new()` :1034;
    - 20 tests (:1108 uses `acquire_concurrency`).
  - `polyoxide-clob/src/api/*.rs`: `Request::`/`AuthMode::` sites — markets 50, auth 14, account 13, orders 13, rewards 13, health 2, notifications 2, plus 8 in `client.rs`. No GET sets a body.
  - `polyoxide-clob/src/api/health.rs`: ping :34-49 (ungated, R8). 3 tests.
  - `polyoxide-clob/src/error.rs`:
    - `Api(#[from] ApiError)` :16;
    - `classify_order_kill` :86-98;
    - `from_response` :106-116 (rows (e) :89, :92, :110-113);
    - tests :314–:389 (rows (e)). A hand-written `From` inserted after :145 shifts them.
  - `polyoxide-clob/src/account/mod.rs`: `credentials: Credentials` :86, `from_parts` :144-151, `credentials()` :379-381.
  - `polyoxide-clob/src/account/credentials.rs`: public `String` fields, redacting `Debug`. 3 tests.
  - `polyoxide-clob/tests/mock_api.rs`: 106 tests.
    - (e) :3365, :3399, :3462;
    - burst and tier :3495, :3533, :3552, :3581;
    - the fast-retry suite :926-1060.
- **Relay**
  - `polyoxide-relay/src/client.rs`:
    - `auth` field :244;
    - `get_with_retry` :285-321;
    - `auth()` :323-336, `authed_get_headers` :338-365;
    - `get_with_retry_authed` :368-420;
    - the routes :435-660;
    - `estimate_call_gas` :1573-1595 (alloy);
    - `post_json` :1785-1880 [AFTER-F, H3 at :1866];
    - the builder :1886-2060, with `with_rate_limiter` :2032 [AFTER-F];
    - 36 tests (:2064 `test_ping` hits the live host; :2070 uses `acquire_concurrency`).
  - Routes: GET `/`, `/nonce`, `/transaction`, `/transactions` (authed), `/relayer/api/keys` (authed), `/deployed`, `/v1/account/transactions/params`, `/v1/account/transactions/{id}` and `/relay-payload`; POST `/submit`, `/v1/session-signers/authorizations` and `/v1/session-signers/revocations`.
  - `RelayError::Api(String)` sites: client 35, account 12, wallet 5, session_signers 4, config 2.
  - `polyoxide-relay/src/error.rs` :9-33 (`Api(String)` :23, `RateLimit` :26, `Core` :32), `Classify` :39-64. 8 tests.
  - `polyoxide-relay/src/config.rs`: `BuilderConfig` :69-186, `AuthConfig` :252-279. `account.rs`: `config` :29, `auth_config()` :108.
  - `polyoxide-relay/tests/mock_api.rs`: 43 tests, none on a non-2xx response.
- **Binance**
  - `polyoxide-binance/src/usdm/request.rs` [AFTER-F, H3 at :71-79]:
    - `WeightedRequest` :19-25;
    - the loop :83-163, with the 418 hold :111-113 and the 429 hold :136-149 (row (a)-Binance).
  - `polyoxide-binance/src/usdm/mod.rs`: `pub use request::WeightedRequest` :19; the `build` wiring. 3 tests.
  - `polyoxide-binance/src/usdm/api/{market,exchange,health}.rs`: eight methods return `WeightedRequest` (market :31, :37, :43, :49, :76; exchange :23, :29; health `time`); four builders, `GetKlines` among them.
  - `polyoxide-binance/src/weight.rs`:
    - constants: `DEFAULT_BAN` :26, `MAX_COOLDOWN` :30;
    - the reserve and pace: `after_reserve` :40, the funding pace :247-251;
    - state: `cooldown_until` :189, `Charge` :197;
    - methods: `acquire` :286, `record_used` :337, `begin_cooldown` :348, `hold_until_next_minute` :359, `await_cooldown` :366, `in_cooldown` :379;
    - 15 tests on tokio's paused clock.
  - `polyoxide-binance/src/error.rs`: `is_retriable` :100-107 (H12 at :103), `retry_after_secs` :129-138. 10 tests.
  - `polyoxide-binance/tests/mock_api.rs`: 25 tests — 418 :409, 429 :441, :580, :616, :652, weight :306, :477, :529, :550, :691.
- **R8 and the other R4 sites**
  - gamma `src/api/health.rs` ping :27-47, test :62-90 (`gamma_default` [AFTER-F]); `src/api/markets.rs` `post_json` :132-164, callers :209 and :247; `tests/mock_api.rs` 42 tests (POST :633, :676, :713).
  - data `src/api/health.rs` ping :37-55; `tests/mock_api.rs` 37 tests (38 after F).
  - data `src/v2/error.rs` `from_parts` :86-100 (R4 :95-98). 6 tests.
  - data `src/error.rs`: the zero row ~:186-194.
  - perps `src/error.rs`: `from_parts` :56-70 (R4 :64-67), H12 at :78, the zero row :307-314. 8 tests.
- **Ledger, gates, docs**
  - `docs/MUTANTS.md`: rows (a)-Binance, (b) and (e), and "not yet covered" (clob :262, relay :293, :392, :1836) [AFTER-F].
  - `.github/scripts/tests/test_mutants_ledger.py` `SNIPPETS` :22-65.
  - `docs/s1-removals.md`, plus F's five keys.
  - `_bmad-output/implementation-artifacts/deferred-work.md`: the 1-6/1-8 entry's 418 item.
  - `spine-amendments/epic-3.md` (F creates it).
  - CLAUDE.md:
    - :13 [AFTER-F], :174, :176 (parsers "until Story 3.10", "a 408 in `Api` final"), :178 [AFTER-F], :195-200 [AFTER-F];
    - the Binance weight paragraph :292-301;
    - the relay environment paragraph.

## Tasks & Acceptance

**Execution:**
- [x] **G0 — Binance's 418 hold on the ledger.**
  - Add row (k), "a 418 holds every request on the budget", at `polyoxide-binance/src/usdm/request.rs:111-113` [AFTER-F].
  - Mutant: delete `self.budget.begin_cooldown(ban);`. It fails `mock_api.rs:409` `a_418_is_not_retried_and_holds_the_next_request`.
  - Prove it, add the snippets, and mark the 418 item of the 1-6/1-8 deferred entry done.
- [x] **G1 — DRIFT R4: one `Retry-After` parser.**
  - The callers:
    - `RetryConfig::retry_delay` [AFTER-F] becomes `polyoxide_venue::retry_delay(ra.and_then(|v| parse_retry_after(v, max_backoff)), self.backoff(attempt))`, and so does F's `ResponseMeta::retry_after` if it parses;
    - Binance's `retry_after_secs` becomes a wrapper over `MAX_COOLDOWN`;
    - perps' and data's `from_parts` use `Duration::MAX`.
  - Tests:
    - one new test per changed crate, `the_one_retry_after_parser_settles_the_old_disagreements`, holding the I/O rows: core (padded), perps (fractional, zero, 20 digits) and data v2 (zero, `-0`, `1e300`);
    - perps' and data's zero rows `(None, secs(0))` become `(None, None)`, with their comments.
  - Ledger: re-cite (b) and (b)-zero to `retry_after.rs:29`, `:35` and `:45`. The zero mutant now needs all three edits. Prove it.
  - CLAUDE.md :176.
  - deferred-work: R4 for the release notes.
- [x] **G2 — Story 3.10: one retriable-status rule.**
  - core `error.rs:98`, perps `:78` and Binance `:103` call `class_for_status(s).is_some_and(|c| c.is_retriable())`.
  - Core's three table rows and comment :505 change, and CLAUDE.md :176's "a 408 in `Api` final" goes.
  - The commit message names each changed row: `api(408)` false → true, `api(429)` false → true, `api(600)` true → false.
  - Record the edge answers in deferred-work.
- [x] **G3 — Story 3.4 (core): what clob, relay and Binance need from core, with F's spec unchanged.**
  - `Request`:
    - `method`, `body` (via `Value`, setting `Content-Type`), `authenticator` and `with_cost`;
    - `Clone` covers them;
    - `send_raw` passes `costs` and the authenticator to `HttpClient::send`.
  - `ApiError::Sign` (`InvalidRequest`, not retriable), with its `every_variant_classifies` row and `is_retriable` arm.
  - `RequestParts::timeout`, which the loop applies per attempt, and `Hold::is_held`.
  - `RetryConfig::attempt_info`, which the loop uses.
  - `PolymarketRetryPolicy` returns `Fail` at `retries_left == 0`, keeping its hold. Re-prove F's "(a), the policy" row at its new line.
  - New tests:
    - `mock_request.rs`: `a_post_sends_its_body_serialised_once`, `an_authenticator_signs_every_attempt_of_a_request`, `a_request_s_costs_reach_the_throttle`, `a_request_parts_timeout_bounds_its_attempt`;
    - `send_loop.rs`: `a_429_with_no_retry_left_is_fail_with_its_hold`.
  - `spine-amendments/epic-3.md`: the A3-2 resolution, and the new A3-3.
- [x] **G4 — Story 3.4: clob on the one loop.**
  - New private `src/authenticator.rs` holds `L2Auth`, `L1Auth` and `L1Signed`. Clob's two `request.rs` tests move there under their own names, calling `sign` on `RequestParts`.
  - `Account` holds `Secret<Credentials>` and an `Arc`'d `L2Auth`.
  - The namespaces build `polyoxide_core::Request<T, ClobError>` with `.method`, `.body`, `.authenticator` and `.with_cost(signer_cost(..))`.
  - `impl RequestError for ClobError`; the hand-written `From<ApiError>`; and `burst_from_refused`, with the test `burst_from_refused_recovers_the_tier_and_bucket_of_every_tier` appended to `error.rs`'s tests.
  - The builder installs `ClobThrottle` and `PolymarketRetryPolicy`.
  - Delete `request.rs`.
  - Tests appended to `mock_api.rs`: `a_retried_l2_request_is_signed_on_every_attempt` and `a_retried_l1_request_is_signed_on_every_attempt`.
  - Ledger: re-prove (e) and (e)-case at their shifted test lines, and drop clob :262 from "not yet covered".
  - Removals: `polyoxide-clob module_missing: mod polyoxide_clob::request …`, `struct_missing … request::Request`, `enum_missing … request::AuthMode` (predicted; confirm against the gate).
  - CLAUDE.md: clob's loop leaves the `note_rate_limited` text.
  - [AFTER-F] Check that `ClobThrottle::observe` reads the signer headers only when the charge holds a signer layer, as clob does today. Record it if not.
- [x] **G5 — DRIFT R8: the four unlooped sends.**
  - The clob, gamma and data pings call `HttpClient::send` with a private timing authenticator; gamma's `post_json` calls `send` too.
  - Tests:
    - clob `api/health.rs` `ping_waits_on_the_shared_request_gate`;
    - `a_429_on_ping_holds_the_next_request`, appended to clob's, gamma's and data's `mock_api.rs`;
    - gamma `a_429_on_query_by_information_is_retried_and_holds`.
  - deferred-work: pings and `post_json` now retry.
- [x] **G6 — Story 3.5: relay's credentials in `Secret`.** `RelayClient`, `RelayClientBuilder` and `BuilderAccount` hold `Option<Secret<AuthConfig>>`; public types and accessors are unchanged.
- [x] **G7 — DRIFT R7: relay on the one loop.**
  - The private send helper, `RelayGetAuth { auth, sign_path }` and `RelayPostAuth { auth, extra_headers }`.
  - Auth checks run before `send`; the session-signer posts set `timeout`.
  - The builder installs `PolymarketRetryPolicy`.
  - `RelayError`: `Api(#[from] ApiError)`; `Core` and `RateLimit` go; `fn validation` serves every local site; `Classify` delegates `Api`; the `estimate_call_gas` comment.
  - Tests:
    - `error.rs`: `test_rate_limit_display` goes with its variant (8 → 7); `test_api_error_display` asserts the new variant's text; `test_from_core_api_error` matches `Api(_)`; `every_variant_classifies` rows become `Api(RateLimit)`, `Api(Api{503})` and `Api(Validation)`;
    - `mock_api.rs`: `each_relay_route_retries_a_429` and `each_relay_route_s_429_holds_the_next_request`, each over the 12 routes with a 200 ms backoff, naming the route in every assertion.
  - Ledger: drop relay's three lines and the "not yet covered" section.
  - Removals: `polyoxide-relay enum_variant_missing: variant RelayError::RateLimit …` and `… RelayError::Core …`.
  - CLAUDE.md :174, :200; deferred-work: relay's changed variants and `Display`.
- [x] **G8 — Story 3.6: Binance on the one loop.**
  - `weight.rs`:
    - `Hold::with_ceiling(MAX_COOLDOWN)` replaces `cooldown_until` and `await_cooldown`, and `in_cooldown` calls `is_held`;
    - `funding_interval` comes from `paced_interval`;
    - `MinuteCharge`, the two `LayerId`s, `From<Cost> for polyoxide_core::Cost`, and `impl Throttle for WeightBudget`.
  - New `usdm/policy.rs` holds `UsdmRetryPolicy`, with tests:
    - `a_2xx_is_done`;
    - `a_429_with_a_retry_left_holds_the_longer_of_its_wait_and_retry_after`;
    - `a_429_with_none_left_holds_its_retry_after`;
    - `a_429_with_none_left_and_no_retry_after_holds_to_the_next_minute`;
    - `a_418_fails_and_holds_its_retry_after`;
    - `a_418_without_retry_after_holds_two_minutes`;
    - `a_425_and_a_5xx_fail_with_no_hold`.
  - The builders: `Routed<T>` in `usdm/request.rs`, and the eight new builders. Delete `WeightedRequest` and its loop.
  - `impl RequestError for BinanceError`. `UsdmBuilder::build` installs the budget and the policy.
  - Test `every_usdm_on_one_budget_is_held_by_one_ban`, appended to `mock_api.rs`.
  - Core's loop: the hold-without-retry WARN becomes `Status <code> on <path>, not retried: every request held <ms>ms`. F's `a_hold_with_no_retry_left_warns` asserts the new text, and the commit message names it. The `Retriable status` line is untouched.
  - Ledger: move (a)-Binance to the policy's no-retry arm (mutant: "hold only when `retries_left > 0`"; tests :616, :652), and (k) to the 418 arm (mutants `hold: None` → :409, and `unwrap_or(Duration::ZERO)` → `a_418_without_retry_after_holds_two_minutes`). Prove both.
  - Removals: `polyoxide-binance struct_missing: struct polyoxide_binance::usdm::request::WeightedRequest …`, plus the `usdm::WeightedRequest` path if printed separately.
  - CLAUDE.md :292-301; deferred-work: the new builders and the log lines.
- [x] **G9 — Story 3.6: the governor fence.** `.github/scripts/tests/test_dependency_fences.py`:
  - `test_no_venue_crate_depends_on_governor`, on the real `cargo metadata` through `publish_order.cargo_metadata`;
  - `test_the_fence_names_a_venue_crate_that_depends_on_governor`, on synthetic metadata.
- [x] **G10 — The transitional methods' tests on public API.**
  - Rewrite the 17 test callers per the Decisions, on G3's `attempt_info` and the policy's `Fail`. Names and asserted values stay.
  - Report the counts.
- [x] **G11 — Remove `should_retry`, `note_rate_limited` and `acquire_rate_limit`.**
  - Move `should_retry`'s "deliberately narrow" rationale to `PolymarketRetryPolicy`'s docs, unless F already did.
  - Re-cite and re-prove (b) at the shifted `client.rs` tests.
  - Removals: `polyoxide-core inherent_method_missing: HttpClient::{should_retry,note_rate_limited,acquire_rate_limit} (src/client.rs)` (three keys).
  - CLAUDE.md's last mention of the three.
- [x] **Deferred work, in the commit that causes each item** (in `deferred-work.md`, with `source_spec` set to this spec).
  - For 4.11's release notes and prader, as F does:
    - R4's answers: a zero `Retry-After` reads as none on perps and data v2 and in Python; perps reads fractions; core honours padded values; a huge data v2 value no longer panics;
    - H12's three edge answers;
    - clob's namespaces return `polyoxide_core::Request<T, ClobError>`, `request` and `AuthMode` are gone, and `ApiError` gains `Sign`;
    - `Secret` on the credential holders, with the public fields unchanged;
    - R8: the pings time their last attempt, and the pings and gamma's `post_json` now retry 429 and 425 and hold;
    - R7: `RelayError::Api` wraps `ApiError`, HTTP failures are classed by status, the `Display` prefix is now `Validation error:`, transport failures are `Api(Network)`, and `Core` and `RateLimit` are gone;
    - Binance: the eight per-route builders replace `WeightedRequest`, its 429 wait is the longer of two jitter draws, its retry and hold lines move to `polyoxide_core` (the hold line reading `not retried`), and core's `Request failed` ERROR now logs its errors;
    - clob's and relay's retry WARNs move to the `polyoxide_core` target.
  - Follow-ups:
    - make `WindowQuotaTable`'s clock injectable, then move Binance's funding bucket onto it, when Binance's tables move into its venue crate in S2 (the AC 3.6 deviation);
    - remove `HttpClient::acquire_concurrency` once the eleven tests that observe the permit through it can observe it another way (the deviation from F's note).

**Acceptance Criteria:**
- Given clob, relay, Binance, gamma and data, when they send, then every request goes through `HttpClient::send`, and no crate but core sends through `HttpClient`'s reqwest client. Relay's alloy gas estimation is the one send outside the loop.
- Given clob's, relay's, Binance's, gamma's, data's and perps' existing suites, when they run, then each passes with its names and asserted values unchanged, except the rows this spec names (R4's two zero rows, H12's three core rows, R7's relay error tests). Each I/O row has a test.
- Given each `docs/MUTANTS.md` row, rows (a)-Binance, (b), (e) and the new (k) included, when its mutant is applied, then its tests fail. `test_mutants_ledger.py` passes, and "not yet covered" is gone.
- Given the removal gate against `v0.38.1`, when it runs, then it reports only F's five keys and G's listed ones.
- Given a venue crate that adds `governor`, when the scripts job runs, then `test_dependency_fences.py` fails naming it.
- Given clob and relay, when an `Account`, `Clob`, `RelayClient` or `BuilderAccount` is printed with `{:?}`, then no key, secret or passphrase appears.

## Design Notes

G's additions to F's surface:

```rust
impl<T, E> Request<T, E> {
    pub fn method(self, method: Method) -> Self;
    pub fn authenticator(self, auth: Arc<DynAuthenticator<'static>>) -> Self;
    pub fn with_cost(self, cost: Cost) -> Self;
}
impl<T, E: From<ApiError>> Request<T, E> {
    pub fn body<B: Serialize + ?Sized>(self, body: &B) -> Result<Self, E>; // via serde_json::Value
}
// ApiError::Sign(Box<dyn std::error::Error + Send + Sync>)  — InvalidRequest, not retriable
// RequestParts { .., timeout: Option<Duration> }
// Hold::is_held(&self) -> bool
// RetryConfig::attempt_info(&self, attempt: u32) -> AttemptInfo
// PolymarketRetryPolicy: 429 with retries_left == 0 → Decision { outcome: Fail, hold: Some(retry_delay(0)) }
```

Clob's error mapping, which keeps the burst-capacity and signing tests on their variants (`burst_from_refused` is clob's own, in `error.rs`):

```rust
impl From<ApiError> for ClobError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Refused(r) => burst_from_refused(&r)
                .map_or(Self::Api(ApiError::Refused(r)), Self::BurstCapacityExceeded),
            ApiError::Sign(e) => e.downcast::<ClobError>().map_or_else(|e| Self::Api(ApiError::Sign(e)), |e| *e),
            other => Self::Api(other),
        }
    }
}
```

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo test -p <crate> --all-features -j 4` for core, venue, clob, relay, binance, gamma, data, perps and test-support -- expected: green.
- `cargo test -p <crate> --all-features <target> -- --list | grep -c ': test$'` before and after each commit -- expected, against F's landed counts:
  - core lib +1, `mock_request` 10 → 14, `send_loop` +1;
  - clob lib net +2 (`request` 2 → 0, `authenticator` 0 → 2, `api::health` 3 → 4, `error` +1), clob `mock_api` 106 → 109;
  - relay lib `error` 8 → 7, relay `mock_api` 43 → 45;
  - binance lib +7 (policy), weight 15 and error 10 unchanged, binance `mock_api` 25 → 26;
  - gamma `mock_api` 42 → 44, data `mock_api` 38 → 39;
  - data lib +1, perps lib +1;
  - `.github/scripts` +2.
- `cargo clippy --workspace --all-targets --all-features -j 4 -- -D warnings`, then `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean.
- `cargo hack check -p polyoxide-core -p polyoxide-clob -p polyoxide-relay -p polyoxide-binance --each-feature --no-dev-deps`, and `cargo +1.91 check --workspace` if installed -- expected: clean.
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: green (`test_mutants_ledger`, `test_dependency_fences`, `test_classify_coverage`).
- `cd polyoxide-py && uv sync --reinstall-package polyoxide && uv run pytest tests/ -q` -- expected: green. The v2 zero `retry_after` is `None`, and no test asserts `0.0`.
- Each `MUTANTS.md` row whose line or test moved, run with its mutant and then without -- expected: fail, then pass.
- `python3 scripts/api_removals.py check --baseline v0.38.1`, once at the end, then `rm -rf target/semver-checks` -- expected: only listed keys.

## Implementation Notes

G0–G11 are `3aeddc2`, `d63ce46`, `a0d2771`, `d8cc425`, `925d8da`, `f86a25d`, `cf0892a`, `207c70f`, `f52745d`, `590a93f`, `6180817` and `01c86e3`. The matrix-audit tests are the commit after them.

- **Verification, 2026-10-09, on a fresh `target/` with the local Rust 1.95.0.** `cargo fmt --check`, clippy with `-D warnings`, the workspace's tests (2,259 passed, 0 failed, 178 ignored), `cargo doc` with `-D warnings`, `cargo hack --each-feature`, `.github/scripts` (902 passed) and `polyoxide-py` (326 passed) are all green. MSRV 1.91 was not checked: no 1.91 toolchain is installed locally.
- **Counts.** They match the Verification list, except where G added a test it does not name or F's review had already moved the baseline:
  - relay `mock_api` 43 → 46, which adds `a_relay_503_is_not_retried_and_a_425_is`;
  - clob `mock_api` 106 → 110, which adds G5's ping test;
  - gamma `mock_api` 43 → 45 and data `mock_api` 39 → 40, because F's review had added one test to each.
- **Matrix audit.** 21 rows are asserted end to end. Four partial rows gained a test:
  - R4 zero in Python: `test_rate_limited_is_a_rate_limit_error` asserts `retry_after is None`;
  - Binance 425 and 5xx: `a_425_and_a_5xx_are_not_retried`, in binance's `mock_api.rs`;
  - relay's local refusal: `a_refused_relay_call_sends_nothing`, which matches `Api(Validation)` and asserts nothing was sent.
- **Partial rows left as they are,** each asserted in parts that compose:
  - **Clob 429 with a retry left.** The WARN and the two-layer hold are asserted generically: core's `a_retry_warns_once_under_polyoxide_core` and `polymarket_throttle.rs`'s `a_429_holds_both_layers` on `clob_throttle`. Clob's own test asserts the re-signing.
  - **Clob batch above burst.** The end-to-end test asserts the cost, the capacity and that nothing is sent. `burst_from_refused_recovers_the_tier_and_bucket_of_every_tier` asserts the tier and the bucket.
  - **Relay session signer, 300 s.** `session_signer_requests_wait_five_minutes_like_py_sdk` pins the constant, and core's `a_request_parts_timeout_bounds_its_attempt` pins the per-attempt bound. No relay test shows that `post_json` sets `parts.timeout`: `RelayClientBuilder` has no timeout setter, so an end-to-end proof would wait out the 30 s client default.
  - **The hold-without-retry WARN for a Binance 418.** The WARN branch in `send.rs` does not depend on the policy, and core's `a_hold_with_no_retry_left_warns` asserts it for a 429.

## Spec Change Log

## Review Triage Log
